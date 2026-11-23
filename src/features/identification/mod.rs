use std::collections::VecDeque;

use gpui_kit::{
    component::{
        Disableable as _, Icon, IconName, Sizable as _, StyledExt as _, WindowExt as _,
        button::Button,
        h_flex,
        label::Label,
        menu::{DropdownMenu as _, PopupMenuItem},
        v_flex,
    },
    prelude::FluentBuilder as _,
    *,
};

use crate::{
    components::{
        button::AppButton,
        status_badge::{StatusBadge, StatusTone},
    },
    features::pensioners::model::{Pensioner, PensionerId, format_soles},
    services::biometric::{BiometricBackendMode, simulator::SimulatedScenario},
    state::{
        reader::{ReaderState, ReaderStatus},
        session::{AppRoute, SessionState},
        store::ComedorStore,
    },
    theme::{ActiveDylanTheme as _, Radii, Spacing, Typography},
};

const FINGERPRINT_SVG: &[u8] = br#"<svg xmlns="http://www.w3.org/2000/svg" width="24" height="24" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.9" stroke-linecap="round" stroke-linejoin="round"><path d="M12 10a2 2 0 0 0-2 2c0 1.02-.1 2.51-.26 4"/><path d="M14 13.12c0 2.38 0 6.38-1 8.88"/><path d="M17.29 21.02c.12-.6.43-2.3.5-3.02"/><path d="M2 12a10 10 0 0 1 18-6"/><path d="M2 16h.01"/><path d="M21.8 16c.2-2 .131-5.354 0-6"/><path d="M5 19.5C5.5 18 6 15 6 12a6 6 0 0 1 .34-2"/><path d="M8.65 22c.21-.66.45-1.32.57-2"/><path d="M9 6.8a6 6 0 0 1 9 5.2v2"/></svg>"#;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IdentificationEvent {
    OpenPensioner(PensionerId),
    FocusSearch,
}

const EVENT_LIMIT: usize = 8;

#[derive(Default)]
struct IdentificationHistory {
    last_match: Option<PensionerId>,
    events: VecDeque<ReaderStatus>,
    last_status: Option<ReaderStatus>,
}

impl IdentificationHistory {
    fn observe(&mut self, status: &ReaderStatus) {
        if self.last_status.as_ref() == Some(status) {
            return;
        }
        if let ReaderStatus::Matched { pensioner_id } = status {
            self.last_match = Some(*pensioner_id);
        }
        self.last_status = Some(status.clone());
        self.events.push_front(status.clone());
        self.events.truncate(EVENT_LIMIT);
    }

    fn reset(&mut self, current_status: &ReaderStatus) {
        *self = Self {
            last_status: Some(current_status.clone()),
            ..Self::default()
        };
    }
}

pub struct IdentificationView {
    focus: FocusHandle,
    store: Entity<ComedorStore>,
    session: Entity<SessionState>,
    reader: Entity<ReaderState>,
    history: IdentificationHistory,
    simulation_target: Option<PensionerId>,
    modal_open: bool,
    _subscriptions: [Subscription; 3],
}

impl EventEmitter<IdentificationEvent> for IdentificationView {}

impl IdentificationView {
    pub fn new(
        store: Entity<ComedorStore>,
        session: Entity<SessionState>,
        reader: Entity<ReaderState>,
        cx: &mut Context<Self>,
    ) -> Self {
        let mut history = IdentificationHistory::default();
        history.reset(reader.read(cx).status());
        let observe_reader = cx.observe(&reader, |view, reader, cx| {
            let status = reader.read(cx).status();
            if view
                .session
                .read(cx)
                .can_access_route(AppRoute::Identification)
            {
                view.history.observe(status);
            } else {
                view.history.reset(status);
            }
            cx.notify();
        });
        let observe_store = cx.observe(&store, |_, _, cx| cx.notify());
        let observe_session = cx.observe(&session, |view, session, cx| {
            if !session.read(cx).can_access_route(AppRoute::Identification) {
                view.history.reset(view.reader.read(cx).status());
                view.simulation_target = None;
            }
            cx.notify();
        });
        Self {
            focus: cx.focus_handle(),
            store,
            session,
            reader,
            history,
            simulation_target: None,
            modal_open: false,
            _subscriptions: [observe_reader, observe_store, observe_session],
        }
    }

    pub fn focus(&self, window: &mut Window, cx: &mut App) {
        window.focus(&self.focus, cx);
    }

