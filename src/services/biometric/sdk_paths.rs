use std::{
    collections::VecDeque,
    fs,
    os::windows::fs::MetadataExt as _,
    path::{Path, PathBuf},
};

use super::{BiometricError, storage::verify_dll_matches_architecture};

const FDX_DLL: &str = "sgfplib.dll";
const WSQ_DLL: &str = "sgwsqlib.dll";
const SECUSEARCH_DLL: &str = "secusearchapi.dll";
const FILE_ATTRIBUTE_REPARSE_POINT: u32 = 0x400;

#[derive(Debug)]
pub(crate) struct SdkPaths {
    pub fdx: PathBuf,
    pub secusearch: PathBuf,
}

pub(crate) fn detect_sdk_paths(folder: &Path) -> Result<SdkPaths, BiometricError> {
    let mut directories = VecDeque::from([folder.to_path_buf()]);
    let mut fdx = None;
    let mut secusearch = None;
    let mut fdx_error = None;
    let mut secusearch_error = None;

    while let Some(dir) = directories.pop_front() {
        let entries = match fs::read_dir(&dir) {
            Ok(entries) => entries,
            Err(err) if dir == folder => {
                return Err(BiometricError::Device(format!(
                    "No se pudo abrir la carpeta {}: {err}",
                    dir.display()
                )));
            }
            Err(_) => continue,
        };

        if fdx.is_none() {
            let path = dir.join(FDX_DLL);
            if path.is_file() {
                match verify_dll_matches_architecture(&path) {
                    Ok(()) => match verify_dll_matches_architecture(&dir.join(WSQ_DLL)) {
                        Ok(()) => fdx = Some(path),
                        Err(err) => fdx_error = Some(err),
                    },
                    Err(err) => {
                        fdx_error.get_or_insert(err);
                    }
                }
            }
        }
        if secusearch.is_none() {
            let path = dir.join(SECUSEARCH_DLL);
            if path.is_file() {
                match verify_dll_matches_architecture(&path) {
                    Ok(()) => secusearch = Some(path),
                    Err(err) => {
                        secusearch_error.get_or_insert(err);
                    }
                }
            }
        }
        if fdx.is_some() && secusearch.is_some() {
            break;
        }

        // Mantiene la búsqueda dentro de la carpeta y evita ciclos en uniones de Windows.
        let mut subdirectories: Vec<_> = entries
            .filter_map(Result::ok)
            .filter(|entry| {
                entry.metadata().is_ok_and(|metadata| {
                    metadata.is_dir()
                        && metadata.file_attributes() & FILE_ATTRIBUTE_REPARSE_POINT == 0
                })
            })
            .map(|entry| entry.path())
            .collect();
        subdirectories.sort_unstable();
        directories.extend(subdirectories);
    }

    match (fdx, secusearch) {
        (Some(fdx), Some(secusearch)) => Ok(SdkPaths { fdx, secusearch }),
        (fdx, secusearch) => {
            let mut errors = Vec::new();
            if fdx.is_none() {
                errors.push(fdx_error.map_or_else(
                    || format!("No se encontró {FDX_DLL} en la carpeta seleccionada."),
                    |err| err.message(),
                ));
            }
            if secusearch.is_none() {
                errors.push(secusearch_error.map_or_else(
                    || format!("No se encontró {SECUSEARCH_DLL} en la carpeta seleccionada."),
                    |err| err.message(),
                ));
            }
            Err(BiometricError::Device(errors.join(" ")))
        }
    }
}

#[cfg(all(test, target_arch = "x86_64"))]
mod tests {
    use std::{
        env, fs,
        path::{Path, PathBuf},
        time::{SystemTime, UNIX_EPOCH},
    };

    use super::{FDX_DLL, SECUSEARCH_DLL, WSQ_DLL, detect_sdk_paths};

    struct Fixture(PathBuf);

    impl Drop for Fixture {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    fn write_pe_header(path: &Path, machine: u16) {
        let mut bytes = [0; 0x46];
        bytes[..2].copy_from_slice(b"MZ");
        bytes[0x3C] = 0x40;
        bytes[0x40..0x44].copy_from_slice(b"PE\0\0");
        bytes[0x44..0x46].copy_from_slice(&machine.to_le_bytes());
        fs::write(path, bytes).unwrap();
    }

    #[test]
    fn finds_nested_x64_sdks_and_requires_wsq_beside_fdx() {
        let unique = SystemTime::now().duration_since(UNIX_EPOCH).unwrap();
        let folder = env::temp_dir().join(format!(
            "dylan-secugen-paths-{}-{}",
            std::process::id(),
            unique.as_nanos()
        ));
        fs::create_dir(&folder).unwrap();
        let fixture = Fixture(folder);
        let fdx_dir = fixture.0.join("FDx SDK Pro").join("bin").join("x64");
        let secusearch_dir = fixture.0.join("SecuSearch Pro").join("bin");
        fs::create_dir_all(&fdx_dir).unwrap();
        fs::create_dir_all(&secusearch_dir).unwrap();

        write_pe_header(&fixture.0.join(FDX_DLL), 0x014C);
        write_pe_header(&fixture.0.join(SECUSEARCH_DLL), 0x014C);
        write_pe_header(&fdx_dir.join(FDX_DLL), 0x8664);
        write_pe_header(&secusearch_dir.join(SECUSEARCH_DLL), 0x8664);
        assert!(
            detect_sdk_paths(&fixture.0)
                .unwrap_err()
                .message()
                .contains(WSQ_DLL)
        );

        write_pe_header(&fdx_dir.join(WSQ_DLL), 0x8664);
        let detected = detect_sdk_paths(&fixture.0).unwrap();
        assert_eq!(detected.fdx, fdx_dir.join(FDX_DLL));
        assert_eq!(detected.secusearch, secusearch_dir.join(SECUSEARCH_DLL));
    }
}
