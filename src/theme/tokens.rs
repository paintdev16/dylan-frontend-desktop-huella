#![allow(dead_code)]

use gpui_kit::{
    component::{ActiveTheme as _, ThemeMode},
    *,
};

/// Escala de espaciado compartida del escritorio del comedor.
pub struct Spacing;

impl Spacing {
    pub const XXS: Pixels = px(2.0);
    pub const XS: Pixels = px(4.0);
    pub const SM: Pixels = px(8.0);
    pub const MD: Pixels = px(12.0);
    pub const LG: Pixels = px(16.0);
    pub const XL: Pixels = px(24.0);
    pub const XXL: Pixels = px(32.0);
}

/// Escala de radios de borde compartida.
pub struct Radii;

impl Radii {
    pub const SM: Pixels = px(6.0);
    pub const MD: Pixels = px(8.0);
    pub const LG: Pixels = px(12.0);
    pub const XL: Pixels = px(18.0);
    pub const FULL: Pixels = px(9999.0);
}

/// Escala tipográfica base del escritorio del comedor.
pub struct Typography;

impl Typography {
    pub const XS: Pixels = px(12.0);
    pub const SM: Pixels = px(13.0);
    pub const BASE: Pixels = px(14.0);
    pub const LG: Pixels = px(16.0);
    pub const XL: Pixels = px(20.0);
    pub const METRIC_VALUE: Pixels = px(26.0);
    pub const PAGE_TITLE: Pixels = px(26.0);
}

/// Tokens cromáticos para estados semánticos (éxito, advertencia, error, info, neutro).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SemanticTone {
    pub solid: Hsla,
    pub solid_hover: Hsla,
    pub solid_active: Hsla,
    pub solid_foreground: Hsla,
    pub tint: Hsla,
    pub border: Hsla,
    pub foreground: Hsla,
    pub icon: Hsla,
}

/// Categorías de métricas compartidas entre Dashboard, Caja y Reportes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum MetricAccent {
    /// Desayunos: Ámbar.
    Breakfasts,
    /// Almuerzos: Naranja.
    Lunches,
    /// Efectivo ingresado: Verde.
    CashIn,
    /// Tickets recibidos: Azul.
    TicketsReceived,
    /// Saldo a favor: Turquesa.
    CreditBalance,
    /// Deuda pendiente: Rojo suave.
    PendingDebt,
    /// Pensionistas registrados, pensionistas activos y raciones: Neutro o lavanda suave.
    Neutral,
}

/// Tokens resueltos para una tarjeta de métrica y su botón contextual.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct MetricTone {
    pub background: Hsla,
    pub border: Hsla,
    pub icon: Hsla,
    pub icon_background: Hsla,
    pub label: Hsla,
    pub value: Hsla,
    pub action_tint: Hsla,
    pub action_hover: Hsla,
    pub action_active: Hsla,
    pub action_foreground: Hsla,
}

/// Recursos visuales de marca compartidos (degradado púrpura→azul, fondos suaves y sombras índigo).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct BrandVisuals {
    pub gradient_start: Hsla,
    pub gradient_end: Hsla,
    pub gradient_foreground: Hsla,
    pub accent_bar_start: Hsla,
    pub accent_bar_end: Hsla,
    pub canvas_start: Hsla,
    pub canvas_end: Hsla,
    pub glow_primary: Hsla,
    pub glow_secondary: Hsla,
    pub glow_wave: Hsla,
    pub card_shadow_color: Hsla,
    pub brand_shadow_color: Hsla,
    pub dot_pattern: Hsla,
}

impl BrandVisuals {
    /// Degradado diagonal púrpura→azul para el botón **Iniciar sesión** en Acceso y el icono de marca.
    pub fn primary_gradient(&self) -> Background {
        linear_gradient(
            135.0,
            linear_color_stop(self.gradient_start, 0.0),
            linear_color_stop(self.gradient_end, 1.0),
        )
    }

    /// Alias semántico para el botón **Iniciar sesión** en Acceso.
    pub fn login_button_gradient(&self) -> Background {
        self.primary_gradient()
    }

    /// Barra horizontal azul→púrpura para detalles de acento y divisores destacados.
    pub fn accent_bar_gradient(&self) -> Background {
        linear_gradient(
            90.0,
            linear_color_stop(self.accent_bar_start, 0.0),
            linear_color_stop(self.accent_bar_end, 1.0),
        )
    }

