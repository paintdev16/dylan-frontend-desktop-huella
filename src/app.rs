use gpui_kit::{
    component::{WindowExt as _, h_flex, notification::Notification, v_flex},
    prelude::FluentBuilder as _,
    *,
};

use crate::{
    components::{
        background::{app_background, app_workspace_background},
        sidebar::AppSidebar,
        status_badge::StatusTone,
        title_bar::{TitleBarSlots, account_menu, app_title_bar, sidebar_toggle},
    },
    features::{
        auth::AuthView,
        cash::{CashEvent, CashView},
        consumptions::{
            ConsumptionsEvent, ConsumptionsView,
            attendance::{AttendanceModal, AttendanceModalEvent, AttendanceOrigin},
        },
        dashboard::{DashboardEvent, DashboardView},
        identification::{IdentificationEvent, IdentificationView},
        pensioners::{
            PensionersEvent, PensionersView, SelectedPensioner, SelectionSource,
            enrollment::{EnrollmentModal, EnrollmentModalEvent},
            global_search::{GlobalPensionerSearch, GlobalSearchEvent},
            model::PensionerId,
        },
        reports::ReportsView,
        settings::SettingsView,
        updates::UpdateView,
    },
    services::{
        api::auth,
        preferences::{PreferencesEvent, PreferencesStore},
    },
    state::{
        reader::{ReaderEvent, ReaderState},
        session::{AppRoute, SessionState},
        store::ComedorStore,
    },
    theme::{ActiveDylanTheme as _, ThemeStore},
};

actions!(desktop, [CheckForUpdates, FocusGlobalSearch]);

/// Resuelve la ruta destino respetando la sesión y los permisos efectivos del operador.
fn resolve_navigation(session: &SessionState, current: AppRoute, target: AppRoute) -> AppRoute {
    if session.can_access_route(target) {
        target
    } else if session.can_access_route(current) {
        current
    } else {
        session
            .available_routes()
            .first()
            .copied()
            .unwrap_or(AppRoute::Dashboard)
    }
}

/// Determina si el atajo `Ctrl+K` puede enfocar **Buscar pensionista**.
/// Queda inactivo en Acceso, sin permiso de búsqueda o cuando Atención automática está abierta.
fn can_focus_global_search(session: &SessionState, modal_open: bool) -> bool {
    session.is_authenticated() && session.can_search_pensioners() && !modal_open
}

/// Determina si se puede abrir Atención automática (solo una abierta a la vez y con permiso).
fn can_open_attendance_modal(session: &SessionState, modal_open: bool) -> bool {
    session.is_authenticated() && session.can_register_attendance() && !modal_open
}

pub struct DesktopApp {
    focus: FocusHandle,
    session: Entity<SessionState>,
    authenticated: bool,
    reader: Entity<ReaderState>,
    store: Entity<ComedorStore>,
    selected_pensioner: Option<SelectedPensioner>,
    route: AppRoute,
    sidebar_collapsed: bool,
    modal_open: bool,
    global_search: Entity<GlobalPensionerSearch>,
    attendance_modal: Entity<AttendanceModal>,
    enrollment_modal: Entity<EnrollmentModal>,
    updates: Entity<UpdateView>,
    updates_visible: bool,
    preferences: Entity<PreferencesStore>,
    theme: Entity<ThemeStore>,
    auth: Entity<AuthView>,
    dashboard: Entity<DashboardView>,
    pensioners: Entity<PensionersView>,
    identification: Entity<IdentificationView>,
    consumptions: Entity<ConsumptionsView>,
    cash: Entity<CashView>,
    reports: Entity<ReportsView>,
    settings: Entity<SettingsView>,
    _subscriptions: Vec<Subscription>,
}

impl DesktopApp {
    pub fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        let focus = cx.focus_handle();
        window.focus(&focus, cx);

