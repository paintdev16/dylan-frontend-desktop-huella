use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};

use gpui_kit::{Context, EventEmitter, SharedString, Task};

use crate::{
    features::pensioners::model::PensionerId,
    services::biometric::{
        BiometricConfig, BiometricError, BiometricReader, CaptureOutcome, EnrollOutcome,
        ReaderOperation,
    },
};

#[cfg(target_os = "windows")]
use std::{mem, time::Duration};

#[cfg(target_os = "windows")]
use crate::services::biometric::{
    secugen::SecuGenBiometricReader,
    usb::{UsbReaderEvent, UsbReaderNotifications},
};

const READER_DISCONNECTED: &str = "Lector desconectado. Reconecta el equipo o usa búsqueda manual.";

/// Estado observable del lector biométrico expuesto a las vistas.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ReaderStatus {
    Disconnected {
        reason: SharedString,
    },
    Ready,
    Capturing {
        attempt_id: u64,
        operation: ReaderOperation,
    },
    Matched {
        pensioner_id: PensionerId,
    },
    Enrolled {
        pensioner_id: PensionerId,
    },
    EnrollmentMismatch,
    Unrecognized,
    Cancelled,
}

impl ReaderStatus {
    pub fn is_capturing(&self) -> bool {
        matches!(self, Self::Capturing { .. })
    }

    pub fn is_enrolling(&self) -> bool {
        matches!(
            self,
            Self::Capturing {
                operation: ReaderOperation::Enroll(_),
                ..
            }
        )
    }

    pub fn is_disconnected(&self) -> bool {
        matches!(self, Self::Disconnected { .. })
    }

    pub fn short_label(&self) -> &'static str {
        match self {
            Self::Disconnected { .. } => "Lector desconectado",
            Self::Ready => "Lector listo",
            Self::Capturing {
                operation: ReaderOperation::Identify,
                ..
            } => "Capturando huella…",
            Self::Capturing {
                operation: ReaderOperation::Enroll(_),
                ..
            } => "Enrolando huella…",
            Self::Matched { .. } => "Huella identificada",
            Self::Enrolled { .. } => "Huella enrolada",
            Self::EnrollmentMismatch => "Muestras no coinciden",
            Self::Unrecognized => "Huella no reconocida",
            Self::Cancelled => "Captura cancelada",
        }
    }
}

/// Eventos tipados emitidos cuando concluye una identificación o enrolamiento válido.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReaderEvent {
    Identified(PensionerId),
    Enrolled(PensionerId),
}

/// Máquina de estados pura del lector (probable en `cargo test` sin abrir GPUI).
/// Invalida intentos cancelados o interrumpidos para que un resultado tardío
/// nunca se aplique a la operación siguiente.
#[derive(Debug, Clone)]
pub struct ReaderMachine {
    status: ReaderStatus,
    active_attempt_id: u64,
}

impl ReaderMachine {
    pub fn new(connected: bool) -> Self {
        let status = if connected {
            ReaderStatus::Ready
        } else {
            ReaderStatus::Disconnected {
                reason: SharedString::from("Lector no conectado."),
            }
        };
        Self {
            status,
            active_attempt_id: 0,
        }
    }

    pub fn status(&self) -> &ReaderStatus {
        &self.status
    }

    pub fn begin_identify(&mut self) -> Result<u64, BiometricError> {
        self.begin_operation(ReaderOperation::Identify)
    }

    pub fn begin_enroll(&mut self, pensioner_id: PensionerId) -> Result<u64, BiometricError> {
        self.begin_operation(ReaderOperation::Enroll(pensioner_id))
    }

    fn begin_operation(&mut self, operation: ReaderOperation) -> Result<u64, BiometricError> {
        if let ReaderStatus::Disconnected { reason } = &self.status {
            return Err(BiometricError::Disconnected(reason.to_string()));
        }
        if self.status.is_capturing() {
            return Err(BiometricError::Busy);
        }

        self.active_attempt_id += 1;
        let attempt_id = self.active_attempt_id;
        self.status = ReaderStatus::Capturing {
            attempt_id,
            operation,
        };
        Ok(attempt_id)
    }

    /// Cancela el intento en curso e invalida su identificador para descartar resultados tardíos.
    pub fn cancel(&mut self) -> bool {
        if !self.status.is_capturing() {
            return false;
        }
        self.active_attempt_id += 1;
        self.status = ReaderStatus::Cancelled;
        true
    }

