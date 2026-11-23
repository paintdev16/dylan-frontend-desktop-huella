use std::fmt;

/// Identificador canónico único compartido entre búsqueda, ficha,
/// identificación biométrica y futuros flujos de Consumos y Caja.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct PensionerId(pub u32);

impl PensionerId {
    pub fn code(self) -> String {
        format!("P-{}", self.0)
    }
}

impl fmt::Display for PensionerId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "P-{}", self.0)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Pensioner {
    pub id: PensionerId,
    pub dni: String,
    pub full_name: String,
    pub phone: Option<String>,
    pub plan: String,
    pub active: bool,
    pub fingerprint_enrolled: bool,
    pub temporary_profile: bool,
    /// Saldo en céntimos: positivo = saldo a favor, negativo = deuda pendiente.
    pub balance_cents: i64,
    pub tickets_balance: u32,
    pub notes: String,
}

#[derive(Debug, Default)]
pub struct PensionerDraft {
    pub full_name: String,
    pub dni: String,
    pub phone: String,
}

impl From<&Pensioner> for PensionerDraft {
    fn from(pensioner: &Pensioner) -> Self {
        Self {
            full_name: pensioner.full_name.clone(),
            dni: pensioner.dni.clone(),
            phone: pensioner.phone.clone().unwrap_or_default(),
        }
    }
}

impl PensionerDraft {
    pub fn validated(self) -> Result<Self, &'static str> {
        let draft = Self {
            full_name: self.full_name.trim().to_owned(),
            dni: self.dni.trim().to_owned(),
            phone: self.phone.trim().to_owned(),
        };
        if draft.full_name.is_empty() {
            return Err("Ingresa el nombre completo.");
        }
        if !draft.phone.is_empty()
            && (!(7..=15).contains(&draft.phone.len())
                || !draft.phone.bytes().all(|digit| digit.is_ascii_digit()))
        {
            return Err("El celular debe tener entre 7 y 15 dígitos.");
        }
        Ok(draft)
    }
}

impl Pensioner {
    pub fn code(&self) -> String {
        self.id.code()
    }

    pub fn credit_cents(&self) -> i64 {
        self.balance_cents.max(0)
    }

    pub fn debt_cents(&self) -> i64 {
        self.balance_cents.min(0).saturating_abs()
    }

    pub fn can_enroll_fingerprint(&self) -> bool {
        self.active && !self.fingerprint_enrolled
    }

    pub(super) fn search_text(&self) -> String {
        normalize_text(&format!(
            "{} {} {} {} {}",
            self.full_name,
            self.dni,
            self.code(),
            self.plan,
            self.phone.as_deref().unwrap_or_default()
        ))
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum PensionerFilter {
    #[default]
    All,
    Active,
    Inactive,
    Enrolled,
}

impl PensionerFilter {
    pub const ALL: [Self; 4] = [Self::All, Self::Active, Self::Inactive, Self::Enrolled];

    pub const fn label(self) -> &'static str {
        match self {
            Self::All => "Todos",
            Self::Active => "Activos",
            Self::Inactive => "Inactivos",
            Self::Enrolled => "Con huella",
        }
    }
}

/// Catálogo de pensionistas de demostración para la Etapa 2.
pub fn demo_pensioners() -> Vec<Pensioner> {
    vec![
        Pensioner {
            id: PensionerId(1001),
            dni: "72481920".into(),
            full_name: "Ana Lucía Quispe Ramos".into(),
            phone: Some("900000001".into()),
            plan: "Desayuno y almuerzo".into(),
            active: true,
            fingerprint_enrolled: true,
            temporary_profile: false,
            balance_cents: 4500,
            tickets_balance: 8,
            notes: "Turno habitual de almuerzo 12:30.".into(),
        },
        Pensioner {
            id: PensionerId(1002),
            dni: "45190382".into(),
            full_name: "Carlos Eduardo Paredes Silva".into(),
            phone: None,
            plan: "Almuerzo diario".into(),
            active: true,
            fingerprint_enrolled: true,
            temporary_profile: false,
            balance_cents: -1850,
            tickets_balance: 2,
            notes: "Regulariza deuda al cierre de semana.".into(),
        },
        Pensioner {
            id: PensionerId(1003),
            dni: "70831245".into(),
            full_name: "María Fernanda Torres Vega".into(),
            phone: None,
            plan: "Desayuno y almuerzo".into(),
            active: true,
            fingerprint_enrolled: true,
            temporary_profile: false,
            balance_cents: 12000,
            tickets_balance: 15,
            notes: "Sin observaciones.".into(),
        },
        Pensioner {
            id: PensionerId(1004),
            dni: "43829104".into(),
            full_name: "Jorge Luis Mendoza Castillo".into(),
            phone: None,
            plan: "Solo desayuno".into(),
            active: true,
            fingerprint_enrolled: false,
            temporary_profile: false,
            balance_cents: 0,
            tickets_balance: 4,
            notes: "Pendiente de enrolamiento biométrico.".into(),
        },
        Pensioner {
            id: PensionerId(1005),
            dni: "74910238".into(),
            full_name: "Rosa Elena Huamán Flores".into(),
            phone: None,
            plan: "Almuerzo diario".into(),
            active: true,
            fingerprint_enrolled: true,
            temporary_profile: false,
            balance_cents: -900,
            tickets_balance: 0,
            notes: "Prefiere menú bajo en sal.".into(),
        },
        Pensioner {
            id: PensionerId(1006),
            dni: "41029384".into(),
            full_name: "Luis Alberto Rojas Chávez".into(),
            phone: None,
            plan: "Almuerzo diario".into(),
            active: false,
            fingerprint_enrolled: false,
            temporary_profile: false,
            balance_cents: 0,
            tickets_balance: 0,
            notes: "Pensión pausada temporalmente.".into(),
        },
    ]
}

pub fn find_by_id(pensioners: &[Pensioner], id: PensionerId) -> Option<&Pensioner> {
    pensioners.iter().find(|p| p.id == id)
}

pub fn search_pensioners<'a>(
    pensioners: &'a [Pensioner],
    query: &str,
    filter: PensionerFilter,
) -> Vec<&'a Pensioner> {
    let needle = normalize_text(query);
    pensioners
        .iter()
        .filter(|p| match filter {
            PensionerFilter::All => true,
            PensionerFilter::Active => p.active,
            PensionerFilter::Inactive => !p.active,
            PensionerFilter::Enrolled => p.fingerprint_enrolled,
        })
        .filter(|p| needle.is_empty() || matches_search_terms(&p.search_text(), &needle))
        .collect()
}

