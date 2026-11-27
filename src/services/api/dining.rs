use reqwest::Method;
use serde::{Deserialize, de::DeserializeOwned};
use serde_json::{Value, json};

mod cash;
mod pensioners;
mod reports;
pub use cash::CashSnapshot;
pub use pensioners::PensionerHistory;
pub use reports::{DailyActivity, DashboardData};

use super::{ApiClient, ApiError};
use crate::{
    features::{
        consumptions::{
            confirm_attendance::AttendanceRequest,
            order::{ConsumptionItem, Meal, Menu},
        },
        pensioners::{
            SelectionSource,
            model::{Pensioner, PensionerDraft, PensionerId},
        },
    },
    state::store::{AttendanceRecord, CalendarDate},
};

#[derive(Clone)]
pub struct DiningApi {
    client: ApiClient,
    token: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct DiningVersions {
    pub dashboard: String,
    pub pensioners: Option<u64>,
    pub attendances: Option<u64>,
    pub cash: Option<u64>,
    pub reports: Option<u64>,
}

impl DiningApi {
    pub fn new(base_url: &str, token: &str) -> Result<Self, ApiError> {
        Ok(Self {
            client: ApiClient::new(base_url)?,
            token: token.to_owned(),
        })
    }

    pub fn matches_token(&self, token: &str) -> bool {
        self.token == token
    }

    pub fn versions(&self) -> Result<DiningVersions, ApiError> {
        self.resource(Method::GET, "/api/sync/versions", None)
    }

    fn request<T: DeserializeOwned>(
        &self,
        method: Method,
        path: &str,
        query: &[(&str, &str)],
        body: Option<Value>,
    ) -> Result<T, ApiError> {
        self.client
            .authenticated_json(method, path, &self.token, query, body)
    }

    fn resource<T: DeserializeOwned>(
        &self,
        method: Method,
        path: &str,
        body: Option<Value>,
    ) -> Result<T, ApiError> {
        self.request(method, path, &[], body)
            .map(|response: Data<T>| response.data)
    }

    fn pages<T: DeserializeOwned>(
        &self,
        path: &str,
        query: &[(&str, &str)],
    ) -> Result<Vec<T>, ApiError> {
        let mut records = Vec::new();
        for page in 1.. {
            let number = page.to_string();
            let mut params = query.to_vec();
            params.extend([("per_page", "100"), ("page", &number)]);
            let response: Page<T> = self.request(Method::GET, path, &params, None)?;
            records.extend(response.data);
            if page >= response.meta.last_page {
                return Ok(records);
            }
        }
        unreachable!()
    }

    pub fn pensioners(&self, directory: bool) -> Result<Vec<Pensioner>, ApiError> {
        let purpose = if directory { "directory" } else { "attendance" };
        self.pages::<PensionerData>("/api/pensioners", &[("purpose", purpose)])
            .map(|rows| rows.into_iter().map(Into::into).collect())
    }

    pub fn search_pensioners(
        &self,
        query: &str,
        directory: bool,
    ) -> Result<Vec<Pensioner>, ApiError> {
        let purpose = if directory { "directory" } else { "attendance" };
        let response: Page<PensionerData> = self.request(
            Method::GET,
            "/api/pensioners",
            &[
                ("purpose", purpose),
                ("q", query),
                ("filter", "active"),
                ("per_page", "50"),
            ],
            None,
        )?;
        Ok(response.data.into_iter().map(Into::into).collect())
    }

    pub fn save_pensioner(
        &self,
        id: Option<PensionerId>,
        draft: &PensionerDraft,
    ) -> Result<Pensioner, ApiError> {
        let (method, path) = id.map_or((Method::POST, "/api/pensioners".to_owned()), |id| {
            (Method::PATCH, format!("/api/pensioners/{}", id.0))
        });
        self.resource::<PensionerData>(
            method,
            &path,
            Some(json!({
                "full_name": draft.full_name,
                "dni": optional_text(&draft.dni),
                "phone": optional_text(&draft.phone),
            })),
        )
        .map(Into::into)
    }

