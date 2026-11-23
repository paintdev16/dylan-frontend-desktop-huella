use std::{
    collections::{BTreeMap, HashSet},
    time::{SystemTime, UNIX_EPOCH},
};

use gpui_kit::Context;

use crate::features::{
    consumptions::{
        confirm_attendance::{
            AttendanceError, AttendanceRequest, AttendanceSummary,
            confirm_attendance as execute_confirm_attendance,
        },
        order::ConsumptionItem,
    },
    pensioners::{
        SelectionSource,
        model::{
            Pensioner, PensionerDraft, PensionerId, demo_pensioners, find_by_id, format_soles,
        },
    },
};

/// Tarifas de demostración en céntimos de sol (alineadas a `dylan-huella-comedor`).
pub const BREAKFAST_PRICE_CENTS: i64 = 700;
pub const LUNCH_PRICE_CENTS: i64 = 700;
/// Valor inicial configurable del ticket físico: S/ 7.00 (700 céntimos).
pub const DEFAULT_TICKET_VALUE_CENTS: i64 = 700;

/// Fecha civil (`YYYY-MM-DD`) sin dependencias externas para filtrar Dashboard y Reportes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct CalendarDate {
    pub year: i32,
    pub month: u8,
    pub day: u8,
}

impl CalendarDate {
    pub fn new(year: i32, month: u8, day: u8) -> Option<Self> {
        if !(1..=12).contains(&month) {
            return None;
        }
        let max_day = days_in_month(year, month);
        if day < 1 || day > max_day {
            return None;
        }
        Some(Self { year, month, day })
    }

    pub fn today() -> Self {
        let secs = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_secs() as i64)
            .unwrap_or(0);
        // Ajuste aproximado a hora local de Perú (UTC-5) para alinear "Hoy" en la estación.
        let local_secs = secs - 5 * 3_600;
        let days = local_secs.div_euclid(86_400);
        Self::from_unix_days(days)
    }

    pub fn parse_iso(raw: &str) -> Option<Self> {
        let trimmed = raw.trim();
        let mut parts = trimmed.split('-');
        let year: i32 = parts.next()?.parse().ok()?;
        let month: u8 = parts.next()?.parse().ok()?;
        let day: u8 = parts.next()?.parse().ok()?;
        if parts.next().is_some() || !(1970..=9999).contains(&year) {
            return None;
        }
        Self::new(year, month, day)
    }

    pub fn format_iso(self) -> String {
        format!("{:04}-{:02}-{:02}", self.year, self.month, self.day)
    }

    pub fn sub_days(self, days: i64) -> Self {
        Self::from_unix_days(self.to_unix_days() - days)
    }

    /// Conversión civil -> días desde 1970-01-01 (algoritmo de Howard Hinnant).
    fn to_unix_days(self) -> i64 {
        let y = i64::from(self.year) - if self.month <= 2 { 1 } else { 0 };
        let era = y.div_euclid(400);
        let yoe = y - era * 400;
        let m = i64::from(self.month);
        let d = i64::from(self.day);
        let doy = (153 * (m + if m > 2 { -3 } else { 9 }) + 2) / 5 + d - 1;
        let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
        era * 146_097 + doe - 719_468
    }

    /// Conversión días desde 1970-01-01 -> fecha civil (algoritmo de Howard Hinnant).
    fn from_unix_days(z: i64) -> Self {
        let z = z + 719_468;
        let era = z.div_euclid(146_097);
        let doe = z - era * 146_097;
        let yoe = (doe - doe / 1_460 + doe / 36_524 - doe / 146_096) / 365;
        let y = yoe + era * 400;
        let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
        let mp = (5 * doy + 2) / 153;
        let d = doy - (153 * mp + 2) / 5 + 1;
        let m = mp + if mp < 10 { 3 } else { -9 };
        let year = (y + if m <= 2 { 1 } else { 0 }) as i32;
        Self {
            year,
            month: m as u8,
            day: d as u8,
        }
    }
}

fn days_in_month(year: i32, month: u8) -> u8 {
    match month {
        1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
        4 | 6 | 9 | 11 => 30,
        2 => {
            let leap = (year % 4 == 0 && year % 100 != 0) || (year % 400 == 0);
            if leap { 29 } else { 28 }
        }
        _ => 0,
    }
}

/// Filtro de periodo compartido entre Dashboard y Reportes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum DatePeriodFilter {
    #[default]
    Today,
    Last7Days,
    ThisMonth,
    All,
    Custom {
        from: CalendarDate,
        to: CalendarDate,
    },
}

impl DatePeriodFilter {
    pub const PRESETS: [Self; 4] = [Self::Today, Self::Last7Days, Self::ThisMonth, Self::All];

    pub fn label(self) -> String {
        match self {
            Self::Today => "Hoy".into(),
            Self::Last7Days => "Últimos 7 días".into(),
            Self::ThisMonth => "Este mes".into(),
            Self::All => "Todo".into(),
            Self::Custom { from, to } => format!("{} a {}", from.format_iso(), to.format_iso()),
        }
    }

    pub fn contains(self, date: CalendarDate, today: CalendarDate) -> bool {
        match self {
            Self::Today => date == today,
            Self::Last7Days => date >= today.sub_days(6) && date <= today,
            Self::ThisMonth => date.year == today.year && date.month == today.month,
            Self::All => true,
            Self::Custom { from, to } => date >= from && date <= to,
        }
    }
}

/// Resumen por pensionista dentro de un periodo consultado.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PensionerPeriodSummary {
    pub pensioner_id: PensionerId,
    pub pensioner_code: String,
    pub pensioner_name: String,
    pub attendances: usize,
    pub breakfasts: u32,
    pub lunches: u32,
    pub consumption_cents: i64,
    pub tickets_count: u32,
    pub ticket_amount_cents: i64,
    pub cash_paid_cents: i64,
    pub current_balance_cents: i64,
}

/// Resumen mensual de consumos, efectivo y tickets por pensionista.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MonthlyPensionerSummary {
    pub month_label: String,
    pub consumed_cents: i64,
    pub cash_paid_cents: i64,
    pub tickets_count: u32,
    pub tickets_cents: i64,
    /// Positivo = a favor en el mes; negativo = déficit del mes.
    pub net_balance_cents: i64,
}

/// Totales acumulados de cuenta corriente para un pensionista.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct PensionerAccountTotals {
    pub consumed_cents: i64,
    pub cash_paid_cents: i64,
    pub tickets_count: u32,
    pub tickets_cents: i64,
}

impl PensionerAccountTotals {
    pub fn total_paid_cents(self) -> i64 {
        self.cash_paid_cents.saturating_add(self.tickets_cents)
    }
}

