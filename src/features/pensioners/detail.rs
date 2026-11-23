use gpui_kit::{
    component::{IconName, StyledExt as _, h_flex, label::Label, v_flex},
    prelude::FluentBuilder as _,
    *,
};

use crate::{
    components::{button::AppButton, metric_card::MetricCard},
    state::{session::AppRoute, store::DatePeriodFilter},
    theme::{ActiveDylanTheme as _, MetricAccent, Radii, Spacing, Typography},
};

use super::{
    PensionersEvent, PensionersView, account_status, fingerprint_status,
    model::{Pensioner, format_soles},
    profile_metadata,
};

impl PensionersView {
    pub(super) fn render_detail(
        &self,
        pensioner: &Pensioner,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let palette = cx.dylan_theme().palette;
        let id = pensioner.id;
        let session = self.session.read(cx);
        let permissions = session.pensioner_permissions();
        let can_view_consumptions = session.can_access_route(AppRoute::Consumptions);
        let can_view_tickets = session.can_view_pensioner_payments();
        let ledger = self.store.read(cx).ledger();
        let today = ledger.current_date();
        let consumptions: Vec<_> = ledger
            .attendances()
            .iter()
            .rev()
            .filter(|record| {
                can_view_consumptions && record.pensioner_id == id && record.rations() > 0
            })
            .collect();
        let tickets: Vec<_> = ledger
            .ticket_movements()
            .iter()
            .rev()
            .filter(|record| can_view_tickets && record.pensioner_id == id)
            .collect();
        let monthly_consumption = consumptions
            .iter()
            .filter(|record| DatePeriodFilter::ThisMonth.contains(record.date, today))
            .map(|record| record.consumption_cents)
            .sum();
        let monthly_tickets = tickets
            .iter()
            .filter(|record| DatePeriodFilter::ThisMonth.contains(record.date, today))
            .map(|record| record.amount_cents)
            .sum();
        let monthly_rows = ledger.monthly_summary(id);
        let mut metadata = vec![pensioner.code()];
        metadata.extend(profile_metadata(pensioner));
        let info = metadata.join(" · ");

        v_flex()
            .id(("pensioner-sheet", id.0 as usize))
            .size_full()
            .min_h_0()
            .overflow_y_scroll()
            .gap(Spacing::LG)
            .child(
                h_flex().child(
                    AppButton::new("back-to-pensioners")
                        .tertiary()
                        .icon(IconName::ArrowLeft)
                        .label("Volver al listado")
                        .on_click(cx.listener(|view, _, window, cx| {
                            view.focus(window, cx);
                            cx.emit(PensionersEvent::Cleared);
                        })),
                ),
            )
            .child(
                v_flex()
                    .gap(Spacing::SM)
                    .child(
                        h_flex()
                            .flex_wrap()
                            .justify_between()
                            .items_center()
                            .gap(Spacing::SM)
                            .child(
                                Label::new(pensioner.full_name.clone())
                                    .text_size(Typography::PAGE_TITLE)
                                    .font_bold(),
                            )
                            .child(
                                h_flex()
                                    .flex_wrap()
                                    .gap(Spacing::XS)
                                    .when(permissions.update, |actions| {
                                        actions.child(
                                            AppButton::new("edit-pensioner-detail")
                                                .secondary()
                                                .label("Editar")
                                                .on_click(cx.listener(
                                                    move |view, _, window, cx| {
                                                        view.open_editor(Some(id), window, cx)
                                                    },
                                                )),
                                        )
                                    })
                                    .when(
                                        permissions.enroll && pensioner.can_enroll_fingerprint(),
                                        |actions| {
                                            actions.child(
                                                AppButton::new("enroll-pensioner-detail")
                                                    .secondary()
                                                    .label("Registrar huella")
                                                    .on_click(cx.listener(move |_, _, _, cx| {
                                                        cx.emit(PensionersEvent::OpenEnrollment(id))
                                                    })),
                                            )
                                        },
                                    ),
                            ),
                    )
                    .child(Label::new(info).text_color(palette.muted_foreground))
                    .child(
                        Label::new(format!(
                            "Tickets registrados: {}",
                            pensioner.tickets_balance
                        ))
                        .text_size(Typography::SM)
                        .text_color(palette.muted_foreground),
                    )
                    .child(
                        h_flex()
                            .flex_wrap()
                            .gap(Spacing::XS)
                            .child(account_status(pensioner))
                            .child(fingerprint_status(pensioner)),
                    )
                    .when(!pensioner.notes.is_empty(), |header| {
                        header.child(
                            Label::new(pensioner.notes.clone())
                                .text_color(palette.muted_foreground),
                        )
                    }),
            )
            .child(
                v_flex()
                    .gap(Spacing::SM)
                    .child(
                        Label::new("Resumen del mes")
                            .text_size(Typography::LG)
                            .font_semibold(),
                    )
                    .child(
                        h_flex()
                            .flex_wrap()
                            .items_stretch()
                            .gap(Spacing::MD)
                            .when(can_view_consumptions, |row| {
                                row.child(MetricCard::new(
                                    MetricAccent::Neutral,
                                    "Consumo del mes",
                                    format_soles(monthly_consumption),
                                ))
                            })
                            .when(can_view_tickets, |row| {
                                row.child(MetricCard::new(
                                    MetricAccent::TicketsReceived,
                                    "Abonos con tickets del mes",
                                    format_soles(monthly_tickets),
                                ))
                            })
                            .child(
                                MetricCard::new(
                                    MetricAccent::CreditBalance,
                                    "Saldo a favor",
                                    format_soles(pensioner.credit_cents()),
                                )
                                .subtitle("Saldo actual"),
                            )
                            .child(
                                MetricCard::new(
                                    MetricAccent::PendingDebt,
                                    "Deuda pendiente",
                                    format_soles(pensioner.debt_cents()),
                                )
                                .subtitle("Saldo actual"),
                            ),
                    ),
            )
            .when(can_view_consumptions, |detail| {
                detail.child(history_panel(
                    "Historial de consumos",
                    "No hay consumos registrados.",
                    consumptions.iter().map(|record| {
                        (
                            format!(
                                "{} · {} desayuno(s) · {} almuerzo(s)",
                                record.date.format_iso(),
                                record.breakfasts,
                                record.lunches
                            ),
                            format_soles(record.consumption_cents),
                        )
                    }),
                    cx,
                ))
            })
            .when(can_view_tickets, |detail| {
                detail.child(history_panel(
                    "Historial de abonos",
                    "No hay abonos con tickets registrados.",
                    tickets.iter().map(|record| {
                        (
                            format!(
                                "{} · {} · {} ticket(s) × {}",
                                record.date.format_iso(),
                                record.movement_code,
                                record.tickets_count,
                                format_soles(record.ticket_unit_cents),
                            ),
                            format_soles(record.amount_cents),
                        )
                    }),
                    cx,
                ))
            })
            .when(
                (can_view_consumptions || can_view_tickets) && !monthly_rows.is_empty(),
                |detail| {
                    detail.child(history_panel(
                        "Desglose mensual",
                        "Sin actividad mensual registrada.",
                        monthly_rows.iter().map(|row| {
                            let balance_text = if row.net_balance_cents > 0 {
                                format!("+{} a favor", format_soles(row.net_balance_cents))
                            } else if row.net_balance_cents < 0 {
                                format!(
                                    "-{} deuda",
                                    format_soles(row.net_balance_cents.saturating_abs())
                                )
                            } else {
                                "Al día".to_string()
                            };
                            (
                                format!(
                                    "{} · Consumido {} · Tickets {} ({}) · Efectivo {}",
                                    row.month_label,
                                    format_soles(row.consumed_cents),
                                    row.tickets_count,
                                    format_soles(row.tickets_cents),
                                    format_soles(row.cash_paid_cents),
                                ),
                                balance_text,
                            )
                        }),
                        cx,
                    ))
                },
            )
    }
}

fn history_panel(
    title: &'static str,
    empty: &'static str,
    records: impl ExactSizeIterator<Item = (String, String)>,
    cx: &App,
) -> impl IntoElement {
    let palette = cx.dylan_theme().palette;
    v_flex()
        .gap(Spacing::SM)
        .p(Spacing::MD)
        .rounded(Radii::LG)
        .bg(palette.surface)
        .border_1()
        .border_color(palette.border)
        .child(Label::new(title).text_size(Typography::LG).font_semibold())
        .when(records.len() == 0, |panel| {
            panel.child(Label::new(empty).text_color(palette.muted_foreground))
        })
        .children(records.map(|(description, amount)| {
            h_flex()
                .justify_between()
                .flex_wrap()
                .gap(Spacing::XS)
                .border_t_1()
                .border_color(palette.border)
                .pt(Spacing::SM)
                .child(Label::new(description).text_size(Typography::SM))
                .child(Label::new(amount).text_size(Typography::SM).font_medium())
        }))
}
