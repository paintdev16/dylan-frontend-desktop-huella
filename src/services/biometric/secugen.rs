#![cfg(target_os = "windows")]

use std::{
    env,
    path::{Path, PathBuf},
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
        mpsc::{self, Sender, SyncSender},
    },
    thread::{self, JoinHandle},
};

use crate::features::pensioners::model::PensionerId;

use super::{
    BiometricConfig, BiometricError, BiometricReader, CaptureOutcome, EnrollOutcome,
    fdx::FdxReader,
    secusearch::{CONF_LEVEL_NORMAL, SecuSearchEngine},
    storage::load_backup_key_from_env,
};

const CAPTURE_TIMEOUT_MS: u64 = 12_000;

enum WorkerCommand {
    Connect {
        reply: SyncSender<Result<String, BiometricError>>,
    },
    Disconnect {
        reply: SyncSender<()>,
    },
    Identify {
        cancel: Arc<AtomicBool>,
        reply: SyncSender<Result<CaptureOutcome, BiometricError>>,
    },
    Enroll {
        pensioner_id: PensionerId,
        cancel: Arc<AtomicBool>,
        reply: SyncSender<Result<EnrollOutcome, BiometricError>>,
    },
    Remove {
        pensioner_id: PensionerId,
        reply: SyncSender<Result<(), BiometricError>>,
    },
    EnrolledCount {
        reply: SyncSender<Result<usize, BiometricError>>,
    },
    SaveBackup {
        destination: PathBuf,
        key: [u8; 32],
        reply: SyncSender<Result<usize, BiometricError>>,
    },
    RestoreBackup {
        source: PathBuf,
        key: [u8; 32],
        reply: SyncSender<Result<usize, BiometricError>>,
    },
    Shutdown,
}

/// Resuelve la ubicación de una DLL de SecuGen por configuración, variable de entorno o rutas de instalación.
pub fn resolve_sdk_dll(
    configured: Option<&Path>,
    env_var: &str,
    relative_candidates: &[&str],
) -> PathBuf {
    if let Some(path) = configured
        && !path.as_os_str().is_empty()
    {
        return path.to_path_buf();
    }
    if let Some(env_path) = env::var_os(env_var).filter(|v| !v.is_empty()) {
        return PathBuf::from(env_path);
    }
    if let Ok(exe_dir) = env::current_exe().and_then(|p| {
        p.parent()
            .map(Path::to_path_buf)
            .ok_or_else(|| std::io::Error::other("no exe parent"))
    }) {
        for candidate in relative_candidates {
            let file_name = Path::new(candidate)
                .file_name()
                .map(PathBuf::from)
                .unwrap_or_default();
            let beside_exe = exe_dir.join(&file_name);
            if beside_exe.is_file() {
                return beside_exe;
            }
        }
    }
    if let Ok(cwd) = env::current_dir() {
        for candidate in relative_candidates {
            let full = cwd.join(candidate);
            if full.is_file() {
                return full;
            }
        }
    }
    PathBuf::from(relative_candidates.first().copied().unwrap_or_default())
}

/// Adaptador real de SecuGen (FDx SDK Pro + SecuSearch Pro) en Windows x64.
///
/// Toda la vida del lector USB y del motor SecuSearch permanece confinada en un
/// hilo dedicado (`secugen-reader-worker`) para garantizar afinidad de hilo, acceso
/// exclusivo y una interfaz GPUI siempre responsiva.
pub struct SecuGenBiometricReader {
    tx: Sender<WorkerCommand>,
    worker: Mutex<Option<JoinHandle<()>>>,
    connected: Arc<AtomicBool>,
    busy: AtomicBool,
    device_label: Mutex<String>,
    engine_version: Arc<Mutex<Option<String>>>,
}

impl SecuGenBiometricReader {
    pub fn new(config: BiometricConfig) -> Result<Self, BiometricError> {
        let (tx, rx) = mpsc::channel::<WorkerCommand>();
        let connected = Arc::new(AtomicBool::new(false));
        let engine_version = Arc::new(Mutex::new(None));

        let connected_worker = connected.clone();
        let version_worker = engine_version.clone();

        let handle = thread::Builder::new()
            .name("secugen-reader-worker".into())
            .spawn(move || {
                run_worker(rx, config, connected_worker, version_worker);
            })
            .map_err(|err| {
                BiometricError::Device(format!(
                    "No se pudo iniciar el hilo dedicado del lector SecuGen: {err}"
                ))
            })?;

        Ok(Self {
            tx,
            worker: Mutex::new(Some(handle)),
            connected,
            busy: AtomicBool::new(false),
            device_label: Mutex::new("SecuGen FDx + SecuSearch (Windows x64)".into()),
            engine_version,
        })
    }

