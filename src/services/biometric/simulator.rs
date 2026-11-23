use std::{
    collections::BTreeSet,
    fs,
    path::Path,
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
    },
    thread,
    time::Duration,
};

use crate::features::pensioners::model::{PensionerId, demo_pensioners};

use super::{
    BiometricError, BiometricReader, CaptureOutcome, EnrollOutcome,
    storage::{decrypt_bytes, encrypt_bytes},
};

/// Escenario configurable para probar los flujos de identificación sin lector físico.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SimulatedScenario {
    /// Coincidencia simulada para un pensionista enrolado tras una captura breve.
    Match {
        pensioner_id: PensionerId,
        delay_ms: u64,
    },
    /// Huella no reconocida tras la lectura.
    Unrecognized { delay_ms: u64 },
    /// Simula desconexión inesperada del equipo durante la captura.
    DisconnectDuringCapture { delay_ms: u64 },
}

impl Default for SimulatedScenario {
    fn default() -> Self {
        Self::Match {
            pensioner_id: PensionerId(1001),
            delay_ms: 450,
        }
    }
}

/// Implementación simulada del contrato `BiometricReader` para desarrollo y Linux.
pub struct SimulatedBiometricReader {
    connected: AtomicBool,
    busy: AtomicBool,
    scenario: Mutex<SimulatedScenario>,
    enrolled_ids: Mutex<BTreeSet<PensionerId>>,
    enroll_delay_ms: u64,
}

impl SimulatedBiometricReader {
    pub fn new(initial_scenario: SimulatedScenario) -> Self {
        let initial_ids: BTreeSet<PensionerId> = demo_pensioners()
            .into_iter()
            .filter(|p| p.fingerprint_enrolled)
            .map(|p| p.id)
            .collect();
        Self {
            connected: AtomicBool::new(true),
            busy: AtomicBool::new(false),
            scenario: Mutex::new(initial_scenario),
            enrolled_ids: Mutex::new(initial_ids),
            enroll_delay_ms: 450,
        }
    }

    pub fn scenario(&self) -> SimulatedScenario {
        *self.scenario.lock().expect("simulator scenario lock")
    }

    pub fn set_scenario(&self, scenario: SimulatedScenario) {
        *self.scenario.lock().expect("simulator scenario lock") = scenario;
    }

    fn wait_cancellable(&self, total_ms: u64, cancel: &AtomicBool) -> bool {
        let step = Duration::from_millis(25);
        let mut elapsed = 0_u64;
        while elapsed < total_ms {
            if cancel.load(Ordering::SeqCst) {
                return false;
            }
            if !self.connected.load(Ordering::SeqCst) {
                return true;
            }
            let remaining = total_ms - elapsed;
            let sleep_ms = remaining.min(step.as_millis() as u64);
            thread::sleep(Duration::from_millis(sleep_ms));
            elapsed += sleep_ms;
        }
        !cancel.load(Ordering::SeqCst)
    }
}

impl BiometricReader for SimulatedBiometricReader {
    fn device_name(&self) -> String {
        "Simulador biométrico".into()
    }

    fn is_connected(&self) -> bool {
        self.connected.load(Ordering::SeqCst)
    }

    fn connect(&self) -> Result<(), BiometricError> {
        self.connected.store(true, Ordering::SeqCst);
        Ok(())
    }

    fn disconnect(&self) {
        self.connected.store(false, Ordering::SeqCst);
    }

    fn identify(&self, cancel: Arc<AtomicBool>) -> Result<CaptureOutcome, BiometricError> {
        if !self.is_connected() {
            return Err(BiometricError::Disconnected(
                "Lector desconectado. Reconecta el equipo o usa búsqueda manual.".into(),
            ));
        }
        if self.busy.swap(true, Ordering::SeqCst) {
            return Err(BiometricError::Busy);
        }

        let scenario = self.scenario();
        let delay_ms = match scenario {
            SimulatedScenario::Match { delay_ms, .. }
            | SimulatedScenario::Unrecognized { delay_ms }
            | SimulatedScenario::DisconnectDuringCapture { delay_ms } => delay_ms,
        };

        let result = if !self.wait_cancellable(delay_ms, &cancel) {
            Ok(CaptureOutcome::Cancelled)
        } else if !self.is_connected() {
            Err(BiometricError::Disconnected(
                "Se perdió la conexión con el lector durante la captura.".into(),
            ))
        } else {
            match scenario {
                SimulatedScenario::Match { pensioner_id, .. } => {
                    Ok(CaptureOutcome::Matched(pensioner_id))
                }
                SimulatedScenario::Unrecognized { .. } => Ok(CaptureOutcome::Unrecognized),
                SimulatedScenario::DisconnectDuringCapture { .. } => {
                    self.connected.store(false, Ordering::SeqCst);
                    Err(BiometricError::Disconnected(
                        "El lector se desconectó durante la captura.".into(),
                    ))
                }
            }
        };

        self.busy.store(false, Ordering::SeqCst);
        result
    }

    fn enroll(
        &self,
        pensioner_id: PensionerId,
        cancel: Arc<AtomicBool>,
    ) -> Result<EnrollOutcome, BiometricError> {
        if !self.is_connected() {
            return Err(BiometricError::Disconnected(
                "Lector desconectado. Reconecta el equipo antes de enrolar.".into(),
            ));
        }
        if self.busy.swap(true, Ordering::SeqCst) {
            return Err(BiometricError::Busy);
        }

        let result = if !self.wait_cancellable(self.enroll_delay_ms, &cancel) {
            Ok(EnrollOutcome::Cancelled)
        } else if !self.is_connected() {
            Err(BiometricError::Disconnected(
                "Se perdió la conexión con el lector durante el enrolamiento.".into(),
            ))
        } else {
            if let Ok(mut guard) = self.enrolled_ids.lock() {
                guard.insert(pensioner_id);
            }
            Ok(EnrollOutcome::Enrolled(pensioner_id))
        };

        self.busy.store(false, Ordering::SeqCst);
        result
    }