pub fn format_soles(cents: i64) -> String {
    let abs = cents.unsigned_abs();
    let soles = abs / 100;
    let rem = abs % 100;
    if cents < 0 {
        format!("-S/ {soles}.{rem:02}")
    } else {
        format!("S/ {soles}.{rem:02}")
    }
}

pub(super) fn matches_search_terms(search_text: &str, normalized_query: &str) -> bool {
    normalized_query
        .split_whitespace()
        .all(|term| search_text.contains(term))
}

pub(super) fn normalize_text(input: &str) -> String {
    input
        .trim()
        .to_lowercase()
        .chars()
        .map(|ch| match ch {
            'á' | 'à' | 'ä' => 'a',
            'é' | 'è' | 'ë' => 'e',
            'í' | 'ì' | 'ï' => 'i',
            'ó' | 'ò' | 'ö' => 'o',
            'ú' | 'ù' | 'ü' => 'u',
            _ => ch,
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::{
        PensionerFilter, PensionerId, demo_pensioners, find_by_id, format_soles, search_pensioners,
    };

    #[test]
    fn search_finds_same_canonical_pensioner_by_name_dni_code_and_phone() {
        let catalog = demo_pensioners();

        let by_name = search_pensioners(&catalog, "ana lucia", PensionerFilter::All);
        let by_dni = search_pensioners(&catalog, "72481920", PensionerFilter::All);
        let by_code = search_pensioners(&catalog, "p-1001", PensionerFilter::All);
        let by_phone = search_pensioners(&catalog, "900000001", PensionerFilter::All);

        assert_eq!(by_name.len(), 1);
        assert_eq!(by_dni.len(), 1);
        assert_eq!(by_code.len(), 1);
        assert_eq!(by_name[0].id, PensionerId(1001));
        assert_eq!(by_dni[0].id, PensionerId(1001));
        assert_eq!(by_code[0].id, PensionerId(1001));
        assert_eq!(by_phone.len(), 1);
        assert_eq!(by_phone[0].id, PensionerId(1001));
        assert_eq!(
            find_by_id(&catalog, PensionerId(1001)).map(|p| p.id),
            Some(PensionerId(1001))
        );
    }

    #[test]
    fn filters_active_inactive_and_enrolled_pensioners_accurately() {
        let catalog = demo_pensioners();

        let active = search_pensioners(&catalog, "", PensionerFilter::Active);
        assert!(active.iter().all(|p| p.active));
        assert_eq!(active.len(), 5);

        let inactive = search_pensioners(&catalog, "", PensionerFilter::Inactive);
        assert_eq!(inactive.len(), 1);
        assert!(!inactive[0].active);

        let enrolled = search_pensioners(&catalog, "", PensionerFilter::Enrolled);
        assert!(enrolled.iter().all(|p| p.fingerprint_enrolled));
        assert_eq!(enrolled.len(), 4);
    }

    #[test]
    fn formats_credit_zero_and_debt_amounts() {
        assert_eq!(format_soles(4500), "S/ 45.00");
        assert_eq!(format_soles(0), "S/ 0.00");
        assert_eq!(format_soles(-1850), "-S/ 18.50");
    }
}
