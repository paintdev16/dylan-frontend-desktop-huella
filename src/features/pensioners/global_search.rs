use gpui_kit::{
    base::actions::SelectDown,
    component::{
        IconName, IndexPath, Sizable as _, StyledExt as _, WindowExt as _,
        combobox::{Combobox, ComboboxEvent, ComboboxState},
        label::Label,
        searchable_list::{SearchableListDelegate, SearchableListItem, SearchableVec},
        v_flex,
    },
    prelude::FluentBuilder as _,
    *,
};

use crate::{
    components::loading,
    state::store::{ComedorStore, DataLoad},
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

struct PensionerChoices {
    items: SearchableVec<PensionerChoice>,
    view: WeakEntity<GlobalPensionerSearch>,
}

impl SearchableListDelegate for PensionerChoices {
    type Item = PensionerChoice;

    fn items_count(&self, section: usize) -> usize {
        self.items.items_count(section)
    }

    fn item(&self, index: IndexPath) -> Option<&Self::Item> {
        self.items.item(index)
    }

    fn position<V>(&self, value: &V) -> Option<IndexPath>
    where
        Self::Item: SearchableListItem<Value = V>,
        V: PartialEq,
    {
        self.items.position(value)
    }

    fn perform_search(&mut self, query: &str, window: &mut Window, cx: &mut App) -> Task<()> {
        let view = self.view.clone();
        let request = query.to_owned();
        window.defer(cx, move |window, cx| {
            let _ = view.update(cx, |view, cx| view.lookup(request, window, cx));
        });
        self.items.perform_search(query, window, cx)
    }
}

pub struct GlobalPensionerSearch {
    store: Entity<ComedorStore>,
    combo: Entity<ComboboxState<PensionerChoices>>,
    choices: Vec<PensionerChoice>,
    query: String,
    lookup_load: DataLoad,
    lookup_revision: u64,
    search_results: Vec<Pensioner>,
    _subscriptions: [Subscription; 2],
}

impl EventEmitter<GlobalSearchEvent> for GlobalPensionerSearch {}

impl GlobalPensionerSearch {
    pub fn new(store: Entity<ComedorStore>, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let choices = active_choices(store.read(cx).ledger().pensioners());
        let view = cx.weak_entity();
        let combo = cx.new(|cx| {
            ComboboxState::new(
                PensionerChoices {
                    items: SearchableVec::new(choices.clone()),
                    view,
                },
                vec![],
                window,
                cx,
            )
            .multiple(false)
            .searchable(true)
        });
        let observe_store = cx.observe_in(&store, window, |search, store, window, cx| {
            let choices = active_choices(store.read(cx).ledger().pensioners());
            if choices != search.choices {
                search.choices = choices.clone();
                if search.query.is_empty() {
                    search.set_choices(choices, window, cx);
                }
            }
            if search.query.is_empty() {
                cx.notify();
            }
        });
        let subscription = cx.subscribe_in(
            &combo,
            window,
            |_, _, event: &ComboboxEvent<PensionerChoices>, window, cx| {
                if let ComboboxEvent::Confirm(values) = event {
                    let choice = values.first().copied();
                    cx.defer_in(window, move |search, window, cx| {
                        if let Some(id) = choice {
                            if let Some(pensioner) = search
                                .search_results
                                .iter()
                                .find(|row| row.id == id)
                                .cloned()
                            {
                                search.store.update(cx, |store, cx| {
                                    store.remember_search_result(pensioner, cx)
                                });
                            }
                            cx.emit(GlobalSearchEvent::Selected(id));
                        }
                        search.reset(window, cx);
                    });
                }
            },
        );
        Self {
            store,
            combo,
            choices,
            query: String::new(),
            lookup_load: DataLoad::Idle,
            lookup_revision: 0,
            search_results: Vec::new(),
            _subscriptions: [observe_store, subscription],
        }
    }

    fn set_choices(
        &self,
        choices: Vec<PensionerChoice>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let view = cx.weak_entity();
        self.combo.update(cx, |combo, cx| {
            let query = combo.query(cx);
            combo.set_items(
                PensionerChoices {
                    items: SearchableVec::new(choices),
                    view,
                },
                window,
                cx,
            );
            combo.set_query(query, window, cx);
        });
    }

    fn lookup(&mut self, query: String, window: &mut Window, cx: &mut Context<Self>) {
        if self.query == query || self.combo.read(cx).query(cx).trim() != query {
            return;
        }
        self.query = query.clone();
        self.lookup_revision += 1;
        let revision = self.lookup_revision;
        self.lookup_load = DataLoad::Idle;
        self.search_results.clear();
        cx.notify();
        if query.is_empty() {
            self.set_choices(self.choices.clone(), window, cx);
            return;
        }
        if query.chars().count() > 120 {
            return;
        }
        if self.store.read(cx).has_valid_pensioner_catalog() {
            self.lookup_load = DataLoad::Ready;
            self.set_choices(self.choices.clone(), window, cx);
            return;
        }
        self.lookup_load = DataLoad::Loading;
        let directory = self.store.read(cx).search_uses_directory(cx);
        let target = cx.weak_entity();
        let window = window.window_handle();
        let term = query.clone();
        self.store.update(cx, |store, cx| {
            store.request(
                move |api| api.search_pensioners(&term, directory),
                move |_, result, cx| {
                    let _ = window.update(cx, |_, window, cx| {
                        let _ = target.update(cx, |view, cx| {
                            if view.lookup_revision != revision
                                || view.combo.read(cx).query(cx).trim() != query
                            {
                                return;
                            }
                            let Ok(rows) = result else {
                                view.lookup_load = DataLoad::Failed;
                                cx.notify();
                                return;
                            };
                            let mut choices = active_choices(&rows);
                            let normalized = normalize_text(&query);
                            for choice in &mut choices {
                                choice.search_text.push(' ');
                                choice.search_text.push_str(&normalized);
                            }
                            view.search_results = rows;
                            view.lookup_load = DataLoad::Ready;
                            view.set_choices(choices, window, cx);
                            cx.notify();
                        });
                    });
                },
                cx,
            )
        });
    }

    fn load_suggestions(&self, cx: &mut Context<Self>) {
        self.store
            .update(cx, |store, cx| store.load_pensioners(false, cx));
    }

    pub fn focus(&self, window: &mut Window, cx: &mut Context<Self>) {
        self.load_suggestions(cx);
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

    pub fn reset(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.query.clear();
        self.lookup_revision += 1;
        self.lookup_load = DataLoad::Idle;
        self.search_results.clear();
        self.combo.update(cx, |combo, cx| {
            if combo.selected_value().is_some() {
                combo.clear_selection(cx);
            }
            if !combo.query(cx).is_empty() {
                combo.set_query("", window, cx);
            }
        });
        self.set_choices(self.choices.clone(), window, cx);
        cx.notify();
    }
}

impl Render for GlobalPensionerSearch {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let viewport = window.viewport_size().width;
        let width = if viewport < px(600.) {
            (viewport - px(294.)).clamp(px(64.), px(176.))
        } else {
            px(240.)
        };

        let view = cx.weak_entity();
        div()
            .w(width)
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(|view, _, _, cx| {
                    if view.query.is_empty() {
                        view.load_suggestions(cx);
                    }
                }),
            )
            .on_key_down(cx.listener(|view, event: &KeyDownEvent, _, cx| {
                if view.query.is_empty()
                    && matches!(event.keystroke.key.as_str(), "enter" | "down" | "space")
                {
                    view.load_suggestions(cx);
                }
            }))
            .child(
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
                    .empty(move |_, cx| {
                        let load = view.upgrade().map_or(DataLoad::Idle, |view| {
                            let view = view.read(cx);
                            if view.query.is_empty() {
                                view.store.read(cx).pensioners_load()
                            } else {
                                view.lookup_load
                            }
                        });
                        let content = if load == DataLoad::Loading {
                            loading::rows(
                                "pensioner-search-loading",
                                "Buscando pensionistas",
                                2,
                                cx,
                            )
                            .into_any_element()
                        } else {
                            Label::new(if load == DataLoad::Failed {
                                "No se pudo completar la búsqueda."
                            } else {
                                "Sin pensionistas activos que coincidan."
                            })
                            .text_size(Typography::SM)
                            .text_color(cx.dylan_theme().palette.muted_foreground)
                            .into_any_element()
                        };
                        div()
                            .px(Spacing::MD)
                            .py(Spacing::SM)
                            .text_center()
                            .child(content)
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
