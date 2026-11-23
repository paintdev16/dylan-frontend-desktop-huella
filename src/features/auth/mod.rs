use gpui_kit::{
    component::{
        Icon, IconName, Sizable as _, StyledExt as _,
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
    services::{
        api::{ApiError, auth},
        preferences::{DEFAULT_API_BASE_URL, PreferencesEvent, PreferencesStore},
    },
    state::session::SessionState,
    theme::{ActiveDylanTheme as _, DylanPalette, Radii, Spacing, Typography},
};

const MAIL_SVG: &[u8] = br#"<svg xmlns="http://www.w3.org/2000/svg" width="24" height="24" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="m22 7-8.991 5.727a2 2 0 0 1-2.009 0L2 7"/><rect x="2" y="4" width="20" height="16" rx="2"/></svg>"#;

const LOCK_SVG: &[u8] = br#"<svg xmlns="http://www.w3.org/2000/svg" width="24" height="24" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><rect width="18" height="11" x="3" y="11" rx="2" ry="2"/><path d="M7 11V7a5 5 0 0 1 10 0v4"/></svg>"#;

/// Pantalla de Acceso conectada a `POST /api/login` del backend Laravel Sanctum.
///
/// Ejecuta las peticiones HTTP fuera del hilo de UI de GPUI, bloquea envíos repetidos
/// mientras el servidor responde y solo persiste el correo recordado (`Recordarme`) y
/// la URL del servidor en `PreferencesStore` (nunca contraseñas ni tokens).
pub struct AuthView {
    session: Entity<SessionState>,
    preferences: Entity<PreferencesStore>,
    email_input: Entity<InputState>,
    password_input: Entity<InputState>,
    api_url_input: Entity<InputState>,
    remember_me: bool,
    submitting: bool,
    show_server_config: bool,
    feedback: Option<(StatusTone, SharedString)>,
    _subscriptions: [Subscription; 4],
}

impl AuthView {
    pub fn new(
        session: Entity<SessionState>,
        preferences: Entity<PreferencesStore>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let initial_prefs = preferences.read(cx).preferences().clone();
        let initial_remember = initial_prefs.remembered_email.is_some();

        let email_input = cx.new(|cx| {
            let state = InputState::new(window, cx).placeholder("Correo electrónico");
            if let Some(email) = initial_prefs.remembered_email.clone() {
                state.default_value(email)
            } else {
                state
            }
        });
        let password_input = cx.new(|cx| {
            InputState::new(window, cx)
                .placeholder("Contraseña")
                .masked(true)
        });
        let api_url_input = cx.new(|cx| {
            InputState::new(window, cx)
                .placeholder(DEFAULT_API_BASE_URL)
                .default_value(initial_prefs.api_base_url.clone())
        });

        let prefs_sub = cx.subscribe_in(
            &preferences,
            window,
            |view, _, event: &PreferencesEvent, window, cx| {
                let PreferencesEvent::Loaded(prefs) = event;
                if view.email_input.read(cx).value().trim().is_empty()
                    && let Some(email) = prefs.remembered_email.clone()
                {
                    view.email_input
                        .update(cx, |state, cx| state.set_value(email, window, cx));
                    view.remember_me = true;
                }
                if !view.show_server_config {
                    let url = prefs.api_base_url.clone();
                    view.api_url_input
                        .update(cx, |state, cx| state.set_value(url, window, cx));
                }
                cx.notify();
            },
        );

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
                        if !view.submitting {
                            view.password_input
                                .update(cx, |state, cx| state.focus(window, cx));
                        }
                    }
                    _ => {}
                },
            );

        let password_sub = cx.subscribe_in(
            &password_input,
            window,
            |view, _, event, window, cx| match event {
                InputEvent::Change => {
                    if view.feedback.is_some() {
                        view.feedback = None;
                        cx.notify();
                    }
                }
                InputEvent::PressEnter { .. } => view.submit(window, cx),
                _ => {}
            },
        );

        let api_url_sub =
            cx.subscribe_in(
                &api_url_input,
                window,
                |view, _, event, window, cx| match event {
                    InputEvent::Change => {
                        if view.feedback.is_some() {
                            view.feedback = None;
                            cx.notify();
                        }
                    }
                    InputEvent::PressEnter { .. } => {
                        let raw_url = view.api_url_input.read(cx).value().to_string();
                        let saved = view
                            .preferences
                            .update(cx, |prefs, cx| prefs.set_api_base_url(&raw_url, cx));
                        match saved {
                            Ok(normalized) => {
                                view.api_url_input.update(cx, |state, cx| {
                                    state.set_value(normalized, window, cx);
                                });
                                view.show_server_config = false;
                                view.feedback = Some((
                                    StatusTone::Info,
                                    SharedString::from("Servidor API actualizado."),
                                ));
                            }
                            Err(msg) => {
                                view.feedback = Some((StatusTone::Error, SharedString::from(msg)));
                            }
                        }
                        cx.notify();
                    }
                    _ => {}
                },
            );

        Self {
            session,
            preferences,
            email_input,
            password_input,
            api_url_input,
            remember_me: initial_remember,
            submitting: false,
            show_server_config: false,
            feedback: None,
            _subscriptions: [prefs_sub, email_sub, password_sub, api_url_sub],
        }
    }

    /// Restablece los campos al cerrar sesión o expirar el token y devuelve el foco al formulario.
    pub fn on_signed_out(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.submitting = false;
        let expiration_notice = self
            .session
            .update(cx, |session, _| session.take_expiration_notice());
        self.feedback = expiration_notice.map(|msg| (StatusTone::Warning, msg));

        self.password_input
            .update(cx, |state, cx| state.clean(window, cx));

        let prefs = self.preferences.read(cx).preferences().clone();
        self.api_url_input.update(cx, |state, cx| {
            state.set_value(prefs.api_base_url, window, cx)
        });

        if let Some(email) = prefs.remembered_email {
            self.remember_me = true;
            self.email_input
                .update(cx, |state, cx| state.set_value(email, window, cx));
            self.password_input
                .update(cx, |state, cx| state.focus(window, cx));
        } else {
            self.remember_me = false;
            self.email_input
                .update(cx, |state, cx| state.clean(window, cx));
            self.email_input
                .update(cx, |state, cx| state.focus(window, cx));
        }
        cx.notify();
    }

    fn submit(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.submitting {
            return;
        }

        let email = self.email_input.read(cx).value().to_string();
        let password = self.password_input.read(cx).value().to_string();
        if email.trim().is_empty() || password.is_empty() {
            self.feedback = Some((
                StatusTone::Error,
                SharedString::from(ApiError::EmptyFields.user_message()),
            ));
            cx.notify();
            return;
        }

        let raw_api_url = self.api_url_input.read(cx).value().to_string();
        let api_base_url = match self
            .preferences
            .update(cx, |prefs, cx| prefs.set_api_base_url(&raw_api_url, cx))
        {
            Ok(valid) => {
                self.api_url_input
                    .update(cx, |state, cx| state.set_value(valid.clone(), window, cx));
                valid
            }
            Err(msg) => {
                self.feedback = Some((StatusTone::Error, SharedString::from(msg)));
                cx.notify();
                return;
            }
        };

        let station_name = self.preferences.read(cx).preferences().station_name.clone();
        let device_name = format!("Dylan Comedor Desktop ({station_name})");
        let remember_me = self.remember_me;
        let email_for_request = email.clone();

        self.submitting = true;
        self.feedback = None;
        cx.notify();

        let background = cx.background_executor().spawn(async move {
            auth::login(&api_base_url, &email_for_request, &password, &device_name)
        });

        cx.spawn_in(window, async move |this, cx| {
            let result = background.await;
            let _ = this.update_in(cx, |view, window, cx| {
                view.submitting = false;
                match result {
                    Ok(authenticated) => {
                        view.feedback = None;
                        view.show_server_config = false;
                        let remembered = remember_me
                            .then(|| email.trim().to_string())
                            .filter(|value| !value.is_empty());
                        view.preferences.update(cx, |prefs, cx| {
                            prefs.set_remembered_email(remembered, cx);
                        });
                        view.password_input
                            .update(cx, |state, cx| state.clean(window, cx));
                        view.session.update(cx, |session, cx| {
                            session.sign_in_with_backend(authenticated, remember_me, cx);
                        });
                    }
                    Err(err) => {
                        let tone = if err.is_warning_tone() {
                            StatusTone::Warning
                        } else {
                            StatusTone::Error
                        };
                        view.feedback = Some((tone, SharedString::from(err.user_message())));
                    }
                }
                cx.notify();
            });
        })
        .detach();
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
        let current_api_url = self.preferences.read(cx).preferences().api_base_url.clone();

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
                                            if !view.submitting {
                                                view.remember_me = *checked;
                                                cx.notify();
                                            }
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
                                        .disabled(self.submitting)
                                        .loading(self.submitting)
                                        .label(if self.submitting {
                                            "Ingresando…"
                                        } else {
                                            "Ingresar"
                                        })
                                        .on_click(cx.listener(|view, _, window, cx| {
                                            view.submit(window, cx);
                                        })),
                                ),
                            )
                            .child(
                                v_flex()
                                    .w_full()
                                    .pt(Spacing::XS)
                                    .gap(Spacing::XS)
                                    .child(
                                        h_flex()
                                            .w_full()
                                            .items_center()
                                            .justify_between()
                                            .gap(Spacing::XS)
                                            .child(
                                                Label::new(format!("Servidor: {current_api_url}"))
                                                    .text_size(Typography::XS)
                                                    .text_color(palette.muted_foreground),
                                            )
                                            .child(
                                                AppButton::new("toggle-server-config")
                                                    .tertiary()
                                                    .xsmall()
                                                    .icon(IconName::Settings)
                                                    .disabled(self.submitting)
                                                    .label(if self.show_server_config {
                                                        "Ocultar"
                                                    } else {
                                                        "Cambiar"
                                                    })
                                                    .on_click(cx.listener(
                                                        |view, _, window, cx| {
                                                            view.show_server_config =
                                                                !view.show_server_config;
                                                            if view.show_server_config {
                                                                view.api_url_input.update(
                                                                    cx,
                                                                    |state, cx| {
                                                                        state.focus(window, cx)
                                                                    },
                                                                );
                                                            }
                                                            cx.notify();
                                                        },
                                                    )),
                                            ),
                                    )
                                    .when(self.show_server_config, |this| {
                                        this.child(
                                            Input::new(&self.api_url_input)
                                                .small()
                                                .rounded(Radii::MD)
                                                .bg(palette.surface),
                                        )
                                    }),
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
