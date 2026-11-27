use gpui_kit::{
    component::{
        Icon, IconName, Selectable as _, Sizable as _, StyledExt as _,
        alert::Alert,
        button::{Button, ButtonGroup},
        h_flex,
        input::{Input, InputState},
        label::Label,
        tab::{Tab, TabBar},
        v_flex,
    },
    prelude::FluentBuilder as _,
    *,
};

use crate::{
    components::{
        button::AppButton,
        loading,
        status_badge::{StatusBadge, StatusTone},
    },
    services::biometric::storage::{
        BACKUP_KEY_ENV, default_backup_path, has_backup_key_configured,
    },
    state::reader::ReaderStatus,
    theme::{ActiveDylanTheme as _, Radii, Spacing, ThemePreference, Typography},
};

use super::{SettingsSection, SettingsView};

impl SettingsView {
    fn station(&self, cx: &Context<Self>) -> Div {
        let palette = cx.dylan_theme().palette;
        let remembered = self
            .preferences
            .read(cx)
            .preferences()
            .remembered_email
            .is_some();
        let theme_preference = self.theme.read(cx).preference();

        v_flex()
            .gap(Spacing::MD)
            .child(
                panel(cx)
                    .child(section_title("Estación y acceso"))
                    .child(
                        h_flex()
                            .flex_wrap()
                            .gap(Spacing::MD)
                            .child(
                                field(
                                    "Nombre de la estación",
                                    &self.station_input,
                                    IconName::Settings,
                                    cx,
                                )
                                .flex_1()
                                .min_w(px(160.))
                                .max_w_full(),
                            )
                            .child(
                                field(
                                    "URL base del servidor",
                                    &self.api_url_input,
                                    assets::IconName::Server,
                                    cx,
                                )
                                .flex_1()
                                .min_w(px(235.))
                                .max_w_full(),
                            ),
                    )
                    .child(
                        v_flex()
                            .gap(Spacing::XS)
                            .child(
                                h_flex()
                                    .flex_wrap()
                                    .justify_between()
                                    .items_center()
                                    .gap(Spacing::XS)
                                    .child(field_label("Correo recordado", cx))
                                    .when(remembered, |row| {
                                        row.child(
                                            AppButton::new("settings-clear-email")
                                                .tertiary()
                                                .xsmall()
                                                .label("Borrar correo")
                                                .on_click(cx.listener(|view, _, window, cx| {
                                                    view.clear_remembered_email(window, cx)
                                                })),
                                        )
                                    }),
                            )
                            .child(settings_input(
                                &self.remembered_email_input,
                                assets::IconName::Mail,
                                cx,
                            ))
                            .child(
                                Label::new("Se completa al abrir la pantalla de acceso.")
                                    .text_size(Typography::XS)
                                    .text_color(palette.muted_foreground),
                            ),
                    )
                    .child(
                        h_flex().justify_end().child(
                            AppButton::new("settings-save")
                                .small()
                                .icon(IconName::Check)
                                .label("Guardar estación y acceso")
                                .on_click(cx.listener(|view, _, window, cx| {
                                    view.save_station_preferences(window, cx)
                                })),
                        ),
                    ),
            )
            .child(
                panel(cx).child(section_title("Apariencia")).child(
                    ButtonGroup::new("settings-theme")
                        .small()
                        .outline()
                        .children(ThemePreference::ALL.map(|preference| {
                            Button::new(preference.label())
                                .label(preference.label())
                                .icon(preference.icon())
                                .selected(preference == theme_preference)
                        }))
                        .on_click(cx.listener(|view, selected: &Vec<usize>, window, cx| {
                            if let Some(&preference) = selected
                                .first()
                                .and_then(|index| ThemePreference::ALL.get(*index))
                                && view.theme.read(cx).preference() != preference
                            {
                                view.theme.update(cx, |theme, cx| {
                                    theme.set_preference(preference, window, cx)
                                });
                            }
                        })),
                ),
            )
    }

