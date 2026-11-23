pub mod auth;

use std::{collections::BTreeMap, sync::OnceLock, time::Duration};

use reqwest::blocking::Client;
use serde::{Deserialize, Serialize};

use crate::services::preferences::validate_api_base_url;

const CONNECT_TIMEOUT: Duration = Duration::from_secs(5);
const REQUEST_TIMEOUT: Duration = Duration::from_secs(12);

/// Respuesta HTTP cruda devuelta por el cliente base.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HttpResponse {
    pub status: u16,
    pub body: String,
}

/// Errores tipados del cliente HTTP para diferenciar credenciales, permisos,
/// sesión expirada, límite de intentos y problemas de red/servidor.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ApiError {
    EmptyFields,
    InvalidBaseUrl(String),
    InvalidCredentials(String),
    Unauthorized(String),
    Forbidden(String),
    RateLimited(String),
    Network(String),
    Server(u16, String),
    Decode(String),
}

impl ApiError {
    pub fn user_message(&self) -> String {
        match self {
            Self::EmptyFields => "Ingresa tu correo electrónico y contraseña.".to_string(),
            Self::InvalidBaseUrl(msg) => msg.clone(),
            Self::InvalidCredentials(msg) => msg.clone(),
            Self::Unauthorized(msg) => msg.clone(),
            Self::Forbidden(msg) => msg.clone(),
            Self::RateLimited(msg) => msg.clone(),
            Self::Network(msg) => msg.clone(),
            Self::Server(code, msg) => {
                if msg.is_empty() {
                    format!("El servidor respondió con un error inesperado (HTTP {code}).")
                } else {
                    format!("{msg} (HTTP {code})")
                }
            }
            Self::Decode(msg) => msg.clone(),
        }
    }

    pub const fn is_unauthorized(&self) -> bool {
        matches!(self, Self::Unauthorized(_))
    }

    pub const fn is_warning_tone(&self) -> bool {
        matches!(
            self,
            Self::Forbidden(_) | Self::RateLimited(_) | Self::Unauthorized(_)
        )
    }
}

/// Contrato mínimo de transporte HTTP para permitir pruebas unitarias determinísticas
/// sin abrir sockets ni depender de red externa.
pub trait HttpTransport {
    fn get(&self, url: &str, bearer_token: Option<&str>) -> Result<HttpResponse, ApiError>;
    fn post_json(
        &self,
        url: &str,
        bearer_token: Option<&str>,
        json_body: &str,
    ) -> Result<HttpResponse, ApiError>;
    fn post_empty(&self, url: &str, bearer_token: Option<&str>) -> Result<HttpResponse, ApiError>;
}

fn shared_reqwest_client() -> Result<&'static Client, ApiError> {
    static CLIENT: OnceLock<Result<Client, String>> = OnceLock::new();
    match CLIENT.get_or_init(|| {
        Client::builder()
            .connect_timeout(CONNECT_TIMEOUT)
            .timeout(REQUEST_TIMEOUT)
            .build()
            .map_err(|err| format!("No se pudo inicializar el cliente HTTP reqwest: {err}"))
    }) {
        Ok(client) => Ok(client),
        Err(msg) => Err(ApiError::Network(msg.clone())),
    }
}

/// Cliente HTTP base basado en `reqwest` para conversar con el backend Laravel del comedor.
///
/// Todas sus llamadas son bloqueantes y deben ejecutarse fuera del hilo de UI de GPUI
/// (por ejemplo con `cx.background_executor().spawn(...)`).
#[derive(Clone)]
pub struct ApiClient {
    base_url: String,
}

impl ApiClient {
    pub fn new(base_url: &str) -> Result<Self, ApiError> {
        let normalized = validate_api_base_url(base_url)
            .map_err(|msg| ApiError::InvalidBaseUrl(msg.to_string()))?;
        Ok(Self {
            base_url: normalized,
        })
    }

    /// Construye la URL absoluta evitando duplicar `/api` si la URL base ya termina en `/api`.
    pub fn url(&self, path: &str) -> String {
        let root = self
            .base_url
            .strip_suffix("/api")
            .unwrap_or(&self.base_url)
            .trim_end_matches('/');
        let clean_path = if path.starts_with('/') {
            path.to_string()
        } else {
            format!("/{path}")
        };
        format!("{root}{clean_path}")
    }

    pub fn serialize_json<T: Serialize>(payload: &T) -> Result<String, ApiError> {
        serde_json::to_string(payload).map_err(|err| {
            ApiError::Decode(format!("No se pudo serializar la solicitud JSON: {err}"))
        })
    }
}

impl HttpTransport for ApiClient {
    fn get(&self, url: &str, bearer_token: Option<&str>) -> Result<HttpResponse, ApiError> {
        let client = shared_reqwest_client()?;
        let mut req = client.get(url).header("Accept", "application/json");
        if let Some(token) = bearer_token {
            req = req.bearer_auth(token);
        }
        let response = req
            .send()
            .map_err(|err| map_transport_error(&self.base_url, err))?;
        read_response(response)
    }

    fn post_json(
        &self,
        url: &str,
        bearer_token: Option<&str>,
        json_body: &str,
    ) -> Result<HttpResponse, ApiError> {
        let client = shared_reqwest_client()?;
        let mut req = client
            .post(url)
            .header("Accept", "application/json")
            .header("Content-Type", "application/json")
            .body(json_body.to_string());
        if let Some(token) = bearer_token {
            req = req.bearer_auth(token);
        }
        let response = req
            .send()
            .map_err(|err| map_transport_error(&self.base_url, err))?;
        read_response(response)
    }

