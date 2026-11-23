use std::rc::Rc;

use gpui_kit::{
    component::{
        ActiveTheme as _, Icon, IconName, Selectable as _, Sizable as _, StyledExt as _,
        button::{Button, ButtonCustomVariant, ButtonVariants as _},
        h_flex,
        label::Label,
        v_flex,
    },
    prelude::FluentBuilder as _,
    *,
};

use crate::{
    components::status_badge::{StatusBadge, StatusTone},
    state::{
        reader::ReaderStatus,
        session::{AppRoute, RouteGroup},
    },
    theme::{ActiveDylanTheme as _, Radii, Spacing, Typography},
};

pub const SIDEBAR_EXPANDED_WIDTH: Pixels = px(236.0);
pub const SIDEBAR_COLLAPSED_WIDTH: Pixels = px(64.0);

type SelectRouteHandler = Rc<dyn Fn(AppRoute, &mut Window, &mut App)>;

/// Agrupa las rutas disponibles siguiendo el orden canónico de grupos del comedor
/// y omite grupos vacíos cuando sus módulos aún no están habilitados o permitidos.
pub fn grouped_routes(routes: &[AppRoute]) -> Vec<(RouteGroup, Vec<AppRoute>)> {
    RouteGroup::ALL
        .into_iter()
        .filter_map(|group| {
            let items: Vec<AppRoute> = routes
                .iter()
                .copied()
                .filter(|route| route.group() == group)
                .collect();
            (!items.is_empty()).then_some((group, items))
        })
        .collect()
}

/// Sidebar colapsable reutilizable del área autenticada.
/// Conserva la ruta activa, respeta los permisos de la sesión y usa iconos con
/// nombres accesibles y tooltips al colapsarse.
#[derive(IntoElement)]
pub struct AppSidebar {
    routes: Vec<AppRoute>,
    active_route: AppRoute,
    collapsed: bool,
    reader_status: ReaderStatus,
    selected_label: Option<SharedString>,
    on_select: SelectRouteHandler,
}

impl AppSidebar {
    pub fn new(
        routes: Vec<AppRoute>,
        active_route: AppRoute,
        collapsed: bool,
        reader_status: ReaderStatus,
        on_select: impl Fn(AppRoute, &mut Window, &mut App) + 'static,
    ) -> Self {
        Self {
            routes,
            active_route,
            collapsed,
            reader_status,
            selected_label: None,
            on_select: Rc::new(on_select),
        }
    }

    pub fn selected_label(mut self, label: Option<SharedString>) -> Self {
        self.selected_label = label;
        self
    }
}

