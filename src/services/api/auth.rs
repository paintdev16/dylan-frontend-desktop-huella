use serde::{Deserialize, Deserializer, Serialize};

use crate::{
    services::api::{
        ApiClient, ApiError, HttpTransport, execute_with_auth_retry, parse_laravel_error_message,
    },
    state::session::SessionPermissions,
};

use super::dining::DiningVersions;

#[derive(Debug, Serialize)]
struct LoginRequest<'a> {
    email: &'a str,
    password: &'a str,
    device_name: &'a str,
}

#[derive(Debug, Deserialize)]
#[serde(untagged)]
enum RoleEntry {
    Name(String),
    Object { name: String },
}

fn deserialize_roles<'de, D>(deserializer: D) -> Result<Vec<String>, D::Error>
where
    D: Deserializer<'de>,
{
    let entries = Option::<Vec<RoleEntry>>::deserialize(deserializer)?.unwrap_or_default();
    Ok(entries
        .into_iter()
        .map(|entry| match entry {
            RoleEntry::Name(name) => name,
            RoleEntry::Object { name } => name,
        })
        .collect())
}

/// Usuario devuelto por `App\Http\Resources\Api\Auth\UserResource` en
/// `dylan-starter-kit-laravel-react`.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct ApiUser {
    pub id: u64,
    pub name: String,
    pub email: String,
    #[serde(default)]
    pub email_verified_at: Option<String>,
    /// Lista de nombres de roles Spatie (`["super-admin"]`, `["admin"]`, `["client"]`, etc.).
    #[serde(default, deserialize_with = "deserialize_roles")]
    pub roles: Vec<String>,
    /// Lista completa de permisos efectivos Spatie (`$user->getAllPermissions()->pluck('name')`).
    #[serde(default)]
    pub permissions: Vec<String>,
    #[serde(default)]
    pub created_at: Option<String>,
    #[serde(default)]
    pub updated_at: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
struct LoginResponse {
    #[serde(default)]
    message: String,
    token: String,
    #[serde(default)]
    token_type: String,
    user: ApiUser,
    #[serde(default)]
    dining_versions: Option<DiningVersions>,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
struct MeResponse {
    user: ApiUser,
}

/// Sesión devuelta por `POST /api/auth/login` del backend Laravel Sanctum.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AuthenticatedSession {
    pub token: String,
    pub token_type: String,
    pub user: ApiUser,
    pub dining_versions: Option<DiningVersions>,
}

impl ApiUser {
    pub fn role_names(&self) -> Vec<String> {
        self.roles.clone()
    }

    pub fn permission_names(&self) -> Vec<String> {
        self.permissions.clone()
    }

    pub fn has_role(&self, expected: &str) -> bool {
        self.roles
            .iter()
            .any(|role| role.eq_ignore_ascii_case(expected))
    }

    pub fn has_permission(&self, expected: &str) -> bool {
        self.permissions
            .iter()
            .any(|permission| permission.eq_ignore_ascii_case(expected))
    }

    pub fn is_super_admin(&self) -> bool {
        self.has_role("super-admin")
    }

    pub fn can_manage_access(&self) -> bool {
        self.is_super_admin()
            || self.has_role("admin")
            || self.has_permission("manage.access")
            || self.has_permission("manage access")
    }

    pub fn primary_role_label(&self) -> String {
        if self.is_super_admin() {
            return "Super Admin".to_string();
        }
        if self.has_role("admin") {
            return "Administrador".to_string();
        }
        if let Some(first) = self.roles.first() {
            return match first.to_ascii_lowercase().as_str() {
                "client" => "Cliente".to_string(),
                "cajero" => "Cajero".to_string(),
                _ => first.clone(),
            };
        }
        "Operador".to_string()
    }
}