    /// Fondo atmosférico lavanda→celeste (o azul noche→índigo en oscuro) para Acceso y portadas.
    pub fn atmospheric_canvas(&self) -> Background {
        linear_gradient(
            135.0,
            linear_color_stop(self.canvas_start, 0.0),
            linear_color_stop(self.canvas_end, 1.0),
        )
    }

    /// Sombra elevada con tinte índigo para tarjetas flotantes.
    pub fn card_shadow(&self) -> Vec<BoxShadow> {
        vec![
            BoxShadow::new(px(0.0), px(14.0), self.card_shadow_color)
                .blur_radius(px(36.0))
                .spread_radius(px(-6.0)),
        ]
    }

    /// Resplandor violeta-azul bajo el icono de marca.
    pub fn brand_shadow(&self) -> Vec<BoxShadow> {
        vec![
            BoxShadow::new(px(0.0), px(8.0), self.brand_shadow_color)
                .blur_radius(px(20.0))
                .spread_radius(px(-2.0)),
        ]
    }
}

/// Paleta semántica completa del escritorio del comedor para el modo activo.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct DylanPalette {
    pub background: Hsla,
    pub surface: Hsla,
    pub surface_elevated: Hsla,
    pub surface_subtle: Hsla,
    pub title_bar: Hsla,
    pub foreground: Hsla,
    pub muted_foreground: Hsla,
    pub border: Hsla,
    pub border_luminous: Hsla,
    pub input_border: Hsla,
    pub focus_ring: Hsla,
    pub primary: Hsla,
    pub primary_hover: Hsla,
    pub primary_active: Hsla,
    pub primary_foreground: Hsla,
    pub brand_accent: Hsla,
    pub secondary_surface: Hsla,
    pub secondary_hover: Hsla,
    pub secondary_active: Hsla,
    pub secondary_foreground: Hsla,
    pub success: SemanticTone,
    pub warning: SemanticTone,
    pub error: SemanticTone,
    pub info: SemanticTone,
    pub neutral: SemanticTone,
    pub brand: BrandVisuals,
}

/// Tema global de Dylan sincronizado con el modo activo (`Light` / `Dark`).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct DylanTheme {
    pub mode: ThemeMode,
    pub palette: DylanPalette,
}

impl Global for DylanTheme {}

pub trait ActiveDylanTheme {
    fn dylan_theme(&self) -> &DylanTheme;
}

impl ActiveDylanTheme for App {
    #[inline]
    fn dylan_theme(&self) -> &DylanTheme {
        self.global::<DylanTheme>()
    }
}

impl DylanTheme {
    pub fn for_mode(mode: ThemeMode) -> Self {
        let palette = if mode.is_dark() {
            DylanPalette::dark()
        } else {
            DylanPalette::light()
        };
        Self { mode, palette }
    }

    pub fn sync_from_active_theme(cx: &mut App) {
        let mode = cx.theme().mode;
        cx.set_global(Self::for_mode(mode));
    }

    pub fn is_dark(&self) -> bool {
        self.mode.is_dark()
    }

    pub fn metric(&self, accent: MetricAccent) -> MetricTone {
        if self.is_dark() {
            metric_dark(accent)
        } else {
            metric_light(accent)
        }
    }
}

#[inline]
pub(crate) fn hex(value: u32) -> Hsla {
    rgb(value).into()
}

#[inline]
pub(crate) fn hex_alpha(value: u32, alpha: f32) -> Hsla {
    hex(value).alpha(alpha)
}