/// Consolidado compartido por Dashboard y Reportes para un mismo periodo.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PeriodReport {
    pub attendances_count: usize,
    pub breakfasts: u32,
    pub lunches: u32,
    pub rations: u32,
    pub consumption_cents: i64,
    pub tickets_received: u32,
    pub ticket_amount_cents: i64,
    pub cash_paid_cents: i64,
    pub cash_movements_count: usize,
    pub credit_balance_cents: i64,
    pub pending_debt_cents: i64,
    pub attendances: Vec<AttendanceRecord>,
    pub ticket_movements: Vec<CashTicketMovement>,
    pub by_pensioner: Vec<PensionerPeriodSummary>,
}

/// Registro confirmado de una atención (consumo, abono con tickets físicos o ambos).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AttendanceRecord {
    pub id: u64,
    pub idempotency_key: String,
    pub pensioner_id: PensionerId,
    pub pensioner_code: String,
    pub pensioner_name: String,
    pub breakfasts: u32,
    pub lunches: u32,
    pub items: Vec<ConsumptionItem>,
    pub picked_up_by: Option<String>,
    pub consumption_cents: i64,
    /// Cantidad de tickets físicos recibidos (almacenada por separado del importe).
    pub tickets_count: u32,
    /// Valor unitario del ticket al momento de la confirmación.
    pub ticket_unit_cents: i64,
    /// Importe monetario del abono en tickets físicos en céntimos.
    pub ticket_amount_cents: i64,
    pub balance_before_cents: i64,
    pub balance_after_cents: i64,
    pub cash_session_id: Option<u64>,
    pub movement_code: Option<String>,
    pub source: SelectionSource,
    pub operator_name: String,
    pub date: CalendarDate,
    pub recorded_at: String,
}

impl AttendanceRecord {
    pub fn rations(&self) -> u32 {
        self.breakfasts + self.lunches
    }

    pub fn kind_label(&self) -> &'static str {
        match (self.rations() > 0, self.tickets_count > 0) {
            (true, true) => "Consumo + Abono",
            (true, false) => "Consumo",
            (false, true) => "Abono con tickets",
            (false, false) => "Sin movimiento",
        }
    }
}

/// Tipos de movimiento auditable en Caja (alineados a `dylan-huella-comedor`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CashMovementKind {
    Opening,
    Closing,
    CashPayment,
    TicketCredit,
}

impl CashMovementKind {
    pub const fn label(self) -> &'static str {
        match self {
            Self::Opening => "Apertura de caja",
            Self::Closing => "Cierre de caja",
            Self::CashPayment => "Pago en efectivo",
            Self::TicketCredit => "Abono por tickets",
        }
    }
}

/// Movimiento auditable de Caja (apertura, cierre, pago en efectivo o abono por tickets)
/// con todos los datos necesarios para auditoría y comprobante ("Recibo").
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CashMovement {
    pub id: String,
    pub cash_session_id: u64,
    pub attendance_id: Option<u64>,
    pub kind: CashMovementKind,
    pub pensioner_id: Option<PensionerId>,
    pub pensioner_code: Option<String>,
    pub pensioner_name: Option<String>,
    pub description: String,
    pub amount_cents: i64,
    pub operator_name: String,
    pub date: CalendarDate,
    pub recorded_at: String,
    pub ticket_count: Option<u32>,
    pub ticket_unit_cents: Option<i64>,
    pub consumption_cents: Option<i64>,
    pub resulting_balance_cents: Option<i64>,
}

/// Movimiento de tickets físicos registrado en Caja por una atención confirmada.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CashTicketMovement {
    pub id: u64,
    pub movement_code: String,
    pub cash_session_id: u64,
    pub attendance_id: u64,
    pub pensioner_id: PensionerId,
    pub pensioner_code: String,
    pub pensioner_name: String,
    pub tickets_count: u32,
    pub ticket_unit_cents: i64,
    pub amount_cents: i64,
    pub operator_name: String,
    pub date: CalendarDate,
    pub recorded_at: String,
}

/// Resultado de un arqueo de caja comparando lo esperado contra lo contado físicamente.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CashAuditRecord {
    pub expected_tickets: u32,
    pub counted_tickets: u32,
    pub ticket_diff: i64,
    pub expected_ticket_amount_cents: i64,
    pub counted_ticket_amount_cents: i64,
    pub expected_cash_cents: i64,
    pub counted_cash_cents: i64,
    pub cash_diff_cents: i64,
    pub audited_at: String,
}

impl CashAuditRecord {
    pub fn is_balanced(&self) -> bool {
        self.ticket_diff == 0 && self.cash_diff_cents == 0
    }
}

/// Turno de caja actualmente abierto.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CashSession {
    pub id: u64,
    pub opened_by: String,
    pub opened_at: String,
    pub opening_cash_cents: i64,
    pub cash_inflow_cents: i64,
    pub last_audit: Option<CashAuditRecord>,
    pub movements: Vec<CashTicketMovement>,
}

impl CashSession {
    pub fn total_tickets(&self) -> u32 {
        self.movements.iter().map(|m| m.tickets_count).sum()
    }

    pub fn total_ticket_amount_cents(&self) -> i64 {
        self.movements.iter().map(|m| m.amount_cents).sum()
    }

    pub fn expected_cash_cents(&self) -> i64 {
        self.opening_cash_cents
            .saturating_add(self.cash_inflow_cents)
    }

    pub fn movement_count(&self) -> usize {
        self.movements.len()
    }
}

/// Registro histórico de un turno de caja cerrado con su arqueo final.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ClosedCashSession {
    pub id: u64,
    pub opened_by: String,
    pub opened_at: String,
    pub closed_by: String,
    pub closed_at: String,
    pub opening_cash_cents: i64,
    pub cash_inflow_cents: i64,
    pub expected_cash_cents: i64,
    pub expected_tickets: u32,
    pub expected_ticket_amount_cents: i64,
    pub audit: CashAuditRecord,
    pub movement_count: usize,
    pub notes: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CashError {
    AlreadyOpen,
    NotOpen,
    InvalidOpeningAmount,
    InvalidCountedCash,
    InvalidTicketValue,
    InvalidPaymentAmount,
    PensionerNotFound,
    DuplicateOperation,
}

impl CashError {
    pub const fn message(self) -> &'static str {
        match self {
            Self::AlreadyOpen => "Ya existe una caja abierta en este turno.",
            Self::NotOpen => "La caja está cerrada. Ábrela antes de realizar esta operación.",
            Self::InvalidOpeningAmount => {
                "El monto de apertura debe ser un número válido mayor o igual a S/ 0.00."
            }
            Self::InvalidCountedCash => {
                "El efectivo contado debe ser un número válido mayor o igual a S/ 0.00."
            }
            Self::InvalidTicketValue => {
                "El valor del ticket físico debe ser un monto mayor a S/ 0.00."
            }
            Self::InvalidPaymentAmount => "El importe del pago debe ser mayor a S/ 0.00.",
            Self::PensionerNotFound => "No se encontró el pensionista indicado.",
            Self::DuplicateOperation => "Esta operación de caja ya fue registrada.",
        }
    }
}

