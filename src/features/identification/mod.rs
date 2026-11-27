use std::collections::VecDeque;

use gpui_kit::{
    component::{Icon, IconName, StyledExt as _, WindowExt as _, h_flex, label::Label, v_flex},
    prelude::FluentBuilder as _,
    *,
};

use crate::{
    components::{
        button::AppButton,
        loading,
        status_badge::{StatusBadge, StatusTone},
    },
    features::pensioners::model::{PensionerId, format_soles},
    state::{
        reader::{ReaderState, ReaderStatus},
        session::{AppRoute, SessionState},
        store::{ComedorStore, DataLoad},
    },
    theme::{ActiveDylanTheme as _, Radii, Spacing, Typography},
};

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
    modal_open: bool,
    _subscriptions: [Subscription; 3],
}

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
            }
            cx.notify();
        });
        Self {
            focus: cx.focus_handle(),
            store,
            session,
            reader,
            history,
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

    fn render_reader(
        &self,
        status: &ReaderStatus,
        can_reconnect: bool,
        cx: &mut Context<Self>,
    ) -> impl IntoElement + use<> {
        let palette = cx.dylan_theme().palette;
        if self.reader.read(cx).connection_loading() {
            return panel("Estado del lector", cx).child(
                loading::region("reader-status-loading", "Comprobando el lector biométrico")
                    .w_full()
                    .items_center()
                    .p(Spacing::MD)
                    .rounded(Radii::MD)
                    .bg(palette.surface_subtle)
                    .child(
                        div()
                            .w(px(76.))
                            .child(loading::block("device-icon", px(76.))),
                    )
                    .child(loading::line("status", px(94.)))
                    .child(loading::line("description", px(196.))),
            );
        }
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
                                    .data(include_bytes!("../../../assets/fingerprint.svg"))
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
                    .when(status.is_disconnected() && can_reconnect, |row| {
                        row.child(
                            AppButton::new("reader-reconnect-inline")
                                .primary()
                                .icon(IconName::RefreshCw)
                                .label("Reconectar lector")
                                .disabled(self.modal_open)
                                .on_click(cx.listener(|view, _, window, cx| {
                                    if view.can_start(window, cx) {
                                        view.reader.update(cx, |reader, cx| reader.reconnect(cx));
                                    }
                                })),
                        )
                    })
                    .when(!status.is_disconnected() && !status.is_capturing(), |row| {
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
                    })
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
                    }),
            )
    }

    fn render_last_match(&self, cx: &mut Context<Self>) -> impl IntoElement + use<> {
        let palette = cx.dylan_theme().palette;
        let store = self.store.read(cx);
        let ledger = store.ledger();
        let last_match = self.history.last_match;
        let identified = last_match.and_then(|id| store.identified_pensioner(id));
        let identified_profile = identified.and_then(|query| query.value.as_ref());
        let pensioner =
            identified_profile.or_else(|| last_match.and_then(|id| ledger.find_pensioner(id)));
        let profile_loading = identified.is_some_and(|query| query.status() == DataLoad::Loading)
            || last_match
                .and_then(|id| store.pensioner_history(id))
                .is_some_and(|query| query.status() == DataLoad::Loading);
        let can_clear = self.history.last_match.is_some()
            && !self.modal_open
            && !self.reader.read(cx).status().is_capturing();
        let content = if let Some(pensioner) = pensioner {
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
                    Label::new(pensioner.code())
                        .text_size(Typography::SM)
                        .text_color(palette.muted_foreground),
                )
                .when_some(identified_profile, |card, profile| {
                    card.child(
                        Label::new(format!(
                            "Saldo actual: {}",
                            format_soles(profile.balance_cents)
                        ))
                        .text_size(Typography::SM)
                        .font_semibold(),
                    )
                })
                .into_any_element()
        } else if self.history.last_match.is_some()
            && (self.modal_open || profile_loading || store.pensioners_load() == DataLoad::Loading)
        {
            loading::region(
                "last-identification-loading",
                "Cargando el pensionista identificado",
            )
            .child(loading::line("name", px(180.)))
            .child(loading::line("code", px(86.)))
            .into_any_element()
        } else {
            Label::new(if self.history.last_match.is_some() {
                identified
                    .and_then(|query| query.error.as_deref())
                    .unwrap_or("No se pudo obtener la ficha asociada a esta huella.")
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
        let can_reconnect = reader.can_reconnect();
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
        let reader_panel = self.render_reader(&status, can_reconnect, cx);
        let last_match = self.render_last_match(cx);
        let events = self.render_events(cx);
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
                    .child(reader_panel),
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
                                        .disabled(
                                            self.modal_open || (disconnected && !can_reconnect),
                                        )
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