impl DylanPalette {
    pub fn light() -> Self {
        Self {
            background: hex(0xF3F4FF),
            surface: hex(0xFFFFFF),
            surface_elevated: hex_alpha(0xFFFFFF, 0.94),
            surface_subtle: hex(0xECEEFF),
            title_bar: hex(0xFFFFFF),
            foreground: hex(0x0F1430),
            muted_foreground: hex(0x5A6282),
            border: hex(0xE3E1F7),
            border_luminous: hex_alpha(0xFFFFFF, 0.92),
            input_border: hex(0xCFD2EE),
            focus_ring: hex(0x6438E8),
            primary: hex(0x6438E8),
            primary_hover: hex(0x542BD1),
            primary_active: hex(0x4622B5),
            primary_foreground: hex(0xFFFFFF),
            brand_accent: hex(0x6438E8),
            secondary_surface: hex(0xFFFFFF),
            secondary_hover: hex(0xF0EEFF),
            secondary_active: hex(0xE4E0FC),
            secondary_foreground: hex(0x191F3D),
            success: SemanticTone {
                solid: hex(0x15803D),
                solid_hover: hex(0x166534),
                solid_active: hex(0x14532D),
                solid_foreground: hex(0xFFFFFF),
                tint: hex(0xECFDF3),
                border: hex(0x86EFAC),
                foreground: hex(0x14532D),
                icon: hex(0x15803D),
            },
            warning: SemanticTone {
                solid: hex(0xB45309),
                solid_hover: hex(0x92400E),
                solid_active: hex(0x78350F),
                solid_foreground: hex(0xFFFFFF),
                tint: hex(0xFFFBEB),
                border: hex(0xFCD34D),
                foreground: hex(0x78350F),
                icon: hex(0xB45309),
            },
            error: SemanticTone {
                solid: hex(0xD92D20),
                solid_hover: hex(0xB42318),
                solid_active: hex(0x912018),
                solid_foreground: hex(0xFFFFFF),
                tint: hex(0xFEF2F2),
                border: hex(0xFCA5A5),
                foreground: hex(0x991B1B),
                icon: hex(0xDC2626),
            },
            info: SemanticTone {
                solid: hex(0x2563EB),
                solid_hover: hex(0x1D4ED8),
                solid_active: hex(0x1E40AF),
                solid_foreground: hex(0xFFFFFF),
                tint: hex(0xEFF5FF),
                border: hex(0x93C5FD),
                foreground: hex(0x1E3A8A),
                icon: hex(0x2563EB),
            },
            neutral: SemanticTone {
                solid: hex(0x475569),
                solid_hover: hex(0x334155),
                solid_active: hex(0x1E293B),
                solid_foreground: hex(0xFFFFFF),
                tint: hex(0xE3DAFC),
                border: hex(0xCFC2FA),
                foreground: hex(0x552BD4),
                icon: hex(0x6438E8),
            },
            brand: BrandVisuals {
                gradient_start: hex(0x8B3DFF),
                gradient_end: hex(0x373BD4),
                gradient_foreground: hex(0xFFFFFF),
                accent_bar_start: hex(0x373BD4),
                accent_bar_end: hex(0x9837FF),
                canvas_start: hex(0xD5D8F9),
                canvas_end: hex(0xF9F8FF),
                glow_primary: hex_alpha(0x6F46F1, 0.28),
                glow_secondary: hex_alpha(0x4246DE, 0.24),
                glow_wave: hex_alpha(0xA979F8, 0.18),
                card_shadow_color: hex_alpha(0x4246DE, 0.18),
                brand_shadow_color: hex_alpha(0x9256E8, 0.44),
                dot_pattern: hex_alpha(0x7474DE, 0.20),
            },
        }
    }

