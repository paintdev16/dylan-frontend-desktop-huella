use std::{sync::mpsc, time::Duration};

use gpui_kit::{
    component::{IconName, Sizable as _, h_flex, label::Label, progress::Progress},
    *,
};

use crate::{
    components::{
        button::AppButton,
        status_badge::{StatusBadge, StatusTone},
    },
    services::updates::{UpdateAction, UpdateState},
    theme::{ActiveDylanTheme as _, Spacing, Typography},
};

const UPDATE_REPOSITORY: Option<&str> = option_env!("DYLAN_UPDATE_REPO");
const CHECK_INTERVAL: Duration = Duration::from_secs(30 * 60);

pub struct UpdateView {
    state: UpdateState,
    progress: Option<i16>,
    work: Option<Task<()>>,
    progress_task: Option<Task<()>>,
    poll_task: Option<Task<()>>,
}

impl UpdateView {
    pub fn new(cx: &mut Context<Self>) -> Self {
        let mut view = Self {
            state: UpdateState::Unavailable,
            progress: None,
            work: None,
            progress_task: None,
            poll_task: None,
        };
        if let Some(repository) = UPDATE_REPOSITORY {
            view.run(UpdateAction::Check(repository), cx);
            view.poll_task = Some(cx.spawn(async move |this, cx| {
                loop {
                    cx.background_executor().timer(CHECK_INTERVAL).await;
                    if this
                        .update(cx, |view, cx| {
                            view.check(cx);
                        })
                        .is_err()
                    {
                        break;
                    }
                }
            }));
        }
        view
    }

    pub fn is_visible(&self) -> bool {
        !matches!(
            self.state,
            UpdateState::Unavailable | UpdateState::Current | UpdateState::Checking
        )
    }

    pub fn is_installing(&self) -> bool {
        matches!(
            self.state,
            UpdateState::Installing(_) | UpdateState::Restarting
        )
    }

    pub fn check(&mut self, cx: &mut Context<Self>) {
        if matches!(self.state, UpdateState::Current | UpdateState::Failed)
            && let Some(repository) = UPDATE_REPOSITORY
        {
            self.run(UpdateAction::Check(repository), cx);
        }
    }

    fn download(&mut self, cx: &mut Context<Self>) {
        let UpdateState::Available(update) = &self.state else {
            return;
        };
        let (sender, receiver) = mpsc::channel();
        self.progress = None;
        self.run(UpdateAction::Download(update.clone(), sender), cx);
        self.progress_task = Some(cx.spawn(async move |this, cx| {
            loop {
                cx.background_executor()
                    .timer(Duration::from_millis(100))
                    .await;
                if let Some(progress) = receiver.try_iter().last() {
                    let result = this.update(cx, |view, cx| {
                        view.progress = Some(progress.clamp(0, 100));
                        cx.notify();
                    });
                    if result.is_err() {
                        break;
                    }
                }
            }
        }));
    }

    fn run(&mut self, action: UpdateAction, cx: &mut Context<Self>) {
        self.state = action.pending_state();
        cx.notify();

        let task = cx.background_executor().spawn(async move { action.run() });
        self.work = Some(cx.spawn(async move |this, cx| {
            let state = task.await.unwrap_or_else(|error| {
                eprintln!("Velopack: {error}");
                UpdateState::Failed
            });
            let _ = this.update(cx, |view, cx| {
                view.progress_task = None;
                if matches!(state, UpdateState::Restarting) {
                    cx.quit();
                }
                view.state = state;
                cx.notify();
            });
        }));
    }
}

impl Render for UpdateView {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let palette = cx.dylan_theme().palette;

        match &self.state {
            UpdateState::Unavailable | UpdateState::Current | UpdateState::Checking => {
                div().hidden()
            }
            UpdateState::Available(update) => {
                let version = update.version();
                h_flex()
                    .items_center()
                    .gap(Spacing::XS)
                    .child(StatusBadge::new(StatusTone::Info, format!("v{version}")))
                    .child(
                        AppButton::new("download")
                            .primary()
                            .xsmall()
                            .icon(IconName::ArrowDown)
                            .label("Descargar")
                            .tooltip(format!("Nueva versión disponible: Dylan v{version}"))
                            .on_click(cx.listener(|view, _, _, cx| view.download(cx))),
                    )
            }
            UpdateState::Downloading(update) => {
                let version = update.version();
                let status = self.progress.map_or_else(
                    || format!("v{version}"),
                    |progress| format!("v{version} · {progress}%"),
                );
                h_flex()
                    .items_center()
                    .gap(Spacing::SM)
                    .child(
                        Label::new(status)
                            .text_size(Typography::XS)
                            .text_color(palette.muted_foreground),
                    )
                    .child(
                        div().w(px(96.0)).child(
                            Progress::new("update-download")
                                .xsmall()
                                .value(f32::from(self.progress.unwrap_or_default()))
                                .loading(self.progress.is_none())
                                .accessibility_label("Progreso de descarga"),
                        ),
                    )
            }
            UpdateState::Downloaded(update) => {
                let version = update.version();
                h_flex()
                    .items_center()
                    .gap(Spacing::XS)
                    .child(StatusBadge::new(StatusTone::Success, format!("v{version}")))
                    .child(
                        AppButton::new("install")
                            .primary()
                            .xsmall()
                            .icon(IconName::RefreshCw)
                            .label("Instalar actualización")
                            .tooltip(format!("Instalar Dylan v{version} y reiniciar"))
                            .on_click(cx.listener(|view, _, _, cx| {
                                if let UpdateState::Downloaded(update) = &view.state {
                                    view.run(UpdateAction::Install(update.clone()), cx);
                                }
                            })),
                    )
            }
            UpdateState::Installing(update) => h_flex().items_center().child(
                StatusBadge::new(
                    StatusTone::Info,
                    format!("Instalando v{}…", update.version()),
                )
                .loading(true),
            ),
            UpdateState::Restarting => h_flex()
                .items_center()
                .child(StatusBadge::new(StatusTone::Info, "Reiniciando…").loading(true)),
            UpdateState::Failed => h_flex()
                .items_center()
                .gap(Spacing::XS)
                .child(StatusBadge::new(StatusTone::Error, "Actualización fallida"))
                .child(
                    AppButton::new("retry-update")
                        .tertiary()
                        .xsmall()
                        .icon(IconName::RefreshCw)
                        .label("Reintentar")
                        .tooltip("No se pudo completar la actualización")
                        .on_click(cx.listener(|view, _, _, cx| view.check(cx))),
                ),
        }
    }
}
