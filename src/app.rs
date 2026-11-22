use gpui_kit::component::{
    button::{Button, ButtonVariants},
    label::Label,
};
use gpui_kit::*;

use crate::services::updates::{UpdateAction, UpdateState};

pub struct DesktopApp {
    updates: UpdateState,
    update_task: Option<Task<()>>,
}

impl DesktopApp {
    pub fn new(cx: &mut Context<Self>) -> Self {
        let mut app = Self {
            updates: UpdateState::Unavailable,
            update_task: None,
        };
        if let Some(repository) = option_env!("DYLAN_UPDATE_REPO") {
            app.run_update_action(UpdateAction::Check(repository), cx);
        }
        app
    }

    fn run_update_action(&mut self, action: UpdateAction, cx: &mut Context<Self>) {
        self.updates = UpdateState::Busy(action.status());
        cx.notify();

        let work = cx.background_executor().spawn(async move { action.run() });
        self.update_task = Some(cx.spawn(async move |this, cx| {
            let state = work.await.unwrap_or_else(|error| {
                eprintln!("Velopack: {error}");
                UpdateState::Failed
            });
            let _ = this.update(cx, |app, cx| {
                if matches!(state, UpdateState::Restarting) {
                    cx.quit();
                }
                app.updates = state;
                cx.notify();
            });
        }));
    }
}

impl Render for DesktopApp {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let action = match &self.updates {
            UpdateState::Available(update) => {
                Some(("Descargar", UpdateAction::Download(update.clone())))
            }
            UpdateState::Downloaded(update) => Some((
                "Instalar y reiniciar",
                UpdateAction::Install(update.clone()),
            )),
            UpdateState::Current | UpdateState::Failed => option_env!("DYLAN_UPDATE_REPO")
                .map(|repo| ("Buscar actualizaciones", UpdateAction::Check(repo))),
            _ => None,
        };

        div()
            .flex()
            .flex_col()
            .size_full()
            .items_center()
            .justify_center()
            .gap_2()
            .child(Label::new("Dylan · Comedor"))
            .child(Label::new(concat!("Versión ", env!("CARGO_PKG_VERSION"))))
            .child(Label::new(self.updates.message()))
            .children(action.map(|(label, action)| {
                Button::new("update-action")
                    .primary()
                    .label(label)
                    .on_click(cx.listener(move |app, _, _, cx| {
                        if !matches!(app.updates, UpdateState::Busy(_) | UpdateState::Restarting) {
                            app.run_update_action(action.clone(), cx);
                        }
                    }))
            }))
    }
}