    pub fn dark() -> Self {
        Self {
            background: hex(0x11152E),
            surface: hex(0x181E3E),
            surface_elevated: hex_alpha(0x1C234A, 0.94),
            surface_subtle: hex(0x222952),
            title_bar: hex(0x141936),
            foreground: hex(0xF4F6FF),
            muted_foreground: hex(0xA9B4D4),
            border: hex(0x2C3566),
            border_luminous: hex_alpha(0xA78BFA, 0.28),
            input_border: hex(0x3B4785),
            focus_ring: hex(0xA78BFA),
            primary: hex(0x6E48F0),
            primary_hover: hex(0x7D5AF5),
            primary_active: hex(0x5B37DA),
            primary_foreground: hex(0xFFFFFF),
            brand_accent: hex(0xA78BFA),
            secondary_surface: hex(0x1E254D),
            secondary_hover: hex(0x283163),
            secondary_active: hex(0x313C78),
            secondary_foreground: hex(0xF4F6FF),
            success: SemanticTone {
                solid: hex(0x15803D),
                solid_hover: hex(0x16A34A),
                solid_active: hex(0x166534),
                solid_foreground: hex(0xFFFFFF),
                tint: hex(0x132D2B),
                border: hex(0x22734E),
                foreground: hex(0x86EFAC),
                icon: hex(0x4ADE80),
            },
            warning: SemanticTone {
                solid: hex(0xB45309),
                solid_hover: hex(0xD97706),
                solid_active: hex(0x92400E),
                solid_foreground: hex(0xFFFFFF),
                tint: hex(0x2E241C),
                border: hex(0x926220),
                foreground: hex(0xFDE68A),
                icon: hex(0xFBBF24),
            },
            error: SemanticTone {
                solid: hex(0xD92D20),
                solid_hover: hex(0xE04035),
                solid_active: hex(0xB42318),
                solid_foreground: hex(0xFFFFFF),
                tint: hex(0x331B2C),
                border: hex(0x9F3142),
                foreground: hex(0xFECACA),
                icon: hex(0xF87171),
            },
            info: SemanticTone {
                solid: hex(0x2563EB),
                solid_hover: hex(0x3B82F6),
                solid_active: hex(0x1D4ED8),
                solid_foreground: hex(0xFFFFFF),
                tint: hex(0x17254D),
                border: hex(0x3159A8),
                foreground: hex(0xBFDBFE),
                icon: hex(0x60A5FA),
            },
            neutral: SemanticTone {
                solid: hex(0x64748B),
                solid_hover: hex(0x7B8CA3),
                solid_active: hex(0x475569),
                solid_foreground: hex(0xFFFFFF),
                tint: hex(0x222654),
                border: hex(0x524899),
                foreground: hex(0xDDD6FE),
                icon: hex(0xA78BFA),
            },
            brand: BrandVisuals {
                gradient_start: hex(0x8B3DFF),
                gradient_end: hex(0x3038C9),
                gradient_foreground: hex(0xFFFFFF),
                accent_bar_start: hex(0x493FEB),
                accent_bar_end: hex(0xA746FF),
                canvas_start: hex(0x0C1338),
                canvas_end: hex(0x191653),
                glow_primary: hex_alpha(0xA347F2, 0.26),
                glow_secondary: hex_alpha(0x263BD1, 0.24),
                glow_wave: hex_alpha(0x9357FA, 0.18),
                card_shadow_color: hex_alpha(0x050714, 0.55),
                brand_shadow_color: hex_alpha(0xA347F2, 0.45),
                dot_pattern: hex_alpha(0x8D91FB, 0.22),
            },
        }
    }
}

fn metric_light(accent: MetricAccent) -> MetricTone {
    match accent {
        MetricAccent::Breakfasts => MetricTone {
            background: hex(0xFFFBEB),
            border: hex(0xF59E0B),
            icon: hex(0xB45309),
            icon_background: hex(0xFEF3C7),
            label: hex(0x78350F),
            value: hex(0x451A03),
            action_tint: hex(0xFEF3C7),
            action_hover: hex(0xFDE68A),
            action_active: hex(0xFCD34D),
            action_foreground: hex(0x78350F),
        },
        MetricAccent::Lunches => MetricTone {
            background: hex(0xFFF7ED),
            border: hex(0xF97316),
            icon: hex(0xC2410C),
            icon_background: hex(0xFFEDD5),
            label: hex(0x7C2D12),
            value: hex(0x431407),
            action_tint: hex(0xFFEDD5),
            action_hover: hex(0xFED7AA),
            action_active: hex(0xFDBA74),
            action_foreground: hex(0x7C2D12),
        },
        MetricAccent::CashIn => MetricTone {
            background: hex(0xECFDF5),
            border: hex(0x16A34A),
            icon: hex(0x15803D),
            icon_background: hex(0xD1FAE5),
            label: hex(0x14532D),
            value: hex(0x052E16),
            action_tint: hex(0xDCFCE7),
            action_hover: hex(0xBBF7D0),
            action_active: hex(0x86EFAC),
            action_foreground: hex(0x14532D),
        },
        MetricAccent::TicketsReceived => MetricTone {
            background: hex(0xEFF5FF),
            border: hex(0x2563EB),
            icon: hex(0x1D4ED8),
            icon_background: hex(0xDBEAFE),
            label: hex(0x1E3A8A),
            value: hex(0x172554),
            action_tint: hex(0xDBEAFE),
            action_hover: hex(0xBFDBFE),
            action_active: hex(0x93C5FD),
            action_foreground: hex(0x1E3A8A),
        },
        MetricAccent::CreditBalance => MetricTone {
            background: hex(0xF0FDFA),
            border: hex(0x0D9488),
            icon: hex(0x0F766E),
            icon_background: hex(0xCCFBF1),
            label: hex(0x115E59),
            value: hex(0x042F2E),
            action_tint: hex(0xCCFBF1),
            action_hover: hex(0x99F6E4),
            action_active: hex(0x5EEAD4),
            action_foreground: hex(0x115E59),
        },
        MetricAccent::PendingDebt => MetricTone {
            background: hex(0xFFF1F2),
            border: hex(0xE11D48),
            icon: hex(0xBE123C),
            icon_background: hex(0xFFE4E6),
            label: hex(0x881337),
            value: hex(0x4C0519),
            action_tint: hex(0xFFE4E6),
            action_hover: hex(0xFECDD3),
            action_active: hex(0xFDA4AF),
            action_foreground: hex(0x881337),
        },
        MetricAccent::Neutral => MetricTone {
            background: hex(0xF4F1FF),
            border: hex(0x8B5CF6),
            icon: hex(0x6438E8),
            icon_background: hex(0xEAE4FF),
            label: hex(0x3B2D73),
            value: hex(0x1A1647),
            action_tint: hex(0xEAE4FF),
            action_hover: hex(0xDDD4FF),
            action_active: hex(0xC6B7FD),
            action_foreground: hex(0x3B2D73),
        },
    }
}

