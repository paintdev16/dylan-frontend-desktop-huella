#![allow(dead_code)]

use std::rc::Rc;

use gpui_kit::{
    component::{
        Disableable as _, IconName, Sizable as _, Size,
        button::{Button, ButtonCustomVariant, ButtonVariants as _},
    },
    prelude::FluentBuilder as _,
    *,
};

use crate::theme::{ActiveDylanTheme as _, MetricAccent};

/// Variantes semánticas de botones definidas por la guía visual del comedor.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum AppButtonVariant {
    /// Acción prioritaria en violeta sólido (ej. Confirmar atención).
    #[default]
    Primary,
    /// Variante con degradado violeta→azul reservada para Iniciar sesión en Acceso.
    PrimaryGradient,
    /// Superficie neutra con borde sutil para acciones de apoyo (ej. Ver detalle, Exportar).
    Secondary,
    /// Texto o icono sobre fondo transparente para acciones discretas (ej. Volver, Limpiar filtros).
    Tertiary,
    /// Rojo para acciones con consecuencia (ej. Anular atención).
    Destructive,
    /// Acento suave de categoría ligado a una tarjeta o métrica (ej. Ver tickets en azul).
    Contextual(MetricAccent),
}

type ClickHandler = Rc<dyn Fn(&ClickEvent, &mut Window, &mut App)>;

/// Botón compartido que aplica las variantes semánticas y estados de puntero,
/// foco de teclado, presión y deshabilitado definidos en `src/theme/`.
#[derive(IntoElement)]
pub struct AppButton {
    id: ElementId,
    variant: AppButtonVariant,
    size: Size,
    label: Option<SharedString>,
    icon: Option<IconName>,
    tooltip: Option<SharedString>,
    disabled: bool,
    loading: bool,
    full_width: bool,
    on_click: Option<ClickHandler>,
}

impl AppButton {
    pub fn new(id: impl Into<ElementId>) -> Self {
        Self {
            id: id.into(),
            variant: AppButtonVariant::Primary,
            size: Size::Medium,
            label: None,
            icon: None,
            tooltip: None,
            disabled: false,
            loading: false,
            full_width: false,
            on_click: None,
        }
    }

    pub fn variant(mut self, variant: AppButtonVariant) -> Self {
        self.variant = variant;
        self
    }

    pub fn primary(self) -> Self {
        self.variant(AppButtonVariant::Primary)
    }

    pub fn primary_gradient(self) -> Self {
        self.variant(AppButtonVariant::PrimaryGradient)
    }

    pub fn secondary(self) -> Self {
        self.variant(AppButtonVariant::Secondary)
    }

    pub fn tertiary(self) -> Self {
        self.variant(AppButtonVariant::Tertiary)
    }

    pub fn destructive(self) -> Self {
        self.variant(AppButtonVariant::Destructive)
    }

    pub fn contextual(self, accent: MetricAccent) -> Self {
        self.variant(AppButtonVariant::Contextual(accent))
    }

    pub fn label(mut self, label: impl Into<SharedString>) -> Self {
        self.label = Some(label.into());
        self
    }

    pub fn icon(mut self, icon: IconName) -> Self {
        self.icon = Some(icon);
        self
    }

    pub fn tooltip(mut self, tooltip: impl Into<SharedString>) -> Self {
        self.tooltip = Some(tooltip.into());
        self
    }

    pub fn with_size(mut self, size: Size) -> Self {
        self.size = size;
        self
    }

    pub fn xsmall(self) -> Self {
        self.with_size(Size::XSmall)
    }

    pub fn small(self) -> Self {
        self.with_size(Size::Small)
    }

    pub fn large(self) -> Self {
        self.with_size(Size::Large)
    }

    pub fn disabled(mut self, disabled: bool) -> Self {
        self.disabled = disabled;
        self
    }

    pub fn loading(mut self, loading: bool) -> Self {
        self.loading = loading;
        self
    }

    pub fn full_width(mut self) -> Self {
        self.full_width = true;
        self
    }

    pub fn on_click(
        mut self,
        handler: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_click = Some(Rc::new(handler));
        self
    }
}

impl RenderOnce for AppButton {
    fn render(self, _: &mut Window, cx: &mut App) -> impl IntoElement {
        let dylan = *cx.dylan_theme();

        let mut button = Button::new(self.id)
            .with_size(self.size)
            .disabled(self.disabled)
            .loading(self.loading);

        button = match self.variant {
            AppButtonVariant::Primary => button.primary(),
            AppButtonVariant::PrimaryGradient => button.primary().when(!self.disabled, |this| {
                this.bg(dylan.palette.brand.login_button_gradient())
            }),
            AppButtonVariant::Secondary => button,
            AppButtonVariant::Tertiary => button.ghost(),
            AppButtonVariant::Destructive => button.danger(),
            AppButtonVariant::Contextual(accent) => {
                let tone = dylan.metric(accent);
                let custom = ButtonCustomVariant::new(cx)
                    .color(tone.border)
                    .foreground(tone.action_foreground)
                    .hover(tone.action_hover)
                    .active(tone.action_active);
                button.custom(custom).outline()
            }
        };

        if self.full_width {
            button = button.w_full();
        }
        if let Some(icon) = self.icon {
            button = button.icon(icon);
        }
        if let Some(label) = self.label {
            button = button.label(label);
        }
        if let Some(tooltip) = self.tooltip {
            button = button.tooltip(tooltip);
        }
        if let Some(on_click) = self.on_click {
            button = button.on_click(move |event, window, cx| on_click(event, window, cx));
        }

        button
    }
}