    fn send_sync<T>(
        &self,
        make_cmd: impl FnOnce(SyncSender<Result<T, BiometricError>>) -> WorkerCommand,
    ) -> Result<T, BiometricError> {
        let (reply_tx, reply_rx) = mpsc::sync_channel(1);
        self.tx.send(make_cmd(reply_tx)).map_err(|_| {
            BiometricError::Disconnected("El hilo del lector SecuGen ya no está activo.".into())
        })?;
        reply_rx.recv().map_err(|_| {
            BiometricError::Disconnected(
                "El hilo del lector SecuGen se cerró inesperadamente.".into(),
            )
        })?
    }
}

impl BiometricReader for SecuGenBiometricReader {
    fn device_name(&self) -> String {
        self.device_label
            .lock()
            .map(|guard| guard.clone())
            .unwrap_or_else(|_| "SecuGen FDx + SecuSearch".into())
    }

    fn secusearch_version(&self) -> Option<String> {
        self.engine_version.lock().ok().and_then(|g| g.clone())
    }

    fn is_connected(&self) -> bool {
        self.connected.load(Ordering::SeqCst)
    }

    fn connect(&self) -> Result<(), BiometricError> {
        let label = self.send_sync(|reply| WorkerCommand::Connect { reply })?;
        if let Ok(mut guard) = self.device_label.lock() {
            *guard = label;
        }
        Ok(())
    }

    fn disconnect(&self) {
        let (reply_tx, reply_rx) = mpsc::sync_channel(1);
        if self
            .tx
            .send(WorkerCommand::Disconnect { reply: reply_tx })
            .is_ok()
        {
            let _ = reply_rx.recv();
        }
    }

    fn identify(&self, cancel: Arc<AtomicBool>) -> Result<CaptureOutcome, BiometricError> {
        if self.busy.swap(true, Ordering::SeqCst) {
            return Err(BiometricError::Busy);
        }
        let result = self.send_sync(|reply| WorkerCommand::Identify { cancel, reply });
        self.busy.store(false, Ordering::SeqCst);
        result
    }

    fn enroll(
        &self,
        pensioner_id: PensionerId,
        cancel: Arc<AtomicBool>,
    ) -> Result<EnrollOutcome, BiometricError> {
        if self.busy.swap(true, Ordering::SeqCst) {
            return Err(BiometricError::Busy);
        }
        let result = self.send_sync(|reply| WorkerCommand::Enroll {
            pensioner_id,
            cancel,
            reply,
        });
        self.busy.store(false, Ordering::SeqCst);
        result
    }

    fn remove_template(&self, pensioner_id: PensionerId) -> Result<(), BiometricError> {
        self.send_sync(|reply| WorkerCommand::Remove {
            pensioner_id,
            reply,
        })
    }

    fn enrolled_count(&self) -> Result<usize, BiometricError> {
        self.send_sync(|reply| WorkerCommand::EnrolledCount { reply })
    }

    fn save_encrypted_backup(
        &self,
        destination: &Path,
        key: &[u8; 32],
    ) -> Result<usize, BiometricError> {
        self.send_sync(|reply| WorkerCommand::SaveBackup {
            destination: destination.to_path_buf(),
            key: *key,
            reply,
        })
    }

    fn restore_encrypted_backup(
        &self,
        source: &Path,
        key: &[u8; 32],
    ) -> Result<usize, BiometricError> {
        self.send_sync(|reply| WorkerCommand::RestoreBackup {
            source: source.to_path_buf(),
            key: *key,
            reply,
        })
    }
}

impl Drop for SecuGenBiometricReader {
    fn drop(&mut self) {
        let _ = self.tx.send(WorkerCommand::Shutdown);
        if let Ok(mut guard) = self.worker.lock()
            && let Some(handle) = guard.take()
        {
            let _ = handle.join();
        }
    }
}

