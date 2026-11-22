use gpui_kit::{component::v_flex, *};

use crate::{
    components::{
        background::{app_background, app_workspace_background},
        title_bar::app_title_bar,
    },
    features::{auth::AuthView, dashboard::DashboardView, updates::UpdateView},
    state::session::SessionState,
    theme::{ActiveDylanTheme as _, ThemeStore},
};

actions!(desktop, [CheckForUpdates]);

pub struct DesktopApp {
    focus: FocusHandle,
    session: Entity<SessionState>,
    authenticated: bool,
    updates: Entity<UpdateView>,
    updates_visible: bool,
    theme: Entity<ThemeStore>,
    auth: Entity<AuthView>,
    dashboard: Entity<DashboardView>,
    _subscriptions: [Subscription; 3],
}

impl DesktopApp {
    pub fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        let focus = cx.focus_handle();
        window.focus(&focus, cx);

        let session = cx.new(|_| SessionState::new());
        let updates = cx.new(UpdateView::new);
        let theme = cx.new(|cx| ThemeStore::new(window, cx));
        let auth = cx.new(|cx| AuthView::new(session.clone(), window, cx));
        let dashboard = cx.new(|cx| DashboardView::new(session.clone(), cx));

        let observe_session = cx.observe_in(&session, window, |app, session, window, cx| {
            let now_authenticated = session.read(cx).can_access_dashboard();
            if app.authenticated && !now_authenticated {
                app.auth
                    .update(cx, |auth, cx| auth.on_signed_out(window, cx));
            }
            if app.authenticated != now_authenticated {
                app.authenticated = now_authenticated;
                cx.notify();
            }
        });
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
            authenticated: session.read(cx).can_access_dashboard(),
            session,
            updates_visible: updates.read(cx).is_visible(),
            updates,
            theme,
            auth,
            dashboard,
            _subscriptions: [observe_session, observe_updates, observe_theme],
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

        let body = if self.session.read(cx).can_access_dashboard() {
            app_workspace_background(&theme, self.dashboard.clone()).into_any_element()
        } else {
            app_background(visuals, self.auth.clone()).into_any_element()
        };

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
            .child(body)
    }
}