    fn reader(&self, cx: &Context<Self>) -> Div {
        let palette = cx.dylan_theme().palette;
        let reader = self.reader.read(cx);
        let connecting = reader.connection_loading();
        let status = reader.status();
        let disconnection_reason = match status {
            ReaderStatus::Disconnected { reason } => Some(reason.clone()),
            _ => None,
        };
        let tone = match status {
            ReaderStatus::Disconnected { .. }
            | ReaderStatus::EnrollmentMismatch
            | ReaderStatus::Unrecognized => StatusTone::Warning,
            ReaderStatus::Capturing { .. } => StatusTone::Info,
            ReaderStatus::Cancelled => StatusTone::Neutral,
            _ => StatusTone::Success,
        };
        #[cfg(target_os = "windows")]
        let searching = self.biometric_folder_task.is_some();
        #[cfg(not(target_os = "windows"))]
        let searching = false;
        let library_header = h_flex()
            .flex_wrap()
            .justify_between()
            .items_center()
            .gap(Spacing::SM)
            .child(section_title("Bibliotecas SecuGen"));
        #[cfg(target_os = "windows")]
        let library_header = library_header.child(
            AppButton::new("settings-select-secugen-folder")
                .small()
                .icon(IconName::FolderOpen)
                .label(if searching {
                    "Buscando bibliotecas…"
                } else {
                    "Seleccionar carpeta"
                })
                .disabled(searching)
                .loading(searching)
                .on_click(
                    cx.listener(|view, _, window, cx| view.select_biometric_folder(window, cx)),
                ),
        );

        v_flex()
            .gap(Spacing::MD)
            .child(
                panel(cx)
                    .child(
                        h_flex()
                            .flex_wrap()
                            .justify_between()
                            .items_center()
                            .gap(Spacing::SM)
                            .child(section_title("Lector biométrico"))
                            .when(!connecting, |header| {
                                header.child(
                                    StatusBadge::new(tone, status.short_label())
                                        .loading(status.is_capturing()),
                                )
                            }),
                    )
                    .when(connecting, |body| {
                        body.child(
                            loading::region(
                                "settings-reader-loading",
                                "Comprobando el lector biométrico",
                            )
                            .w_full()
                            .child(loading::block("device", px(48.))),
                        )
                    })
                    .when(!connecting, |body| {
                        body.child(
                            h_flex()
                                .flex_wrap()
                                .gap(Spacing::MD)
                                .p(Spacing::SM)
                                .rounded(Radii::MD)
                                .bg(palette.surface_subtle)
                                .child(
                                    v_flex()
                                        .flex_1()
                                        .min_w(px(120.))
                                        .gap(Spacing::XXS)
                                        .child(field_label("Dispositivo", cx))
                                        .child(
                                            Label::new(reader.device_name())
                                                .text_size(Typography::SM)
                                                .font_semibold(),
                                        ),
                                )
                                .when_some(reader.secusearch_version(), |row, version| {
                                    row.child(
                                        v_flex()
                                            .gap(Spacing::XXS)
                                            .child(field_label("Motor SecuSearch", cx))
                                            .child(
                                                Label::new(version)
                                                    .text_size(Typography::SM)
                                                    .font_semibold(),
                                            ),
                                    )
                                }),
                        )
                    })
                    .when_some(
                        disconnection_reason.filter(|_| !connecting),
                        |body, reason| {
                            body.child(Alert::warning("settings-reader-status", reason).small())
                        },
                    ),
            )
            .child(
                panel(cx)
                    .child(library_header)
                    .child(field(
                        "FDx SDK Pro · sgfplib.dll (x64)",
                        &self.fdx_dll_input,
                        IconName::FileText,
                        cx,
                    ))
                    .child(field(
                        "SecuSearch Pro · secusearchapi.dll (x64)",
                        &self.secusearch_dll_input,
                        IconName::FileText,
                        cx,
                    ))
                    .child(
                        h_flex().justify_end().child(
                            AppButton::new("settings-apply-biometric")
                                .secondary()
                                .small()
                                .disabled(searching)
                                .icon(IconName::RefreshCw)
                                .label("Aplicar rutas")
                                .on_click(
                                    cx.listener(|view, _, _, cx| view.apply_biometric_paths(cx)),
                                ),
                        ),
                    ),
            )
    }

    fn files(&self, cx: &Context<Self>) -> Div {
        let palette = cx.dylan_theme().palette;
        let prefs_path = self.preferences.read(cx).path().display().to_string();
        let backup_path = default_backup_path().display().to_string();
        let key_ready = has_backup_key_configured();

        v_flex()
            .gap(Spacing::MD)
            .child(panel(cx)
                .child(section_title("Almacenamiento local"))
                .child(path_row("Archivo de preferencias", prefs_path, cx))
                .child(path_row("Respaldo biométrico cifrado · AES-256-GCM", backup_path, cx))
                .child(h_flex().flex_wrap().items_center().justify_between().gap(Spacing::SM)
                    .child(Label::new(BACKUP_KEY_ENV).text_size(Typography::XS).text_color(palette.muted_foreground))
                    .child(if key_ready {
                        StatusBadge::new(StatusTone::Success, "Clave configurada")
                    } else {
                        StatusBadge::new(StatusTone::Warning, "Clave sin configurar")
                    })))
            .child(panel(cx)
                .child(section_title("Restablecer preferencias"))
                .child(Label::new("Vuelve a los valores de fábrica para la estación, el acceso, la apariencia y el lector.")
                    .text_size(Typography::SM).text_color(palette.muted_foreground))
                .child(h_flex().justify_end().child(
                    AppButton::new("settings-reset-defaults").destructive().small().label("Restablecer valores")
                        .on_click(cx.listener(|view, _, window, cx| view.reset_defaults(window, cx))),
                )))
    }
}