    pub fn set_pensioner_active(
        &self,
        id: PensionerId,
        active: bool,
    ) -> Result<Pensioner, ApiError> {
        self.resource::<PensionerData>(
            Method::PATCH,
            &format!("/api/pensioners/{}/status", id.0),
            Some(json!({"active": active})),
        )
        .map(Into::into)
    }

    pub fn record_enrollment(&self, id: PensionerId) -> Result<Pensioner, ApiError> {
        self.resource::<PensionerData>(
            Method::PATCH,
            &format!("/api/pensioners/{}/fingerprint", id.0),
            Some(json!({"fingerprint_enrolled": true})),
        )
        .map(Into::into)
    }

    pub fn attendances(&self) -> Result<Vec<AttendanceRecord>, ApiError> {
        self.pages::<AttendanceData>("/api/attendances", &[])?
            .into_iter()
            .map(TryInto::try_into)
            .collect()
    }

    pub fn attendance_context(&self, id: PensionerId) -> Result<AttendanceContext, ApiError> {
        let response: Data<AttendanceContextData> = self.request(
            Method::GET,
            "/api/attendance/context",
            &[("pensioner_id", &id.0.to_string())],
            None,
        )?;
        let row = response.data;
        let mut pensioner: Pensioner = row.pensioner.into();
        pensioner.balance_cents = row.balance_cents;
        Ok(AttendanceContext {
            pensioner,
            cash_open: row.cash_open,
            ticket_value_cents: row.ticket_value_cents,
            breakfast_price_cents: row.breakfast_price_cents,
            lunch_price_cents: row.lunch_price_cents,
        })
    }

