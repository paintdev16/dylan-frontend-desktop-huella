use gpui_kit::{Context, SharedString};

/// Credenciales de demostración para la Etapa 1 (sesión simulada).
pub const DEMO_EMAIL: &str = "admin@dylan.pe";
pub const DEMO_PASSWORD: &str = "dylan123";
pub const DEMO_OPERATOR_NAME: &str = "Operador de demostración";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SessionUser {
    pub name: SharedString,
    pub email: SharedString,
    pub role: SharedString,
    pub remember_me: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AuthError {
    EmptyFields,
    InvalidCredentials,
}

impl AuthError {
    pub const fn message(self) -> &'static str {
        match self {
            Self::EmptyFields => "Ingresa tu correo electrónico y contraseña.",
            Self::InvalidCredentials => {
                "Credenciales inválidas. Verifica tu correo electrónico y contraseña."
            }
        }
    }
}

/// Estado observable de la sesión del operador.
/// Inicia siempre sin autenticar y protege el acceso al Dashboard.
#[derive(Debug, Default)]
pub struct SessionState {
    user: Option<SessionUser>,
}

impl SessionState {
    pub const fn new() -> Self {
        Self { user: None }
    }

    pub fn is_authenticated(&self) -> bool {
        self.user.is_some()
    }

    pub fn can_access_dashboard(&self) -> bool {
        self.is_authenticated()
    }

    pub fn current_user(&self) -> Option<&SessionUser> {
        self.user.as_ref()
    }

    /// Valida las credenciales contra la cuenta de demostración sin requerir contexto de GPUI.
    pub fn authenticate(
        &mut self,
        email: &str,
        password: &str,
        remember_me: bool,
    ) -> Result<&SessionUser, AuthError> {
        let normalized_email = email.trim();
        if normalized_email.is_empty() || password.is_empty() {
            return Err(AuthError::EmptyFields);
        }

        if !normalized_email.eq_ignore_ascii_case(DEMO_EMAIL) || password != DEMO_PASSWORD {
            return Err(AuthError::InvalidCredentials);
        }

        self.user = Some(SessionUser {
            name: SharedString::from(DEMO_OPERATOR_NAME),
            email: SharedString::from(DEMO_EMAIL),
            role: SharedString::from("Administrador"),
            remember_me,
        });

        Ok(self.user.as_ref().expect("session user was just set"))
    }

    /// Inicia sesión y notifica a las vistas observadoras.
    pub fn sign_in(
        &mut self,
        email: &str,
        password: &str,
        remember_me: bool,
        cx: &mut Context<Self>,
    ) -> Result<(), AuthError> {
        self.authenticate(email, password, remember_me)?;
        cx.notify();
        Ok(())
    }

    /// Cierra la sesión actual sin requerir contexto de GPUI.
    pub fn clear(&mut self) -> bool {
        self.user.take().is_some()
    }

    /// Cierra la sesión actual y notifica a las vistas observadoras.
    pub fn sign_out(&mut self, cx: &mut Context<Self>) {
        if self.clear() {
            cx.notify();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{AuthError, DEMO_EMAIL, DEMO_PASSWORD, SessionState};

    #[test]
    fn starts_unauthenticated_and_blocks_dashboard() {
        let session = SessionState::new();
        assert!(!session.is_authenticated());
        assert!(!session.can_access_dashboard());
        assert!(session.current_user().is_none());
    }

    #[test]
    fn rejects_empty_and_invalid_credentials_keeping_dashboard_blocked() {
        let mut session = SessionState::new();

        assert_eq!(
            session.authenticate("", DEMO_PASSWORD, false),
            Err(AuthError::EmptyFields)
        );
        assert_eq!(
            session.authenticate(DEMO_EMAIL, "", false),
            Err(AuthError::EmptyFields)
        );
        assert_eq!(
            session.authenticate("otro@dylan.pe", DEMO_PASSWORD, false),
            Err(AuthError::InvalidCredentials)
        );
        assert_eq!(
            session.authenticate(DEMO_EMAIL, "clave-incorrecta", false),
            Err(AuthError::InvalidCredentials)
        );

        assert!(!session.is_authenticated());
        assert!(!session.can_access_dashboard());
    }

    #[test]
    fn authenticates_demo_account_and_revokes_access_on_sign_out() {
        let mut session = SessionState::new();

        let user = session
            .authenticate("  ADMIN@DYLAN.PE  ", DEMO_PASSWORD, true)
            .expect("demo credentials must succeed");
        assert_eq!(user.email.as_ref(), DEMO_EMAIL);
        assert!(user.remember_me);
        assert!(session.is_authenticated());
        assert!(session.can_access_dashboard());

        session.clear();
        assert!(!session.is_authenticated());
        assert!(!session.can_access_dashboard());
        assert!(session.current_user().is_none());
    }
}
