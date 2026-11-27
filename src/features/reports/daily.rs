#[cfg(test)]
use std::collections::BTreeMap;

#[cfg(test)]
use chrono::NaiveDate;
use gpui_kit::{
    component::{
        StyledExt as _,
        chart::{AreaChart, BarChart},
        h_flex,
        label::Label,
        v_flex,
    },
    prelude::FluentBuilder as _,
    *,
};

use crate::{
    components::loading,
    services::api::dining::DailyActivity,
    state::store::CalendarDate,
    theme::{ActiveDylanTheme as _, MetricAccent, Radii, Spacing, Typography},
};

#[cfg(test)]
use crate::{
    features::consumptions::order::{Meal, Menu},
    state::store::DemoLedger,
};

#[cfg(test)]
use super::from_naive_date;
use super::to_naive_date;

const DAYS_PER_PAGE: i64 = 14;

pub(super) struct DailyPage {
    pub from: CalendarDate,
    pub to: CalendarDate,
    pub offset: i64,
    pub has_older_dates: bool,
}

impl DailyPage {
    pub fn new(from: CalendarDate, to: CalendarDate, offset: i64) -> Self {
        let days = to_naive_date(to)
            .signed_duration_since(to_naive_date(from))
            .num_days();
        let offset = offset.clamp(0, days.max(0) / DAYS_PER_PAGE);
        let to = to.sub_days(offset * DAYS_PER_PAGE);
        let start = to.sub_days(DAYS_PER_PAGE - 1).max(from);
        Self {
            from: start,
            to,
            offset,
            has_older_dates: start > from,
        }
    }
}

#[cfg(test)]
impl DailyActivity {
    fn empty(date: NaiveDate) -> Self {
        Self {
            date,
            breakfasts: 0,
            economic_lunches: 0,
            special_lunches: 0,
            unclassified_lunches: 0,
            attendances: 0,
            third_party_pickups: 0,
            tickets_count: 0,
        }
    }
}

#[cfg(test)]
fn daily_activity(page: &DailyPage, ledger: &DemoLedger) -> Vec<DailyActivity> {
    let to = to_naive_date(page.to);
    let mut days: BTreeMap<_, _> = to_naive_date(page.from)
        .iter_days()
        .take_while(|date| *date <= to)
        .map(|date| (from_naive_date(date), DailyActivity::empty(date)))
        .collect();

    for record in ledger.attendances() {
        let Some(day) = days.get_mut(&record.date) else {
            continue;
        };
        day.attendances += 1;
        day.third_party_pickups += i64::from(record.picked_up_by.is_some());
        day.breakfasts += i64::from(record.breakfasts);
        let mut classified_lunches = 0;
        for item in record.items.iter().filter(|item| item.meal == Meal::Lunch) {
            let portions = i64::from(item.portions);
            classified_lunches += portions;
            match item.menu {
                Menu::Economic => day.economic_lunches += portions,
                Menu::Special { .. } => day.special_lunches += portions,
            }
        }
        day.unclassified_lunches += (i64::from(record.lunches) - classified_lunches).max(0);
    }
    for movement in ledger.ticket_movements() {
        if let Some(day) = days.get_mut(&movement.date) {
            day.tickets_count += i64::from(movement.tickets_count);
        }
    }

    days.into_values().collect()
}

struct Series {
    label: &'static str,
    axis_label: &'static str,
    color: Hsla,
    value: fn(&DailyActivity) -> i64,
}

pub(super) fn loading_charts(compact: bool, cx: &App) -> impl IntoElement {
    let palette = cx.dylan_theme().palette;
    h_flex().w_full().flex_wrap().gap(Spacing::MD).children(
        ["daily-meals-loading", "daily-activity-loading"].map(|id| {
            let panel = loading::region(id, "Cargando gráfica de actividad diaria")
                .p(Spacing::MD)
                .rounded(Radii::LG)
                .bg(palette.surface)
                .border_1()
                .border_color(palette.border)
                .child(loading::line("title", px(150.)))
                .child(
                    h_flex().h(px(180.)).items_end().gap(Spacing::SM).children(
                        [58., 102., 72., 143., 96., 125., 81.]
                            .into_iter()
                            .enumerate()
                            .map(|(index, height)| {
                                div()
                                    .id(("bar", index))
                                    .flex_1()
                                    .min_w_0()
                                    .child(loading::block("height", px(height)))
                            }),
                    ),
                )
                .child(loading::line("legend", px(120.)));
            if compact {
                panel.w_full()
            } else {
                panel.flex_1().min_w(px(300.)).max_w_full()
            }
        }),
    )
}

