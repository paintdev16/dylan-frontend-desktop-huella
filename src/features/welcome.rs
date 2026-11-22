use gpui_kit::{
    component::{StyledExt as _, h_flex, label::Label, v_flex},
    *,
};

use crate::{
    components::brand::brand_tile,
    theme::{DylanTheme, Radii, Spacing, visuals::AppVisuals},
};

pub fn welcome_view(theme: &DylanTheme, visuals: AppVisuals) -> impl IntoElement {
    v_flex()
        .relative()
        .flex_1()
        .min_h_0()
        .w_full()
        .items_center()
        .justify_center()
        .px(Spacing::XL)
        .pb(px(28.0))
        .child(
            div()
                .relative()
                .w(px(504.0))
                .max_w_full()
                .pt(px(33.0))
                .child(
                    v_flex()
                        .w_full()
                        .min_h(px(230.0))
                        .items_center()
                        .px(Spacing::XL)
                        .pt(px(89.0))
                        .pb(Spacing::XXL)
                        .gap(Spacing::XS)
                        .rounded(px(24.0))
                        .bg(visuals.panel)
                        .border_1()
                        .border_color(visuals.panel_border)
                        .shadow(visuals.panel_shadows().to_vec())
                        .child(
                            Label::new("Comedor Dylan")
                                .text_size(px(38.0))
                                .line_height(px(49.0))
                                .font_bold()
                                .text_center()
                                .text_color(theme.palette.foreground),
                        )
                        .child(
                            Label::new("Gestiona el comedor de forma moderna y eficiente")
                                .text_size(px(17.0))
                                .line_height(px(24.0))
                                .font_medium()
                                .text_center()
                                .text_color(theme.palette.muted_foreground),
                        )
                        .child(
                            h_flex()
                                .mt(Spacing::MD)
                                .w(px(220.0))
                                .items_center()
                                .justify_center()
                                .gap(Spacing::SM)
                                .child(div().flex_1().h(px(1.0)).bg(theme.palette.border))
                                .child(
                                    div()
                                        .w(px(64.0))
                                        .h(px(4.0))
                                        .rounded(Radii::FULL)
                                        .bg(theme.palette.brand.accent_bar_gradient()),
                                )
                                .child(div().flex_1().h(px(1.0)).bg(theme.palette.border)),
                        ),
                )
                .child(
                    div()
                        .absolute()
                        .top_0()
                        .left_0()
                        .w_full()
                        .flex()
                        .justify_center()
                        .child(brand_tile(visuals, px(104.0))),
                ),
        )
}