/// Fuente única en memoria compartida por Pensionistas, Consumos, Caja, Dashboard y Reportes.
/// Probable de forma determinista sin abrir GPUI.
#[derive(Debug, Clone)]
pub struct DemoLedger {
    pensioners: Vec<Pensioner>,
    attendances: Vec<AttendanceRecord>,
    active_cash: Option<CashSession>,
    closed_cash_sessions: Vec<ClosedCashSession>,
    ticket_movements: Vec<CashTicketMovement>,
    cash_movements: Vec<CashMovement>,
    ticket_value_cents: i64,
    processed_keys: HashSet<String>,
    next_attendance_id: u64,
    next_movement_id: u64,
    next_cash_movement_seq: u32,
    next_cash_session_id: u64,
    next_event_seq: u32,
    clock_override: Option<CalendarDate>,
}

impl Default for DemoLedger {
    fn default() -> Self {
        Self::new()
    }
}

impl DemoLedger {
    pub fn new() -> Self {
        Self {
            pensioners: demo_pensioners(),
            attendances: Vec::new(),
            active_cash: None,
            closed_cash_sessions: Vec::new(),
            ticket_movements: Vec::new(),
            cash_movements: Vec::new(),
            ticket_value_cents: DEFAULT_TICKET_VALUE_CENTS,
            processed_keys: HashSet::new(),
            next_attendance_id: 1,
            next_movement_id: 1,
            next_cash_movement_seq: 1,
            next_cash_session_id: 1,
            next_event_seq: 1,
            clock_override: None,
        }
    }

    pub fn current_date(&self) -> CalendarDate {
        self.clock_override.unwrap_or_else(CalendarDate::today)
    }

    #[cfg(test)]
    pub fn set_test_date(&mut self, date: CalendarDate) {
        self.clock_override = Some(date);
    }

    pub fn ticket_value_cents(&self) -> i64 {
        self.ticket_value_cents
    }

    pub fn set_ticket_value_cents(&mut self, cents: i64) -> Result<(), CashError> {
        if cents <= 0 {
            return Err(CashError::InvalidTicketValue);
        }
        self.ticket_value_cents = cents;
        Ok(())
    }

    pub fn pensioners(&self) -> &[Pensioner] {
        &self.pensioners
    }

    pub fn find_pensioner(&self, id: PensionerId) -> Option<&Pensioner> {
        find_by_id(&self.pensioners, id)
    }

