use gpui_kit::{
    component::{
        IconName, Sizable as _, StyledExt as _, h_flex,
        input::{Input, InputEvent, InputState},
        label::Label,
        v_flex,
    },
    prelude::FluentBuilder as _,
    *,
};

use crate::{
    components::{
        button::AppButton,
        metric_card::MetricCard,
        status_badge::{StatusBadge, StatusTone},
    },
    features::pensioners::model::format_soles,
    state::store::{CalendarDate, ComedorStore, DatePeriodFilter},
    theme::{ActiveDylanTheme as _, MetricAccent, Radii, Spacing, Typography},
};

/// Módulo de **Reportes** (Etapa 4): consulta los mismos registros confirmados que
/// Consumos, Caja y Dashboard con filtros por fecha (preajustes y rango personalizado).
pub struct ReportsView {
    store: Entity<ComedorStore>,
    period: DatePeriodFilter,
    from_input: Entity<InputState>,
    to_input: Entity<InputState>,
    filter_error: Option<SharedString>,
    _subscriptions: [Subscription; 3],
}

impl ReportsView {
    pub fn new(store: Entity<ComedorStore>, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let today = store.read(cx).ledger().current_date().format_iso();
        let from_input = cx.new(|cx| {
            InputState::new(window, cx)
                .placeholder("YYYY-MM-DD")
                .default_value(today.clone())
        });
        let to_input = cx.new(|cx| {
            InputState::new(window, cx)
                .placeholder("YYYY-MM-DD")
                .default_value(today)
        });

        let observe_store = cx.observe(&store, |_, _, cx| cx.notify());
        let sub_from = cx.subscribe(&from_input, |view, _, event, cx| {
            if matches!(event, InputEvent::Change) && view.filter_error.is_some() {
                view.filter_error = None;
                cx.notify();
            }
        });
        let sub_to = cx.subscribe(&to_input, |view, _, event, cx| {
            if matches!(event, InputEvent::Change) && view.filter_error.is_some() {
                view.filter_error = None;
                cx.notify();
            }
        });

        Self {
            store,
            period: DatePeriodFilter::Today,
            from_input,
            to_input,
            filter_error: None,
            _subscriptions: [observe_store, sub_from, sub_to],
        }
    }

    fn apply_preset(
        &mut self,
        preset: DatePeriodFilter,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.period = preset;
        self.filter_error = None;
        let today = self.store.read(cx).ledger().current_date();
        let (from, to) = match preset {
            DatePeriodFilter::Today => (today, today),
            DatePeriodFilter::Last7Days => (today.sub_days(6), today),
            DatePeriodFilter::ThisMonth => (
                CalendarDate::new(today.year, today.month, 1).unwrap_or(today),
                today,
            ),
            DatePeriodFilter::All => (today.sub_days(30), today),
            DatePeriodFilter::Custom { from, to } => (from, to),
        };
        self.from_input.update(cx, |input, cx| {
            input.set_value(from.format_iso(), window, cx)
        });
        self.to_input
            .update(cx, |input, cx| input.set_value(to.format_iso(), window, cx));
        cx.notify();
    }

    fn apply_custom_range(&mut self, cx: &mut Context<Self>) {
        let from_raw = self.from_input.read(cx).value();
        let to_raw = self.to_input.read(cx).value();
        let Some(from) = CalendarDate::parse_iso(from_raw.as_ref()) else {
            self.filter_error = Some(SharedString::from(
                "Fecha 'Desde' inválida. Usa el formato YYYY-MM-DD.",
            ));
            cx.notify();
            return;
        };
        let Some(to) = CalendarDate::parse_iso(to_raw.as_ref()) else {
            self.filter_error = Some(SharedString::from(
                "Fecha 'Hasta' inválida. Usa el formato YYYY-MM-DD.",
            ));
            cx.notify();
            return;
        };
        if from > to {
            self.filter_error = Some(SharedString::from(
                "La fecha 'Desde' no puede ser posterior a 'Hasta'.",
            ));
            cx.notify();
            return;
        }

        self.filter_error = None;
        self.period = DatePeriodFilter::Custom { from, to };
        cx.notify();
    }
}

impl Render for ReportsView {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let palette = cx.dylan_theme().palette;
        let compact = window.viewport_size().width < px(640.0);
        let outer_padding = if compact { Spacing::MD } else { Spacing::XL };
        let card_padding = if compact { Spacing::MD } else { Spacing::LG };

        let report = self.store.read(cx).ledger().report_for_period(self.period);
        let period_label = self.period.label();
        let attendances: Vec<_> = report.attendances.iter().rev().cloned().collect();
        let movements: Vec<_> = report.ticket_movements.iter().rev().cloned().collect();
        let by_pensioner = report.by_pensioner.clone();