    fn remove_template(&self, pensioner_id: PensionerId) -> Result<(), BiometricError> {
        if let Ok(mut guard) = self.enrolled_ids.lock() {
            guard.remove(&pensioner_id);
        }
        Ok(())
    }

    fn enrolled_count(&self) -> Result<usize, BiometricError> {
        Ok(self.enrolled_ids.lock().map(|g| g.len()).unwrap_or(0))
    }

    fn save_encrypted_backup(
        &self,
        destination: &Path,
        key: &[u8; 32],
    ) -> Result<usize, BiometricError> {
        let ids: Vec<u32> = self
            .enrolled_ids
            .lock()
            .map(|g| g.iter().map(|id| id.0).collect())
            .unwrap_or_default();
        let mut plaintext = Vec::with_capacity(ids.len() * 4);
        for id in &ids {
            plaintext.extend_from_slice(&id.to_le_bytes());
        }
        let encrypted = encrypt_bytes(&plaintext, key)?;
        if let Some(parent) = destination.parent() {
            fs::create_dir_all(parent).map_err(|err| {
                BiometricError::Storage(format!("No se pudo crear directorio de respaldo: {err}"))
            })?;
        }
        fs::write(destination, encrypted).map_err(|err| {
            BiometricError::Storage(format!("No se pudo guardar respaldo simulado: {err}"))
        })?;
        Ok(ids.len())
    }

    fn restore_encrypted_backup(
        &self,
        source: &Path,
        key: &[u8; 32],
    ) -> Result<usize, BiometricError> {
        let data = fs::read(source).map_err(|err| {
            BiometricError::Storage(format!("No se pudo leer respaldo simulado: {err}"))
        })?;
        let plaintext = decrypt_bytes(&data, key)?;
        if plaintext.len() % 4 != 0 {
            return Err(BiometricError::Storage(
                "Formato de respaldo simulado inválido.".into(),
            ));
        }
        let mut restored = BTreeSet::new();
        for chunk in plaintext.chunks_exact(4) {
            restored.insert(PensionerId(u32::from_le_bytes(chunk.try_into().unwrap())));
        }
        let count = restored.len();
        if let Ok(mut guard) = self.enrolled_ids.lock() {
            *guard = restored;
        }
        Ok(count)
    }
}

#[cfg(test)]
mod tests {
    use std::{
        env, fs,
        sync::{
            Arc,
            atomic::{AtomicBool, Ordering},
        },
    };

    use super::{SimulatedBiometricReader, SimulatedScenario};
    use crate::{
        features::pensioners::model::PensionerId,
        services::biometric::{BiometricError, BiometricReader, CaptureOutcome, EnrollOutcome},
    };

    #[test]
    fn simulator_handles_match_unrecognized_disconnect_cancel_enroll_and_backup() {
        let reader = SimulatedBiometricReader::new(SimulatedScenario::Match {
            pensioner_id: PensionerId(1002),
            delay_ms: 0,
        });
        let cancel = Arc::new(AtomicBool::new(false));
        assert_eq!(
            reader.identify(cancel.clone()),
            Ok(CaptureOutcome::Matched(PensionerId(1002)))
        );

        reader.set_scenario(SimulatedScenario::Unrecognized { delay_ms: 0 });
        assert_eq!(
            reader.identify(cancel.clone()),
            Ok(CaptureOutcome::Unrecognized)
        );

        let pre_cancelled = Arc::new(AtomicBool::new(true));
        reader.set_scenario(SimulatedScenario::Match {
            pensioner_id: PensionerId(1001),
            delay_ms: 50,
        });
        assert_eq!(
            reader.identify(pre_cancelled.clone()),
            Ok(CaptureOutcome::Cancelled)
        );
        assert_eq!(
            reader.enroll(PensionerId(1004), pre_cancelled),
            Ok(EnrollOutcome::Cancelled)
        );

        assert_eq!(
            reader.enroll(PensionerId(1004), cancel.clone()),
            Ok(EnrollOutcome::Enrolled(PensionerId(1004)))
        );
        assert_eq!(reader.enrolled_count().unwrap(), 5);

        let backup = env::temp_dir().join("dylan-sim-backup-test.fpdb.enc");
        let key = [7u8; 32];
        assert_eq!(reader.save_encrypted_backup(&backup, &key).unwrap(), 5);
        reader.remove_template(PensionerId(1004)).unwrap();
        assert_eq!(reader.enrolled_count().unwrap(), 4);
        assert_eq!(reader.restore_encrypted_backup(&backup, &key).unwrap(), 5);
        let _ = fs::remove_file(&backup);

        reader.set_scenario(SimulatedScenario::DisconnectDuringCapture { delay_ms: 0 });
        assert!(matches!(
            reader.identify(cancel.clone()),
            Err(BiometricError::Disconnected(_))
        ));
        assert!(!reader.is_connected());

        reader.connect().expect("reconnect must succeed");
        assert!(reader.is_connected());
        cancel.store(false, Ordering::SeqCst);
        reader.set_scenario(SimulatedScenario::Match {
            pensioner_id: PensionerId(1003),
            delay_ms: 0,
        });
        assert_eq!(
            reader.identify(cancel),
            Ok(CaptureOutcome::Matched(PensionerId(1003)))
        );
    }
}