    fn save_pensioner(
        &mut self,
        id: Option<PensionerId>,
        draft: PensionerDraft,
    ) -> Result<PensionerId, &'static str> {
        let draft = draft.validated()?;
        if !draft.dni.is_empty()
            && self
                .pensioners
                .iter()
                .any(|p| Some(p.id) != id && p.dni == draft.dni)
        {
            return Err("Ya existe un pensionista con ese DNI.");
        }
        let phone = (!draft.phone.is_empty()).then_some(draft.phone);
        if let Some(id) = id {
            let pensioner = self
                .pensioners
                .iter_mut()
                .find(|p| p.id == id)
                .ok_or("El pensionista ya no está disponible.")?;
            pensioner.full_name = draft.full_name;
            pensioner.dni = draft.dni;
            pensioner.phone = phone;
            return Ok(id);
        }
        let id = self
            .pensioners
            .iter()
            .map(|p| p.id.0)
            .max()
            .unwrap_or_default()
            .checked_add(1)
            .map(PensionerId)
            .ok_or("No se pudo asignar un código.")?;
        self.pensioners.push(Pensioner {
            id,
            full_name: draft.full_name,
            dni: draft.dni,
            phone,
            plan: String::new(),
            notes: String::new(),
            active: true,
            fingerprint_enrolled: false,
            temporary_profile: true,
            balance_cents: 0,
            tickets_balance: 0,
        });
        Ok(id)
    }

    pub fn set_fingerprint_enrolled(&mut self, id: PensionerId, enrolled: bool) -> bool {
        if let Some(pensioner) = self.pensioners.iter_mut().find(|p| p.id == id) {
            if pensioner.fingerprint_enrolled != enrolled {
                pensioner.fingerprint_enrolled = enrolled;
                return true;
            }
        }
        false
    }

    pub fn attendances(&self) -> &[AttendanceRecord] {
        &self.attendances
    }

    pub fn is_cash_open(&self) -> bool {
        self.active_cash.is_some()
    }

    pub fn active_cash(&self) -> Option<&CashSession> {
        self.active_cash.as_ref()
    }

    pub fn closed_cash_sessions(&self) -> &[ClosedCashSession] {
        &self.closed_cash_sessions
    }

    pub fn ticket_movements(&self) -> &[CashTicketMovement] {
        &self.ticket_movements
    }

    pub fn cash_movements(&self) -> &[CashMovement] {
        &self.cash_movements
    }

    pub fn find_cash_movement(&self, movement_id: &str) -> Option<&CashMovement> {
        self.cash_movements.iter().find(|m| m.id == movement_id)
    }

    #[allow(dead_code)]
    pub fn active_cash_inflow_cents(&self) -> i64 {
        self.active_cash
            .as_ref()
            .map(|cash| cash.cash_inflow_cents)
            .unwrap_or(0)
    }

    pub fn expected_cash_cents(&self) -> i64 {
        self.active_cash
            .as_ref()
            .map(CashSession::expected_cash_cents)
            .unwrap_or(0)
    }

    pub fn total_cash_inflow_cents(&self) -> i64 {
        self.cash_movements
            .iter()
            .filter(|m| m.kind == CashMovementKind::CashPayment)
            .map(|m| m.amount_cents)
            .sum()
    }

    pub fn total_breakfasts(&self) -> u32 {
        self.attendances.iter().map(|a| a.breakfasts).sum()
    }

    pub fn total_lunches(&self) -> u32 {
        self.attendances.iter().map(|a| a.lunches).sum()
    }

    pub fn total_tickets_received(&self) -> u32 {
        self.ticket_movements.iter().map(|m| m.tickets_count).sum()
    }

    pub fn total_ticket_amount_cents(&self) -> i64 {
        self.ticket_movements.iter().map(|m| m.amount_cents).sum()
    }

    pub fn total_credit_balance_cents(&self) -> i64 {
        self.pensioners.iter().map(Pensioner::credit_cents).sum()
    }

    pub fn total_pending_debt_cents(&self) -> i64 {
        self.pensioners.iter().map(Pensioner::debt_cents).sum()
    }

    /// Totales históricos de cuenta corriente para un pensionista (consumos, efectivo y tickets).
    pub fn pensioner_totals(&self, id: PensionerId) -> PensionerAccountTotals {
        let consumed_cents = self
            .attendances
            .iter()
            .filter(|a| a.pensioner_id == id)
            .map(|a| a.consumption_cents)
            .sum();
        let cash_paid_cents = self
            .cash_movements
            .iter()
            .filter(|m| m.kind == CashMovementKind::CashPayment && m.pensioner_id == Some(id))
            .map(|m| m.amount_cents)
            .sum();
        let (tickets_count, tickets_cents) = self
            .ticket_movements
            .iter()
            .filter(|m| m.pensioner_id == id)
            .fold((0u32, 0i64), |(cnt, amt), m| {
                (cnt + m.tickets_count, amt + m.amount_cents)
            });
        PensionerAccountTotals {
            consumed_cents,
            cash_paid_cents,
            tickets_count,
            tickets_cents,
        }
    }

    /// Desglose mensual por pensionista (consumido, efectivo, tickets y balance neto del mes),
    /// ordenado del mes más reciente al más antiguo.
    pub fn monthly_summary(&self, id: PensionerId) -> Vec<MonthlyPensionerSummary> {
        let mut by_month: BTreeMap<(i32, u8), MonthlyPensionerSummary> = BTreeMap::new();

        for a in self
            .attendances
            .iter()
            .filter(|a| a.pensioner_id == id && a.consumption_cents > 0)
        {
            let key = (a.date.year, a.date.month);
            let entry = by_month
                .entry(key)
                .or_insert_with(|| MonthlyPensionerSummary {
                    month_label: format!("{:02}/{}", a.date.month, a.date.year),
                    consumed_cents: 0,
                    cash_paid_cents: 0,
                    tickets_count: 0,
                    tickets_cents: 0,
                    net_balance_cents: 0,
                });
            entry.consumed_cents += a.consumption_cents;
        }

        for m in self
            .cash_movements
            .iter()
            .filter(|m| m.pensioner_id == Some(id))
        {
            let key = (m.date.year, m.date.month);
            match m.kind {
                CashMovementKind::CashPayment => {
                    let entry = by_month
                        .entry(key)
                        .or_insert_with(|| MonthlyPensionerSummary {
                            month_label: format!("{:02}/{}", m.date.month, m.date.year),
                            consumed_cents: 0,
                            cash_paid_cents: 0,
                            tickets_count: 0,
                            tickets_cents: 0,
                            net_balance_cents: 0,
                        });
                    entry.cash_paid_cents += m.amount_cents;
                }
                CashMovementKind::TicketCredit => {
                    let entry = by_month
                        .entry(key)
                        .or_insert_with(|| MonthlyPensionerSummary {
                            month_label: format!("{:02}/{}", m.date.month, m.date.year),
                            consumed_cents: 0,
                            cash_paid_cents: 0,
                            tickets_count: 0,
                            tickets_cents: 0,
                            net_balance_cents: 0,
                        });
                    entry.tickets_count += m.ticket_count.unwrap_or(0);
                    entry.tickets_cents += m.amount_cents;
                }
                CashMovementKind::Opening | CashMovementKind::Closing => {}
            }
        }

        by_month
            .into_values()
            .rev()
            .map(|mut row| {
                row.net_balance_cents =
                    row.cash_paid_cents + row.tickets_cents - row.consumed_cents;
                row
            })
            .collect()
    }

    /// Calcula el consolidado del periodo indicado sobre los mismos registros de Consumos y Caja.
    pub fn report_for_period(&self, filter: DatePeriodFilter) -> PeriodReport {
        let today = self.current_date();
        let attendances: Vec<AttendanceRecord> = self
            .attendances
            .iter()
            .filter(|a| filter.contains(a.date, today))
            .cloned()
            .collect();
        let ticket_movements: Vec<CashTicketMovement> = self
            .ticket_movements
            .iter()
            .filter(|m| filter.contains(m.date, today))
            .cloned()
            .collect();
        let cash_payments: Vec<&CashMovement> = self
            .cash_movements
            .iter()
            .filter(|m| m.kind == CashMovementKind::CashPayment && filter.contains(m.date, today))
            .collect();

        let breakfasts: u32 = attendances.iter().map(|a| a.breakfasts).sum();
        let lunches: u32 = attendances.iter().map(|a| a.lunches).sum();
        let consumption_cents: i64 = attendances.iter().map(|a| a.consumption_cents).sum();
        let tickets_received: u32 = ticket_movements.iter().map(|m| m.tickets_count).sum();
        let ticket_amount_cents: i64 = ticket_movements.iter().map(|m| m.amount_cents).sum();
        let cash_paid_cents: i64 = cash_payments.iter().map(|m| m.amount_cents).sum();

        let mut by_pensioner: Vec<PensionerPeriodSummary> = Vec::new();
        for record in &attendances {
            if let Some(entry) = by_pensioner
                .iter_mut()
                .find(|entry| entry.pensioner_id == record.pensioner_id)
            {
                entry.attendances += 1;
                entry.breakfasts += record.breakfasts;
                entry.lunches += record.lunches;
                entry.consumption_cents += record.consumption_cents;
                entry.tickets_count += record.tickets_count;
                entry.ticket_amount_cents += record.ticket_amount_cents;
            } else {
                let current_balance_cents = self
                    .find_pensioner(record.pensioner_id)
                    .map(|p| p.balance_cents)
                    .unwrap_or(record.balance_after_cents);
                by_pensioner.push(PensionerPeriodSummary {
                    pensioner_id: record.pensioner_id,
                    pensioner_code: record.pensioner_code.clone(),
                    pensioner_name: record.pensioner_name.clone(),
                    attendances: 1,
                    breakfasts: record.breakfasts,
                    lunches: record.lunches,
                    consumption_cents: record.consumption_cents,
                    tickets_count: record.tickets_count,
                    ticket_amount_cents: record.ticket_amount_cents,
                    cash_paid_cents: 0,
                    current_balance_cents,
                });
            }
        }
        for pay in &cash_payments {
            if let Some(pid) = pay.pensioner_id {
                if let Some(entry) = by_pensioner.iter_mut().find(|e| e.pensioner_id == pid) {
                    entry.cash_paid_cents += pay.amount_cents;
                } else {
                    let current_balance_cents = self
                        .find_pensioner(pid)
                        .map(|p| p.balance_cents)
                        .unwrap_or(0);
                    by_pensioner.push(PensionerPeriodSummary {
                        pensioner_id: pid,
                        pensioner_code: pay.pensioner_code.clone().unwrap_or_else(|| pid.code()),
                        pensioner_name: pay
                            .pensioner_name
                            .clone()
                            .unwrap_or_else(|| "Pensionista".into()),
                        attendances: 0,
                        breakfasts: 0,
                        lunches: 0,
                        consumption_cents: 0,
                        tickets_count: 0,
                        ticket_amount_cents: 0,
                        cash_paid_cents: pay.amount_cents,
                        current_balance_cents,
                    });
                }
            }
        }
        by_pensioner.sort_by_key(|entry| entry.pensioner_id);

        PeriodReport {
            attendances_count: attendances.len(),
            breakfasts,
            lunches,
            rations: breakfasts + lunches,
            consumption_cents,
            tickets_received,
            ticket_amount_cents,
            cash_paid_cents,
            cash_movements_count: ticket_movements.len() + cash_payments.len(),
            credit_balance_cents: self.total_credit_balance_cents(),
            pending_debt_cents: self.total_pending_debt_cents(),
            attendances,
            ticket_movements,
            by_pensioner,
        }
    }

    pub fn open_cash(
        &mut self,
        operator: impl Into<String>,
        opening_cash_cents: i64,
    ) -> Result<&CashSession, CashError> {
        if self.active_cash.is_some() {
            return Err(CashError::AlreadyOpen);
        }
        if opening_cash_cents < 0 {
            return Err(CashError::InvalidOpeningAmount);
        }

        let id = self.next_cash_session_id;
        self.next_cash_session_id += 1;
        let date = self.current_date();
        let opened_at = self.next_timestamp(date);
        let operator_name = operator.into();
        let mov_code = self.next_cash_movement_code();

        self.cash_movements.push(CashMovement {
            id: mov_code,
            cash_session_id: id,
            attendance_id: None,
            kind: CashMovementKind::Opening,
            pensioner_id: None,
            pensioner_code: None,
            pensioner_name: None,
            description: format!("Apertura de caja · Turno #{id}"),
            amount_cents: opening_cash_cents,
            operator_name: operator_name.clone(),
            date,
            recorded_at: opened_at.clone(),
            ticket_count: None,
            ticket_unit_cents: None,
            consumption_cents: None,
            resulting_balance_cents: None,
        });

        self.active_cash = Some(CashSession {
            id,
            opened_by: operator_name,
            opened_at,
            opening_cash_cents,
            cash_inflow_cents: 0,
            last_audit: None,
            movements: Vec::new(),
        });

        Ok(self.active_cash.as_ref().expect("cash session just opened"))
    }

    /// Registra un pago en efectivo separado de los tickets físicos (suma a `cash_inflow_cents`).
    #[allow(dead_code)]
    pub fn record_cash_payment(
        &mut self,
        idempotency_key: impl Into<String>,
        pensioner_id: PensionerId,
        amount_cents: i64,
        operator: impl Into<String>,
    ) -> Result<CashMovement, CashError> {
        let key = idempotency_key.into();
        if key.trim().is_empty() || self.processed_keys.contains(&key) {
            return Err(CashError::DuplicateOperation);
        }
        if self.active_cash.is_none() {
            return Err(CashError::NotOpen);
        }
        if amount_cents <= 0 {
            return Err(CashError::InvalidPaymentAmount);
        }

        let (pensioner_code, pensioner_name, resulting_balance) = {
            let pensioner = self
                .pensioners
                .iter_mut()
                .find(|p| p.id == pensioner_id)
                .ok_or(CashError::PensionerNotFound)?;
            pensioner.balance_cents = pensioner.balance_cents.saturating_add(amount_cents);
            (
                pensioner.code(),
                pensioner.full_name.clone(),
                pensioner.balance_cents,
            )
        };

        let date = self.current_date();
        let recorded_at = self.next_timestamp(date);
        let mov_code = self.next_cash_movement_code();
        let operator_name = operator.into();

        let cash = self.active_cash.as_mut().expect("open cash verified above");
        cash.cash_inflow_cents = cash.cash_inflow_cents.saturating_add(amount_cents);
        let cash_session_id = cash.id;

        let movement = CashMovement {
            id: mov_code,
            cash_session_id,
            attendance_id: None,
            kind: CashMovementKind::CashPayment,
            pensioner_id: Some(pensioner_id),
            pensioner_code: Some(pensioner_code),
            pensioner_name: Some(pensioner_name.clone()),
            description: format!("Pago en efectivo · {pensioner_name}"),
            amount_cents,
            operator_name,
            date,
            recorded_at,
            ticket_count: None,
            ticket_unit_cents: None,
            consumption_cents: None,
            resulting_balance_cents: Some(resulting_balance),
        };

        self.processed_keys.insert(key);
        self.cash_movements.push(movement.clone());
        Ok(movement)
    }

    pub fn audit_cash(
        &mut self,
        counted_tickets: u32,
        counted_cash_cents: i64,
    ) -> Result<CashAuditRecord, CashError> {
        if counted_cash_cents < 0 {
            return Err(CashError::InvalidCountedCash);
        }
        let Some(session) = self.active_cash.as_ref() else {
            return Err(CashError::NotOpen);
        };
        let expected_tickets = session.total_tickets();
        let expected_ticket_amount_cents = session.total_ticket_amount_cents();
        let expected_cash_cents = session.expected_cash_cents();
        let ticket_unit_cents = self.ticket_value_cents;
        let date = self.current_date();
        let audited_at = self.next_timestamp(date);

        let audit = build_audit(
            expected_tickets,
            counted_tickets,
            ticket_unit_cents,
            expected_ticket_amount_cents,
            expected_cash_cents,
            counted_cash_cents,
            audited_at,
        );

        if let Some(session_mut) = self.active_cash.as_mut() {
            session_mut.last_audit = Some(audit.clone());
        }

        Ok(audit)
    }

    pub fn close_cash(
        &mut self,
        operator: impl Into<String>,
        counted_tickets: u32,
        counted_cash_cents: i64,
        notes: impl Into<String>,
    ) -> Result<&ClosedCashSession, CashError> {
        if counted_cash_cents < 0 {
            return Err(CashError::InvalidCountedCash);
        }
        let Some(session) = self.active_cash.take() else {
            return Err(CashError::NotOpen);
        };

        let expected_tickets = session.total_tickets();
        let expected_ticket_amount_cents = session.total_ticket_amount_cents();
        let expected_cash_cents = session.expected_cash_cents();
        let cash_inflow_cents = session.cash_inflow_cents;
        let movement_count = session.movement_count();
        let ticket_unit_cents = self.ticket_value_cents;
        let date = self.current_date();
        let closed_at = self.next_timestamp(date);
        let operator_name = operator.into();
        let mov_code = self.next_cash_movement_code();

        let audit = build_audit(
            expected_tickets,
            counted_tickets,
            ticket_unit_cents,
            expected_ticket_amount_cents,
            expected_cash_cents,
            counted_cash_cents,
            closed_at.clone(),
        );

        self.cash_movements.push(CashMovement {
            id: mov_code,
            cash_session_id: session.id,
            attendance_id: None,
            kind: CashMovementKind::Closing,
            pensioner_id: None,
            pensioner_code: None,
            pensioner_name: None,
            description: format!("Cierre y arqueo de caja · Turno #{}", session.id),
            amount_cents: expected_cash_cents,
            operator_name: operator_name.clone(),
            date,
            recorded_at: closed_at.clone(),
            ticket_count: None,
            ticket_unit_cents: None,
            consumption_cents: None,
            resulting_balance_cents: None,
        });

        let closed = ClosedCashSession {
            id: session.id,
            opened_by: session.opened_by,
            opened_at: session.opened_at,
            closed_by: operator_name,
            closed_at,
            opening_cash_cents: session.opening_cash_cents,
            cash_inflow_cents,
            expected_cash_cents,
            expected_tickets,
            expected_ticket_amount_cents,
            audit,
            movement_count,
            notes: notes.into().trim().to_string(),
        };

        self.closed_cash_sessions.push(closed);
        Ok(self
            .closed_cash_sessions
            .last()
            .expect("closed cash session just pushed"))
    }

    pub(crate) fn has_processed_key(&self, key: &str) -> bool {
        self.processed_keys.contains(key)
    }

    pub(crate) fn commit_attendance_atomic(
        &mut self,
        request: AttendanceRequest,
        summary: AttendanceSummary,
    ) -> AttendanceRecord {
        let AttendanceRequest {
            idempotency_key,
            pensioner_id,
            items,
            picked_up_by,
            tickets_count,
            source,
            operator_name,
            ..
        } = request;
        let AttendanceSummary {
            breakfasts,
            lunches,
            consumption_cents,
            ticket_unit_cents,
            ticket_amount_cents,
            projected_balance_cents: balance_after_cents,
        } = summary;
        let date = self.current_date();
        let recorded_at = self.next_timestamp(date);
        let attendance_id = self.next_attendance_id;
        self.next_attendance_id += 1;

        let pensioner = self
            .pensioners
            .iter_mut()
            .find(|p| p.id == pensioner_id)
            .expect("pensioner validated before commit");

        let balance_before_cents = pensioner.balance_cents;
        pensioner.balance_cents = balance_after_cents;
        pensioner.tickets_balance = pensioner.tickets_balance.saturating_add(tickets_count);

        let pensioner_code = pensioner.code();
        let pensioner_name = pensioner.full_name.clone();

        let (cash_session_id, movement_code) = if tickets_count > 0 {
            let movement_id = self.next_movement_id;
            self.next_movement_id += 1;
            let mov_code = self.next_cash_movement_code();

            let cash = self
                .active_cash
                .as_mut()
                .expect("open cash validated before commit");
            let session_id = cash.id;

            let ticket_mov = CashTicketMovement {
                id: movement_id,
                movement_code: mov_code.clone(),
                cash_session_id: session_id,
                attendance_id,
                pensioner_id,
                pensioner_code: pensioner_code.clone(),
                pensioner_name: pensioner_name.clone(),
                tickets_count,
                ticket_unit_cents,
                amount_cents: ticket_amount_cents,
                operator_name: operator_name.clone(),
                date,
                recorded_at: recorded_at.clone(),
            };
            cash.movements.push(ticket_mov.clone());
            self.ticket_movements.push(ticket_mov);

            self.cash_movements.push(CashMovement {
                id: mov_code.clone(),
                cash_session_id: session_id,
                attendance_id: Some(attendance_id),
                kind: CashMovementKind::TicketCredit,
                pensioner_id: Some(pensioner_id),
                pensioner_code: Some(pensioner_code.clone()),
                pensioner_name: Some(pensioner_name.clone()),
                description: format!(
                    "{tickets_count} ticket(s) × {} · {pensioner_name}",
                    format_soles(ticket_unit_cents)
                ),
                amount_cents: ticket_amount_cents,
                operator_name: operator_name.clone(),
                date,
                recorded_at: recorded_at.clone(),
                ticket_count: Some(tickets_count),
                ticket_unit_cents: Some(ticket_unit_cents),
                consumption_cents: (consumption_cents > 0).then_some(consumption_cents),
                resulting_balance_cents: Some(balance_after_cents),
            });

            (Some(session_id), Some(mov_code))
        } else {
            (None, None)
        };

        self.processed_keys.insert(idempotency_key.clone());

        let record = AttendanceRecord {
            id: attendance_id,
            idempotency_key,
            pensioner_id,
            pensioner_code,
            pensioner_name,
            breakfasts,
            lunches,
            items,
            picked_up_by,
            consumption_cents,
            tickets_count,
            ticket_unit_cents,
            ticket_amount_cents,
            balance_before_cents,
            balance_after_cents,
            cash_session_id,
            movement_code,
            source,
            operator_name,
            date,
            recorded_at,
        };
        self.attendances.push(record.clone());
        record
    }

    fn next_cash_movement_code(&mut self) -> String {
        let code = format!("MOV-{:04}", self.next_cash_movement_seq);
        self.next_cash_movement_seq += 1;
        code
    }

    fn next_timestamp(&mut self, date: CalendarDate) -> String {
        let seq = self.next_event_seq;
        self.next_event_seq += 1;
        let minutes = (seq.saturating_sub(1)) % 60;
        let hours = 8 + ((seq.saturating_sub(1)) / 60) % 14;
        format!("{} {hours:02}:{minutes:02} · #{seq:03}", date.format_iso())
    }
}