impl Series {
    fn new(
        label: &'static str,
        axis_label: &'static str,
        color: Hsla,
        value: fn(&DailyActivity) -> i64,
    ) -> Self {
        Self {
            label,
            axis_label,
            color,
            value,
        }
    }
}

pub(super) fn charts(days: &[DailyActivity], compact: bool, cx: &App) -> Div {
    let theme = cx.dylan_theme();
    let palette = theme.palette;
    let breakfasts = theme.metric(MetricAccent::Breakfasts).icon;
    let lunches = theme.metric(MetricAccent::Lunches).icon;
    let tickets = theme.metric(MetricAccent::TicketsReceived).icon;
    let mut meals = vec![
        Series::new("Almuerzos económicos", "Económicos", lunches, |day| {
            day.economic_lunches
        }),
        Series::new(
            "Almuerzos especiales",
            "Especiales",
            palette.brand_accent,
            |day| day.special_lunches,
        ),
        Series::new("Desayunos", "Desayunos", breakfasts, |day| day.breakfasts),
    ];
    if days.iter().any(|day| day.unclassified_lunches > 0) {
        meals.push(Series::new(
            "Almuerzos sin clasificar",
            "Sin tipo",
            palette.muted_foreground,
            |day| day.unclassified_lunches,
        ));
    }

    let panels = [
        chart_panel("daily-meals", "Raciones servidas", days, &meals, cx),
        chart_panel(
            "daily-activity",
            "Atenciones y tickets",
            days,
            &[
                Series::new(
                    "Atenciones registradas",
                    "Atenciones",
                    palette.brand_accent,
                    |day| day.attendances,
                ),
                Series::new(
                    "Retiros por terceros",
                    "Retiros",
                    theme.metric(MetricAccent::CreditBalance).icon,
                    |day| day.third_party_pickups,
                ),
                Series::new("Tickets recibidos", "Tickets", tickets, |day| {
                    day.tickets_count
                }),
            ],
            cx,
        ),
    ];

    h_flex()
        .w_full()
        .flex_wrap()
        .gap(Spacing::MD)
        .children(panels.map(|panel| {
            if compact {
                panel.w_full()
            } else {
                panel.flex_1().min_w(px(300.)).max_w_full()
            }
        }))
}

fn chart_panel(
    id: &'static str,
    title: &'static str,
    days: &[DailyActivity],
    series: &[Series],
    cx: &App,
) -> Div {
    let palette = cx.dylan_theme().palette;
    let any_values = days
        .iter()
        .any(|day| series.iter().any(|series| (series.value)(day) != 0));

    let plot = if !any_values {
        Label::new("Sin registros para este indicador en las fechas mostradas.")
            .text_size(Typography::SM)
            .text_color(palette.muted_foreground)
            .into_any_element()
    } else if let [day] = days {
        let values = series.iter().map(|series| {
            (
                series.label,
                series.axis_label,
                series.color,
                (series.value)(day),
            )
        });
        BarChart::new(values)
            .id(id)
            .band(|row| row.1)
            .value(|row| row.3 as f64)
            .fill(|row, _, _, _| row.2)
            .tooltip_title(|row| row.0.into())
            .tooltip_value(|row, _| row.3.to_string().into())
            .value_axis(true)
            .value_tick_format(axis_tick)
            .into_any_element()
    } else {
        let mut chart = AreaChart::new(days.iter().cloned())
            .id(id)
            .x(|day| day.date.format("%d/%m").to_string())
            .tooltip_title(|day| day.date.format("%d/%m/%Y").to_string().into())
            .tooltip_value(|_, _, value| format!("{value:.0}").into())
            .y_axis(true)
            .y_tick_format(axis_tick)
            .x_tick_count(days.len().min(5));
        for series in series {
            let value = series.value;
            chart = chart
                .y(move |day| value(day) as f64)
                .linear()
                .name(series.label)
                .stroke(series.color)
                .fill(series.color.opacity(0.08));
        }
        chart.into_any_element()
    };

    v_flex()
        .gap(Spacing::MD)
        .p(Spacing::MD)
        .rounded(Radii::LG)
        .bg(palette.surface)
        .border_1()
        .border_color(palette.border)
        .child(Label::new(title).text_size(Typography::LG).font_semibold())
        .child(
            h_flex()
                .flex_wrap()
                .gap(Spacing::SM)
                .children(series.iter().map(|series| {
                    let total: i64 = days.iter().map(series.value).sum();
                    let amount = total.to_string();
                    h_flex()
                        .items_center()
                        .gap(Spacing::XS)
                        .px(Spacing::SM)
                        .py(Spacing::XS)
                        .rounded(Radii::MD)
                        .bg(palette.surface_subtle)
                        .child(
                            div()
                                .size(px(8.))
                                .rounded(Radii::FULL)
                                .bg(series.color)
                                .flex_shrink_0(),
                        )
                        .child(
                            Label::new(series.label)
                                .text_size(Typography::XS)
                                .text_color(palette.muted_foreground),
                        )
                        .child(Label::new(amount).text_size(Typography::SM).font_semibold())
                })),
        )
        .child(
            div()
                .w_full()
                .when(any_values, |plot| plot.h(px(180.)))
                .child(plot),
        )
}