fn run_worker(
    rx: mpsc::Receiver<WorkerCommand>,
    config: BiometricConfig,
    connected_flag: Arc<AtomicBool>,
    version_slot: Arc<Mutex<Option<String>>>,
) {
    let mut fdx: Option<FdxReader> = None;
    let mut sse: Option<SecuSearchEngine> = None;

    let fdx_dll = resolve_sdk_dll(
        config.fdx_dll_path.as_deref(),
        "SECUGEN_FDX_DLL",
        &[
            "secugen/FDx_SDK_Pro_Windows_v4.3.1_J1.21/FDx SDK Pro for Windows v4.3.1/bin/x64/sgfplib.dll",
            "secugen/FDx SDK Pro for Windows v4.3.1/bin/x64/sgfplib.dll",
            "sgfplib.dll",
        ],
    );
    let sse_dll = resolve_sdk_dll(
        config.secusearch_dll_path.as_deref(),
        "SECUGEN_SECUSEARCH_DLL",
        &[
            "secugen/SecuSearch_Pro_SDK_v3.5.1/bin/secusearchapi.dll",
            "secusearchapi.dll",
        ],
    );

    while let Ok(command) = rx.recv() {
        match command {
            WorkerCommand::Shutdown => break,
            WorkerCommand::Disconnect { reply } => {
                fdx = None;
                connected_flag.store(false, Ordering::SeqCst);
                let _ = reply.send(());
            }
            WorkerCommand::Connect { reply } => {
                let res = (|| {
                    if sse.is_none() {
                        let mut engine = SecuSearchEngine::load(&sse_dll)?;
                        if let Ok(mut slot) = version_slot.lock() {
                            *slot = Some(engine.version().to_string());
                        }
                        // Si ya existe un respaldo cifrado y la clave de custodia está presente, restaurarlo.
                        if config.backup_path.is_file()
                            && let Ok(key) = load_backup_key_from_env()
                        {
                            let _ = engine.restore_encrypted_backup(&config.backup_path, &key);
                        }
                        sse = Some(engine);
                    }

                    if fdx.is_none() {
                        fdx = Some(FdxReader::load(&fdx_dll)?);
                    }
                    let reader = fdx.as_mut().expect("fdx loaded");
                    let meta = reader.open()?;
                    let sse_ver = sse
                        .as_ref()
                        .map(|e| e.version().to_string())
                        .unwrap_or_else(|| "N/D".into());
                    let label = if meta.serial_number.is_empty() {
                        format!(
                            "SecuGen FDx ({}x{} @ {} dpi, FW {}) · SecuSearch {sse_ver}",
                            meta.image_width, meta.image_height, meta.image_dpi, meta.fw_version
                        )
                    } else {
                        format!(
                            "SecuGen SN {} (FW {}) · SecuSearch {sse_ver}",
                            meta.serial_number, meta.fw_version
                        )
                    };
                    connected_flag.store(true, Ordering::SeqCst);
                    Ok(label)
                })();
                if res.is_err() {
                    fdx = None;
                    connected_flag.store(false, Ordering::SeqCst);
                }
                let _ = reply.send(res);
            }
            WorkerCommand::Identify { cancel, reply } => {
                let res = (|| {
                    let Some(reader) = fdx.as_mut().filter(|r| r.is_connected()) else {
                        connected_flag.store(false, Ordering::SeqCst);
                        return Err(BiometricError::Disconnected(
                            "Lector SecuGen desconectado. Reconecta el equipo o usa búsqueda manual."
                                .into(),
                        ));
                    };
                    let Some(engine) = sse.as_mut() else {
                        return Err(BiometricError::Device(
                            "Motor SecuSearch no inicializado.".into(),
                        ));
                    };

                    let captured = reader.capture_sg400_template(&cancel, CAPTURE_TIMEOUT_MS);
                    let mut template = match captured {
                        Ok(Some(t)) => t,
                        Ok(None) => return Ok(CaptureOutcome::Cancelled),
                        Err(err) => {
                            if matches!(err, BiometricError::Disconnected(_)) {
                                connected_flag.store(false, Ordering::SeqCst);
                            }
                            return Err(err);
                        }
                    };

                    let matched = engine.identify(&template, CONF_LEVEL_NORMAL);
                    template.fill(0);
                    match matched? {
                        Some(id) => Ok(CaptureOutcome::Matched(PensionerId(id))),
                        None => Ok(CaptureOutcome::Unrecognized),
                    }
                })();
                let _ = reply.send(res);
            }
            WorkerCommand::Enroll {
                pensioner_id,
                cancel,
                reply,
            } => {
                let res = (|| {
                    let Some(reader) = fdx.as_mut().filter(|r| r.is_connected()) else {
                        connected_flag.store(false, Ordering::SeqCst);
                        return Err(BiometricError::Disconnected(
                            "Lector SecuGen desconectado. Conecta el lector antes de enrolar."
                                .into(),
                        ));
                    };
                    let Some(engine) = sse.as_mut() else {
                        return Err(BiometricError::Device(
                            "Motor SecuSearch no inicializado.".into(),
                        ));
                    };

                    let mut sample_one =
                        match reader.capture_sg400_template(&cancel, CAPTURE_TIMEOUT_MS) {
                            Ok(Some(t)) => t,
                            Ok(None) => return Ok(EnrollOutcome::Cancelled),
                            Err(err) => {
                                if matches!(err, BiometricError::Disconnected(_)) {
                                    connected_flag.store(false, Ordering::SeqCst);
                                }
                                return Err(err);
                            }
                        };

                    let mut sample_two =
                        match reader.capture_sg400_template(&cancel, CAPTURE_TIMEOUT_MS) {
                            Ok(Some(t)) => t,
                            Ok(None) => {
                                sample_one.fill(0);
                                return Ok(EnrollOutcome::Cancelled);
                            }
                            Err(err) => {
                                sample_one.fill(0);
                                if matches!(err, BiometricError::Disconnected(_)) {
                                    connected_flag.store(false, Ordering::SeqCst);
                                }
                                return Err(err);
                            }
                        };

                    let same_finger = reader.verify_match_pair(&sample_one, &sample_two);
                    sample_two.fill(0);
                    match same_finger {
                        Ok(false) => {
                            sample_one.fill(0);
                            return Ok(EnrollOutcome::Mismatch);
                        }
                        Err(err) => {
                            sample_one.fill(0);
                            return Err(err);
                        }
                        Ok(true) => {}
                    }

                    let reg = engine.register_or_replace(&sample_one, pensioner_id.0);
                    sample_one.fill(0);
                    reg?;

                    // Si existe clave de respaldo configurada, persiste el respaldo cifrado automáticamente.
                    if let Ok(key) = load_backup_key_from_env() {
                        let _ = engine.save_encrypted_backup(&config.backup_path, &key);
                    }

                    Ok(EnrollOutcome::Enrolled(pensioner_id))
                })();
                let _ = reply.send(res);
            }
            WorkerCommand::Remove {
                pensioner_id,
                reply,
            } => {
                let res = match sse.as_mut() {
                    Some(engine) => engine.remove(pensioner_id.0),
                    None => Ok(()),
                };
                let _ = reply.send(res);
            }
            WorkerCommand::EnrolledCount { reply } => {
                let res = match sse.as_mut() {
                    Some(engine) => engine.count(),
                    None => Ok(0),
                };
                let _ = reply.send(res);
            }
            WorkerCommand::SaveBackup {
                destination,
                key,
                reply,
            } => {
                let res = match sse.as_mut() {
                    Some(engine) => engine.save_encrypted_backup(&destination, &key),
                    None => Err(BiometricError::Device(
                        "Motor SecuSearch no inicializado para guardar respaldo.".into(),
                    )),
                };
                let _ = reply.send(res);
            }
            WorkerCommand::RestoreBackup { source, key, reply } => {
                let res = (|| {
                    if sse.is_none() {
                        let engine = SecuSearchEngine::load(&sse_dll)?;
                        if let Ok(mut slot) = version_slot.lock() {
                            *slot = Some(engine.version().to_string());
                        }
                        sse = Some(engine);
                    }
                    sse.as_mut()
                        .expect("engine initialized")
                        .restore_encrypted_backup(&source, &key)
                })();
                let _ = reply.send(res);
            }
        }
    }

    drop(fdx);
    drop(sse);
    connected_flag.store(false, Ordering::SeqCst);
}
