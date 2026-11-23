use gpui_kit::{Context, SharedString, component::IconName};

use crate::services::api::auth::{ApiUser, AuthenticatedSession, has_full_access, map_permissions};

/// Grupos de navegación definidos para el sidebar del área autenticada.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[allow(dead_code)]
pub enum RouteGroup {
    Home,
    Operation,
    Administration,
    System,
}

impl RouteGroup {
    pub const ALL: [Self; 4] = [
        Self::Home,
        Self::Operation,
        Self::Administration,
        Self::System,
    ];

    pub const fn label(self) -> &'static str {
        match self {
            Self::Home => "Inicio",
            Self::Operation => "Operación",
            Self::Administration => "Administración",
            Self::System => "Sistema",
        }
    }
}

/// Rutas implementadas del área autenticada.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum AppRoute {
    #[default]
    Dashboard,
    Pensioners,
    Identification,
    Consumptions,
    Cash,
    Reports,
    Settings,
}

impl AppRoute {
    pub const IMPLEMENTED: [Self; 7] = [
        Self::Dashboard,
        Self::Pensioners,
        Self::Identification,
        Self::Consumptions,
        Self::Cash,
        Self::Reports,
        Self::Settings,
    ];

    pub const fn group(self) -> RouteGroup {
        match self {
            Self::Dashboard => RouteGroup::Home,
            Self::Pensioners | Self::Identification | Self::Consumptions => RouteGroup::Operation,
            Self::Cash | Self::Reports => RouteGroup::Administration,
            Self::Settings => RouteGroup::System,
        }
    }

    pub const fn id(self) -> &'static str {
        match self {
            Self::Dashboard => "sidebar-route-dashboard",
            Self::Pensioners => "sidebar-route-pensioners",
            Self::Identification => "sidebar-route-identification",
            Self::Consumptions => "sidebar-route-consumptions",
            Self::Cash => "sidebar-route-cash",
            Self::Reports => "sidebar-route-reports",
            Self::Settings => "sidebar-route-settings",
        }
    }

    pub const fn label(self) -> &'static str {
        match self {
            Self::Dashboard => "Dashboard",
            Self::Pensioners => "Pensionistas",
            Self::Identification => "Identificación",
            Self::Consumptions => "Consumos",
            Self::Cash => "Caja",
            Self::Reports => "Reportes",
            Self::Settings => "Configuración",
        }
    }

    pub const fn icon(self) -> IconName {
        match self {
            Self::Dashboard => IconName::LayoutDashboard,
            Self::Pensioners => IconName::User,
            Self::Identification => IconName::Search,
            Self::Consumptions => IconName::FileText,
            Self::Cash => IconName::Inbox,
            Self::Reports => IconName::ChartPie,
            Self::Settings => IconName::Settings,
        }
    }
}

/// Permisos efectivos de la sesión para habilitar rutas y accesos.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SessionPermissions {
    pub dashboard: bool,
    pub pensioners: bool,
    pub identification: bool,
    pub consumptions: bool,
    pub register_attendance: bool,
    pub cash: bool,
    pub reports: bool,
    pub settings: bool,
    pub search_pensioners: bool,
}

impl SessionPermissions {
    pub const fn full_access() -> Self {
        Self {
            dashboard: true,
            pensioners: true,
            identification: true,
            consumptions: true,
            register_attendance: true,
            cash: true,
            reports: true,
            settings: true,
            search_pensioners: true,
        }
    }

