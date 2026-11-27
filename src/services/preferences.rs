use std::{
    env, fs,
    path::{Path, PathBuf},
};

use gpui_kit::{Context, EventEmitter};

use crate::{
    services::biometric::{BiometricConfig, storage::default_backup_path},
    theme::ThemePreference,
};

pub const DEFAULT_STATION_NAME: &str = "Caja principal";
pub const DEFAULT_API_BASE_URL: &str = "http://localhost:8000";

/// Preferencias locales **no sensibles** persistidas fuera del directorio de instalación
/// que reemplaza Velopack al actualizar.
///
/// Nunca almacena contraseñas, tokens de sesión, claves de firma ni datos biométricos.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AppPreferences {
    pub theme: ThemePreference,
    pub remembered_email: Option<String>,
    pub station_name: String,
    pub api_base_url: String,
    pub fdx_dll_path: String,
    pub secusearch_dll_path: String,
}

impl Default for AppPreferences {
    fn default() -> Self {
        Self {
            theme: ThemePreference::System,
            remembered_email: None,
            station_name: DEFAULT_STATION_NAME.to_string(),
            api_base_url: DEFAULT_API_BASE_URL.to_string(),
            fdx_dll_path: String::new(),
            secusearch_dll_path: String::new(),
        }
    }
}

impl AppPreferences {
    pub fn biometric_config(&self) -> BiometricConfig {
        let fdx = self.fdx_dll_path.trim();
        let sse = self.secusearch_dll_path.trim();
        BiometricConfig {
            fdx_dll_path: (!fdx.is_empty()).then(|| PathBuf::from(fdx)),
            secusearch_dll_path: (!sse.is_empty()).then(|| PathBuf::from(sse)),
            backup_path: default_backup_path(),
        }
    }
}

/// Resuelve la ruta del archivo de preferencias en el directorio de datos de usuario del SO
/// (fuera del directorio de instalación que sobrescribe Velopack y fuera de la AppImage).
pub fn default_preferences_path() -> PathBuf {
    #[cfg(target_os = "windows")]
    {
        if let Some(app_data) = env::var_os("APPDATA").filter(|v| !v.is_empty()) {
            return PathBuf::from(app_data)
                .join("DylanComedor")
                .join("preferences.json");
        }
        if let Some(local_data) = env::var_os("LOCALAPPDATA").filter(|v| !v.is_empty()) {
            return PathBuf::from(local_data)
                .join("DylanComedorData")
                .join("preferences.json");
        }
        if let Some(profile) = env::var_os("USERPROFILE").filter(|v| !v.is_empty()) {
            return PathBuf::from(profile)
                .join(".dylan-comedor")
                .join("preferences.json");
        }
        PathBuf::from("dylan-comedor-preferences.json")
    }

    #[cfg(target_os = "macos")]
    {
        if let Some(home) = env::var_os("HOME").filter(|v| !v.is_empty()) {
            return PathBuf::from(home)
                .join("Library")
                .join("Application Support")
                .join("DylanComedor")
                .join("preferences.json");
        }
        PathBuf::from("dylan-comedor-preferences.json")
    }

    #[cfg(not(any(target_os = "windows", target_os = "macos")))]
    {
        if let Some(xdg) = env::var_os("XDG_CONFIG_HOME").filter(|v| !v.is_empty()) {
            return PathBuf::from(xdg)
                .join("dylan-comedor")
                .join("preferences.json");
        }
        if let Some(home) = env::var_os("HOME").filter(|v| !v.is_empty()) {
            return PathBuf::from(home)
                .join(".config")
                .join("dylan-comedor")
                .join("preferences.json");
        }
        PathBuf::from("dylan-comedor-preferences.json")
    }
}

