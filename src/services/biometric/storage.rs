use std::{
    env, fs,
    path::{Path, PathBuf},
    sync::atomic::{AtomicU64, Ordering},
};

use ring::{
    aead::{AES_256_GCM, Aad, LessSafeKey, NONCE_LEN, Nonce, UnboundKey},
    rand::{SecureRandom, SystemRandom},
};

use super::BiometricError;

/// Cabecera de versión compatible con el formato de respaldo cifrado `SGENC001` (AES-256-GCM).
pub const BACKUP_MAGIC: &[u8; 8] = b"SGENC001";
pub const BACKUP_KEY_ENV: &str = "SECUGEN_BACKUP_KEY";

static TEMP_SEQ: AtomicU64 = AtomicU64::new(1);

/// Resuelve la ubicación por defecto del respaldo cifrado de plantillas,
/// fuera del repositorio y fuera del directorio de instalación de Velopack.
pub fn default_backup_path() -> PathBuf {
    #[cfg(target_os = "windows")]
    {
        if let Some(app_data) = env::var_os("APPDATA").filter(|v| !v.is_empty()) {
            return PathBuf::from(app_data)
                .join("DylanComedor")
                .join("biometric")
                .join("secusearch.fpdb.enc");
        }
        if let Some(profile) = env::var_os("USERPROFILE").filter(|v| !v.is_empty()) {
            return PathBuf::from(profile)
                .join(".dylan-comedor")
                .join("biometric")
                .join("secusearch.fpdb.enc");
        }
        PathBuf::from("secusearch.fpdb.enc")
    }

    #[cfg(not(target_os = "windows"))]
    {
        if let Some(xdg) = env::var_os("XDG_DATA_HOME").filter(|v| !v.is_empty()) {
            return PathBuf::from(xdg)
                .join("dylan-comedor")
                .join("biometric")
                .join("secusearch.fpdb.enc");
        }
        if let Some(home) = env::var_os("HOME").filter(|v| !v.is_empty()) {
            return PathBuf::from(home)
                .join(".local")
                .join("share")
                .join("dylan-comedor")
                .join("biometric")
                .join("secusearch.fpdb.enc");
        }
        PathBuf::from("secusearch.fpdb.enc")
    }
}

/// Parsea una clave simétrica AES-256 en 64 caracteres hexadecimales (32 bytes).
pub fn parse_hex_key(raw: &str) -> Result<[u8; 32], BiometricError> {
    let trimmed = raw.trim();
    if trimmed.len() != 64 {
        return Err(BiometricError::Storage(
            "La clave de respaldo biométrico debe contener exactamente 32 bytes (64 caracteres hex)."
                .into(),
        ));
    }

    let mut key = [0u8; 32];
    let bytes = trimmed.as_bytes();
    for (index, slot) in key.iter_mut().enumerate() {
        let hi = decode_nibble(bytes[index * 2])?;
        let lo = decode_nibble(bytes[index * 2 + 1])?;
        *slot = (hi << 4) | lo;
    }
    Ok(key)
}

fn decode_nibble(byte: u8) -> Result<u8, BiometricError> {
    match byte {
        b'0'..=b'9' => Ok(byte - b'0'),
        b'a'..=b'f' => Ok(byte - b'a' + 10),
        b'A'..=b'F' => Ok(byte - b'A' + 10),
        _ => Err(BiometricError::Storage(
            "La clave de respaldo biométrico contiene caracteres no hexadecimales.".into(),
        )),
    }
}

/// Carga la clave AES-256 desde la variable de entorno `SECUGEN_BACKUP_KEY`.
/// Nunca registra el valor de la clave en logs ni la persiste en `preferences.json`.
pub fn load_backup_key_from_env() -> Result<[u8; 32], BiometricError> {
    let value = env::var(BACKUP_KEY_ENV).map_err(|_| {
        BiometricError::Storage(format!(
            "No está configurada la variable de entorno {BACKUP_KEY_ENV} para cifrar o restaurar respaldos."
        ))
    })?;
    parse_hex_key(&value)
}

pub fn has_backup_key_configured() -> bool {
    load_backup_key_from_env().is_ok()
}