    pub fn mark_disconnected(&mut self, reason: impl Into<SharedString>) {
        self.active_attempt_id += 1;
        self.status = ReaderStatus::Disconnected {
            reason: reason.into(),
        };
    }

    pub fn mark_reconnected(&mut self) {
        self.active_attempt_id += 1;
        self.status = ReaderStatus::Ready;
    }

    /// Aplica el resultado de identificación solo si corresponde al intento activo.
    pub fn apply_result(
        &mut self,
        attempt_id: u64,
        result: Result<CaptureOutcome, BiometricError>,
    ) -> Option<PensionerId> {
        if attempt_id != self.active_attempt_id
            || !matches!(
                self.status,
                ReaderStatus::Capturing {
                    operation: ReaderOperation::Identify,
                    ..
                }
            )
        {
            return None;
        }

        match result {
            Ok(CaptureOutcome::Matched(pensioner_id)) => {
                self.status = ReaderStatus::Matched { pensioner_id };
                Some(pensioner_id)
            }
            Ok(CaptureOutcome::Unrecognized) => {
                self.status = ReaderStatus::Unrecognized;
                None
            }
            Ok(CaptureOutcome::Cancelled) => {
                self.status = ReaderStatus::Cancelled;
                None
            }
            Err(BiometricError::Disconnected(reason))
            | Err(BiometricError::Device(reason))
            | Err(BiometricError::Storage(reason)) => {
                self.status = ReaderStatus::Disconnected {
                    reason: SharedString::from(reason),
                };
                None
            }
            Err(BiometricError::Busy) => {
                self.status = ReaderStatus::Ready;
                None
            }
        }
    }

    /// Aplica el resultado de enrolamiento solo si corresponde al intento activo.
    pub fn apply_enroll_result(
        &mut self,
        attempt_id: u64,
        result: Result<EnrollOutcome, BiometricError>,
    ) -> Option<PensionerId> {
        if attempt_id != self.active_attempt_id
            || !matches!(
                self.status,
                ReaderStatus::Capturing {
                    operation: ReaderOperation::Enroll(_),
                    ..
                }
            )
        {
            return None;
        }

        match result {
            Ok(EnrollOutcome::Enrolled(pensioner_id)) => {
                self.status = ReaderStatus::Enrolled { pensioner_id };
                Some(pensioner_id)
            }
            Ok(EnrollOutcome::Mismatch) => {
                self.status = ReaderStatus::EnrollmentMismatch;
                None
            }
            Ok(EnrollOutcome::Cancelled) => {
                self.status = ReaderStatus::Cancelled;
                None
            }
            Err(BiometricError::Disconnected(reason))
            | Err(BiometricError::Device(reason))
            | Err(BiometricError::Storage(reason)) => {
                self.status = ReaderStatus::Disconnected {
                    reason: SharedString::from(reason),
                };
                None
            }
            Err(BiometricError::Busy) => {
                self.status = ReaderStatus::Ready;
                None
            }
        }
    }
}

/// Entidad reactiva que coordina el lector SecuGen fuera del hilo de GPUI.
pub struct ReaderState {
    machine: ReaderMachine,
    config: Option<BiometricConfig>,
    reader: Option<Arc<dyn BiometricReader>>,
    connection_task: Option<Task<()>>,
    active_cancel: Option<Arc<AtomicBool>>,
    capture_task: Option<Task<()>>,
    #[cfg(target_os = "windows")]
    usb_monitor: Option<Task<()>>,
    #[cfg(target_os = "windows")]
    usb_disconnect_pending: bool,
    #[cfg(target_os = "windows")]
    usb_reconnect_pending: bool,
}

impl EventEmitter<ReaderEvent> for ReaderState {}

impl ReaderState {
    pub fn new() -> Self {
        let mut machine = ReaderMachine::new(false);
        machine.mark_disconnected(if cfg!(target_os = "windows") {
            "Cargando preferencias del lector SecuGen…"
        } else {
            "SecuGen está disponible en Windows x64. Usa la búsqueda manual de pensionistas."
        });
        Self {
            machine,
            config: None,
            reader: None,
            connection_task: None,
            active_cancel: None,
            capture_task: None,
            #[cfg(target_os = "windows")]
            usb_monitor: None,
            #[cfg(target_os = "windows")]
            usb_disconnect_pending: false,
            #[cfg(target_os = "windows")]
            usb_reconnect_pending: false,
        }
    }

