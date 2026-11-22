use gpui_kit::{
    component::{IconName, Theme, ThemeMode},
    *,
};

/// Preferencia de tema de la aplicación.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ThemePreference {
    #[default]
    System,
    Light,
    Dark,
}

impl ThemePreference {
    pub const ALL: [Self; 3] = [Self::System, Self::Light, Self::Dark];

    pub const fn label(self) -> &'static str {
        match self {
            Self::System => "Sistema",
            Self::Light => "Claro",
            Self::Dark => "Oscuro",
        }
    }

    pub const fn icon(self) -> IconName {
        match self {
            Self::System => IconName::Palette,
            Self::Light => IconName::Sun,
            Self::Dark => IconName::Moon,
        }
    }

    pub fn apply(self, window: &mut Window, cx: &mut App) {
        match self {
            Self::System => Theme::sync_system_appearance(Some(window), cx),
            Self::Light => Theme::change(ThemeMode::Light, Some(window), cx),
            Self::Dark => Theme::change(ThemeMode::Dark, Some(window), cx),
        }
    }
}

/// Estado reactivo del tema de Dylan.
/// Cuando está en `System`, sincroniza automáticamente cambios de apariencia del SO.
pub struct ThemeStore {
    preference: ThemePreference,
    _appearance_subscription: Subscription,
}

impl ThemeStore {
    pub fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        let preference = ThemePreference::default();
        preference.apply(window, cx);

        let appearance_subscription = cx.observe_window_appearance(window, |this, window, cx| {
            if this.preference == ThemePreference::System {
                Theme::sync_system_appearance(Some(window), cx);
                cx.notify();
            }
        });

        Self {
            preference,
            _appearance_subscription: appearance_subscription,
        }
    }

    pub const fn preference(&self) -> ThemePreference {
        self.preference
    }

    pub fn set_preference(
        &mut self,
        preference: ThemePreference,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.preference = preference;
        preference.apply(window, cx);
        cx.notify();
    }
}