    fn post_empty(&self, url: &str, bearer_token: Option<&str>) -> Result<HttpResponse, ApiError> {
        let client = shared_reqwest_client()?;
        let mut req = client.post(url).header("Accept", "application/json");
        if let Some(token) = bearer_token {
            req = req.bearer_auth(token);
        }
        let response = req
            .send()
            .map_err(|err| map_transport_error(&self.base_url, err))?;
        read_response(response)
    }
}

fn read_response(response: reqwest::blocking::Response) -> Result<HttpResponse, ApiError> {
    let status = response.status().as_u16();
    let body = response
        .text()
        .map_err(|err| ApiError::Network(format!("Error al leer respuesta del servidor: {err}")))?;
    Ok(HttpResponse { status, body })
}

fn map_transport_error(base_url: &str, err: reqwest::Error) -> ApiError {
    if err.is_timeout() {
        ApiError::Network(format!(
            "El servidor ({base_url}) tardó demasiado en responder. Verifica la conexión e intenta nuevamente."
        ))
    } else if err.is_connect() {
        ApiError::Network(format!(
            "No se pudo conectar con {base_url}. Verifica que el backend esté en ejecución."
        ))
    } else {
        ApiError::Network(format!("Error de conexión con {base_url}: {err}"))
    }
}

#[derive(Debug, Deserialize)]
struct LaravelErrorBody {
    #[serde(default)]
    message: Option<String>,
    #[serde(default)]
    errors: Option<BTreeMap<String, Vec<String>>>,
}

/// Extrae el mensaje más claro de una respuesta de error JSON de Laravel.
pub fn parse_laravel_error_message(body: &str) -> Option<String> {
    let parsed: LaravelErrorBody = serde_json::from_str(body).ok()?;
    if let Some(errors) = parsed.errors {
        for messages in errors.values() {
            if let Some(first) = messages.iter().find(|msg| !msg.trim().is_empty()) {
                return Some(first.trim().to_string());
            }
        }
    }
    parsed
        .message
        .map(|msg| msg.trim().to_string())
        .filter(|msg| !msg.is_empty())
}

/// Ejecuta una operación autenticada siguiendo el contrato `401 -> refresh -> reintento o expiración`.
///
/// En Laravel Sanctum no existe endpoint `/api/refresh`, por lo que el callback `refresh`
/// devuelve `Ok(None)` y la sesión expira ordenadamente devolviendo `ApiError::Unauthorized`.
/// Si en el futuro el backend habilita renovación de token, la función reintenta una sola vez
/// con el nuevo token y lo retorna al llamador para actualizar `SessionState`.
pub fn execute_with_auth_retry<T, F, R>(
    initial_token: &str,
    mut request: F,
    mut refresh: R,
) -> Result<(T, Option<String>), ApiError>
where
    F: FnMut(&str) -> Result<T, ApiError>,
    R: FnMut() -> Result<Option<String>, ApiError>,
{
    match request(initial_token) {
        Ok(value) => Ok((value, None)),
        Err(ApiError::Unauthorized(_)) => match refresh() {
            Ok(Some(new_token)) if !new_token.trim().is_empty() => {
                let retried = request(&new_token)?;
                Ok((retried, Some(new_token)))
            }
            _ => Err(ApiError::Unauthorized(
                "Tu sesión expiró o fue revocada. Inicia sesión nuevamente.".to_string(),
            )),
        },
        Err(err) => Err(err),
    }
}

#[cfg(test)]
mod tests {
    use super::{ApiClient, ApiError, execute_with_auth_retry, parse_laravel_error_message};

    #[test]
    fn normalizes_api_urls_without_duplicating_prefix() {
        let plain = ApiClient::new("http://localhost:8000/").expect("valid url");
        assert_eq!(plain.url("/api/login"), "http://localhost:8000/api/login");

        let with_api = ApiClient::new("http://localhost:8000/api").expect("valid url");
        assert_eq!(with_api.url("/api/user"), "http://localhost:8000/api/user");
    }

    #[test]
    fn extracts_first_laravel_validation_error_or_message() {
        let validation = r#"{"message":"The given data was invalid.","errors":{"email":["Las credenciales proporcionadas son incorrectas."]}}"#;
        assert_eq!(
            parse_laravel_error_message(validation).as_deref(),
            Some("Las credenciales proporcionadas son incorrectas.")
        );

        let generic = r#"{"message":"Unauthenticated."}"#;
        assert_eq!(
            parse_laravel_error_message(generic).as_deref(),
            Some("Unauthenticated.")
        );
    }

    #[test]
    fn retries_once_after_401_when_refresh_provides_token_and_expires_otherwise() {
        let mut attempts = 0;
        let (value, rotated) = execute_with_auth_retry(
            "expired-token",
            |token| {
                attempts += 1;
                if token == "fresh-token" {
                    Ok("ok")
                } else {
                    Err(ApiError::Unauthorized("expired".into()))
                }
            },
            || Ok(Some("fresh-token".to_string())),
        )
        .expect("refresh retry should succeed");

        assert_eq!(attempts, 2);
        assert_eq!(value, "ok");
        assert_eq!(rotated.as_deref(), Some("fresh-token"));

        let expired = execute_with_auth_retry(
            "expired-token",
            |_| Err::<&str, _>(ApiError::Unauthorized("expired".into())),
            || Ok(None),
        );
        assert!(matches!(expired, Err(ApiError::Unauthorized(_))));
    }
}