fn metric_dark(accent: MetricAccent) -> MetricTone {
    match accent {
        MetricAccent::Breakfasts => MetricTone {
            background: hex(0x28211F),
            border: hex(0xD97706),
            icon: hex(0xFBBF24),
            icon_background: hex(0x3B2E1E),
            label: hex(0xFDE68A),
            value: hex(0xFFFBEB),
            action_tint: hex(0x3B2E1E),
            action_hover: hex(0x4D3B22),
            action_active: hex(0x5E4726),
            action_foreground: hex(0xFDE68A),
        },
        MetricAccent::Lunches => MetricTone {
            background: hex(0x2B1F21),
            border: hex(0xEA580C),
            icon: hex(0xFB923C),
            icon_background: hex(0x422921),
            label: hex(0xFED7AA),
            value: hex(0xFFF7ED),
            action_tint: hex(0x422921),
            action_hover: hex(0x543326),
            action_active: hex(0x663D2B),
            action_foreground: hex(0xFED7AA),
        },
        MetricAccent::CashIn => MetricTone {
            background: hex(0x142A2B),
            border: hex(0x22C55E),
            icon: hex(0x4ADE80),
            icon_background: hex(0x183E36),
            label: hex(0xBBF7D0),
            value: hex(0xECFDF5),
            action_tint: hex(0x183E36),
            action_hover: hex(0x1E4E42),
            action_active: hex(0x245E4F),
            action_foreground: hex(0xBBF7D0),
        },
        MetricAccent::TicketsReceived => MetricTone {
            background: hex(0x162447),
            border: hex(0x3B82F6),
            icon: hex(0x60A5FA),
            icon_background: hex(0x1E3363),
            label: hex(0xBFDBFE),
            value: hex(0xEFF6FF),
            action_tint: hex(0x1E3363),
            action_hover: hex(0x25407A),
            action_active: hex(0x2D4D91),
            action_foreground: hex(0xBFDBFE),
        },
        MetricAccent::CreditBalance => MetricTone {
            background: hex(0x132A38),
            border: hex(0x14B8A6),
            icon: hex(0x2DD4BF),
            icon_background: hex(0x173F4D),
            label: hex(0x99F6E4),
            value: hex(0xF0FDFA),
            action_tint: hex(0x173F4D),
            action_hover: hex(0x1D4F60),
            action_active: hex(0x235F73),
            action_foreground: hex(0x99F6E4),
        },
        MetricAccent::PendingDebt => MetricTone {
            background: hex(0x2E1A2D),
            border: hex(0xF43F5E),
            icon: hex(0xFB7185),
            icon_background: hex(0x462238),
            label: hex(0xFECDD3),
            value: hex(0xFFF1F2),
            action_tint: hex(0x462238),
            action_hover: hex(0x592A45),
            action_active: hex(0x6B3252),
            action_foreground: hex(0xFECDD3),
        },
        MetricAccent::Neutral => MetricTone {
            background: hex(0x1E224A),
            border: hex(0xA78BFA),
            icon: hex(0xA78BFA),
            icon_background: hex(0x2B2F63),
            label: hex(0xDDD6FE),
            value: hex(0xF5F3FF),
            action_tint: hex(0x2B2F63),
            action_hover: hex(0x363B7A),
            action_active: hex(0x424791),
            action_foreground: hex(0xDDD6FE),
        },
    }
}

#[cfg(test)]
mod tests {
    use super::{DylanTheme, MetricAccent};
    use gpui_kit::{Hsla, Rgba, component::ThemeMode};