        let store = cx.new(|_| ComedorStore::new());
        let preferences = cx.new(PreferencesStore::new);
        let session = cx.new(|_| SessionState::new());
        let reader = cx.new(|_| ReaderState::new_simulated());
        let global_search = cx.new(|cx| GlobalPensionerSearch::new(store.clone(), window, cx));
        let attendance_modal =
            cx.new(|cx| AttendanceModal::new(store.clone(), session.clone(), window, cx));
        let enrollment_modal = cx.new(|cx| EnrollmentModal::new(store.clone(), reader.clone(), cx));
        let updates = cx.new(UpdateView::new);
        let theme = cx.new(|cx| ThemeStore::new(window, cx));
        let auth = cx.new(|cx| AuthView::new(session.clone(), preferences.clone(), window, cx));
        let dashboard =
            cx.new(|cx| DashboardView::new(session.clone(), reader.clone(), store.clone(), cx));
        let pensioners = cx.new(|cx| {
            PensionersView::new(store.clone(), session.clone(), reader.clone(), window, cx)
        });
        let identification = cx
            .new(|cx| IdentificationView::new(store.clone(), session.clone(), reader.clone(), cx));
        let consumptions = cx.new(|cx| ConsumptionsView::new(store.clone(), session.clone(), cx));
        let cash = cx.new(|cx| CashView::new(store.clone(), session.clone(), window, cx));
        let reports = cx.new(|cx| ReportsView::new(store.clone(), window, cx));
        let settings = cx.new(|cx| {
            SettingsView::new(
                preferences.clone(),
                theme.clone(),
                reader.clone(),
                window,
                cx,
            )
        });

        let observe_session = cx.observe_in(&session, window, |app, session, window, cx| {
            let session_ref = session.read(cx);
            let now_authenticated =
                session_ref.is_authenticated() && !session_ref.available_routes().is_empty();

            if app.authenticated && !now_authenticated {
                window.close_all_dialogs(cx);
                app.route = AppRoute::Dashboard;
                app.set_modal_open(false, cx);
                app.attendance_modal
                    .update(cx, |modal, cx| modal.close_for_sign_out(cx));
                app.enrollment_modal
                    .update(cx, |modal, cx| modal.close_for_sign_out(cx));
                app.clear_selection(cx);
                app.pensioners
                    .update(cx, |view, cx| view.reset_list(window, cx));
                app.global_search
                    .update(cx, |search, cx| search.reset(window, cx));
                app.reader
                    .update(cx, |reader, cx| reader.reset_for_sign_out(cx));
                app.dashboard.update(cx, |dashboard, cx| {
                    dashboard.set_session_sync(false, None, cx)
                });
                app.auth
                    .update(cx, |auth, cx| auth.on_signed_out(window, cx));
            } else if now_authenticated && !session_ref.can_access_route(app.route) {
                app.route = resolve_navigation(session_ref, app.route, app.route);
            }

            if app.authenticated != now_authenticated {
                app.authenticated = now_authenticated;
                cx.notify();
            }
        });

        let observe_store = cx.observe(&store, |_, _, cx| cx.notify());
        let observe_reader = cx.observe(&reader, |_, _, cx| cx.notify());
        let subscribe_reader = cx.subscribe_in(
            &reader,
            window,
            |app, _, event: &ReaderEvent, window, cx| match *event {
                ReaderEvent::Identified(id) => {
                    if app.modal_open
                        || window.has_active_dialog(cx)
                        || !app
                            .session
                            .read(cx)
                            .can_access_route(AppRoute::Identification)
                        || !app
                            .store
                            .read(cx)
                            .ledger()
                            .find_pensioner(id)
                            .is_some_and(|p| p.active)
                    {
                        return;
                    }
                    app.select_pensioner(id, SelectionSource::BiometricMatch, cx);
                    app.open_attendance_modal(id, AttendanceOrigin::Identification, window, cx);
                }
                ReaderEvent::Enrolled(id) => {
                    app.store
                        .update(cx, |store, cx| store.set_fingerprint_enrolled(id, true, cx));
                }
            },
        );

