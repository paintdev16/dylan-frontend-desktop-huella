use gpui_kit::{
    component::{IconName, StyledExt as _, h_flex, label::Label, v_flex},
    prelude::FluentBuilder as _,
    *,
};

use crate::{
    components::{button::AppButton, loading, metric_card::MetricCard},
    state::store::{CalendarDate, CashMovementKind, DataLoad},
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
        let permissions = self.session.read(cx).pensioner_permissions();
        let query = self.store.read(cx).pensioner_history(id);
        let history = query.and_then(|query| query.value.as_ref());
        let history_loading = query.is_some_and(|query| query.status() == DataLoad::Loading);
        let history_error = query.and_then(|query| query.error.clone());
        let pensioner = history.map_or(pensioner, |history| &history.pensioner);
        let consumptions = history.and_then(|history| history.consumptions.as_deref());
        let payments = history.and_then(|history| history.payments.as_deref());
        let can_view_consumptions = consumptions.is_some();
        let can_view_tickets =
            payments.is_some() && self.session.read(cx).can_view_pensioner_payments();
        let today = CalendarDate::today();
        let month = format!("{:04}-{:02}", today.year, today.month);
        let monthly_consumption = consumptions
            .unwrap_or_default()
            .iter()
            .filter(|row| row.date.year == today.year && row.date.month == today.month)
            .map(|row| row.consumption_cents)
            .sum();
        let monthly_tickets = payments
            .unwrap_or_default()
            .iter()
            .filter(|row| {
                row.kind == CashMovementKind::TicketCredit && row.service_date.starts_with(&month)
            })
            .map(|row| row.amount_cents)
            .sum();
        let monthly_rows = history.map_or_else(Vec::new, |history| history.monthly());
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
                    ),
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
            .when_some(history_error, |detail, error| {
                detail.child(
                    h_flex()
                        .items_center()
                        .gap(Spacing::SM)
                        .child(Label::new(error))
                        .child(
                            AppButton::new("retry-pensioner-history")
                                .secondary()
                                .small()
                                .label("Reintentar")
                                .on_click(cx.listener(|view, _, _, cx| view.load_data(true, cx))),
                        ),
                )
            })
            .when(history_loading, |detail| {
                detail.child(loading::rows(
                    "pensioner-history-loading",
                    "Cargando historial del pensionista",
                    3,
                    cx,
                ))
            })
            .when(can_view_consumptions, |detail| {
                detail.child(history_panel(
                    "Historial de consumos",
                    "No hay consumos registrados.",
                    consumptions.unwrap_or_default().iter().map(|record| {
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
                    "No hay abonos registrados.",
                    payments.unwrap_or_default().iter().map(|record| {
                        let payment = if record.kind == CashMovementKind::TicketCredit {
                            format!(
                                "{} ticket(s) × {}",
                                record.ticket_count.unwrap_or_default(),
                                format_soles(record.ticket_unit_cents.unwrap_or_default())
                            )
                        } else {
                            "Pago en efectivo".to_string()
                        };
                        (
                            format!("{} · {} · {payment}", record.service_date, record.code),
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
                            let balance_text = if !(can_view_consumptions && can_view_tickets) {
                                String::new()
                            } else if row.net_balance_cents > 0 {
                                format!("+{} a favor", format_soles(row.net_balance_cents))
                            } else if row.net_balance_cents < 0 {
                                format!(
                                    "-{} deuda",
                                    format_soles(row.net_balance_cents.saturating_abs())
                                )
                            } else {
                                "Al día".to_string()
                            };
                            let mut values = vec![row.month_label.clone()];
                            if can_view_consumptions {
                                values.push(format!(
                                    "Consumido {}",
                                    format_soles(row.consumed_cents)
                                ));
                            }
                            if can_view_tickets {
                                values.push(format!(
                                    "Tickets {} ({}) · Efectivo {}",
                                    row.tickets_count,
                                    format_soles(row.tickets_cents),
                                    format_soles(row.cash_paid_cents)
                                ));
                            }
                            (values.join(" · "), balance_text)
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
