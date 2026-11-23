use gpui_kit::{
    component::{
        Icon, IconName, Sizable as _, StyledExt as _, h_flex,
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
        metric_card::MetricCard,
        status_badge::{StatusBadge, StatusTone},
    },
    features::pensioners::model::{PensionerId, format_soles},
    state::{
        session::SessionState,
        store::{CashMovement, CashMovementKind, ComedorStore, parse_soles_to_cents},
    },
    theme::{ActiveDylanTheme as _, MetricAccent, Radii, Spacing, Typography},
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CashEvent {
    OpenPensioner(PensionerId),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum CashFilter {
    #[default]
    All,
    CashPayments,
    Tickets,
    Audit,
}

impl CashFilter {
    pub const ALL: [Self; 4] = [Self::All, Self::CashPayments, Self::Tickets, Self::Audit];

    pub const fn label(self) -> &'static str {
        match self {
            Self::All => "Todos",
            Self::CashPayments => "Pagos en efectivo",
            Self::Tickets => "Tickets físicos",
            Self::Audit => "Apertura y cierre",
        }
    }

    pub const fn matches_kind(self, kind: CashMovementKind) -> bool {
        match self {
            Self::All => true,
            Self::CashPayments => matches!(kind, CashMovementKind::CashPayment),
            Self::Tickets => matches!(kind, CashMovementKind::TicketCredit),
            Self::Audit => matches!(kind, CashMovementKind::Opening | CashMovementKind::Closing),
        }
    }
}

/// Módulo de **Caja y control financiero**: apertura y cierre de caja, configuración del
/// valor de ticket físico, arqueo, auditoría filtrable y vista previa de comprobantes.
/// La entrega diaria de tickets físicos se centraliza en **Atención automática**.
pub struct CashView {
    store: Entity<ComedorStore>,
    session: Entity<SessionState>,
    opening_cash_input: Entity<InputState>,
    counted_tickets_input: Entity<InputState>,
    counted_cash_input: Entity<InputState>,
    close_notes_input: Entity<InputState>,
    ticket_value_input: Entity<InputState>,
    search_input: Entity<InputState>,
    filter: CashFilter,
    selected_pensioner: Option<PensionerId>,
    editing_ticket_value: bool,
    receipt_movement_id: Option<String>,
    receipt_focus: FocusHandle,
    feedback: Option<(StatusTone, SharedString)>,
    _subscriptions: Vec<Subscription>,
}

impl EventEmitter<CashEvent> for CashView {}

impl CashView {
    pub fn new(
        store: Entity<ComedorStore>,
        session: Entity<SessionState>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let current_ticket = store.read(cx).ledger().ticket_value_cents();
        let opening_cash_input = cx.new(|cx| {
            InputState::new(window, cx)
                .placeholder("100.00")
                .default_value("100.00")
        });
        let counted_tickets_input = cx.new(|cx| InputState::new(window, cx).placeholder("0"));
        let counted_cash_input = cx.new(|cx| InputState::new(window, cx).placeholder("0.00"));
        let close_notes_input = cx.new(|cx| {
            InputState::new(window, cx).placeholder("Observaciones del cierre (opcional)")
        });
        let ticket_value_input = cx.new(|cx| {
            InputState::new(window, cx)
                .placeholder("7.00")
                .default_value(cents_to_plain_soles(current_ticket))
        });
        let search_input = cx.new(|cx| {
            InputState::new(window, cx)
                .placeholder("Buscar por código MOV, pensionista, concepto o responsable…")
        });

        let observe_store = cx.observe(&store, |_, _, cx| cx.notify());
        let observe_session = cx.observe(&session, |_, _, cx| cx.notify());
        let sub_open = cx.subscribe(&opening_cash_input, |view, _, event, cx| {
            if matches!(event, InputEvent::Change) && view.feedback.is_some() {
                view.feedback = None;
                cx.notify();
            }
        });
        let sub_tickets = cx.subscribe(&counted_tickets_input, |view, _, event, cx| {
            if matches!(event, InputEvent::Change) && view.feedback.is_some() {
                view.feedback = None;
                cx.notify();
            }
        });
        let sub_cash = cx.subscribe(&counted_cash_input, |view, _, event, cx| {
            if matches!(event, InputEvent::Change) && view.feedback.is_some() {
                view.feedback = None;
                cx.notify();
            }
        });
        let sub_ticket_val = cx.subscribe(&ticket_value_input, |view, _, event, cx| {
            if matches!(event, InputEvent::Change) && view.feedback.is_some() {
                view.feedback = None;
                cx.notify();
            }
        });
        let sub_search = cx.subscribe(&search_input, |_, _, event, cx| {
            if matches!(event, InputEvent::Change) {
                cx.notify();
            }
        });

        Self {
            store,
            session,
            opening_cash_input,
            counted_tickets_input,
            counted_cash_input,
            close_notes_input,
            ticket_value_input,
            search_input,
            filter: CashFilter::All,
            selected_pensioner: None,
            editing_ticket_value: false,
            receipt_movement_id: None,
            receipt_focus: cx.focus_handle(),
            feedback: None,
            _subscriptions: vec![
                observe_store,
                observe_session,
                sub_open,
                sub_tickets,
                sub_cash,
                sub_ticket_val,
                sub_search,
            ],
        }
    }

    fn operator_name(&self, cx: &App) -> String {
        self.session
            .read(cx)
            .current_user()
            .map(|u| u.name.to_string())
            .unwrap_or_else(|| "Operador".to_string())
    }

    fn open_ticket_editor(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let current = self.store.read(cx).ledger().ticket_value_cents();
        self.ticket_value_input.update(cx, |input, cx| {
            input.set_value(cents_to_plain_soles(current), window, cx);
            input.focus(window, cx);
        });
        self.editing_ticket_value = true;
        self.feedback = None;
        cx.notify();
    }

    fn save_ticket_value(&mut self, cx: &mut Context<Self>) {
        let raw = self.ticket_value_input.read(cx).value();
        let Some(cents) = parse_soles_to_cents(raw.as_ref()).filter(|value| *value > 0) else {
            self.feedback = Some((
                StatusTone::Error,
                SharedString::from("Ingresa un valor de ticket válido mayor a S/ 0.00 (ej. 7.00)."),
            ));
            cx.notify();
            return;
        };

        match self
            .store
            .update(cx, |store, cx| store.set_ticket_value_cents(cents, cx))
        {
            Ok(()) => {
                self.editing_ticket_value = false;
                self.feedback = Some((
                    StatusTone::Success,
                    SharedString::from(format!(
                        "Valor del ticket físico actualizado a {}.",
                        format_soles(cents)
                    )),
                ));
            }
            Err(err) => {
                self.feedback = Some((StatusTone::Error, SharedString::from(err.message())));
            }
        }
        cx.notify();
    }

    fn handle_open_cash(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let raw = self.opening_cash_input.read(cx).value();
        let Some(opening_cents) = parse_soles_to_cents(raw.as_ref()) else {
            self.feedback = Some((
                StatusTone::Error,
                SharedString::from("Ingresa un monto de apertura válido (ej. 0.00 o 100.00)."),
            ));
            cx.notify();
            return;
        };

        let operator = self.operator_name(cx);
        let result = self
            .store
            .update(cx, |store, cx| store.open_cash(operator, opening_cents, cx));

        match result {
            Ok(()) => {
                let opening_text = cents_to_plain_soles(opening_cents);
                self.counted_tickets_input
                    .update(cx, |input, cx| input.set_value("0", window, cx));
                self.counted_cash_input
                    .update(cx, |input, cx| input.set_value(opening_text, window, cx));
                self.close_notes_input
                    .update(cx, |input, cx| input.clean(window, cx));
                self.feedback = Some((
                    StatusTone::Success,
                    SharedString::from(format!(
                        "Caja abierta con monto inicial de {}.",
                        format_soles(opening_cents)
                    )),
                ));
            }
            Err(err) => {
                self.feedback = Some((StatusTone::Error, SharedString::from(err.message())));
            }
        }
        cx.notify();
    }

    fn prefill_expected(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let expected = self
            .store
            .read(cx)
            .ledger()
            .active_cash()
            .map(|cash| (cash.total_tickets(), cash.expected_cash_cents()));

        if let Some((tickets, cash_cents)) = expected {
            self.counted_tickets_input.update(cx, |input, cx| {
                input.set_value(tickets.to_string(), window, cx)
            });
            self.counted_cash_input.update(cx, |input, cx| {
                input.set_value(cents_to_plain_soles(cash_cents), window, cx)
            });
            self.feedback = None;
            cx.notify();
        }
    }

    fn parse_audit_inputs(&self, cx: &App) -> Result<(u32, i64), &'static str> {
        let raw_tickets = self.counted_tickets_input.read(cx).value();
        let trimmed_tickets = raw_tickets.trim();
        let counted_tickets: u32 = if trimmed_tickets.is_empty() {
            0
        } else {
            trimmed_tickets
                .parse()
                .map_err(|_| "Ingresa una cantidad entera válida de tickets contados.")?
        };

        let raw_cash = self.counted_cash_input.read(cx).value();
        let counted_cash_cents = parse_soles_to_cents(raw_cash.as_ref())
            .ok_or("Ingresa un monto válido de efectivo contado (ej. 100.00).")?;

        Ok((counted_tickets, counted_cash_cents))
    }

    fn handle_audit_cash(&mut self, cx: &mut Context<Self>) {
        let (counted_tickets, counted_cash_cents) = match self.parse_audit_inputs(cx) {
            Ok(values) => values,
            Err(msg) => {
                self.feedback = Some((StatusTone::Error, SharedString::from(msg)));
                cx.notify();
                return;
            }
        };

        let result = self.store.update(cx, |store, cx| {
            store.audit_cash(counted_tickets, counted_cash_cents, cx)
        });

        match result {
            Ok(audit) => {
                let (tone, message) = if audit.is_balanced() {
                    (
                        StatusTone::Success,
                        "Arqueo cuadrado: tickets y efectivo coinciden con el sistema.".to_string(),
                    )
                } else {
                    (
                        StatusTone::Warning,
                        format!(
                            "Arqueo con diferencia: tickets {:+}, efectivo {}.",
                            audit.ticket_diff,
                            format_soles(audit.cash_diff_cents)
                        ),
                    )
                };
                self.feedback = Some((tone, SharedString::from(message)));
            }
            Err(err) => {
                self.feedback = Some((StatusTone::Error, SharedString::from(err.message())));
            }
        }
        cx.notify();
    }

    fn handle_close_cash(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let (counted_tickets, counted_cash_cents) = match self.parse_audit_inputs(cx) {
            Ok(values) => values,
            Err(msg) => {
                self.feedback = Some((StatusTone::Error, SharedString::from(msg)));
                cx.notify();
                return;
            }
        };

        let notes = self.close_notes_input.read(cx).value().to_string();
        let operator = self.operator_name(cx);
        let result = self.store.update(cx, |store, cx| {
            store.close_cash(operator, counted_tickets, counted_cash_cents, notes, cx)
        });

        match result {
            Ok(()) => {
                self.opening_cash_input
                    .update(cx, |input, cx| input.set_value("100.00", window, cx));
                self.counted_tickets_input
                    .update(cx, |input, cx| input.clean(window, cx));
                self.counted_cash_input
                    .update(cx, |input, cx| input.clean(window, cx));
                self.close_notes_input
                    .update(cx, |input, cx| input.clean(window, cx));
                self.feedback = Some((
                    StatusTone::Success,
                    SharedString::from("Turno de caja cerrado y archivado en el historial."),
                ));
            }
            Err(err) => {
                self.feedback = Some((StatusTone::Error, SharedString::from(err.message())));
            }
        }
        cx.notify();
    }

    fn open_receipt(&mut self, mov_id: String, window: &mut Window, cx: &mut Context<Self>) {
        self.receipt_movement_id = Some(mov_id);
        window.focus(&self.receipt_focus, cx);
        cx.notify();
    }

    fn close_receipt(&mut self, cx: &mut Context<Self>) {
        if self.receipt_movement_id.take().is_some() {
            cx.notify();
        }
    }

    fn render_receipt_modal(
        &self,
        movement: &CashMovement,
        current_balance_cents: Option<i64>,
        window: &Window,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let palette = cx.dylan_theme().palette;
        let viewport = window.viewport_size();
        let holder = match (&movement.pensioner_code, &movement.pensioner_name) {
            (Some(code), Some(name)) => format!("{name} ({code})"),
            (None, Some(name)) => name.clone(),
            _ => "Operación de caja general".to_string(),
        };

        let card = v_flex()
            .id("cash-receipt-card")
            .occlude()
            .w(px(460.0))
            .max_w(viewport.width - Spacing::MD * 2.0)
            .gap(Spacing::MD)
            .p(Spacing::LG)
            .rounded(Radii::XL)
            .bg(palette.surface)
            .border_1()
            .border_color(palette.border)
            .shadow(palette.brand.card_shadow())
            .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
            .child(
                h_flex()
                    .w_full()
                    .items_center()
                    .justify_between()
                    .gap(Spacing::SM)
                    .child(
                        v_flex()
                            .gap(Spacing::XXS)
                            .child(
                                Label::new(format!("Comprobante · {}", movement.id))
                                    .text_size(Typography::LG)
                                    .font_bold()
                                    .text_color(palette.foreground),
                            )
                            .child(
                                Label::new("Comedor Dylan · Auditoría de caja")
                                    .text_size(Typography::XS)
                                    .text_color(palette.muted_foreground),
                            ),
                    )
                    .child(movement_badge(movement.kind)),
            )
            .child(
                v_flex()
                    .w_full()
                    .gap(Spacing::XS)
                    .p(Spacing::MD)
                    .rounded(Radii::MD)
                    .bg(palette.surface_subtle)
                    .border_1()
                    .border_color(palette.border)
                    .child(
                        Label::new(format!(
                            "Fecha / Registro: {} · Responsable: {}",
                            movement.recorded_at, movement.operator_name
                        ))
                        .text_size(Typography::XS)
                        .text_color(palette.muted_foreground),
                    )
                    .child(
                        Label::new(format!("Titular: {holder}"))
                            .text_size(Typography::SM)
                            .font_semibold()
                            .text_color(palette.foreground),
                    )
                    .child(
                        Label::new(format!("Concepto: {}", movement.description))
                            .text_size(Typography::SM)
                            .text_color(palette.foreground),
                    )
                    .when_some(
                        movement.ticket_count.zip(movement.ticket_unit_cents),
                        |this, (count, unit)| {
                            this.child(
                                Label::new(format!(
                                    "Detalle de tickets: {count} unidad(es) × {}",
                                    format_soles(unit)
                                ))
                                .text_size(Typography::XS)
                                .text_color(palette.muted_foreground),
                            )
                        },
                    )
                    .when_some(movement.consumption_cents, |this, consumed| {
                        this.child(
                            Label::new(format!("Consumo asociado: {}", format_soles(consumed)))
                                .text_size(Typography::XS)
                                .text_color(palette.muted_foreground),
                        )
                    })
                    .when_some(movement.resulting_balance_cents, |this, res_bal| {
                        this.child(
                            Label::new(format!(
                                "Saldo resultante en esa operación: {}",
                                format_balance_label(res_bal)
                            ))
                            .text_size(Typography::XS)
                            .text_color(palette.muted_foreground),
                        )
                    })
                    .child(
                        h_flex()
                            .w_full()
                            .items_center()
                            .justify_between()
                            .pt(Spacing::SM)
                            .mt(Spacing::XS)
                            .border_t_1()
                            .border_color(palette.border)
                            .child(
                                Label::new("Importe:")
                                    .text_size(Typography::SM)
                                    .font_semibold()
                                    .text_color(palette.foreground),
                            )
                            .child(
                                Label::new(format_soles(movement.amount_cents))
                                    .text_size(Typography::LG)
                                    .font_bold()
                                    .text_color(palette.foreground),
                            ),
                    )
                    .when_some(current_balance_cents, |this, current| {
                        this.child(
                            Label::new(format!(
                                "Saldo actual del titular: {}",
                                format_balance_label(current)
                            ))
                            .text_size(Typography::XS)
                            .text_color(palette.muted_foreground),
                        )
                    }),
            )
            .child(
                h_flex().w_full().justify_end().child(
                    AppButton::new("cash-receipt-close")
                        .primary()
                        .label("Cerrar")
                        .on_click(cx.listener(|view, _, _, cx| view.close_receipt(cx))),
                ),
            );

        deferred(
            div()
                .id("cash-receipt-overlay")
                .track_focus(&self.receipt_focus)
                .occlude()
                .absolute()
                .inset_0()
                .flex()
                .items_center()
                .justify_center()
                .bg(black().alpha(0.45))
                .on_mouse_down(
                    MouseButton::Left,
                    cx.listener(|view, _, _, cx| view.close_receipt(cx)),
                )
                .capture_key_down(cx.listener(|view, event: &KeyDownEvent, window, cx| {
                    if event.keystroke.key == "escape" {
                        cx.stop_propagation();
                        window.prevent_default();
                        view.close_receipt(cx);
                    }
                }))
                .child(card),
        )
        .with_priority(2)
    }
}

