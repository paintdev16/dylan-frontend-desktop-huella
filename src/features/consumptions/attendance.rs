use chrono::{Local, Timelike as _};
use gpui_kit::{
    base::FocusTrapElement as _,
    component::{
        IconName, Selectable as _, Sizable as _, StyledExt as _,
        alert::Alert,
        button::{Button, ButtonGroup, ButtonVariants as _},
        combobox::{Combobox, ComboboxEvent, ComboboxState},
        h_flex,
        input::{Input, InputEvent, InputState},
        label::Label,
        searchable_list::{SearchableListItem, SearchableVec},
        select::{Select, SelectEvent, SelectState},
        switch::Switch,
        v_flex,
    },
    prelude::FluentBuilder as _,
    *,
};

use crate::{
    components::{button::AppButton, loading},
    features::pensioners::{
        SelectionSource,
        model::{PensionerId, format_soles, matches_search_terms, normalize_text},
    },
    services::api::dining::{AttendanceContext, confirmation_uncertain},
    state::{
        session::SessionState,
        store::{AttendanceRecord, ComedorStore, DataLoad, parse_soles_to_cents},
    },
    theme::{ActiveDylanTheme as _, Radii, Spacing, Typography},
};

use super::{
    confirm_attendance::{AttendanceRequest, AttendanceSummary},
    order::{ConsumptionItem, Meal, Menu, add_to_cart, parse_ticket_count},
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AttendanceOrigin {
    GlobalSearch,
    Identification,
}

impl AttendanceOrigin {
    fn source(self) -> SelectionSource {
        match self {
            Self::GlobalSearch => SelectionSource::GlobalSearch,
            Self::Identification => SelectionSource::BiometricMatch,
        }
    }
}

#[derive(Debug, Clone)]
pub enum AttendanceModalEvent {
    Cancelled {
        origin: AttendanceOrigin,
    },
    Confirmed {
        origin: AttendanceOrigin,
        record: AttendanceRecord,
    },
}

impl SearchableListItem for Meal {
    type Value = Self;

    fn title(&self) -> SharedString {
        self.label().into()
    }

    fn value(&self) -> &Self {
        self
    }
}

#[derive(Clone, PartialEq, Eq)]
enum PickupChoice {
    Registered(SharedString),
    Other,
}

impl SearchableListItem for PickupChoice {
    type Value = Self;

    fn title(&self) -> SharedString {
        match self {
            Self::Registered(name) => name.clone(),
            Self::Other => "Escribir otro nombre…".into(),
        }
    }

    fn value(&self) -> &Self {
        self
    }

    fn matches(&self, query: &str) -> bool {
        match self {
            Self::Registered(name) => {
                matches_search_terms(&normalize_text(name), &normalize_text(query))
            }
            Self::Other => true,
        }
    }
}

type PickupChoices = SearchableVec<PickupChoice>;

struct ActiveDraft {
    origin: AttendanceOrigin,
    pensioner_id: PensionerId,
    idempotency_key: String,
    context: Option<AttendanceContext>,
    loading_context: bool,
    pending_request: Option<AttendanceRequest>,
    special: bool,
    portions: u32,
    cart: Vec<ConsumptionItem>,
    third_party: bool,
    submitting: bool,
    error: Option<SharedString>,
}

pub struct AttendanceModal {
    focus: FocusHandle,
    store: Entity<ComedorStore>,
    session: Entity<SessionState>,
    meal: Entity<SelectState<Vec<Meal>>>,
    special_price: Entity<InputState>,
    pickup: Entity<ComboboxState<PickupChoices>>,
    pickup_choices: Vec<PickupChoice>,
    pickup_name: Entity<InputState>,
    ticket_input: Entity<InputState>,
    draft: Option<ActiveDraft>,
    _subscriptions: [Subscription; 7],
}

impl EventEmitter<AttendanceModalEvent> for AttendanceModal {}

impl AttendanceModal {
    pub fn new(
        store: Entity<ComedorStore>,
        session: Entity<SessionState>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let meal = cx.new(|cx| SelectState::new(Meal::ALL.to_vec(), None, window, cx));
        let special_price = cx.new(|cx| InputState::new(window, cx).placeholder("12.00"));
        let pickup = cx.new(|cx| {
            ComboboxState::new(SearchableVec::new(vec![]), vec![], window, cx)
                .multiple(false)
                .searchable(true)
        });
        let pickup_name =
            cx.new(|cx| InputState::new(window, cx).placeholder("Nombres y apellidos"));
        let ticket_input = cx.new(|cx| InputState::new(window, cx));
        let observe_store = cx.observe_in(&store, window, |view, _, window, cx| {
            if view.draft.as_ref().is_some_and(|draft| draft.third_party) {
                view.sync_pickup_choices(window, cx);
            }
            cx.notify();
        });
        let observe_session = cx.observe(&session, |_, _, cx| cx.notify());
        let meal_changed = cx.subscribe(&meal, |view, _, event, cx| {
            if matches!(event, SelectEvent::Confirm(_)) {
                view.edit(cx, |_| {});
            }
        });
        let pickup_selected = cx.subscribe(&pickup, |view, _, event, cx| {
            if matches!(event, ComboboxEvent::Confirm(_)) {
                view.edit(cx, |_| {});
            }
        });
        let inputs = [&special_price, &pickup_name, &ticket_input];
        let [price_changed, pickup_changed, tickets_changed] = inputs.map(|input| {
            cx.subscribe(input, |view, _, event, cx| {
                if matches!(event, InputEvent::Change) {
                    view.edit(cx, |_| {});
                }
            })
        });

        Self {
            focus: cx.focus_handle(),
            store,
            session,
            meal,
            special_price,
            pickup,
            pickup_choices: Vec::new(),
            pickup_name,
            ticket_input,
            draft: None,
            _subscriptions: [
                observe_store,
                observe_session,
                meal_changed,
                pickup_selected,
                price_changed,
                pickup_changed,
                tickets_changed,
            ],
        }
    }

    pub fn is_open(&self) -> bool {
        self.draft.is_some()
    }

    pub fn open(
        &mut self,
        pensioner_id: PensionerId,
        origin: AttendanceOrigin,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> bool {
        if self.is_open() || !self.session.read(cx).can_register_attendance() {
            return false;
        }

        self.meal.update(cx, |select, cx| {
            select.set_selected_value(&Meal::for_hour(Local::now().hour()), window, cx);
        });
        self.special_price
            .update(cx, |input, cx| input.set_value("12.00", window, cx));
        self.pickup.update(cx, |combo, cx| {
            combo.set_selected_values(&[], window, cx);
            combo.set_query("", window, cx);
        });
        self.pickup_name
            .update(cx, |input, cx| input.clean(window, cx));
        self.ticket_input
            .update(cx, |input, cx| input.set_value("0", window, cx));
        self.draft = Some(ActiveDraft {
            origin,
            pensioner_id,
            idempotency_key: uuid::Uuid::new_v4().to_string(),
            context: None,
            loading_context: false,
            pending_request: None,
            special: false,
            portions: 1,
            cart: Vec::new(),
            third_party: false,
            submitting: false,
            error: None,
        });
        self.load_context(cx);
        window.focus(&self.focus, cx);
        cx.notify();
        true
    }

    fn sync_pickup_choices(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let mut choices: Vec<_> = self
            .store
            .read(cx)
            .ledger()
            .pensioners()
            .iter()
            .map(|pensioner| PickupChoice::Registered(pensioner.full_name.clone().into()))
            .collect();
        choices.push(PickupChoice::Other);
        if self.pickup_choices != choices {
            self.pickup_choices = choices.clone();
            self.pickup.update(cx, |combo, cx| {
                let query = combo.query(cx);
                combo.set_items(SearchableVec::new(choices), window, cx);
                combo.set_query(query, window, cx);
            });
        }
    }

    pub fn close_for_sign_out(&mut self, cx: &mut Context<Self>) {
        if self.draft.take().is_some() {
            cx.notify();
        }
    }

    fn cancel(&mut self, cx: &mut Context<Self>) {
        if self.draft.as_ref().is_some_and(|draft| draft.submitting) {
            return;
        }
        if let Some(draft) = self.draft.take() {
            cx.emit(AttendanceModalEvent::Cancelled {
                origin: draft.origin,
            });
            cx.notify();
        }
    }

    fn edit(&mut self, cx: &mut Context<Self>, edit: impl FnOnce(&mut ActiveDraft)) {
        if let Some(draft) = self
            .draft
            .as_mut()
            .filter(|draft| !draft.submitting && draft.pending_request.is_none())
        {
            draft.error = None;
            edit(draft);
            cx.notify();
        }
    }

    fn load_context(&mut self, cx: &mut Context<Self>) {
        let Some(draft) = self.draft.as_mut().filter(|draft| !draft.loading_context) else {
            return;
        };
        let id = draft.pensioner_id;
        let key = draft.idempotency_key.clone();
        draft.loading_context = true;
        draft.error = None;
        let target = cx.weak_entity();
        self.store.update(cx, |store, cx| {
            store.request(
                move |api| api.attendance_context(id),
                move |store, result, cx| {
                    if let Ok(context) = &result {
                        store.remember_search_result(context.pensioner.clone(), cx);
                    }
                    let _ = target.update(cx, |view, cx| {
                        let Some(draft) = view
                            .draft
                            .as_mut()
                            .filter(|draft| draft.idempotency_key == key)
                        else {
                            return;
                        };
                        draft.loading_context = false;
                        match result {
                            Ok(context) if draft.origin == AttendanceOrigin::Identification
                                && !context.pensioner.fingerprint_enrolled =>
                            {
                                draft.error = Some("La ficha en Laravel aún no tiene una huella confirmada. Verifica el enrolamiento o busca al pensionista por nombre.".into());
                            }
                            Ok(context) => draft.context = Some(context),
                            Err(error) => draft.error = Some(error.user_message().into()),
                        }
                        cx.notify();
                    });
                },
                cx,
            )
        });
        cx.notify();
    }

    fn add_item(&mut self, cx: &mut Context<Self>) {
        let Some(meal) = self.meal.read(cx).selected_value().copied() else {
            return;
        };
        let price =
            parse_soles_to_cents(&self.special_price.read(cx).value()).filter(|cents| *cents > 0);
        self.edit(cx, |draft| {
            let portions = std::mem::replace(&mut draft.portions, 1);
            let menu = if draft.special {
                let Some(unit_cents) = price else {
                    draft.error = Some("Ingresa un precio especial mayor a S/ 0.00.".into());
                    return;
                };
                Menu::Special { unit_cents }
            } else {
                Menu::Economic
            };
            if add_to_cart(
                &mut draft.cart,
                ConsumptionItem {
                    meal,
                    menu,
                    portions,
                },
            )
            .is_none()
            {
                draft.error = Some("La cantidad de raciones excede el límite permitido.".into());
            }
        });
    }

    fn set_tickets(&mut self, value: u32, window: &mut Window, cx: &mut Context<Self>) {
        self.ticket_input.update(cx, |input, cx| {
            input.set_value(value.to_string(), window, cx)
        });
        self.edit(cx, |_| {});
    }

    fn submit(&mut self, cx: &mut Context<Self>) {
        if !self.session.read(cx).can_register_attendance() {
            return;
        }
        let Some(draft) = self
            .draft
            .as_mut()
            .filter(|draft| !draft.submitting && draft.context.is_some())
        else {
            return;
        };
        if let Some(request) = draft.pending_request.clone() {
            self.send_confirmation(request, cx);
            return;
        }
        let Some(tickets_count) = parse_ticket_count(&self.ticket_input.read(cx).value()) else {
            draft.error = Some("Ingresa una cantidad entera válida de tickets.".into());
            cx.notify();
            return;
        };
        let picked_up_by = (draft.third_party && !draft.cart.is_empty()).then(|| {
            match self.pickup.read(cx).selected_value() {
                Some(PickupChoice::Registered(name)) => name.to_string(),
                Some(PickupChoice::Other) => self.pickup_name.read(cx).value().to_string(),
                None => String::new(),
            }
        });
        if picked_up_by
            .as_deref()
            .is_some_and(|name| name.trim().chars().count() < 2)
        {
            draft.error = Some("Selecciona o escribe el nombre de quien recoge el consumo.".into());
            cx.notify();
            return;
        }
        let request = AttendanceRequest {
            idempotency_key: draft.idempotency_key.clone(),
            pensioner_id: draft.pensioner_id,
            items: draft.cart.clone(),
            picked_up_by,
            tickets_count,
            source: draft.origin.source(),
            #[cfg(test)]
            operator_name: String::new(),
            #[cfg(test)]
            can_register_attendance: true,
        };
        self.send_confirmation(request, cx);
    }

    fn send_confirmation(&mut self, request: AttendanceRequest, cx: &mut Context<Self>) {
        let Some(draft) = self.draft.as_mut() else {
            return;
        };
        let key = request.idempotency_key.clone();
        draft.submitting = true;
        draft.pending_request = Some(request.clone());
        draft.error = None;
        cx.notify();
        let target = cx.weak_entity();
        self.store.update(cx, |store, cx| {
            store.request(
                move |api| api.confirm_attendance(&request),
                move |store, result, cx| {
                    if let Ok(record) = &result {
                        store.attendance_changed(record, cx);
                    }
                    let _ = target.update(cx, |view, cx| {
                        let Some(draft) = view
                            .draft
                            .as_mut()
                            .filter(|draft| draft.idempotency_key == key)
                        else {
                            return;
                        };
                        draft.submitting = false;
                        match result {
                            Ok(record) => {
                                let origin = draft.origin;
                                view.draft = None;
                                cx.emit(AttendanceModalEvent::Confirmed { origin, record });
                            }
                            Err(error) => {
                                if confirmation_uncertain(&error) {
                                    draft.error = Some(
                                        format!(
                                            "{} Reintenta para verificar esta misma atención.",
                                            error.user_message()
                                        )
                                        .into(),
                                    );
                                } else {
                                    draft.pending_request = None;
                                    draft.error = Some(error.user_message().into());
                                }
                            }
                        }
                        cx.notify();
                    });
                },
                cx,
            )
        });
    }

    fn consumption_picker(&self, draft: &ActiveDraft, cx: &mut Context<Self>) -> impl IntoElement {
        let palette = cx.dylan_theme().palette;
        let meal = self.meal.read(cx).selected_value().copied();
        let economic_label = meal.zip(draft.context.as_ref()).map_or_else(
            || "Económico".into(),
            |(meal, context)| format!("Económico · {}", format_soles(context.economic_price(meal))),
        );

        v_flex()
            .flex_shrink_0()
            .p(Spacing::MD)
            .gap(Spacing::SM)
            .rounded(Radii::LG)
            .bg(palette.surface_subtle)
            .border_1()
            .border_color(palette.border)
            .child(
                h_flex()
                    .flex_wrap()
                    .items_center()
                    .gap(Spacing::SM)
                    .child(
                        div().flex_1().min_w(px(132.)).child(
                            Select::new(&self.meal)
                                .id("attendance-meal")
                                .small()
                                .w_full()
                                .accessibility_label("Tipo de comida")
                                .placeholder("Tipo de comida"),
                        ),
                    )
                    .child(
                        ButtonGroup::new("attendance-menu")
                            .small()
                            .children(
                                [
                                    ("menu-economic", economic_label, !draft.special),
                                    ("menu-special", "Especial".into(), draft.special),
                                ]
                                .map(|(id, label, selected)| {
                                    Button::new(id)
                                        .label(label)
                                        .selected(selected)
                                        .when(selected, |button| {
                                            button.primary().icon(IconName::Check).font_semibold()
                                        })
                                        .when(!selected, |button| button.outline())
                                }),
                            )
                            .on_click(cx.listener(|view, selected: &Vec<usize>, _, cx| {
                                view.edit(cx, |draft| draft.special = selected.contains(&1));
                            })),
                    ),
            )
            .child(
                h_flex()
                    .flex_wrap()
                    .justify_between()
                    .items_center()
                    .gap(Spacing::SM)
                    .child(
                        h_flex()
                            .flex_wrap()
                            .gap(Spacing::SM)
                            .items_center()
                            .when(draft.special, |row| {
                                row.child(
                                    h_flex()
                                        .items_center()
                                        .gap(Spacing::XS)
                                        .child(
                                            Label::new("Precio unitario (S/)")
                                                .text_size(Typography::XS),
                                        )
                                        .child(
                                            div().w(px(72.)).child(
                                                Input::new(&self.special_price)
                                                    .small()
                                                    .bg(palette.surface),
                                            ),
                                        ),
                                )
                            })
                            .child(
                                h_flex()
                                    .items_center()
                                    .gap(Spacing::XS)
                                    .child(Label::new("Raciones").text_size(Typography::XS))
                                    .child(
                                        step_button("portions-down", false)
                                            .disabled(draft.portions <= 1)
                                            .on_click(cx.listener(|view, _, _, cx| {
                                                view.edit(cx, |draft| {
                                                    draft.portions =
                                                        draft.portions.saturating_sub(1).max(1)
                                                })
                                            })),
                                    )
                                    .child(Label::new(draft.portions.to_string()).font_semibold())
                                    .child(
                                        step_button("portions-up", true)
                                            .disabled(draft.portions == u32::MAX)
                                            .on_click(cx.listener(|view, _, _, cx| {
                                                view.edit(cx, |draft| {
                                                    draft.portions =
                                                        draft.portions.saturating_add(1)
                                                })
                                            })),
                                    ),
                            ),
                    )
                    .child(
                        AppButton::new("add-consumption")
                            .small()
                            .secondary()
                            .disabled(meal.is_none())
                            .icon(IconName::Plus)
                            .label(if draft.special {
                                "Agregar especial"
                            } else {
                                "Agregar económico"
                            })
                            .on_click(cx.listener(|view, _, _, cx| view.add_item(cx))),
                    ),
            )
    }

    fn cart_panel(&self, draft: &ActiveDraft, cx: &mut Context<Self>) -> impl IntoElement {
        let palette = cx.dylan_theme().palette;
        v_flex()
            .flex_shrink_0()
            .child(
                Label::new("Carrito")
                    .text_size(Typography::SM)
                    .font_semibold(),
            )
            .children(draft.cart.iter().enumerate().map(|(index, item)| {
                let unit = match item.menu {
                    Menu::Economic => draft
                        .context
                        .as_ref()
                        .map_or(0, |context| context.economic_price(item.meal)),
                    Menu::Special { unit_cents } => unit_cents,
                };
                let subtotal = unit
                    .checked_mul(i64::from(item.portions))
                    .map(format_soles)
                    .unwrap_or_else(|| "Importe fuera de rango".into());
                h_flex()
                    .w_full()
                    .flex_wrap()
                    .items_center()
                    .justify_between()
                    .gap(Spacing::XS)
                    .border_b_1()
                    .border_color(palette.border)
                    .py(Spacing::XS)
                    .child(
                        v_flex()
                            .gap(Spacing::XXS)
                            .child(
                                Label::new(format!(
                                    "{} · {}",
                                    item.meal.label(),
                                    item.menu.label()
                                ))
                                .font_semibold()
                                .text_size(Typography::SM),
                            )
                            .child(
                                Label::new(format!("{} por ración", format_soles(unit)))
                                    .text_size(Typography::XS)
                                    .text_color(palette.muted_foreground),
                            ),
                    )
                    .child(
                        h_flex()
                            .items_center()
                            .gap(Spacing::XS)
                            .child(
                                step_button(("cart-down", index), false).on_click(cx.listener(
                                    move |view, _, _, cx| {
                                        view.edit(cx, |draft| {
                                            if let Some(item) = draft.cart.get_mut(index) {
                                                if item.portions <= 1 {
                                                    draft.cart.remove(index);
                                                } else {
                                                    item.portions -= 1;
                                                }
                                            }
                                        });
                                    },
                                )),
                            )
                            .child(Label::new(item.portions.to_string()))
                            .child(
                                step_button(("cart-up", index), true)
                                    .disabled(item.portions == u32::MAX)
                                    .on_click(cx.listener(move |view, _, _, cx| {
                                        view.edit(cx, |draft| {
                                            if let Some(item) = draft.cart.get_mut(index) {
                                                item.portions = item.portions.saturating_add(1);
                                            }
                                        })
                                    })),
                            )
                            .child(
                                Label::new(subtotal)
                                    .text_size(Typography::SM)
                                    .font_semibold(),
                            )
                            .child(
                                AppButton::new(("cart-remove", index))
                                    .tertiary()
                                    .xsmall()
                                    .icon(IconName::Close)
                                    .tooltip("Quitar consumo")
                                    .on_click(cx.listener(move |view, _, _, cx| {
                                        view.edit(cx, |draft| {
                                            if index < draft.cart.len() {
                                                draft.cart.remove(index);
                                            }
                                        })
                                    })),
                            ),
                    )
            }))
    }

    fn pickup_panel(&self, draft: &ActiveDraft, cx: &mut Context<Self>) -> impl IntoElement {
        let palette = cx.dylan_theme().palette;
        h_flex()
            .flex_wrap()
            .items_start()
            .flex_shrink_0()
            .gap(Spacing::SM)
            .border_t_1()
            .border_color(palette.border)
            .pt(Spacing::SM)
            .child(
                div().flex_shrink_0().child(
                    Switch::new("third-party-pickup")
                        .small()
                        .label("Recoge otra persona")
                        .checked(draft.third_party)
                        .on_change(cx.listener(|view, checked, window, cx| {
                            view.edit(cx, |draft| draft.third_party = *checked);
                            if *checked {
                                view.sync_pickup_choices(window, cx);
                                view.store
                                    .update(cx, |store, cx| store.load_pensioners(false, cx));
                            }
                        })),
                ),
            )
            .when(draft.third_party, |panel| {
                panel.child(
                    div()
                        .flex_1()
                        .min_w(px(240.))
                        .max_w_full()
                        .child(self.pickup_name_field(cx)),
                )
            })
    }

    fn pickup_name_field(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let palette = cx.dylan_theme().palette;
        let load = self.store.read(cx).pensioners_load();
        v_flex()
            .gap(Spacing::XS)
            .child(
                Combobox::new(&self.pickup)
                    .small()
                    .w_full()
                    .placeholder("Selecciona un pensionista u otro nombre")
                    .search_placeholder("Busca por nombre o apellido"),
            )
            .when(load == DataLoad::Loading, |field| {
                field.child(
                    loading::region("pickup-loading", "Cargando nombres registrados")
                        .child(loading::line("registered-names", px(136.))),
                )
            })
            .when(load == DataLoad::Failed, |field| {
                field.child(
                    Label::new("No se pudieron cargar los nombres registrados.")
                        .text_size(Typography::XS)
                        .text_color(palette.muted_foreground),
                )
            })
            .when(
                self.pickup.read(cx).selected_value() == Some(PickupChoice::Other),
                |field| {
                    field.child(
                        Input::new(&self.pickup_name)
                            .small()
                            .w_full()
                            .bg(palette.surface),
                    )
                },
            )
    }

    fn ticket_panel(
        &self,
        tickets: Option<u32>,
        ticket_unit_cents: i64,
        cash_open: bool,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let count = tickets.unwrap_or(0);
        let palette = cx.dylan_theme().palette;
        let can_increment = ticket_unit_cents > 0
            && count < u32::MAX
            && i64::from(count.saturating_add(1))
                .checked_mul(ticket_unit_cents)
                .is_some();
        v_flex()
            .flex_shrink_0()
            .gap(Spacing::XS)
            .border_t_1()
            .border_color(palette.border)
            .pt(Spacing::SM)
            .child(
                h_flex()
                    .flex_wrap()
                    .items_center()
                    .justify_between()
                    .gap(Spacing::SM)
                    .child(
                        v_flex()
                            .child(
                                Label::new("Tickets físicos")
                                    .text_size(Typography::SM)
                                    .font_semibold(),
                            )
                            .child(
                                Label::new(format!(
                                    "{} por ticket",
                                    format_soles(ticket_unit_cents)
                                ))
                                .text_size(Typography::XS)
                                .text_color(palette.muted_foreground),
                            ),
                    )
                    .child(
                        h_flex()
                            .items_center()
                            .gap(Spacing::XS)
                            .child(
                                step_button("tickets-down", false)
                                    .disabled(count == 0)
                                    .on_click(cx.listener(move |view, _, window, cx| {
                                        view.set_tickets(count.saturating_sub(1), window, cx)
                                    })),
                            )
                            .child(
                                div()
                                    .w(px(64.))
                                    .child(Input::new(&self.ticket_input).small()),
                            )
                            .child(
                                step_button("tickets-up", true)
                                    .disabled(!can_increment)
                                    .on_click(cx.listener(move |view, _, window, cx| {
                                        view.set_tickets(count.saturating_add(1), window, cx)
                                    })),
                            )
                            .when(count > 0, |row| {
                                row.child(
                                    AppButton::new("tickets-zero")
                                        .tertiary()
                                        .xsmall()
                                        .label("Quitar abono")
                                        .on_click(cx.listener(|view, _, window, cx| {
                                            view.set_tickets(0, window, cx)
                                        })),
                                )
                            }),
                    ),
            )
            .when(tickets.is_none(), |panel| {
                panel.child(
                    Alert::error("tickets-invalid", "Ingresa una cantidad entera válida.").xsmall(),
                )
            })
            .when(count > 0 && !cash_open, |panel| {
                panel.child(
                    Alert::warning(
                        "tickets-cash-closed",
                        "Abre la caja para recibir tickets o deja la cantidad en 0.",
                    )
                    .xsmall(),
                )
            })
    }
}

impl Render for AttendanceModal {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let Some(draft) = &self.draft else {
            return div().into_any_element();
        };
        let context = draft.context.as_ref();
        let pensioner = context.map(|context| &context.pensioner);
        let cash_open = context.is_some_and(|context| context.cash_open);
        let ticket_unit_cents = context.map_or(0, |context| context.ticket_value_cents);
        let can_register = self.session.read(cx).can_register_attendance()
            && pensioner.is_some_and(|pensioner| pensioner.active);
        let name = pensioner.map_or_else(
            || "Pensionista".into(),
            |pensioner| pensioner.full_name.clone(),
        );
        let metadata = [
            draft.pensioner_id.code(),
            pensioner.map_or_else(String::new, |pensioner| pensioner.dni.clone()),
            draft.origin.source().label().into(),
        ]
        .into_iter()
        .filter(|text| !text.is_empty())
        .collect::<Vec<_>>()
        .join(" · ");
        let balance = pensioner.map_or(0, |pensioner| pensioner.balance_cents);
        let tickets = parse_ticket_count(&self.ticket_input.read(cx).value());
        let summary = tickets.zip(context).map(|(count, context)| {
            AttendanceSummary::calculate_with_prices(
                balance,
                &draft.cart,
                count,
                ticket_unit_cents,
                |meal| context.economic_price(meal),
            )
        });
        let can_submit = can_register
            && !draft.submitting
            && !draft.loading_context
            && (draft.pending_request.is_some()
                || (summary.as_ref().is_some_and(Result::is_ok)
                    && (cash_open || tickets == Some(0))
                    && (!draft.cart.is_empty() || tickets != Some(0))));

        let palette = cx.dylan_theme().palette;
        let viewport = window.viewport_size();
        let modal = v_flex()
            .id("attendance-card")
            .occlude()
            .w(px(584.))
            .max_w(viewport.width - Spacing::SM * 2.)
            .max_h(viewport.height - Spacing::SM * 2.)
            .gap(Spacing::MD)
            .p(Spacing::MD)
            .rounded(Radii::XL)
            .bg(palette.surface)
            .border_1()
            .border_color(palette.border)
            .shadow(palette.brand.card_shadow())
            .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
            .child(
                h_flex()
                    .w_full()
                    .items_start()
                    .justify_between()
                    .gap(Spacing::XS)
                    .flex_shrink_0()
                    .child(
                        v_flex()
                            .min_w_0()
                            .gap(Spacing::XXS)
                            .child(
                                Label::new("Atención automática")
                                    .text_size(Typography::LG)
                                    .font_bold(),
                            )
                            .child(
                                h_flex()
                                    .flex_wrap()
                                    .items_center()
                                    .gap_x(Spacing::SM)
                                    .child(
                                        Label::new(name).text_size(Typography::SM).font_semibold(),
                                    )
                                    .child(
                                        Label::new(metadata)
                                            .text_size(Typography::XS)
                                            .text_color(palette.muted_foreground),
                                    ),
                            ),
                    )
                    .child(
                        AppButton::new("attendance-close")
                            .tertiary()
                            .xsmall()
                            .icon(IconName::Close)
                            .tooltip("Cerrar")
                            .disabled(draft.submitting)
                            .on_click(cx.listener(|view, _, _, cx| view.cancel(cx))),
                    ),
            )
            .when(draft.loading_context, |modal| {
                modal.child(
                    loading::region("attendance-form-loading", "Cargando datos de la atención")
                        .w_full()
                        .child(loading::block("order", px(88.)))
                        .child(loading::block("balance", px(42.)))
                        .child(loading::block("pickup", px(36.))),
                )
            })
            .when(
                context.is_some() && draft.pending_request.is_none() && !draft.loading_context,
                |modal| {
                    modal.child(
                        v_flex()
                            .id("attendance-content")
                            .min_h_0()
                            .w_full()
                            .overflow_y_scroll()
                            .gap(Spacing::SM)
                            .child(self.consumption_picker(draft, cx))
                            .when(!draft.cart.is_empty(), |body| {
                                body.child(self.cart_panel(draft, cx))
                            })
                            .child(self.pickup_panel(draft, cx))
                            .child(self.ticket_panel(tickets, ticket_unit_cents, cash_open, cx))
                            .when_some(
                                summary.as_ref().and_then(|value| value.as_ref().err()),
                                |body, error| {
                                    body.child(
                                        Alert::error("attendance-summary-error", error.message())
                                            .xsmall(),
                                    )
                                },
                            ),
                    )
                },
            )
            .child(
                v_flex()
                    .flex_shrink_0()
                    .gap(Spacing::SM)
                    .when_some(summary.and_then(Result::ok), |footer, summary| {
                        footer.child(balance_summary(balance, summary, cx))
                    })
                    .when_some(draft.error.clone(), |footer, error| {
                        footer.child(Alert::error("attendance-submit-error", error).xsmall())
                    })
                    .child(
                        h_flex()
                            .gap(Spacing::SM)
                            .justify_end()
                            .flex_wrap()
                            .child(
                                AppButton::new("attendance-cancel")
                                    .secondary()
                                    .small()
                                    .label("Cancelar")
                                    .disabled(draft.submitting)
                                    .on_click(cx.listener(|view, _, _, cx| view.cancel(cx))),
                            )
                            .when(context.is_none() && !draft.loading_context, |footer| {
                                footer.child(
                                    AppButton::new("attendance-reload-context")
                                        .secondary()
                                        .small()
                                        .label("Reintentar consulta")
                                        .on_click(
                                            cx.listener(|view, _, _, cx| view.load_context(cx)),
                                        ),
                                )
                            })
                            .when(context.is_some(), |footer| {
                                footer.child(
                                    AppButton::new("attendance-confirm")
                                        .small()
                                        .icon(IconName::Check)
                                        .label(
                                            if draft.pending_request.is_some() && !draft.submitting
                                            {
                                                "Reintentar confirmación"
                                            } else {
                                                "Confirmar atención"
                                            },
                                        )
                                        .disabled(!can_submit)
                                        .loading(draft.submitting)
                                        .on_click(cx.listener(|view, _, _, cx| view.submit(cx))),
                                )
                            }),
                    ),
            );

        deferred(
            div()
                .id("attendance-overlay")
                .track_focus(&self.focus)
                .occlude()
                .absolute()
                .inset_0()
                .flex()
                .items_center()
                .justify_center()
                .bg(black().alpha(0.45))
                .capture_key_down(cx.listener(|view, event: &KeyDownEvent, window, cx| {
                    if event.keystroke.key == "escape" {
                        cx.stop_propagation();
                        window.prevent_default();
                        view.cancel(cx);
                    }
                }))
                .on_key_down(cx.listener(move |view, event: &KeyDownEvent, window, cx| {
                    if event.keystroke.key != "enter"
                        || event.is_held
                        || event.keystroke.modifiers.modified()
                    {
                        return;
                    }

                    if view
                        .special_price
                        .read(cx)
                        .focus_handle(cx)
                        .is_focused(window)
                    {
                        view.add_item(cx);
                    } else if can_submit
                        && (view.focus.is_focused(window)
                            || [&view.ticket_input, &view.pickup_name]
                                .iter()
                                .any(|input| input.read(cx).focus_handle(cx).is_focused(window)))
                    {
                        view.submit(cx);
                    } else {
                        return;
                    }

                    cx.stop_propagation();
                    window.prevent_default();
                }))
                .child(modal)
                .focus_trap("attendance-focus", &self.focus),
        )
        .with_priority(2)
        .into_any_element()
    }
}

fn step_button(id: impl Into<ElementId>, up: bool) -> AppButton {
    AppButton::new(id)
        .secondary()
        .xsmall()
        .icon(if up { IconName::Plus } else { IconName::Minus })
        .tooltip(if up {
            "Aumentar cantidad"
        } else {
            "Reducir cantidad"
        })
}

fn balance_summary(balance: i64, summary: AttendanceSummary, cx: &App) -> impl IntoElement {
    let palette = cx.dylan_theme().palette;
    let (projected_label, foreground) = match summary.projected_balance_cents.cmp(&0) {
        std::cmp::Ordering::Less => ("Deuda resultante", palette.error.foreground),
        std::cmp::Ordering::Equal => ("Saldo al día", palette.foreground),
        std::cmp::Ordering::Greater => ("Saldo a favor", palette.success.foreground),
    };
    let totals = [
        (
            "Saldo anterior".into(),
            format_soles(balance),
            palette.foreground,
        ),
        (
            format!("Consumo · {} rac.", summary.breakfasts + summary.lunches),
            format_soles(summary.consumption_cents),
            palette.foreground,
        ),
        (
            "Abono con tickets".into(),
            format_soles(summary.ticket_amount_cents),
            palette.foreground,
        ),
        (
            projected_label.into(),
            format_soles(summary.projected_balance_cents.saturating_abs()),
            foreground,
        ),
    ];

    h_flex()
        .flex_wrap()
        .gap(Spacing::SM)
        .p(Spacing::SM)
        .rounded(Radii::LG)
        .bg(palette.surface_subtle)
        .children(totals.map(|(label, amount, foreground)| {
            v_flex()
                .flex_1()
                .min_w(px(104.))
                .child(
                    Label::new(label)
                        .text_size(Typography::XS)
                        .text_color(palette.muted_foreground),
                )
                .child(
                    Label::new(amount)
                        .text_size(Typography::SM)
                        .font_semibold()
                        .text_color(foreground),
                )
        }))
}

#[cfg(test)]
mod tests {
    use super::{PickupChoice, SearchableListItem};

    #[test]
    fn pickup_search_keeps_manual_entry_available_when_no_name_matches() {
        let registered = PickupChoice::Registered("Ana Lucía Quispe".into());

        assert!(registered.matches("quispe lucia"));
        assert!(!registered.matches("María Ramos"));
        assert!(PickupChoice::Other.matches("María Ramos"));
    }
}