        let subscribe_global_search = cx.subscribe_in(
            &global_search,
            window,
            |app, _, event: &GlobalSearchEvent, window, cx| {
                let GlobalSearchEvent::Selected(id) = *event;
                if !can_focus_global_search(app.session.read(cx), app.modal_open)
                    || window.has_active_dialog(cx)
                    || !app
                        .store
                        .read(cx)
                        .ledger()
                        .find_pensioner(id)
                        .is_some_and(|p| p.active)
                {
                    return;
                }
                app.select_pensioner(id, SelectionSource::GlobalSearch, cx);
                app.open_attendance_modal(id, AttendanceOrigin::GlobalSearch, window, cx);
            },
        );

        let subscribe_attendance_modal = cx.subscribe_in(
            &attendance_modal,
            window,
            |app, _, event: &AttendanceModalEvent, window, cx| {
                let origin = match event {
                    AttendanceModalEvent::Cancelled { origin } => *origin,
                    AttendanceModalEvent::Confirmed { origin, record } => {
                        window.push_notification(
                            Notification::success(format!(
                                "Atención confirmada · {} raciones · {} tickets",
                                record.rations(),
                                record.tickets_count,
                            )),
                            cx,
                        );
                        *origin
                    }
                };
                app.set_modal_open(false, cx);
                app.restore_origin_focus(origin, window, cx);
            },
        );

        let subscribe_enrollment_modal = cx.subscribe_in(
            &enrollment_modal,
            window,
            |app, _, _: &EnrollmentModalEvent, window, cx| {
                app.set_modal_open(false, cx);
                app.pensioners.update(cx, |view, cx| view.focus(window, cx));
            },
        );

        let subscribe_pensioners = cx.subscribe_in(
            &pensioners,
            window,
            |app, _, event: &PensionersEvent, window, cx| match *event {
                PensionersEvent::Selected(id) => {
                    app.select_pensioner(id, SelectionSource::ManualSearch, cx);
                }
                PensionersEvent::Cleared => app.clear_selection(cx),
                PensionersEvent::OpenEnrollment(id) => {
                    if !app.modal_open
                        && !window.has_active_dialog(cx)
                        && app.session.read(cx).pensioner_permissions().enroll
                        && app
                            .store
                            .read(cx)
                            .ledger()
                            .find_pensioner(id)
                            .is_some_and(|pensioner| pensioner.can_enroll_fingerprint())
                    {
                        let opened = app
                            .enrollment_modal
                            .update(cx, |modal, cx| modal.open(id, window, cx));
                        if opened {
                            app.set_modal_open(true, cx);
                        }
                    }
                }
            },
        );

        let subscribe_identification = cx.subscribe_in(
            &identification,
            window,
            |app, _, event: &IdentificationEvent, window, cx| match *event {
                IdentificationEvent::OpenPensioner(id) => {
                    if app.session.read(cx).can_access_route(AppRoute::Pensioners)
                        && !app.modal_open
                    {
                        app.select_pensioner(id, SelectionSource::BiometricMatch, cx);
                        app.navigate(AppRoute::Pensioners, cx);
                    }
                }
                IdentificationEvent::FocusSearch => {
                    if can_focus_global_search(app.session.read(cx), app.modal_open)
                        && !window.has_active_dialog(cx)
                    {
                        app.global_search
                            .update(cx, |search, cx| search.focus(window, cx));
                    }
                }
            },
        );

        let subscribe_consumptions = cx.subscribe(
            &consumptions,
            |app, _, event: &ConsumptionsEvent, cx| match *event {
                ConsumptionsEvent::OpenIdentification => {
                    app.navigate(AppRoute::Identification, cx);
                }
                ConsumptionsEvent::OpenPensioners => {
                    app.navigate(AppRoute::Pensioners, cx);
                }
                ConsumptionsEvent::OpenCash => {
                    app.navigate(AppRoute::Cash, cx);
                }
            },
        );