fn cents_to_plain_soles(cents: i64) -> String {
    let abs = cents.unsigned_abs();
    format!("{}.{:02}", abs / 100, abs % 100)
}

fn format_balance_label(balance_cents: i64) -> String {
    if balance_cents > 0 {
        format!("A favor {}", format_soles(balance_cents))
    } else if balance_cents < 0 {
        format!("Deuda {}", format_soles(balance_cents.saturating_abs()))
    } else {
        "S/ 0.00 (Al día)".to_string()
    }
}

fn movement_badge(kind: CashMovementKind) -> StatusBadge {
    let tone = match kind {
        CashMovementKind::Opening => StatusTone::Info,
        CashMovementKind::Closing => StatusTone::Neutral,
        CashMovementKind::CashPayment => StatusTone::Success,
        CashMovementKind::TicketCredit => StatusTone::Info,
    };
    StatusBadge::new(tone, kind.label())
}

fn normalize_query(input: &str) -> String {
    input.trim().to_lowercase()
}

impl Render for CashView {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let palette = cx.dylan_theme().palette;
        let compact = window.viewport_size().width < px(640.0);
        let outer_padding = if compact { Spacing::MD } else { Spacing::XL };
        let card_padding = if compact { Spacing::MD } else { Spacing::LG };

        let store = self.store.read(cx);
        let ledger = store.ledger();
        let active_cash = ledger.active_cash().cloned();
        let ticket_unit_cents = ledger.ticket_value_cents();
        let total_credit = ledger.total_credit_balance_cents();
        let total_debt = ledger.total_pending_debt_cents();
        let closed_sessions: Vec<_> = ledger
            .closed_cash_sessions()
            .iter()
            .rev()
            .cloned()
            .collect();

