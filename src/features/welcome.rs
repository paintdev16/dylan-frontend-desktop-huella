use gpui_kit::{
    component::{
        Icon, Sizable as _, StyledExt as _,
        checkbox::Checkbox,
        h_flex,
        input::{Input, InputEvent, InputState},
        label::Label,
        v_flex,
    },
    prelude::FluentBuilder as _,
    *,
};

use crate::{
    components::{
        brand::brand_tile,
        button::AppButton,
        status_badge::{StatusBadge, StatusTone},
    },
    theme::{ActiveDylanTheme as _, DylanPalette, Radii, Spacing, Typography},
};

const MAIL_SVG: &[u8] = br#"<svg xmlns="http://www.w3.org/2000/svg" width="24" height="24" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="m22 7-8.991 5.727a2 2 0 0 1-2.009 0L2 7"/><rect x="2" y="4" width="20" height="16" rx="2"/></svg>"#;

const LOCK_SVG: &[u8] = br#"<svg xmlns="http://www.w3.org/2000/svg" width="24" height="24" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><rect width="18" height="11" x="3" y="11" rx="2" ry="2"/><path d="M7 11V7a5 5 0 0 1 10 0v4"/></svg>"#;

pub struct WelcomeView {
    email_input: Entity<InputState>,
    password_input: Entity<InputState>,
    remember_me: bool,
    feedback: Option<(StatusTone, SharedString)>,
    _subscriptions: [Subscription; 2],
}

impl WelcomeView {
    pub fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        let email_input =
            cx.new(|cx| InputState::new(window, cx).placeholder("Correo electrónico"));
        let password_input = cx.new(|cx| {
            InputState::new(window, cx)
                .placeholder("Contraseña")
                .masked(true)
        });

        let email_sub =
            cx.subscribe_in(
                &email_input,
                window,
                |view, _, event, window, cx| match event {
                    InputEvent::Change => {
                        if view.feedback.is_some() {
                            view.feedback = None;
                            cx.notify();
                        }
                    }
                    InputEvent::PressEnter { .. } => {
                        view.password_input
                            .update(cx, |state, cx| state.focus(window, cx));
                    }
                    _ => {}
                },
            );

        let password_sub =
            cx.subscribe_in(
                &password_input,
                window,
                |view, _, event, _, cx| match event {
                    InputEvent::Change => {
                        if view.feedback.is_some() {
                            view.feedback = None;
                            cx.notify();
                        }
                    }
                    InputEvent::PressEnter { .. } => view.submit(cx),
                    _ => {}
                },
            );

        Self {
            email_input,
            password_input,
            remember_me: false,
            feedback: None,
            _subscriptions: [email_sub, password_sub],
        }
    }

    fn submit(&mut self, cx: &mut Context<Self>) {
        let email = self.email_input.read(cx).value();
        let password = self.password_input.read(cx).value();

        if email.trim().is_empty() || password.is_empty() {
            self.feedback = Some((
                StatusTone::Error,
                SharedString::from("Ingresa tu correo electrónico y contraseña."),
            ));
        } else {
            self.feedback = Some((
                StatusTone::Info,
                SharedString::from("Autenticación pendiente de conectar a la sesión."),
            ));
        }
        cx.notify();
    }
}

impl Render for WelcomeView {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = *cx.dylan_theme();
        let palette = theme.palette;
        let visuals = theme.visuals();

        v_flex()
            .relative()
            .flex_1()
            .min_h_0()
            .w_full()
            .items_center()
            .justify_center()
            .p(Spacing::XL)
            .child(
                v_flex()
                    .w(px(420.0))
                    .max_w_full()
                    .items_center()
                    .px(Spacing::XXL)
                    .py(Spacing::XXL)
                    .rounded(px(26.0))
                    .bg(visuals.panel)
                    .border_1()
                    .border_color(visuals.panel_border)
                    .shadow(visuals.panel_shadows().to_vec())
                    .child(brand_tile(visuals, px(84.0)))
                    .child(
                        v_flex()
                            .w_full()
                            .items_center()
                            .mt(Spacing::LG)
                            .gap(Spacing::XS)
                            .child(
                                Label::new("Iniciar sesión")
                                    .text_size(px(30.0))
                                    .line_height(px(36.0))
                                    .font_bold()
                                    .text_center()
                                    .text_color(palette.foreground),
                            )
                            .child(
                                Label::new("Accede al sistema del comedor")
                                    .text_size(Typography::BASE)
                                    .font_medium()
                                    .text_center()
                                    .text_color(palette.muted_foreground),
                            )
                            .child(
                                div()
                                    .mt(Spacing::SM)
                                    .w(px(56.0))
                                    .h(px(4.0))
                                    .rounded(Radii::FULL)
                                    .bg(palette.brand.accent_bar_gradient()),
                            ),
                    )
                    .child(
                        v_flex()
                            .w_full()
                            .mt(Spacing::XL)
                            .gap(Spacing::MD)
                            .child(
                                Input::new(&self.email_input)
                                    .large()
                                    .h(px(44.0))
                                    .rounded(Radii::LG)
                                    .bg(palette.surface)
                                    .prefix(input_icon_prefix(MAIL_SVG, palette)),
                            )
                            .child(
                                Input::new(&self.password_input)
                                    .large()
                                    .h(px(44.0))
                                    .rounded(Radii::LG)
                                    .bg(palette.surface)
                                    .prefix(input_icon_prefix(LOCK_SVG, palette))
                                    .mask_toggle(),
                            )
                            .child(
                                h_flex().w_full().items_center().pt(Spacing::XS).child(
                                    Checkbox::new("remember-me")
                                        .checked(self.remember_me)
                                        .label("Recordarme")
                                        .on_change(cx.listener(|view, checked, _, cx| {
                                            view.remember_me = *checked;
                                            cx.notify();
                                        })),
                                ),
                            )
                            .when_some(self.feedback.clone(), |this, (tone, message)| {
                                this.child(
                                    h_flex()
                                        .w_full()
                                        .justify_center()
                                        .child(StatusBadge::new(tone, message)),
                                )
                            })
                            .child(
                                div().w_full().pt(Spacing::SM).child(
                                    AppButton::new("login-submit")
                                        .primary_gradient()
                                        .large()
                                        .full_width()
                                        .label("Ingresar")
                                        .on_click(cx.listener(|view, _, _, cx| {
                                            view.submit(cx);
                                        })),
                                ),
                            ),
                    ),
            )
    }
}

fn input_icon_prefix(svg_bytes: &'static [u8], palette: DylanPalette) -> impl IntoElement {
    h_flex()
        .items_center()
        .gap(Spacing::SM)
        .pr(Spacing::XS)
        .child(
            Icon::default()
                .data(svg_bytes)
                .size(px(18.0))
                .text_color(palette.brand_accent),
        )
        .child(div().w(px(1.0)).h(px(18.0)).bg(palette.border))
}