    pub fn device_name(&self) -> String {
        self.reader
            .as_ref()
            .map_or_else(|| "SecuGen".into(), |reader| reader.device_name())
    }

    pub fn secusearch_version(&self) -> Option<String> {
        self.reader
            .as_ref()
            .and_then(|reader| reader.secusearch_version())
    }

    pub fn status(&self) -> &ReaderStatus {
        self.machine.status()
    }

    pub fn can_reconnect(&self) -> bool {
        cfg!(target_os = "windows") && self.config.is_some() && self.connection_task.is_none()
    }

    pub fn connection_loading(&self) -> bool {
        cfg!(target_os = "windows") && (self.config.is_none() || self.connection_task.is_some())
    }

    pub fn clear_identification(&mut self, cx: &mut Context<Self>) {
        if !self.status().is_capturing() && !self.status().is_disconnected() {
            self.machine.mark_reconnected();
            cx.notify();
        }
    }

    pub fn configure(&mut self, config: BiometricConfig, cx: &mut Context<Self>) {
        #[cfg(target_os = "windows")]
        self.monitor_usb(cx);
        if self.config.as_ref() == Some(&config) {
            if self.status().is_disconnected() {
                self.reconnect(cx);
            }
            return;
        }
        self.cancel_in_flight();
        self.connection_task = None;
        self.reader = None;
        self.config = Some(config);
        #[cfg(target_os = "windows")]
        {
            self.usb_disconnect_pending = false;
            self.usb_reconnect_pending = false;
        }
        self.reconnect(cx);
    }

    pub fn start_identification(&mut self, cx: &mut Context<Self>) {
        let Some(reader) = self.reader.clone() else {
            return;
        };
        let Ok(attempt_id) = self.machine.begin_identify() else {
            return;
        };

        let cancel = Arc::new(AtomicBool::new(false));
        self.active_cancel = Some(cancel.clone());
        cx.notify();

        let background = cx
            .background_executor()
            .spawn(async move { reader.identify(cancel) });

        self.capture_task = Some(cx.spawn(async move |this, cx| {
            let outcome = background.await;
            let _ = this.update(cx, |state, cx| {
                if let Some(pensioner_id) = state.machine.apply_result(attempt_id, outcome) {
                    state.active_cancel = None;
                    state.capture_task = None;
                    cx.emit(ReaderEvent::Identified(pensioner_id));
                    cx.notify();
                } else if !state.machine.status().is_capturing() {
                    state.active_cancel = None;
                    state.capture_task = None;
                    cx.notify();
                }
            });
        }));
    }

    pub fn start_enrollment(&mut self, pensioner_id: PensionerId, cx: &mut Context<Self>) {
        let Some(reader) = self.reader.clone() else {
            return;
        };
        let Ok(attempt_id) = self.machine.begin_enroll(pensioner_id) else {
            return;
        };

        let cancel = Arc::new(AtomicBool::new(false));
        self.active_cancel = Some(cancel.clone());
        cx.notify();

        let background = cx
            .background_executor()
            .spawn(async move { reader.enroll(pensioner_id, cancel) });

        self.capture_task = Some(cx.spawn(async move |this, cx| {
            let outcome = background.await;
            let _ = this.update(cx, |state, cx| {
                if let Some(enrolled_id) = state.machine.apply_enroll_result(attempt_id, outcome) {
                    state.active_cancel = None;
                    state.capture_task = None;
                    cx.emit(ReaderEvent::Enrolled(enrolled_id));
                    cx.notify();
                } else if !state.machine.status().is_capturing() {
                    state.active_cancel = None;
                    state.capture_task = None;
                    cx.notify();
                }
            });
        }));
    }

    pub fn cancel_capture(&mut self, cx: &mut Context<Self>) {
        self.cancel_in_flight();
        if self.machine.cancel() {
            cx.notify();
        }
    }

    pub fn disconnect(&mut self, cx: &mut Context<Self>) {
        let Some(reader) = self
            .reader
            .clone()
            .filter(|_| self.connection_task.is_none())
        else {
            return;
        };
        self.cancel_in_flight();
        let background = cx
            .background_executor()
            .spawn(async move { reader.disconnect() });
        self.connection_task = Some(cx.spawn(async move |this, cx| {
            background.await;
            let _ = this.update(cx, |state, cx| {
                state.connection_task = None;
                #[cfg(target_os = "windows")]
                state.apply_pending_usb_events(cx);
                cx.notify();
            });
        }));
        self.machine.mark_disconnected(READER_DISCONNECTED);
        cx.notify();
    }

