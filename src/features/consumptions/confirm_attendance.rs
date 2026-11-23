use crate::{
    features::pensioners::{SelectionSource, model::PensionerId},
    state::store::{AttendanceRecord, DemoLedger},
};

use super::order::{ConsumptionItem, Meal};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AttendanceRequest {
    pub idempotency_key: String,
    pub pensioner_id: PensionerId,
    pub items: Vec<ConsumptionItem>,
    pub picked_up_by: Option<String>,
    pub tickets_count: u32,
    pub source: SelectionSource,
    pub operator_name: String,
    pub can_register_attendance: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AttendanceSummary {
    pub breakfasts: u32,
    pub lunches: u32,
    pub consumption_cents: i64,
    pub ticket_unit_cents: i64,
    pub ticket_amount_cents: i64,
    pub projected_balance_cents: i64,
}

impl AttendanceSummary {
    pub fn calculate(
        balance: i64,
        items: &[ConsumptionItem],
        tickets: u32,
        ticket_unit_cents: i64,
    ) -> Result<Self, AttendanceError> {
        let mut breakfasts = 0u32;
        let mut lunches = 0u32;
        let mut consumption_cents = 0i64;
        for item in items {
            let unit = item.menu.unit_cents(item.meal);
            if item.portions == 0 || unit <= 0 {
                return Err(AttendanceError::InvalidItem);
            }
            let portions = match item.meal {
                Meal::Breakfast => &mut breakfasts,
                Meal::Lunch => &mut lunches,
            };
            *portions = portions
                .checked_add(item.portions)
                .ok_or(AttendanceError::AmountOverflow)?;
            let subtotal = i64::from(item.portions)
                .checked_mul(unit)
                .ok_or(AttendanceError::AmountOverflow)?;
            consumption_cents = consumption_cents
                .checked_add(subtotal)
                .ok_or(AttendanceError::AmountOverflow)?;
        }
        breakfasts
            .checked_add(lunches)
            .ok_or(AttendanceError::AmountOverflow)?;

        if tickets > 0 && ticket_unit_cents <= 0 {
            return Err(AttendanceError::InvalidTicketValue);
        }
        let ticket_amount_cents = i64::from(tickets)
            .checked_mul(ticket_unit_cents)
            .ok_or(AttendanceError::AmountOverflow)?;
        let delta = ticket_amount_cents
            .checked_sub(consumption_cents)
            .ok_or(AttendanceError::AmountOverflow)?;
        let projected_balance_cents = balance
            .checked_add(delta)
            .ok_or(AttendanceError::AmountOverflow)?;

        Ok(Self {
            breakfasts,
            lunches,
            consumption_cents,
            ticket_unit_cents,
            ticket_amount_cents,
            projected_balance_cents,
        })
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AttendanceError {
    Unauthorized,
    PensionerNotFound(PensionerId),
    InactivePensioner,
    EmptyOperation,
    CashClosed,
    DuplicateConfirmation,
    InvalidIdempotencyKey,
    InvalidItem,
    InvalidPickup,
    InvalidTicketValue,
    AmountOverflow,
}

impl AttendanceError {
    pub const fn message(self) -> &'static str {
        match self {
            Self::Unauthorized => "Tu sesión no tiene permiso para registrar atenciones.",
            Self::PensionerNotFound(_) => "No se encontró el pensionista.",
            Self::InactivePensioner => "El pensionista está inactivo.",
            Self::EmptyOperation => "Agrega al menos un consumo o un ticket físico.",
            Self::CashClosed => "Abre la caja para recibir tickets o deja la cantidad en cero.",
            Self::DuplicateConfirmation => "Esta atención ya fue confirmada.",
            Self::InvalidIdempotencyKey => "No se pudo validar la clave de confirmación.",
            Self::InvalidItem => "Cada consumo debe tener raciones y un precio mayor a cero.",
            Self::InvalidPickup => "Ingresa el nombre de quien recoge el consumo.",
            Self::InvalidTicketValue => "El valor configurado del ticket debe ser mayor a S/ 0.00.",
            Self::AmountOverflow => "El importe o la cantidad excede el límite permitido.",
        }
    }
}

pub fn confirm_attendance(
    ledger: &mut DemoLedger,
    mut request: AttendanceRequest,
) -> Result<AttendanceRecord, AttendanceError> {
    if !request.can_register_attendance {
        return Err(AttendanceError::Unauthorized);
    }

    request.idempotency_key = request.idempotency_key.trim().to_owned();
    if request.idempotency_key.is_empty() {
        return Err(AttendanceError::InvalidIdempotencyKey);
    }
    if ledger.has_processed_key(&request.idempotency_key) {
        return Err(AttendanceError::DuplicateConfirmation);
    }

    let pensioner = ledger
        .find_pensioner(request.pensioner_id)
        .ok_or(AttendanceError::PensionerNotFound(request.pensioner_id))?;
    if !pensioner.active {
        return Err(AttendanceError::InactivePensioner);
    }

    if request.items.is_empty() && request.tickets_count == 0 {
        return Err(AttendanceError::EmptyOperation);
    }
    if request.tickets_count > 0 && !ledger.is_cash_open() {
        return Err(AttendanceError::CashClosed);
    }

    if request.items.is_empty() {
        request.picked_up_by = None;
    } else if let Some(name) = &mut request.picked_up_by {
        *name = name.trim().to_owned();
        if name.chars().count() < 2 {
            return Err(AttendanceError::InvalidPickup);
        }
    }

    let ticket_unit_cents = ledger.ticket_value_cents();
    let summary = AttendanceSummary::calculate(
        pensioner.balance_cents,
        &request.items,
        request.tickets_count,
        ticket_unit_cents,
    )?;
    let limit = AttendanceError::AmountOverflow;
    ledger
        .total_breakfasts()
        .checked_add(ledger.total_lunches())
        .and_then(|total| total.checked_add(summary.breakfasts + summary.lunches))
        .ok_or(limit)?;
    ledger
        .total_tickets_received()
        .checked_add(request.tickets_count)
        .ok_or(limit)?;
    pensioner
        .tickets_balance
        .checked_add(request.tickets_count)
        .ok_or(limit)?;
    ledger
        .attendances()
        .iter()
        .try_fold(summary.consumption_cents, |total, record| {
            total.checked_add(record.consumption_cents).ok_or(limit)
        })?;
    if summary.projected_balance_cents < 0 {
        let debt = summary.projected_balance_cents.checked_abs().ok_or(limit)?;
        let remaining_debt = ledger.total_pending_debt_cents() - pensioner.debt_cents();
        remaining_debt.checked_add(debt).ok_or(limit)?;
    }
    Ok(ledger.commit_attendance_atomic(request, summary))
}

#[cfg(test)]
mod tests {
    use super::super::order::{ConsumptionItem, Meal, Menu};
    use super::{AttendanceError, AttendanceRequest, AttendanceSummary, confirm_attendance};
    use crate::{
        features::pensioners::{SelectionSource, model::PensionerId},
        state::store::{
            BREAKFAST_PRICE_CENTS, DEFAULT_TICKET_VALUE_CENTS, DemoLedger, LUNCH_PRICE_CENTS,
        },
    };

    fn base_request(key: &str, pensioner_id: PensionerId) -> AttendanceRequest {
        AttendanceRequest {
            idempotency_key: key.to_string(),
            pensioner_id,
            items: vec![],
            picked_up_by: None,
            tickets_count: 0,
            source: SelectionSource::GlobalSearch,
            operator_name: "Operador Demo".into(),
            can_register_attendance: true,
        }
    }

    #[test]
    fn mixed_cart_and_pickup_are_validated_and_saved_with_one_ticket_movement() {
        use super::super::order::add_to_cart;

        let mut ledger = DemoLedger::new();
        ledger.open_cash("Operador", 0).unwrap();
        let id = PensionerId(1001);
        let before = ledger.find_pensioner(id).unwrap().balance_cents;
        let mut request = base_request("mixed", id);
        for (meal, menu, portions) in [
            (Meal::Breakfast, Menu::Economic, 1),
            (Meal::Breakfast, Menu::Economic, 2),
            (Meal::Lunch, Menu::Special { unit_cents: 1350 }, 2),
            (Meal::Lunch, Menu::Special { unit_cents: 1800 }, 1),
        ] {
            add_to_cart(
                &mut request.items,
                ConsumptionItem {
                    meal,
                    menu,
                    portions,
                },
            )
            .unwrap();
        }
        request.tickets_count = 2;
        request.picked_up_by = Some(" ".into());
        assert_eq!(
            confirm_attendance(&mut ledger, request.clone()),
            Err(AttendanceError::InvalidPickup)
        );
        assert!(ledger.ticket_movements().is_empty());
        assert!(ledger.attendances().is_empty());

        request.picked_up_by = Some(" Ana Ramos ".into());
        let record = confirm_attendance(&mut ledger, request).unwrap();
        let total = 3 * BREAKFAST_PRICE_CENTS + 2 * 1350 + 1800;
        assert_eq!((record.breakfasts, record.lunches), (3, 3));
        assert_eq!(record.items.len(), 3);
        assert_eq!(record.items[0].portions, 3);
        assert_eq!(record.picked_up_by.as_deref(), Some("Ana Ramos"));
        assert_eq!(record.consumption_cents, total);
        assert_eq!(record.ticket_unit_cents, DEFAULT_TICKET_VALUE_CENTS);
        assert_eq!(
            record.balance_after_cents,
            before - total + 2 * DEFAULT_TICKET_VALUE_CENTS
        );
        assert_eq!(ledger.ticket_movements().len(), 1);
    }

    #[test]
    fn configurable_ticket_value_applies_to_new_ticket_credits_without_altering_cash_inflow() {
        let mut ledger = DemoLedger::new();
        ledger.open_cash("Operador", 10_000).unwrap();
        ledger.set_ticket_value_cents(850).unwrap();

        let id = PensionerId(1002);
        let before = ledger.find_pensioner(id).unwrap().balance_cents;
        let mut req = base_request("cfg-ticket", id);
        req.tickets_count = 2;

        let record = confirm_attendance(&mut ledger, req).unwrap();
        assert_eq!(record.ticket_unit_cents, 850);
        assert_eq!(record.ticket_amount_cents, 1_700);
        assert_eq!(record.balance_after_cents, before + 1_700);
        // Los tickets físicos no aumentan el efectivo físico esperado en caja.
        assert_eq!(ledger.active_cash_inflow_cents(), 0);
        assert_eq!(ledger.expected_cash_cents(), 10_000);
    }

    #[test]
    fn rejects_inactive_profiles_and_out_of_range_prices_without_writing() {
        let mut ledger = DemoLedger::new();
        let mut request = base_request("limit", PensionerId(1006));
        request.items.push(ConsumptionItem {
            meal: Meal::Lunch,
            menu: Menu::Special {
                unit_cents: i64::MAX,
            },
            portions: 2,
        });
        assert_eq!(
            confirm_attendance(&mut ledger, request.clone()),
            Err(AttendanceError::InactivePensioner)
        );
        request.pensioner_id = PensionerId(1001);
        assert_eq!(
            confirm_attendance(&mut ledger, request),
            Err(AttendanceError::AmountOverflow)
        );
        assert!(ledger.attendances().is_empty());
    }

    #[test]
    fn confirms_consumption_without_tickets_even_when_cash_is_closed() {
        let mut ledger = DemoLedger::new();
        assert!(!ledger.is_cash_open());

        let id = PensionerId(1001);
        let before = ledger.find_pensioner(id).unwrap().balance_cents;

        let mut req = base_request("att-1", id);
        req.items.push(ConsumptionItem {
            meal: Meal::Breakfast,
            menu: Menu::Economic,
            portions: 1,
        });
        req.items.push(ConsumptionItem {
            meal: Meal::Lunch,
            menu: Menu::Economic,
            portions: 1,
        });

        let record = confirm_attendance(&mut ledger, req).expect("consumption without tickets ok");
        let expected_cost = BREAKFAST_PRICE_CENTS + LUNCH_PRICE_CENTS;

        assert_eq!(record.consumption_cents, expected_cost);
        assert_eq!(record.tickets_count, 0);
        assert_eq!(record.ticket_amount_cents, 0);
        assert_eq!(record.balance_after_cents, before - expected_cost);
        assert_eq!(
            ledger.find_pensioner(id).unwrap().balance_cents,
            before - expected_cost
        );
        assert_eq!(ledger.attendances().len(), 1);
        assert!(ledger.ticket_movements().is_empty());
    }

    #[test]
    fn confirms_tickets_only_payment_when_cash_is_open() {
        let mut ledger = DemoLedger::new();
        ledger.open_cash("Operador Demo", 5_000).expect("open cash");

        let id = PensionerId(1002);
        let before = ledger.find_pensioner(id).unwrap().balance_cents;

        let mut req = base_request("att-2", id);
        req.tickets_count = 3;

        let record = confirm_attendance(&mut ledger, req).expect("tickets only ok");
        let expected_amount = 3 * DEFAULT_TICKET_VALUE_CENTS;

        assert_eq!(record.consumption_cents, 0);
        assert_eq!(record.tickets_count, 3);
        assert_eq!(record.ticket_amount_cents, expected_amount);
        assert_eq!(record.balance_after_cents, before + expected_amount);
        assert_eq!(
            ledger.find_pensioner(id).unwrap().balance_cents,
            before + expected_amount
        );
        assert_eq!(ledger.ticket_movements().len(), 1);
        assert_eq!(ledger.ticket_movements()[0].tickets_count, 3);
        assert_eq!(ledger.ticket_movements()[0].amount_cents, expected_amount);
        assert_eq!(ledger.active_cash().unwrap().total_tickets(), 3);
    }

    #[test]
    fn confirms_consumption_and_tickets_atomically_in_one_operation() {
        let mut ledger = DemoLedger::new();
        ledger.open_cash("Operador Demo", 0).expect("open cash");

        let id = PensionerId(1002);
        let before = ledger.find_pensioner(id).unwrap().balance_cents;

        let mut req = base_request("att-3", id);
        req.items.push(ConsumptionItem {
            meal: Meal::Breakfast,
            menu: Menu::Economic,
            portions: 1,
        });
        req.items.push(ConsumptionItem {
            meal: Meal::Lunch,
            menu: Menu::Economic,
            portions: 1,
        });
        req.tickets_count = 2;
        req.source = SelectionSource::BiometricMatch;

        let record = confirm_attendance(&mut ledger, req).expect("combined operation ok");
        let expected_summary =
            AttendanceSummary::calculate(before, &record.items, 2, DEFAULT_TICKET_VALUE_CENTS)
                .unwrap();

        assert_eq!(record.consumption_cents, expected_summary.consumption_cents);
        assert_eq!(record.tickets_count, 2);
        assert_eq!(
            record.ticket_amount_cents,
            expected_summary.ticket_amount_cents
        );
        assert_eq!(
            record.balance_after_cents,
            expected_summary.projected_balance_cents
        );
        assert_eq!(ledger.attendances().len(), 1);
        assert_eq!(ledger.ticket_movements().len(), 1);
        assert_eq!(ledger.ticket_movements()[0].attendance_id, record.id);
        assert_eq!(
            ledger.find_pensioner(id).unwrap().balance_cents,
            expected_summary.projected_balance_cents
        );
        assert_eq!(ledger.total_breakfasts(), 1);
        assert_eq!(ledger.total_lunches(), 1);
        assert_eq!(ledger.total_tickets_received(), 2);
    }

    #[test]
    fn rejects_entire_operation_when_tickets_requested_and_cash_is_closed() {
        let mut ledger = DemoLedger::new();
        assert!(!ledger.is_cash_open());

        let id = PensionerId(1001);
        let before_balance = ledger.find_pensioner(id).unwrap().balance_cents;
        let before_tickets = ledger.find_pensioner(id).unwrap().tickets_balance;

        let mut req = base_request("att-closed", id);
        req.items.push(ConsumptionItem {
            meal: Meal::Lunch,
            menu: Menu::Economic,
            portions: 2,
        });
        req.tickets_count = 1;

        assert_eq!(
            confirm_attendance(&mut ledger, req),
            Err(AttendanceError::CashClosed)
        );

        // Ningún registro, movimiento ni saldo debe cambiar.
        assert!(ledger.attendances().is_empty());
        assert!(ledger.ticket_movements().is_empty());
        assert_eq!(
            ledger.find_pensioner(id).unwrap().balance_cents,
            before_balance
        );
        assert_eq!(
            ledger.find_pensioner(id).unwrap().tickets_balance,
            before_tickets
        );
        // La clave rechazada no debe quedar marcada como procesada: al abrir caja puede reintentarse.
        ledger.open_cash("Operador Demo", 0).expect("open cash");
        let mut retry = base_request("att-closed", id);
        retry.items.push(ConsumptionItem {
            meal: Meal::Lunch,
            menu: Menu::Economic,
            portions: 2,
        });
        retry.tickets_count = 1;
        assert!(confirm_attendance(&mut ledger, retry).is_ok());
    }

    #[test]
    fn rejects_empty_operation_unauthorized_and_duplicate_confirmation() {
        let mut ledger = DemoLedger::new();
        ledger.open_cash("Operador Demo", 0).expect("open cash");

        let id = PensionerId(1003);
        let empty = base_request("att-empty", id);
        assert_eq!(
            confirm_attendance(&mut ledger, empty),
            Err(AttendanceError::EmptyOperation)
        );

        let mut unauthorized = base_request("att-unauth", id);
        unauthorized.items.push(ConsumptionItem {
            meal: Meal::Lunch,
            menu: Menu::Economic,
            portions: 1,
        });
        unauthorized.can_register_attendance = false;
        assert_eq!(
            confirm_attendance(&mut ledger, unauthorized),
            Err(AttendanceError::Unauthorized)
        );

        let before = ledger.find_pensioner(id).unwrap().balance_cents;
        let mut first = base_request("att-dup", id);
        first.items.push(ConsumptionItem {
            meal: Meal::Lunch,
            menu: Menu::Economic,
            portions: 1,
        });
        first.tickets_count = 1;

        confirm_attendance(&mut ledger, first.clone()).expect("first confirmation ok");
        assert_eq!(
            confirm_attendance(&mut ledger, first),
            Err(AttendanceError::DuplicateConfirmation)
        );

        // El doble clic o reintento con la misma clave deja exactamente una atención y un movimiento.
        assert_eq!(ledger.attendances().len(), 1);
        assert_eq!(ledger.ticket_movements().len(), 1);
        assert_eq!(
            ledger.find_pensioner(id).unwrap().balance_cents,
            before - LUNCH_PRICE_CENTS + DEFAULT_TICKET_VALUE_CENTS
        );
    }
}