    pub fn set_modal_open(&mut self, open: bool, cx: &mut Context<Self>) {
        if self.modal_open != open {
            self.modal_open = open;
            cx.notify();
        }
    }

    fn can_start(&self, window: &mut Window, cx: &mut App) -> bool {
        !self.modal_open
            && !window.has_active_dialog(cx)
            && self
                .session
                .read(cx)
                .can_access_route(AppRoute::Identification)
    }

    fn simulate(&self, scenario: SimulatedScenario, window: &mut Window, cx: &mut Context<Self>) {
        if !self.can_start(window, cx) {
            return;
        }
        if let SimulatedScenario::Match { pensioner_id, .. } = scenario
            && !self
                .store
                .read(cx)
                .ledger()
                .find_pensioner(pensioner_id)
                .is_some_and(simulation_candidate)
        {
            return;
        }
        self.reader.update(cx, |reader, cx| {
            reader.simulate_identification(scenario, cx)
        });
    }

    fn render_reader(
        &self,
        status: &ReaderStatus,
        simulated: bool,
        cx: &mut Context<Self>,
    ) -> impl IntoElement + use<> {
        let palette = cx.dylan_theme().palette;
        let tone = status_tone(status);
        let tint = match tone {
            StatusTone::Success => palette.success,
            StatusTone::Warning => palette.warning,
            StatusTone::Error => palette.error,
            StatusTone::Info => palette.info,
            StatusTone::Neutral => palette.neutral,
        };
        let detail = match status {
            ReaderStatus::Disconnected { reason } => reason.as_ref(),
            ReaderStatus::Ready if simulated => {
                "Elige un escenario del simulador para iniciar una lectura."
            }
            ReaderStatus::Ready => "Inicia la lectura y coloca un dedo en el lector.",
            ReaderStatus::Capturing { .. } => "Esperando la respuesta del lector.",
            ReaderStatus::Matched { .. } => "La huella coincide con un pensionista.",
            ReaderStatus::Unrecognized => {
                "Puedes reintentar o usar el buscador de la barra de título."
            }
            ReaderStatus::Cancelled => "El lector está disponible para un nuevo intento.",
            ReaderStatus::Enrolled { .. } => {
                "La huella quedó registrada. Puedes iniciar una identificación."
            }
            ReaderStatus::EnrollmentMismatch => {
                "Repite el enrolamiento desde la ficha del pensionista."
            }
        };

        panel("Estado del lector", cx)
            .child(
                v_flex()
                    .w_full()
                    .items_center()
                    .gap(Spacing::SM)
                    .p(Spacing::MD)
                    .rounded(Radii::MD)
                    .bg(palette.surface_subtle)
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .justify_center()
                            .size(px(76.))
                            .rounded(Radii::FULL)
                            .border_1()
                            .border_color(tint.border)
                            .bg(tint.tint)
                            .child(
                                Icon::default()
                                    .data(FINGERPRINT_SVG)
                                    .size(px(38.))
                                    .text_color(tint.icon),
                            ),
                    )
                    .child(
                        StatusBadge::new(tone, status.short_label()).loading(status.is_capturing()),
                    )
                    .child(
                        Label::new(detail.to_owned())
                            .text_size(Typography::SM)
                            .text_center()
                            .text_color(palette.muted_foreground),
                    ),
            )
            .when(self.modal_open, |card| {
                card.child(StatusBadge::new(
                    StatusTone::Warning,
                    "Lector en pausa mientras el modal está abierto",
                ))
            })
            .child(
                h_flex()
                    .flex_wrap()
                    .gap(Spacing::SM)
                    .when(
                        !simulated && !status.is_disconnected() && !status.is_capturing(),
                        |row| {
                            row.child(
                                AppButton::new("reader-start")
                                    .label("Iniciar lectura")
                                    .disabled(self.modal_open)
                                    .on_click(cx.listener(|view, _, window, cx| {
                                        if view.can_start(window, cx) {
                                            view.reader.update(cx, |reader, cx| {
                                                reader.start_identification(cx)
                                            });
                                        }
                                    })),
                            )
                        },
                    )
                    .when(status.is_capturing() && !status.is_enrolling(), |row| {
                        row.child(
                            AppButton::new("reader-cancel")
                                .secondary()
                                .label("Cancelar captura")
                                .disabled(self.modal_open)
                                .on_click(cx.listener(|view, _, _, cx| {
                                    view.reader
                                        .update(cx, |reader, cx| reader.cancel_capture(cx));
                                })),
                        )
                    })
                    .when(self.session.read(cx).can_search_pensioners(), |row| {
                        row.child(
                            AppButton::new("reader-search")
                                .tertiary()
                                .icon(IconName::Search)
                                .label("Buscar pensionista")
                                .disabled(self.modal_open)
                                .on_click(cx.listener(|_, _, _, cx| {
                                    cx.emit(IdentificationEvent::FocusSearch)
                                })),
                        )
                    }),
            )
    }

    fn render_simulator(
        &self,
        status: &ReaderStatus,
        cx: &mut Context<Self>,
    ) -> impl IntoElement + use<> {
        let palette = cx.dylan_theme().palette;
        let mut candidates: Vec<_> = self
            .store
            .read(cx)
            .ledger()
            .pensioners()
            .iter()
            .filter(|pensioner| simulation_candidate(pensioner))
            .map(|pensioner| (pensioner.id, pensioner.full_name.clone()))
            .collect();
        candidates.sort_by_key(|(id, _)| *id);
        let target = candidates
            .iter()
            .find(|(id, _)| Some(*id) == self.simulation_target)
            .or(candidates.first());
        let target_id = target.map(|(id, _)| *id);
        let target_name = target.map(|(_, name)| name.clone());
        let disabled = self.modal_open || status.is_capturing() || status.is_disconnected();
        let owner = cx.weak_entity();
        let modes = [
            (
                "sim-no-match",
                "Simular huella no reconocida",
                SimulatedScenario::Unrecognized { delay_ms: 450 },
            ),
            (
                "sim-error",
                "Simular desconexión",
                SimulatedScenario::DisconnectDuringCapture { delay_ms: 450 },
            ),
        ];
        panel("Simulador biométrico", cx)
            .child(
                h_flex()
                    .items_center()
                    .flex_wrap()
                    .gap(Spacing::SM)
                    .when_some(target_name, |row, name| {
                        row.child(Label::new(name).text_size(Typography::SM).font_medium())
                            .child(
                                Button::new("sim-target")
                                    .small()
                                    .label("Cambiar pensionista")
                                    .dropdown_caret(true)
                                    .when(disabled, |button| button.disabled(true))
                                    .dropdown_menu_with_anchor(
                                        Anchor::TopLeft,
                                        move |mut menu, _, _| {
                                            for (id, name) in &candidates {
                                                let id = *id;
                                                let owner = owner.clone();
                                                menu = menu.item(
                                                    PopupMenuItem::new(name.clone())
                                                        .checked(Some(id) == target_id)
                                                        .on_click(move |_, _, cx| {
                                                            let _ = owner.update(cx, |view, cx| {
                                                                view.simulation_target = Some(id);
                                                                cx.notify();
                                                            });
                                                        }),
                                                );
                                            }
                                            menu
                                        },
                                    ),
                            )
                    }),
            )
            .when(target_id.is_none(), |card| {
                card.child(
                    Label::new("No hay pensionistas activos con huella registrada.")
                        .text_size(Typography::SM)
                        .text_color(palette.muted_foreground),
                )
            })
            .child(
                h_flex()
                    .flex_wrap()
                    .gap(Spacing::SM)
                    .when_some(target_id, |row, id| {
                        row.child(
                            AppButton::new("sim-match")
                                .small()
                                .label("Simular huella reconocida")
                                .disabled(disabled)
                                .on_click(cx.listener(move |view, _, window, cx| {
                                    view.simulate(
                                        SimulatedScenario::Match {
                                            pensioner_id: id,
                                            delay_ms: 450,
                                        },
                                        window,
                                        cx,
                                    );
                                })),
                        )
                        .child(
                            AppButton::new("sim-slow")
                                .small()
                                .tertiary()
                                .label("Simular lectura lenta")
                                .disabled(disabled)
                                .on_click(cx.listener(move |view, _, window, cx| {
                                    view.simulate(
                                        SimulatedScenario::Match {
                                            pensioner_id: id,
                                            delay_ms: 2000,
                                        },
                                        window,
                                        cx,
                                    );
                                })),
                        )
                    })
                    .children(modes.map(|(id, label, scenario)| {
                        AppButton::new(id)
                            .small()
                            .secondary()
                            .label(label)
                            .disabled(disabled)
                            .on_click(cx.listener(move |view, _, window, cx| {
                                view.simulate(scenario, window, cx)
                            }))
                    })),
            )
    }

    fn render_last_match(&self, cx: &mut Context<Self>) -> impl IntoElement + use<> {
        let palette = cx.dylan_theme().palette;
        let session = self.session.read(cx);
        let can_view = session.can_access_route(AppRoute::Pensioners);
        let ledger = self.store.read(cx).ledger();
        let pensioner = self
            .history
            .last_match
            .and_then(|id| ledger.find_pensioner(id));
        let can_clear = self.history.last_match.is_some()
            && !self.modal_open
            && !self.reader.read(cx).status().is_capturing();
        let content = if let Some(pensioner) = pensioner {
            let id = pensioner.id;
            let balance = pensioner.balance_cents;
            v_flex()
                .gap(Spacing::SM)
                .child(
                    h_flex()
                        .flex_wrap()
                        .gap(Spacing::XS)
                        .items_center()
                        .child(
                            Label::new(pensioner.full_name.clone())
                                .text_size(Typography::LG)
                                .font_semibold(),
                        )
                        .child(StatusBadge::new(
                            if pensioner.active {
                                StatusTone::Success
                            } else {
                                StatusTone::Neutral
                            },
                            if pensioner.active {
                                "Activo"
                            } else {
                                "Inactivo"
                            },
                        )),
                )
                .child(
                    Label::new(format!(
                        "{} · {}",
                        pensioner.code(),
                        if balance < 0 {
                            format!("Deuda pendiente: {}", format_soles(pensioner.debt_cents()))
                        } else if balance > 0 {
                            format!("Saldo a favor: {}", format_soles(balance))
                        } else {
                            "Al día".into()
                        }
                    ))
                    .text_size(Typography::SM)
                    .text_color(palette.muted_foreground),
                )
                .when(can_view, |card| {
                    card.child(
                        h_flex().child(
                            AppButton::new("bio-open-profile")
                                .secondary()
                                .small()
                                .label("Ver ficha")
                                .disabled(self.modal_open)
                                .on_click(cx.listener(move |_, _, _, cx| {
                                    cx.emit(IdentificationEvent::OpenPensioner(id))
                                })),
                        ),
                    )
                })
                .into_any_element()
        } else {
            Label::new(if self.history.last_match.is_some() {
                "La huella coincide con un código que no está en el padrón."
            } else {
                "Todavía no hay una huella reconocida."
            })
            .text_size(Typography::SM)
            .text_color(palette.muted_foreground)
            .into_any_element()
        };
        panel("Última identificación", cx)
            .child(content)
            .when(can_clear, |card| {
                card.child(
                    h_flex().child(
                        AppButton::new("bio-clear-last")
                            .tertiary()
                            .small()
                            .label("Limpiar resultado")
                            .on_click(cx.listener(|view, _, _, cx| {
                                view.history.last_match = None;
                                view.reader
                                    .update(cx, |reader, cx| reader.clear_identification(cx));
                                cx.notify();
                            })),
                    ),
                )
            })
    }

    fn render_events(&self, cx: &App) -> impl IntoElement + use<> {
        let palette = cx.dylan_theme().palette;
        panel("Eventos del lector", cx)
            .when(self.history.events.is_empty(), |card| {
                card.child(
                    Label::new("Todavía no hay eventos del lector.")
                        .text_size(Typography::SM)
                        .text_color(palette.muted_foreground),
                )
            })
            .children(self.history.events.iter().map(|status| {
                h_flex().child(StatusBadge::new(status_tone(status), status.short_label()))
            }))
    }
}