        let subscribe_cash = cx.subscribe(&cash, |app, _, event: &CashEvent, cx| match *event {
            CashEvent::OpenPensioner(id) => {
                if app.session.read(cx).can_access_route(AppRoute::Pensioners) {
                    app.select_pensioner(id, SelectionSource::ManualSearch, cx);
                    app.navigate(AppRoute::Pensioners, cx);
                }
            }
        });

        let subscribe_dashboard =
            cx.subscribe(
                &dashboard,
                |app, _, event: &DashboardEvent, cx| match *event {
                    DashboardEvent::OpenPensioners => app.navigate(AppRoute::Pensioners, cx),
                    DashboardEvent::OpenIdentification => {
                        app.navigate(AppRoute::Identification, cx)
                    }
                    DashboardEvent::OpenConsumptions => app.navigate(AppRoute::Consumptions, cx),
                    DashboardEvent::OpenCash => app.navigate(AppRoute::Cash, cx),
                    DashboardEvent::OpenReports => app.navigate(AppRoute::Reports, cx),
                    DashboardEvent::OpenSettings => app.navigate(AppRoute::Settings, cx),
                    DashboardEvent::RefreshSession => app.refresh_session_from_backend(cx),
                },
            );

        let subscribe_preferences = cx.subscribe_in(
            &preferences,
            window,
            |app, _, event: &PreferencesEvent, window, cx| {
                let PreferencesEvent::Loaded(prefs) = event;
                let saved_theme = prefs.theme;
                if app.theme.read(cx).preference() != saved_theme {
                    app.theme.update(cx, |theme, cx| {
                        theme.set_preference(saved_theme, window, cx);
                    });
                }
                let bio_mode = prefs.biometric_mode;
                let bio_config = prefs.biometric_config();
                app.reader.update(cx, |reader, cx| {
                    reader.configure_backend(bio_mode, bio_config, cx);
                });
            },
        );

        let observe_updates = cx.observe(&updates, |app, updates, cx| {
            let visible = updates.read(cx).is_visible();
            if app.updates_visible != visible {
                app.updates_visible = visible;
                cx.notify();
            }
        });
        let observe_theme = cx.observe(&theme, |app, theme, cx| {
            let pref = theme.read(cx).preference();
            app.preferences
                .update(cx, |store, cx| store.set_theme(pref, cx));
            cx.notify();
        });

        let authenticated = {
            let session_ref = session.read(cx);
            session_ref.is_authenticated() && !session_ref.available_routes().is_empty()
        };