    #[cfg(target_os = "windows")]
    fn monitor_usb(&mut self, cx: &mut Context<Self>) {
        if self.usb_monitor.is_some() {
            return;
        }
        let (sender, receiver) = async_channel::unbounded();
        let registration = match UsbReaderNotifications::register(sender) {
            Ok(registration) => registration,
            Err(code) => {
                eprintln!(
                    "No se pudo escuchar la conexión USB de SecuGen (código Windows {code})."
                );
                return;
            }
        };
        self.usb_monitor = Some(cx.spawn(async move |this, cx| {
            let _registration = registration;
            while let Ok(event) = receiver.recv().await {
                if this
                    .update(cx, |state, cx| state.on_usb_event(event, cx))
                    .is_err()
                {
                    break;
                }
            }
        }));
    }

    #[cfg(target_os = "windows")]
    fn on_usb_event(&mut self, event: UsbReaderEvent, cx: &mut Context<Self>) {
        match event {
            UsbReaderEvent::Attached => self.usb_reconnect_pending = true,
            UsbReaderEvent::Detached => {
                self.usb_reconnect_pending = false;
                self.usb_disconnect_pending = true;
                self.cancel_in_flight();
                self.machine.mark_disconnected(READER_DISCONNECTED);
                cx.notify();
            }
        }
        self.apply_pending_usb_events(cx);
    }

    #[cfg(target_os = "windows")]
    fn apply_pending_usb_events(&mut self, cx: &mut Context<Self>) {
        if self.connection_task.is_some() {
            return;
        }
        if mem::take(&mut self.usb_disconnect_pending) {
            self.disconnect(cx);
        }
        if self.connection_task.is_none()
            && mem::take(&mut self.usb_reconnect_pending)
            && self.status().is_disconnected()
        {
            self.connect(cx, true);
        }
    }

    #[cfg(target_os = "windows")]
    pub fn reconnect(&mut self, cx: &mut Context<Self>) {
        self.connect(cx, false);
    }

    #[cfg(target_os = "windows")]
    fn connect(&mut self, cx: &mut Context<Self>, wait_for_usb_driver: bool) {
        if !self.can_reconnect() {
            return;
        }
        let Some(config) = self.config.clone() else {
            return;
        };
        self.cancel_in_flight();
        if self.reader.is_none() {
            match SecuGenBiometricReader::new(config) {
                Ok(reader) => self.reader = Some(Arc::new(reader)),
                Err(err) => {
                    self.machine.mark_disconnected(err.message());
                    cx.notify();
                    return;
                }
            }
        }
        let Some(reader) = self.reader.clone() else {
            return;
        };
        self.machine
            .mark_disconnected("Conectando el lector SecuGen…");
        cx.notify();
        let executor = cx.background_executor().clone();
        let background = cx.background_executor().spawn(async move {
            if wait_for_usb_driver {
                executor.timer(Duration::from_millis(350)).await;
            }
            let result = reader.connect();
            if wait_for_usb_driver && matches!(result, Err(BiometricError::Disconnected(_))) {
                executor.timer(Duration::from_millis(700)).await;
                return reader.connect();
            }
            result
        });

        self.connection_task = Some(cx.spawn(async move |this, cx| {
            let result = background.await;
            let _ = this.update(cx, |state, cx| {
                state.connection_task = None;
                if !state.usb_disconnect_pending {
                    match result {
                        Ok(()) => state.machine.mark_reconnected(),
                        Err(err) => state.machine.mark_disconnected(err.message()),
                    }
                }
                state.apply_pending_usb_events(cx);
                cx.notify();
            });
        }));
    }

    #[cfg(not(target_os = "windows"))]
    pub fn reconnect(&mut self, _: &mut Context<Self>) {}

    pub fn reset_for_sign_out(&mut self, cx: &mut Context<Self>) {
        self.cancel_in_flight();
        if self.connection_task.is_none() && !self.status().is_disconnected() {
            if self
                .reader
                .as_ref()
                .is_some_and(|reader| reader.is_connected())
            {
                self.machine.mark_reconnected();
            } else {
                self.machine.mark_disconnected("Lector no conectado.");
            }
        }
        cx.notify();
    }

