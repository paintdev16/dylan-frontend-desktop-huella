use chrono::{Local, Timelike as _};
use gpui_kit::{
    component::{
        IconName, StyledExt as _,
        checkbox::Checkbox,
        h_flex,
        input::{Input, InputEvent, InputState},
        label::Label,
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
    features::pensioners::{
        SelectionSource,
        model::{PensionerId, format_soles},
    },
    state::{
        session::SessionState,
        store::{AttendanceRecord, ComedorStore, parse_soles_to_cents},
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

struct ActiveDraft {
    origin: AttendanceOrigin,
    pensioner_id: PensionerId,
    idempotency_key: String,
    meal: Meal,
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
    special_price: Entity<InputState>,
    pickup_name: Entity<InputState>,
    ticket_input: Entity<InputState>,
    draft: Option<ActiveDraft>,
    next_draft_seq: u64,
    _subscriptions: [Subscription; 5],
}

impl EventEmitter<AttendanceModalEvent> for AttendanceModal {}

impl AttendanceModal {
    pub fn new(
        store: Entity<ComedorStore>,
        session: Entity<SessionState>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let special_price = cx.new(|cx| InputState::new(window, cx).placeholder("12.00"));
        let pickup_name =
            cx.new(|cx| InputState::new(window, cx).placeholder("Nombre de quien recoge"));
        let ticket_input = cx.new(|cx| InputState::new(window, cx));
        let observe_store = cx.observe(&store, |_, _, cx| cx.notify());
        let observe_session = cx.observe(&session, |_, _, cx| cx.notify());
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
            special_price,
            pickup_name,
            ticket_input,
            draft: None,
            next_draft_seq: 1,
            _subscriptions: [
                observe_store,
                observe_session,
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
        if self.is_open()
            || !self.session.read(cx).can_register_attendance()
            || !self
                .store
                .read(cx)
                .ledger()
                .find_pensioner(pensioner_id)
                .is_some_and(|p| p.active)
        {
            return false;
        }

        let seq = self.next_draft_seq;
        self.next_draft_seq += 1;
        self.special_price
            .update(cx, |input, cx| input.set_value("12.00", window, cx));
        self.pickup_name
            .update(cx, |input, cx| input.clean(window, cx));
        self.ticket_input
            .update(cx, |input, cx| input.set_value("0", window, cx));
        self.draft = Some(ActiveDraft {
            origin,
            pensioner_id,
            idempotency_key: format!("att-{seq}-p{}", pensioner_id.0),
            meal: Meal::for_hour(Local::now().hour()),
            special: false,
            portions: 1,
            cart: Vec::new(),
            third_party: false,
            submitting: false,
            error: None,
        });
        window.focus(&self.focus, cx);
        cx.notify();
        true
    }

    pub fn close_for_sign_out(&mut self, cx: &mut Context<Self>) {
        if self.draft.take().is_some() {
            cx.notify();
        }
    }

    fn cancel(&mut self, cx: &mut Context<Self>) {
        if let Some(draft) = self.draft.take() {
            cx.emit(AttendanceModalEvent::Cancelled {
                origin: draft.origin,
            });
            cx.notify();
        }
    }

    fn edit(&mut self, cx: &mut Context<Self>, edit: impl FnOnce(&mut ActiveDraft)) {
        if let Some(draft) = self.draft.as_mut().filter(|draft| !draft.submitting) {
            draft.error = None;
            edit(draft);
            cx.notify();
        }
    }

    fn add_item(&mut self, cx: &mut Context<Self>) {
        let price =
            parse_soles_to_cents(&self.special_price.read(cx).value()).filter(|cents| *cents > 0);
        self.edit(cx, |draft| {
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
                    meal: draft.meal,
                    menu,
                    portions: draft.portions,
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
        let Some(draft) = self.draft.as_mut().filter(|draft| !draft.submitting) else {
            return;
        };
        let Some(tickets_count) = parse_ticket_count(&self.ticket_input.read(cx).value()) else {
            draft.error = Some("Ingresa una cantidad entera válida de tickets.".into());
            cx.notify();
            return;
        };
        let picked_up_by = draft
            .third_party
            .then(|| self.pickup_name.read(cx).value().to_string());
        let session = self.session.read(cx);
        let request = AttendanceRequest {
            idempotency_key: draft.idempotency_key.clone(),
            pensioner_id: draft.pensioner_id,
            items: draft.cart.clone(),
            picked_up_by,
            tickets_count,
            source: draft.origin.source(),
            operator_name: session
                .current_user()
                .map(|user| user.name.to_string())
                .unwrap_or_default(),
            can_register_attendance: session.can_register_attendance(),
        };
        let origin = draft.origin;
        draft.submitting = true;
        draft.error = None;
        cx.notify();

        match self
            .store
            .update(cx, |store, cx| store.confirm_attendance(request, cx))
        {
            Ok(record) => {
                self.draft = None;
                cx.emit(AttendanceModalEvent::Confirmed { origin, record });
            }
            Err(err) => {
                if let Some(draft) = &mut self.draft {
                    draft.submitting = false;
                    draft.error = Some(err.message().into());
                }
            }
        }
        cx.notify();
    }

    fn consumption_picker(&self, draft: &ActiveDraft, cx: &mut Context<Self>) -> impl IntoElement {
        let palette = cx.dylan_theme().palette;
        section("Agregar consumo", cx)
            .child(
                h_flex()
                    .flex_wrap()
                    .gap(Spacing::XS)
                    .children(Meal::ALL.map(|meal| {
                        let button = AppButton::new(meal.label())
                            .small()
                            .label(meal.label())
                            .on_click(cx.listener(move |view, _, _, cx| {
                                view.edit(cx, |draft| draft.meal = meal)
                            }));
                        if meal == draft.meal {
                            button.primary()
                        } else {
                            button.secondary()
                        }
                    })),
            )
            .child(
                h_flex()
                    .flex_wrap()
                    .gap(Spacing::XS)
                    .children([false, true].map(|special| {
                        let (id, label) = if special {
                            ("menu-special", "Especial".into())
                        } else {
                            (
                                "menu-economic",
                                format!(
                                    "Económico ({})",
                                    format_soles(draft.meal.economic_price())
                                ),
                            )
                        };
                        let button = AppButton::new(id)
                            .small()
                            .label(label)
                            .on_click(cx.listener(move |view, _, _, cx| {
                                view.edit(cx, |draft| draft.special = special)
                            }));
                        if draft.special == special {
                            button.primary()
                        } else {
                            button.secondary()
                        }
                    }))
                    .when(draft.special, |row| {
                        row.child(
                            h_flex()
                                .items_center()
                                .gap(Spacing::XS)
                                .child(Label::new("Precio unitario (S/)").text_size(Typography::SM))
                                .child(
                                    div()
                                        .w(px(94.))
                                        .child(Input::new(&self.special_price).bg(palette.surface)),
                                ),
                        )
                    }),
            )
            .child(
                h_flex()
                    .flex_wrap()
                    .justify_between()
                    .items_center()
                    .gap(Spacing::SM)
                    .child(
                        h_flex()
                            .gap(Spacing::SM)
                            .items_center()
                            .child(Label::new("Raciones").text_size(Typography::SM))
                            .child(
                                step_button("portions-down", false)
                                    .disabled(draft.portions <= 1)
                                    .on_click(cx.listener(|view, _, _, cx| {
                                        view.edit(cx, |draft| {
                                            draft.portions = draft.portions.saturating_sub(1).max(1)
                                        })
                                    })),
                            )
                            .child(Label::new(draft.portions.to_string()).font_semibold())
                            .child(
                                step_button("portions-up", true)
                                    .disabled(draft.portions == u32::MAX)
                                    .on_click(cx.listener(|view, _, _, cx| {
                                        view.edit(cx, |draft| {
                                            draft.portions = draft.portions.saturating_add(1)
                                        })
                                    })),
                            ),
                    )
                    .child(
                        AppButton::new("add-consumption")
                            .small()
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
        section("Carrito de consumos", cx)
            .when(draft.cart.is_empty(), |panel| {
                panel.child(
                    Label::new("El carrito está vacío.")
                        .text_size(Typography::SM)
                        .text_color(palette.muted_foreground),
                )
            })
            .children(draft.cart.iter().enumerate().map(|(index, item)| {
                let unit = item.menu.unit_cents(item.meal);
                let subtotal = unit
                    .checked_mul(i64::from(item.portions))
                    .map(format_soles)
                    .unwrap_or_else(|| "Importe fuera de rango".into());
                h_flex()
                    .w_full()
                    .flex_wrap()
                    .items_center()
                    .justify_between()
                    .gap(Spacing::SM)
                    .border_t_1()
                    .border_color(palette.border)
                    .pt(Spacing::XS)
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
                                Label::new(format!(
                                    "{} por ración · {subtotal}",
                                    format_soles(unit)
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
            .when(!draft.cart.is_empty(), |panel| {
                panel
                    .child(
                        Checkbox::new("third-party-pickup")
                            .label("Recogido por tercero")
                            .checked(draft.third_party)
                            .on_change(cx.listener(|view, checked, _, cx| {
                                view.edit(cx, |draft| draft.third_party = *checked)
                            })),
                    )
                    .when(draft.third_party, |panel| {
                        panel.child(Input::new(&self.pickup_name).w_full().bg(palette.surface))
                    })
            })
    }

    fn ticket_panel(
        &self,
        tickets: Option<u32>,
        ticket_unit_cents: i64,
        balance: i64,
        projected_balance_cents: Option<i64>,
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
        let tickets_value_cents = i64::from(count).saturating_mul(ticket_unit_cents);
        let projected = projected_balance_cents.unwrap_or(balance);

        let helper_text = if count == 0 {
            if balance > 0 {
                format!(
                    "Se descontará del saldo a favor actual ({}).",
                    format_soles(balance)
                )
            } else {
                "Modo actual: registrar consumo como deuda.".to_string()
            }
        } else if projected > 0 {
            format!(
                "{count} ticket(s) = {} abonados · {} quedará a favor",
                format_soles(tickets_value_cents),
                format_soles(projected)
            )
        } else if projected < 0 {
            format!(
                "{count} ticket(s) = {} abonados (deuda final: {})",
                format_soles(tickets_value_cents),
                format_soles(projected.saturating_abs())
            )
        } else {
            format!(
                "{count} ticket(s) = {} abonados (queda al día)",
                format_soles(tickets_value_cents)
            )
        };

        section("Tickets físicos entregados", cx)
            .child(
                h_flex()
                    .items_center()
                    .flex_wrap()
                    .justify_between()
                    .gap(Spacing::SM)
                    .child(
                        v_flex()
                            .gap(Spacing::XXS)
                            .child(
                                Label::new(format!(
                                    "Valor por ticket: {}",
                                    format_soles(ticket_unit_cents)
                                ))
                                .text_size(Typography::SM)
                                .font_medium()
                                .text_color(palette.foreground),
                            )
                            .child(
                                Label::new(helper_text)
                                    .text_size(Typography::XS)
                                    .text_color(palette.muted_foreground),
                            ),
                    )
                    .child(StatusBadge::new(
                        if cash_open {
                            StatusTone::Success
                        } else {
                            StatusTone::Warning
                        },
                        if cash_open {
                            "Caja abierta"
                        } else {
                            "Caja cerrada"
                        },
                    )),
            )
            .child(
                h_flex()
                    .flex_wrap()
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
                            .w(px(80.))
                            .child(Input::new(&self.ticket_input).bg(palette.surface)),
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
                                .small()
                                .label("Solo deuda")
                                .on_click(cx.listener(|view, _, window, cx| {
                                    view.set_tickets(0, window, cx)
                                })),
                        )
                    }),
            )
            .when(tickets.is_none(), |panel| {
                panel.child(
                    Label::new("Ingresa una cantidad entera válida.")
                        .text_size(Typography::SM)
                        .text_color(palette.error.foreground),
                )
            })
            .when(count > 0 && !cash_open, |panel| {
                panel.child(
                    Label::new(
                        "La caja está cerrada: ábrela en Caja para recibir tickets, o deja 0 tickets para registrar como deuda.",
                    )
                    .text_size(Typography::SM)
                    .text_color(palette.warning.foreground),
                )
            })
            .when(cash_open && count > 0 && projected > 0, |panel| {
                panel.child(
                    Label::new(format!(
                        "El excedente ({}) se registrará como saldo a favor y se descontará automáticamente de futuros consumos.",
                        format_soles(projected)
                    ))
                    .text_size(Typography::XS)
                    .text_color(palette.brand_accent),
                )
            })
    }
}

impl Render for AttendanceModal {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let Some(draft) = &self.draft else {
            return div().into_any_element();
        };
        let ledger = self.store.read(cx).ledger();
        let Some(pensioner) = ledger.find_pensioner(draft.pensioner_id) else {
            return div().into_any_element();
        };
        let cash_open = ledger.is_cash_open();
        let ticket_unit_cents = ledger.ticket_value_cents();
        let can_register = self.session.read(cx).can_register_attendance() && pensioner.active;
        let name = pensioner.full_name.clone();
        let metadata = [
            pensioner.code(),
            pensioner.dni.clone(),
            draft.origin.source().label().into(),
        ]
        .into_iter()
        .filter(|text| !text.is_empty())
        .collect::<Vec<_>>()
        .join(" · ");
        let balance = pensioner.balance_cents;
        let tickets = parse_ticket_count(&self.ticket_input.read(cx).value());
        let summary = tickets.map(|count| {
            AttendanceSummary::calculate(balance, &draft.cart, count, ticket_unit_cents)
        });
        let projected_balance = summary
            .as_ref()
            .and_then(|res| res.as_ref().ok())
            .map(|s| s.projected_balance_cents);
        let can_submit = summary.as_ref().is_some_and(Result::is_ok)
            && (cash_open || tickets == Some(0))
            && (!draft.cart.is_empty() || tickets != Some(0))
            && can_register
            && !draft.submitting;

        let palette = cx.dylan_theme().palette;
        let viewport = window.viewport_size();
        let padding = if viewport.width < px(600.) {
            Spacing::MD
        } else {
            Spacing::LG
        };
        let modal = v_flex()
            .id("attendance-card")
            .occlude()
            .w(px(680.))
            .max_w(viewport.width - Spacing::MD * 2.)
            .max_h(viewport.height - Spacing::SM * 2.)
            .gap(Spacing::SM)
            .p(padding)
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
                                    .text_size(Typography::XL)
                                    .font_bold(),
                            )
                            .child(Label::new(name).font_semibold())
                            .child(
                                Label::new(metadata)
                                    .text_size(Typography::XS)
                                    .text_color(palette.muted_foreground),
                            ),
                    )
                    .child(
                        AppButton::new("attendance-close")
                            .tertiary()
                            .small()
                            .icon(IconName::Close)
                            .tooltip("Cerrar")
                            .on_click(cx.listener(|view, _, _, cx| view.cancel(cx))),
                    ),
            )
            .child(
                v_flex()
                    .id("attendance-content")
                    .min_h_0()
                    .w_full()
                    .overflow_y_scroll()
                    .gap(Spacing::SM)
                    .p(Spacing::XS)
                    .child(self.consumption_picker(draft, cx))
                    .child(self.cart_panel(draft, cx))
                    .child(self.ticket_panel(
                        tickets,
                        ticket_unit_cents,
                        balance,
                        projected_balance,
                        cash_open,
                        cx,
                    ))
                    .when_some(summary, |body, summary| {
                        body.child(match summary {
                            Ok(summary) => {
                                balance_summary(balance, tickets.unwrap_or(0), summary, cx)
                                    .into_any_element()
                            }
                            Err(error) => Label::new(error.message())
                                .text_color(palette.error.foreground)
                                .into_any_element(),
                        })
                    }),
            )
            .child(
                v_flex()
                    .flex_shrink_0()
                    .gap(Spacing::XS)
                    .border_t_1()
                    .border_color(palette.border)
                    .pt(Spacing::SM)
                    .when_some(draft.error.clone(), |footer, error| {
                        footer.child(
                            Label::new(error)
                                .text_size(Typography::SM)
                                .text_color(palette.error.foreground),
                        )
                    })
                    .child(
                        h_flex()
                            .gap(Spacing::SM)
                            .justify_end()
                            .flex_wrap()
                            .child(
                                AppButton::new("attendance-cancel")
                                    .secondary()
                                    .label("Cancelar")
                                    .disabled(draft.submitting)
                                    .on_click(cx.listener(|view, _, _, cx| view.cancel(cx))),
                            )
                            .child(
                                AppButton::new("attendance-confirm")
                                    .icon(IconName::Check)
                                    .label("Confirmar atención")
                                    .disabled(!can_submit)
                                    .loading(draft.submitting)
                                    .on_click(cx.listener(|view, _, _, cx| view.submit(cx))),
                            ),
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
                .child(modal),
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

fn section(title: &'static str, cx: &App) -> Div {
    let palette = cx.dylan_theme().palette;
    v_flex()
        .w_full()
        .flex_shrink_0()
        .p(Spacing::SM)
        .gap(Spacing::SM)
        .rounded(Radii::MD)
        .bg(palette.surface_subtle)
        .border_1()
        .border_color(palette.border)
        .child(Label::new(title).font_semibold())
}

fn balance_summary(
    balance: i64,
    tickets_count: u32,
    summary: AttendanceSummary,
    cx: &App,
) -> impl IntoElement {
    let palette = cx.dylan_theme().palette;
    let previous_label = match balance.cmp(&0) {
        std::cmp::Ordering::Greater => format!("Saldo a favor anterior: {}", format_soles(balance)),
        std::cmp::Ordering::Less => {
            format!("Deuda anterior: {}", format_soles(balance.saturating_abs()))
        }
        std::cmp::Ordering::Equal => "Saldo anterior: S/ 0.00".to_string(),
    };
    let tickets_line = if tickets_count > 0 {
        format!(
            "Tickets recibidos: {tickets_count} × {} = {}",
            format_soles(summary.ticket_unit_cents),
            format_soles(summary.ticket_amount_cents)
        )
    } else {
        "Tickets recibidos: 0 (S/ 0.00)".to_string()
    };
    let (projected_label, foreground) = match summary.projected_balance_cents.cmp(&0) {
        std::cmp::Ordering::Less => ("Saldo resultante: Deuda", palette.error.foreground),
        std::cmp::Ordering::Equal => ("Saldo resultante: Al día", palette.muted_foreground),
        std::cmp::Ordering::Greater => ("Saldo resultante: A favor", palette.success.foreground),
    };
    section("Resumen de atención", cx)
        .child(
            Label::new(previous_label)
                .text_size(Typography::SM)
                .text_color(palette.muted_foreground),
        )
        .child(
            Label::new(format!(
                "Consumo actual ({} raciones): {}",
                summary.breakfasts + summary.lunches,
                format_soles(summary.consumption_cents)
            ))
            .text_size(Typography::SM),
        )
        .child(
            Label::new(tickets_line)
                .text_size(Typography::SM)
                .text_color(palette.muted_foreground),
        )
        .child(
            Label::new(format!(
                "{projected_label} {}",
                format_soles(summary.projected_balance_cents.saturating_abs())
            ))
            .font_semibold()
            .text_color(foreground),
        )
}
