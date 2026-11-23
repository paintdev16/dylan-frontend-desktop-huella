use gpui_kit::{
    component::{
        IconName, StyledExt as _, h_flex,
        input::{Input, InputEvent, InputState},
        label::Label,
        v_flex,
    },
    prelude::FluentBuilder as _,
    *,
};

use crate::{
    components::{
        button::AppButton,
        status_badge::{StatusBadge, StatusTone},
    },
    services::{
        biometric::{
            BiometricBackendMode,
            storage::{BACKUP_KEY_ENV, default_backup_path, has_backup_key_configured},
        },
        preferences::{AppPreferences, PreferencesEvent, PreferencesStore},
    },
    state::reader::ReaderState,
    theme::{ActiveDylanTheme as _, Radii, Spacing, ThemePreference, ThemeStore, Typography},
};

/// Módulo de **Configuración** (Etapas 4 y 5): gestiona tema, correo recordado (`Recordarme`),
/// parámetros locales de estación y rutas del SDK biométrico SecuGen fuera de Velopack.
pub struct SettingsView {
    preferences: Entity<PreferencesStore>,
    theme: Entity<ThemeStore>,
    reader: Entity<ReaderState>,
    station_input: Entity<InputState>,
    api_url_input: Entity<InputState>,
    remembered_email_input: Entity<InputState>,
    fdx_dll_input: Entity<InputState>,
    secusearch_dll_input: Entity<InputState>,
    feedback: Option<(StatusTone, SharedString)>,
    _subscriptions: Vec<Subscription>,
}

impl SettingsView {
    pub fn new(
        preferences: Entity<PreferencesStore>,
        theme: Entity<ThemeStore>,
        reader: Entity<ReaderState>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let initial = preferences.read(cx).preferences().clone();
        let station_input = cx.new(|cx| {
            InputState::new(window, cx)
                .placeholder("Caja principal")
                .default_value(initial.station_name.clone())
        });
        let api_url_input = cx.new(|cx| {
            InputState::new(window, cx)
                .placeholder(crate::services::preferences::DEFAULT_API_BASE_URL)
                .default_value(initial.api_base_url.clone())
        });
        let remembered_email_input = cx.new(|cx| {
            InputState::new(window, cx)
                .placeholder("Sin correo recordado")
                .default_value(initial.remembered_email.clone().unwrap_or_default())
        });
        let fdx_dll_input = cx.new(|cx| {
            InputState::new(window, cx)
                .placeholder("Autodetectar sgfplib.dll (x64)")
                .default_value(initial.fdx_dll_path.clone())
        });
        let secusearch_dll_input = cx.new(|cx| {
            InputState::new(window, cx)
                .placeholder("Autodetectar secusearchapi.dll (x64)")
                .default_value(initial.secusearch_dll_path.clone())
        });

        let sub_loaded = cx.subscribe_in(
            &preferences,
            window,
            |view, _, event: &PreferencesEvent, window, cx| {
                let PreferencesEvent::Loaded(prefs) = event;
                view.sync_inputs(prefs, window, cx);
            },
        );
        let observe_prefs = cx.observe(&preferences, |_, _, cx| cx.notify());
        let observe_theme = cx.observe(&theme, |_, _, cx| cx.notify());
        let observe_reader = cx.observe(&reader, |_, _, cx| cx.notify());
        let sub_station = cx.subscribe(&station_input, |view, _, event, cx| {
            if matches!(event, InputEvent::Change) && view.feedback.is_some() {
                view.feedback = None;
                cx.notify();
            }
        });
        let sub_url = cx.subscribe(&api_url_input, |view, _, event, cx| {
            if matches!(event, InputEvent::Change) && view.feedback.is_some() {
                view.feedback = None;
                cx.notify();
            }
        });
        let sub_email = cx.subscribe(&remembered_email_input, |view, _, event, cx| {
            if matches!(event, InputEvent::Change) && view.feedback.is_some() {
                view.feedback = None;
                cx.notify();
            }
        });

        Self {
            preferences,
            theme,
            reader,
            station_input,
            api_url_input,
            remembered_email_input,
            fdx_dll_input,
            secusearch_dll_input,
            feedback: None,
            _subscriptions: vec![
                sub_loaded,
                observe_prefs,
                observe_theme,
                observe_reader,
                sub_station,
                sub_url,
                sub_email,
            ],
        }
    }