impl RenderOnce for AppSidebar {
    fn render(self, _: &mut Window, cx: &mut App) -> impl IntoElement {
        let gpui_theme = cx.theme().colors;
        let transparent = cx.theme().transparent;
        let palette = cx.dylan_theme().palette;
        let groups = grouped_routes(&self.routes);
        let collapsed = self.collapsed;

        let reader_tone = match self.reader_status {
            ReaderStatus::Disconnected { .. } => StatusTone::Error,
            ReaderStatus::Ready | ReaderStatus::Matched { .. } | ReaderStatus::Enrolled { .. } => {
                StatusTone::Success
            }
            ReaderStatus::Capturing { .. } => StatusTone::Info,
            ReaderStatus::Unrecognized | ReaderStatus::EnrollmentMismatch => StatusTone::Warning,
            ReaderStatus::Cancelled => StatusTone::Neutral,
        };
        let reader_dot_color = match reader_tone {
            StatusTone::Success => palette.success.icon,
            StatusTone::Warning => palette.warning.icon,
            StatusTone::Error => palette.error.icon,
            StatusTone::Info => palette.info.icon,
            StatusTone::Neutral => palette.neutral.icon,
        };

        let nav_sections = v_flex()
            .id("sidebar-nav-scroll")
            .flex_1()
            .min_h_0()
            .w_full()
            .overflow_y_scroll()
            .p(Spacing::SM)
            .gap(Spacing::MD)
            .children(
                groups
                    .into_iter()
                    .enumerate()
                    .map(|(index, (group, items))| {
                        let on_select_group = self.on_select.clone();

                        v_flex()
                            .w_full()
                            .gap(Spacing::XS)
                            .when(collapsed && index > 0, |this| {
                                this.pt(Spacing::XS)
                                    .border_t_1()
                                    .border_color(gpui_theme.sidebar_border)
                            })
                            .when(!collapsed, |this| {
                                this.child(
                                    div().px(Spacing::SM).pb(Spacing::XXS).child(
                                        Label::new(group.label())
                                            .text_size(Typography::XS)
                                            .font_semibold()
                                            .text_color(palette.muted_foreground),
                                    ),
                                )
                            })
                            .children(items.into_iter().map(|route| {
                                let is_active = route == self.active_route;
                                let on_select = on_select_group.clone();
                                let label = route.label();

                                let mut item = Button::new(route.id())
                                    .rounded(Radii::MD)
                                    .selected(is_active)
                                    .accessibility_label(label)
                                    .when(collapsed, |this| this.tooltip(label))
                                    .on_click(move |_, window, cx| on_select(route, window, cx));

                                if is_active {
                                    let active_style = ButtonCustomVariant::new(cx)
                                        .color(gpui_theme.sidebar_primary)
                                        .foreground(gpui_theme.sidebar_accent_foreground)
                                        .hover(gpui_theme.sidebar_accent)
                                        .active(gpui_theme.sidebar_accent);
                                    item = item
                                        .custom(active_style)
                                        .outline()
                                        .bg(gpui_theme.sidebar_accent)
                                        .border_color(gpui_theme.sidebar_primary);
                                } else {
                                    item = item.ghost();
                                }

                                if collapsed {
                                    item.icon(route.icon()).w(px(40.0)).h(px(38.0))
                                } else {
                                    let indicator_color = if is_active {
                                        gpui_theme.sidebar_primary
                                    } else {
                                        transparent
                                    };
                                    let text_color = if is_active {
                                        gpui_theme.sidebar_accent_foreground
                                    } else {
                                        gpui_theme.sidebar_foreground
                                    };
                                    item.w_full().h(px(38.0)).px(Spacing::SM).child(
                                        h_flex()
                                            .w_full()
                                            .items_center()
                                            .gap(Spacing::SM)
                                            .child(
                                                div()
                                                    .w(px(3.0))
                                                    .h(px(18.0))
                                                    .rounded(Radii::FULL)
                                                    .bg(indicator_color),
                                            )
                                            .child(
                                                Icon::new(route.icon())
                                                    .small()
                                                    .text_color(text_color),
                                            )
                                            .child(
                                                Label::new(label)
                                                    .text_size(Typography::SM)
                                                    .font_medium()
                                                    .text_color(text_color),
                                            ),
                                    )
                                }
                            }))
                    }),
            );

        let on_select_pensioners = self.on_select.clone();
        let can_open_pensioners = self.routes.contains(&AppRoute::Pensioners);

        let footer = v_flex()
            .w_full()
            .flex_shrink_0()
            .p(Spacing::SM)
            .gap(Spacing::SM)
            .border_t_1()
            .border_color(gpui_theme.sidebar_border)
            .when(!collapsed, |this| {
                this.when_some(self.selected_label.clone(), |panel, chip| {
                    panel.child(
                        Button::new("sidebar-selected-pensioner")
                            .ghost()
                            .w_full()
                            .rounded(Radii::MD)
                            .bg(palette.neutral.tint)
                            .border_1()
                            .border_color(palette.neutral.border)
                            .tooltip("Ver ficha del pensionista seleccionado")
                            .when(can_open_pensioners, |button| {
                                let on_select = on_select_pensioners.clone();
                                button.on_click(move |_, window, cx| {
                                    on_select(AppRoute::Pensioners, window, cx);
                                })
                            })
                            .child(
                                h_flex()
                                    .w_full()
                                    .min_w_0()
                                    .items_center()
                                    .gap(Spacing::XS)
                                    .child(
                                        Icon::new(IconName::Check)
                                            .xsmall()
                                            .text_color(palette.neutral.icon),
                                    )
                                    .child(
                                        div()
                                            .flex_1()
                                            .min_w_0()
                                            .overflow_hidden()
                                            .whitespace_nowrap()
                                            .text_ellipsis()
                                            .child(
                                                Label::new(chip)
                                                    .text_size(Typography::XS)
                                                    .font_medium()
                                                    .text_color(palette.neutral.foreground),
                                            ),
                                    ),
                            ),
                    )
                })
                .child(
                    h_flex().w_full().child(
                        StatusBadge::new(reader_tone, self.reader_status.short_label())
                            .loading(self.reader_status.is_capturing()),
                    ),
                )
            })
            .when(collapsed, |this| {
                this.items_center().child(
                    Button::new("sidebar-reader-status-dot")
                        .ghost()
                        .xsmall()
                        .accessibility_label(self.reader_status.short_label())
                        .tooltip(self.reader_status.short_label())
                        .child(
                            div()
                                .size(px(10.0))
                                .rounded(Radii::FULL)
                                .bg(reader_dot_color),
                        ),
                )
            });

        v_flex()
            .id("app-sidebar")
            .w(if collapsed {
                SIDEBAR_COLLAPSED_WIDTH
            } else {
                SIDEBAR_EXPANDED_WIDTH
            })
            .h_full()
            .flex_shrink_0()
            .bg(gpui_theme.sidebar)
            .border_r_1()
            .border_color(gpui_theme.sidebar_border)
            .child(nav_sections)
            .child(footer)
    }
}

#[cfg(test)]
mod tests {
    use super::grouped_routes;
    use crate::state::session::{AppRoute, RouteGroup};

    #[test]
    fn groups_only_available_routes_in_canonical_order() {
        let groups = grouped_routes(&AppRoute::IMPLEMENTED);
        assert_eq!(groups.len(), 4);
        assert_eq!(groups[0], (RouteGroup::Home, vec![AppRoute::Dashboard]));
        assert_eq!(
            groups[1],
            (
                RouteGroup::Operation,
                vec![
                    AppRoute::Pensioners,
                    AppRoute::Identification,
                    AppRoute::Consumptions
                ]
            )
        );
        assert_eq!(
            groups[2],
            (
                RouteGroup::Administration,
                vec![AppRoute::Cash, AppRoute::Reports]
            )
        );
        assert_eq!(groups[3], (RouteGroup::System, vec![AppRoute::Settings]));

        let restricted = grouped_routes(&[AppRoute::Dashboard]);
        assert_eq!(restricted.len(), 1);
        assert_eq!(restricted[0], (RouteGroup::Home, vec![AppRoute::Dashboard]));
    }
}
