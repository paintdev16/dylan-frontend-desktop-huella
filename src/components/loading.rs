use gpui_kit::{
    component::{h_flex, skeleton::Skeleton, v_flex},
    *,
};

use crate::theme::{ActiveDylanTheme as _, Radii, Spacing};

pub fn region(id: &'static str, label: &'static str) -> Stateful<Div> {
    v_flex()
        .id(id)
        .role(Role::Status)
        .aria_label(label)
        .gap(Spacing::SM)
}

pub fn line(id: &'static str, width: Pixels) -> impl IntoElement {
    div()
        .id(id)
        .w(width)
        .max_w_full()
        .child(Skeleton::new().h(px(12.)).rounded(Radii::SM))
}

pub fn block(id: &'static str, height: Pixels) -> impl IntoElement {
    div()
        .id(id)
        .w_full()
        .child(Skeleton::new().h(height).rounded(Radii::MD))
}

pub fn rows(id: &'static str, label: &'static str, count: usize, cx: &App) -> impl IntoElement {
    let palette = cx.dylan_theme().palette;
    region(id, label).w_full().children((0..count).map(|index| {
        h_flex()
            .id(("loading-row", index))
            .justify_between()
            .items_center()
            .gap(Spacing::SM)
            .p(Spacing::MD)
            .rounded(Radii::MD)
            .bg(palette.surface_subtle)
            .child(
                v_flex()
                    .flex_1()
                    .min_w_0()
                    .gap(Spacing::SM)
                    .child(line("primary", px(160.)))
                    .child(line("secondary", px(96.))),
            )
            .child(line("summary", px(46.)))
    }))
}

pub fn metrics(id: &'static str, label: &'static str, count: usize, cx: &App) -> impl IntoElement {
    let palette = cx.dylan_theme().palette;
    h_flex()
        .id(id)
        .role(Role::Status)
        .aria_label(label)
        .w_full()
        .flex_wrap()
        .gap(Spacing::MD)
        .children((0..count).map(|index| {
            v_flex()
                .id(("loading-metric", index))
                .flex_1()
                .min_w(px(160.))
                .max_w_full()
                .p(Spacing::LG)
                .gap(Spacing::SM)
                .rounded(Radii::LG)
                .bg(palette.surface)
                .border_1()
                .border_color(palette.border)
                .child(line("label", px(94.)))
                .child(div().w(px(74.)).child(block("value", px(26.))))
                .child(line("detail", px(114.)))
        }))
}