fn axis_tick(value: f64) -> String {
    if value.fract().abs() < 0.001 {
        format!("{value:.0}")
    } else {
        format!("{value:.2}")
    }
}

#[cfg(test)]
mod tests {
    use super::{DailyPage, daily_activity};
    use crate::{
        features::{
            consumptions::{
                confirm_attendance::{AttendanceRequest, confirm_attendance},
                order::{ConsumptionItem, Meal, Menu},
            },
            pensioners::{SelectionSource, model::PensionerId},
        },
        state::store::{CalendarDate, DatePeriodFilter, DemoLedger},
    };

    fn november(day: u8) -> CalendarDate {
        CalendarDate::new(2026, 11, day).unwrap()
    }

    #[test]
    fn activity_separates_rations_and_tickets_and_keeps_days_without_movement() {
        let mut ledger = DemoLedger::new();
        let pensioner_id = PensionerId(1001);
        ledger.set_test_date(november(10));
        ledger.open_cash("Caja", 5000).unwrap();
        confirm_attendance(
            &mut ledger,
            AttendanceRequest {
                idempotency_key: "daily-meals".into(),
                pensioner_id,
                items: [
                    (Meal::Breakfast, Menu::Economic, 1),
                    (Meal::Lunch, Menu::Economic, 2),
                    (Meal::Lunch, Menu::Special { unit_cents: 1500 }, 3),
                ]
                .map(|(meal, menu, portions)| ConsumptionItem {
                    meal,
                    menu,
                    portions,
                })
                .to_vec(),
                picked_up_by: Some("María Ramos".into()),
                tickets_count: 2,
                source: SelectionSource::GlobalSearch,
                operator_name: "Caja".into(),
                can_register_attendance: true,
            },
        )
        .unwrap();
        let report = ledger.report_for_period(DatePeriodFilter::Custom {
            from: november(10),
            to: november(12),
        });
        let page = DailyPage::new(november(10), november(12), 0);
        let days = daily_activity(&page, &ledger);

        assert_eq!(days.len(), 3);
        let first = &days[0];
        assert_eq!(
            (
                first.breakfasts,
                first.economic_lunches,
                first.special_lunches
            ),
            (1, 2, 3)
        );
        assert_eq!(
            (
                first.attendances,
                first.third_party_pickups,
                first.tickets_count
            ),
            (1, 1, 2)
        );
        assert_eq!(first.tickets_count, i64::from(report.tickets_received));
        for day in &days[1..] {
            assert_eq!((day.attendances, day.tickets_count), (0, 0));
        }
    }

    #[test]
    fn chart_pages_cover_the_range_without_overlapping() {
        let from = november(1);
        let to = november(30);
        let latest = DailyPage::new(from, to, 0);
        let middle = DailyPage::new(from, to, 1);
        let earliest = DailyPage::new(from, to, 9);

        assert_eq!((latest.from, latest.to), (november(17), to));
        assert_eq!((middle.from, middle.to), (november(3), november(16)));
        assert_eq!((earliest.from, earliest.to), (from, november(2)));
        assert!(!earliest.has_older_dates);
    }
}