    fn sync_inputs(&mut self, prefs: &AppPreferences, window: &mut Window, cx: &mut Context<Self>) {
        self.station_input.update(cx, |input, cx| {
            input.set_value(prefs.station_name.clone(), window, cx)
        });
        self.api_url_input.update(cx, |input, cx| {
            input.set_value(prefs.api_base_url.clone(), window, cx)
        });
        self.remembered_email_input.update(cx, |input, cx| {
            input.set_value(
                prefs.remembered_email.clone().unwrap_or_default(),
                window,
                cx,
            )
        });
        self.fdx_dll_input.update(cx, |input, cx| {
            input.set_value(prefs.fdx_dll_path.clone(), window, cx)
        });
        self.secusearch_dll_input.update(cx, |input, cx| {
            input.set_value(prefs.secusearch_dll_path.clone(), window, cx)
        });
        cx.notify();
    }

    fn apply_biometric_mode(&mut self, mode: BiometricBackendMode, cx: &mut Context<Self>) {
        let fdx = self.fdx_dll_input.read(cx).value().to_string();
        let sse = self.secusearch_dll_input.read(cx).value().to_string();
        self.preferences
            .update(cx, |store, cx| store.update_biometric(mode, &fdx, &sse, cx));
        let config = self.preferences.read(cx).preferences().biometric_config();
        self.reader
            .update(cx, |reader, cx| reader.configure_backend(mode, config, cx));
        self.feedback = Some((
            StatusTone::Info,
            SharedString::from(format!(
                "Adaptador biométrico configurado: {}.",
                mode.label()
            )),
        ));
        cx.notify();
    }

    fn save_station_preferences(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let station = self.station_input.read(cx).value().to_string();
        let api_url = self.api_url_input.read(cx).value().to_string();
        let remembered = self.remembered_email_input.read(cx).value().to_string();

        let result = self.preferences.update(cx, |store, cx| {
            store.update_station(&station, &api_url, Some(&remembered), cx)
        });

        match result {
            Ok(()) => {
                let updated = self.preferences.read(cx).preferences().clone();
                self.sync_inputs(&updated, window, cx);
                self.feedback = Some((
                    StatusTone::Success,
                    SharedString::from(
                        "Preferencias guardadas. Se conservarán al reiniciar y actualizar la app.",
                    ),
                ));
            }
            Err(err) => {
                self.feedback = Some((StatusTone::Error, SharedString::from(err)));
            }
        }
        cx.notify();
    }

    fn clear_remembered_email(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.preferences
            .update(cx, |store, cx| store.set_remembered_email(None, cx));
        self.remembered_email_input
            .update(cx, |input, cx| input.clean(window, cx));
        self.feedback = Some((
            StatusTone::Info,
            SharedString::from("Se eliminó el correo guardado para Recordarme."),
        ));
        cx.notify();
    }

    fn reset_defaults(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.preferences
            .update(cx, |store, cx| store.reset_defaults(cx));
        self.theme.update(cx, |theme, cx| {
            theme.set_preference(ThemePreference::System, window, cx)
        });
        let defaults = self.preferences.read(cx).preferences().clone();
        self.sync_inputs(&defaults, window, cx);
        self.feedback = Some((
            StatusTone::Info,
            SharedString::from("Se restablecieron las preferencias por defecto."),
        ));
        cx.notify();
    }
}

impl Render for SettingsView {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let palette = cx.dylan_theme().palette;
        let compact = window.viewport_size().width < px(640.0);
        let outer_padding = if compact { Spacing::MD } else { Spacing::XL };
        let card_padding = if compact { Spacing::MD } else { Spacing::LG };