    pub fn confirm_attendance(
        &self,
        draft: &AttendanceRequest,
    ) -> Result<AttendanceRecord, ApiError> {
        let result = self
            .resource::<AttendanceData>(
                Method::POST,
                "/api/attendances",
                Some(attendance_payload(draft)),
            )
            .and_then(TryInto::try_into);
        match result {
            Err(original) if confirmation_uncertain(&original) => {
                let path = format!("/api/attendances/by-key/{}", draft.idempotency_key);
                match self
                    .resource::<AttendanceData>(Method::GET, &path, None)
                    .and_then(TryInto::try_into)
                {
                    Ok(record) => Ok(record),
                    Err(err) if err.is_unauthorized() => Err(err),
                    Err(_) => Err(original),
                }
            }
            result => result,
        }
    }
}

pub fn confirmation_uncertain(error: &ApiError) -> bool {
    matches!(
        error,
        ApiError::Network(_) | ApiError::Server(500..=599, _) | ApiError::Decode(_)
    )
}

fn optional_text(value: &str) -> Option<&str> {
    (!value.trim().is_empty()).then(|| value.trim())
}

fn read_date(value: &str) -> Result<CalendarDate, ApiError> {
    CalendarDate::parse_iso(value)
        .ok_or_else(|| ApiError::Decode("El servidor devolvió una fecha inválida.".into()))
}

fn local_timestamp(value: String) -> String {
    let lima = chrono::FixedOffset::west_opt(5 * 3600).expect("valid Lima offset");
    chrono::DateTime::parse_from_rfc3339(&value)
        .map(|date| {
            date.with_timezone(&lima)
                .format("%Y-%m-%d %H:%M:%S")
                .to_string()
        })
        .unwrap_or(value)
}

fn attendance_payload(draft: &AttendanceRequest) -> Value {
    let source = match draft.source {
        SelectionSource::GlobalSearch => "global_search",
        SelectionSource::ManualSearch => "manual_search",
        SelectionSource::BiometricMatch => "biometric_match",
    };
    let items: Vec<_> = draft.items.iter().map(|item| {
        let meal = match item.meal { Meal::Breakfast => "breakfast", Meal::Lunch => "lunch" };
        match item.menu {
            Menu::Economic => json!({ "meal": meal, "menu_type": "economic", "portions": item.portions }),
            Menu::Special { unit_cents } => json!({ "meal": meal, "menu_type": "special", "portions": item.portions, "unit_price_cents": unit_cents }),
        }
    }).collect();
    json!({
        "idempotency_key": draft.idempotency_key,
        "pensioner_id": draft.pensioner_id.0,
        "source": source,
        "items": items,
        "picked_up_by": draft.picked_up_by,
        "tickets_count": draft.tickets_count,
    })
}

#[derive(Deserialize)]
struct Data<T> {
    data: T,
}

#[derive(Deserialize)]
struct Page<T> {
    data: Vec<T>,
    meta: PageMeta,
}

#[derive(Deserialize)]
struct PageMeta {
    last_page: u32,
}

pub struct AttendanceContext {
    pub pensioner: Pensioner,
    pub cash_open: bool,
    pub ticket_value_cents: i64,
    breakfast_price_cents: i64,
    lunch_price_cents: i64,
}

impl AttendanceContext {
    pub fn economic_price(&self, meal: Meal) -> i64 {
        match meal {
            Meal::Breakfast => self.breakfast_price_cents,
            Meal::Lunch => self.lunch_price_cents,
        }
    }
}

#[derive(Deserialize)]
struct AttendanceContextData {
    pensioner: PensionerData,
    cash_open: bool,
    balance_cents: i64,
    ticket_value_cents: i64,
    breakfast_price_cents: i64,
    lunch_price_cents: i64,
}

#[derive(Deserialize)]
pub struct PensionerData {
    id: u32,
    full_name: String,
    dni: Option<String>,
    phone: Option<String>,
    active: bool,
    fingerprint_enrolled: bool,
    #[serde(default)]
    temporary_profile: bool,
    #[serde(default)]
    balance_cents: i64,
    #[serde(default)]
    tickets_balance: u32,
}

impl From<PensionerData> for Pensioner {
    fn from(row: PensionerData) -> Self {
        Self {
            id: PensionerId(row.id),
            full_name: row.full_name,
            dni: row.dni.unwrap_or_default(),
            phone: row.phone,
            active: row.active,
            fingerprint_enrolled: row.fingerprint_enrolled,
            temporary_profile: row.temporary_profile,
            balance_cents: row.balance_cents,
            tickets_balance: row.tickets_balance,
        }
    }
}

#[derive(Deserialize)]
#[serde(rename_all = "snake_case")]
enum ApiSource {
    GlobalSearch,
    ManualSearch,
    BiometricMatch,
}

#[derive(Deserialize)]
#[serde(rename_all = "snake_case")]
enum ApiMeal {
    Breakfast,
    Lunch,
}

#[derive(Deserialize)]
#[serde(rename_all = "snake_case")]
enum ApiMenu {
    Economic,
    Special,
}

#[derive(Deserialize)]
struct AttendanceItemData {
    meal: ApiMeal,
    menu_type: ApiMenu,
    portions: u32,
    unit_price_cents: i64,
}

#[derive(Deserialize)]
pub struct AttendanceData {
    id: u64,
    idempotency_key: Option<String>,
    pensioner_id: u32,
    pensioner_code: String,
    pensioner_name: String,
    #[serde(default)]
    breakfasts: u32,
    #[serde(default)]
    lunches: u32,
    #[serde(default)]
    items: Vec<AttendanceItemData>,
    picked_up_by: Option<String>,
    consumption_cents: Option<i64>,
    tickets_count: Option<u32>,
    #[serde(default)]
    ticket_unit_cents: i64,
    #[serde(default)]
    ticket_amount_cents: i64,
    balance_before_cents: Option<i64>,
    balance_after_cents: Option<i64>,
    cash_session_id: Option<u64>,
    movement_code: Option<String>,
    source: ApiSource,
    operator_name: String,
    service_date: String,
    recorded_at: String,
}

impl TryFrom<AttendanceData> for AttendanceRecord {
    type Error = ApiError;