        v_flex()
            .id("reports-scroll")
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
                                        v_flex()
                                            .min_w_0()
                                            .gap(Spacing::XXS)
                                            .child(
                                                Label::new("Reportes")
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
                                                    "Consumos, raciones y abonos con tickets consultados desde la misma fuente que Consumos, Caja y Dashboard.",
                                                )
                                                .text_size(Typography::SM)
                                                .text_color(palette.muted_foreground),
                                            ),
                                    )
                                    .child(StatusBadge::new(
                                        StatusTone::Neutral,
                                        format!("Periodo: {period_label}"),
                                    )),
                            )
                            .child(
                                h_flex()
                                    .w_full()
                                    .flex_wrap()
                                    .items_center()
                                    .gap(Spacing::XS)
                                    .children(DatePeriodFilter::PRESETS.into_iter().map(
                                        |preset| {
                                            let active = self.period == preset;
                                            let btn = AppButton::new(SharedString::from(format!(
                                                "report-period-{preset:?}"
                                            )))
                                            .small()
                                            .label(preset.label())
                                            .on_click(cx.listener(move |view, _, window, cx| {
                                                view.apply_preset(preset, window, cx);
                                            }));
                                            if active {
                                                btn.primary()
                                            } else {
                                                btn.secondary()
                                            }
                                        },
                                    )),
                            )
                            .child(
                                h_flex()
                                    .w_full()
                                    .flex_wrap()
                                    .items_end()
                                    .gap(Spacing::SM)
                                    .child(
                                        v_flex()
                                            .w(px(160.0))
                                            .gap(Spacing::XXS)
                                            .child(
                                                Label::new("Desde (YYYY-MM-DD)")
                                                    .text_size(Typography::XS)
                                                    .font_semibold()
                                                    .text_color(palette.muted_foreground),
                                            )
                                            .child(
                                                Input::new(&self.from_input)
                                                    .small()
                                                    .rounded(Radii::MD)
                                                    .bg(palette.surface),
                                            ),
                                    )
                                    .child(
                                        v_flex()
                                            .w(px(160.0))
                                            .gap(Spacing::XXS)
                                            .child(
                                                Label::new("Hasta (YYYY-MM-DD)")
                                                    .text_size(Typography::XS)
                                                    .font_semibold()
                                                    .text_color(palette.muted_foreground),
                                            )
                                            .child(
                                                Input::new(&self.to_input)
                                                    .small()
                                                    .rounded(Radii::MD)
                                                    .bg(palette.surface),
                                            ),
                                    )
                                    .child(
                                        AppButton::new("report-apply-range")
                                            .secondary()
                                            .small()
                                            .icon(IconName::Calendar)
                                            .label("Aplicar rango")
                                            .on_click(cx.listener(|view, _, _, cx| {
                                                view.apply_custom_range(cx);
                                            })),
                                    )
                                    .when(self.period != DatePeriodFilter::Today, |this| {
                                        this.child(
                                            AppButton::new("report-reset-period")
                                                .tertiary()
                                                .small()
                                                .label("Limpiar filtros")
                                                .on_click(cx.listener(|view, _, window, cx| {
                                                    view.apply_preset(
                                                        DatePeriodFilter::Today,
                                                        window,
                                                        cx,
                                                    );
                                                })),
                                        )
                                    }),
                            )
                            .when_some(self.filter_error.clone(), |this, message| {
                                this.child(StatusBadge::new(StatusTone::Error, message))
                            }),
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
                                    report.breakfasts.to_string(),
                                )
                                .icon(IconName::Sun)
                                .subtitle(period_label.clone()),
                            )
                            .child(
                                MetricCard::new(
                                    MetricAccent::Lunches,
                                    "Almuerzos",
                                    report.lunches.to_string(),
                                )
                                .icon(IconName::Calendar)
                                .subtitle(period_label.clone()),
                            )
                            .child(
                                MetricCard::new(
                                    MetricAccent::Neutral,
                                    "Raciones",
                                    report.rations.to_string(),
                                )
                                .icon(IconName::ChartPie)
                                .subtitle(format!(
                                    "{} atenciones · {}",
                                    report.attendances_count,
                                    format_soles(report.consumption_cents)
                                )),
                            )
                            .child(
                                MetricCard::new(
                                    MetricAccent::TicketsReceived,
                                    "Tickets recibidos",
                                    report.tickets_received.to_string(),
                                )
                                .icon(IconName::Info)
                                    .subtitle(format!(
                                        "{} abonos · {} · Efectivo {}",
                                        report.cash_movements_count,
                                        format_soles(report.ticket_amount_cents),
                                        format_soles(report.cash_paid_cents),
                                    )),
                            )
                            .child(
                                MetricCard::new(
                                    MetricAccent::CreditBalance,
                                    "Saldo a favor",
                                    format_soles(report.credit_balance_cents),
                                )
                                .icon(IconName::Check)
                                .subtitle("Consolidado actual"),
                            )
                            .child(
                                MetricCard::new(
                                    MetricAccent::PendingDebt,
                                    "Deuda pendiente",
                                    format_soles(report.pending_debt_cents),
                                )
                                .icon(IconName::TriangleAlert)
                                .subtitle("Consolidado actual"),
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
                                Label::new("Resumen por pensionista en el periodo")
                                    .text_size(Typography::LG)
                                    .font_semibold()
                                    .text_color(palette.foreground),
                            )
                            .child(if by_pensioner.is_empty() {
                                Label::new(
                                    "No hay consumos ni abonos registrados en el periodo seleccionado.",
                                )
                                .text_size(Typography::SM)
                                .text_color(palette.muted_foreground)
                                .into_any_element()
                            } else {
                                v_flex()
                                    .w_full()
                                    .gap(Spacing::SM)
                                    .children(by_pensioner.into_iter().map(|row| {
                                        h_flex()
                                            .w_full()
                                            .flex_wrap()
                                            .items_center()
                                            .justify_between()
                                            .gap(Spacing::SM)
                                            .p(Spacing::MD)
                                            .rounded(Radii::MD)
                                            .bg(palette.surface_subtle)
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
                                                                Label::new(row.pensioner_name)
                                                                    .text_size(Typography::BASE)
                                                                    .font_semibold()
                                                                    .text_color(palette.foreground),
                                                            )
                                                            .child(
                                                                Label::new(row.pensioner_code)
                                                                    .text_size(Typography::XS)
                                                                    .font_medium()
                                                                    .text_color(
                                                                        palette.brand_accent,
                                                                    ),
                                                            ),
                                                    )
                                                    .child(
                                                        Label::new(format!(
                                                            "{} atenciones · {} des. / {} alm. ({}) · {} tickets ({})",
                                                            row.attendances,
                                                            row.breakfasts,
                                                            row.lunches,
                                                            format_soles(row.consumption_cents),
                                                            row.tickets_count,
                                                            format_soles(row.ticket_amount_cents),
                                                        ))
                                                        .text_size(Typography::XS)
                                                        .text_color(palette.muted_foreground),
                                                    ),
                                            )
                                            .child(
                                                Label::new(format!(
                                                    "Saldo actual: {}",
                                                    format_soles(row.current_balance_cents)
                                                ))
                                                .text_size(Typography::SM)
                                                .font_semibold()
                                                .text_color(if row.current_balance_cents < 0 {
                                                    palette.error.foreground
                                                } else {
                                                    palette.success.foreground
                                                }),
                                            )
                                    }))
                                    .into_any_element()
                            }),
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
                                    .items_center()
                                    .justify_between()
                                    .gap(Spacing::SM)
                                    .child(
                                        Label::new("Detalle de consumos y tickets del periodo")
                                            .text_size(Typography::LG)
                                            .font_semibold()
                                            .text_color(palette.foreground),
                                    )
                                    .child(
                                        Label::new(format!(
                                            "{} atenciones · {} abonos en caja",
                                            attendances.len(),
                                            movements.len()
                                        ))
                                        .text_size(Typography::XS)
                                        .text_color(palette.muted_foreground),
                                    ),
                            )
                            .child(if attendances.is_empty() {
                                v_flex()
                                    .w_full()
                                    .items_center()
                                    .gap(Spacing::XS)
                                    .py(Spacing::LG)
                                    .px(Spacing::MD)
                                    .rounded(Radii::MD)
                                    .bg(palette.surface_subtle)
                                    .child(StatusBadge::new(
                                        StatusTone::Neutral,
                                        "Sin movimientos en este rango",
                                    ))
                                    .child(
                                        Label::new(
                                            "Confirma una atención desde Buscar pensionista (Ctrl+K) o Identificación para verla reflejada aquí.",
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
                                        let kind_label = record.kind_label();
                                        h_flex()
                                            .w_full()
                                            .flex_wrap()
                                            .items_center()
                                            .justify_between()
                                            .gap(Spacing::SM)
                                            .p(Spacing::MD)
                                            .rounded(Radii::MD)
                                            .bg(palette.surface_subtle)
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
                                                                Label::new(record.pensioner_name)
                                                                    .text_size(Typography::SM)
                                                                    .font_semibold()
                                                                    .text_color(palette.foreground),
                                                            )
                                                            .child(
                                                                Label::new(record.pensioner_code)
                                                                    .text_size(Typography::XS)
                                                                    .font_medium()
                                                                    .text_color(
                                                                        palette.brand_accent,
                                                                    ),
                                                            )
                                                            .child(StatusBadge::new(
                                                                StatusTone::Info,
                                                                kind_label,
                                                            )),
                                                    )
                                                    .child(
                                                        Label::new(format!(
                                                            "{} · Desayunos {} · Almuerzos {} ({}) · Tickets {} ({})",
                                                            record.recorded_at,
                                                            record.breakfasts,
                                                            record.lunches,
                                                            format_soles(record.consumption_cents),
                                                            record.tickets_count,
                                                            format_soles(
                                                                record.ticket_amount_cents
                                                            ),
                                                        ))
                                                        .text_size(Typography::XS)
                                                        .text_color(palette.muted_foreground),
                                                    ),
                                            )
                                            .child(
                                                Label::new(format!(
                                                    "Saldo: {}",
                                                    format_soles(record.balance_after_cents)
                                                ))
                                                .text_size(Typography::XS)
                                                .font_semibold()
                                                .text_color(if record.balance_after_cents < 0 {
                                                    palette.error.foreground
                                                } else {
                                                    palette.success.foreground
                                                }),
                                            )
                                    }))
                                    .into_any_element()
                            }),
                    ),
            )
    }
}
