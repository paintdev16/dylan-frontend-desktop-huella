mod view;

use gpui_kit::{
    component::input::{InputEvent, InputState},
    *,
};

use crate::{
    components::status_badge::StatusTone,
    services::preferences::{AppPreferences, PreferencesEvent, PreferencesStore},
    state::reader::ReaderState,
    theme::ThemeStore,
};

#[cfg(target_os = "windows")]
use crate::services::biometric::sdk_paths::detect_sdk_paths;

#[derive(Clone, Copy)]
enum SettingsSection {
    Station,
    Reader,
    Files,
}

impl SettingsSection {
    const ALL: [Self; 3] = [Self::Station, Self::Reader, Self::Files];

    const fn label(self) -> &'static str {
        match self {
            Self::Station => "Estación",
            Self::Reader => "Lector",
            Self::Files => "Archivos",
        }
    }
}

/// Módulo de **Configuración** (Etapas 4 y 5): gestiona correo recordado (`Recordarme`),
/// parámetros locales de estación y rutas del SDK biométrico SecuGen fuera de Velopack.
pub struct SettingsView {
    preferences: Entity<PreferencesStore>,
    reader: Entity<ReaderState>,
    theme: Entity<ThemeStore>,
    section: SettingsSection,
    station_input: Entity<InputState>,
    api_url_input: Entity<InputState>,
    remembered_email_input: Entity<InputState>,
    fdx_dll_input: Entity<InputState>,
    secusearch_dll_input: Entity<InputState>,
    #[cfg(target_os = "windows")]
    biometric_folder_task: Option<Task<()>>,
    feedback: Option<(StatusTone, SharedString)>,
    _subscriptions: Vec<Subscription>,
}

impl SettingsView {
    pub fn new(
        preferences: Entity<PreferencesStore>,
        reader: Entity<ReaderState>,
        theme: Entity<ThemeStore>,
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
        let observe_reader = cx.observe(&reader, |_, _, cx| cx.notify());
        let observe_theme = cx.observe(&theme, |_, _, cx| cx.notify());
        let mut subscriptions = vec![sub_loaded, observe_prefs, observe_reader, observe_theme];
        for input in [
            &station_input,
            &api_url_input,
            &remembered_email_input,
            &fdx_dll_input,
            &secusearch_dll_input,
        ] {
            subscriptions.push(cx.subscribe(input, |view, _, event, cx| {
                if matches!(event, InputEvent::Change) && view.feedback.take().is_some() {
                    cx.notify();
                }
            }));
        }

        Self {
            preferences,
            reader,
            theme,
            section: SettingsSection::Station,
            station_input,
            api_url_input,
            remembered_email_input,
            fdx_dll_input,
            secusearch_dll_input,
            #[cfg(target_os = "windows")]
            biometric_folder_task: None,
            feedback: None,
            _subscriptions: subscriptions,
        }
    }

    fn sync_inputs(&mut self, prefs: &AppPreferences, window: &mut Window, cx: &mut Context<Self>) {
        self.sync_station_inputs(prefs, window, cx);
        self.fdx_dll_input.update(cx, |input, cx| {
            input.set_value(prefs.fdx_dll_path.clone(), window, cx)
        });
        self.secusearch_dll_input.update(cx, |input, cx| {
            input.set_value(prefs.secusearch_dll_path.clone(), window, cx)
        });
        cx.notify();
    }

    fn sync_station_inputs(
        &mut self,
        prefs: &AppPreferences,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
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
    }

    fn apply_biometric_paths(&mut self, cx: &mut Context<Self>) {
        let fdx = self.fdx_dll_input.read(cx).value().to_string();
        let sse = self.secusearch_dll_input.read(cx).value().to_string();
        self.save_biometric_paths(&fdx, &sse, cx);
    }

    fn save_biometric_paths(&mut self, fdx: &str, sse: &str, cx: &mut Context<Self>) {
        self.preferences
            .update(cx, |store, cx| store.update_biometric(fdx, sse, cx));
        let config = self.preferences.read(cx).preferences().biometric_config();
        self.reader
            .update(cx, |reader, cx| reader.configure(config, cx));
        self.feedback = Some((
            StatusTone::Info,
            SharedString::from("Se aplicaron las rutas de SecuGen."),
        ));
        cx.notify();
    }

    #[cfg(target_os = "windows")]
    fn select_biometric_folder(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.biometric_folder_task.is_some() {
            return;
        }
        let picker = cx.prompt_for_paths(PathPromptOptions {
            files: false,
            directories: true,
            multiple: false,
            prompt: Some("Seleccionar carpeta de SecuGen".into()),
        });
        let executor = cx.background_executor().clone();
        self.biometric_folder_task = Some(cx.spawn_in(window, async move |this, cx| {
            let result = match picker.await {
                Ok(Ok(paths)) => match paths.and_then(|paths| paths.into_iter().next()) {
                    Some(folder) => executor
                        .spawn(
                            async move { detect_sdk_paths(&folder).map_err(|err| err.message()) },
                        )
                        .await
                        .map(Some),
                    None => Ok(None),
                },
                Ok(Err(err)) => Err(format!(
                    "No se pudo seleccionar la carpeta de SecuGen: {err}"
                )),
                Err(_) => Err("Se cerró el selector de carpetas de SecuGen.".into()),
            };
            let _ = this.update_in(cx, |view, window, cx| {
                view.biometric_folder_task = None;
                match result {
                    Ok(Some(paths)) => {
                        let fdx = paths.fdx.display().to_string();
                        let sse = paths.secusearch.display().to_string();
                        view.fdx_dll_input
                            .update(cx, |input, cx| input.set_value(fdx.clone(), window, cx));
                        view.secusearch_dll_input
                            .update(cx, |input, cx| input.set_value(sse.clone(), window, cx));
                        view.save_biometric_paths(&fdx, &sse, cx);
                    }
                    Ok(None) => {}
                    Err(err) => view.feedback = Some((StatusTone::Error, err.into())),
                }
                cx.notify();
            });
        }));
        self.feedback = None;
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
                self.sync_station_inputs(&updated, window, cx);
                self.feedback = Some((
                    StatusTone::Success,
                    SharedString::from("Se aplicaron los ajustes de estación y acceso."),
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
            .update(cx, |input, cx| input.set_value("", window, cx));
        self.feedback = Some((
            StatusTone::Info,
            SharedString::from("Se eliminó el correo guardado para Recordarme."),
        ));
        cx.notify();
    }

    fn reset_defaults(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        #[cfg(target_os = "windows")]
        {
            self.biometric_folder_task = None;
        }
        self.preferences
            .update(cx, |store, cx| store.reset_defaults(cx));
        let defaults = self.preferences.read(cx).preferences().clone();
        self.sync_inputs(&defaults, window, cx);
        if self.theme.read(cx).preference() != defaults.theme {
            self.theme.update(cx, |theme, cx| {
                theme.set_preference(defaults.theme, window, cx)
            });
        }
        self.reader.update(cx, |reader, cx| {
            reader.configure(defaults.biometric_config(), cx);
        });
        self.feedback = Some((
            StatusTone::Info,
            SharedString::from("Se restablecieron las preferencias por defecto."),
        ));
        cx.notify();
    }
}
