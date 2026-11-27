use std::collections::BTreeMap;

use reqwest::Method;
use serde::Deserialize;

use super::{ApiError, AttendanceData, Data, DiningApi, Page, PensionerData};
use crate::{
    features::pensioners::model::{Pensioner, PensionerId},
    state::store::{AttendanceRecord, CashMovementKind, MonthlyPensionerSummary},
};

pub struct PensionerHistory {
    pub pensioner: Pensioner,
    pub consumptions: Option<Vec<AttendanceRecord>>,
    pub payments: Option<Vec<PensionerPayment>>,
}

impl DiningApi {
    pub fn identified_pensioner(&self, id: PensionerId) -> Result<Pensioner, ApiError> {
        let path = format!("/api/identification/pensioners/{}", id.0);
        self.request::<Data<PensionerData>>(Method::GET, &path, &[], None)
            .map(|response| response.data.into())
    }

    pub fn pensioner_history(&self, id: PensionerId) -> Result<PensionerHistory, ApiError> {
        let path = format!("/api/pensioners/{}", id.0);
        let read_page = |page: u32| {
            let page = page.to_string();
            self.request::<Data<HistoryData>>(
                Method::GET,
                &path,
                &[
                    ("consumptions_page", &page),
                    ("payments_page", &page),
                    ("per_page", "100"),
                ],
                None,
            )
            .map(|response| response.data)
        };
        let first = read_page(1)?;
        let last = first
            .consumptions
            .as_ref()
            .map_or(1, |page| page.meta.last_page)
            .max(
                first
                    .payments
                    .as_ref()
                    .map_or(1, |page| page.meta.last_page),
            );
        let mut consumptions = first.consumptions.map(|page| page.data);
        let mut payments = first.payments.map(|page| page.data);
        for page in 2..=last {
            let next = read_page(page)?;
            if let (Some(records), Some(next)) = (&mut consumptions, next.consumptions) {
                records.extend(next.data);
            }
            if let (Some(records), Some(next)) = (&mut payments, next.payments) {
                records.extend(next.data);
            }
        }
        if let Some(records) = &mut consumptions {
            records.sort_by_key(|row| std::cmp::Reverse(row.id));
            records.dedup_by_key(|row| row.id);
        }
        if let Some(records) = &mut payments {
            records.sort_by_key(|row| std::cmp::Reverse(row.id));
            records.dedup_by_key(|row| row.id);
        }
        Ok(PensionerHistory {
            pensioner: first.pensioner.into(),
            consumptions: consumptions
                .map(|records| records.into_iter().map(TryInto::try_into).collect())
                .transpose()?,
            payments,
        })
    }
}

#[derive(Deserialize)]
struct HistoryData {
    pensioner: PensionerData,
    consumptions: Option<Page<AttendanceData>>,
    payments: Option<Page<PensionerPayment>>,
}

#[derive(Deserialize)]
pub struct PensionerPayment {
    id: u64,
    #[serde(rename = "type")]
    pub kind: CashMovementKind,
    pub code: String,
    pub service_date: String,
    pub amount_cents: i64,
    pub ticket_count: Option<u32>,
    pub ticket_unit_cents: Option<i64>,
}

impl PensionerHistory {
    pub fn monthly(&self) -> Vec<MonthlyPensionerSummary> {
        let mut months = BTreeMap::new();
        for item in self.consumptions.as_deref().unwrap_or_default() {
            let month = (item.date.year, item.date.month);
            let row = months.entry(month).or_insert_with(|| month_summary(month));
            row.consumed_cents = row.consumed_cents.saturating_add(item.consumption_cents);
        }
        for item in self.payments.as_deref().unwrap_or_default() {
            let Ok(date) = super::read_date(&item.service_date) else {
                continue;
            };
            let month = (date.year, date.month);
            let row = months.entry(month).or_insert_with(|| month_summary(month));
            match item.kind {
                CashMovementKind::TicketCredit => {
                    row.tickets_count = row
                        .tickets_count
                        .saturating_add(item.ticket_count.unwrap_or_default());
                    row.tickets_cents = row.tickets_cents.saturating_add(item.amount_cents);
                }
                CashMovementKind::CashPayment => {
                    row.cash_paid_cents = row.cash_paid_cents.saturating_add(item.amount_cents)
                }
                _ => {}
            }
        }
        months
            .into_values()
            .rev()
            .map(|mut row| {
                row.net_balance_cents = row
                    .tickets_cents
                    .saturating_add(row.cash_paid_cents)
                    .saturating_sub(row.consumed_cents);
                row
            })
            .collect()
    }
}

fn month_summary((year, month): (i32, u8)) -> MonthlyPensionerSummary {
    MonthlyPensionerSummary {
        month_label: format!("{month:02}/{year}"),
        consumed_cents: 0,
        cash_paid_cents: 0,
        tickets_count: 0,
        tickets_cents: 0,
        net_balance_cents: 0,
    }
}