        let (
            opening_value,
            opening_subtitle,
            cash_inflow_cents,
            expected_cash_cents,
            tickets_count,
            tickets_amount_cents,
        ) = match &active_cash {
            Some(cash) => (
                format_soles(cash.opening_cash_cents),
                format!("Turno #{} · {}", cash.id, cash.opened_at),
                cash.cash_inflow_cents,
                cash.expected_cash_cents(),
                cash.total_tickets(),
                cash.total_ticket_amount_cents(),
            ),
            None => (
                "S/ 0.00".to_string(),
                "Caja cerrada".to_string(),
                ledger.total_cash_inflow_cents(),
                0,
                ledger.total_tickets_received(),
                ledger.total_ticket_amount_cents(),
            ),
        };

        let selected_person = self
            .selected_pensioner
            .and_then(|pid| ledger.find_pensioner(pid).cloned())
            .map(|person| {
                let totals = ledger.pensioner_totals(person.id);
                (person, totals)
            });

        let query = normalize_query(self.search_input.read(cx).value().as_ref());
        let filtered_movements: Vec<CashMovement> = ledger
            .cash_movements()
            .iter()
            .rev()
            .filter(|m| {
                if !self.filter.matches_kind(m.kind) {
                    return false;
                }
                if let Some(pid) = self.selected_pensioner
                    && m.pensioner_id != Some(pid)
                {
                    return false;
                }
                if query.is_empty() {
                    return true;
                }
                let haystack = normalize_query(&format!(
                    "{} {} {} {} {} {}",
                    m.id,
                    m.kind.label(),
                    m.description,
                    m.operator_name,
                    m.pensioner_code.as_deref().unwrap_or_default(),
                    m.pensioner_name.as_deref().unwrap_or_default(),
                ));
                query.split_whitespace().all(|tok| haystack.contains(tok))
            })
            .cloned()
            .collect();