    pub const fn allows_route(self, route: AppRoute) -> bool {
        match route {
            AppRoute::Dashboard => self.dashboard,
            AppRoute::Pensioners => self.pensioners,
            AppRoute::Identification => self.identification,
            AppRoute::Consumptions => self.consumptions,
            AppRoute::Cash => self.cash,
            AppRoute::Reports => self.reports,
            AppRoute::Settings => self.settings,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SessionUser {
    pub id: u64,
    pub name: SharedString,
    pub email: SharedString,
    pub role: SharedString,
    pub roles: Vec<String>,
    pub can_manage_access: bool,
    pub is_super_admin: bool,
    pub email_verified: bool,
    pub remember_me: bool,
    pub permissions: SessionPermissions,
    pensioner_permissions: PensionerPermissions,
    view_payments: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct PensionerPermissions {
    pub create: bool,
    pub update: bool,
    pub enroll: bool,
}

impl SessionUser {
    pub fn from_api(user: &ApiUser, remember_me: bool) -> Self {
        let full_access = has_full_access(user);
        let manage = full_access || user.has_permission("pensioners.manage");
        Self {
            id: user.id,
            name: SharedString::from(user.name.clone()),
            email: SharedString::from(user.email.clone()),
            role: SharedString::from(user.primary_role_label()),
            roles: user.role_names(),
            can_manage_access: user.can_manage_access,
            is_super_admin: user.is_super_admin,
            email_verified: user.email_verified_at.is_some(),
            remember_me,
            permissions: map_permissions(user),
            pensioner_permissions: PensionerPermissions {
                create: manage || user.has_permission("pensioners.create"),
                update: manage || user.has_permission("pensioners.update"),
                enroll: full_access || user.has_permission("biometrics.enroll"),
            },
            view_payments: full_access
                || user.has_permission("payments.view")
                || user.has_permission("cash.view")
                || user.has_permission("cash.manage"),
        }
    }
}

/// Estado observable de la sesión del operador autenticado contra el backend Laravel Sanctum.
///
/// Inicia siempre sin autenticar, mantiene el token únicamente en memoria (nunca en
/// `preferences.json`) y protege el acceso al Dashboard y demás rutas según permisos reales.
#[derive(Debug, Default)]
pub struct SessionState {
    user: Option<SessionUser>,
    access_token: Option<String>,
    expiration_notice: Option<SharedString>,
}

impl SessionState {
    pub const fn new() -> Self {
        Self {
            user: None,
            access_token: None,
            expiration_notice: None,
        }
    }

    pub fn is_authenticated(&self) -> bool {
        self.user.is_some() && self.access_token.is_some()
    }

    pub fn access_token(&self) -> Option<&str> {
        self.access_token.as_deref()
    }

    pub fn can_access_dashboard(&self) -> bool {
        self.can_access_route(AppRoute::Dashboard)
    }

    pub fn can_access_route(&self, route: AppRoute) -> bool {
        self.is_authenticated()
            && self
                .user
                .as_ref()
                .is_some_and(|user| user.permissions.allows_route(route))
    }

    pub fn can_search_pensioners(&self) -> bool {
        self.is_authenticated()
            && self
                .user
                .as_ref()
                .is_some_and(|user| user.permissions.search_pensioners)
    }

    pub fn pensioner_permissions(&self) -> PensionerPermissions {
        self.user
            .as_ref()
            .filter(|_| self.can_access_route(AppRoute::Pensioners))
            .map_or_else(PensionerPermissions::default, |user| {
                user.pensioner_permissions
            })
    }

    pub fn can_view_pensioner_payments(&self) -> bool {
        self.can_access_route(AppRoute::Pensioners)
            && self.user.as_ref().is_some_and(|user| user.view_payments)
    }

    pub fn can_register_attendance(&self) -> bool {
        self.is_authenticated()
            && self
                .user
                .as_ref()
                .is_some_and(|user| user.permissions.register_attendance)
    }

    /// Devuelve únicamente las rutas implementadas y permitidas para la sesión activa.
    pub fn available_routes(&self) -> Vec<AppRoute> {
        AppRoute::IMPLEMENTED
            .into_iter()
            .filter(|&route| self.can_access_route(route))
            .collect()
    }

    pub fn current_user(&self) -> Option<&SessionUser> {
        self.user.as_ref()
    }

    /// Aplica una sesión autenticada por el backend sin requerir contexto de GPUI.
    pub fn apply_authenticated(
        &mut self,
        authenticated: AuthenticatedSession,
        remember_me: bool,
    ) -> &SessionUser {
        self.expiration_notice = None;
        self.access_token = Some(authenticated.token);
        self.user = Some(SessionUser::from_api(&authenticated.user, remember_me));
        self.user.as_ref().expect("session user was just set")
    }

    /// Inicia sesión con el token y usuario devueltos por Laravel Sanctum y notifica a las vistas.
    pub fn sign_in_with_backend(
        &mut self,
        authenticated: AuthenticatedSession,
        remember_me: bool,
        cx: &mut Context<Self>,
    ) {
        self.apply_authenticated(authenticated, remember_me);
        cx.notify();
    }

    /// Actualiza los datos y permisos del usuario autenticado (ej. tras consultar `GET /api/user`).
    pub fn apply_refreshed_user(&mut self, api_user: &ApiUser) -> bool {
        let Some(existing) = self.user.as_ref() else {
            return false;
        };
        let remember_me = existing.remember_me;
        self.user = Some(SessionUser::from_api(api_user, remember_me));
        true
    }

    /// Sincroniza el perfil y permisos desde el backend y notifica a las vistas observadoras.
    pub fn sync_user_from_backend(&mut self, api_user: &ApiUser, cx: &mut Context<Self>) {
        if self.apply_refreshed_user(api_user) {
            cx.notify();
        }
    }

    /// Expira la sesión local con un aviso visible al volver a la pantalla de Acceso.
    pub fn clear_with_expiration(&mut self, notice: impl Into<SharedString>) -> Option<String> {
        self.expiration_notice = Some(notice.into());
        self.user = None;
        self.access_token.take()
    }

    /// Expira la sesión (por ejemplo tras un `401`) y notifica para volver a Acceso.
    pub fn expire_session(&mut self, notice: impl Into<SharedString>, cx: &mut Context<Self>) {
        let _ = self.clear_with_expiration(notice);
        cx.notify();
    }

    pub fn take_expiration_notice(&mut self) -> Option<SharedString> {
        self.expiration_notice.take()
    }

    /// Cierra la sesión actual sin requerir contexto de GPUI y devuelve el token para revocación.
    pub fn clear(&mut self) -> Option<String> {
        self.expiration_notice = None;
        self.user = None;
        self.access_token.take()
    }

    /// Cierra la sesión actual, notifica a las vistas y devuelve el token para `POST /api/logout`.
    pub fn sign_out(&mut self, cx: &mut Context<Self>) -> Option<String> {
        let token = self.clear();
        cx.notify();
        token
    }

    #[cfg(test)]
    pub(crate) fn with_permissions(permissions: SessionPermissions) -> Self {
        Self {
            user: Some(SessionUser {
                id: 99,
                name: SharedString::from("Operador de prueba"),
                email: SharedString::from("operador@dylan.pe"),
                role: SharedString::from("Prueba"),
                roles: vec!["test".to_string()],
                can_manage_access: false,
                is_super_admin: false,
                email_verified: true,
                remember_me: false,
                permissions,
                pensioner_permissions: PensionerPermissions::default(),
                view_payments: permissions.cash,
            }),
            access_token: Some("99|test_token".to_string()),
            expiration_notice: None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{AppRoute, SessionPermissions, SessionState};
    use crate::services::api::auth::{ApiRole, ApiUser, AuthenticatedSession};

    fn sample_super_admin_session() -> AuthenticatedSession {
        AuthenticatedSession {
            token: "1|sanctum_super_admin_token".to_string(),
            token_type: "Bearer".to_string(),
            user: ApiUser {
                id: 1,
                name: "Administrador Principal".to_string(),
                email: "paint@gmail.com".to_string(),
                email_verified_at: Some("2026-01-01T00:00:00Z".to_string()),
                roles: vec![ApiRole {
                    id: 1,
                    name: "super-admin".to_string(),
                }],
                permissions: vec![],
                can_manage_access: true,
                is_super_admin: true,
            },
        }
    }

    #[test]
    fn starts_unauthenticated_and_blocks_dashboard() {
        let session = SessionState::new();
        assert!(!session.is_authenticated());
        assert!(!session.can_access_dashboard());
        assert!(session.available_routes().is_empty());
        assert!(session.current_user().is_none());
        assert!(session.access_token().is_none());
    }

    #[test]
    fn read_only_pensioner_access_does_not_grant_profile_or_biometric_changes() {
        let mut auth = sample_super_admin_session();
        auth.user.is_super_admin = false;
        auth.user.can_manage_access = false;
        auth.user.roles.clear();
        auth.user.permissions = vec!["pensioners.view".into()];
        let mut session = SessionState::new();
        session.apply_authenticated(auth.clone(), false);
        assert!(session.can_access_route(AppRoute::Pensioners));
        assert!(!session.pensioner_permissions().create);
        assert!(!session.pensioner_permissions().update);
        assert!(!session.pensioner_permissions().enroll);
        assert!(!session.can_view_pensioner_payments());

        auth.user
            .permissions
            .extend(["pensioners.update".into(), "payments.view".into()]);
        assert!(session.apply_refreshed_user(&auth.user));
        assert!(session.pensioner_permissions().update);
        assert!(!session.pensioner_permissions().create);
        assert!(session.can_view_pensioner_payments());
    }

    #[test]
    fn authenticates_backend_session_syncs_permissions_and_revokes_on_sign_out() {
        let mut session = SessionState::new();

        let user = session.apply_authenticated(sample_super_admin_session(), true);
        assert_eq!(user.email.as_ref(), "paint@gmail.com");
        assert_eq!(user.role.as_ref(), "Super Admin");
        assert!(user.remember_me);
        assert!(user.is_super_admin);
        assert!(user.can_manage_access);
        assert!(session.is_authenticated());
        assert_eq!(session.access_token(), Some("1|sanctum_super_admin_token"));
        assert!(session.can_access_dashboard());
        assert_eq!(session.available_routes(), AppRoute::IMPLEMENTED.to_vec());

        // Simula degradación de rol al sincronizar con GET /api/user.
        let downgraded = ApiUser {
            id: 1,
            name: "Administrador Principal".to_string(),
            email: "paint@gmail.com".to_string(),
            email_verified_at: Some("2026-01-01T00:00:00Z".to_string()),
            roles: vec![ApiRole {
                id: 3,
                name: "client".to_string(),
            }],
            permissions: vec![],
            can_manage_access: false,
            is_super_admin: false,
        };
        assert!(session.apply_refreshed_user(&downgraded));
        assert_eq!(session.available_routes(), vec![AppRoute::Dashboard]);
        assert!(!session.can_access_route(AppRoute::Cash));
        assert!(!session.can_search_pensioners());

        let token = session.clear();
        assert_eq!(token.as_deref(), Some("1|sanctum_super_admin_token"));
        assert!(!session.is_authenticated());
        assert!(!session.can_access_dashboard());
        assert!(session.available_routes().is_empty());
        assert!(session.current_user().is_none());
    }

    #[test]
    fn expiration_clears_session_and_stores_one_shot_notice() {
        let mut session = SessionState::new();
        session.apply_authenticated(sample_super_admin_session(), false);

        let revoked = session.clear_with_expiration("Tu sesión expiró.");
        assert_eq!(revoked.as_deref(), Some("1|sanctum_super_admin_token"));
        assert!(!session.is_authenticated());
        assert_eq!(
            session.take_expiration_notice().as_deref(),
            Some("Tu sesión expiró.")
        );
        assert!(session.take_expiration_notice().is_none());
    }

    #[test]
    fn hides_and_blocks_routes_without_permission() {
        let session = SessionState::with_permissions(SessionPermissions {
            dashboard: true,
            pensioners: false,
            identification: true,
            consumptions: false,
            register_attendance: false,
            cash: false,
            reports: false,
            settings: false,
            search_pensioners: false,
        });

        assert!(session.can_access_route(AppRoute::Dashboard));
        assert!(!session.can_access_route(AppRoute::Pensioners));
        assert!(session.can_access_route(AppRoute::Identification));
        assert!(!session.can_access_route(AppRoute::Consumptions));
        assert!(!session.can_access_route(AppRoute::Cash));
        assert!(!session.can_access_route(AppRoute::Reports));
        assert!(!session.can_access_route(AppRoute::Settings));
        assert!(!session.can_search_pensioners());
        assert!(!session.can_register_attendance());
        assert_eq!(
            session.available_routes(),
            vec![AppRoute::Dashboard, AppRoute::Identification]
        );
    }
}