/// Valida y normaliza la URL base del servidor configurada en la estación.
pub fn validate_api_base_url(raw: &str) -> Result<String, &'static str> {
    let trimmed = raw.trim().trim_end_matches('/');
    if trimmed.is_empty() {
        return Ok(DEFAULT_API_BASE_URL.to_string());
    }
    let rest = trimmed
        .strip_prefix("https://")
        .or_else(|| trimmed.strip_prefix("http://"))
        .ok_or("La URL base debe comenzar con http:// o https://.")?;
    if rest.is_empty() || rest.chars().any(char::is_whitespace) {
        return Err("Ingresa una URL base válida sin espacios (ej. https://api.dylan.pe).");
    }
    Ok(trimmed.to_string())
}

pub fn normalize_station_name(raw: &str) -> String {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        DEFAULT_STATION_NAME.to_string()
    } else {
        trimmed.to_string()
    }
}

pub fn normalize_remembered_email(raw: &str) -> Option<String> {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        None
    } else {
        Some(trimmed.to_string())
    }
}

pub fn load_preferences_from(path: &Path) -> AppPreferences {
    let Ok(content) = fs::read_to_string(path) else {
        return AppPreferences::default();
    };
    parse_preferences(&content)
}

pub fn save_preferences_to(path: &Path, prefs: &AppPreferences) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .map_err(|err| format!("No se pudo crear el directorio de preferencias: {err}"))?;
    }
    let payload = serialize_preferences(prefs);
    let temp_path = path.with_extension("json.tmp");
    if fs::write(&temp_path, &payload).is_ok() {
        if fs::rename(&temp_path, path).is_ok() {
            return Ok(());
        }
        let _ = fs::remove_file(&temp_path);
    }
    fs::write(path, payload).map_err(|err| format!("No se pudo guardar las preferencias: {err}"))
}

pub fn serialize_preferences(prefs: &AppPreferences) -> String {
    let theme = match prefs.theme {
        ThemePreference::System => "system",
        ThemePreference::Light => "light",
        ThemePreference::Dark => "dark",
    };
    let remembered = match &prefs.remembered_email {
        Some(email) => format!("\"{}\"", escape_json_str(email)),
        None => "null".to_string(),
    };
    format!(
        "{{\n  \"theme\": \"{theme}\",\n  \"remembered_email\": {remembered},\n  \"station_name\": \"{}\",\n  \"api_base_url\": \"{}\",\n  \"fdx_dll_path\": \"{}\",\n  \"secusearch_dll_path\": \"{}\"\n}}\n",
        escape_json_str(&prefs.station_name),
        escape_json_str(&prefs.api_base_url),
        escape_json_str(&prefs.fdx_dll_path),
        escape_json_str(&prefs.secusearch_dll_path),
    )
}

pub fn parse_preferences(raw: &str) -> AppPreferences {
    let mut prefs = AppPreferences::default();
    for line in raw.lines() {
        let trimmed = line.trim().trim_end_matches(',');
        let Some((key_raw, val_raw)) = trimmed.split_once(':') else {
            continue;
        };
        let Some(key) = unquote_json_str(key_raw.trim()) else {
            continue;
        };
        let val = val_raw.trim();

        match key.as_str() {
            "theme" => {
                if let Some(mode) = unquote_json_str(val) {
                    prefs.theme = match mode.as_str() {
                        "light" => ThemePreference::Light,
                        "dark" => ThemePreference::Dark,
                        _ => ThemePreference::System,
                    };
                }
            }
            "remembered_email" => {
                if val == "null" {
                    prefs.remembered_email = None;
                } else if let Some(email) = unquote_json_str(val) {
                    prefs.remembered_email = normalize_remembered_email(&email);
                }
            }
            "station_name" => {
                if let Some(name) = unquote_json_str(val) {
                    prefs.station_name = normalize_station_name(&name);
                }
            }
            "api_base_url" => {
                if let Some(url) = unquote_json_str(val)
                    && let Ok(valid) = validate_api_base_url(&url)
                {
                    prefs.api_base_url = valid;
                }
            }
            "fdx_dll_path" => {
                if let Some(path) = unquote_json_str(val) {
                    prefs.fdx_dll_path = path.trim().to_string();
                }
            }
            "secusearch_dll_path" => {
                if let Some(path) = unquote_json_str(val) {
                    prefs.secusearch_dll_path = path.trim().to_string();
                }
            }
            _ => {}
        }
    }
    prefs
}