    const ALL_METRICS: [MetricAccent; 7] = [
        MetricAccent::Breakfasts,
        MetricAccent::Lunches,
        MetricAccent::CashIn,
        MetricAccent::TicketsReceived,
        MetricAccent::CreditBalance,
        MetricAccent::PendingDebt,
        MetricAccent::Neutral,
    ];

    fn channel_linear(c: f32) -> f32 {
        if c <= 0.04045 {
            c / 12.92
        } else {
            ((c + 0.055) / 1.055).powf(2.4)
        }
    }

    fn relative_luminance(color: Hsla) -> f32 {
        let rgba: Rgba = color.into();
        0.2126 * channel_linear(rgba.r)
            + 0.7152 * channel_linear(rgba.g)
            + 0.0722 * channel_linear(rgba.b)
    }

    fn contrast_ratio(a: Hsla, b: Hsla) -> f32 {
        let l1 = relative_luminance(a);
        let l2 = relative_luminance(b);
        let (lighter, darker) = if l1 >= l2 { (l1, l2) } else { (l2, l1) };
        (lighter + 0.05) / (darker + 0.05)
    }

    #[test]
    fn base_palettes_meet_wcag_aa_contrast_in_both_modes() {
        for mode in [ThemeMode::Light, ThemeMode::Dark] {
            let theme = DylanTheme::for_mode(mode);
            let p = theme.palette;

            assert!(
                contrast_ratio(p.foreground, p.background) >= 7.0,
                "foreground/background contrast failed in {mode:?}"
            );
            assert!(
                contrast_ratio(p.foreground, p.surface) >= 7.0,
                "foreground/surface contrast failed in {mode:?}"
            );
            assert!(
                contrast_ratio(p.muted_foreground, p.surface) >= 4.5,
                "muted_foreground/surface contrast failed in {mode:?}"
            );
            assert!(
                contrast_ratio(p.primary_foreground, p.primary) >= 4.5,
                "primary button contrast failed in {mode:?}"
            );
            assert!(
                contrast_ratio(p.primary_foreground, p.primary_hover) >= 4.5,
                "primary hover button contrast failed in {mode:?}"
            );
            assert!(
                contrast_ratio(p.secondary_foreground, p.secondary_surface) >= 7.0,
                "secondary button contrast failed in {mode:?}"
            );
            assert!(
                contrast_ratio(p.error.solid_foreground, p.error.solid) >= 4.5,
                "destructive button contrast failed in {mode:?}"
            );
            assert!(
                contrast_ratio(p.brand.gradient_foreground, p.brand.gradient_start) >= 4.5,
                "brand gradient start contrast failed in {mode:?}"
            );
            assert!(
                contrast_ratio(p.brand.gradient_foreground, p.brand.gradient_end) >= 4.5,
                "brand gradient end contrast failed in {mode:?}"
            );
            assert!(
                contrast_ratio(p.brand_accent, p.background) >= 4.5,
                "brand_accent/background contrast failed in {mode:?}"
            );
        }
    }

    #[test]
    fn status_tones_and_metric_cards_meet_wcag_aa_contrast() {
        for mode in [ThemeMode::Light, ThemeMode::Dark] {
            let theme = DylanTheme::for_mode(mode);
            let p = theme.palette;

            for (name, tone) in [
                ("success", p.success),
                ("warning", p.warning),
                ("error", p.error),
                ("info", p.info),
                ("neutral", p.neutral),
            ] {
                assert!(
                    contrast_ratio(tone.foreground, tone.tint) >= 4.5,
                    "{name} badge foreground/tint contrast failed in {mode:?}"
                );
                assert!(
                    contrast_ratio(tone.solid_foreground, tone.solid) >= 4.5,
                    "{name} solid foreground/bg contrast failed in {mode:?}"
                );
            }

            for accent in ALL_METRICS {
                let metric = theme.metric(accent);
                assert!(
                    contrast_ratio(metric.value, metric.background) >= 7.0,
                    "{accent:?} value contrast failed in {mode:?}"
                );
                assert!(
                    contrast_ratio(metric.label, metric.background) >= 4.5,
                    "{accent:?} label contrast failed in {mode:?}"
                );
                assert!(
                    contrast_ratio(metric.action_foreground, metric.action_tint) >= 4.5,
                    "{accent:?} contextual button contrast failed in {mode:?}"
                );
                assert!(
                    contrast_ratio(metric.action_foreground, metric.action_hover) >= 4.5,
                    "{accent:?} contextual hover contrast failed in {mode:?}"
                );
            }
        }
    }
}