    fn try_from(row: AttendanceData) -> Result<Self, Self::Error> {
        let date = CalendarDate::parse_iso(&row.service_date)
            .ok_or_else(|| ApiError::Decode("La atención tiene una fecha inválida.".into()))?;
        Ok(Self {
            id: row.id,
            idempotency_key: row.idempotency_key.unwrap_or_default(),
            pensioner_id: PensionerId(row.pensioner_id),
            pensioner_code: row.pensioner_code,
            pensioner_name: row.pensioner_name,
            breakfasts: row.breakfasts,
            lunches: row.lunches,
            item_prices: row.items.iter().map(|item| item.unit_price_cents).collect(),
            has_consumptions: row.consumption_cents.is_some(),
            has_payments: row.tickets_count.is_some(),
            has_balance: row.balance_after_cents.is_some() && row.balance_before_cents.is_some(),
            items: row
                .items
                .into_iter()
                .map(|item| ConsumptionItem {
                    meal: match item.meal {
                        ApiMeal::Breakfast => Meal::Breakfast,
                        ApiMeal::Lunch => Meal::Lunch,
                    },
                    menu: match item.menu_type {
                        ApiMenu::Economic => Menu::Economic,
                        ApiMenu::Special => Menu::Special {
                            unit_cents: item.unit_price_cents,
                        },
                    },
                    portions: item.portions,
                })
                .collect(),
            picked_up_by: row.picked_up_by,
            consumption_cents: row.consumption_cents.unwrap_or_default(),
            tickets_count: row.tickets_count.unwrap_or_default(),
            ticket_unit_cents: row.ticket_unit_cents,
            ticket_amount_cents: row.ticket_amount_cents,
            balance_before_cents: row.balance_before_cents.unwrap_or_default(),
            balance_after_cents: row.balance_after_cents.unwrap_or_default(),
            cash_session_id: row.cash_session_id,
            movement_code: row.movement_code,
            operator_name: row.operator_name,
            date,
            recorded_at: local_timestamp(row.recorded_at),
            source: match row.source {
                ApiSource::GlobalSearch => SelectionSource::GlobalSearch,
                ApiSource::ManualSearch => SelectionSource::ManualSearch,
                ApiSource::BiometricMatch => SelectionSource::BiometricMatch,
            },
        })
    }
}

#[cfg(test)]
mod tests {
    use std::{
        io::{BufRead, BufReader, Read, Write},
        net::{TcpListener, TcpStream},
        thread,
        time::Duration,
    };

    use serde_json::{Value, json};

    use super::{AttendanceData, AttendanceRecord, DiningApi};
    use crate::features::{
        consumptions::{
            confirm_attendance::AttendanceRequest,
            order::{ConsumptionItem, Meal, Menu},
        },
        pensioners::{SelectionSource, model::PensionerId},
    };

    const KEY: &str = "6d90b69e-04d9-4240-89e5-9b096764fa5c";
    const TOKEN: &str = "12|test-token";

    fn request(stream: &mut TcpStream) -> (String, String) {
        stream
            .set_read_timeout(Some(Duration::from_secs(3)))
            .unwrap();
        let mut reader = BufReader::new(stream);
        let mut line = String::new();
        reader.read_line(&mut line).unwrap();
        let first_line = line.trim().to_owned();
        let mut content_length = 0;
        let mut has_bearer = false;
        loop {
            line.clear();
            assert_ne!(reader.read_line(&mut line).unwrap(), 0);
            if line == "\r\n" {
                break;
            }
            let (name, value) = line.split_once(':').unwrap();
            if name.eq_ignore_ascii_case("content-length") {
                content_length = value.trim().parse().unwrap();
            }
            if name.eq_ignore_ascii_case("authorization") {
                has_bearer = value.trim() == format!("Bearer {TOKEN}");
            }
        }
        assert!(has_bearer);
        let mut body = vec![0; content_length];
        reader.read_exact(&mut body).unwrap();
        (first_line, String::from_utf8(body).unwrap())
    }

