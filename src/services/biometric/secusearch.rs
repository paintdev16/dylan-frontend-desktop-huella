#![cfg(target_os = "windows")]

use std::{
    ffi::{CStr, CString, c_char},
    path::Path,
    ptr,
};

use libloading::Library;

use super::{
    BiometricError,
    storage::{TempPlaintextFile, decrypt_file, encrypt_file, verify_dll_matches_architecture},
};

/// Tamaño fijo de una plantilla nativa SG400 en SecuSearch Pro (`SecuSearchDef.h`).
pub const SG400_TEMPLATE_SIZE: usize = 400;
pub type Sg400Template = [u8; SG400_TEMPLATE_SIZE];

/// Constantes oficiales tomadas de `SecuSearchDef.h` y `SecuSearchErr.h`.
#[allow(dead_code)]
pub const SS_TEMPLATE_ANSI378: u32 = 2;
pub const CONF_LEVEL_NORMAL: u32 = 5;

pub const FPS_ERROR_NONE: i32 = 0;
pub const FPS_ERROR_DATA_EXIST: i32 = 105;
pub const FPS_ERROR_IDENTIFICATION_FAIL: i32 = 202;
pub const FPS_ERROR_DATA_NOT_FOUND: i32 = 2103;

#[repr(C)]
struct SseParam {
    concurrency: i32,
    candidate_count: i32,
    license_file: *mut c_char,
    enable_rotation: i32,
    license_token: *mut u8,
}

type FnGetVersion = unsafe extern "C" fn() -> *const c_char;
type FnInitializeEngine = unsafe extern "C" fn(*const SseParam) -> i32;
type FnTerminateEngine = unsafe extern "C" fn() -> i32;
type FnExtractTemplate = unsafe extern "C" fn(*const u8, u32, u32, *mut u8) -> i32;
type FnRegisterFp = unsafe extern "C" fn(*const u8, u32) -> i32;
type FnRemoveFp = unsafe extern "C" fn(u32) -> i32;
type FnIdentifyFp = unsafe extern "C" fn(*const u8, u32, *mut u32) -> i32;
type FnFileOp = unsafe extern "C" fn(*const c_char) -> i32;
type FnClearFpDb = unsafe extern "C" fn() -> i32;
type FnGetFpCount = unsafe extern "C" fn(*mut usize) -> i32;

fn map_sse_error(code: i32, operation: &str) -> BiometricError {
    BiometricError::Device(format!("{operation}: código SecuSearch {code}"))
}

/// Motor nativo SecuSearch Pro cargado dinámicamente vía `libloading`.
/// Debe crearse y utilizarse exclusivamente desde el hilo dedicado al lector biométrico.
pub struct SecuSearchEngine {
    _library: Library,
    _license: CString,
    version: String,
    terminate: FnTerminateEngine,
    #[allow(dead_code)]
    extract: FnExtractTemplate,
    register: FnRegisterFp,
    remove: FnRemoveFp,
    identify: FnIdentifyFp,
    save: FnFileOp,
    load: FnFileOp,
    clear: FnClearFpDb,
    count: FnGetFpCount,
    active: bool,
}

