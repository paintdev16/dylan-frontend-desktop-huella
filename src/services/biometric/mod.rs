#[cfg(target_os = "windows")]
pub mod fdx;
#[cfg(target_os = "windows")]
pub(crate) mod sdk_paths;
#[cfg(target_os = "windows")]
pub mod secugen;
#[cfg(target_os = "windows")]
pub mod secusearch;
pub mod storage;
#[cfg(target_os = "windows")]
pub mod usb;

use std::{
    path::{Path, PathBuf},
    sync::{Arc, atomic::AtomicBool},
};

use crate::features::pensioners::model::PensionerId;

/// Configuración no sensible de ubicaciones de SDK y respaldo cifrado por instalación.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BiometricConfig {
    pub fdx_dll_path: Option<PathBuf>,
    pub secusearch_dll_path: Option<PathBuf>,
    pub backup_path: PathBuf,
}

impl Default for BiometricConfig {
    fn default() -> Self {
        Self {
            fdx_dll_path: None,
            secusearch_dll_path: None,
            backup_path: storage::default_backup_path(),
        }
    }
}

/// Operación exclusiva en curso sobre el lector biométrico.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReaderOperation {
    Identify,
    Enroll(PensionerId),
}

/// Resultado tipado de una identificación biométrica (nunca expone muestras ni plantillas).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CaptureOutcome {
    Matched(PensionerId),
    Unrecognized,
    Cancelled,
}

/// Resultado tipado de un enrolamiento biométrico (nunca expone muestras ni plantillas).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EnrollOutcome {
    Enrolled(PensionerId),
    Mismatch,
    Cancelled,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BiometricError {
    Disconnected(String),
    Busy,
    Device(String),
    Storage(String),
}

impl BiometricError {
    pub fn message(&self) -> String {
        match self {
            Self::Disconnected(reason) => reason.clone(),
            Self::Busy => "Ya existe una operación biométrica en curso.".into(),
            Self::Device(reason) | Self::Storage(reason) => reason.clone(),
        }
    }
}

/// Contrato del lector biométrico. Las llamadas bloqueantes se ejecutan fuera de GPUI.
#[allow(dead_code)]
pub trait BiometricReader: Send + Sync {
    fn device_name(&self) -> String;
    fn secusearch_version(&self) -> Option<String> {
        None
    }
    fn is_connected(&self) -> bool;
    fn connect(&self) -> Result<(), BiometricError>;
    fn disconnect(&self);
    fn identify(&self, cancel: Arc<AtomicBool>) -> Result<CaptureOutcome, BiometricError>;
    fn enroll(
        &self,
        pensioner_id: PensionerId,
        cancel: Arc<AtomicBool>,
    ) -> Result<EnrollOutcome, BiometricError>;
    fn remove_template(&self, pensioner_id: PensionerId) -> Result<(), BiometricError>;
    fn enrolled_count(&self) -> Result<usize, BiometricError>;
    fn save_encrypted_backup(
        &self,
        destination: &Path,
        key: &[u8; 32],
    ) -> Result<usize, BiometricError>;
    fn restore_encrypted_backup(
        &self,
        source: &Path,
        key: &[u8; 32],
    ) -> Result<usize, BiometricError>;
}