impl Render for IdentificationView {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let palette = cx.dylan_theme().palette;
        let reader = self.reader.read(cx);
        let simulated = reader.backend_mode() == BiometricBackendMode::Simulated;
        let status = reader.status().clone();
        let device = reader.device_name();
        let disconnected = status.is_disconnected();
        let width = window.viewport_size().width;
        let padding = if width < px(640.) {
            Spacing::MD
        } else {
            Spacing::XL
        };
        let wide = width >= px(1080.);
        let reader_panel = self.render_reader(&status, simulated, cx);
        let last_match = self.render_last_match(cx);
        let events = self.render_events(cx);
        let simulator = simulated.then(|| self.render_simulator(&status, cx));
        let columns = div()
            .flex()
            .when(wide, |d| d.flex_row())
            .when(!wide, |d| d.flex_col())
            .items_start()
            .gap(Spacing::LG)
            .w_full()
            .child(
                v_flex()
                    .w_full()
                    .flex_1()
                    .min_w_0()
                    .gap(Spacing::LG)
                    .child(reader_panel)
                    .children(simulator),
            )
            .child(
                v_flex()
                    .min_w_0()
                    .gap(Spacing::LG)
                    .w_full()
                    .when(wide, |column| column.w(px(300.)).flex_shrink_0())
                    .child(last_match)
                    .child(events),
            );

