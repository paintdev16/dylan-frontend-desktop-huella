mod config;
mod tokens;
pub mod visuals;

use gpui_kit::{
    component::{Icon, IconName, Theme, ThemeMode},
    *,
};

assets::icon_assets!(ThemeIcons, [Monitor]);

#[allow(unused_imports)]
pub use tokens::{
    ActiveDylanTheme, BrandVisuals, DylanPalette, DylanTheme, MetricAccent, MetricTone, Radii,
    SemanticTone, Spacing, Typography,
};

/// Registra los temas claro y oscuro del comedor en GPUI Kit y sincroniza los
/// tokens semánticos propios (`DylanTheme`).
pub fn install_themes(cx: &mut App) {
    let mode = Theme::global(cx).mode;
    Theme::update(cx, |theme| {
        theme.light_theme = config::dylan_light_config();
        theme.dark_theme = config::dylan_dark_config();
    });
    Theme::change(mode, None, cx);
    DylanTheme::sync_from_active_theme(cx);
}

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

    pub fn icon(self) -> Icon {
        match self {
            Self::System => ThemeIcons
                .load(&assets::IconName::Monitor.path())
                .ok()
                .flatten()
                .map_or_else(
                    || Icon::new(IconName::Palette),
                    |icon| Icon::default().data(&icon),
                ),
            Self::Light => Icon::new(IconName::Sun),
            Self::Dark => Icon::new(IconName::Moon),
        }
    }

    pub fn apply(self, window: &mut Window, cx: &mut App) {
        match self {
            Self::System => Theme::sync_system_appearance(Some(window), cx),
            Self::Light => Theme::change(ThemeMode::Light, Some(window), cx),
            Self::Dark => Theme::change(ThemeMode::Dark, Some(window), cx),
        }
        DylanTheme::sync_from_active_theme(cx);
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
        if !cx.has_global::<DylanTheme>() {
            install_themes(cx);
        }

        let preference = ThemePreference::default();
        preference.apply(window, cx);

        let appearance_subscription = cx.observe_window_appearance(window, |this, window, cx| {
            if this.preference == ThemePreference::System {
                Theme::sync_system_appearance(Some(window), cx);
                DylanTheme::sync_from_active_theme(cx);
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
