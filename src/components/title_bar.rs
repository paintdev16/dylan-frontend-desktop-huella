use crate::components::brand::brand_tile;
use crate::theme::{
    ActiveDylanTheme as _, Radii, Spacing, ThemePreference, ThemeStore, Typography,
    visuals::{TITLE_BAR_HEIGHT, WINDOW_CONTROL_WIDTH},
};
use gpui_kit::{
    component::{
        ActiveTheme as _, Icon, IconName, Side, Sizable as _, StyledExt as _,
        button::{Button, ButtonVariants as _},
        h_flex,
        label::Label,
        menu::{DropdownMenu as _, PopupMenuItem},
    },
    prelude::FluentBuilder as _,
    *,
};

/// Barra de título personalizada de Dylan dividida en 3 zonas independientes:
/// 1. Zona arrastrable (`WindowControlArea::Drag`): marca, título, versión y espacio libre.
/// 2. Zona interactiva (sin `Drag` -> `HTCLIENT` en Windows): controles de actualización y menú de tema.
/// 3. Controles nativos de ventana (`Min`, `Max`, `Close`).
///
/// Mantener la zona interactiva fuera de `WindowControlArea::Drag` evita que `WM_NCHITTEST`
/// (`HTCAPTION`) en Windows capture el primer clic sobre botones de la barra.
pub fn app_title_bar(
    title: impl Into<SharedString>,
    version: &str,
    update_slot: Option<AnyElement>,
    theme_store: Entity<ThemeStore>,
    window: &Window,
    cx: &App,
) -> impl IntoElement {
    let theme = cx.dylan_theme();
    let palette = theme.palette;
    let visuals = theme.visuals();
    let title_bar_bg = palette.title_bar;
    let title_bar_border = cx.theme().title_bar_border;
    let current_theme = theme_store.read(cx).preference();

    let show_window_controls = cfg!(target_os = "windows")
        || (cfg!(target_os = "linux")
            && matches!(window.window_decorations(), Decorations::Client { .. }));
    let viewport_width = window.viewport_size().width;
    let compact = viewport_width < px(540.0);
    let show_version = viewport_width >= px(430.0);

    h_flex()
        .id("app-title-bar")
        .w_full()
        .h(TITLE_BAR_HEIGHT)
        .flex_shrink_0()
        .items_center()
        .bg(title_bar_bg)
        .border_b_1()
        .border_color(title_bar_border)
        .child(
            h_flex()
                .id("app-title-bar-drag")
                .flex_1()
                .min_w_0()
                .overflow_hidden()
                .h_full()
                .items_center()
                .gap(Spacing::SM)
                .pl(if compact { Spacing::MD } else { Spacing::LG })
                .pr(Spacing::SM)
                .window_control_area(WindowControlArea::Drag)
                .when(cfg!(target_os = "linux"), |this| {
                    this.on_mouse_down(MouseButton::Left, |event, window, _| {
                        if event.click_count >= 2 {
                            window.zoom_window();
                        } else {
                            window.start_window_move();
                        }
                    })
                })
                .child(brand_tile(visuals, px(22.0)))
                .child(
                    div()
                        .min_w_0()
                        .overflow_hidden()
                        .whitespace_nowrap()
                        .text_ellipsis()
                        .child(
                            Label::new(title)
                                .text_size(if compact {
                                    Typography::BASE
                                } else {
                                    Typography::LG
                                })
                                .font_semibold()
                                .text_color(palette.foreground),
                        ),
                )
                .when(show_version, |this| {
                    this.child(
                        div()
                            .flex_shrink_0()
                            .px(Spacing::MD)
                            .py(Spacing::XS)
                            .rounded(Radii::FULL)
                            .bg(visuals.version_background)
                            .child(
                                Label::new(format!("v{version}"))
                                    .text_size(Typography::BASE)
                                    .font_semibold()
                                    .text_color(visuals.version_foreground),
                            ),
                    )
                }),
        )
        .child(
            h_flex()
                .id("app-title-bar-actions")
                .h_full()
                .flex_shrink_0()
                .items_center()
                .gap(Spacing::SM)
                .pr(Spacing::SM)
                .when_some(update_slot, |slot, child| slot.child(child))
                .child(theme_dropdown(current_theme, theme_store, compact)),
        )
        .when(show_window_controls, |this| {
            this.child(window_controls(window, cx))
        })
}

fn theme_dropdown(
    current: ThemePreference,
    theme_store: Entity<ThemeStore>,
    compact: bool,
) -> impl IntoElement {
    Button::new("theme-menu")
        .ghost()
        .small()
        .icon(current.icon())
        .when(!compact, |this| this.label(current.label()))
        .tooltip(format!("Cambiar tema ({})", current.label()))
        .dropdown_menu_with_anchor(Anchor::TopRight, move |mut menu, _, _| {
            menu = menu.check_side(Side::Right);
            for pref in ThemePreference::ALL {
                let store = theme_store.clone();
                menu = menu.item(
                    PopupMenuItem::new(pref.label())
                        .icon(pref.icon())
                        .checked(current == pref)
                        .on_click(move |_, window, cx| {
                            store.update(cx, |store, cx| {
                                store.set_preference(pref, window, cx);
                            });
                        }),
                );
            }
            menu
        })
}

fn window_controls(window: &Window, cx: &App) -> impl IntoElement {
    let is_windows = cfg!(target_os = "windows");
    let is_maximized = window.is_maximized();
    let max_icon = if is_maximized {
        IconName::WindowRestore
    } else {
        IconName::WindowMaximize
    };

    h_flex()
        .id("app-window-controls")
        .h_full()
        .flex_shrink_0()
        .items_center()
        .child(window_control_button(
            "window-minimize",
            IconName::WindowMinimize,
            WindowControlArea::Min,
            false,
            is_windows,
            cx,
        ))
        .child(window_control_button(
            "window-maximize",
            max_icon,
            WindowControlArea::Max,
            false,
            is_windows,
            cx,
        ))
        .child(window_control_button(
            "window-close",
            IconName::WindowClose,
            WindowControlArea::Close,
            true,
            is_windows,
            cx,
        ))
}

fn window_control_button(
    id: &'static str,
    icon: IconName,
    area: WindowControlArea,
    is_close: bool,
    is_windows: bool,
    cx: &App,
) -> impl IntoElement {
    let theme = cx.theme();
    let hover_bg = if is_close {
        theme.danger
    } else {
        theme.secondary_hover
    };
    let active_bg = if is_close {
        theme.danger_active
    } else {
        theme.secondary_active
    };
    let hover_fg = if is_close {
        theme.danger_foreground
    } else {
        theme.secondary_foreground
    };

    div()
        .id(id)
        .flex()
        .w(WINDOW_CONTROL_WIDTH)
        .h_full()
        .flex_shrink_0()
        .items_center()
        .justify_center()
        .text_color(theme.foreground)
        .hover(move |style| style.bg(hover_bg).text_color(hover_fg))
        .active(move |style| style.bg(active_bg).text_color(hover_fg))
        .when(is_windows, |this| this.window_control_area(area))
        .when(!is_windows, |this| {
            this.on_mouse_down(MouseButton::Left, |_, window, cx| {
                window.prevent_default();
                cx.stop_propagation();
            })
            .on_click(move |_, window, cx| {
                cx.stop_propagation();
                match area {
                    WindowControlArea::Min => window.minimize_window(),
                    WindowControlArea::Max => window.zoom_window(),
                    WindowControlArea::Close => window.remove_window(),
                    WindowControlArea::Drag => {}
                }
            })
        })
        .child(Icon::new(icon).small())
}