        Self {
            focus,
            authenticated,
            session,
            reader,
            store,
            selected_pensioner: None,
            route: AppRoute::Dashboard,
            sidebar_collapsed: false,
            modal_open: false,
            global_search,
            attendance_modal,
            enrollment_modal,
            updates_visible: updates.read(cx).is_visible(),
            updates,
            preferences,
            theme,
            auth,
            dashboard,
            pensioners,
            identification,
            consumptions,
            cash,
            reports,
            settings,
            _subscriptions: vec![
                observe_session,
                observe_store,
                observe_reader,
                subscribe_reader,
                subscribe_global_search,
                subscribe_attendance_modal,
                subscribe_enrollment_modal,
                subscribe_pensioners,
                subscribe_identification,
                subscribe_consumptions,
                subscribe_cash,
                subscribe_dashboard,
                subscribe_preferences,
                observe_updates,
                observe_theme,
            ],
        }
    }

    pub fn can_close(&self, cx: &App) -> bool {
        !self.updates.read(cx).is_installing()
    }

    fn navigate(&mut self, route: AppRoute, cx: &mut Context<Self>) {
        let next = resolve_navigation(self.session.read(cx), self.route, route);
        if self.route != next {
            self.route = next;
            cx.notify();
        }
    }

    fn toggle_sidebar(&mut self, cx: &mut Context<Self>) {
        self.sidebar_collapsed = !self.sidebar_collapsed;
        cx.notify();
    }

    fn sign_out(&mut self, cx: &mut Context<Self>) {
        let api_base_url = self.preferences.read(cx).preferences().api_base_url.clone();
        let revoked_token = self.session.update(cx, |session, cx| session.sign_out(cx));
        if let Some(token) = revoked_token {
            cx.background_executor()
                .spawn(async move {
                    let _ = auth::logout(&api_base_url, &token);
                })
                .detach();
        }
    }

    fn refresh_session_from_backend(&mut self, cx: &mut Context<Self>) {
        let Some(token) = self.session.read(cx).access_token().map(str::to_string) else {
            return;
        };
        let api_base_url = self.preferences.read(cx).preferences().api_base_url.clone();
        self.dashboard.update(cx, |dashboard, cx| {
            dashboard.set_session_sync(true, None, cx);
        });

        let background = cx
            .background_executor()
            .spawn(async move { auth::fetch_me(&api_base_url, &token) });

        cx.spawn(async move |this, cx| {
            let result = background.await;
            let _ = this.update(cx, |app, cx| match result {
                Ok(api_user) => {
                    app.session.update(cx, |session, cx| {
                        session.sync_user_from_backend(&api_user, cx);
                    });
                    app.dashboard.update(cx, |dashboard, cx| {
                        dashboard.set_session_sync(
                            false,
                            Some((
                                StatusTone::Success,
                                SharedString::from("Permisos verificados con el servidor."),
                            )),
                            cx,
                        );
                    });
                }
                Err(err) if err.is_unauthorized() => {
                    app.dashboard.update(cx, |dashboard, cx| {
                        dashboard.set_session_sync(false, None, cx);
                    });
                    app.session.update(cx, |session, cx| {
                        session.expire_session(err.user_message(), cx);
                    });
                }
                Err(err) => {
                    let tone = if err.is_warning_tone() {
                        StatusTone::Warning
                    } else {
                        StatusTone::Error
                    };
                    app.dashboard.update(cx, |dashboard, cx| {
                        dashboard.set_session_sync(
                            false,
                            Some((tone, SharedString::from(err.user_message()))),
                            cx,
                        );
                    });
                }
            });
        })
        .detach();
    }

    fn set_modal_open(&mut self, modal_open: bool, cx: &mut Context<Self>) {
        if self.modal_open != modal_open {
            self.modal_open = modal_open;
            self.identification
                .update(cx, |view, cx| view.set_modal_open(modal_open, cx));
            cx.notify();
        }
    }

    fn open_attendance_modal(
        &mut self,
        pensioner_id: PensionerId,
        origin: AttendanceOrigin,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if !can_open_attendance_modal(self.session.read(cx), self.modal_open)
            || window.has_active_dialog(cx)
        {
            return;
        }
        let opened = self
            .attendance_modal
            .update(cx, |modal, cx| modal.open(pensioner_id, origin, window, cx));
        if opened {
            self.set_modal_open(true, cx);
        }
    }

    fn restore_origin_focus(
        &self,
        origin: AttendanceOrigin,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        match origin {
            AttendanceOrigin::GlobalSearch => {
                if self.session.read(cx).can_search_pensioners() {
                    self.global_search
                        .update(cx, |search, cx| search.restore_focus(window, cx));
                } else {
                    window.focus(&self.focus, cx);
                }
            }
            AttendanceOrigin::Identification => {
                self.identification
                    .update(cx, |view, cx| view.focus(window, cx));
            }
        }
    }

    fn select_pensioner(
        &mut self,
        id: PensionerId,
        source: SelectionSource,
        cx: &mut Context<Self>,
    ) {
        if self.store.read(cx).ledger().find_pensioner(id).is_none() {
            return;
        }
        let selection = Some(SelectedPensioner { id, source });
        self.selected_pensioner = selection;
        self.sync_selection_to_views(selection, cx);
        cx.notify();
    }

    fn clear_selection(&mut self, cx: &mut Context<Self>) {
        self.selected_pensioner = None;
        self.sync_selection_to_views(None, cx);
        cx.notify();
    }

    fn sync_selection_to_views(
        &self,
        selection: Option<SelectedPensioner>,
        cx: &mut Context<Self>,
    ) {
        self.dashboard
            .update(cx, |view, cx| view.set_selected(selection, cx));
        self.pensioners
            .update(cx, |view, cx| view.set_selected(selection, cx));
        self.consumptions
            .update(cx, |view, cx| view.set_selected(selection, cx));
    }
}