fn escape_json_str(input: &str) -> String {
    let mut out = String::with_capacity(input.len());
    for ch in input.chars() {
        match ch {
            '\\' => out.push_str("\\\\"),
            '"' => out.push_str("\\\""),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c if c.is_control() => {}
            c => out.push(c),
        }
    }
    out
}

fn unquote_json_str(input: &str) -> Option<String> {
    let inner = input.strip_prefix('"')?.strip_suffix('"')?;
    let mut out = String::with_capacity(inner.len());
    let mut chars = inner.chars();
    while let Some(ch) = chars.next() {
        if ch == '\\' {
            match chars.next()? {
                '\\' => out.push('\\'),
                '"' => out.push('"'),
                'n' => out.push('\n'),
                'r' => out.push('\r'),
                't' => out.push('\t'),
                other => out.push(other),
            }
        } else {
            out.push(ch);
        }
    }
    Some(out)
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PreferencesEvent {
    Loaded(AppPreferences),
}

/// Entidad reactiva de preferencias que carga y persiste fuera del hilo de GPUI.
pub struct PreferencesStore {
    path: PathBuf,
    preferences: AppPreferences,
    loaded: bool,
    last_error: Option<String>,
}

impl EventEmitter<PreferencesEvent> for PreferencesStore {}

impl PreferencesStore {
    pub fn new(cx: &mut Context<Self>) -> Self {
        let path = default_preferences_path();
        let path_for_load = path.clone();

        let background = cx
            .background_executor()
            .spawn(async move { load_preferences_from(&path_for_load) });

        cx.spawn(async move |this, cx| {
            let loaded = background.await;
            let _ = this.update(cx, |store, cx| {
                store.preferences = loaded.clone();
                store.loaded = true;
                cx.emit(PreferencesEvent::Loaded(loaded));
                cx.notify();
            });
        })
        .detach();

        Self {
            path,
            preferences: AppPreferences::default(),
            loaded: false,
            last_error: None,
        }
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    pub fn preferences(&self) -> &AppPreferences {
        &self.preferences
    }

    pub fn is_loaded(&self) -> bool {
        self.loaded
    }

    pub fn last_error(&self) -> Option<&str> {
        self.last_error.as_deref()
    }

    pub fn set_theme(&mut self, theme: ThemePreference, cx: &mut Context<Self>) {
        if self.preferences.theme != theme {
            self.preferences.theme = theme;
            self.persist_in_background(cx);
            cx.notify();
        }
    }

    pub fn set_remembered_email(&mut self, email: Option<String>, cx: &mut Context<Self>) {
        let normalized = email.and_then(|value| normalize_remembered_email(&value));
        if self.preferences.remembered_email != normalized {
            self.preferences.remembered_email = normalized;
            self.persist_in_background(cx);
            cx.notify();
        }
    }

    pub fn set_api_base_url(
        &mut self,
        api_base_url: &str,
        cx: &mut Context<Self>,
    ) -> Result<String, &'static str> {
        let valid_url = validate_api_base_url(api_base_url)?;
        if self.preferences.api_base_url != valid_url {
            self.preferences.api_base_url = valid_url.clone();
            self.last_error = None;
            self.persist_in_background(cx);
            cx.notify();
        }
        Ok(valid_url)
    }

    pub fn update_station(
        &mut self,
        station_name: &str,
        api_base_url: &str,
        remembered_email: Option<&str>,
        cx: &mut Context<Self>,
    ) -> Result<(), &'static str> {
        let valid_url = validate_api_base_url(api_base_url)?;
        self.preferences.station_name = normalize_station_name(station_name);
        self.preferences.api_base_url = valid_url;
        if let Some(email) = remembered_email {
            self.preferences.remembered_email = normalize_remembered_email(email);
        }
        self.last_error = None;
        self.persist_in_background(cx);
        cx.notify();
        Ok(())
    }

    pub fn update_biometric(
        &mut self,
        fdx_dll_path: &str,
        secusearch_dll_path: &str,
        cx: &mut Context<Self>,
    ) {
        self.preferences.fdx_dll_path = fdx_dll_path.trim().to_string();
        self.preferences.secusearch_dll_path = secusearch_dll_path.trim().to_string();
        self.last_error = None;
        self.persist_in_background(cx);
        cx.notify();
    }

    pub fn reset_defaults(&mut self, cx: &mut Context<Self>) {
        self.preferences = AppPreferences::default();
        self.last_error = None;
        self.persist_in_background(cx);
        cx.notify();
    }

    fn persist_in_background(&self, cx: &mut Context<Self>) {
        let path = self.path.clone();
        let snapshot = self.preferences.clone();
        let background = cx
            .background_executor()
            .spawn(async move { save_preferences_to(&path, &snapshot) });

        cx.spawn(async move |this, cx| {
            let result = background.await;
            let _ = this.update(cx, |store, cx| {
                store.last_error = result.err();
                cx.notify();
            });
        })
        .detach();
    }
}