/// Mapea roles y permisos recibidos desde Laravel + Spatie Permission (`UserResource`)
/// a los permisos efectivos del escritorio.
///
/// - `super-admin`, `admin` o `manage.access` habilitan la operación completa del escritorio.
/// - Si el backend envía permisos granulares en `permissions` (catálogo de acceso o módulos
///   del comedor), se habilita cada sección correspondiente.
/// - Un usuario autenticado con rol básico (ej. `client` o sin permisos) solo accede al
///   Dashboard y queda bloqueado de módulos operativos, administrativos y de configuración.
pub fn map_permissions(user: &ApiUser) -> SessionPermissions {
    if has_full_access(user) {
        return SessionPermissions::full_access();
    }

    let pensioners = user.has_permission("pensioners.view")
        || user.has_permission("pensioners.manage")
        || user.has_permission("pensioners.create")
        || user.has_permission("pensioners.update");
    let identification = user.has_permission("identification.use");
    let consumptions =
        user.has_permission("consumptions.view") || user.has_permission("payments.view");
    let register_attendance = user.has_permission("attendance.register");
    let cash = user.has_permission("cash.manage") || user.has_permission("cash.view");
    let reports = user.has_permission("reports.view");
    let settings = user.has_permission("settings.manage")
        || user.has_permission("roles.view")
        || user.has_permission("permissions.view");
    let search_pensioners = user.has_permission("pensioners.view")
        || user.has_permission("pensioners.manage")
        || user.has_permission("pensioners.search")
        || register_attendance;

    SessionPermissions {
        dashboard: true,
        pensioners,
        identification,
        consumptions,
        register_attendance,
        cash,
        reports,
        settings,
        search_pensioners,
    }
}

pub fn has_full_access(user: &ApiUser) -> bool {
    user.can_manage_access()
}

/// Autentica contra `POST /api/auth/login` (Laravel Sanctum).
pub fn login(
    base_url: &str,
    email: &str,
    password: &str,
    device_name: &str,
) -> Result<AuthenticatedSession, ApiError> {
    let client = ApiClient::new(base_url)?;
    let url = client.url("/api/auth/login");
    login_with_transport(&client, &url, email, password, device_name)
}

fn login_with_transport(
    transport: &impl HttpTransport,
    url: &str,
    email: &str,
    password: &str,
    device_name: &str,
) -> Result<AuthenticatedSession, ApiError> {
    let trimmed_email = email.trim();
    if trimmed_email.is_empty() || password.is_empty() {
        return Err(ApiError::EmptyFields);
    }

    let device = if device_name.trim().is_empty() {
        "Dylan Comedor Desktop"
    } else {
        device_name.trim()
    };

    let payload = ApiClient::serialize_json(&LoginRequest {
        email: trimmed_email,
        password,
        device_name: device,
    })?;
    let response = transport.post_json(url, None, &payload)?;

    match response.status {
        200 | 201 => {
            let parsed: LoginResponse = serde_json::from_str(&response.body).map_err(|err| {
                ApiError::Decode(format!(
                    "La respuesta de inicio de sesión del servidor no tiene el formato esperado: {err}"
                ))
            })?;
            if parsed.token.trim().is_empty() {
                return Err(ApiError::Decode(
                    "El servidor no devolvió un token de acceso válido.".to_string(),
                ));
            }
            let token_type = if parsed.token_type.trim().is_empty() {
                "Bearer".to_string()
            } else {
                parsed.token_type
            };
            let _ = parsed.message;
            Ok(AuthenticatedSession {
                token: parsed.token,
                token_type,
                user: parsed.user,
                dining_versions: parsed.dining_versions,
            })
        }
        401 | 422 => {
            let detail = parse_laravel_error_message(&response.body).unwrap_or_else(|| {
                "Credenciales inválidas. Verifica tu correo electrónico y contraseña.".to_string()
            });
            Err(ApiError::InvalidCredentials(detail))
        }
        403 => {
            let detail = parse_laravel_error_message(&response.body).unwrap_or_else(|| {
                "Tu cuenta no tiene permisos para ingresar a esta estación.".to_string()
            });
            Err(ApiError::Forbidden(detail))
        }
        429 => {
            let detail = parse_laravel_error_message(&response.body).unwrap_or_else(|| {
                "Demasiados intentos de inicio de sesión. Espera un momento antes de reintentar."
                    .to_string()
            });
            Err(ApiError::RateLimited(detail))
        }
        status => {
            let detail = parse_laravel_error_message(&response.body).unwrap_or_default();
            Err(ApiError::Server(status, detail))
        }
    }
}

/// Consulta `GET /api/auth/me` para validar la vigencia del token y sincronizar roles/permisos.
pub fn fetch_me(base_url: &str, bearer_token: &str) -> Result<ApiUser, ApiError> {
    let client = ApiClient::new(base_url)?;
    let url = client.url("/api/auth/me");
    fetch_me_with_transport(&client, &url, bearer_token)
}

fn fetch_me_with_transport(
    transport: &impl HttpTransport,
    url: &str,
    bearer_token: &str,
) -> Result<ApiUser, ApiError> {
    let (user, _rotated) = execute_with_auth_retry(
        bearer_token,
        |token| fetch_me_once(transport, url, token),
        // Laravel Sanctum no expone endpoint de refresh; si el token devuelve 401 debe expirar.
        || Ok(None),
    )?;
    Ok(user)
}

