#![allow(dead_code)]

use gpui_kit::{
    component::{Icon, IconName, Sizable as _, StyledExt as _, h_flex, label::Label, v_flex},
    prelude::FluentBuilder as _,
    *,
};

use crate::theme::{ActiveDylanTheme as _, MetricAccent, Radii, Spacing, Typography};

/// Tarjeta compartida de métricas para Dashboard, Caja y Reportes.
/// Aplica el acento semántico de `MetricAccent` con tinte suave de fondo,
/// borde/icono destacado y alto contraste en etiqueta y cifra.
#[derive(IntoElement)]
pub struct MetricCard {
    accent: MetricAccent,
    label: SharedString,
    value: SharedString,
    subtitle: Option<SharedString>,
    icon: Option<IconName>,
    action: Option<AnyElement>,
}

impl MetricCard {
    pub fn new(
        accent: MetricAccent,
        label: impl Into<SharedString>,
        value: impl Into<SharedString>,
    ) -> Self {
        Self {
            accent,
            label: label.into(),
            value: value.into(),
            subtitle: None,
            icon: None,
            action: None,
        }
    }

    pub fn subtitle(mut self, subtitle: impl Into<SharedString>) -> Self {
        self.subtitle = Some(subtitle.into());
        self
    }

    pub fn icon(mut self, icon: IconName) -> Self {
        self.icon = Some(icon);
        self
    }

    pub fn action(mut self, action: impl IntoElement) -> Self {
        self.action = Some(action.into_any_element());
        self
    }
}

impl RenderOnce for MetricCard {
    fn render(self, _: &mut Window, cx: &mut App) -> impl IntoElement {
        let tone = cx.dylan_theme().metric(self.accent);

        v_flex()
            .flex_1()
            .min_w(px(190.0))
            .max_w_full()
            .p(Spacing::LG)
            .gap(Spacing::SM)
            .rounded(Radii::LG)
            .bg(tone.background)
            .border_1()
            .border_color(tone.border)
            .child(
                h_flex()
                    .items_center()
                    .justify_between()
                    .gap(Spacing::SM)
                    .child(
                        Label::new(self.label)
                            .text_size(Typography::SM)
                            .font_medium()
                            .text_color(tone.label),
                    )
                    .when_some(self.icon, |this, icon| {
                        this.child(
                            div()
                                .flex()
                                .items_center()
                                .justify_center()
                                .size(px(30.0))
                                .rounded(Radii::MD)
                                .bg(tone.icon_background)
                                .child(Icon::new(icon).small().text_color(tone.icon)),
                        )
                    }),
            )
            .child(
                Label::new(self.value)
                    .text_size(Typography::METRIC_VALUE)
                    .font_bold()
                    .text_color(tone.value),
            )
            .when(self.subtitle.is_some() || self.action.is_some(), |this| {
                this.child(
                    h_flex()
                        .items_center()
                        .justify_between()
                        .gap(Spacing::SM)
                        .pt(Spacing::XS)
                        .when_some(self.subtitle, |row, subtitle| {
                            row.child(
                                Label::new(subtitle)
                                    .text_size(Typography::XS)
                                    .text_color(tone.label),
                            )
                        })
                        .when_some(self.action, |row, action| row.child(action)),
                )
            })
    }
}