/// Cifra bytes con AES-256-GCM generando un nonce aleatorio de 12 bytes por operación.
pub fn encrypt_bytes(plaintext: &[u8], key: &[u8; 32]) -> Result<Vec<u8>, BiometricError> {
    let unbound = UnboundKey::new(&AES_256_GCM, key)
        .map_err(|_| BiometricError::Storage("Clave AES-256 inválida.".into()))?;
    let sealing_key = LessSafeKey::new(unbound);

    let mut nonce_bytes = [0u8; NONCE_LEN];
    SystemRandom::new()
        .fill(&mut nonce_bytes)
        .map_err(|_| BiometricError::Storage("No se pudo generar un nonce aleatorio.".into()))?;
    let nonce = Nonce::assume_unique_for_key(nonce_bytes);

    let mut in_out = plaintext.to_vec();
    sealing_key
        .seal_in_place_append_tag(nonce, Aad::empty(), &mut in_out)
        .map_err(|_| BiometricError::Storage("No se pudo cifrar el respaldo biométrico.".into()))?;

    let mut output = Vec::with_capacity(BACKUP_MAGIC.len() + NONCE_LEN + in_out.len());
    output.extend_from_slice(BACKUP_MAGIC);
    output.extend_from_slice(&nonce_bytes);
    output.extend_from_slice(&in_out);
    Ok(output)
}

/// Descifra y verifica integridad autenticada (GCM) de un respaldo `SGENC001`.
pub fn decrypt_bytes(data: &[u8], key: &[u8; 32]) -> Result<Vec<u8>, BiometricError> {
    let min_size = BACKUP_MAGIC.len() + NONCE_LEN + AES_256_GCM.tag_len();
    if data.len() < min_size {
        return Err(BiometricError::Storage(
            "El archivo de respaldo cifrado está incompleto.".into(),
        ));
    }
    if &data[..BACKUP_MAGIC.len()] != BACKUP_MAGIC {
        return Err(BiometricError::Storage(
            "Formato de respaldo biométrico no reconocido.".into(),
        ));
    }

    let nonce_start = BACKUP_MAGIC.len();
    let cipher_start = nonce_start + NONCE_LEN;
    let nonce_bytes: [u8; NONCE_LEN] = data[nonce_start..cipher_start]
        .try_into()
        .map_err(|_| BiometricError::Storage("Nonce inválido en el respaldo cifrado.".into()))?;

    let unbound = UnboundKey::new(&AES_256_GCM, key)
        .map_err(|_| BiometricError::Storage("Clave AES-256 inválida.".into()))?;
    let opening_key = LessSafeKey::new(unbound);

    let mut buf = data[cipher_start..].to_vec();
    let plaintext = opening_key
        .open_in_place(
            Nonce::assume_unique_for_key(nonce_bytes),
            Aad::empty(),
            &mut buf,
        )
        .map_err(|_| {
            BiometricError::Storage(
                "No se pudo descifrar el respaldo: clave incorrecta o archivo alterado.".into(),
            )
        })?;
    Ok(plaintext.to_vec())
}

pub fn encrypt_file(
    source: &Path,
    destination: &Path,
    key: &[u8; 32],
) -> Result<(), BiometricError> {
    let mut plaintext = fs::read(source)
        .map_err(|err| BiometricError::Storage(format!("No se pudo leer base temporal: {err}")))?;
    let encrypted = encrypt_bytes(&plaintext, key);
    plaintext.fill(0);
    let encrypted = encrypted?;

    if let Some(parent) = destination.parent() {
        fs::create_dir_all(parent).map_err(|err| {
            BiometricError::Storage(format!("No se pudo crear directorio de respaldo: {err}"))
        })?;
    }
    fs::write(destination, encrypted).map_err(|err| {
        BiometricError::Storage(format!("No se pudo escribir respaldo cifrado: {err}"))
    })
}

pub fn decrypt_file(
    source: &Path,
    destination: &Path,
    key: &[u8; 32],
) -> Result<(), BiometricError> {
    if !source.is_file() {
        return Err(BiometricError::Storage(
            "No existe el archivo de respaldo cifrado.".into(),
        ));
    }
    let encrypted = fs::read(source).map_err(|err| {
        BiometricError::Storage(format!("No se pudo leer respaldo cifrado: {err}"))
    })?;
    let mut plaintext = decrypt_bytes(&encrypted, key)?;
    let write_res = fs::write(destination, &plaintext);
    plaintext.fill(0);
    write_res.map_err(|err| {
        BiometricError::Storage(format!(
            "No se pudo preparar base temporal para restauración: {err}"
        ))
    })
}

/// Archivo temporal que sobrescribe con ceros y elimina su contenido en disco al salir de alcance.
pub struct TempPlaintextFile {
    path: PathBuf,
}

