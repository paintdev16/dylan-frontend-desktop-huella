use gpui_kit::{
    component::{IconName, StyledExt as _, h_flex, label::Label, v_flex},
    prelude::FluentBuilder as _,
    *,
};

use crate::{
    components::{
        button::AppButton,
        status_badge::{StatusBadge, StatusTone},
    },
    features::pensioners::model::PensionerId,
    services::biometric::BiometricBackendMode,
    state::{
        reader::{ReaderState, ReaderStatus},
        store::ComedorStore,
    },
    theme::{ActiveDylanTheme as _, Radii, Spacing, Typography},
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EnrollmentModalEvent {
    Closed,
}

/// Modal de **Enrolamiento biométrico** (Etapa 5) lanzado desde la ficha del pensionista.
/// Observa el estado tipado de `ReaderState` sin recibir muestras ni plantillas crudas.
pub struct EnrollmentModal {
    focus: FocusHandle,
    store: Entity<ComedorStore>,
    reader: Entity<ReaderState>,
    target_id: Option<PensionerId>,
    _subscriptions: [Subscription; 2],
}

impl EventEmitter<EnrollmentModalEvent> for EnrollmentModal {}

impl EnrollmentModal {
    pub(super) fn can_use_reader(temporary_profile: bool, reader: &ReaderState) -> bool {
        // El respaldo SecuGen sobrevive al catálogo temporal y conserva sus códigos.
        !temporary_profile || reader.backend_mode() == BiometricBackendMode::Simulated
    }

    pub fn new(
        store: Entity<ComedorStore>,
        reader: Entity<ReaderState>,
        cx: &mut Context<Self>,
    ) -> Self {
        let focus = cx.focus_handle();
        let observe_store = cx.observe(&store, |_, _, cx| cx.notify());
        let observe_reader = cx.observe(&reader, |_, _, cx| cx.notify());
        Self {
            focus,
            store,
            reader,
            target_id: None,
            _subscriptions: [observe_store, observe_reader],
        }
    }

    pub fn is_open(&self) -> bool {
        self.target_id.is_some()
    }

    pub fn open(
        &mut self,
        pensioner_id: PensionerId,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> bool {
        if self.target_id.is_some() {
            return false;
        }
        if self
            .store
            .read(cx)
            .ledger()
            .find_pensioner(pensioner_id)
            .is_none()
        {
            return false;
        }
        self.target_id = Some(pensioner_id);
        window.focus(&self.focus, cx);
        cx.notify();
        true
    }

    pub fn close(&mut self, cx: &mut Context<Self>) {
        if self.target_id.take().is_some() {
            if self.reader.read(cx).status().is_enrolling() {
                self.reader
                    .update(cx, |reader, cx| reader.cancel_capture(cx));
            }
            cx.emit(EnrollmentModalEvent::Closed);
            cx.notify();
        }
    }

    pub fn close_for_sign_out(&mut self, cx: &mut Context<Self>) {
        if self.target_id.take().is_some() {
            cx.notify();
        }
    }
}

impl Render for EnrollmentModal {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let Some(pensioner_id) = self.target_id else {
            return div().hidden().into_any_element();
        };

        let palette = cx.dylan_theme().palette;
        let compact = window.viewport_size().width < px(600.0);

        let Some(pensioner) = self
            .store
            .read(cx)
            .ledger()
            .find_pensioner(pensioner_id)
            .cloned()
        else {
            return div().hidden().into_any_element();
        };

        let reader_ref = self.reader.read(cx);
        let status = reader_ref.status().clone();
        let device_name = reader_ref.device_name();
        let is_capturing = status.is_capturing();
        let is_disconnected = status.is_disconnected();
        let can_use_reader = Self::can_use_reader(pensioner.temporary_profile, reader_ref);

        let (tone, heading, detail) = match &status {
            _ if !can_use_reader => (
                StatusTone::Warning,
                "Perfil temporal",
                "Usa el simulador para este pensionista. El lector físico requiere un perfil con un código persistente.".to_string(),
            ),
            ReaderStatus::Disconnected { reason } => (
                StatusTone::Error,
                "Lector no disponible",
                reason.to_string(),
            ),
            ReaderStatus::Capturing { .. } => (
                StatusTone::Info,
                "Capturando muestras para enrolamiento…",
                "Coloca el mismo dedo en el lector dos veces seguidas. La ventana permanece responsiva y puedes cancelar en cualquier momento.".to_string(),
            ),
            ReaderStatus::Enrolled { pensioner_id: enrolled } if *enrolled == pensioner_id => (
                StatusTone::Success,
                "Huella enrolada correctamente",
                format!(
                    "La plantilla quedó registrada bajo el identificador canónico {} sin exponer muestras a la interfaz.",
                    pensioner.code()
                ),
            ),
            ReaderStatus::EnrollmentMismatch => (
                StatusTone::Warning,
                "Las dos capturas no coincidieron",
                "Usa el mismo dedo en ambas lecturas y vuelve a iniciar el enrolamiento.".to_string(),
            ),
            ReaderStatus::Cancelled => (
                StatusTone::Neutral,
                "Enrolamiento cancelado",
                "El intento fue invalidado de forma segura. Puedes reintentar cuando estés listo.".to_string(),
            ),
            _ => (
                StatusTone::Success,
                if pensioner.fingerprint_enrolled {
                    "Listo para re-enrolar huella"
                } else {
                    "Listo para enrolar huella"
                },
                "Se capturarán dos muestras en el hilo dedicado del lector y se almacenará solo la plantilla en el motor biométrico.".to_string(),
            ),
        };

        let card = v_flex()
            .id("enrollment-modal-card")
            .occlude()
            .w(px(520.0))
            .max_w(window.viewport_size().width - px(32.0))
            .rounded(Radii::XL)
            .bg(palette.surface)
            .border_1()
            .border_color(palette.border)
            .shadow(palette.brand.card_shadow())
            .p(if compact { Spacing::MD } else { Spacing::XL })
            .gap(Spacing::MD)
            .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
            .child(
                h_flex()
                    .w_full()
                    .items_center()
                    .justify_between()
                    .gap(Spacing::SM)
                    .child(
                        v_flex()
                            .min_w_0()
                            .gap(Spacing::XXS)
                            .child(
                                Label::new("Enrolamiento biométrico")
                                    .text_size(Typography::XL)
                                    .font_bold()
                                    .text_color(palette.foreground),
                            )
                            .child(
                                Label::new(device_name)
                                    .text_size(Typography::XS)
                                    .text_color(palette.muted_foreground),
                            ),
                    )
                    .child(
                        AppButton::new("enroll-modal-close-icon")
                            .tertiary()
                            .xsmall()
                            .icon(IconName::Close)
                            .tooltip("Cerrar (Esc)")
                            .on_click(cx.listener(|view, _, _, cx| view.close(cx))),
                    ),
            )
            .child(
                v_flex()
                    .w_full()
                    .gap(Spacing::XXS)
                    .p(Spacing::MD)
                    .rounded(Radii::MD)
                    .bg(palette.surface_subtle)
                    .border_1()
                    .border_color(palette.border)
                    .child(
                        Label::new(pensioner.full_name.clone())
                            .text_size(Typography::LG)
                            .font_semibold()
                            .text_color(palette.foreground),
                    )
                    .child(
                        Label::new(format!(
                            "ID canónico {} · DNI {} · {}",
                            pensioner.code(),
                            pensioner.dni,
                            if pensioner.fingerprint_enrolled {
                                "Con huella registrada"
                            } else {
                                "Sin huella registrada"
                            }
                        ))
                        .text_size(Typography::XS)
                        .text_color(palette.muted_foreground),
                    ),
            )
            .child(
                v_flex()
                    .w_full()
                    .gap(Spacing::XS)
                    .p(Spacing::MD)
                    .rounded(Radii::MD)
                    .bg(palette.surface_subtle)
                    .child(StatusBadge::new(tone, status.short_label()).loading(is_capturing))
                    .child(
                        Label::new(heading)
                            .text_size(Typography::BASE)
                            .font_semibold()
                            .text_color(palette.foreground),
                    )
                    .child(
                        Label::new(detail)
                            .text_size(Typography::SM)
                            .text_color(palette.muted_foreground),
                    ),
            )
            .child(
                h_flex()
                    .w_full()
                    .flex_wrap()
                    .items_center()
                    .justify_between()
                    .gap(Spacing::SM)
                    .pt(Spacing::XS)
                    .border_t_1()
                    .border_color(palette.border)
                    .child(
                        AppButton::new("enroll-modal-close")
                            .tertiary()
                            .label("Cerrar")
                            .on_click(cx.listener(|view, _, _, cx| view.close(cx))),
                    )
                    .child(
                        h_flex()
                            .flex_wrap()
                            .items_center()
                            .gap(Spacing::XS)
                            .when(is_disconnected, |bar| {
                                bar.child(
                                    AppButton::new("enroll-reconnect")
                                        .secondary()
                                        .icon(IconName::RefreshCw)
                                        .label("Reconectar lector")
                                        .on_click(cx.listener(|view, _, _, cx| {
                                            view.reader
                                                .update(cx, |reader, cx| reader.reconnect(cx));
                                        })),
                                )
                            })
                            .when(is_capturing, |bar| {
                                bar.child(
                                    AppButton::new("enroll-cancel-capture")
                                        .destructive()
                                        .icon(IconName::Close)
                                        .label("Cancelar captura")
                                        .on_click(cx.listener(|view, _, _, cx| {
                                            view.reader.update(cx, |reader, cx| {
                                                reader.cancel_capture(cx);
                                            });
                                        })),
                                )
                            })
                            .child(
                                AppButton::new("enroll-start")
                                    .primary()
                                    .icon(IconName::Check)
                                    .label(if is_capturing {
                                        "Enrolando…"
                                    } else if pensioner.fingerprint_enrolled {
                                        "Re-enrolar huella"
                                    } else {
                                        "Iniciar enrolamiento"
                                    })
                                    .loading(is_capturing)
                                    .disabled(is_capturing || is_disconnected || !can_use_reader)
                                    .on_click(cx.listener(move |view, _, _, cx| {
                                        let pensioner = view
                                            .store
                                            .read(cx)
                                            .ledger()
                                            .find_pensioner(pensioner_id);
                                        if pensioner.is_some_and(|p| {
                                            Self::can_use_reader(
                                                p.temporary_profile,
                                                view.reader.read(cx),
                                            )
                                        }) {
                                            view.reader.update(cx, |reader, cx| {
                                                reader.start_enrollment(pensioner_id, cx);
                                            });
                                        }
                                    })),
                            ),
                    ),
            );

        let backdrop = div()
            .id("enrollment-modal-backdrop")
            .track_focus(&self.focus)
            .occlude()
            .absolute()
            .inset_0()
            .flex()
            .items_center()
            .justify_center()
            .bg(black().alpha(0.45))
            .on_key_down(cx.listener(|view, event: &KeyDownEvent, _, cx| {
                if event.keystroke.key == "escape" {
                    cx.stop_propagation();
                    view.close(cx);
                }
            }))
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(|view, _, _, cx| {
                    cx.stop_propagation();
                    view.close(cx);
                }),
            )
            .child(card);

        deferred(backdrop).with_priority(2).into_any_element()
    }
}
