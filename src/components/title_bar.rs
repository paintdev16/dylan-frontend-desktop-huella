use crate::components::brand::dylan_mark;
use crate::theme::{
    ActiveDylanTheme as _, Radii, Spacing, ThemePreference, ThemeStore, Typography,
    visuals::{TITLE_BAR_HEIGHT, WINDOW_CONTROL_WIDTH},
};
use gpui_kit::{
    component::{
        ActiveTheme as _, Icon, IconName, Placement, Side, Sizable as _, StyledExt as _,
        button::{Button, ButtonVariants as _},
        h_flex,
        label::Label,
        menu::{DropdownMenu as _, PopupMenuItem},
        tooltip::Tooltip,
    },
    prelude::FluentBuilder as _,
    *,
};

const SIDEBAR_TOGGLE_SIZE: Pixels = px(28.);

/// Barra de título personalizada de Dylan dividida en zonas independientes:
/// 1. Zonas arrastrables (`WindowControlArea::Drag`): marca, título y espaciadores libres.
/// 2. Zonas interactivas (sin `Drag` -> `HTCLIENT` en Windows): sidebar, buscador,
///    actualización, tema y cuenta.
/// 3. Controles nativos de ventana (`Min`, `Max`, `Close`).
///
/// Mantener las zonas interactivas fuera de `WindowControlArea::Drag` evita que `WM_NCHITTEST`
/// (`HTCAPTION`) en Windows capture el primer clic sobre botones o campos de la barra.
pub struct TitleBarSlots {
    pub sidebar_toggle: Option<AnyElement>,
    pub search: Option<AnyElement>,
    pub reader_connected: bool,
    pub update: Option<AnyElement>,
    pub account: Option<AnyElement>,
}

pub fn app_title_bar(
    title: impl Into<SharedString>,
    slots: TitleBarSlots,
    theme_store: Entity<ThemeStore>,
    window: &Window,
    cx: &App,
) -> impl IntoElement {
    let palette = cx.dylan_theme().palette;
    let title_bar_bg = palette.title_bar;
    let title_bar_border = cx.theme().title_bar_border;
    let current_theme = theme_store.read(cx).preference();

    let show_window_controls = cfg!(target_os = "windows")
        || (cfg!(target_os = "linux")
            && matches!(window.window_decorations(), Decorations::Client { .. }));
    let viewport_width = window.viewport_size().width;
    let compact = viewport_width < px(540.0);
    let show_title = slots.search.is_none() || viewport_width >= px(560.0);

    let title_region = title_bar_drag("app-title-bar-drag")
        .flex()
        .flex_initial()
        .min_w_0()
        .overflow_hidden()
        .items_center()
        .gap(Spacing::SM)
        .pl(if compact { Spacing::XS } else { Spacing::LG })
        .pr(if compact { Spacing::XS } else { Spacing::SM })
        .child(dylan_mark(px(24.0), cx))
        .when(show_title, |this| {
            this.child(
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
        });

    // Los laterales se reparten el espacio disponible para centrar el buscador.
    let leading = h_flex()
        .flex_1()
        .min_w(if slots.sidebar_toggle.is_some() {
            SIDEBAR_TOGGLE_SIZE + Spacing::XS * 2.
        } else {
            px(0.)
        })
        .h_full()
        .items_center()
        .child(title_region)
        .when_some(slots.sidebar_toggle, |side, toggle| side.child(toggle))
        .child(
            title_bar_drag("app-title-bar-drag-left-space")
                .flex_1()
                .min_w_0(),
        );

    let trailing = h_flex()
        .flex_1()
        .h_full()
        .items_center()
        .child(
            title_bar_drag("app-title-bar-drag-right")
                .flex_1()
                .min_w_0(),
        )
        .child(
            h_flex()
                .id("app-title-bar-actions")
                .h_full()
                .flex_shrink_0()
                .items_center()
                .gap(if compact { Spacing::XS } else { Spacing::SM })
                .pr(Spacing::SM)
                .child(reader_indicator(slots.reader_connected, cx))
                .when_some(slots.update, |slot, child| slot.child(child))
                .child(theme_dropdown(current_theme, theme_store, compact))
                .when_some(slots.account, |slot, child| slot.child(child)),
        )
        .when(show_window_controls, |this| {
            this.child(window_controls(window, cx))
        });

    h_flex()
        .id("app-title-bar")
        .w_full()
        .h(TITLE_BAR_HEIGHT)
        .flex_shrink_0()
        .items_center()
        .bg(title_bar_bg)
        .border_b_1()
        .border_color(title_bar_border)
        .child(leading)
        .when_some(slots.search, |bar, search| {
            bar.child(
                h_flex()
                    .id("app-title-bar-search")
                    .h_full()
                    .flex_shrink_0()
                    .items_center()
                    .px(Spacing::XS)
                    .child(search),
            )
        })
        .child(trailing)
}

fn title_bar_drag(id: &'static str) -> Stateful<Div> {
    div()
        .id(id)
        .h_full()
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
}

fn reader_indicator(connected: bool, cx: &App) -> impl IntoElement {
    let palette = cx.dylan_theme().palette;
    let (tone, label) = if connected {
        (palette.success, "Lector conectado")
    } else {
        (palette.error, "Lector desconectado")
    };

    h_flex()
        .id("titlebar-reader-status")
        .role(Role::Status)
        .aria_label(label)
        .size(px(24.))
        .flex_shrink_0()
        .items_center()
        .justify_center()
        .rounded(Radii::FULL)
        .bg(tone.tint)
        .tooltip(move |window, cx| Tooltip::new(label).build(window, cx))
        .child(
            Icon::default()
                .data(include_bytes!("../../assets/fingerprint.svg"))
                .small()
                .text_color(tone.icon),
        )
}

pub fn sidebar_toggle(collapsed: bool, on_toggle: impl Fn(&mut App) + 'static) -> impl IntoElement {
    let (icon, label) = if collapsed {
        (IconName::PanelLeftOpen, "Expandir menú lateral")
    } else {
        (IconName::PanelLeftClose, "Colapsar menú lateral")
    };
    Button::new("titlebar-sidebar-toggle")
        .ghost()
        .with_size(SIDEBAR_TOGGLE_SIZE)
        .icon(icon)
        .accessibility_label(label)
        .tooltip(label)
        .tooltip_placement(Placement::Right)
        .on_click(move |_, _, cx| on_toggle(cx))
}

pub fn account_menu(
    name: SharedString,
    window: &Window,
    on_sign_out: impl Fn(&mut App) + Clone + 'static,
) -> impl IntoElement {
    let show_name = window.viewport_size().width >= px(900.0);
    Button::new("titlebar-account")
        .ghost()
        .xsmall()
        .icon(IconName::User)
        .when(show_name, |button| {
            button.child(
                div()
                    .max_w(px(148.0))
                    .overflow_hidden()
                    .whitespace_nowrap()
                    .text_ellipsis()
                    .child(name.clone()),
            )
        })
        .dropdown_caret(show_name)
        .accessibility_label("Opciones de cuenta")
        .tooltip("Opciones de cuenta")
        .dropdown_menu_with_anchor(Anchor::TopRight, move |menu, _, _| {
            let on_sign_out = on_sign_out.clone();
            menu.item(PopupMenuItem::new(name.clone()).disabled(true))
                .separator()
                .item(
                    PopupMenuItem::new("Cerrar sesión")
                        .icon(IconName::ArrowLeft)
                        .on_click(move |_, _, cx| on_sign_out(cx)),
                )
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
