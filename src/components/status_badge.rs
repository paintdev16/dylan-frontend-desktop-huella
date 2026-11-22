#![allow(dead_code)]

use gpui_kit::{
    component::{
        Icon, IconName, Sizable as _, StyledExt as _, h_flex, label::Label, spinner::Spinner,
    },
    *,
};

use crate::theme::{ActiveDylanTheme as _, Radii, SemanticTone, Spacing, Typography};

/// Tonos semánticos para estados visibles (siempre acompañados de icono y texto).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StatusTone {
    Success,
    Warning,
    Error,
    Info,
    Neutral,
}

impl StatusTone {
    fn default_icon(self) -> IconName {
        match self {
            Self::Success => IconName::CircleCheck,
            Self::Warning => IconName::TriangleAlert,
            Self::Error => IconName::CircleX,
            Self::Info => IconName::Info,
            Self::Neutral => IconName::Info,
        }
    }

    fn resolve(self, cx: &App) -> SemanticTone {
        let palette = cx.dylan_theme().palette;
        match self {
            Self::Success => palette.success,
            Self::Warning => palette.warning,
            Self::Error => palette.error,
            Self::Info => palette.info,
            Self::Neutral => palette.neutral,
        }
    }
}

/// Etiqueta de estado con icono o spinner y contraste garantizado en claro y oscuro.
#[derive(IntoElement)]
pub struct StatusBadge {
    tone: StatusTone,
    label: SharedString,
    icon: Option<IconName>,
    loading: bool,
}

impl StatusBadge {
    pub fn new(tone: StatusTone, label: impl Into<SharedString>) -> Self {
        Self {
            tone,
            label: label.into(),
            icon: None,
            loading: false,
        }
    }

    pub fn icon(mut self, icon: IconName) -> Self {
        self.icon = Some(icon);
        self
    }

    pub fn loading(mut self, loading: bool) -> Self {
        self.loading = loading;
        self
    }
}

impl RenderOnce for StatusBadge {
    fn render(self, _: &mut Window, cx: &mut App) -> impl IntoElement {
        let tone = self.tone.resolve(cx);
        let icon = self.icon.unwrap_or_else(|| self.tone.default_icon());

        h_flex()
            .items_center()
            .gap(Spacing::XS)
            .px(Spacing::SM)
            .py(Spacing::XXS)
            .rounded(Radii::FULL)
            .bg(tone.tint)
            .border_1()
            .border_color(tone.border)
            .child(if self.loading {
                Spinner::new().xsmall().color(tone.icon).into_any_element()
            } else {
                Icon::new(icon)
                    .xsmall()
                    .text_color(tone.icon)
                    .into_any_element()
            })
            .child(
                Label::new(self.label)
                    .text_size(Typography::XS)
                    .font_medium()
                    .text_color(tone.foreground),
            )
    }
}
