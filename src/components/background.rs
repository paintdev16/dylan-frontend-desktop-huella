use gpui_kit::{component::v_flex, *};

use crate::{
    components::brand::paint_cutlery_silhouette_rotated,
    theme::{DylanTheme, visuals::AppVisuals},
};

/// Fondo minimalista para pantallas de portada y Acceso: degradado suave
/// más patrón sutil de cubiertos pequeños en toda la superficie.
pub fn app_background(visuals: AppVisuals, content: impl IntoElement) -> impl IntoElement {
    v_flex()
        .relative()
        .flex_1()
        .min_h_0()
        .w_full()
        .overflow_hidden()
        .bg(visuals.background)
        .child(cutlery_pattern_backdrop(
            Some(visuals.ambient_light),
            visuals.watermark.alpha(0.085),
        ))
        .child(content)
}

/// Fondo neutro para el panel administrativo (Dashboard, Consumos, Caja, Reportes):
/// lienzo limpio del tema + patrón de cubiertos pequeños muy sutil (3%).
#[allow(dead_code)]
pub fn app_workspace_background(theme: &DylanTheme, content: impl IntoElement) -> impl IntoElement {
    let visuals = theme.visuals();

    v_flex()
        .relative()
        .flex_1()
        .min_h_0()
        .w_full()
        .overflow_hidden()
        .bg(theme.palette.background)
        .child(cutlery_pattern_backdrop(
            None,
            visuals.watermark.alpha(0.03),
        ))
        .child(content)
}

fn cutlery_pattern_backdrop(
    ambient_light: Option<Background>,
    watermark: Hsla,
) -> impl IntoElement {
    canvas(
        |_, _, _| (),
        move |bounds, (), window, _| {
            if let Some(light) = ambient_light {
                window.paint_quad(fill(bounds, light));
            }

            let cell_x = px(124.0);
            let cell_y = px(112.0);
            let icon_h = px(42.0);
            let icon_w = icon_h * 0.76;

            let cols = (bounds.size.width / cell_x).ceil() as i32 + 1;
            let rows = (bounds.size.height / cell_y).ceil() as i32 + 1;

            for row in -1..=rows {
                let row_offset = if row.rem_euclid(2) == 1 {
                    cell_x * 0.5
                } else {
                    px(0.0)
                };

                for col in -1..=cols {
                    let center_x = bounds.origin.x + cell_x * col as f32 + row_offset;
                    let center_y = bounds.origin.y + cell_y * row as f32;
                    let angle = if (row + col).rem_euclid(2) == 0 {
                        -0.28_f32
                    } else {
                        0.22_f32
                    };
                    let origin = point(center_x - icon_w * 0.5, center_y - icon_h * 0.5);
                    paint_cutlery_silhouette_rotated(
                        window,
                        Bounds::new(origin, size(icon_w, icon_h)),
                        watermark,
                        angle,
                    );
                }
            }
        },
    )
    .absolute()
    .size_full()
    .top_0()
    .left_0()
}
