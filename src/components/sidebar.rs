use std::{rc::Rc, time::Duration};

use gpui_kit::{
    component::{
        ActiveTheme as _, Icon, Placement, Selectable as _, Sizable as _, StyledExt as _,
        button::{Button, ButtonCustomVariant, ButtonVariants as _},
        h_flex,
        label::Label,
        v_flex,
    },
    prelude::FluentBuilder as _,
    *,
};

use crate::{
    state::session::{AppRoute, RouteGroup},
    theme::{ActiveDylanTheme as _, Radii, Spacing, Typography},
};

pub const SIDEBAR_EXPANDED_WIDTH: Pixels = px(236.0);
pub const SIDEBAR_COLLAPSED_WIDTH: Pixels = px(64.0);
const COLLAPSED_TOOLTIP_DELAY: Duration = Duration::from_millis(120);

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
    on_select: SelectRouteHandler,
}

impl AppSidebar {
    pub fn new(
        routes: Vec<AppRoute>,
        active_route: AppRoute,
        collapsed: bool,
        on_select: impl Fn(AppRoute, &mut Window, &mut App) + 'static,
    ) -> Self {
        Self {
            routes,
            active_route,
            collapsed,
            on_select: Rc::new(on_select),
        }
    }
}

impl RenderOnce for AppSidebar {
    fn render(self, _: &mut Window, cx: &mut App) -> impl IntoElement {
        let gpui_theme = cx.theme().colors;
        let transparent = cx.theme().transparent;
        let palette = cx.dylan_theme().palette;
        let groups = grouped_routes(&self.routes);
        let collapsed = self.collapsed;

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
                            .when(collapsed, |this| this.items_center())
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
                                    .when(collapsed, |this| {
                                        this.tooltip(label)
                                            .tooltip_placement(Placement::Right)
                                            .tooltip_show_delay(COLLAPSED_TOOLTIP_DELAY)
                                    })
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
