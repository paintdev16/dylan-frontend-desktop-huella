pub mod attendance;
pub mod confirm_attendance;
pub(crate) mod order;

use gpui_kit::{
    component::{StyledExt as _, h_flex, label::Label, v_flex},
    prelude::FluentBuilder as _,
    *,
};

use crate::{
    components::{
        loading,
        status_badge::{StatusBadge, StatusTone},
    },
    features::pensioners::model::format_soles,
    state::store::{ComedorStore, DataLoad},
    theme::{ActiveDylanTheme as _, Radii, Spacing, Typography},
};

/// Módulo de **Consumos** (Etapa 3): muestra el historial detallado de atenciones confirmadas.
pub struct ConsumptionsView {
    store: Entity<ComedorStore>,
    _subscription: Subscription,
}

impl ConsumptionsView {
    pub fn new(store: Entity<ComedorStore>, cx: &mut Context<Self>) -> Self {
        let observe_store = cx.observe(&store, |_, _, cx| cx.notify());
        Self {
            store,
            _subscription: observe_store,
        }
    }
}

impl Render for ConsumptionsView {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let palette = cx.dylan_theme().palette;
        let compact = window.viewport_size().width < px(640.0);
        let outer_padding = if compact { Spacing::MD } else { Spacing::XL };
        let card_padding = if compact { Spacing::MD } else { Spacing::LG };

        let store = self.store.read(cx);
        let load = store.attendances_load();
        let attendances: Vec<_> = store.ledger().attendances().iter().rev().cloned().collect();

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
                        v_flex()
                            .w_full()
                            .gap(Spacing::XXS)
                            .p(card_padding)
                            .rounded(Radii::LG)
                            .bg(palette.surface)
                            .border_1()
                            .border_color(palette.border)
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
                                    .when(load == DataLoad::Ready, |header| header.child(
                                        Label::new(format!("{} registros", attendances.len()))
                                            .text_size(Typography::XS)
                                            .text_color(palette.muted_foreground),
                                    ))
                                    .when(load == DataLoad::Loading, |header| {
                                        header.child(loading::line("attendance-count-loading", px(66.)))
                                    }),
                            )
                            .child(if load == DataLoad::Loading {
                                loading::rows("consumption-history-loading", "Cargando atenciones", 4, cx)
                                    .into_any_element()
                            } else if load == DataLoad::Failed {
                                Label::new("No se pudo cargar el historial de atenciones.")
                                    .text_size(Typography::SM)
                                    .text_color(palette.muted_foreground)
                                    .into_any_element()
                            } else if attendances.is_empty() {
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
                                                    .when(record.has_consumptions, |row| row.child(
                                                        Label::new(format!("Desayunos {} · Almuerzos {} · {}", record.breakfasts, record.lunches, format_soles(record.consumption_cents)))
                                                            .text_size(Typography::SM)
                                                            .text_color(palette.foreground)
                                                    ))
                                                    .when(record.has_payments, |row| row.child(
                                                        Label::new(format!("Tickets {} × {} · {}{}", record.tickets_count,
                                                            format_soles(record.ticket_unit_cents), format_soles(record.ticket_amount_cents),
                                                            record.movement_code.as_ref().map_or_else(String::new, |code| format!(" · {code}"))))
                                                            .text_size(Typography::SM)
                                                            .text_color(palette.foreground)
                                                    ))
                                                    .when(record.has_balance, |row| row.child(
                                                        Label::new(format!("Saldo {} → {}", format_soles(record.balance_before_cents), format_soles(record.balance_after_cents)))
                                                            .text_size(Typography::XS)
                                                            .font_semibold()
                                                            .text_color(if record.balance_after_cents < 0 {
                                                                palette.error.foreground
                                                            } else {
                                                                palette.success.foreground
                                                            }),
                                                    )),
                                            )
                                            .children(record.items.iter().zip(&record.item_prices).map(|(item, price)| {
                                                Label::new(format!(
                                                    "{} · {} · {} × {}",
                                                    item.meal.label(),
                                                    item.menu.label(),
                                                    item.portions,
                                                    format_soles(*price),
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