    fn respond(stream: &mut TcpStream, status: &str, body: Value) {
        let body = body.to_string();
        write!(stream, "HTTP/1.1 {status}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}", body.len()).unwrap();
        stream.flush().unwrap();
    }

    #[test]
    fn recovers_confirmation_using_the_same_uuid_and_preserves_applied_prices() {
        let listener = TcpListener::bind(("127.0.0.1", 0)).unwrap();
        let address = listener.local_addr().unwrap();
        let server = thread::spawn(move || {
            let (mut stream, _) = listener.accept().unwrap();
            let (line, body) = request(&mut stream);
            assert_eq!(line, "POST /api/attendances HTTP/1.1");
            let body: Value = serde_json::from_str(&body).unwrap();
            assert_eq!(body["idempotency_key"], KEY);
            assert_eq!(body["source"], "global_search");
            assert_eq!(
                body["items"][0],
                json!({"meal": "lunch", "menu_type": "economic", "portions": 1})
            );
            assert_eq!(body["items"][1]["unit_price_cents"], 1200);
            assert!(body.get("operator_name").is_none());
            respond(
                &mut stream,
                "503 Service Unavailable",
                json!({"message": "Respuesta interrumpida"}),
            );
            drop(stream);

            let (mut stream, _) = listener.accept().unwrap();
            assert_eq!(
                request(&mut stream).0,
                format!("GET /api/attendances/by-key/{KEY} HTTP/1.1")
            );
            respond(
                &mut stream,
                "200 OK",
                json!({"data": {
                    "id": 42, "idempotency_key": KEY, "pensioner_id": 7, "pensioner_code": "P-7", "pensioner_name": "Rosa",
                    "operator_name": "Caja", "source": "global_search", "service_date": "2026-11-26",
                    "recorded_at": "2026-11-26T16:00:00.000Z", "consumption_cents": 2050, "lunches": 2,
                    "balance_before_cents": 0, "balance_after_cents": -2050, "tickets_count": 0,
                    "items": [{"meal": "lunch", "menu_type": "economic", "portions": 1, "unit_price_cents": 850}]
                }}),
            );
        });
        let api = DiningApi::new(&format!("http://{address}/api"), TOKEN).unwrap();
        let record = api
            .confirm_attendance(&AttendanceRequest {
                idempotency_key: KEY.into(),
                pensioner_id: PensionerId(7),
                tickets_count: 0,
                picked_up_by: None,
                source: SelectionSource::GlobalSearch,
                items: vec![
                    ConsumptionItem {
                        meal: Meal::Lunch,
                        menu: Menu::Economic,
                        portions: 1,
                    },
                    ConsumptionItem {
                        meal: Meal::Lunch,
                        menu: Menu::Special { unit_cents: 1200 },
                        portions: 1,
                    },
                ],
                operator_name: "ignored by API".into(),
                can_register_attendance: true,
            })
            .unwrap();
        server.join().unwrap();
        assert_eq!(record.id, 42);
        assert_eq!(record.item_prices, vec![850]);
        assert_eq!(record.balance_after_cents, -2050);
    }

    #[test]
    fn missing_payment_details_are_not_represented_as_visible_zeroes() {
        let row: AttendanceData = serde_json::from_value(json!({
            "id": 42, "pensioner_id": 7, "pensioner_code": "P-7", "pensioner_name": "Rosa",
            "operator_name": "Caja", "source": "global_search", "service_date": "2026-11-26",
            "recorded_at": "2026-11-26T16:00:00.000Z", "consumption_cents": 850, "lunches": 1
        }))
        .unwrap();
        let record = AttendanceRecord::try_from(row).unwrap();
        assert!(record.has_consumptions);
        assert!(!record.has_payments);
        assert!(!record.has_balance);
    }
}