        let receipt_modal_data = self.receipt_movement_id.as_deref().and_then(|id| {
            ledger.find_cash_movement(id).cloned().map(|mov| {
                let current_balance = mov
                    .pensioner_id
                    .and_then(|pid| ledger.find_pensioner(pid))
                    .map(|p| p.balance_cents);
                (mov, current_balance)
            })
        });

        v_flex()
            .id("cash-scroll")
            .relative()
            .flex_1()
            .min_h_0()
            .w_full()
            .overflow_y_scroll()
            .items_center()
            .p(outer_padding)
            .child(
                v_flex()
                    .w_full()
                    .max_w(px(1080.0))
                    .gap(if compact { Spacing::MD } else { Spacing::LG })
                    .child(
                        v_flex()
                            .w_full()
                            .gap(Spacing::MD)
                            .p(card_padding)
                            .rounded(Radii::LG)
                            .bg(palette.surface)
                            .border_1()
                            .border_color(palette.border)
                            .child(
                                h_flex()
                                    .w_full()
                                    .flex_wrap()
                                    .items_center()
                                    .justify_between()
                                    .gap(Spacing::MD)
                                    .child(
                                        v_flex()
                                            .min_w_0()
                                            .gap(Spacing::XXS)
                                            .child(
                                                Label::new("Caja y control financiero")
                                                    .text_size(if compact {
                                                        Typography::XL
                                                    } else {
                                                        Typography::PAGE_TITLE
                                                    })
                                                    .font_bold()
                                                    .text_color(palette.foreground),
                                            )
                                            .child(
                                                Label::new(
                                                    "Apertura y cierre de caja, valor de ticket, arqueo y auditoría (la entrega de tickets se centraliza en Atención automática).",
                                                )
                                                .text_size(Typography::SM)
                                                .text_color(palette.muted_foreground),
                                            ),
                                    )
                                    .child(
                                        h_flex()
                                            .flex_wrap()
                                            .items_center()
                                            .gap(Spacing::SM)
                                            .child(if let Some(cash) = &active_cash {
                                                StatusBadge::new(
                                                    StatusTone::Success,
                                                    format!("Caja abierta · Turno #{}", cash.id),
                                                )
                                            } else {
                                                StatusBadge::new(
                                                    StatusTone::Warning,
                                                    "Caja cerrada",
                                                )
                                            })
                                            .child(
                                                AppButton::new("cash-ticket-value-toggle")
                                                    .secondary()
                                                    .small()
                                                    .icon(IconName::Settings)
                                                    .label(format!(
                                                        "Ticket: {}",
                                                        format_soles(ticket_unit_cents)
                                                    ))
                                                    .on_click(cx.listener(|view, _, window, cx| {
                                                        if view.editing_ticket_value {
                                                            view.editing_ticket_value = false;
                                                            cx.notify();
                                                        } else {
                                                            view.open_ticket_editor(window, cx);
                                                        }
                                                    })),
                                            ),
                                    ),
                            )
                            .when(self.editing_ticket_value, |this| {
                                this.child(
                                    h_flex()
                                        .w_full()
                                        .flex_wrap()
                                        .items_end()
                                        .gap(Spacing::SM)
                                        .p(Spacing::SM)
                                        .rounded(Radii::MD)
                                        .bg(palette.surface_subtle)
                                        .border_1()
                                        .border_color(palette.border)
                                        .child(
                                            v_flex()
                                                .w(px(210.0))
                                                .gap(Spacing::XXS)
                                                .child(
                                                    Label::new("Valor del ticket físico (S/)")
                                                        .text_size(Typography::XS)
                                                        .font_semibold()
                                                        .text_color(palette.muted_foreground),
                                                )
                                                .child(
                                                    Input::new(&self.ticket_value_input)
                                                        .small()
                                                        .rounded(Radii::MD)
                                                        .bg(palette.surface),
                                                ),
                                        )
                                        .child(
                                            AppButton::new("cash-ticket-value-save")
                                                .primary()
                                                .small()
                                                .icon(IconName::Check)
                                                .label("Guardar valor")
                                                .on_click(cx.listener(|view, _, _, cx| {
                                                    view.save_ticket_value(cx);
                                                })),
                                        )
                                        .child(
                                            AppButton::new("cash-ticket-value-cancel")
                                                .tertiary()
                                                .small()
                                                .label("Cancelar")
                                                .on_click(cx.listener(|view, _, _, cx| {
                                                    view.editing_ticket_value = false;
                                                    cx.notify();
                                                })),
                                        )
                                        .child(
                                            Label::new(
                                                "Predeterminado: S/ 7.00. Aplica a las próximas atenciones.",
                                            )
                                            .text_size(Typography::XS)
                                            .text_color(palette.muted_foreground),
                                        ),
                                )
                            }),
                    )
                    .child(
                        h_flex()
                            .w_full()
                            .flex_wrap()
                            .gap(Spacing::MD)
                            .child(
                                MetricCard::new(
                                    MetricAccent::Neutral,
                                    "Apertura de caja",
                                    opening_value,
                                )
                                .icon(IconName::Inbox)
                                .subtitle(opening_subtitle),
                            )
                            .child(
                                MetricCard::new(
                                    MetricAccent::CashIn,
                                    "Efectivo ingresado",
                                    format_soles(cash_inflow_cents),
                                )
                                .icon(IconName::CircleCheck)
                                .subtitle("Ingresos en efectivo"),
                            )
                            .child(
                                MetricCard::new(
                                    MetricAccent::TicketsReceived,
                                    "Tickets recibidos",
                                    format!("{tickets_count} und."),
                                )
                                .icon(IconName::Info)
                                .subtitle(format!(
                                    "Equivalente: {}",
                                    format_soles(tickets_amount_cents)
                                )),
                            )
                            .child(
                                MetricCard::new(
                                    MetricAccent::Breakfasts,
                                    "Saldo esperado en caja",
                                    format_soles(expected_cash_cents),
                                )
                                .icon(IconName::Check)
                                .subtitle("Apertura + efectivo ingresado"),
                            )
                            .child(
                                MetricCard::new(
                                    MetricAccent::CreditBalance,
                                    "Saldo a favor",
                                    format_soles(total_credit),
                                )
                                .icon(IconName::Check)
                                .subtitle("Excedentes de pensionistas"),
                            )
                            .child(
                                MetricCard::new(
                                    MetricAccent::PendingDebt,
                                    "Deuda pendiente",
                                    format_soles(total_debt),
                                )
                                .icon(IconName::TriangleAlert)
                                .subtitle("Por cobrar a pensionistas"),
                            ),
                    )
                    .child(match active_cash {
                        None => v_flex()
                            .w_full()
                            .gap(Spacing::MD)
                            .p(card_padding)
                            .rounded(Radii::LG)
                            .bg(palette.surface)
                            .border_1()
                            .border_color(palette.border)
                            .child(
                                v_flex()
                                    .gap(Spacing::XXS)
                                    .child(
                                        Label::new("Apertura de caja")
                                            .text_size(Typography::LG)
                                            .font_semibold()
                                            .text_color(palette.foreground),
                                    )
                                    .child(
                                        Label::new(
                                            "Mientras la caja permanezca cerrada, Atención automática bloqueará la entrega de tickets físicos sin alterar los saldos.",
                                        )
                                        .text_size(Typography::SM)
                                        .text_color(palette.muted_foreground),
                                    ),
                            )
                            .child(
                                h_flex()
                                    .w_full()
                                    .flex_wrap()
                                    .items_end()
                                    .gap(Spacing::MD)
                                    .child(
                                        v_flex()
                                            .w(px(240.0))
                                            .max_w_full()
                                            .gap(Spacing::XS)
                                            .child(
                                                Label::new("Monto inicial de apertura (S/)")
                                                    .text_size(Typography::XS)
                                                    .font_semibold()
                                                    .text_color(palette.muted_foreground),
                                            )
                                            .child(
                                                Input::new(&self.opening_cash_input)
                                                    .rounded(Radii::MD)
                                                    .bg(palette.surface),
                                            ),
                                    )
                                    .child(
                                        AppButton::new("cash-open-submit")
                                            .primary()
                                            .icon(IconName::Check)
                                            .label("Abrir caja")
                                            .on_click(cx.listener(|view, _, window, cx| {
                                                view.handle_open_cash(window, cx);
                                            })),
                                    ),
                            )
                            .when_some(self.feedback.clone(), |this, (tone, msg)| {
                                this.child(StatusBadge::new(tone, msg))
                            })
                            .into_any_element(),
                        Some(cash) => v_flex()
                            .w_full()
                            .gap(Spacing::MD)
                            .p(card_padding)
                            .rounded(Radii::LG)
                            .bg(palette.surface)
                            .border_1()
                            .border_color(palette.border)
                            .child(
                                h_flex()
                                    .w_full()
                                    .flex_wrap()
                                    .items_center()
                                    .justify_between()
                                    .gap(Spacing::SM)
                                    .child(
                                        v_flex()
                                            .gap(Spacing::XXS)
                                            .child(
                                                Label::new(format!(
                                                    "Arqueo y cierre · Turno #{}",
                                                    cash.id
                                                ))
                                                .text_size(Typography::LG)
                                                .font_semibold()
                                                .text_color(palette.foreground),
                                            )
                                            .child(
                                                Label::new(format!(
                                                    "Abierto por {} ({}) · Esperado: {} tickets ({}) y {} en efectivo",
                                                    cash.opened_by,
                                                    cash.opened_at,
                                                    cash.total_tickets(),
                                                    format_soles(cash.total_ticket_amount_cents()),
                                                    format_soles(cash.expected_cash_cents()),
                                                ))
                                                .text_size(Typography::XS)
                                                .text_color(palette.muted_foreground),
                                            ),
                                    )
                                    .child(
                                        AppButton::new("cash-prefill-expected")
                                            .tertiary()
                                            .small()
                                            .label("Igualar al esperado")
                                            .on_click(cx.listener(|view, _, window, cx| {
                                                view.prefill_expected(window, cx);
                                            })),
                                    ),
                            )
                            .child(
                                h_flex()
                                    .w_full()
                                    .flex_wrap()
                                    .gap(Spacing::MD)
                                    .child(
                                        v_flex()
                                            .w(px(180.0))
                                            .max_w_full()
                                            .gap(Spacing::XS)
                                            .child(
                                                Label::new("Tickets contados")
                                                    .text_size(Typography::XS)
                                                    .font_semibold()
                                                    .text_color(palette.muted_foreground),
                                            )
                                            .child(
                                                Input::new(&self.counted_tickets_input)
                                                    .rounded(Radii::MD)
                                                    .bg(palette.surface),
                                            ),
                                    )
                                    .child(
                                        v_flex()
                                            .w(px(200.0))
                                            .max_w_full()
                                            .gap(Spacing::XS)
                                            .child(
                                                Label::new("Efectivo contado (S/)")
                                                    .text_size(Typography::XS)
                                                    .font_semibold()
                                                    .text_color(palette.muted_foreground),
                                            )
                                            .child(
                                                Input::new(&self.counted_cash_input)
                                                    .rounded(Radii::MD)
                                                    .bg(palette.surface),
                                            ),
                                    )
                                    .child(
                                        v_flex()
                                            .flex_1()
                                            .min_w(px(220.0))
                                            .gap(Spacing::XS)
                                            .child(
                                                Label::new("Observaciones")
                                                    .text_size(Typography::XS)
                                                    .font_semibold()
                                                    .text_color(palette.muted_foreground),
                                            )
                                            .child(
                                                Input::new(&self.close_notes_input)
                                                    .rounded(Radii::MD)
                                                    .bg(palette.surface),
                                            ),
                                    ),
                            )
                            .when_some(cash.last_audit.clone(), |this, audit| {
                                let tone = if audit.is_balanced() {
                                    StatusTone::Success
                                } else {
                                    StatusTone::Warning
                                };
                                this.child(
                                    h_flex()
                                        .w_full()
                                        .flex_wrap()
                                        .items_center()
                                        .justify_between()
                                        .gap(Spacing::SM)
                                        .p(Spacing::SM)
                                        .rounded(Radii::MD)
                                        .bg(palette.surface_subtle)
                                        .child(StatusBadge::new(
                                            tone,
                                            if audit.is_balanced() {
                                                "Último arqueo: cuadrado"
                                            } else {
                                                "Último arqueo: con diferencia"
                                            },
                                        ))
                                        .child(
                                            Label::new(format!(
                                                "{} · Tickets {}/{} ({:+}) · Efectivo {}/{} (dif. {})",
                                                audit.audited_at,
                                                audit.counted_tickets,
                                                audit.expected_tickets,
                                                audit.ticket_diff,
                                                format_soles(audit.counted_cash_cents),
                                                format_soles(audit.expected_cash_cents),
                                                format_soles(audit.cash_diff_cents),
                                            ))
                                            .text_size(Typography::XS)
                                            .text_color(palette.muted_foreground),
                                        ),
                                )
                            })
                            .when_some(self.feedback.clone(), |this, (tone, msg)| {
                                this.child(StatusBadge::new(tone, msg))
                            })
                            .child(
                                h_flex()
                                    .w_full()
                                    .flex_wrap()
                                    .items_center()
                                    .justify_between()
                                    .gap(Spacing::SM)
                                    .child(
                                        AppButton::new("cash-audit-submit")
                                            .secondary()
                                            .icon(IconName::Check)
                                            .label("Registrar arqueo")
                                            .on_click(cx.listener(|view, _, _, cx| {
                                                view.handle_audit_cash(cx);
                                            })),
                                    )
                                    .child(
                                        AppButton::new("cash-close-submit")
                                            .destructive()
                                            .icon(IconName::Close)
                                            .label("Cerrar caja y archivar turno")
                                            .on_click(cx.listener(|view, _, window, cx| {
                                                view.handle_close_cash(window, cx);
                                            })),
                                    ),
                            )
                            .into_any_element(),
                    })
                    .when_some(selected_person, |this, (person, totals)| {
                        let pid = person.id;
                        let badge = if person.credit_cents() > 0 {
                            StatusBadge::new(
                                StatusTone::Success,
                                format!("A favor {}", format_soles(person.credit_cents())),
                            )
                        } else if person.debt_cents() > 0 {
                            StatusBadge::new(
                                StatusTone::Warning,
                                format!("Deuda {}", format_soles(person.debt_cents())),
                            )
                        } else {
                            StatusBadge::new(StatusTone::Info, "Al día")
                        };
                        this.child(
                            h_flex()
                                .w_full()
                                .flex_wrap()
                                .items_center()
                                .justify_between()
                                .gap(Spacing::SM)
                                .p(card_padding)
                                .rounded(Radii::LG)
                                .bg(palette.surface)
                                .border_1()
                                .border_color(palette.brand_accent)
                                .child(
                                    v_flex()
                                        .min_w_0()
                                        .gap(Spacing::XXS)
                                        .child(
                                            h_flex()
                                                .flex_wrap()
                                                .items_center()
                                                .gap(Spacing::SM)
                                                .child(
                                                    Label::new(format!(
                                                        "Movimientos de {} ({})",
                                                        person.full_name,
                                                        person.code()
                                                    ))
                                                    .text_size(Typography::BASE)
                                                    .font_bold()
                                                    .text_color(palette.foreground),
                                                )
                                                .child(badge),
                                        )
                                        .child(
                                            Label::new(format!(
                                                "Consumido: {} · Abonado total: {} · Tickets entregados: {} und. ({})",
                                                format_soles(totals.consumed_cents),
                                                format_soles(totals.total_paid_cents()),
                                                totals.tickets_count,
                                                format_soles(totals.tickets_cents),
                                            ))
                                            .text_size(Typography::XS)
                                            .text_color(palette.muted_foreground),
                                        ),
                                )
                                .child(
                                    h_flex()
                                        .gap(Spacing::XS)
                                        .child(
                                            AppButton::new("cash-open-pensioner-sheet")
                                                .secondary()
                                                .xsmall()
                                                .icon(IconName::User)
                                                .label("Ver ficha")
                                                .on_click(cx.listener(move |_, _, _, cx| {
                                                    cx.emit(CashEvent::OpenPensioner(pid));
                                                })),
                                        )
                                        .child(
                                            AppButton::new("cash-clear-pensioner-filter")
                                                .tertiary()
                                                .xsmall()
                                                .label("Ver todos")
                                                .on_click(cx.listener(|view, _, _, cx| {
                                                    view.selected_pensioner = None;
                                                    cx.notify();
                                                })),
                                        ),
                                ),
                        )
                    })
                    .child(
                        v_flex()
                            .w_full()
                            .gap(Spacing::MD)
                            .p(card_padding)
                            .rounded(Radii::LG)
                            .bg(palette.surface)
                            .border_1()
                            .border_color(palette.border)
                            .child(
                                h_flex()
                                    .w_full()
                                    .flex_wrap()
                                    .items_center()
                                    .justify_between()
                                    .gap(Spacing::SM)
                                    .child(
                                        Label::new("Historial y auditoría de movimientos de caja")
                                            .text_size(Typography::LG)
                                            .font_semibold()
                                            .text_color(palette.foreground),
                                    )
                                    .child(
                                        Label::new(format!(
                                            "{} movimientos",
                                            filtered_movements.len()
                                        ))
                                        .text_size(Typography::XS)
                                        .text_color(palette.muted_foreground),
                                    ),
                            )
                            .child(
                                h_flex()
                                    .w_full()
                                    .flex_wrap()
                                    .items_center()
                                    .justify_between()
                                    .gap(Spacing::SM)
                                    .child(
                                        div().w(px(340.0)).max_w_full().child(
                                            Input::new(&self.search_input)
                                                .small()
                                                .cleanable(true)
                                                .rounded(Radii::MD)
                                                .bg(palette.surface)
                                                .prefix(Icon::new(IconName::Search).xsmall()),
                                        ),
                                    )
                                    .child(
                                        h_flex()
                                            .flex_wrap()
                                            .gap(Spacing::XS)
                                            .children(CashFilter::ALL.into_iter().map(|choice| {
                                                let active = self.filter == choice;
                                                let btn = AppButton::new(SharedString::from(
                                                    format!("cash-filter-{choice:?}"),
                                                ))
                                                .xsmall()
                                                .label(choice.label())
                                                .on_click(cx.listener(move |view, _, _, cx| {
                                                    view.filter = choice;
                                                    cx.notify();
                                                }));
                                                if active {
                                                    btn.primary()
                                                } else {
                                                    btn.secondary()
                                                }
                                            })),
                                    ),
                            )
                            .child(if filtered_movements.is_empty() {
                                v_flex()
                                    .w_full()
                                    .items_center()
                                    .gap(Spacing::XS)
                                    .py(Spacing::LG)
                                    .px(Spacing::MD)
                                    .rounded(Radii::MD)
                                    .bg(palette.surface_subtle)
                                    .child(StatusBadge::new(
                                        StatusTone::Neutral,
                                        "Sin movimientos en esta vista",
                                    ))
                                    .child(
                                        Label::new(
                                            "Abre caja o confirma una atención con tickets desde Atención automática para ver los movimientos y sus comprobantes.",
                                        )
                                        .text_size(Typography::SM)
                                        .text_center()
                                        .text_color(palette.muted_foreground),
                                    )
                                    .into_any_element()
                            } else {
                                v_flex()
                                    .w_full()
                                    .gap(Spacing::SM)
                                    .children(filtered_movements.into_iter().map(|mov| {
                                        let mov_id_for_receipt = mov.id.clone();
                                        let amount_color = match mov.kind {
                                            CashMovementKind::CashPayment
                                            | CashMovementKind::TicketCredit => {
                                                palette.success.foreground
                                            }
                                            CashMovementKind::Opening
                                            | CashMovementKind::Closing => palette.foreground,
                                        };
                                        h_flex()
                                            .w_full()
                                            .flex_wrap()
                                            .items_center()
                                            .justify_between()
                                            .gap(Spacing::SM)
                                            .p(Spacing::MD)
                                            .rounded(Radii::MD)
                                            .bg(palette.surface_subtle)
                                            .border_1()
                                            .border_color(palette.border)
                                            .child(
                                                v_flex()
                                                    .min_w_0()
                                                    .gap(Spacing::XXS)
                                                    .child(
                                                        h_flex()
                                                            .flex_wrap()
                                                            .items_center()
                                                            .gap(Spacing::XS)
                                                            .child(
                                                                Label::new(mov.id.clone())
                                                                    .text_size(Typography::XS)
                                                                    .font_bold()
                                                                    .text_color(
                                                                        palette.brand_accent,
                                                                    ),
                                                            )
                                                            .child(movement_badge(mov.kind))
                                                            .child(match (
                                                                mov.pensioner_id,
                                                                mov.pensioner_code.clone(),
                                                                mov.pensioner_name.clone(),
                                                            ) {
                                                                (
                                                                    Some(pid),
                                                                    Some(code),
                                                                    Some(name),
                                                                ) => AppButton::new(
                                                                    SharedString::from(format!(
                                                                        "cash-mov-person-{}",
                                                                        mov.id
                                                                    )),
                                                                )
                                                                .tertiary()
                                                                .xsmall()
                                                                .label(format!("{code} {name}"))
                                                                .tooltip(
                                                                    "Filtrar movimientos de este pensionista",
                                                                )
                                                                .on_click(cx.listener(
                                                                    move |view, _, _, cx| {
                                                                        view.selected_pensioner =
                                                                            Some(pid);
                                                                        view.filter =
                                                                            CashFilter::All;
                                                                        cx.notify();
                                                                    },
                                                                ))
                                                                .into_any_element(),
                                                                _ => Label::new("Caja general")
                                                                    .text_size(Typography::XS)
                                                                    .text_color(
                                                                        palette.muted_foreground,
                                                                    )
                                                                    .into_any_element(),
                                                            }),
                                                    )
                                                    .child(
                                                        Label::new(mov.description.clone())
                                                            .text_size(Typography::SM)
                                                            .text_color(palette.foreground),
                                                    )
                                                    .child(
                                                        Label::new(format!(
                                                            "Turno #{} · {} · {}",
                                                            mov.cash_session_id,
                                                            mov.operator_name,
                                                            mov.recorded_at
                                                        ))
                                                        .text_size(Typography::XS)
                                                        .text_color(palette.muted_foreground),
                                                    ),
                                            )
                                            .child(
                                                h_flex()
                                                    .items_center()
                                                    .gap(Spacing::SM)
                                                    .child(
                                                        Label::new(format_soles(mov.amount_cents))
                                                            .text_size(Typography::BASE)
                                                            .font_bold()
                                                            .text_color(amount_color),
                                                    )
                                                    .child(
                                                        AppButton::new(SharedString::from(format!(
                                                            "cash-receipt-btn-{}",
                                                            mov.id
                                                        )))
                                                        .secondary()
                                                        .xsmall()
                                                        .icon(IconName::FileText)
                                                        .label("Recibo")
                                                        .tooltip("Ver comprobante")
                                                        .on_click(cx.listener(
                                                            move |view, _, window, cx| {
                                                                view.open_receipt(
                                                                    mov_id_for_receipt.clone(),
                                                                    window,
                                                                    cx,
                                                                );
                                                            },
                                                        )),
                                                    ),
                                            )
                                    }))
                                    .into_any_element()
                            }),
                    )
                    .child(
                        v_flex()
                            .w_full()
                            .gap(Spacing::MD)
                            .p(card_padding)
                            .rounded(Radii::LG)
                            .bg(palette.surface)
                            .border_1()
                            .border_color(palette.border)
                            .child(
                                Label::new("Historial de turnos cerrados")
                                    .text_size(Typography::LG)
                                    .font_semibold()
                                    .text_color(palette.foreground),
                            )
                            .child(if closed_sessions.is_empty() {
                                Label::new("Aún no se ha cerrado ningún turno de caja.")
                                    .text_size(Typography::SM)
                                    .text_color(palette.muted_foreground)
                                    .into_any_element()
                            } else {
                                v_flex()
                                    .w_full()
                                    .gap(Spacing::SM)
                                    .children(closed_sessions.into_iter().map(|closed| {
                                        let tone = if closed.audit.is_balanced() {
                                            StatusTone::Success
                                        } else {
                                            StatusTone::Warning
                                        };
                                        v_flex()
                                            .w_full()
                                            .gap(Spacing::XS)
                                            .p(Spacing::MD)
                                            .rounded(Radii::MD)
                                            .bg(palette.surface_subtle)
                                            .border_1()
                                            .border_color(palette.border)
                                            .child(
                                                h_flex()
                                                    .w_full()
                                                    .flex_wrap()
                                                    .items_center()
                                                    .justify_between()
                                                    .gap(Spacing::SM)
                                                    .child(
                                                        h_flex()
                                                            .items_center()
                                                            .gap(Spacing::SM)
                                                            .child(
                                                                Label::new(format!(
                                                                    "Turno #{}",
                                                                    closed.id
                                                                ))
                                                                .text_size(Typography::BASE)
                                                                .font_semibold()
                                                                .text_color(palette.foreground),
                                                            )
                                                            .child(StatusBadge::new(
                                                                tone,
                                                                if closed.audit.is_balanced() {
                                                                    "Cuadrado"
                                                                } else {
                                                                    "Con diferencia"
                                                                },
                                                            )),
                                                    )
                                                    .child(
                                                        Label::new(format!(
                                                            "{} → {}",
                                                            closed.opened_at, closed.closed_at
                                                        ))
                                                        .text_size(Typography::XS)
                                                        .text_color(palette.muted_foreground),
                                                    ),
                                            )
                                            .child(
                                                Label::new(format!(
                                                    "Movimientos: {} · Tickets {}/{} ({}) · Efectivo {}/{}",
                                                    closed.movement_count,
                                                    closed.audit.counted_tickets,
                                                    closed.expected_tickets,
                                                    format_soles(
                                                        closed.expected_ticket_amount_cents
                                                    ),
                                                    format_soles(closed.audit.counted_cash_cents),
                                                    format_soles(closed.expected_cash_cents),
                                                ))
                                                .text_size(Typography::SM)
                                                .text_color(palette.foreground),
                                            )
                                            .when(!closed.notes.is_empty(), |this| {
                                                this.child(
                                                    Label::new(format!(
                                                        "Nota ({}): {}",
                                                        closed.closed_by, closed.notes
                                                    ))
                                                    .text_size(Typography::XS)
                                                    .text_color(palette.muted_foreground),
                                                )
                                            })
                                    }))
                                    .into_any_element()
                            }),
                    ),
            )
            .when_some(receipt_modal_data, |this, (mov, current_bal)| {
                this.child(self.render_receipt_modal(&mov, current_bal, window, cx))
            })
    }
}