impl Render for DesktopApp {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = *cx.dylan_theme();
        let visuals = theme.visuals();
        let version = env!("CARGO_PKG_VERSION");

        let (is_authenticated, can_search, available_routes, account_details) = {
            let session = self.session.read(cx);
            (
                session.is_authenticated(),
                session.can_search_pensioners(),
                session.available_routes(),
                session
                    .current_user()
                    .map(|user| (user.name.clone(), user.role.clone())),
            )
        };

        let title_slots = TitleBarSlots {
            sidebar_toggle: (is_authenticated && !available_routes.is_empty()).then(|| {
                let app = cx.weak_entity();
                sidebar_toggle(self.sidebar_collapsed, move |cx| {
                    let _ = app.update(cx, |app, cx| app.toggle_sidebar(cx));
                })
                .into_any_element()
            }),
            search: (is_authenticated && can_search)
                .then(|| self.global_search.clone().into_any_element()),
            update: self
                .updates_visible
                .then(|| self.updates.clone().into_any_element()),
            account: account_details.map(|(name, role)| {
                let app = cx.weak_entity();
                account_menu(name, role, window, move |cx| {
                    let _ = app.update(cx, |app, cx| app.sign_out(cx));
                })
                .into_any_element()
            }),
        };

        let body = if is_authenticated && !available_routes.is_empty() {
            let active_route = if available_routes.contains(&self.route) {
                self.route
            } else {
                available_routes[0]
            };

            let active_view = match active_route {
                AppRoute::Dashboard => self.dashboard.clone().into_any_element(),
                AppRoute::Pensioners => self.pensioners.clone().into_any_element(),
                AppRoute::Identification => self.identification.clone().into_any_element(),
                AppRoute::Consumptions => self.consumptions.clone().into_any_element(),
                AppRoute::Cash => self.cash.clone().into_any_element(),
                AppRoute::Reports => self.reports.clone().into_any_element(),
                AppRoute::Settings => self.settings.clone().into_any_element(),
            };

            let reader_status = self.reader.read(cx).status().clone();
            let selected_chip = self.selected_pensioner.and_then(|sel| {
                self.store
                    .read(cx)
                    .ledger()
                    .find_pensioner(sel.id)
                    .map(|p| SharedString::from(format!("{} · {}", p.code(), p.full_name)))
            });

            let app_for_select = cx.entity();

            let sidebar = AppSidebar::new(
                available_routes,
                active_route,
                self.sidebar_collapsed,
                reader_status,
                move |route, _, cx| {
                    app_for_select.update(cx, |app, cx| app.navigate(route, cx));
                },
            )
            .selected_label(selected_chip);

            h_flex()
                .flex_1()
                .min_h_0()
                .w_full()
                .child(sidebar)
                .child(app_workspace_background(&theme, active_view))
                .into_any_element()
        } else {
            app_background(visuals, self.auth.clone()).into_any_element()
        };

        let modal_open = self.attendance_modal.read(cx).is_open();
        let enrollment_open = self.enrollment_modal.read(cx).is_open();

        v_flex()
            .relative()
            .track_focus(&self.focus)
            .on_action(cx.listener(|app, _: &CheckForUpdates, _, cx| {
                app.updates.update(cx, |updates, cx| updates.check(cx));
            }))
            .on_action(cx.listener(|app, _: &FocusGlobalSearch, window, cx| {
                if can_focus_global_search(app.session.read(cx), app.modal_open)
                    && !window.has_active_dialog(cx)
                {
                    app.global_search
                        .update(cx, |search, cx| search.focus(window, cx));
                }
            }))
            .size_full()
            .bg(theme.palette.background)
            .text_color(theme.palette.foreground)
            .child(app_title_bar(
                "Comedor Dylan",
                version,
                title_slots,
                self.theme.clone(),
                window,
                cx,
            ))
            .child(body)
            .when(modal_open, |this| this.child(self.attendance_modal.clone()))
            .when(enrollment_open, |this| {
                this.child(self.enrollment_modal.clone())
            })
    }
}