fn fetch_me_once(
    transport: &impl HttpTransport,
    url: &str,
    bearer_token: &str,
) -> Result<ApiUser, ApiError> {
    let response = transport.get(url, Some(bearer_token))?;
    match response.status {
        200 => {
            let parsed: MeResponse = serde_json::from_str(&response.body).map_err(|err| {
                ApiError::Decode(format!(
                    "No se pudo interpretar el perfil del usuario devuelto por el servidor: {err}"
                ))
            })?;
            Ok(parsed.user)
        }
        401 => Err(ApiError::Unauthorized(
            "Tu sesión expiró o fue revocada. Inicia sesión nuevamente.".to_string(),
        )),
        403 => {
            let detail = parse_laravel_error_message(&response.body).unwrap_or_else(|| {
                "Tu cuenta ya no tiene permisos para esta operación.".to_string()
            });
            Err(ApiError::Forbidden(detail))
        }
        429 => Err(ApiError::RateLimited(
            "Demasiadas solicitudes al servidor. Intenta nuevamente en unos segundos.".to_string(),
        )),
        status => {
            let detail = parse_laravel_error_message(&response.body).unwrap_or_default();
            Err(ApiError::Server(status, detail))
        }
    }
}

/// Revoca el token actual en Laravel Sanctum (`POST /api/auth/logout`).
///
/// Un `401` se considera éxito porque el token ya no es válido en el servidor.
pub fn logout(base_url: &str, bearer_token: &str) -> Result<(), ApiError> {
    if bearer_token.trim().is_empty() {
        return Ok(());
    }
    let client = ApiClient::new(base_url)?;
    let url = client.url("/api/auth/logout");
    logout_with_transport(&client, &url, bearer_token)
}

fn logout_with_transport(
    transport: &impl HttpTransport,
    url: &str,
    bearer_token: &str,
) -> Result<(), ApiError> {
    if bearer_token.trim().is_empty() {
        return Ok(());
    }
    let response = transport.post_empty(url, Some(bearer_token))?;
    match response.status {
        200 | 204 | 401 => Ok(()),
        status => {
            let detail = parse_laravel_error_message(&response.body).unwrap_or_default();
            Err(ApiError::Server(status, detail))
        }
    }
}

#[cfg(test)]
mod tests {
    use std::cell::RefCell;

    use super::{
        ApiUser, fetch_me_with_transport, login_with_transport, logout_with_transport,
        map_permissions,
    };
    use crate::{
        services::api::{ApiError, HttpResponse, HttpTransport},
        state::session::AppRoute,
    };

    #[derive(Debug, Clone, PartialEq, Eq)]
    struct RecordedRequest {
        method: &'static str,
        url: String,
        bearer_token: Option<String>,
        body: Option<String>,
    }

    struct MockTransport {
        response: Result<HttpResponse, ApiError>,
        recorded: RefCell<Vec<RecordedRequest>>,
    }

    impl MockTransport {
        fn with_status(status: u16, body: &str) -> Self {
            Self {
                response: Ok(HttpResponse {
                    status,
                    body: body.to_string(),
                }),
                recorded: RefCell::new(Vec::new()),
            }
        }

        fn with_error(err: ApiError) -> Self {
            Self {
                response: Err(err),
                recorded: RefCell::new(Vec::new()),
            }
        }

        fn last_request(&self) -> RecordedRequest {
            self.recorded
                .borrow()
                .last()
                .cloned()
                .expect("at least one request should have been recorded")
        }
    }

    impl HttpTransport for MockTransport {
        fn get(&self, url: &str, bearer_token: Option<&str>) -> Result<HttpResponse, ApiError> {
            self.recorded.borrow_mut().push(RecordedRequest {
                method: "GET",
                url: url.to_string(),
                bearer_token: bearer_token.map(str::to_string),
                body: None,
            });
            self.response.clone()
        }

        fn post_json(
            &self,
            url: &str,
            bearer_token: Option<&str>,
            json_body: &str,
        ) -> Result<HttpResponse, ApiError> {
            self.recorded.borrow_mut().push(RecordedRequest {
                method: "POST_JSON",
                url: url.to_string(),
                bearer_token: bearer_token.map(str::to_string),
                body: Some(json_body.to_string()),
            });
            self.response.clone()
        }

        fn post_empty(
            &self,
            url: &str,
            bearer_token: Option<&str>,
        ) -> Result<HttpResponse, ApiError> {
            self.recorded.borrow_mut().push(RecordedRequest {
                method: "POST_EMPTY",
                url: url.to_string(),
                bearer_token: bearer_token.map(str::to_string),
                body: None,
            });
            self.response.clone()
        }
    }