        let current_theme = self.theme.read(cx).preference();
        let prefs_ref = self.preferences.read(cx);
        let prefs_path = prefs_ref.path().display().to_string();
        let persist_error = prefs_ref.last_error().map(ToOwned::to_owned);
        let has_remembered_email = prefs_ref.preferences().remembered_email.is_some();
        let biometric_mode = prefs_ref.preferences().biometric_mode;
        let reader_ref = self.reader.read(cx);
        let reader_name = reader_ref.device_name();
        let sse_version = reader_ref.secusearch_version();
        let backup_path = default_backup_path().display().to_string();
        let backup_key_ready = has_backup_key_configured();

        v_flex()
            .id("settings-scroll")
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
                    .max_w(px(920.0))
                    .gap(if compact { Spacing::MD } else { Spacing::LG })
                    .child(
                        v_flex()
                            .w_full()
                            .gap(Spacing::XXS)
                            .p(card_padding)
                            .rounded(Radii::LG)
                            .bg(palette.surface)
                            .border_1()
                            .border_color(palette.border)
                            .child(
                                Label::new("Configuración")
                                    .text_size(if compact {
                                        Typography::XL
                                    } else {
                                        Typography::PAGE_TITLE
                                    })
                                    .font_bold()
                                    .text_color(palette.foreground),
                            )
                            .child(
                                Label::new(
                                    "Preferencias locales de la estación persistidas fuera del directorio que reemplaza Velopack.",
                                )
                                .text_size(Typography::SM)
                                .text_color(palette.muted_foreground),
                            ),
                    )
                    .child(
                        v_flex()
                            .w_full()
                            .gap(Spacing::MD)
                            .p(card_padding)
                            .rounded(Radii::LG)
                            .bg(palette.surface)
                            .border_1()
                            .border_color(palette.border)
                            .child(
                                v_flex()
                                    .gap(Spacing::XXS)
                                    .child(
                                        Label::new("Apariencia")
                                            .text_size(Typography::LG)
                                            .font_semibold()
                                            .text_color(palette.foreground),
                                    )
                                    .child(
                                        Label::new(
                                            "Selecciona el tema de la aplicación. Se aplica al instante y se recupera al volver a abrir.",
                                        )
                                        .text_size(Typography::SM)
                                        .text_color(palette.muted_foreground),
                                    ),
                            )
                            .child(
                                h_flex()
                                    .w_full()
                                    .flex_wrap()
                                    .gap(Spacing::SM)
                                    .children(ThemePreference::ALL.into_iter().map(|pref| {
                                        let active = current_theme == pref;
                                        let btn = AppButton::new(SharedString::from(format!(
                                            "settings-theme-{pref:?}"
                                        )))
                                        .label(pref.label())
                                        .on_click(cx.listener(move |view, _, window, cx| {
                                            view.theme.update(cx, |theme, cx| {
                                                theme.set_preference(pref, window, cx);
                                            });
                                            view.preferences.update(cx, |store, cx| {
                                                store.set_theme(pref, cx);
                                            });
                                            view.feedback = Some((
                                                StatusTone::Success,
                                                SharedString::from(format!(
                                                    "Tema actualizado a {}.",
                                                    pref.label()
                                                )),
                                            ));
                                            cx.notify();
                                        }));
                                        if active {
                                            btn.primary()
                                        } else {
                                            btn.secondary()
                                        }
                                    })),
                            ),
                    )
                    .child(
                        v_flex()
                            .w_full()
                            .gap(Spacing::MD)
                            .p(card_padding)
                            .rounded(Radii::LG)
                            .bg(palette.surface)
                            .border_1()
                            .border_color(palette.border)
                            .child(
                                v_flex()
                                    .gap(Spacing::XXS)
                                    .child(
                                        Label::new("Estación y acceso")
                                            .text_size(Typography::LG)
                                            .font_semibold()
                                            .text_color(palette.foreground),
                                    )
                                    .child(
                                        Label::new(
                                            "Solo se guardan datos no sensibles. Contraseñas, tokens de sesión, claves de firma y datos biométricos nunca se escriben en este archivo.",
                                        )
                                        .text_size(Typography::SM)
                                        .text_color(palette.muted_foreground),
                                    ),
                            )
                            .child(
                                v_flex()
                                    .w_full()
                                    .gap(Spacing::MD)
                                    .child(
                                        v_flex()
                                            .w_full()
                                            .gap(Spacing::XS)
                                            .child(
                                                Label::new("Nombre de la estación")
                                                    .text_size(Typography::XS)
                                                    .font_semibold()
                                                    .text_color(palette.muted_foreground),
                                            )
                                            .child(
                                                Input::new(&self.station_input)
                                                    .rounded(Radii::MD)
                                                    .bg(palette.surface),
                                            ),
                                    )
                                    .child(
                                        v_flex()
                                            .w_full()
                                            .gap(Spacing::XS)
                                            .child(
                                                Label::new("URL base del servidor (API)")
                                                    .text_size(Typography::XS)
                                                    .font_semibold()
                                                    .text_color(palette.muted_foreground),
                                            )
                                            .child(
                                                Input::new(&self.api_url_input)
                                                    .rounded(Radii::MD)
                                                    .bg(palette.surface),
                                            ),
                                    )
                                    .child(
                                        v_flex()
                                            .w_full()
                                            .gap(Spacing::XS)
                                            .child(
                                                h_flex()
                                                    .w_full()
                                                    .items_center()
                                                    .justify_between()
                                                    .child(
                                                        Label::new("Correo guardado (Recordarme)")
                                                            .text_size(Typography::XS)
                                                            .font_semibold()
                                                            .text_color(palette.muted_foreground),
                                                    )
                                                    .when(has_remembered_email, |this| {
                                                        this.child(
                                                            AppButton::new(
                                                                "settings-clear-email",
                                                            )
                                                            .tertiary()
                                                            .xsmall()
                                                            .label("Limpiar correo")
                                                            .on_click(cx.listener(
                                                                |view, _, window, cx| {
                                                                    view.clear_remembered_email(
                                                                        window, cx,
                                                                    );
                                                                },
                                                            )),
                                                        )
                                                    }),
                                            )
                                            .child(
                                                Input::new(&self.remembered_email_input)
                                                    .rounded(Radii::MD)
                                                    .bg(palette.surface),
                                            ),
                                    ),
                            )
                            .when_some(self.feedback.clone(), |this, (tone, msg)| {
                                this.child(StatusBadge::new(tone, msg))
                            })
                            .when_some(persist_error, |this, err| {
                                this.child(StatusBadge::new(StatusTone::Error, err))
                            })
                            .child(
                                h_flex()
                                    .w_full()
                                    .flex_wrap()
                                    .items_center()
                                    .justify_between()
                                    .gap(Spacing::SM)
                                    .child(
                                        AppButton::new("settings-reset-defaults")
                                            .secondary()
                                            .label("Restablecer valores")
                                            .on_click(cx.listener(|view, _, window, cx| {
                                                view.reset_defaults(window, cx);
                                            })),
                                    )
                                    .child(
                                        AppButton::new("settings-save")
                                            .primary()
                                            .icon(IconName::Check)
                                            .label("Guardar preferencias")
                                            .on_click(cx.listener(|view, _, window, cx| {
                                                view.save_station_preferences(window, cx);
                                            })),
                                    ),
                            ),
                    )
                    .child(
                        v_flex()
                            .w_full()
                            .gap(Spacing::MD)
                            .p(card_padding)
                            .rounded(Radii::LG)
                            .bg(palette.surface)
                            .border_1()
                            .border_color(palette.border)
                            .child(
                                h_flex()
                                    .w_full()
                                    .flex_wrap()
                                    .items_center()
                                    .justify_between()
                                    .gap(Spacing::SM)
                                    .child(
                                        v_flex()
                                            .gap(Spacing::XXS)
                                            .child(
                                                Label::new("Lector biométrico (SecuGen)")
                                                    .text_size(Typography::LG)
                                                    .font_semibold()
                                                    .text_color(palette.foreground),
                                            )
                                            .child(
                                                Label::new(format!(
                                                    "Activo: {reader_name}{}",
                                                    sse_version
                                                        .map(|v| format!(" · Motor {v}"))
                                                        .unwrap_or_default()
                                                ))
                                                .text_size(Typography::XS)
                                                .text_color(palette.muted_foreground),
                                            ),
                                    )
                                    .child(if backup_key_ready {
                                        StatusBadge::new(
                                            StatusTone::Success,
                                            format!("{BACKUP_KEY_ENV} configurada"),
                                        )
                                    } else {
                                        StatusBadge::new(
                                            StatusTone::Warning,
                                            format!("{BACKUP_KEY_ENV} no definida"),
                                        )
                                    }),
                            )
                            .child(
                                h_flex()
                                    .w_full()
                                    .flex_wrap()
                                    .gap(Spacing::SM)
                                    .children(BiometricBackendMode::ALL.into_iter().map(|mode| {
                                        let active = biometric_mode == mode;
                                        let btn = AppButton::new(SharedString::from(format!(
                                            "settings-bio-mode-{mode:?}"
                                        )))
                                        .small()
                                        .label(mode.label())
                                        .on_click(cx.listener(move |view, _, _, cx| {
                                            view.apply_biometric_mode(mode, cx);
                                        }));
                                        if active {
                                            btn.primary()
                                        } else {
                                            btn.secondary()
                                        }
                                    })),
                            )
                            .child(
                                v_flex()
                                    .w_full()
                                    .gap(Spacing::SM)
                                    .child(
                                        v_flex()
                                            .w_full()
                                            .gap(Spacing::XS)
                                            .child(
                                                Label::new("Ruta FDx SDK Pro (sgfplib.dll x64)")
                                                    .text_size(Typography::XS)
                                                    .font_semibold()
                                                    .text_color(palette.muted_foreground),
                                            )
                                            .child(
                                                Input::new(&self.fdx_dll_input)
                                                    .rounded(Radii::MD)
                                                    .bg(palette.surface),
                                            ),
                                    )
                                    .child(
                                        v_flex()
                                            .w_full()
                                            .gap(Spacing::XS)
                                            .child(
                                                Label::new(
                                                    "Ruta SecuSearch Pro (secusearchapi.dll x64)",
                                                )
                                                .text_size(Typography::XS)
                                                .font_semibold()
                                                .text_color(palette.muted_foreground),
                                            )
                                            .child(
                                                Input::new(&self.secusearch_dll_input)
                                                    .rounded(Radii::MD)
                                                    .bg(palette.surface),
                                            ),
                                    )
                                    .child(
                                        h_flex()
                                            .w_full()
                                            .flex_wrap()
                                            .items_center()
                                            .justify_between()
                                            .gap(Spacing::SM)
                                            .child(
                                                Label::new(format!(
                                                    "Respaldo cifrado AES-256-GCM: {backup_path}"
                                                ))
                                                .text_size(Typography::XS)
                                                .text_color(palette.muted_foreground),
                                            )
                                            .child(
                                                AppButton::new("settings-apply-biometric")
                                                    .secondary()
                                                    .small()
                                                    .icon(IconName::RefreshCw)
                                                    .label("Aplicar rutas del lector")
                                                    .on_click(cx.listener(move |view, _, _, cx| {
                                                        view.apply_biometric_mode(
                                                            biometric_mode,
                                                            cx,
                                                        );
                                                    })),
                                            ),
                                    ),
                            ),
                    )
                    .child(
                        v_flex()
                            .w_full()
                            .gap(Spacing::XS)
                            .p(card_padding)
                            .rounded(Radii::LG)
                            .bg(palette.surface)
                            .border_1()
                            .border_color(palette.border)
                            .child(
                                Label::new("Ubicación del archivo de preferencias")
                                    .text_size(Typography::SM)
                                    .font_semibold()
                                    .text_color(palette.foreground),
                            )
                            .child(
                                Label::new(prefs_path)
                                    .text_size(Typography::XS)
                                    .text_color(palette.muted_foreground),
                            ),
                    ),
            )
    }
}
