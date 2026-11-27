use gpui_kit::{
    component::{IconName, StyledExt as _, h_flex, label::Label, v_flex},
    prelude::FluentBuilder as _,
    *,
};

use crate::{
    components::{
        button::AppButton,
        loading,
        metric_card::MetricCard,
        status_badge::{StatusBadge, StatusTone},
    },
    features::pensioners::{
        SelectedPensioner,
        model::{find_by_id, format_soles},
    },
    state::{
        reader::{ReaderState, ReaderStatus},
        session::{AppRoute, SessionState},
        store::{CalendarDate, ComedorStore, DataLoad, DatePeriodFilter},
    },
    theme::{ActiveDylanTheme as _, MetricAccent, Radii, Spacing, Typography},
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DashboardEvent {
    OpenPensioners,
    OpenIdentification,
    OpenConsumptions,
    OpenCash,
    OpenReports,
    OpenSettings,
    RefreshSession,
}

/// Panel principal: protegido por `SessionState`, muestra métricas vivas desde `ComedorStore`
/// filtradas por periodo usando la misma fuente y consolidado que Reportes, Consumos y Caja.
pub struct DashboardView {
    session: Entity<SessionState>,
    reader: Entity<ReaderState>,
    store: Entity<ComedorStore>,
    period: DatePeriodFilter,
    selected: Option<SelectedPensioner>,
    syncing_session: bool,
    session_notice: Option<(StatusTone, SharedString)>,
    _subscriptions: [Subscription; 3],
}

impl EventEmitter<DashboardEvent> for DashboardView {}

impl DashboardView {
    pub fn new(
        session: Entity<SessionState>,
        reader: Entity<ReaderState>,
        store: Entity<ComedorStore>,
        cx: &mut Context<Self>,
    ) -> Self {
        let observe_session = cx.observe(&session, |_, _, cx| cx.notify());
        let observe_reader = cx.observe(&reader, |_, _, cx| cx.notify());
        let observe_store = cx.observe(&store, |_, _, cx| cx.notify());
        Self {
            session,
            reader,
            store,
            period: DatePeriodFilter::Today,
            selected: None,
            syncing_session: false,
            session_notice: None,
            _subscriptions: [observe_session, observe_reader, observe_store],
        }
    }

    pub fn set_selected(&mut self, selected: Option<SelectedPensioner>, cx: &mut Context<Self>) {
        if self.selected != selected {
            self.selected = selected;
            cx.notify();
        }
    }

    fn date_range(&self) -> (CalendarDate, CalendarDate) {
        let today = CalendarDate::today();
        match self.period {
            DatePeriodFilter::Today => (today, today),
            DatePeriodFilter::Last7Days => (today.sub_days(6), today),
            DatePeriodFilter::ThisMonth => (CalendarDate { day: 1, ..today }, today),
            DatePeriodFilter::All => (
                CalendarDate {
                    year: 1,
                    month: 1,
                    day: 1,
                },
                today,
            ),
            DatePeriodFilter::Custom { from, to } => (from, to),
        }
    }

    pub fn load_metrics(&self, force: bool, cx: &mut Context<Self>) {
        let range = self.date_range();
        self.store.update(cx, |store, cx| {
            store.load_dashboard(range, force, cx);
        });
        cx.notify();
    }

    pub fn set_session_sync(
        &mut self,
        syncing: bool,
        notice: Option<(StatusTone, SharedString)>,
        cx: &mut Context<Self>,
    ) {
        self.syncing_session = syncing;
        self.session_notice = notice;
        cx.notify();
    }
}

impl Render for DashboardView {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let session = self.session.read(cx);
        if !session.can_access_dashboard() {
            return div().hidden().into_any_element();
        }
        let can_open_id = session.can_access_route(AppRoute::Identification);
        let can_open_pensioners = session.can_access_route(AppRoute::Pensioners);
        let can_open_consumptions = session.can_access_route(AppRoute::Consumptions);
        let can_open_cash = session.can_access_route(AppRoute::Cash);
        let can_open_reports = session.can_access_route(AppRoute::Reports);
        let can_open_settings = session.can_access_route(AppRoute::Settings);

        let palette = cx.dylan_theme().palette;
        let compact = window.viewport_size().width < px(640.0);
        let outer_padding = if compact { Spacing::MD } else { Spacing::XL };
        let card_padding = if compact { Spacing::MD } else { Spacing::LG };

        let store = self.store.read(cx);
        let ledger = store.ledger();
        let catalog = ledger.pensioners();
        let period_label = self.period.label();
        let metrics = store.dashboard(self.date_range());
        let metrics_loading = metrics.is_some_and(|query| query.status() == DataLoad::Loading);
        let error = metrics.and_then(|query| query.error.clone());
        let data = metrics.and_then(|query| query.value.as_ref());
        let consumptions = data.and_then(|data| data.consumptions);
        let tickets = data.and_then(|data| data.tickets);
        let pensioners = data.and_then(|data| data.pensioners);
        let cash = data.and_then(|data| data.cash.as_ref());

        let reader = self.reader.read(cx);
        let reader_loading = reader.connection_loading();
        let reader_status = reader.status().clone();
        let reader_tone = match reader_status {
            ReaderStatus::Disconnected { .. } => StatusTone::Error,
            ReaderStatus::Ready | ReaderStatus::Matched { .. } | ReaderStatus::Enrolled { .. } => {
                StatusTone::Success
            }
            ReaderStatus::Capturing { .. } => StatusTone::Info,
            ReaderStatus::Unrecognized | ReaderStatus::EnrollmentMismatch => StatusTone::Warning,
            ReaderStatus::Cancelled => StatusTone::Neutral,
        };

        let selected_summary = self.selected.and_then(|sel| {
            find_by_id(catalog, sel.id).map(|p| {
                format!(
                    "{} ({}) · vía {} · Saldo {}",
                    p.full_name,
                    p.code(),
                    sel.source.label(),
                    format_soles(p.balance_cents),
                )
            })
        });

        v_flex()
            .id("dashboard-scroll")
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
                                    .gap(Spacing::MD)
                                    .child(
                                        v_flex()
                                            .min_w_0()
                                            .gap(Spacing::XXS)
                                            .child(
                                                Label::new("Dashboard")
                                                    .text_size(if compact {
                                                        Typography::XL
                                                    } else {
                                                        Typography::PAGE_TITLE
                                                    })
                                                    .font_bold()
                                                    .text_color(palette.foreground),
                                            )
                                            .when_some(
                                                self.session_notice.clone(),
                                                |this, (tone, msg)| {
                                                    this.child(
                                                        div()
                                                            .pt(Spacing::XXS)
                                                            .child(StatusBadge::new(tone, msg)),
                                                    )
                                                },
                                            ),
                                    )
                                    .child(
                                        h_flex()
                                            .flex_wrap()
                                            .items_center()
                                            .gap(Spacing::SM)
                                            .when_some(cash, |row, cash| row.child(if cash.status == "open" {
                                                StatusBadge::new(StatusTone::Success, "Caja abierta")
                                            } else { StatusBadge::new(StatusTone::Warning, "Caja cerrada") }))
                                            .child(
                                                AppButton::new("dashboard-refresh-session")
                                                    .tertiary()
                                                    .small()
                                                    .disabled(self.syncing_session)
                                                    .loading(self.syncing_session)
                                                    .label(if self.syncing_session {
                                                        "Verificando…"
                                                    } else {
                                                        "Verificar permisos"
                                                    })
                                                    .on_click(cx.listener(|_, _, _, cx| {
                                                        cx.emit(DashboardEvent::RefreshSession);
                                                    })),
                                            )
                                            .when(can_open_id, |this| {
                                                this.child(
                                                    AppButton::new("dashboard-open-id")
                                                        .primary()
                                                        .small()
                                                        .icon(IconName::User)
                                                        .label("Identificar")
                                                        .on_click(cx.listener(|_, _, _, cx| {
                                                            cx.emit(
                                                                DashboardEvent::OpenIdentification,
                                                            );
                                                        })),
                                                )
                                            })
                                            .when(can_open_consumptions, |this| {
                                                this.child(
                                                    AppButton::new("dashboard-open-consumptions")
                                                        .secondary()
                                                        .small()
                                                        .icon(IconName::FileText)
                                                        .label("Consumos")
                                                        .on_click(cx.listener(|_, _, _, cx| {
                                                            cx.emit(
                                                                DashboardEvent::OpenConsumptions,
                                                            );
                                                        })),
                                                )
                                            })
                                            .when(can_open_cash, |this| {
                                                this.child(
                                                    AppButton::new("dashboard-open-cash")
                                                        .secondary()
                                                        .small()
                                                        .icon(IconName::Inbox)
                                                        .label("Caja")
                                                        .on_click(cx.listener(|_, _, _, cx| {
                                                            cx.emit(DashboardEvent::OpenCash);
                                                        })),
                                                )
                                            })
                                            .when(can_open_reports, |this| {
                                                this.child(
                                                    AppButton::new("dashboard-open-reports")
                                                        .secondary()
                                                        .small()
                                                        .icon(IconName::ChartPie)
                                                        .label("Reportes")
                                                        .on_click(cx.listener(|_, _, _, cx| {
                                                            cx.emit(DashboardEvent::OpenReports);
                                                        })),
                                                )
                                            })
                                            .when(can_open_pensioners, |this| {
                                                this.child(
                                                    AppButton::new("dashboard-open-pensioners")
                                                        .tertiary()
                                                        .small()
                                                        .icon(IconName::Search)
                                                        .label("Pensionistas")
                                                        .on_click(cx.listener(|_, _, _, cx| {
                                                            cx.emit(DashboardEvent::OpenPensioners);
                                                        })),
                                                )
                                            })
                                            .when(can_open_settings, |this| {
                                                this.child(
                                                    AppButton::new("dashboard-open-settings")
                                                        .tertiary()
                                                        .small()
                                                        .icon(IconName::Settings)
                                                        .tooltip("Configuración")
                                                        .on_click(cx.listener(|_, _, _, cx| {
                                                            cx.emit(DashboardEvent::OpenSettings);
                                                        })),
                                                )
                                            }),
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
                                        h_flex()
                                            .flex_wrap()
                                            .items_center()
                                            .gap(Spacing::XS)
                                            .children(DatePeriodFilter::PRESETS.into_iter().map(
                                                |preset| {
                                                    let active = self.period == preset;
                                                    let btn = AppButton::new(SharedString::from(
                                                        format!("dashboard-period-{preset:?}"),
                                                    ))
                                                    .xsmall()
                                                    .label(preset.label())
                                                    .on_click(cx.listener(move |view, _, _, cx| {
                                                        view.period = preset;
                                                        view.load_metrics(false, cx);
                                                    }));
                                                    if active {
                                                        btn.primary()
                                                    } else {
                                                        btn.secondary()
                                                    }
                                                },
                                            )),
                                    )
                                    .when_some(consumptions, |row, report| row.child(
                                        Label::new(format!(
                                            "{} atenciones · {} raciones en {period_label}",
                                            report.attendances, report.rations
                                        ))
                                        .text_size(Typography::XS)
                                        .text_color(palette.muted_foreground),
                                    )),
                            ),
                    )
                    .when_some(error, |view, error| view.child(
                        h_flex().gap(Spacing::SM).items_center()
                            .child(Label::new(error))
                            .child(AppButton::new("reload-dashboard").secondary().small().label("Reintentar")
                                .on_click(cx.listener(|view, _, _, cx| view.load_metrics(true, cx)))),
                    ))
                    .when(metrics_loading, |view| {
                        view.child(loading::metrics(
                            "dashboard-metrics-loading",
                            "Cargando métricas del panel",
                            4,
                            cx,
                        ))
                    })
                    .child(
                        h_flex().w_full().flex_wrap().gap(Spacing::MD)
                            .when_some(consumptions, |row, stats| row
                                .child(MetricCard::new(MetricAccent::Breakfasts, "Desayunos", stats.breakfasts.to_string())
                                    .icon(IconName::Sun).subtitle(period_label.clone()))
                                .child(MetricCard::new(MetricAccent::Lunches, "Almuerzos", stats.lunches.to_string())
                                    .icon(IconName::Calendar).subtitle(period_label.clone())))
                            .when_some(cash, |row, cash| row.child(
                                MetricCard::new(MetricAccent::CashIn, "Efectivo en caja",
                                    cash.expected_cash_cents.map_or_else(|| "Caja cerrada".into(), format_soles))
                                    .icon(IconName::CircleCheck).subtitle("Apertura + efectivo ingresado"),
                            ))
                            .when_some(tickets, |row, stats| row.child(
                                MetricCard::new(MetricAccent::TicketsReceived, "Tickets recibidos", stats.tickets_count.to_string())
                                    .icon(IconName::Info).subtitle(format!("{} · {}", period_label, format_soles(stats.ticket_amount_cents))),
                            ))
                            .when_some(pensioners, |row, stats| row
                                .child(MetricCard::new(MetricAccent::CreditBalance, "Saldo a favor", format_soles(stats.credit_balance_cents))
                                    .icon(IconName::Check).subtitle("Consolidado del padrón"))
                                .child(MetricCard::new(MetricAccent::PendingDebt, "Deuda pendiente", format_soles(stats.pending_debt_cents))
                                    .icon(IconName::TriangleAlert).subtitle("Consolidado del padrón"))
                                .child(MetricCard::new(MetricAccent::Neutral, "Pensionistas activos", stats.active.to_string())
                                    .icon(IconName::User).subtitle(format!("{} registrados · {} con huella", stats.registered, stats.fingerprint_enrolled)))),
                    )
                    .child(
                        v_flex()
                            .w_full()
                            .min_w_0()
                            .p(card_padding)
                            .gap(Spacing::SM)
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
                                        Label::new("Estado de identificación y selección")
                                            .text_size(Typography::LG)
                                            .font_semibold()
                                            .text_color(palette.foreground),
                                    )
                                    .when(reader_loading, |header| {
                                        header.child(
                                            loading::region("dashboard-reader-loading", "Comprobando el lector")
                                                .child(loading::line("status", px(92.))),
                                        )
                                    })
                                    .when(!reader_loading, |header| header.child(
                                        StatusBadge::new(reader_tone, reader_status.short_label())
                                            .loading(reader_status.is_capturing()),
                                    )),
                            )
                            .child(
                                Label::new(selected_summary.unwrap_or_else(|| {
                                    "Para abrir Atención automática, usa Buscar pensionista (Ctrl+K) o identifica al pensionista por huella.".into()
                                }))
                                .text_size(Typography::SM)
                                .text_color(palette.muted_foreground),
                            )
                            ,
                    ),
            )
            .into_any_element()
    }
}