    #[test]
    fn maps_super_admin_admin_granular_and_client_permissions() {
        let super_admin = ApiUser {
            id: 1,
            name: "Paint".into(),
            email: "paint@gmail.com".into(),
            email_verified_at: Some("2026-01-01T00:00:00.000000Z".into()),
            roles: vec!["super-admin".into()],
            permissions: vec!["manage.access".into()],
            created_at: None,
            updated_at: None,
        };
        let perms = map_permissions(&super_admin);
        for route in AppRoute::IMPLEMENTED {
            assert!(perms.allows_route(route));
        }
        assert!(perms.register_attendance);
        assert!(perms.search_pensioners);
        assert!(super_admin.is_super_admin());
        assert!(super_admin.can_manage_access());
        assert_eq!(super_admin.primary_role_label(), "Super Admin");

        let granular = ApiUser {
            id: 3,
            name: "Auditor".into(),
            email: "auditor@dylan.pe".into(),
            email_verified_at: None,
            roles: vec!["auditor".into()],
            permissions: vec!["pensioners.view".into(), "permissions.view".into()],
            created_at: None,
            updated_at: None,
        };
        let granular_perms = map_permissions(&granular);
        assert!(granular_perms.dashboard);
        assert!(granular_perms.pensioners);
        assert!(granular_perms.search_pensioners);
        assert!(granular_perms.settings);
        assert!(!granular_perms.cash);
        assert!(!granular_perms.register_attendance);

        let client = ApiUser {
            id: 2,
            name: "Usuario Consulta".into(),
            email: "cliente@dylan.pe".into(),
            email_verified_at: None,
            roles: vec!["client".into()],
            permissions: vec![],
            created_at: None,
            updated_at: None,
        };
        let client_perms = map_permissions(&client);
        assert!(client_perms.dashboard);
        assert!(!client_perms.pensioners);
        assert!(!client_perms.identification);
        assert!(!client_perms.consumptions);
        assert!(!client_perms.register_attendance);
        assert!(!client_perms.cash);
        assert!(!client_perms.reports);
        assert!(!client_perms.settings);
        assert!(!client_perms.search_pensioners);
        assert_eq!(client.primary_role_label(), "Cliente");
    }

    #[test]
    fn login_succeeds_against_starter_kit_auth_login_contract() {
        let body = r#"{
            "message": "Inicio de sesión exitoso.",
            "token": "1|sanctum_test_token_abc",
            "token_type": "Bearer",
            "dining_versions": {
                "dashboard": "2:3:4",
                "pensioners": 2,
                "attendances": 3,
                "cash": 4,
                "reports": 3
            },
            "user": {
                "id": 1,
                "name": "Paint",
                "email": "paint@gmail.com",
                "email_verified_at": "2026-01-01T00:00:00.000000Z",
                "roles": ["super-admin"],
                "permissions": ["manage.access"],
                "created_at": "2026-01-01T00:00:00.000000Z",
                "updated_at": "2026-01-01T00:00:00.000000Z"
            }
        }"#;
        let transport = MockTransport::with_status(200, body);

        let result = login_with_transport(
            &transport,
            "http://localhost:8000/api/auth/login",
            "  paint@gmail.com  ",
            "123456789",
            "Caja principal",
        )
        .expect("valid login must succeed");

        let req = transport.last_request();
        assert_eq!(req.method, "POST_JSON");
        assert_eq!(req.url, "http://localhost:8000/api/auth/login");
        assert!(req.bearer_token.is_none());
        let sent_body = req.body.expect("json body");
        assert!(sent_body.contains("\"email\":\"paint@gmail.com\""));
        assert!(sent_body.contains("\"device_name\":\"Caja principal\""));