impl TempPlaintextFile {
    pub fn new(prefix: &str) -> Result<Self, BiometricError> {
        let dir = env::temp_dir().join("dylan-comedor-biometric-tmp");
        fs::create_dir_all(&dir).map_err(|err| {
            BiometricError::Storage(format!("No se pudo preparar directorio temporal: {err}"))
        })?;
        let seq = TEMP_SEQ.fetch_add(1, Ordering::Relaxed);
        let pid = std::process::id();
        let path = dir.join(format!("{prefix}-{pid}-{seq}.fpdb"));
        Ok(Self { path })
    }

    pub fn path(&self) -> &Path {
        &self.path
    }
}

impl Drop for TempPlaintextFile {
    fn drop(&mut self) {
        if let Ok(meta) = fs::metadata(&self.path) {
            let zeros = vec![0u8; meta.len().min(8 * 1024 * 1024) as usize];
            let _ = fs::write(&self.path, zeros);
        }
        let _ = fs::remove_file(&self.path);
    }
}

/// Verifica que la DLL Windows corresponda a la misma arquitectura del ejecutable (`x86_64` / `x86`).
#[cfg(target_os = "windows")]
pub fn verify_dll_matches_architecture(path: &Path) -> Result<(), BiometricError> {
    if !path.is_file() {
        return Err(BiometricError::Device(format!(
            "No se encontró la biblioteca requerida: {}",
            path.display()
        )));
    }
    let bytes = fs::read(path).map_err(|err| {
        BiometricError::Device(format!(
            "No se pudo inspeccionar la biblioteca {}: {err}",
            path.display()
        ))
    })?;
    if bytes.len() < 0x40 || &bytes[0..2] != b"MZ" {
        return Err(BiometricError::Device(format!(
            "El archivo {} no es una DLL PE válida.",
            path.display()
        )));
    }
    let pe_offset = u32::from_le_bytes(bytes[0x3C..0x40].try_into().unwrap()) as usize;
    if bytes.len() < pe_offset + 6 || &bytes[pe_offset..pe_offset + 4] != b"PE\0\0" {
        return Err(BiometricError::Device(format!(
            "Cabecera PE inválida en {}.",
            path.display()
        )));
    }
    let machine = u16::from_le_bytes(bytes[pe_offset + 4..pe_offset + 6].try_into().unwrap());
    let expected = if cfg!(target_arch = "x86_64") {
        0x8664_u16
    } else if cfg!(target_arch = "x86") {
        0x014C_u16
    } else if cfg!(target_arch = "aarch64") {
        0xAA64_u16
    } else {
        machine
    };
    if machine != expected {
        return Err(BiometricError::Device(format!(
            "Arquitectura incompatible en {} (machine=0x{machine:04X}, esperado 0x{expected:04X}). Usa la DLL de 64 bits.",
            path.display()
        )));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{BACKUP_MAGIC, decrypt_bytes, encrypt_bytes, parse_hex_key};

    #[test]
    fn aes_256_gcm_round_trip_and_tamper_detection() {
        let key = parse_hex_key("0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef")
            .expect("valid 32-byte hex key");
        let plaintext = b"respaldo secusearch de prueba";

        let mut encrypted = encrypt_bytes(plaintext, &key).expect("encrypt ok");
        assert_eq!(&encrypted[..8], BACKUP_MAGIC);
        assert_eq!(
            decrypt_bytes(&encrypted, &key).expect("decrypt ok"),
            plaintext
        );

        // Nonce aleatorio produce salidas distintas para el mismo contenido.
        let encrypted_again = encrypt_bytes(plaintext, &key).expect("second encrypt ok");
        assert_ne!(encrypted, encrypted_again);

        // Clave incorrecta falla autenticación.
        let wrong_key = [0xAA; 32];
        assert!(decrypt_bytes(&encrypted, &wrong_key).is_err());

        // Alteración de 1 bit falla autenticación GCM.
        let last = encrypted.len() - 1;
        encrypted[last] ^= 0x01;
        assert!(decrypt_bytes(&encrypted, &key).is_err());

        // Archivo truncado falla.
        assert!(decrypt_bytes(b"SGENC001short", &key).is_err());
    }

    #[test]
    fn rejects_invalid_hex_backup_keys() {
        assert!(parse_hex_key("1234").is_err());
        assert!(parse_hex_key(&"z".repeat(64)).is_err());
    }
}
