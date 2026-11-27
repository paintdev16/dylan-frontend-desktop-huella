use std::sync::Arc;

use gpui_kit::*;

const DYLAN_SVG: &str = include_str!("../../assets/dylan.svg");
const LOGO_VIEW: &str = r#"viewBox="150 84 500 410" width="500" height="410""#;
const MARK_VIEW: &str = r#"viewBox="306 88 188 188" width="188" height="188""#;

struct DylanBrand {
    logo: Arc<RenderImage>,
    mark: Arc<RenderImage>,
}

impl Global for DylanBrand {}

pub fn init(cx: &mut App) {
    let renderer = cx.svg_renderer();
    let mark = DYLAN_SVG.replacen(LOGO_VIEW, MARK_VIEW, 1);
    cx.set_global(DylanBrand {
        logo: renderer
            .render_single_frame(DYLAN_SVG.as_bytes(), 0.5)
            .expect("El SVG de Dylan debe ser válido"),
        mark: renderer
            .render_single_frame(mark.as_bytes(), 0.5)
            .expect("El isotipo de Dylan debe ser válido"),
    });
}

pub fn dylan_logo(height: Pixels, cx: &App) -> impl IntoElement {
    div()
        .id("dylan-logo")
        .flex_shrink_0()
        .p(px(4.))
        .child(
            img(cx.global::<DylanBrand>().logo.clone())
                .w(height * (500. / 410.))
                .h(height),
        )
        .role(Role::Image)
        .aria_label("Dylan Restaurante")
}

pub fn dylan_mark(side: Pixels, cx: &App) -> impl IntoElement {
    div()
        .id("dylan-mark")
        .flex_shrink_0()
        .p(px(2.))
        .child(img(cx.global::<DylanBrand>().mark.clone()).size(side))
        .role(Role::Image)
        .aria_label("Dylan Restaurante")
}

/// Silueta sólida continua (sin trazos ni círculos superpuestos) para el fondo decorativo.
pub fn paint_cutlery_silhouette_rotated(
    window: &mut Window,
    bounds: Bounds<Pixels>,
    color: Hsla,
    angle_rad: f32,
) {
    let (sin, cos) = angle_rad.sin_cos();
    let at = |x: f32, y: f32| {
        let x = bounds.size.width * (x - 0.5);
        let y = bounds.size.height * (y - 0.5);
        bounds.center() + point(x * cos - y * sin, x * sin + y * cos)
    };

    // Tenedor sólido en un único contorno cerrado
    let mut fork = PathBuilder::fill();
    fork.move_to(at(0.112, 0.135));
    fork.cubic_bezier_to(at(0.168, 0.135), at(0.112, 0.095), at(0.168, 0.095));
    fork.line_to(at(0.168, 0.355));
    fork.cubic_bezier_to(at(0.242, 0.355), at(0.168, 0.405), at(0.242, 0.405));
    fork.line_to(at(0.242, 0.135));
    fork.cubic_bezier_to(at(0.298, 0.135), at(0.242, 0.095), at(0.298, 0.095));
    fork.line_to(at(0.298, 0.355));
    fork.cubic_bezier_to(at(0.372, 0.355), at(0.298, 0.405), at(0.372, 0.405));
    fork.line_to(at(0.372, 0.135));
    fork.cubic_bezier_to(at(0.428, 0.135), at(0.372, 0.095), at(0.428, 0.095));
    fork.line_to(at(0.428, 0.39));
    fork.cubic_bezier_to(at(0.312, 0.54), at(0.428, 0.485), at(0.355, 0.515));
    fork.line_to(at(0.326, 0.87));
    fork.cubic_bezier_to(at(0.214, 0.87), at(0.326, 0.935), at(0.214, 0.935));
    fork.line_to(at(0.228, 0.54));
    fork.cubic_bezier_to(at(0.112, 0.39), at(0.185, 0.515), at(0.112, 0.485));
    fork.close();
    if let Ok(path) = fork.build() {
        window.paint_path(path, color);
    }

    // Cuchara sólida en un único contorno cerrado
    let mut spoon = PathBuilder::fill();
    spoon.move_to(at(0.73, 0.09));
    spoon.cubic_bezier_to(at(0.89, 0.31), at(0.83, 0.09), at(0.89, 0.19));
    spoon.cubic_bezier_to(at(0.772, 0.54), at(0.89, 0.44), at(0.815, 0.50));
    spoon.line_to(at(0.786, 0.87));
    spoon.cubic_bezier_to(at(0.674, 0.87), at(0.786, 0.935), at(0.674, 0.935));
    spoon.line_to(at(0.688, 0.54));
    spoon.cubic_bezier_to(at(0.57, 0.31), at(0.645, 0.50), at(0.57, 0.44));
    spoon.cubic_bezier_to(at(0.73, 0.09), at(0.57, 0.19), at(0.63, 0.09));
    spoon.close();
    if let Ok(path) = spoon.build() {
        window.paint_path(path, color);
    }
}