impl SecuSearchEngine {
    pub fn load(dll_path: &Path) -> Result<Self, BiometricError> {
        verify_dll_matches_architecture(dll_path)?;

        // SAFETY: Las firmas FFI coinciden con `SecuSearchAPI.h` y `SecuSearchDef.h` (x64, __cdecl).
        unsafe {
            let library = Library::new(dll_path).map_err(|err| {
                BiometricError::Device(format!(
                    "No se pudo cargar SecuSearch ({}): {err}",
                    dll_path.display()
                ))
            })?;

            let get_version: FnGetVersion = *library
                .get(b"SecuSearch_GetVersion\0")
                .map_err(|err| BiometricError::Device(format!("Símbolo faltante: {err}")))?;
            let initialize: FnInitializeEngine = *library
                .get(b"SecuSearch_InitializeEngine\0")
                .map_err(|err| BiometricError::Device(format!("Símbolo faltante: {err}")))?;
            let terminate: FnTerminateEngine = *library
                .get(b"SecuSearch_TerminateEngine\0")
                .map_err(|err| BiometricError::Device(format!("Símbolo faltante: {err}")))?;
            let extract: FnExtractTemplate = *library
                .get(b"SecuSearch_ExtractTemplate\0")
                .map_err(|err| BiometricError::Device(format!("Símbolo faltante: {err}")))?;
            let register: FnRegisterFp = *library
                .get(b"SecuSearch_RegisterFP\0")
                .map_err(|err| BiometricError::Device(format!("Símbolo faltante: {err}")))?;
            let remove: FnRemoveFp = *library
                .get(b"SecuSearch_RemoveFP\0")
                .map_err(|err| BiometricError::Device(format!("Símbolo faltante: {err}")))?;
            let identify: FnIdentifyFp = *library
                .get(b"SecuSearch_IdentifyFP\0")
                .map_err(|err| BiometricError::Device(format!("Símbolo faltante: {err}")))?;
            let save: FnFileOp = *library
                .get(b"SecuSearch_SaveFPDB\0")
                .map_err(|err| BiometricError::Device(format!("Símbolo faltante: {err}")))?;
            let load: FnFileOp = *library
                .get(b"SecuSearch_LoadFPDB\0")
                .map_err(|err| BiometricError::Device(format!("Símbolo faltante: {err}")))?;
            let clear: FnClearFpDb = *library
                .get(b"SecuSearch_ClearFPDB\0")
                .map_err(|err| BiometricError::Device(format!("Símbolo faltante: {err}")))?;
            let count: FnGetFpCount = *library
                .get(b"SecuSearch_GetFPCount\0")
                .map_err(|err| BiometricError::Device(format!("Símbolo faltante: {err}")))?;

            let version_ptr = get_version();
            if version_ptr.is_null() {
                return Err(BiometricError::Device(
                    "SecuSearch no reportó su versión de biblioteca.".into(),
                ));
            }
            let version = CStr::from_ptr(version_ptr).to_string_lossy().into_owned();

            let license = CString::new("")
                .map_err(|_| BiometricError::Device("Ruta de licencia inválida.".into()))?;
            let params = SseParam {
                concurrency: 0,
                candidate_count: 10,
                license_file: license.as_ptr() as *mut c_char,
                enable_rotation: 1,
                license_token: ptr::null_mut(),
            };

            let init_code = initialize(&params);
            if init_code != FPS_ERROR_NONE {
                return Err(map_sse_error(init_code, "Inicializar SecuSearch"));
            }

            Ok(Self {
                _library: library,
                _license: license,
                version,
                terminate,
                extract,
                register,
                remove,
                identify,
                save,
                load,
                clear,
                count,
                active: true,
            })
        }
    }

    pub fn version(&self) -> &str {
        &self.version
    }

    /// Extrae una plantilla SG400 de 400 bytes desde un búfer ANSI 378 confiable.
    #[allow(dead_code)]
    pub fn extract_ansi378(
        &mut self,
        ansi_bytes: &[u8],
        view_index: u32,
    ) -> Result<Sg400Template, BiometricError> {
        if ansi_bytes.is_empty() {
            return Err(BiometricError::Device("Plantilla ANSI 378 vacía.".into()));
        }
        let mut output = [0u8; SG400_TEMPLATE_SIZE];
        // SAFETY: `output` tiene capacidad exacta `SG400_TEMPLATE_SIZE` (400 bytes).
        let code = unsafe {
            (self.extract)(
                ansi_bytes.as_ptr(),
                SS_TEMPLATE_ANSI378,
                view_index,
                output.as_mut_ptr(),
            )
        };
        if code != FPS_ERROR_NONE {
            return Err(map_sse_error(code, "Extraer plantilla SG400"));
        }
        Ok(output)
    }

    /// Registra o reemplaza una plantilla SG400 asociada al identificador canónico.
    pub fn register_or_replace(
        &mut self,
        template: &Sg400Template,
        template_id: u32,
    ) -> Result<(), BiometricError> {
        // SAFETY: `template` apunta a 400 bytes válidos SG400.
        let code = unsafe { (self.register)(template.as_ptr(), template_id) };
        if code == FPS_ERROR_DATA_EXIST {
            self.remove(template_id)?;
            let retry = unsafe { (self.register)(template.as_ptr(), template_id) };
            if retry != FPS_ERROR_NONE {
                return Err(map_sse_error(retry, "Re-registrar plantilla"));
            }
            return Ok(());
        }
        if code != FPS_ERROR_NONE {
            return Err(map_sse_error(code, "Registrar plantilla"));
        }
        Ok(())
    }