fn build_audit(
    expected_tickets: u32,
    counted_tickets: u32,
    ticket_unit_cents: i64,
    expected_ticket_amount_cents: i64,
    expected_cash_cents: i64,
    counted_cash_cents: i64,
    audited_at: String,
) -> CashAuditRecord {
    let counted_ticket_amount_cents = i64::from(counted_tickets).saturating_mul(ticket_unit_cents);
    CashAuditRecord {
        expected_tickets,
        counted_tickets,
        ticket_diff: i64::from(counted_tickets) - i64::from(expected_tickets),
        expected_ticket_amount_cents,
        counted_ticket_amount_cents,
        expected_cash_cents,
        counted_cash_cents,
        cash_diff_cents: counted_cash_cents - expected_cash_cents,
        audited_at,
    }
}

/// Parsea un importe en soles (ej. `"150"`, `"45.50"`, `"45,5"`) a céntimos.
pub fn parse_soles_to_cents(raw: &str) -> Option<i64> {
    let trimmed = raw.trim().trim_start_matches("S/").trim();
    if trimmed.is_empty() {
        return Some(0);
    }
    let normalized = trimmed.replace(',', ".");
    if normalized.starts_with('-') {
        return None;
    }
    let mut parts = normalized.split('.');
    let whole_str = parts.next()?;
    let frac_str = parts.next();
    if parts.next().is_some() {
        return None;
    }
    let whole: i64 = if whole_str.is_empty() {
        0
    } else {
        whole_str.parse().ok()?
    };
    let frac: i64 = match frac_str {
        None | Some("") => 0,
        Some(f) if f.len() == 1 => f.parse::<i64>().ok()? * 10,
        Some(f) if f.len() == 2 => f.parse::<i64>().ok()?,
        Some(_) => return None,
    };
    whole.checked_mul(100)?.checked_add(frac)
}

