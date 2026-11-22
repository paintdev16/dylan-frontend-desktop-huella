use gpui_kit::{component::v_flex, *};

use crate::{
    components::{background::app_background, title_bar::app_title_bar},
    features::{updates::UpdateView, welcome::WelcomeView},
    theme::{ActiveDylanTheme as _, ThemeStore},
};

actions!(desktop, [CheckForUpdates]);

pub struct DesktopApp {
    focus: FocusHandle,
    updates: Entity<UpdateView>,
    updates_visible: bool,
    theme: Entity<ThemeStore>,
    welcome: Entity<WelcomeView>,
    _subscriptions: [Subscription; 2],
}

impl DesktopApp {
    pub fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        let focus = cx.focus_handle();
        window.focus(&focus, cx);
        let updates = cx.new(UpdateView::new);
        let theme = cx.new(|cx| ThemeStore::new(window, cx));
        let welcome = cx.new(|cx| WelcomeView::new(window, cx));
        let observe_updates = cx.observe(&updates, |app, updates, cx| {
            let visible = updates.read(cx).is_visible();
            if app.updates_visible != visible {
                app.updates_visible = visible;
                cx.notify();
            }
        });
        let observe_theme = cx.observe(&theme, |_, _, cx| cx.notify());

        Self {
            focus,
            updates_visible: updates.read(cx).is_visible(),
            updates,
            theme,
            welcome,
            _subscriptions: [observe_updates, observe_theme],
        }
    }

    pub fn can_close(&self, cx: &App) -> bool {
        !self.updates.read(cx).is_installing()
    }
}

impl Render for DesktopApp {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = *cx.dylan_theme();
        let visuals = theme.visuals();
        let update_slot = self
            .updates_visible
            .then(|| self.updates.clone().into_any_element());
        let version = env!("CARGO_PKG_VERSION");

        v_flex()
            .track_focus(&self.focus)
            .on_action(cx.listener(|app, _: &CheckForUpdates, _, cx| {
                app.updates.update(cx, |updates, cx| updates.check(cx));
            }))
            .size_full()
            .bg(theme.palette.background)
            .text_color(theme.palette.foreground)
            .child(app_title_bar(
                "Comedor Dylan",
                version,
                update_slot,
                self.theme.clone(),
                window,
                cx,
            ))
            .child(app_background(visuals, self.welcome.clone()))
    }
}