        v_flex()
            .id("identification-scroll")
            .track_focus(&self.focus)
            .flex_1()
            .min_h_0()
            .w_full()
            .overflow_y_scroll()
            .items_center()
            .p(padding)
            .child(
                v_flex()
                    .w_full()
                    .max_w(px(1080.))
                    .gap(Spacing::LG)
                    .child(
                        h_flex()
                            .flex_wrap()
                            .w_full()
                            .justify_between()
                            .items_center()
                            .gap(Spacing::SM)
                            .child(
                                v_flex()
                                    .gap(Spacing::XS)
                                    .child(
                                        Label::new("Identificación biométrica")
                                            .text_size(Typography::PAGE_TITLE)
                                            .font_bold(),
                                    )
                                    .child(
                                        Label::new(device)
                                            .text_size(Typography::SM)
                                            .text_color(palette.muted_foreground),
                                    ),
                            )
                            .child(
                                h_flex().flex_wrap().items_center().gap(Spacing::SM).child(
                                    AppButton::new("reader-connection")
                                        .secondary()
                                        .small()
                                        .label(if disconnected {
                                            "Conectar lector"
                                        } else {
                                            "Desconectar"
                                        })
                                        .disabled(self.modal_open)
                                        .on_click(cx.listener(|view, _, window, cx| {
                                            if view.can_start(window, cx) {
                                                view.reader.update(cx, |reader, cx| {
                                                    if reader.status().is_disconnected() {
                                                        reader.reconnect(cx);
                                                    } else {
                                                        reader.disconnect(cx);
                                                    }
                                                });
                                            }
                                        })),
                                ),
                            ),
                    )
                    .child(columns),
            )
    }
}