    pub fn remove(&mut self, template_id: u32) -> Result<(), BiometricError> {
        let code = unsafe { (self.remove)(template_id) };
        if code == FPS_ERROR_NONE || code == FPS_ERROR_DATA_NOT_FOUND {
            Ok(())
        } else {
            Err(map_sse_error(code, "Eliminar plantilla"))
        }
    }

    /// Identifica 1:N. Devuelve `Ok(Some(id))` si coincide, `Ok(None)` si no se reconoce.
    pub fn identify(
        &mut self,
        template: &Sg400Template,
        security_level: u32,
    ) -> Result<Option<u32>, BiometricError> {
        let mut matched_id = u32::MAX;
        let code = unsafe { (self.identify)(template.as_ptr(), security_level, &mut matched_id) };
        if code == FPS_ERROR_NONE {
            Ok(Some(matched_id))
        } else if code == FPS_ERROR_IDENTIFICATION_FAIL || code == FPS_ERROR_DATA_NOT_FOUND {
            Ok(None)
        } else {
            Err(map_sse_error(code, "Identificar huella 1:N"))
        }
    }

    pub fn count(&mut self) -> Result<usize, BiometricError> {
        let mut count = 0_usize;
        let code = unsafe { (self.count)(&mut count) };
        if code != FPS_ERROR_NONE {
            return Err(map_sse_error(code, "Consultar cantidad de plantillas"));
        }
        Ok(count)
    }

    pub fn clear(&mut self) -> Result<(), BiometricError> {
        let code = unsafe { (self.clear)() };
        if code != FPS_ERROR_NONE {
            return Err(map_sse_error(code, "Limpiar base de plantillas"));
        }
        Ok(())
    }

    fn save_plaintext(&mut self, path: &Path) -> Result<(), BiometricError> {
        let c_path = CString::new(path.to_string_lossy().as_bytes())
            .map_err(|_| BiometricError::Storage("Ruta temporal inválida.".into()))?;
        let code = unsafe { (self.save)(c_path.as_ptr()) };
        if code != FPS_ERROR_NONE {
            return Err(map_sse_error(code, "Guardar base temporal de SecuSearch"));
        }
        Ok(())
    }

    fn load_plaintext(&mut self, path: &Path) -> Result<(), BiometricError> {
        let c_path = CString::new(path.to_string_lossy().as_bytes())
            .map_err(|_| BiometricError::Storage("Ruta temporal inválida.".into()))?;
        let code = unsafe { (self.load)(c_path.as_ptr()) };
        if code != FPS_ERROR_NONE {
            return Err(map_sse_error(code, "Cargar base temporal en SecuSearch"));
        }
        Ok(())
    }

    /// Guarda la base actual en un archivo temporal que se cifra con AES-256-GCM y se limpia al salir.
    pub fn save_encrypted_backup(
        &mut self,
        destination: &Path,
        key: &[u8; 32],
    ) -> Result<usize, BiometricError> {
        let temp = TempPlaintextFile::new("sse-save")?;
        self.save_plaintext(temp.path())?;
        encrypt_file(temp.path(), destination, key)?;
        self.count()
    }

    /// Descifra un respaldo AES-256-GCM en un archivo temporal efímero, lo carga en SecuSearch y limpia el temporal.
    pub fn restore_encrypted_backup(
        &mut self,
        source: &Path,
        key: &[u8; 32],
    ) -> Result<usize, BiometricError> {
        let temp = TempPlaintextFile::new("sse-restore")?;
        decrypt_file(source, temp.path(), key)?;
        self.clear()?;
        self.load_plaintext(temp.path())?;
        self.count()
    }