impl Render for SettingsView {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let compact = window.viewport_size().width < px(640.);
        let padding = if compact { Spacing::MD } else { Spacing::XL };
        let section = if self.preferences.read(cx).is_loaded() {
            match self.section {
                SettingsSection::Station => self.station(cx),
                SettingsSection::Reader => self.reader(cx),
                SettingsSection::Files => self.files(cx),
            }
        } else {
            panel(cx).child(
                loading::region("settings-loading", "Cargando configuración")
                    .child(loading::line("section", px(124.)))
                    .child(loading::block("first-field", px(38.)))
                    .child(loading::block("second-field", px(38.))),
            )
        };
        let error = self
            .preferences
            .read(cx)
            .last_error()
            .map(ToOwned::to_owned);

        v_flex()
            .id("settings-scroll")
            .relative()
            .flex_1()
            .min_h_0()
            .w_full()
            .overflow_y_scroll()
            .items_center()
            .p(padding)
            .child(
                v_flex()
                    .w_full()
                    .max_w(px(860.))
                    .flex_shrink_0()
                    .gap(Spacing::MD)
                    .child(
                        Label::new("Configuración")
                            .text_size(if compact {
                                Typography::XL
                            } else {
                                Typography::PAGE_TITLE
                            })
                            .font_bold(),
                    )
                    .child(
                        TabBar::new("settings-sections")
                            .segmented()
                            .small()
                            .children(
                                SettingsSection::ALL
                                    .map(|section| Tab::new().label(section.label())),
                            )
                            .selected_index(self.section as usize)
                            .on_click(cx.listener(|view, index: &usize, _, cx| {
                                if let Some(&section) = SettingsSection::ALL.get(*index) {
                                    view.section = section;
                                    cx.notify();
                                }
                            })),
                    )
                    .when_some(self.feedback.clone(), |view, (tone, message)| {
                        view.child(
                            match tone {
                                StatusTone::Success => Alert::success("settings-feedback", message),
                                StatusTone::Error => Alert::error("settings-feedback", message),
                                _ => Alert::info("settings-feedback", message),
                            }
                            .small(),
                        )
                    })
                    .when_some(error, |view, error| {
                        view.child(Alert::error("settings-persist-error", error).small())
                    })
                    .child(section),
            )
    }
}

fn panel(cx: &App) -> Div {
    let palette = cx.dylan_theme().palette;
    v_flex()
        .w_full()
        .gap(Spacing::MD)
        .p(Spacing::MD)
        .rounded(Radii::LG)
        .bg(palette.surface)
        .border_1()
        .border_color(palette.border)
}

fn section_title(title: &'static str) -> Label {
    Label::new(title).text_size(Typography::LG).font_semibold()
}

fn field_label(label: &'static str, cx: &App) -> Label {
    Label::new(label)
        .text_size(Typography::XS)
        .font_medium()
        .text_color(cx.dylan_theme().palette.muted_foreground)
}

fn settings_input(input: &Entity<InputState>, icon: impl Into<Icon>, cx: &App) -> Input {
    Input::new(input).small().w_full().prefix(
        Icon::new(icon)
            .small()
            .text_color(cx.dylan_theme().palette.muted_foreground),
    )
}

fn field(label: &'static str, input: &Entity<InputState>, icon: impl Into<Icon>, cx: &App) -> Div {
    v_flex()
        .gap(Spacing::XS)
        .child(field_label(label, cx))
        .child(settings_input(input, icon, cx))
}

fn path_row(label: &'static str, path: String, cx: &App) -> Div {
    let palette = cx.dylan_theme().palette;
    v_flex()
        .gap(Spacing::XXS)
        .p(Spacing::SM)
        .rounded(Radii::MD)
        .bg(palette.surface_subtle)
        .child(field_label(label, cx))
        .child(Label::new(path).text_size(Typography::SM))
}
