use chrono::NaiveDate;
use reqwest::Method;
use serde::{Deserialize, Deserializer, de::Error as _};

use super::{ApiError, Data, DiningApi};
use crate::state::store::CalendarDate;

impl DiningApi {
    pub fn daily_activity(
        &self,
        from: CalendarDate,
        to: CalendarDate,
    ) -> Result<Vec<DailyActivity>, ApiError> {
        self.request::<Data<ReportData>>(
            Method::GET,
            "/api/reports/period",
            &range_params(from, to)
                .each_ref()
                .map(|(key, value)| (*key, value.as_str())),
            None,
        )
        .map(|response| response.data.daily)
    }

    pub fn dashboard(
        &self,
        from: CalendarDate,
        to: CalendarDate,
    ) -> Result<DashboardData, ApiError> {
        self.request::<Data<DashboardData>>(
            Method::GET,
            "/api/dashboard",
            &range_params(from, to)
                .each_ref()
                .map(|(key, value)| (*key, value.as_str())),
            None,
        )
        .map(|response| response.data)
    }
}

fn range_params(from: CalendarDate, to: CalendarDate) -> [(&'static str, String); 2] {
    [("from", from.format_iso()), ("to", to.format_iso())]
}

#[derive(Deserialize)]
struct ReportData {
    daily: Vec<DailyActivity>,
}

#[derive(Clone, Deserialize)]
pub struct DailyActivity {
    #[serde(deserialize_with = "parse_date")]
    pub date: NaiveDate,
    pub breakfasts: i64,
    pub economic_lunches: i64,
    pub special_lunches: i64,
    #[serde(default)]
    pub unclassified_lunches: i64,
    pub attendances: i64,
    pub third_party_pickups: i64,
    pub tickets_count: i64,
}

fn parse_date<'de, D: Deserializer<'de>>(deserializer: D) -> Result<NaiveDate, D::Error> {
    NaiveDate::parse_from_str(&String::deserialize(deserializer)?, "%Y-%m-%d")
        .map_err(D::Error::custom)
}

#[derive(Clone, Deserialize)]
pub struct DashboardData {
    pub consumptions: Option<DashboardConsumptions>,
    pub tickets: Option<DashboardTickets>,
    pub pensioners: Option<DashboardPensioners>,
    pub cash: Option<DashboardCash>,
}

#[derive(Clone, Copy, Deserialize)]
pub struct DashboardConsumptions {
    pub breakfasts: u32,
    pub lunches: u32,
    pub rations: u32,
    pub attendances: usize,
}

#[derive(Clone, Copy, Deserialize)]
pub struct DashboardTickets {
    pub tickets_count: u32,
    pub ticket_amount_cents: i64,
}

#[derive(Clone, Copy, Deserialize)]
pub struct DashboardPensioners {
    pub registered: usize,
    pub active: usize,
    pub fingerprint_enrolled: usize,
    pub credit_balance_cents: i64,
    pub pending_debt_cents: i64,
}

#[derive(Clone, Deserialize)]
pub struct DashboardCash {
    pub status: String,
    pub expected_cash_cents: Option<i64>,
}