    pub fn close(&mut self) -> Result<(), BiometricError> {
        if self.active {
            self.active = false;
            let code = unsafe { (self.terminate)() };
            if code != FPS_ERROR_NONE {
                return Err(map_sse_error(code, "Finalizar motor SecuSearch"));
            }
        }
        Ok(())
    }
}

impl Drop for SecuSearchEngine {
    fn drop(&mut self) {
        let _ = self.close();
    }
}

#[cfg(test)]
mod tests {
    use std::{env, fs, path::PathBuf};

    use super::{CONF_LEVEL_NORMAL, SecuSearchEngine};
    use crate::services::biometric::storage::parse_hex_key;

    #[test]
    #[ignore = "requiere secusearchapi.dll local y ejecución fuera de sandbox Windows (SeLockMemoryPrivilege)"]
    fn validates_secusearch_engine_and_aes_256_gcm_backup_with_test_data() {
        let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        let dll = root.join("secugen/SecuSearch_Pro_SDK_v3.5.1/bin/secusearchapi.dll");
        let sample = root.join("secugen/SecuSearch_Pro_SDK_v3.5.1/test_data/ansi378_2views.bin");

        if !dll.is_file() || !sample.is_file() {
            // En runners de CI donde el SDK propietario no está presente, la prueba de integración local se omite.
            return;
        }

        let key = parse_hex_key("a1b2c3d4e5f60718293a4b5c6d7e8f90a1b2c3d4e5f60718293a4b5c6d7e8f90")
            .expect("valid test key");
        let ansi_bytes = fs::read(&sample).expect("read ansi378 test sample");

        let backup_path = env::temp_dir().join("dylan-secusearch-validation-test.fpdb.enc");
        let _ = fs::remove_file(&backup_path);

        let version: String;
        let matched_id: u32;
        {
            let mut engine = SecuSearchEngine::load(&dll).expect("load secusearchapi.dll x64");
            version = engine.version().to_string();
            assert!(!version.is_empty());

            let template_v0 = engine
                .extract_ansi378(&ansi_bytes, 0)
                .expect("extract view 0 to SG400");
            let template_v1 = engine
                .extract_ansi378(&ansi_bytes, 1)
                .expect("extract view 1 to SG400");

            // Registrar 200 registros de prueba tal como especifica la skill comedor-secugen.
            let total = 200_u32;
            for offset in 0..total {
                engine
                    .register_or_replace(&template_v0, 10_000 + offset)
                    .expect("register test template");
            }
            assert_eq!(engine.count().expect("count before backup"), total as usize);

            let saved_count = engine
                .save_encrypted_backup(&backup_path, &key)
                .expect("save encrypted AES-256-GCM backup");
            assert_eq!(saved_count, total as usize);
            assert!(backup_path.is_file());

            // Limpiar motor en memoria y verificar que sin restaurar está vacío.
            engine.clear().expect("clear fpdb");
            assert_eq!(engine.count().expect("count after clear"), 0);

            // Restaurar desde respaldo cifrado y comprobar recuperación exacta + identificación posterior.
            let restored_count = engine
                .restore_encrypted_backup(&backup_path, &key)
                .expect("restore encrypted AES-256-GCM backup");
            assert_eq!(restored_count, total as usize);

            let matched = engine
                .identify(&template_v0, CONF_LEVEL_NORMAL)
                .expect("identify registered view 0 after restore");
            matched_id = matched.expect("expected match in restored 200 test records");
            assert!(
                (10_000..10_000 + total).contains(&matched_id),
                "unexpected matched id: {matched_id}"
            );

            // Vista distinta (view 1) no registrada debe devolver None (FPS_ERROR_IDENTIFICATION_FAIL).
            let non_match = engine
                .identify(&template_v1, CONF_LEVEL_NORMAL)
                .expect("identify unregistered view 1");
            assert_eq!(non_match, None);

            // Clave incorrecta debe rechazar la restauración.
            assert!(
                engine
                    .restore_encrypted_backup(&backup_path, &[0xFF; 32])
                    .is_err()
            );
        }

        eprintln!(
            "[SecuSearch validation] DLL version={version} | backup=200 | restored=200 | matched_id={matched_id} | non_match=None"
        );
        let _ = fs::remove_file(&backup_path);
    }
}
