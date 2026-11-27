mod daily;

use chrono::{Datelike as _, NaiveDate, Weekday};
use gpui_kit::{
    component::{
        Icon, IconName, Sizable as _, StyledExt as _,
        button::Button,
        calendar::{Calendar, CalendarEvent, CalendarState, Date, Matcher},
        h_flex,
        label::Label,
        popover::Popover,
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
    state::store::{CalendarDate, ComedorStore, DataLoad, DatePeriodFilter},
    theme::{ActiveDylanTheme as _, Radii, Spacing, Typography},
};

fn to_naive_date(date: CalendarDate) -> NaiveDate {
    NaiveDate::from_ymd_opt(date.year, u32::from(date.month), u32::from(date.day))
        .unwrap_or_else(|| NaiveDate::from_ymd_opt(2026, 1, 1).expect("valid fallback date"))
}

fn from_naive_date(date: NaiveDate) -> CalendarDate {
    CalendarDate {
        year: date.year(),
        month: date.month() as u8,
        day: date.day() as u8,
    }
}

fn normalize_range(from: CalendarDate, to: CalendarDate) -> (CalendarDate, CalendarDate) {
    if from > to { (from, from) } else { (from, to) }
}

/// Actividad diaria para el rango de fechas elegido.
pub struct ReportsView {
    store: Entity<ComedorStore>,
    period: DatePeriodFilter,
    from_date: CalendarDate,
    to_date: CalendarDate,
    from_calendar: Entity<CalendarState>,
    to_calendar: Entity<CalendarState>,
    from_open: bool,
    to_open: bool,
    daily_offset: i64,
    _subscriptions: [Subscription; 3],
}

impl ReportsView {
    pub fn new(store: Entity<ComedorStore>, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let today = store.read(cx).ledger().current_date();
        let today_naive = to_naive_date(today);

        let from_calendar = cx.new(|cx| {
            let mut calendar = CalendarState::new(window, cx);
            calendar.set_date(Date::Single(Some(today_naive)), window, cx);
            calendar
        });
        let to_calendar = cx.new(|cx| {
            let mut calendar = CalendarState::new(window, cx)
                .disabled_matcher(Matcher::custom(move |d| *d < today_naive));
            calendar.set_date(Date::Single(Some(today_naive)), window, cx);
            calendar
        });

        let observe_store = cx.observe(&store, |_, _, cx| cx.notify());
        let sub_from = cx.subscribe_in(
            &from_calendar,
            window,
            |view, _, event: &CalendarEvent, window, cx| {
                let CalendarEvent::Selected(date) = event;
                if let Some(selected) = date.start() {
                    view.on_from_selected(selected, window, cx);
                }
            },
        );
        let sub_to = cx.subscribe(&to_calendar, |view, _, event: &CalendarEvent, cx| {
            let CalendarEvent::Selected(date) = event;
            if let Some(selected) = date.start() {
                view.on_to_selected(selected, cx);
            }
        });

        Self {
            store,
            period: DatePeriodFilter::Today,
            from_date: today,
            to_date: today,
            from_calendar,
            to_calendar,
            from_open: false,
            to_open: false,
            daily_offset: 0,
            _subscriptions: [observe_store, sub_from, sub_to],
        }
    }

    fn set_popover_open(&mut self, is_from: bool, open: bool, cx: &mut Context<Self>) {
        if is_from {
            self.from_open = open;
            if open {
                self.to_open = false;
            }
        } else {
            self.to_open = open;
            if open {
                self.from_open = false;
            }
        }
        cx.notify();
    }

    fn on_from_selected(&mut self, naive: NaiveDate, window: &mut Window, cx: &mut Context<Self>) {
        let (from, to) = normalize_range(from_naive_date(naive), self.to_date);
        self.from_date = from;
        self.to_date = to;
        self.from_open = false;

        let min_to = to_naive_date(self.from_date);
        let current_to = to_naive_date(self.to_date);
        self.to_calendar.update(cx, |calendar, cx| {
            calendar.set_disabled_matcher(Matcher::custom(move |d| *d < min_to), window, cx);
            calendar.set_date(Date::Single(Some(current_to)), window, cx);
        });
        self.sync_period(cx);
    }

    fn on_to_selected(&mut self, naive: NaiveDate, cx: &mut Context<Self>) {
        let picked = from_naive_date(naive);
        self.to_date = picked.max(self.from_date);
        self.to_open = false;
        self.sync_period(cx);
    }

    fn sync_period(&mut self, cx: &mut Context<Self>) {
        self.daily_offset = 0;
        let today = self.store.read(cx).ledger().current_date();
        self.period = if self.from_date == today && self.to_date == today {
            DatePeriodFilter::Today
        } else {
            DatePeriodFilter::Custom {
                from: self.from_date,
                to: self.to_date,
            }
        };
        self.load_daily(false, cx);
    }

    fn daily_page(&self) -> daily::DailyPage {
        let (from, to) = if self.period == DatePeriodFilter::Today {
            let today = CalendarDate::today();
            (today, today)
        } else {
            (self.from_date, self.to_date)
        };
        daily::DailyPage::new(from, to, self.daily_offset)
    }

    pub fn load_daily(&self, force: bool, cx: &mut Context<Self>) {
        let page = self.daily_page();
        self.store.update(cx, |store, cx| {
            store.load_daily_activity((page.from, page.to), force, cx);
        });
        cx.notify();
    }

    fn select_recent_days(&mut self, days: i64, window: &mut Window, cx: &mut Context<Self>) {
        let today = self.store.read(cx).ledger().current_date();
        let today_naive = to_naive_date(today);
        self.from_date = today.sub_days(days - 1);
        self.to_date = today;
        self.from_open = false;
        self.to_open = false;

        let from_naive = to_naive_date(self.from_date);
        self.from_calendar.update(cx, |calendar, cx| {
            calendar.set_date(Date::Single(Some(from_naive)), window, cx);
        });
        self.to_calendar.update(cx, |calendar, cx| {
            calendar.set_disabled_matcher(Matcher::custom(move |d| *d < from_naive), window, cx);
            calendar.set_date(Date::Single(Some(today_naive)), window, cx);
        });
        self.sync_period(cx);
    }

    fn render_date_field(
        &self,
        label: &'static str,
        popover_id: &'static str,
        button_id: &'static str,
        date: CalendarDate,
        calendar: &Entity<CalendarState>,
        is_open: bool,
        is_from: bool,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let palette = cx.dylan_theme().palette;
        let trigger = Button::new(button_id)
            .outline()
            .small()
            .w(px(172.0))
            .rounded(Radii::MD)
            .bg(palette.surface)
            .border_color(if is_open {
                palette.focus_ring
            } else {
                palette.input_border
            })
            .child(
                h_flex()
                    .w_full()
                    .items_center()
                    .justify_between()
                    .gap(Spacing::XS)
                    .child(
                        Label::new(date.format_iso())
                            .text_size(Typography::SM)
                            .text_color(palette.foreground),
                    )
                    .child(
                        Icon::new(IconName::Calendar)
                            .xsmall()
                            .text_color(palette.muted_foreground),
                    ),
            );

        v_flex()
            .gap(Spacing::XXS)
            .child(
                Label::new(label)
                    .text_size(Typography::XS)
                    .font_semibold()
                    .text_color(palette.muted_foreground),
            )
            .child(
                Popover::new(popover_id)
                    .open(is_open)
                    .on_open_change(cx.listener(move |view, open: &bool, _, cx| {
                        view.set_popover_open(is_from, *open, cx);
                    }))
                    .trigger(trigger)
                    .p(Spacing::XS)
                    .child(
                        Calendar::new(calendar)
                            .small()
                            .first_day_of_week(Weekday::Mon)
                            .border_0()
                            .p_0(),
                    ),
            )
    }
}

impl Render for ReportsView {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let palette = cx.dylan_theme().palette;
        let compact = window.viewport_size().width < px(640.0);
        let outer_padding = if compact { Spacing::MD } else { Spacing::XL };
        let card_padding = if compact { Spacing::MD } else { Spacing::LG };

        let daily_page = self.daily_page();
        let query = self
            .store
            .read(cx)
            .daily_activity((daily_page.from, daily_page.to));
        let is_loading = query.is_some_and(|query| query.status() == DataLoad::Loading);
        let report_error = query.and_then(|query| query.error.clone());
        let daily_charts = query
            .and_then(|query| query.value.as_ref())
            .map(|days| daily::charts(days, compact, cx));
        let period_label = self.period.label();

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
                                        Label::new("Reportes")
                                            .text_size(if compact {
                                                Typography::XL
                                            } else {
                                                Typography::PAGE_TITLE
                                            })
                                            .font_bold()
                                            .text_color(palette.foreground),
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
                                    .items_end()
                                    .gap(Spacing::SM)
                                    .child(self.render_date_field(
                                        "Desde",
                                        "report-from-popover",
                                        "report-from-trigger",
                                        self.from_date,
                                        &self.from_calendar,
                                        self.from_open,
                                        true,
                                        cx,
                                    ))
                                    .child(self.render_date_field(
                                        "Hasta",
                                        "report-to-popover",
                                        "report-to-trigger",
                                        self.to_date,
                                        &self.to_calendar,
                                        self.to_open,
                                        false,
                                        cx,
                                    ))
                                    .child(
                                        AppButton::new("report-last-seven-days")
                                            .secondary()
                                            .small()
                                            .label("Últimos 7 días")
                                            .on_click(cx.listener(|view, _, window, cx| {
                                                view.select_recent_days(7, window, cx);
                                            })),
                                    )
                                    .when(self.period != DatePeriodFilter::Today, |this| {
                                        this.child(
                                            AppButton::new("report-reset-period")
                                                .tertiary()
                                                .small()
                                                .label("Hoy")
                                                .on_click(cx.listener(|view, _, window, cx| {
                                                    view.select_recent_days(1, window, cx);
                                                })),
                                        )
                                    }),
                            ),
                    )
                    .child(
                        h_flex()
                            .flex_wrap()
                            .items_center()
                            .justify_between()
                            .gap(Spacing::SM)
                            .child(
                                Label::new("Actividad por día")
                                    .text_size(Typography::XL)
                                    .font_semibold(),
                            )
                            .child(
                                h_flex()
                                    .items_center()
                                    .gap(Spacing::XS)
                                    .when(
                                        daily_page.has_older_dates || daily_page.offset > 0,
                                        |row| {
                                            let offset = daily_page.offset;
                                            row.child(
                                                AppButton::new("reports-older-days")
                                                    .secondary()
                                                    .xsmall()
                                                    .icon(IconName::ChevronLeft)
                                                    .tooltip("Fechas anteriores")
                                                    .disabled(!daily_page.has_older_dates)
                                                    .on_click(cx.listener(
                                                        move |view, _, _, cx| {
                                                            view.daily_offset = offset + 1;
                                                            view.load_daily(false, cx);
                                                        },
                                                    )),
                                            )
                                        },
                                    )
                                    .child(
                                        Label::new(if daily_page.from == daily_page.to {
                                            to_naive_date(daily_page.to)
                                                .format("%d/%m/%Y")
                                                .to_string()
                                        } else {
                                            format!(
                                                "{} – {}",
                                                to_naive_date(daily_page.from).format("%d/%m/%Y"),
                                                to_naive_date(daily_page.to).format("%d/%m/%Y")
                                            )
                                        })
                                        .text_size(Typography::XS)
                                        .text_color(palette.muted_foreground),
                                    )
                                    .when(
                                        daily_page.has_older_dates || daily_page.offset > 0,
                                        |row| {
                                            let offset = daily_page.offset;
                                            row.child(
                                                AppButton::new("reports-newer-days")
                                                    .secondary()
                                                    .xsmall()
                                                    .icon(IconName::ChevronRight)
                                                    .tooltip("Fechas posteriores")
                                                    .disabled(offset == 0)
                                                    .on_click(cx.listener(
                                                        move |view, _, _, cx| {
                                                            view.daily_offset = offset - 1;
                                                            view.load_daily(false, cx);
                                                        },
                                                    )),
                                            )
                                        },
                                    ),
                            ),
                    )
                    .when_some(daily_charts, |view, charts| view.child(charts))
                    .when_some(report_error, |view, error| {
                        view.child(
                            h_flex()
                                .gap(Spacing::SM)
                                .items_center()
                                .child(Label::new(error))
                                .child(
                                    AppButton::new("reload-daily")
                                        .secondary()
                                        .small()
                                        .label("Reintentar")
                                        .on_click(
                                            cx.listener(|view, _, _, cx| view.load_daily(true, cx)),
                                        ),
                                ),
                        )
                    })
                    .when(is_loading, |view| {
                        view.child(daily::loading_charts(compact, cx))
                    }),
            )
    }
}

#[cfg(test)]
mod tests {
    use super::{CalendarDate, from_naive_date, normalize_range, to_naive_date};

    #[test]
    fn calendar_date_converts_cleanly_and_normalizes_inverted_ranges() {
        let d1 = CalendarDate::new(2026, 11, 5).unwrap();
        let d2 = CalendarDate::new(2026, 11, 24).unwrap();
        assert_eq!(from_naive_date(to_naive_date(d1)), d1);
        assert_eq!(from_naive_date(to_naive_date(d2)), d2);
        assert_eq!(normalize_range(d1, d2), (d1, d2));
        assert_eq!(normalize_range(d2, d1), (d2, d2));
    }
}