#[cfg(test)]
mod tests {
    use super::{can_focus_global_search, can_open_attendance_modal, resolve_navigation};
    use crate::state::session::{AppRoute, SessionPermissions, SessionState};

    #[test]
    fn ctrl_k_shortcut_is_inactive_on_auth_without_permission_and_when_modal_is_open() {
        let session = SessionState::new();
        assert!(!can_focus_global_search(&session, false));

        let mut admin = SessionState::with_permissions(SessionPermissions::full_access());
        assert!(can_focus_global_search(&admin, false));
        assert!(!can_focus_global_search(&admin, true));

        admin.clear();
        assert!(!can_focus_global_search(&admin, false));

        let without_search = SessionState::with_permissions(SessionPermissions {
            dashboard: true,
            pensioners: true,
            identification: true,
            consumptions: true,
            register_attendance: true,
            cash: true,
            reports: true,
            settings: true,
            search_pensioners: false,
        });
        assert!(!can_focus_global_search(&without_search, false));
    }

    #[test]
    fn attendance_modal_requires_permission_and_single_active_instance() {
        let session = SessionState::new();
        assert!(!can_open_attendance_modal(&session, false));

        let admin = SessionState::with_permissions(SessionPermissions::full_access());
        assert!(can_open_attendance_modal(&admin, false));
        assert!(!can_open_attendance_modal(&admin, true));

        let read_only = SessionState::with_permissions(SessionPermissions {
            dashboard: true,
            pensioners: true,
            identification: true,
            consumptions: true,
            register_attendance: false,
            cash: false,
            reports: true,
            settings: false,
            search_pensioners: true,
        });
        assert!(!can_open_attendance_modal(&read_only, false));
    }

    #[test]
    fn navigation_respects_session_authentication_and_route_permissions() {
        let session = SessionState::new();
        assert_eq!(
            resolve_navigation(&session, AppRoute::Dashboard, AppRoute::Pensioners),
            AppRoute::Dashboard
        );

        let admin = SessionState::with_permissions(SessionPermissions::full_access());
        assert_eq!(
            resolve_navigation(&admin, AppRoute::Dashboard, AppRoute::Consumptions),
            AppRoute::Consumptions
        );
        assert_eq!(
            resolve_navigation(&admin, AppRoute::Consumptions, AppRoute::Cash),
            AppRoute::Cash
        );
        assert_eq!(
            resolve_navigation(&admin, AppRoute::Cash, AppRoute::Reports),
            AppRoute::Reports
        );
        assert_eq!(
            resolve_navigation(&admin, AppRoute::Reports, AppRoute::Settings),
            AppRoute::Settings
        );

        let limited = SessionState::with_permissions(SessionPermissions {
            dashboard: true,
            pensioners: false,
            identification: true,
            consumptions: false,
            register_attendance: false,
            cash: false,
            reports: false,
            settings: false,
            search_pensioners: false,
        });
        assert_eq!(
            resolve_navigation(&limited, AppRoute::Dashboard, AppRoute::Cash),
            AppRoute::Dashboard
        );
        assert_eq!(
            resolve_navigation(&limited, AppRoute::Dashboard, AppRoute::Reports),
            AppRoute::Dashboard
        );
        assert_eq!(
            resolve_navigation(&limited, AppRoute::Dashboard, AppRoute::Settings),
            AppRoute::Dashboard
        );
        assert_eq!(
            resolve_navigation(&limited, AppRoute::Pensioners, AppRoute::Pensioners),
            AppRoute::Dashboard
        );
        assert_eq!(
            resolve_navigation(&limited, AppRoute::Dashboard, AppRoute::Identification),
            AppRoute::Identification
        );
    }
}
