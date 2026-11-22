use gpui_kit::*;

use crate::theme::visuals::AppVisuals;

pub fn brand_tile(visuals: AppVisuals, side: Pixels) -> impl IntoElement {
    div()
        .flex()
        .flex_shrink_0()
        .items_center()
        .justify_center()
        .size(side)
        .rounded(side * 0.25)
        .bg(visuals.brand_background)
        .border_1()
        .border_color(visuals.brand_border)
        .shadow(vec![
            BoxShadow::new(px(0.0), side * 0.10, visuals.glow.alpha(0.48)).blur_radius(side * 0.28),
        ])
        .child(
            canvas(
                |_, _, _| (),
                move |bounds, (), window, _| {
                    paint_cutlery(window, bounds, visuals.brand_foreground, false);
                },
            )
            .size(side * 0.62),
        )
}

pub fn paint_cutlery(window: &mut Window, bounds: Bounds<Pixels>, color: Hsla, silhouette: bool) {
    if silhouette {
        paint_cutlery_silhouette_rotated(window, bounds, color, -0.14);
    } else {
        paint_cutlery_icon(window, bounds, color);
    }
}

/// Silueta sólida continua rotada en radianes para patrones de fondo.
pub fn paint_cutlery_silhouette_rotated(
    window: &mut Window,
    bounds: Bounds<Pixels>,
    color: Hsla,
    angle_rad: f32,
) {
    paint_cutlery_silhouette(window, bounds, color, angle_rad);
}

/// Icono limpio de tenedor y cuchara con trazo uniforme y puntas redondeadas.
fn paint_cutlery_icon(window: &mut Window, bounds: Bounds<Pixels>, color: Hsla) {
    let at = |x: f32, y: f32| {
        let x = bounds.size.width * (x - 0.5);
        let y = bounds.size.height * (y - 0.5);
        bounds.center() + point(x, y)
    };
    let stroke = bounds.size.width * 0.068;

    let mut fork = PathBuilder::stroke(stroke);
    // Diente izquierdo, base en U simétrica y diente derecho (sin barra horizontal interna)
    fork.move_to(at(0.14, 0.12));
    fork.line_to(at(0.14, 0.35));
    fork.cubic_bezier_to(at(0.275, 0.50), at(0.14, 0.445), at(0.20, 0.50));
    fork.cubic_bezier_to(at(0.41, 0.35), at(0.35, 0.50), at(0.41, 0.445));
    fork.line_to(at(0.41, 0.12));
    // Diente central continuo hasta el mango
    fork.move_to(at(0.275, 0.12));
    fork.line_to(at(0.275, 0.88));
    if let Ok(path) = fork.build() {
        window.paint_path(path, color);
    }

    let mut spoon = PathBuilder::stroke(stroke);
    // Cuchara ovalada simétrica
    spoon.move_to(at(0.725, 0.11));
    spoon.cubic_bezier_to(at(0.86, 0.31), at(0.805, 0.11), at(0.86, 0.195));
    spoon.cubic_bezier_to(at(0.725, 0.50), at(0.86, 0.43), at(0.80, 0.50));
    spoon.cubic_bezier_to(at(0.59, 0.31), at(0.65, 0.50), at(0.59, 0.43));
    spoon.cubic_bezier_to(at(0.725, 0.11), at(0.59, 0.195), at(0.645, 0.11));
    spoon.close();
    // Mango continuo desde la base de la cuchara
    spoon.move_to(at(0.725, 0.50));
    spoon.line_to(at(0.725, 0.88));
    if let Ok(path) = spoon.build() {
        window.paint_path(path, color);
    }

    // Remates redondeados exactos al grosor del trazo en los extremos abiertos
    for tip in [
        (0.14, 0.12),
        (0.275, 0.12),
        (0.41, 0.12),
        (0.275, 0.88),
        (0.725, 0.88),
    ] {
        paint_dot(window, at(tip.0, tip.1), stroke, color);
    }
}

/// Silueta sólida continua (sin trazos ni círculos superpuestos) para el fondo decorativo.
fn paint_cutlery_silhouette(
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

pub fn paint_dot(window: &mut Window, center: Point<Pixels>, diameter: Pixels, color: Hsla) {
    let radius = diameter * 0.5;
    let bounds = Bounds::new(center - point(radius, radius), size(diameter, diameter));
    window.paint_quad(fill(bounds, color).corner_radii(radius));
}