        assert_eq!(result.token, "1|sanctum_test_token_abc");
        assert_eq!(result.token_type, "Bearer");
        assert_eq!(result.dining_versions.unwrap().cash, Some(4));
        assert_eq!(result.user.id, 1);
        assert_eq!(result.user.roles, vec!["super-admin".to_string()]);
        assert_eq!(result.user.permissions, vec!["manage.access".to_string()]);
        assert!(result.user.is_super_admin());
        assert!(result.user.can_manage_access());
    }

    #[test]
    fn login_rejects_empty_fields_422_401_403_429_and_network_errors() {
        let dummy = MockTransport::with_status(200, "{}");
        assert_eq!(
            login_with_transport(
                &dummy,
                "http://localhost:8000/api/auth/login",
                "",
                "secret",
                "Caja"
            ),
            Err(ApiError::EmptyFields)
        );
        assert_eq!(
            login_with_transport(
                &dummy,
                "http://localhost:8000/api/auth/login",
                "user@dylan.pe",
                "",
                "Caja"
            ),
            Err(ApiError::EmptyFields)
        );
        assert!(dummy.recorded.borrow().is_empty());

        let t422 = MockTransport::with_status(
            422,
            r#"{"message":"The given data was invalid.","errors":{"email":["Las credenciales proporcionadas no coinciden con nuestros registros."]}}"#,
        );
        assert_eq!(
            login_with_transport(
                &t422,
                "http://localhost:8000/api/auth/login",
                "paint@gmail.com",
                "mal",
                "Caja"
            ),
            Err(ApiError::InvalidCredentials(
                "Las credenciales proporcionadas no coinciden con nuestros registros.".to_string()
            ))
        );

        let t401 = MockTransport::with_status(401, r#"{"message":"Credenciales inválidas."}"#);
        assert_eq!(
            login_with_transport(
                &t401,
                "http://localhost:8000/api/auth/login",
                "paint@gmail.com",
                "mal",
                "Caja"
            ),
            Err(ApiError::InvalidCredentials(
                "Credenciales inválidas.".to_string()
            ))
        );

        let t403 = MockTransport::with_status(403, r#"{"message":"Usuario sin acceso."}"#);
        assert_eq!(
            login_with_transport(
                &t403,
                "http://localhost:8000/api/auth/login",
                "client@dylan.pe",
                "123456789",
                "Caja"
            ),
            Err(ApiError::Forbidden("Usuario sin acceso.".to_string()))
        );

        let t429 = MockTransport::with_status(429, r#"{"message":"Too Many Attempts."}"#);
        assert!(matches!(
            login_with_transport(
                &t429,
                "http://localhost:8000/api/auth/login",
                "paint@gmail.com",
                "123",
                "Caja"
            ),
            Err(ApiError::RateLimited(_))
        ));

        let t_net = MockTransport::with_error(ApiError::Network("sin conexión".to_string()));
        assert!(matches!(
            login_with_transport(
                &t_net,
                "http://localhost:8000/api/auth/login",
                "paint@gmail.com",
                "123456789",
                "Caja"
            ),
            Err(ApiError::Network(_))
        ));
    }

    #[test]
    fn fetch_me_sends_bearer_token_and_maps_401_to_unauthorized() {
        let ok_body = r#"{
            "user": {
                "id": 7,
                "name": "Operador Caja",
                "email": "caja@dylan.pe",
                "email_verified_at": "2026-01-01T00:00:00.000000Z",
                "roles": ["admin"],
                "permissions": ["manage.access"]
            }
        }"#;
        let t_ok = MockTransport::with_status(200, ok_body);
        let user =
            fetch_me_with_transport(&t_ok, "http://localhost:8000/api/auth/me", "7|valid_token")
                .expect("valid /api/auth/me");
        let req_ok = t_ok.last_request();
        assert_eq!(req_ok.method, "GET");
        assert_eq!(req_ok.url, "http://localhost:8000/api/auth/me");
        assert_eq!(req_ok.bearer_token.as_deref(), Some("7|valid_token"));
        assert_eq!(user.email, "caja@dylan.pe");
        assert_eq!(user.roles, vec!["admin".to_string()]);
        assert!(map_permissions(&user).cash);

        let t_401 = MockTransport::with_status(401, r#"{"message":"Unauthenticated."}"#);
        let err = fetch_me_with_transport(
            &t_401,
            "http://localhost:8000/api/auth/me",
            "7|revoked_token",
        )
        .unwrap_err();
        assert!(err.is_unauthorized());
    }

    #[test]
    fn logout_sends_bearer_token_and_accepts_200_or_401() {
        let t_ok =
            MockTransport::with_status(200, r#"{"message":"Sesión cerrada correctamente."}"#);
        logout_with_transport(
            &t_ok,
            "http://localhost:8000/api/auth/logout",
            "1|token_to_revoke",
        )
        .expect("logout 200");
        let req = t_ok.last_request();
        assert_eq!(req.method, "POST_EMPTY");
        assert_eq!(req.url, "http://localhost:8000/api/auth/logout");
        assert_eq!(req.bearer_token.as_deref(), Some("1|token_to_revoke"));

        let t_401 = MockTransport::with_status(401, r#"{"message":"Unauthenticated."}"#);
        logout_with_transport(
            &t_401,
            "http://localhost:8000/api/auth/logout",
            "1|already_revoked",
        )
        .expect("logout 401 should be treated as signed out");
    }
}