fn simulation_candidate(pensioner: &Pensioner) -> bool {
    pensioner.active && pensioner.fingerprint_enrolled
}

fn panel(title: &'static str, cx: &App) -> Div {
    let palette = cx.dylan_theme().palette;
    v_flex()
        .w_full()
        .gap(Spacing::SM)
        .p(Spacing::LG)
        .rounded(Radii::LG)
        .bg(palette.surface)
        .border_1()
        .border_color(palette.border)
        .child(Label::new(title).text_size(Typography::LG).font_semibold())
}

fn status_tone(status: &ReaderStatus) -> StatusTone {
    match status {
        ReaderStatus::Disconnected { .. } => StatusTone::Error,
        ReaderStatus::Ready | ReaderStatus::Matched { .. } | ReaderStatus::Enrolled { .. } => {
            StatusTone::Success
        }
        ReaderStatus::Capturing { .. } => StatusTone::Info,
        ReaderStatus::Unrecognized | ReaderStatus::EnrollmentMismatch => StatusTone::Warning,
        ReaderStatus::Cancelled => StatusTone::Neutral,
    }
}

#[cfg(test)]
mod tests {
    use super::{EVENT_LIMIT, IdentificationHistory, ReaderStatus};
    use crate::features::pensioners::model::PensionerId;

    #[test]
    fn history_preserves_last_match_after_other_events_and_clears_on_reset() {
        let mut log = IdentificationHistory::default();
        log.observe(&ReaderStatus::Matched {
            pensioner_id: PensionerId(1001),
        });
        log.observe(&ReaderStatus::Unrecognized);
        log.observe(&ReaderStatus::Unrecognized);
        assert_eq!(log.last_match, Some(PensionerId(1001)));
        assert_eq!(log.events.len(), 2);

        for _ in 0..EVENT_LIMIT {
            log.observe(&ReaderStatus::Ready);
            log.observe(&ReaderStatus::Cancelled);
        }
        assert_eq!(log.events.len(), EVENT_LIMIT);

        log.reset(&ReaderStatus::Ready);
        log.observe(&ReaderStatus::Ready);
        assert_eq!(log.last_match, None);
        assert!(log.events.is_empty());
    }
}