/// Entidad reactiva compartida que expone `DemoLedger` a todas las vistas de GPUI.
pub struct ComedorStore {
    ledger: DemoLedger,
}

impl ComedorStore {
    pub fn new() -> Self {
        Self {
            ledger: DemoLedger::new(),
        }
    }

    pub fn ledger(&self) -> &DemoLedger {
        &self.ledger
    }

    pub fn set_ticket_value_cents(
        &mut self,
        cents: i64,
        cx: &mut Context<Self>,
    ) -> Result<(), CashError> {
        self.ledger.set_ticket_value_cents(cents)?;
        cx.notify();
        Ok(())
    }

    pub fn save_pensioner(
        &mut self,
        id: Option<PensionerId>,
        draft: PensionerDraft,
        cx: &mut Context<Self>,
    ) -> Result<PensionerId, &'static str> {
        let id = self.ledger.save_pensioner(id, draft)?;
        cx.notify();
        Ok(id)
    }

    pub fn set_pensioner_active(&mut self, id: PensionerId, active: bool, cx: &mut Context<Self>) {
        if let Some(pensioner) = self.ledger.pensioners.iter_mut().find(|p| p.id == id) {
            pensioner.active = active;
            cx.notify();
        }
    }

    pub fn confirm_attendance(
        &mut self,
        request: AttendanceRequest,
        cx: &mut Context<Self>,
    ) -> Result<AttendanceRecord, AttendanceError> {
        let record = execute_confirm_attendance(&mut self.ledger, request)?;
        cx.notify();
        Ok(record)
    }

    pub fn set_fingerprint_enrolled(
        &mut self,
        id: PensionerId,
        enrolled: bool,
        cx: &mut Context<Self>,
    ) {
        if self.ledger.set_fingerprint_enrolled(id, enrolled) {
            cx.notify();
        }
    }

    pub fn open_cash(
        &mut self,
        operator: impl Into<String>,
        opening_cash_cents: i64,
        cx: &mut Context<Self>,
    ) -> Result<(), CashError> {
        self.ledger.open_cash(operator, opening_cash_cents)?;
        cx.notify();
        Ok(())
    }

    pub fn audit_cash(
        &mut self,
        counted_tickets: u32,
        counted_cash_cents: i64,
        cx: &mut Context<Self>,
    ) -> Result<CashAuditRecord, CashError> {
        let audit = self
            .ledger
            .audit_cash(counted_tickets, counted_cash_cents)?;
        cx.notify();
        Ok(audit)
    }

    pub fn close_cash(
        &mut self,
        operator: impl Into<String>,
        counted_tickets: u32,
        counted_cash_cents: i64,
        notes: impl Into<String>,
        cx: &mut Context<Self>,
    ) -> Result<(), CashError> {
        self.ledger
            .close_cash(operator, counted_tickets, counted_cash_cents, notes)?;
        cx.notify();
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::{
        BREAKFAST_PRICE_CENTS, CalendarDate, CashError, CashMovementKind,
        DEFAULT_TICKET_VALUE_CENTS, DatePeriodFilter, DemoLedger, LUNCH_PRICE_CENTS,
        parse_soles_to_cents,
    };
    use crate::features::{
        consumptions::{
            confirm_attendance::{AttendanceRequest, confirm_attendance},
            order::{ConsumptionItem, Meal, Menu},
        },
        pensioners::{
            SelectionSource,
            model::{PensionerDraft, PensionerId},
        },
    };

    #[test]
    fn editing_a_profile_preserves_biometric_and_account_state() {
        let mut ledger = DemoLedger::new();
        let id = PensionerId(1001);
        let before = ledger.find_pensioner(id).unwrap().clone();
        let mut draft = PensionerDraft::from(&before);
        draft.full_name = "  Ana Quispe  ".into();
        draft.phone = " 912345678 ".into();

        assert_eq!(ledger.save_pensioner(Some(id), draft), Ok(id));
        let after = ledger.find_pensioner(id).unwrap();
        assert_eq!(after.full_name, "Ana Quispe");
        assert_eq!(after.phone.as_deref(), Some("912345678"));
        assert_eq!(after.balance_cents, before.balance_cents);
        assert_eq!(after.tickets_balance, before.tickets_balance);
        assert_eq!(after.fingerprint_enrolled, before.fingerprint_enrolled);
        assert_eq!(after.active, before.active);
        assert_eq!(after.plan, before.plan);
        assert_eq!(after.notes, before.notes);
    }

    #[test]
    fn creating_profiles_validates_contact_data_and_allocates_a_unique_code() {
        let mut ledger = DemoLedger::new();
        let previous_count = ledger.pensioners().len();
        let draft = |dni: &str, phone: &str| PensionerDraft {
            full_name: "Nuevo pensionista".into(),
            dni: dni.into(),
            phone: phone.into(),
        };

        assert!(ledger.save_pensioner(None, draft("", "ABC1234")).is_err());
        assert!(ledger.save_pensioner(None, draft("72481920", "")).is_err());
        assert_eq!(ledger.pensioners().len(), previous_count);
        let id = ledger.save_pensioner(None, draft("00000000", "")).unwrap();
        let stored = ledger.find_pensioner(id).unwrap();
        assert!(stored.active);
        assert!(!stored.fingerprint_enrolled);
        assert!(stored.temporary_profile);
        assert_eq!(stored.balance_cents, 0);
        assert_eq!(ledger.pensioners().len(), previous_count + 1);
    }

    #[test]
    fn parses_valid_and_rejects_invalid_soles_amounts() {
        assert_eq!(parse_soles_to_cents(""), Some(0));
        assert_eq!(parse_soles_to_cents("0"), Some(0));
        assert_eq!(parse_soles_to_cents("120"), Some(12_000));
        assert_eq!(parse_soles_to_cents("S/ 7.00"), Some(700));
        assert_eq!(parse_soles_to_cents("45.5"), Some(4_550));
        assert_eq!(parse_soles_to_cents("45,75"), Some(4_575));
        assert_eq!(parse_soles_to_cents("-10"), None);
        assert_eq!(parse_soles_to_cents("12.345"), None);
        assert_eq!(parse_soles_to_cents("abc"), None);
    }

    #[test]
    fn cash_lifecycle_tracks_opening_cash_payments_tickets_and_closing_audit() {
        let mut ledger = DemoLedger::new();
        assert!(!ledger.is_cash_open());
        assert_eq!(ledger.audit_cash(0, 0), Err(CashError::NotOpen));
        assert_eq!(
            ledger.set_ticket_value_cents(0),
            Err(CashError::InvalidTicketValue)
        );
        assert_eq!(ledger.ticket_value_cents(), DEFAULT_TICKET_VALUE_CENTS);

        let opened = ledger.open_cash("Operador", 10_000).expect("open cash");
        assert_eq!(opened.opening_cash_cents, 10_000);
        assert_eq!(
            ledger.open_cash("Operador", 5_000),
            Err(CashError::AlreadyOpen)
        );

        // Un pago en efectivo suma al efectivo esperado; un abono por tickets no.
        let cash_mov = ledger
            .record_cash_payment("pay-1", PensionerId(1002), 1_500, "Cajero")
            .expect("cash payment ok");
        assert_eq!(cash_mov.kind, CashMovementKind::CashPayment);
        assert_eq!(ledger.active_cash_inflow_cents(), 1_500);
        assert_eq!(ledger.expected_cash_cents(), 11_500);

        confirm_attendance(
            &mut ledger,
            AttendanceRequest {
                idempotency_key: "tkt-1".into(),
                pensioner_id: PensionerId(1002),
                items: vec![],
                picked_up_by: None,
                tickets_count: 2,
                source: SelectionSource::GlobalSearch,
                operator_name: "Cajero".into(),
                can_register_attendance: true,
            },
        )
        .expect("ticket credit ok");
        assert_eq!(ledger.expected_cash_cents(), 11_500);

        let audit = ledger.audit_cash(2, 11_500).expect("audit open cash");
        assert!(audit.is_balanced());

        let closed = ledger
            .close_cash("Operador", 2, 11_000, "Diferencia de prueba")
            .expect("close cash");
        assert!(!closed.audit.is_balanced());
        assert_eq!(closed.audit.cash_diff_cents, -500);
        assert_eq!(closed.expected_cash_cents, 11_500);
        assert!(!ledger.is_cash_open());
        assert_eq!(ledger.closed_cash_sessions().len(), 1);
        assert_eq!(ledger.cash_movements().len(), 4);
        assert_eq!(ledger.cash_movements()[0].kind, CashMovementKind::Opening);
        assert_eq!(
            ledger.cash_movements()[1].kind,
            CashMovementKind::CashPayment
        );
        assert_eq!(
            ledger.cash_movements()[2].kind,
            CashMovementKind::TicketCredit
        );
        assert_eq!(ledger.cash_movements()[3].kind, CashMovementKind::Closing);
    }

    #[test]
    fn calendar_date_and_period_report_stay_consistent_across_modules() {
        let oct_20 = CalendarDate::parse_iso("2026-10-20").expect("valid iso date");
        let nov_22 = CalendarDate::parse_iso("2026-11-22").expect("valid iso date");
        assert_eq!(oct_20.format_iso(), "2026-10-20");
        assert_eq!(nov_22.sub_days(6).format_iso(), "2026-11-16");
        assert!(CalendarDate::parse_iso("2026-02-29").is_none());
        assert!(CalendarDate::parse_iso("2024-02-29").is_some());

        let mut ledger = DemoLedger::new();
        ledger.set_test_date(oct_20);
        ledger.open_cash("Operador", 5_000).expect("open cash");
        confirm_attendance(
            &mut ledger,
            AttendanceRequest {
                idempotency_key: "hist-1".into(),
                pensioner_id: PensionerId(1001),
                items: vec![ConsumptionItem {
                    meal: Meal::Breakfast,
                    menu: Menu::Economic,
                    portions: 1,
                }],
                picked_up_by: None,
                tickets_count: 1,
                source: SelectionSource::ManualSearch,
                operator_name: "Operador".into(),
                can_register_attendance: true,
            },
        )
        .expect("historical record ok");

        // Caso integral en la fecha actual: consumo con tickets.
        ledger.set_test_date(nov_22);
        let id = PensionerId(1002);
        let balance_before = ledger.find_pensioner(id).unwrap().balance_cents;
        confirm_attendance(
            &mut ledger,
            AttendanceRequest {
                idempotency_key: "today-1".into(),
                pensioner_id: id,
                items: vec![
                    ConsumptionItem {
                        meal: Meal::Breakfast,
                        menu: Menu::Economic,
                        portions: 1,
                    },
                    ConsumptionItem {
                        meal: Meal::Lunch,
                        menu: Menu::Economic,
                        portions: 1,
                    },
                ],
                picked_up_by: None,
                tickets_count: 2,
                source: SelectionSource::GlobalSearch,
                operator_name: "Operador".into(),
                can_register_attendance: true,
            },
        )
        .expect("today record ok");

        let today_report = ledger.report_for_period(DatePeriodFilter::Today);
        assert_eq!(today_report.attendances_count, 1);
        assert_eq!(today_report.breakfasts, 1);
        assert_eq!(today_report.lunches, 1);
        assert_eq!(today_report.rations, 2);
        assert_eq!(
            today_report.consumption_cents,
            BREAKFAST_PRICE_CENTS + LUNCH_PRICE_CENTS
        );
        assert_eq!(today_report.tickets_received, 2);
        assert_eq!(
            today_report.ticket_amount_cents,
            2 * DEFAULT_TICKET_VALUE_CENTS
        );
        assert_eq!(today_report.cash_movements_count, 1);
        assert_eq!(today_report.by_pensioner.len(), 1);
        assert_eq!(today_report.by_pensioner[0].pensioner_id, id);
        assert_eq!(
            ledger.find_pensioner(id).unwrap().balance_cents,
            balance_before - BREAKFAST_PRICE_CENTS - LUNCH_PRICE_CENTS
                + 2 * DEFAULT_TICKET_VALUE_CENTS
        );

        let monthly = ledger.monthly_summary(id);
        assert_eq!(monthly.len(), 1);
        assert_eq!(monthly[0].month_label, "11/2026");
        assert_eq!(monthly[0].tickets_count, 2);
        assert_eq!(monthly[0].tickets_cents, 2 * DEFAULT_TICKET_VALUE_CENTS);
        assert_eq!(
            monthly[0].consumed_cents,
            BREAKFAST_PRICE_CENTS + LUNCH_PRICE_CENTS
        );

        let all_report = ledger.report_for_period(DatePeriodFilter::All);
        assert_eq!(all_report.attendances_count, 2);
        assert_eq!(all_report.tickets_received, 3);

        let custom = ledger.report_for_period(DatePeriodFilter::Custom {
            from: oct_20,
            to: oct_20,
        });
        assert_eq!(custom.attendances_count, 1);
        assert_eq!(custom.tickets_received, 1);
    }
}
