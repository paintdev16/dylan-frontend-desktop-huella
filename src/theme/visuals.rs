use gpui_kit::{Background, BoxShadow, Hsla, Pixels, linear_color_stop, linear_gradient, px};

use super::tokens::{DylanTheme, hex};

pub const TITLE_BAR_HEIGHT: Pixels = px(42.0);
pub const WINDOW_CONTROL_WIDTH: Pixels = px(44.0);

#[derive(Clone, Copy)]
pub struct AppVisuals {
    pub background: Background,
    pub ambient_light: Background,
    pub panel: Background,
    pub panel_border: Hsla,
    pub panel_shadow: Hsla,
    pub glow: Hsla,
    pub brand_background: Background,
    pub brand_border: Hsla,
    pub brand_foreground: Hsla,
    pub watermark: Hsla,
    pub version_background: Hsla,
    pub version_foreground: Hsla,
}

impl AppVisuals {
    pub fn panel_shadows(&self) -> [BoxShadow; 2] {
        [
            BoxShadow::new(px(-10.0), px(21.0), self.glow.alpha(0.24))
                .blur_radius(px(48.0))
                .spread_radius(px(-12.0)),
            BoxShadow::new(px(12.0), px(24.0), self.panel_shadow.alpha(0.28))
                .blur_radius(px(40.0))
                .spread_radius(px(-9.0)),
        ]
    }
}

impl DylanTheme {
    pub fn visuals(&self) -> AppVisuals {
        if self.is_dark() {
            AppVisuals {
                background: gradient(152.0, hex(0x0E1333), hex(0x161B45)),
                ambient_light: linear_gradient(
                    145.0,
                    linear_color_stop(hex(0x6438E8).alpha(0.16), 0.0),
                    linear_color_stop(hex(0x2563EB).alpha(0.0), 0.65),
                ),
                panel: gradient(152.0, hex(0x262E5C).alpha(0.60), hex(0x181F47).alpha(0.54)),
                panel_border: hex(0xA78BFA).alpha(0.45),
                panel_shadow: hex(0x070A1D),
                glow: hex(0x8B5CF6),
                brand_background: gradient(145.0, hex(0x9837FF), hex(0x3038C9)),
                brand_border: hex(0xD0ACFF).alpha(0.62),
                brand_foreground: hex(0xFFFFFF),
                watermark: hex(0xA78BFA),
                version_background: hex(0x413EB8),
                version_foreground: hex(0xFFFFFF),
            }
        } else {
            AppVisuals {
                background: gradient(152.0, hex(0xECEEFF), hex(0xF9F9FF)),
                ambient_light: linear_gradient(
                    145.0,
                    linear_color_stop(hex(0x7C3AED).alpha(0.09), 0.0),
                    linear_color_stop(hex(0x2563EB).alpha(0.0), 0.65),
                ),
                panel: gradient(150.0, hex(0xFFFFFF).alpha(0.60), hex(0xF0F1FF).alpha(0.50)),
                panel_border: hex(0xFFFFFF).alpha(0.88),
                panel_shadow: hex(0x4246DE),
                glow: hex(0x8B5CF6),
                brand_background: gradient(146.0, hex(0x9837FF), hex(0x373BD4)),
                brand_border: hex(0xE3C4FF).alpha(0.78),
                brand_foreground: hex(0xFFFFFF),
                watermark: hex(0x6438E8),
                version_background: hex(0xE3DAFC),
                version_foreground: hex(0x552BD4),
            }
        }
    }
}

pub fn gradient(angle: f32, start: Hsla, end: Hsla) -> Background {
    linear_gradient(
        angle,
        linear_color_stop(start, 0.0),
        linear_color_stop(end, 1.0),
    )
}