#[cfg(test)]
mod tests {
    use std::{env, fs};

    use super::{
        AppPreferences, DEFAULT_API_BASE_URL, load_preferences_from, parse_preferences,
        save_preferences_to, serialize_preferences, validate_api_base_url,
    };
    use crate::theme::ThemePreference;

    #[test]
    fn round_trips_preferences_on_disk_and_never_serializes_secrets() {
        let temp_dir = env::temp_dir().join("dylan-comedor-prefs-test");
        let path = temp_dir.join("preferences.json");
        let _ = fs::remove_file(&path);

        let prefs = AppPreferences {
            theme: ThemePreference::Dark,
            remembered_email: Some("admin@dylan.pe".into()),
            station_name: "Estación Comedor \"Norte\"".into(),
            api_base_url: "https://api.dylan.pe".into(),
            fdx_dll_path: "C:\\SecuGen\\sgfplib.dll".into(),
            secusearch_dll_path: "C:\\SecuGen\\secusearchapi.dll".into(),
        };

        let serialized = serialize_preferences(&prefs);
        assert!(!serialized.contains("password"));
        assert!(!serialized.contains("token"));
        assert!(!serialized.contains("signing"));

        save_preferences_to(&path, &prefs).expect("save preferences");
        let loaded = load_preferences_from(&path);
        assert_eq!(loaded, prefs);

        let _ = fs::remove_file(&path);
        let _ = fs::remove_dir(&temp_dir);
    }

    #[test]
    fn reads_sdk_paths_from_preferences_with_a_legacy_mode() {
        let parsed = parse_preferences(
            r#"{
            "biometric_mode": "simulated",
            "fdx_dll_path": "C:/SecuGen/sgfplib.dll",
            "secusearch_dll_path": "C:/SecuGen/secusearchapi.dll"
        }"#,
        );

        assert_eq!(parsed.fdx_dll_path, "C:/SecuGen/sgfplib.dll");
        assert_eq!(parsed.secusearch_dll_path, "C:/SecuGen/secusearchapi.dll");
        assert!(!serialize_preferences(&parsed).contains("biometric_mode"));
    }

    #[test]
    fn falls_back_to_defaults_on_corrupt_content_and_validates_urls() {
        let parsed = parse_preferences("esto no es json valido");
        assert_eq!(parsed, AppPreferences::default());

        assert_eq!(
            validate_api_base_url("  "),
            Ok(DEFAULT_API_BASE_URL.to_string())
        );
        assert_eq!(
            validate_api_base_url("https://comedor.dylan.pe/"),
            Ok("https://comedor.dylan.pe".to_string())
        );
        assert!(validate_api_base_url("ftp://invalido.pe").is_err());
        assert!(validate_api_base_url("http://con espacio.pe").is_err());
    }
}