    fn cancel_in_flight(&mut self) {
        if let Some(cancel) = self.active_cancel.take() {
            cancel.store(true, Ordering::SeqCst);
        }
        self.capture_task = None;
    }
}

#[cfg(test)]
mod tests {
    use super::{ReaderMachine, ReaderState, ReaderStatus};
    use crate::{
        features::pensioners::model::PensionerId,
        services::biometric::{BiometricError, CaptureOutcome, EnrollOutcome},
    };

    #[test]
    fn reader_starts_disconnected_until_preferences_are_loaded() {
        let reader = ReaderState::new();
        assert!(reader.status().is_disconnected());
        assert!(!reader.can_reconnect());
        assert!(reader.reader.is_none());
    }

    #[test]
    fn enforces_exclusive_capture_and_ignores_stale_cancelled_results() {
        let mut machine = ReaderMachine::new(true);
        assert_eq!(machine.status(), &ReaderStatus::Ready);

        let first = machine.begin_identify().expect("first capture starts");
        assert!(machine.status().is_capturing());
        assert_eq!(machine.begin_identify(), Err(BiometricError::Busy));
        assert_eq!(
            machine.begin_enroll(PensionerId(1004)),
            Err(BiometricError::Busy)
        );

        assert!(machine.cancel());
        assert_eq!(machine.status(), &ReaderStatus::Cancelled);

        // Resultado tardío del intento cancelado debe ignorarse por completo.
        assert_eq!(
            machine.apply_result(first, Ok(CaptureOutcome::Matched(PensionerId(1001)))),
            None
        );
        assert_eq!(machine.status(), &ReaderStatus::Cancelled);

        // El siguiente intento funciona limpiamente tras cancelar.
        let second = machine.begin_identify().expect("retry after cancel works");
        assert_eq!(
            machine.apply_result(second, Ok(CaptureOutcome::Matched(PensionerId(1002)))),
            Some(PensionerId(1002))
        );
        assert_eq!(
            machine.status(),
            &ReaderStatus::Matched {
                pensioner_id: PensionerId(1002)
            }
        );
    }

    #[test]
    fn enrollment_machine_handles_mismatch_cancel_and_success_exclusively() {
        let mut machine = ReaderMachine::new(true);
        let id = PensionerId(1004);

        let first = machine.begin_enroll(id).expect("start enrollment");
        assert!(machine.status().is_enrolling());
        assert_eq!(machine.begin_identify(), Err(BiometricError::Busy));

        assert_eq!(
            machine.apply_enroll_result(first, Ok(EnrollOutcome::Mismatch)),
            None
        );
        assert_eq!(machine.status(), &ReaderStatus::EnrollmentMismatch);

        let cancelled = machine.begin_enroll(id).expect("retry enrollment");
        assert!(machine.cancel());
        assert_eq!(
            machine.apply_enroll_result(cancelled, Ok(EnrollOutcome::Enrolled(id))),
            None
        );
        assert_eq!(machine.status(), &ReaderStatus::Cancelled);

        let final_attempt = machine.begin_enroll(id).expect("third attempt");
        assert_eq!(
            machine.apply_enroll_result(final_attempt, Ok(EnrollOutcome::Enrolled(id))),
            Some(id)
        );
        assert_eq!(
            machine.status(),
            &ReaderStatus::Enrolled { pensioner_id: id }
        );
    }

    #[test]
    fn handles_unrecognized_disconnect_and_reconnect_flow() {
        let mut machine = ReaderMachine::new(true);

        let attempt = machine.begin_identify().expect("capture starts");
        assert_eq!(
            machine.apply_result(attempt, Ok(CaptureOutcome::Unrecognized)),
            None
        );
        assert_eq!(machine.status(), &ReaderStatus::Unrecognized);

        machine.mark_disconnected("Cable desconectado");
        assert!(machine.status().is_disconnected());
        assert!(matches!(
            machine.begin_identify(),
            Err(BiometricError::Disconnected(_))
        ));

        machine.mark_reconnected();
        assert_eq!(machine.status(), &ReaderStatus::Ready);

        let retry = machine.begin_identify().expect("capture after reconnect");
        assert_eq!(
            machine.apply_result(retry, Ok(CaptureOutcome::Matched(PensionerId(1003)))),
            Some(PensionerId(1003))
        );
    }
}
