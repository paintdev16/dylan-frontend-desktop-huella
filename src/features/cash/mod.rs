mod view;

use gpui_kit::{
    component::{
        IndexPath,
        input::{InputEvent, InputState},
        select::{SelectEvent, SelectState},
    },
    *,
};

use crate::{
    components::status_badge::StatusTone,
    features::pensioners::model::{PensionerId, format_soles},
    services::api::{ApiError, dining::DiningApi},
    state::{
        session::SessionState,
        store::{CashMovementKind, ComedorStore, parse_soles_to_cents},
    },
};

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
    pending: bool,
    store: Entity<ComedorStore>,
    session: Entity<SessionState>,
    opening_cash_input: Entity<InputState>,
    counted_tickets_input: Entity<InputState>,
    counted_cash_input: Entity<InputState>,
    ticket_value_input: Entity<InputState>,
    search_input: Entity<InputState>,
    filter: Entity<SelectState<Vec<CashFilter>>>,
    show_closed_sessions: bool,
    selected_pensioner: Option<PensionerId>,
    editing_ticket_value: bool,
    receipt_movement_id: Option<String>,
    receipt_focus: FocusHandle,
    feedback: Option<(StatusTone, SharedString)>,
    _subscriptions: Vec<Subscription>,
}

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
        let counted_tickets_input =
            cx.new(|cx| InputState::new(window, cx).placeholder("Total registrado"));
        let counted_cash_input = cx.new(|cx| InputState::new(window, cx).placeholder("0.00"));
        let ticket_value_input = cx.new(|cx| {
            InputState::new(window, cx)
                .placeholder("7.00")
                .default_value(cents_to_plain_soles(current_ticket))
        });
        let search_input = cx.new(|cx| {
            InputState::new(window, cx).placeholder("Código, pensionista, concepto o responsable")
        });
        let filter = cx.new(|cx| {
            SelectState::new(
                CashFilter::ALL.to_vec(),
                Some(IndexPath::default()),
                window,
                cx,
            )
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
        let sub_filter = cx.subscribe(&filter, |_, _, event, cx| {
            if matches!(event, SelectEvent::Confirm(_)) {
                cx.notify();
            }
        });

        Self {
            pending: false,
            store,
            session,
            opening_cash_input,
            counted_tickets_input,
            counted_cash_input,
            ticket_value_input,
            search_input,
            filter,
            show_closed_sessions: false,
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
                sub_filter,
            ],
        }
    }

    fn can_manage_cash(&self, cx: &App) -> bool {
        self.session.read(cx).can_operate_cash()
    }

    fn open_ticket_editor(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if !self.session.read(cx).can_change_ticket_value() {
            return;
        }
        let current = self.store.read(cx).ledger().ticket_value_cents();
        self.ticket_value_input.update(cx, |input, cx| {
            input.set_value(cents_to_plain_soles(current), window, cx);
            input.focus(window, cx);
        });
        self.editing_ticket_value = true;
        self.feedback = None;
        cx.notify();
    }

    fn operation<T: Send + 'static>(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
        request: impl FnOnce(&DiningApi) -> Result<T, ApiError> + Send + 'static,
        success: impl FnOnce(&mut Self, T, &mut Window, &mut Context<Self>) + 'static,
    ) {
        if self.pending || !self.can_manage_cash(cx) {
            return;
        }
        self.pending = true;
        self.feedback = None;
        cx.notify();
        let target = cx.weak_entity();
        let window = window.window_handle();
        self.store.update(cx, |store, cx| {
            store.request(
                request,
                move |store, result, cx| {
                    if result.is_ok() {
                        store.cash_changed(cx);
                    }
                    let _ = window.update(cx, |_, window, cx| {
                        let _ = target.update(cx, |view, cx| {
                            view.pending = false;
                            match result {
                                Ok(result) => success(view, result, window, cx),
                                Err(error) => {
                                    view.feedback =
                                        Some((StatusTone::Error, error.user_message().into()))
                                }
                            }
                            cx.notify();
                        });
                    });
                },
                cx,
            )
        });
    }

    fn save_ticket_value(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if !self.session.read(cx).can_change_ticket_value() {
            return;
        }
        let raw = self.ticket_value_input.read(cx).value();
        let Some(cents) = parse_soles_to_cents(raw.as_ref()).filter(|value| *value > 0) else {
            self.feedback = Some((
                StatusTone::Error,
                "Ingresa un valor de ticket válido mayor a S/ 0.00 (ej. 7.00).".into(),
            ));
            cx.notify();
            return;
        };
        self.operation(
            window,
            cx,
            move |api| api.set_ticket_value(cents),
            move |view, (), _, _| {
                view.editing_ticket_value = false;
                view.feedback = Some((
                    StatusTone::Success,
                    format!(
                        "Valor del ticket físico actualizado a {}.",
                        format_soles(cents)
                    )
                    .into(),
                ));
            },
        );
    }

    fn handle_open_cash(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let raw = self.opening_cash_input.read(cx).value();
        let Some(opening_cents) = parse_soles_to_cents(raw.as_ref()) else {
            self.feedback = Some((
                StatusTone::Error,
                "Ingresa un monto de apertura válido (ej. 0.00 o 100.00).".into(),
            ));
            cx.notify();
            return;
        };
        self.operation(
            window,
            cx,
            move |api| api.open_cash(opening_cents),
            move |view, (), window, cx| {
                view.counted_tickets_input
                    .update(cx, |input, cx| input.clean(window, cx));
                view.counted_cash_input.update(cx, |input, cx| {
                    input.set_value(cents_to_plain_soles(opening_cents), window, cx)
                });
                view.feedback = Some((
                    StatusTone::Success,
                    format!(
                        "Caja abierta con monto inicial de {}.",
                        format_soles(opening_cents)
                    )
                    .into(),
                ));
            },
        );
    }

    fn prefill_expected(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if let Some(cash_cents) = self
            .store
            .read(cx)
            .ledger()
            .active_cash()
            .map(|cash| cash.expected_cash_cents())
        {
            self.counted_tickets_input
                .update(cx, |input, cx| input.clean(window, cx));
            self.counted_cash_input.update(cx, |input, cx| {
                input.set_value(cents_to_plain_soles(cash_cents), window, cx)
            });
            self.feedback = None;
            cx.notify();
        }
    }

    fn parse_audit_inputs(&self, cx: &App) -> Result<(Option<u32>, i64), &'static str> {
        let raw_tickets = self.counted_tickets_input.read(cx).value();
        let counted_tickets = parse_counted_tickets(&raw_tickets)?;
        let raw_cash = self.counted_cash_input.read(cx).value();
        let counted_cash = parse_soles_to_cents(raw_cash.as_ref())
            .ok_or("Ingresa un monto válido de efectivo contado (ej. 100.00).")?;
        Ok((counted_tickets, counted_cash))
    }

    fn handle_audit_cash(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let (tickets, cash) = match self.parse_audit_inputs(cx) {
            Ok(values) => values,
            Err(msg) => {
                self.feedback = Some((StatusTone::Error, msg.into()));
                cx.notify();
                return;
            }
        };
        self.operation(
            window,
            cx,
            move |api| api.audit_cash(tickets, cash),
            |view, audit, _, _| {
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
                view.feedback = Some((tone, message.into()));
            },
        );
    }

    fn handle_close_cash(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let (tickets, cash) = match self.parse_audit_inputs(cx) {
            Ok(values) => values,
            Err(msg) => {
                self.feedback = Some((StatusTone::Error, msg.into()));
                cx.notify();
                return;
            }
        };
        self.operation(
            window,
            cx,
            move |api| api.close_cash(tickets, cash),
            |view, (), window, cx| {
                view.opening_cash_input
                    .update(cx, |input, cx| input.set_value("100.00", window, cx));
                view.counted_tickets_input
                    .update(cx, |input, cx| input.clean(window, cx));
                view.counted_cash_input
                    .update(cx, |input, cx| input.clean(window, cx));
                view.feedback = Some((
                    StatusTone::Success,
                    "Turno de caja cerrado y archivado en el historial.".into(),
                ));
            },
        );
    }

    fn open_receipt(&mut self, mov_id: String, window: &mut Window, cx: &mut Context<Self>) {
        self.store.update(cx, |store, cx| {
            if let Some(id) = store
                .ledger()
                .find_cash_movement(&mov_id)
                .and_then(|movement| movement.pensioner_id)
            {
                store.load_pensioner_history(id, false, cx);
            }
        });
        self.receipt_movement_id = Some(mov_id);
        window.focus(&self.receipt_focus, cx);
        cx.notify();
    }

    fn close_receipt(&mut self, cx: &mut Context<Self>) {
        if self.receipt_movement_id.take().is_some() {
            cx.notify();
        }
    }

    pub fn reset_session(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.pending = false;
        self.feedback = None;
        self.editing_ticket_value = false;
        self.receipt_movement_id = None;
        self.selected_pensioner = None;
        self.show_closed_sessions = false;
        self.opening_cash_input
            .update(cx, |input, cx| input.set_value("100.00", window, cx));
        for input in [
            &self.counted_tickets_input,
            &self.counted_cash_input,
            &self.search_input,
        ] {
            input.update(cx, |input, cx| input.clean(window, cx));
        }
        cx.notify();
    }
}

fn parse_counted_tickets(input: &str) -> Result<Option<u32>, &'static str> {
    let input = input.trim();
    if input.is_empty() {
        return Ok(None);
    }
    input
        .parse()
        .map(Some)
        .map_err(|_| "Ingresa una cantidad entera válida de tickets contados.")
}

fn cents_to_plain_soles(cents: i64) -> String {
    let abs = cents.unsigned_abs();
    format!("{}.{:02}", abs / 100, abs % 100)
}

#[cfg(test)]
mod tests {
    use super::parse_counted_tickets;

    #[test]
    fn an_empty_ticket_count_uses_the_current_registered_total() {
        assert_eq!(parse_counted_tickets(""), Ok(None));
        assert_eq!(parse_counted_tickets("  "), Ok(None));
    }

    #[test]
    fn a_manual_ticket_count_overrides_the_registered_total() {
        assert_eq!(parse_counted_tickets(" 18 "), Ok(Some(18)));
        assert_eq!(parse_counted_tickets("0"), Ok(Some(0)));
        assert!(parse_counted_tickets("-1").is_err());
        assert!(parse_counted_tickets("1.5").is_err());
    }
}
