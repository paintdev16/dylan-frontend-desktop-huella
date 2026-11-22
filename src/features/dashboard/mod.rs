use gpui_kit::{
    component::{IconName, StyledExt as _, h_flex, label::Label, v_flex},
    *,
};

use crate::{
    components::{
        button::AppButton,
        metric_card::MetricCard,
        status_badge::{StatusBadge, StatusTone},
    },
    state::session::SessionState,
    theme::{ActiveDylanTheme as _, MetricAccent, Radii, Spacing, Typography},
};

/// Panel principal (Etapa 1): protegido por `SessionState` con cierre de sesión
/// y resumen visual inicial usando los acentos de métricas del tema del comedor.
pub struct DashboardView {
    session: Entity<SessionState>,
    _subscription: Subscription,
}

impl DashboardView {
    pub fn new(session: Entity<SessionState>, cx: &mut Context<Self>) -> Self {
        let subscription = cx.observe(&session, |_, _, cx| cx.notify());
        Self {
            session,
            _subscription: subscription,
        }
    }
}

impl Render for DashboardView {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let session = self.session.read(cx);
        let Some(user) = session.current_user().cloned() else {
            return div().hidden().into_any_element();
        };

        let palette = cx.dylan_theme().palette;
        let compact = window.viewport_size().width < px(640.0);
        let outer_padding = if compact { Spacing::MD } else { Spacing::XL };
        let card_padding = if compact { Spacing::MD } else { Spacing::LG };

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
                                        Label::new("Dashboard")
                                            .text_size(if compact {
                                                Typography::XL
                                            } else {
                                                Typography::PAGE_TITLE
                                            })
                                            .font_bold()
                                            .text_color(palette.foreground),
                                    )
                                    .child(
                                        Label::new(format!(
                                            "{} · {} ({})",
                                            user.name, user.email, user.role
                                        ))
                                        .text_size(Typography::SM)
                                        .text_color(palette.muted_foreground),
                                    ),
                            )
                            .child(
                                h_flex()
                                    .flex_wrap()
                                    .items_center()
                                    .gap(Spacing::SM)
                                    .child(StatusBadge::new(StatusTone::Success, "Sesión activa"))
                                    .child(
                                        AppButton::new("dashboard-sign-out")
                                            .secondary()
                                            .icon(IconName::ArrowLeft)
                                            .label("Cerrar sesión")
                                            .on_click(cx.listener(|view, _, _, cx| {
                                                view.session.update(cx, |session, cx| {
                                                    session.sign_out(cx);
                                                });
                                            })),
                                    ),
                            ),
                    )
                    .child(
                        h_flex()
                            .w_full()
                            .flex_wrap()
                            .gap(Spacing::MD)
                            .child(
                                MetricCard::new(MetricAccent::Breakfasts, "Desayunos", "0")
                                    .icon(IconName::Sun)
                                    .subtitle("Hoy"),
                            )
                            .child(
                                MetricCard::new(MetricAccent::Lunches, "Almuerzos", "0")
                                    .icon(IconName::Calendar)
                                    .subtitle("Hoy"),
                            )
                            .child(
                                MetricCard::new(
                                    MetricAccent::CashIn,
                                    "Efectivo ingresado",
                                    "S/ 0.00",
                                )
                                .icon(IconName::CircleCheck)
                                .subtitle("Caja en espera"),
                            )
                            .child(
                                MetricCard::new(
                                    MetricAccent::TicketsReceived,
                                    "Tickets recibidos",
                                    "0",
                                )
                                .icon(IconName::Info)
                                .subtitle("Periodo actual"),
                            )
                            .child(
                                MetricCard::new(
                                    MetricAccent::CreditBalance,
                                    "Saldo a favor",
                                    "S/ 0.00",
                                )
                                .icon(IconName::Check)
                                .subtitle("Consolidado"),
                            )
                            .child(
                                MetricCard::new(
                                    MetricAccent::PendingDebt,
                                    "Deuda pendiente",
                                    "S/ 0.00",
                                )
                                .icon(IconName::TriangleAlert)
                                .subtitle("Sin cobros pendientes"),
                            )
                            .child(
                                MetricCard::new(
                                    MetricAccent::Neutral,
                                    "Pensionistas activos",
                                    "0",
                                )
                                .icon(IconName::User)
                                .subtitle("Base de demostración"),
                            ),
                    )
                    .child(
                        v_flex()
                            .w_full()
                            .min_w_0()
                            .p(card_padding)
                            .gap(Spacing::XS)
                            .rounded(Radii::LG)
                            .bg(palette.surface)
                            .border_1()
                            .border_color(palette.border)
                            .child(
                                Label::new("Sesión de demostración activa")
                                    .text_size(Typography::LG)
                                    .font_semibold()
                                    .text_color(palette.foreground),
                            )
                            .child(
                                Label::new(
                                    "Las rutas de Pensionistas, Identificación, Consumos, Caja, Reportes y Configuración se habilitarán conforme se implementen sus flujos en las siguientes etapas.",
                                )
                                .text_size(Typography::SM)
                                .text_color(palette.muted_foreground),
                            ),
                    ),
            )
            .into_any_element()
    }
}
