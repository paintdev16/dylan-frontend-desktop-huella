use gpui_kit::{
    base::actions::SelectDown,
    component::{
        IconName, Sizable as _, StyledExt as _, WindowExt as _,
        combobox::{Combobox, ComboboxEvent, ComboboxState},
        label::Label,
        searchable_list::{SearchableListItem, SearchableVec},
        v_flex,
    },
    prelude::FluentBuilder as _,
    *,
};

use crate::{
    state::store::ComedorStore,
    theme::{ActiveDylanTheme as _, Spacing, Typography},
};

use super::model::{Pensioner, PensionerId, matches_search_terms, normalize_text};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GlobalSearchEvent {
    Selected(PensionerId),
}

#[derive(Clone, PartialEq, Eq)]
struct PensionerChoice {
    id: PensionerId,
    name: SharedString,
    phone: Option<SharedString>,
    search_text: String,
}

impl SearchableListItem for PensionerChoice {
    type Value = PensionerId;

    fn title(&self) -> SharedString {
        match &self.phone {
            Some(phone) => format!("{} · {phone}", self.name).into(),
            None => self.name.clone(),
        }
    }

    fn value(&self) -> &PensionerId {
        &self.id
    }

    fn matches(&self, query: &str) -> bool {
        matches_search_terms(&self.search_text, &normalize_text(query))
    }

    fn render(&self, _: &mut Window, cx: &mut App) -> impl IntoElement {
        v_flex()
            .min_w_0()
            .gap(Spacing::XXS)
            .child(
                Label::new(self.name.clone())
                    .text_size(Typography::SM)
                    .font_semibold()
                    .truncate(),
            )
            .when_some(self.phone.clone(), |row, phone| {
                row.child(
                    Label::new(phone)
                        .text_size(Typography::XS)
                        .text_color(cx.dylan_theme().palette.muted_foreground),
                )
            })
    }
}

fn active_choices(catalog: &[Pensioner]) -> Vec<PensionerChoice> {
    let mut choices: Vec<_> = catalog
        .iter()
        .filter(|pensioner| pensioner.active)
        .map(|pensioner| PensionerChoice {
            id: pensioner.id,
            name: pensioner.full_name.clone().into(),
            phone: pensioner
                .phone
                .as_deref()
                .filter(|phone| !phone.is_empty())
                .map(Into::into),
            search_text: pensioner.search_text(),
        })
        .collect();
    choices.sort_by_key(|choice| choice.id);
    choices
}

type PensionerChoices = SearchableVec<PensionerChoice>;

pub struct GlobalPensionerSearch {
    combo: Entity<ComboboxState<PensionerChoices>>,
    choices: Vec<PensionerChoice>,
    _subscriptions: [Subscription; 2],
}

impl EventEmitter<GlobalSearchEvent> for GlobalPensionerSearch {}

impl GlobalPensionerSearch {
    pub fn new(store: Entity<ComedorStore>, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let choices = active_choices(store.read(cx).ledger().pensioners());
        let combo = cx.new(|cx| {
            ComboboxState::new(SearchableVec::new(choices.clone()), vec![], window, cx)
                .multiple(false)
                .searchable(true)
        });
        let observe_store = cx.observe_in(&store, window, |search, store, window, cx| {
            let choices = active_choices(store.read(cx).ledger().pensioners());
            if choices != search.choices {
                search.choices = choices.clone();
                search.combo.update(cx, |combo, cx| {
                    let query = combo.query(cx);
                    combo.set_items(SearchableVec::new(choices), window, cx);
                    combo.set_query(query, window, cx);
                });
            }
        });
        let subscription = cx.subscribe_in(
            &combo,
            window,
            |_, _, event: &ComboboxEvent<PensionerChoices>, window, cx| {
                if let ComboboxEvent::Confirm(values) = event {
                    let choice = values.first().copied();
                    cx.defer_in(window, move |search, window, cx| {
                        search.reset(window, cx);
                        if let Some(id) = choice {
                            cx.emit(GlobalSearchEvent::Selected(id));
                        }
                    });
                }
            },
        );
        Self {
            combo,
            choices,
            _subscriptions: [observe_store, subscription],
        }
    }

    pub fn focus(&self, window: &mut Window, cx: &mut App) {
        let focus = self.combo.read(cx).focus_handle(cx);
        focus.focus(window, cx);
        window.defer(cx, move |window, cx| {
            if focus.is_focused(window) && !window.has_active_dialog(cx) {
                window.dispatch_action(Box::new(SelectDown), cx);
            }
        });
    }

    pub fn restore_focus(&self, window: &mut Window, cx: &mut App) {
        self.combo.read(cx).focus_handle(cx).focus(window, cx);
    }

    pub fn reset(&self, window: &mut Window, cx: &mut App) {
        self.combo.update(cx, |combo, cx| {
            if combo.selected_value().is_some() {
                combo.clear_selection(cx);
            }
            if !combo.query(cx).is_empty() {
                combo.set_query("", window, cx);
            }
        });
    }
}

impl Render for GlobalPensionerSearch {
    fn render(&mut self, window: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        let viewport = window.viewport_size().width;
        let width = if viewport < px(600.) {
            (viewport - px(294.)).clamp(px(64.), px(176.))
        } else {
            px(240.)
        };

        div().w(width).child(
            Combobox::new(&self.combo)
                .xsmall()
                .icon(IconName::Search)
                .placeholder(if viewport < px(600.) {
                    "Buscar"
                } else {
                    "Buscar pensionista (Ctrl+K)"
                })
                .search_placeholder("Nombre, apellido o celular…")
                .menu_width(px(360.).min(viewport - Spacing::MD))
                .cleanable(true)
                .empty(|_, cx| {
                    div().px(Spacing::MD).py(Spacing::LG).text_center().child(
                        Label::new("Sin pensionistas activos que coincidan.")
                            .text_size(Typography::SM)
                            .text_color(cx.dylan_theme().palette.muted_foreground),
                    )
                }),
        )
    }
}

#[cfg(test)]
mod tests {
    use gpui_kit::component::searchable_list::SearchableListItem;

    use super::active_choices;
    use crate::features::pensioners::model::{PensionerId, demo_pensioners};

    #[test]
    fn suggestions_exclude_inactive_pensioners_and_refresh_on_status_change() {
        let mut catalog = demo_pensioners();
        assert!(
            active_choices(&catalog)
                .iter()
                .all(|choice| choice.id != PensionerId(1006))
        );

        catalog
            .iter_mut()
            .find(|p| p.id == PensionerId(1006))
            .unwrap()
            .active = true;
        assert!(
            active_choices(&catalog)
                .iter()
                .any(|choice| choice.id == PensionerId(1006))
        );
    }

    #[test]
    fn searches_each_term_in_names_and_also_accepts_phone_dni_and_code() {
        let choices = active_choices(&demo_pensioners());
        let ana = &choices[0];

        assert_eq!(ana.id, PensionerId(1001));
        assert!(ana.matches("Quispe lucia"));
        assert!(ana.matches("900000001"));
        assert!(ana.matches("72481920"));
        assert!(ana.matches("P-1001"));
        assert!(!ana.matches("María Quispe"));
    }
}
