pub mod attendance;
pub mod confirm_attendance;
pub(crate) mod order;

use gpui_kit::{
    component::{IconName, StyledExt as _, h_flex, label::Label, v_flex},
    prelude::FluentBuilder as _,
    *,
};

use crate::{
    components::{
        button::AppButton,
        metric_card::MetricCard,
        status_badge::{StatusBadge, StatusTone},
    },
    features::pensioners::{SelectedPensioner, model::format_soles},
    state::{
        session::{AppRoute, SessionState},
        store::ComedorStore,
    },
    theme::{ActiveDylanTheme as _, MetricAccent, Radii, Spacing, Typography},
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConsumptionsEvent {
    OpenIdentification,
    OpenPensioners,
    OpenCash,
}

/// Módulo de **Consumos** (Etapa 3): muestra el historial de atenciones confirmadas,
/// métricas consolidadas y el saldo del pensionista seleccionado.
pub struct ConsumptionsView {
    store: Entity<ComedorStore>,
    session: Entity<SessionState>,
    selected: Option<SelectedPensioner>,
    _subscriptions: [Subscription; 2],
}

impl EventEmitter<ConsumptionsEvent> for ConsumptionsView {}

impl ConsumptionsView {
    pub fn new(
        store: Entity<ComedorStore>,
        session: Entity<SessionState>,
        cx: &mut Context<Self>,
    ) -> Self {
        let observe_store = cx.observe(&store, |_, _, cx| cx.notify());
        let observe_session = cx.observe(&session, |_, _, cx| cx.notify());
        Self {
            store,
            session,
            selected: None,
            _subscriptions: [observe_store, observe_session],
        }
    }

    pub fn set_selected(&mut self, selected: Option<SelectedPensioner>, cx: &mut Context<Self>) {
        if self.selected != selected {
            self.selected = selected;
            cx.notify();
        }
    }
}

impl Render for ConsumptionsView {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let palette = cx.dylan_theme().palette;
        let compact = window.viewport_size().width < px(640.0);
        let outer_padding = if compact { Spacing::MD } else { Spacing::XL };
        let card_padding = if compact { Spacing::MD } else { Spacing::LG };

        let (can_open_cash, can_open_id, can_open_pensioners) = {
            let session = self.session.read(cx);
            (
                session.can_access_route(AppRoute::Cash),
                session.can_access_route(AppRoute::Identification),
                session.can_access_route(AppRoute::Pensioners),
            )
        };

        let store = self.store.read(cx);
        let ledger = store.ledger();
        let cash_open = ledger.is_cash_open();
        let total_breakfasts = ledger.total_breakfasts();
        let total_lunches = ledger.total_lunches();
        let total_tickets = ledger.total_tickets_received();
        let total_ticket_amount = ledger.total_ticket_amount_cents();
        let attendances: Vec<_> = ledger.attendances().iter().rev().cloned().collect();
        let selected_pensioner = self.selected.and_then(|sel| {
            ledger
                .find_pensioner(sel.id)
                .cloned()
                .map(|p| (p, sel.source))
        });

        v_flex()
            .id("consumptions-scroll")
            .relative()
            .flex_1()
            .min_h_0()
            .w_full()
            .overflow_y_scroll()
            .items_center()
            .p(outer_padding)
            .child(
                v_flex()
                    .w_full()
                    .max_w(px(1080.0))
                    .gap(if compact { Spacing::MD } else { Spacing::LG })
                    .child(
                        h_flex()
                            .w_full()
                            .flex_wrap()
                            .items_center()
                            .justify_between()
                            .gap(Spacing::MD)
                            .p(card_padding)
                            .rounded(Radii::LG)
                            .bg(palette.surface)
                            .border_1()
                            .border_color(palette.border)
                            .child(
                                v_flex()
                                    .min_w_0()
                                    .gap(Spacing::XXS)
                                    .child(
                                        Label::new("Consumos y atenciones")
                                            .text_size(if compact {
                                                Typography::XL
                                            } else {
                                                Typography::PAGE_TITLE
                                            })
                                            .font_bold()
                                            .text_color(palette.foreground),
                                    )
                                    .child(
                                        Label::new(
                                            "Atenciones confirmadas desde Buscar pensionista (Ctrl+K) o Identificación.",
                                        )
                                        .text_size(Typography::SM)
                                        .text_color(palette.muted_foreground),
                                    ),
                            )
                            .child(
                                h_flex()
                                    .flex_wrap()
                                    .items_center()
                                    .gap(Spacing::SM)
                                    .child(if cash_open {
                                        StatusBadge::new(StatusTone::Success, "Caja abierta")
                                    } else {
                                        StatusBadge::new(StatusTone::Warning, "Caja cerrada")
                                    })
                                    .when(can_open_cash && !cash_open, |this| {
                                        this.child(
                                            AppButton::new("consumptions-open-cash")
                                                .secondary()
                                                .small()
                                                .icon(IconName::Inbox)
                                                .label("Abrir caja")
                                                .on_click(cx.listener(|_, _, _, cx| {
                                                    cx.emit(ConsumptionsEvent::OpenCash);
                                                })),
                                        )
                                    }),
                            ),
                    )
                    .child(
                        h_flex()
                            .w_full()
                            .flex_wrap()
                            .gap(Spacing::MD)
                            .child(
                                MetricCard::new(
                                    MetricAccent::Breakfasts,
                                    "Desayunos",
                                    total_breakfasts.to_string(),
                                )
                                .icon(IconName::Sun)
                                .subtitle("Confirmados hoy"),
                            )
                            .child(
                                MetricCard::new(
                                    MetricAccent::Lunches,
                                    "Almuerzos",
                                    total_lunches.to_string(),
                                )
                                .icon(IconName::Calendar)
                                .subtitle("Confirmados hoy"),
                            )
                            .child(
                                MetricCard::new(
                                    MetricAccent::TicketsReceived,
                                    "Tickets recibidos",
                                    total_tickets.to_string(),
                                )
                                .icon(IconName::Info)
                                .subtitle(format!(
                                    "Abono {}",
                                    format_soles(total_ticket_amount)
                                )),
                            )
                            .child(
                                MetricCard::new(
                                    MetricAccent::Neutral,
                                    "Atenciones",
                                    attendances.len().to_string(),
                                )
                                .icon(IconName::FileText)
                                .subtitle(format!(
                                    "{} raciones totales",
                                    total_breakfasts + total_lunches
                                )),
                            ),
                    )
                    .when_some(selected_pensioner, |this, (pensioner, source)| {
                        this.child(
                            h_flex()
                                .w_full()
                                .flex_wrap()
                                .items_center()
                                .justify_between()
                                .gap(Spacing::MD)
                                .p(card_padding)
                                .rounded(Radii::LG)
                                .bg(palette.surface)
                                .border_1()
                                .border_color(palette.border)
                                .child(
                                    v_flex()
                                        .min_w_0()
                                        .gap(Spacing::XXS)
                                        .child(
                                            h_flex()
                                                .flex_wrap()
                                                .items_center()
                                                .gap(Spacing::SM)
                                                .child(
                                                    Label::new(format!(
                                                        "Seleccionado: {} ({})",
                                                        pensioner.full_name,
                                                        pensioner.code()
                                                    ))
                                                    .text_size(Typography::BASE)
                                                    .font_semibold()
                                                    .text_color(palette.foreground),
                                                )
                                                .child(StatusBadge::new(
                                                    StatusTone::Neutral,
                                                    source.label(),
                                                )),
                                        )
                                        .child(
                                            Label::new(format!(
                                                "Saldo individual: {} · Tickets registrados: {}",
                                                format_soles(pensioner.balance_cents),
                                                pensioner.tickets_balance
                                            ))
                                            .text_size(Typography::XS)
                                            .text_color(palette.muted_foreground),
                                        ),
                                ),
                        )
                    })
                    .child(
                        v_flex()
                            .w_full()
                            .gap(Spacing::MD)
                            .p(card_padding)
                            .rounded(Radii::LG)
                            .bg(palette.surface)
                            .border_1()
                            .border_color(palette.border)
                            .child(
                                h_flex()
                                    .w_full()
                                    .flex_wrap()
                                    .items_center()
                                    .justify_between()
                                    .gap(Spacing::SM)
                                    .child(
                                        Label::new("Historial de atenciones confirmadas")
                                            .text_size(Typography::LG)
                                            .font_semibold()
                                            .text_color(palette.foreground),
                                    )
                                    .child(
                                        h_flex()
                                            .flex_wrap()
                                            .gap(Spacing::XS)
                                            .when(can_open_id, |bar| {
                                                bar.child(
                                                    AppButton::new("consumptions-go-id")
                                                        .tertiary()
                                                        .small()
                                                        .icon(IconName::Search)
                                                        .label("Ir a Identificación")
                                                        .on_click(cx.listener(|_, _, _, cx| {
                                                            cx.emit(
                                                                ConsumptionsEvent::OpenIdentification,
                                                            );
                                                        })),
                                                )
                                            })
                                            .when(can_open_pensioners, |bar| {
                                                bar.child(
                                                    AppButton::new("consumptions-go-pensioners")
                                                        .tertiary()
                                                        .small()
                                                        .icon(IconName::User)
                                                        .label("Ver Pensionistas")
                                                        .on_click(cx.listener(|_, _, _, cx| {
                                                            cx.emit(
                                                                ConsumptionsEvent::OpenPensioners,
                                                            );
                                                        })),
                                                )
                                            }),
                                    ),
                            )
                            .child(if attendances.is_empty() {
                                v_flex()
                                    .w_full()
                                    .items_center()
                                    .gap(Spacing::SM)
                                    .py(Spacing::XL)
                                    .px(Spacing::MD)
                                    .rounded(Radii::MD)
                                    .bg(palette.surface_subtle)
                                    .child(StatusBadge::new(
                                        StatusTone::Neutral,
                                        "Sin atenciones registradas",
                                    ))
                                    .child(
                                        Label::new(
                                            "Busca un pensionista con Ctrl+K o identifícalo por huella para abrir Atención automática.",
                                        )
                                        .text_size(Typography::SM)
                                        .text_center()
                                        .text_color(palette.muted_foreground),
                                    )
                                    .into_any_element()
                            } else {
                                v_flex()
                                    .w_full()
                                    .gap(Spacing::SM)
                                    .children(attendances.into_iter().map(|record| {
                                        let tone = match (
                                            record.rations() > 0,
                                            record.tickets_count > 0,
                                        ) {
                                            (true, true) => StatusTone::Success,
                                            (true, false) => StatusTone::Info,
                                            (false, true) => StatusTone::Neutral,
                                            (false, false) => StatusTone::Neutral,
                                        };

                                        v_flex()
                                            .w_full()
                                            .gap(Spacing::XS)
                                            .p(Spacing::MD)
                                            .rounded(Radii::MD)
                                            .bg(palette.surface_subtle)
                                            .border_1()
                                            .border_color(palette.border)
                                            .child(
                                                h_flex()
                                                    .w_full()
                                                    .flex_wrap()
                                                    .items_center()
                                                    .justify_between()
                                                    .gap(Spacing::SM)
                                                    .child(
                                                        h_flex()
                                                            .flex_wrap()
                                                            .items_center()
                                                            .gap(Spacing::SM)
                                                            .child(
                                                                Label::new(
                                                                    record.pensioner_name.clone(),
                                                                )
                                                                .text_size(Typography::BASE)
                                                                .font_semibold()
                                                                .text_color(palette.foreground),
                                                            )
                                                            .child(
                                                                Label::new(
                                                                    record.pensioner_code.clone(),
                                                                )
                                                                .text_size(Typography::XS)
                                                                .font_medium()
                                                                .text_color(palette.brand_accent),
                                                            )
                                                            .child(StatusBadge::new(
                                                                tone,
                                                                record.kind_label(),
                                                            )),
                                                    )
                                                    .child(
                                                        Label::new(format!(
                                                            "{} · {}",
                                                            record.recorded_at,
                                                            record.source.label()
                                                        ))
                                                        .text_size(Typography::XS)
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
                                                    .child(
                                                        Label::new(if record.tickets_count > 0 {
                                                            let mov_suffix = record
                                                                .movement_code
                                                                .as_deref()
                                                                .map(|code| format!(" · {code}"))
                                                                .unwrap_or_default();
                                                            format!(
                                                                "Desayunos: {} · Almuerzos: {} ({}) · Tickets: {} × {} ({}{mov_suffix})",
                                                                record.breakfasts,
                                                                record.lunches,
                                                                format_soles(record.consumption_cents),
                                                                record.tickets_count,
                                                                format_soles(record.ticket_unit_cents),
                                                                format_soles(record.ticket_amount_cents),
                                                            )
                                                        } else {
                                                            format!(
                                                                "Desayunos: {} · Almuerzos: {} ({}) · Tickets: 0",
                                                                record.breakfasts,
                                                                record.lunches,
                                                                format_soles(record.consumption_cents),
                                                            )
                                                        })
                                                        .text_size(Typography::SM)
                                                        .text_color(palette.foreground),
                                                    )
                                                    .child(
                                                        Label::new(format!(
                                                            "Saldo: {} → {}",
                                                            format_soles(
                                                                record.balance_before_cents
                                                            ),
                                                            format_soles(record.balance_after_cents)
                                                        ))
                                                        .text_size(Typography::XS)
                                                        .font_semibold()
                                                        .text_color(
                                                            if record.balance_after_cents < 0 {
                                                                palette.error.foreground
                                                            } else {
                                                                palette.success.foreground
                                                            },
                                                        ),
                                                    ),
                                            )
                                            .children(record.items.iter().map(|item| {
                                                Label::new(format!(
                                                    "{} · {} · {} × {}",
                                                    item.meal.label(),
                                                    item.menu.label(),
                                                    item.portions,
                                                    format_soles(item.menu.unit_cents(item.meal)),
                                                ))
                                                .text_size(Typography::XS)
                                                .text_color(palette.muted_foreground)
                                            }))
                                            .when_some(record.picked_up_by, |row, name| {
                                                row.child(
                                                    Label::new(format!("Recogido por: {name}"))
                                                        .text_size(Typography::XS)
                                                        .text_color(palette.muted_foreground),
                                                )
                                            })
                                    }))
                                    .into_any_element()
                            }),
                    ),
            )
    }
}
