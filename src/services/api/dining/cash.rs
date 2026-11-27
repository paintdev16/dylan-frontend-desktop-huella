use reqwest::Method;
use serde::Deserialize;
use serde_json::{Value, json};

use super::{ApiError, Data, DiningApi, Page, local_timestamp, read_date};
use crate::{
    features::pensioners::model::PensionerId,
    state::store::{
        CashAuditRecord, CashMovement, CashMovementKind, CashSession, ClosedCashSession,
    },
};

pub struct CashSnapshot {
    pub active: Option<CashSession>,
    pub movements: Vec<CashMovement>,
    pub closed: Vec<ClosedCashSession>,
    pub ticket_value_cents: i64,
}

impl DiningApi {
    pub fn cash(&self) -> Result<CashSnapshot, ApiError> {
        let read_page = |page: u32| {
            let page = page.to_string();
            self.request::<Data<CashData>>(
                Method::GET,
                "/api/cash",
                &[("page", &page), ("closed_page", &page), ("per_page", "100")],
                None,
            )
            .map(|response| response.data)
        };
        let first = read_page(1)?;
        let last_page = first
            .movements
            .meta
            .last_page
            .max(first.closed_sessions.meta.last_page);
        let mut movements = first.movements.data;
        let mut closed = first.closed_sessions.data;
        for page in 2..=last_page {
            let next = read_page(page)?;
            movements.extend(next.movements.data);
            closed.extend(next.closed_sessions.data);
        }
        movements.sort_by_key(|row| row.id);
        movements.dedup_by_key(|row| row.id);
        let active = first.active_session.map(SessionData::open_session);
        closed.sort_by_key(|row| row.id);
        closed.dedup_by_key(|row| row.id);
        Ok(CashSnapshot {
            active,
            ticket_value_cents: first.ticket_value_cents,
            closed: closed
                .into_iter()
                .map(SessionData::closed_session)
                .collect::<Result<_, _>>()?,
            movements: movements
                .into_iter()
                .map(TryInto::try_into)
                .collect::<Result<_, _>>()?,
        })
    }

    pub fn set_ticket_value(&self, cents: i64) -> Result<(), ApiError> {
        self.resource::<Value>(
            Method::PATCH,
            "/api/cash/ticket-value",
            Some(json!({"ticket_value_cents": cents})),
        )
        .map(drop)
    }

    pub fn open_cash(&self, cents: i64) -> Result<(), ApiError> {
        self.resource::<Value>(
            Method::POST,
            "/api/cash/open",
            Some(json!({"opening_cash_cents": cents})),
        )
        .map(drop)
    }

    pub fn audit_cash(&self, tickets: Option<u32>, cash: i64) -> Result<CashAuditRecord, ApiError> {
        self.resource(
            Method::POST,
            "/api/cash/audit",
            Some(json!({"counted_tickets": tickets, "counted_cash_cents": cash})),
        )
        .map(local_audit)
    }

    pub fn close_cash(&self, tickets: Option<u32>, cash: i64) -> Result<(), ApiError> {
        self.resource::<Value>(
            Method::POST,
            "/api/cash/close",
            Some(json!({"counted_tickets": tickets, "counted_cash_cents": cash})),
        )
        .map(drop)
    }
}

#[derive(Deserialize)]
struct CashData {
    active_session: Option<SessionData>,
    ticket_value_cents: i64,
    movements: Page<MovementData>,
    closed_sessions: Page<SessionData>,
}

#[derive(Deserialize)]
struct SessionData {
    id: u64,
    opened_by_name: String,
    opened_at: String,
    closed_by_name: Option<String>,
    closed_at: Option<String>,
    opening_cash_cents: i64,
    cash_inflow_cents: i64,
    expected_cash_cents: i64,
    expected_tickets: u32,
    expected_ticket_amount_cents: i64,
    movement_count: usize,
    last_audit: Option<CashAuditRecord>,
}

impl SessionData {
    fn open_session(self) -> CashSession {
        CashSession {
            id: self.id,
            opened_by: self.opened_by_name,
            opened_at: local_timestamp(self.opened_at),
            opening_cash_cents: self.opening_cash_cents,
            cash_inflow_cents: self.cash_inflow_cents,
            last_audit: self.last_audit.map(local_audit),
            expected_tickets: self.expected_tickets,
            expected_ticket_amount_cents: self.expected_ticket_amount_cents,
            #[cfg(test)]
            movements: Vec::new(),
        }
    }

    fn closed_session(self) -> Result<ClosedCashSession, ApiError> {
        let (Some(closed_by), Some(closed_at), Some(audit)) =
            (self.closed_by_name, self.closed_at, self.last_audit)
        else {
            return Err(ApiError::Decode(format!(
                "El turno #{} tiene un cierre incompleto.",
                self.id
            )));
        };
        Ok(ClosedCashSession {
            id: self.id,
            opened_by: self.opened_by_name,
            opened_at: local_timestamp(self.opened_at),
            closed_by,
            closed_at: local_timestamp(closed_at),
            opening_cash_cents: self.opening_cash_cents,
            cash_inflow_cents: self.cash_inflow_cents,
            expected_cash_cents: self.expected_cash_cents,
            expected_tickets: self.expected_tickets,
            expected_ticket_amount_cents: self.expected_ticket_amount_cents,
            audit: local_audit(audit),
            movement_count: self.movement_count,
        })
    }
}

#[derive(Deserialize)]
struct MovementData {
    id: u64,
    code: String,
    cash_session_id: u64,
    attendance_id: Option<u64>,
    #[serde(rename = "type")]
    kind: CashMovementKind,
    pensioner_id: Option<u32>,
    pensioner_code: Option<String>,
    pensioner_name: Option<String>,
    description: String,
    amount_cents: i64,
    operator_name: String,
    service_date: String,
    recorded_at: String,
    ticket_count: Option<u32>,
    ticket_unit_cents: Option<i64>,
    consumption_cents: Option<i64>,
    resulting_balance_cents: Option<i64>,
}

impl TryFrom<MovementData> for CashMovement {
    type Error = ApiError;

    fn try_from(row: MovementData) -> Result<Self, ApiError> {
        let date = read_date(&row.service_date)?;
        Ok(Self {
            id: row.code,
            cash_session_id: row.cash_session_id,
            attendance_id: row.attendance_id,
            kind: row.kind,
            pensioner_id: row.pensioner_id.map(PensionerId),
            pensioner_code: row.pensioner_code,
            pensioner_name: row.pensioner_name,
            description: row.description,
            amount_cents: row.amount_cents,
            operator_name: row.operator_name,
            date,
            recorded_at: local_timestamp(row.recorded_at),
            ticket_count: row.ticket_count,
            ticket_unit_cents: row.ticket_unit_cents,
            consumption_cents: row.consumption_cents,
            resulting_balance_cents: row.resulting_balance_cents,
        })
    }
}

fn local_audit(mut audit: CashAuditRecord) -> CashAuditRecord {
    audit.audited_at = local_timestamp(audit.audited_at);
    audit
}
