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
    state::session::{DEMO_EMAIL, DEMO_PASSWORD, SessionState},
    theme::{ActiveDylanTheme as _, DylanPalette, Radii, Spacing, Typography},
};

const MAIL_SVG: &[u8] = br#"<svg xmlns="http://www.w3.org/2000/svg" width="24" height="24" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="m22 7-8.991 5.727a2 2 0 0 1-2.009 0L2 7"/><rect x="2" y="4" width="20" height="16" rx="2"/></svg>"#;

const LOCK_SVG: &[u8] = br#"<svg xmlns="http://www.w3.org/2000/svg" width="24" height="24" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><rect width="18" height="11" x="3" y="11" rx="2" ry="2"/><path d="M7 11V7a5 5 0 0 1 10 0v4"/></svg>"#;

/// Pantalla de Acceso (Etapa 1): inicio de sesión contra cuenta de demostración.
/// El registro de cuentas y la recuperación de contraseña corresponden a una etapa posterior.
pub struct AuthView {
    session: Entity<SessionState>,
    email_input: Entity<InputState>,
    password_input: Entity<InputState>,
    remember_me: bool,
    error: Option<SharedString>,
    _subscriptions: [Subscription; 2],
}

impl AuthView {
    pub fn new(session: Entity<SessionState>, window: &mut Window, cx: &mut Context<Self>) -> Self {
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
                        if view.error.is_some() {
                            view.error = None;
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

        let password_sub = cx.subscribe_in(
            &password_input,
            window,
            |view, _, event, window, cx| match event {
                InputEvent::Change => {
                    if view.error.is_some() {
                        view.error = None;
                        cx.notify();
                    }
                }
                InputEvent::PressEnter { .. } => view.submit(window, cx),
                _ => {}
            },
        );

        Self {
            session,
            email_input,
            password_input,
            remember_me: false,
            error: None,
            _subscriptions: [email_sub, password_sub],
        }
    }

    /// Restablece los campos al cerrar sesión y devuelve el foco al formulario.
    pub fn on_signed_out(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.error = None;
        self.password_input
            .update(cx, |state, cx| state.clean(window, cx));
        if !self.remember_me {
            self.email_input
                .update(cx, |state, cx| state.clean(window, cx));
            self.email_input
                .update(cx, |state, cx| state.focus(window, cx));
        } else {
            self.password_input
                .update(cx, |state, cx| state.focus(window, cx));
        }
        cx.notify();
    }

    fn submit(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let email = self.email_input.read(cx).value();
        let password = self.password_input.read(cx).value();

        let result = self.session.update(cx, |session, cx| {
            session.sign_in(email.as_ref(), password.as_ref(), self.remember_me, cx)
        });

        match result {
            Ok(()) => {
                self.error = None;
                self.password_input
                    .update(cx, |state, cx| state.clean(window, cx));
            }
            Err(err) => {
                self.error = Some(SharedString::from(err.message()));
            }
        }
        cx.notify();
    }
}

impl Render for AuthView {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = *cx.dylan_theme();
        let palette = theme.palette;
        let visuals = theme.visuals();
        let viewport = window.viewport_size();
        let compact = viewport.width < px(520.0) || viewport.height < px(580.0);
        let card_padding = if compact { Spacing::LG } else { Spacing::XXL };
        let outer_padding = if compact { Spacing::MD } else { Spacing::XL };
        let logo_size = if compact { px(64.0) } else { px(84.0) };

        v_flex()
            .id("auth-scroll")
            .relative()
            .flex_1()
            .min_h_0()
            .w_full()
            .overflow_y_scroll()
            .items_center()
            .p(outer_padding)
            .child(
                v_flex()
                    .my_auto()
                    .w(px(420.0))
                    .max_w_full()
                    .flex_shrink_0()
                    .items_center()
                    .px(card_padding)
                    .py(card_padding)
                    .rounded(px(26.0))
                    .bg(visuals.panel)
                    .border_1()
                    .border_color(visuals.panel_border)
                    .shadow(visuals.panel_shadows().to_vec())
                    .child(brand_tile(visuals, logo_size))
                    .child(
                        v_flex()
                            .w_full()
                            .items_center()
                            .mt(if compact { Spacing::MD } else { Spacing::LG })
                            .gap(Spacing::XS)
                            .child(
                                Label::new("Iniciar sesión")
                                    .text_size(if compact { px(24.0) } else { px(30.0) })
                                    .line_height(if compact { px(30.0) } else { px(36.0) })
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
                            .mt(if compact { Spacing::LG } else { Spacing::XL })
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
                            .when_some(self.error.clone(), |this, message| {
                                this.child(
                                    h_flex()
                                        .w_full()
                                        .justify_center()
                                        .child(StatusBadge::new(StatusTone::Error, message)),
                                )
                            })
                            .child(
                                div().w_full().pt(Spacing::SM).child(
                                    AppButton::new("login-submit")
                                        .primary_gradient()
                                        .large()
                                        .full_width()
                                        .label("Ingresar")
                                        .on_click(cx.listener(|view, _, window, cx| {
                                            view.submit(window, cx);
                                        })),
                                ),
                            )
                            .child(
                                h_flex().w_full().justify_center().pt(Spacing::XS).child(
                                    Label::new(format!(
                                        "Cuenta demo: {DEMO_EMAIL} · {DEMO_PASSWORD}"
                                    ))
                                    .text_size(Typography::XS)
                                    .text_color(palette.muted_foreground),
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
