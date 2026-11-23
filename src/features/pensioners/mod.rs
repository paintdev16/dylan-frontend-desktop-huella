mod detail;
mod editor;
pub mod enrollment;
pub mod global_search;
pub mod model;
mod table;

use gpui_kit::{
    component::{
        Icon, IconName, Sizable as _, StyledExt as _, h_flex,
        input::{Input, InputEvent, InputState},
        label::Label,
        table::{DataTable, TableState},
        v_flex,
    },
    prelude::FluentBuilder as _,
    *,
};

use crate::{
    components::{
        button::AppButton,
        status_badge::{StatusBadge, StatusTone},
    },
    state::{reader::ReaderState, session::SessionState, store::ComedorStore},
    theme::{ActiveDylanTheme as _, Radii, Spacing, Typography},
};

use self::{
    model::{Pensioner, PensionerFilter, PensionerId, search_pensioners},
    table::PensionersTable,
};

/// Origen de la selección actual del pensionista (buscador global, listado o huella).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SelectionSource {
    GlobalSearch,
    ManualSearch,
    BiometricMatch,
}

impl SelectionSource {
    pub const fn label(self) -> &'static str {
        match self {
            Self::GlobalSearch => "Buscador global",
            Self::ManualSearch => "Listado Pensionistas",
            Self::BiometricMatch => "Huella simulada",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SelectedPensioner {
    pub id: PensionerId,
    pub source: SelectionSource,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PensionersEvent {
    Selected(PensionerId),
    Cleared,
    OpenEnrollment(PensionerId),
}

pub struct PensionersView {
    focus: FocusHandle,
    store: Entity<ComedorStore>,
    session: Entity<SessionState>,
    reader: Entity<ReaderState>,
    search_input: Entity<InputState>,
    table: Entity<TableState<PensionersTable>>,
    filter: PensionerFilter,
    selected: Option<SelectedPensioner>,
    _subscriptions: [Subscription; 3],
}

impl EventEmitter<PensionersEvent> for PensionersView {}

impl PensionersView {
    pub fn new(
        store: Entity<ComedorStore>,
        session: Entity<SessionState>,
        reader: Entity<ReaderState>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let focus = cx.focus_handle();
        let search_input = cx.new(|cx| {
            InputState::new(window, cx)
                .placeholder("Buscar por nombre, DNI, código, celular o plan…")
        });
        let rows = store.read(cx).ledger().pensioners().to_vec();
        let owner = cx.weak_entity();
        let table = cx.new(|cx| {
            TableState::new(PensionersTable::new(owner, rows), window, cx)
                .col_movable(false)
                .row_selectable(false)
                .col_selectable(false)
                .cell_selectable(false)
        });

        let observe_store = cx.observe(&store, |view, _, cx| view.sync_table(cx));
        let observe_session = cx.observe(&session, |view, _, cx| view.sync_table(cx));
        let subscription = cx.subscribe(&search_input, |view, _, event, cx| {
            if matches!(event, InputEvent::Change) {
                view.sync_table(cx);
            }
        });

        Self {
            focus,
            store,
            session,
            reader,
            search_input,
            table,
            filter: PensionerFilter::All,
            selected: None,
            _subscriptions: [observe_store, observe_session, subscription],
        }
    }

    pub fn focus(&self, window: &mut Window, cx: &mut App) {
        window.focus(&self.focus, cx);
    }

    pub fn set_selected(&mut self, selected: Option<SelectedPensioner>, cx: &mut Context<Self>) {
        if self.selected != selected {
            self.selected = selected;
            cx.notify();
        }
    }

    pub fn reset_list(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.clear_filters(window, cx);
        self.table.update(cx, |table, cx| {
            table.delegate_mut().hide_balances();
            cx.notify();
        });
    }

    fn clear_filters(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.filter = PensionerFilter::All;
        self.search_input
            .update(cx, |input, cx| input.clean(window, cx));
        self.sync_table(cx);
    }

    fn sync_table(&mut self, cx: &mut Context<Self>) {
        let ledger = self.store.read(cx).ledger();
        let query = self.search_input.read(cx).value();
        let rows = search_pensioners(ledger.pensioners(), query.as_ref(), self.filter)
            .into_iter()
            .cloned()
            .collect();
        let permissions = self.session.read(cx).pensioner_permissions();
        self.table.update(cx, |table, cx| {
            table.delegate_mut().set_rows(rows, permissions);
            cx.notify();
        });
        cx.notify();
    }

    fn render_list(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let palette = cx.dylan_theme().palette;
        let count = self.table.read(cx).delegate().len();
        let total = self.store.read(cx).ledger().pensioners().len();
        let permissions = self.session.read(cx).pensioner_permissions();
        let filtered = self.filter != PensionerFilter::All
            || !self.search_input.read(cx).value().trim().is_empty();

        v_flex()
            .size_full()
            .min_h_0()
            .gap(Spacing::MD)
            .child(
                h_flex()
                    .flex_wrap()
                    .gap(Spacing::SM)
                    .justify_between()
                    .items_center()
                    .child(
                        v_flex()
                            .gap(Spacing::XS)
                            .child(
                                Label::new("Pensionistas")
                                    .text_size(Typography::PAGE_TITLE)
                                    .font_bold(),
                            )
                            .child(
                                Label::new(format!("{count} de {total} pensionistas"))
                                    .text_size(Typography::SM)
                                    .text_color(palette.muted_foreground),
                            ),
                    )
                    .when(permissions.create, |header| {
                        header.child(
                            AppButton::new("new-pensioner")
                                .icon(IconName::Plus)
                                .label("Nuevo pensionista")
                                .on_click(cx.listener(|view, _, window, cx| {
                                    view.open_editor(None, window, cx)
                                })),
                        )
                    }),
            )
            .child(
                v_flex()
                    .flex_1()
                    .min_h_0()
                    .min_w_0()
                    .rounded(Radii::LG)
                    .bg(palette.surface)
                    .border_1()
                    .border_color(palette.border)
                    .overflow_hidden()
                    .child(
                        h_flex()
                            .p(Spacing::MD)
                            .gap(Spacing::SM)
                            .items_center()
                            .flex_wrap()
                            .children(PensionerFilter::ALL.map(|filter| {
                                let button = AppButton::new(SharedString::from(format!(
                                    "pensioner-filter-{filter:?}"
                                )))
                                .small()
                                .label(filter.label())
                                .on_click(cx.listener(move |view, _, _, cx| {
                                    view.filter = filter;
                                    view.sync_table(cx);
                                }));
                                if self.filter == filter {
                                    button.primary()
                                } else {
                                    button.secondary()
                                }
                            }))
                            .child(
                                div().flex_1().min_w(px(200.)).child(
                                    Input::new(&self.search_input)
                                        .cleanable(true)
                                        .prefix(Icon::new(IconName::Search).small()),
                                ),
                            )
                            .when(filtered, |toolbar| {
                                toolbar.child(
                                    AppButton::new("clear-pensioner-filters")
                                        .tertiary()
                                        .small()
                                        .label("Limpiar")
                                        .on_click(cx.listener(|view, _, window, cx| {
                                            view.clear_filters(window, cx)
                                        })),
                                )
                            }),
                    )
                    .child(
                        div().flex_1().min_h_0().min_w_0().child(
                            DataTable::new(&self.table)
                                .large()
                                .accessibility_label("Listado de pensionistas")
                                .bordered(false)
                                .scrollbar_visible(true, true),
                        ),
                    ),
            )
    }
}

impl Render for PensionersView {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let compact = window.viewport_size().width < px(640.0);
        let selected = self.selected.and_then(|selection| {
            self.store
                .read(cx)
                .ledger()
                .find_pensioner(selection.id)
                .cloned()
        });
        let content = match selected {
            Some(pensioner) => self.render_detail(&pensioner, cx).into_any_element(),
            None => self.render_list(cx).into_any_element(),
        };

        div()
            .id("pensioners-view")
            .track_focus(&self.focus)
            .size_full()
            .min_w_0()
            .min_h_0()
            .p(if compact { Spacing::MD } else { Spacing::XL })
            .child(content)
    }
}

fn account_status(pensioner: &Pensioner) -> StatusBadge {
    if pensioner.active {
        StatusBadge::new(StatusTone::Success, "Activo")
    } else {
        StatusBadge::new(StatusTone::Neutral, "Inactivo")
    }
}

fn fingerprint_status(pensioner: &Pensioner) -> StatusBadge {
    if pensioner.fingerprint_enrolled {
        StatusBadge::new(StatusTone::Success, "Registrada")
    } else {
        StatusBadge::new(StatusTone::Warning, "Pendiente")
    }
}

fn profile_metadata(pensioner: &Pensioner) -> Vec<String> {
    [
        (!pensioner.dni.is_empty()).then(|| format!("DNI {}", pensioner.dni)),
        pensioner.phone.clone(),
        (!pensioner.plan.is_empty()).then(|| pensioner.plan.clone()),
    ]
    .into_iter()
    .flatten()
    .collect()
}
