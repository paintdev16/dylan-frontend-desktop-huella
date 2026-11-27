#[cfg(test)]
use crate::state::store::{BREAKFAST_PRICE_CENTS, LUNCH_PRICE_CENTS};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Meal {
    Breakfast,
    Lunch,
}

impl Meal {
    pub const ALL: [Self; 2] = [Self::Breakfast, Self::Lunch];

    pub const fn label(self) -> &'static str {
        match self {
            Self::Breakfast => "Desayuno",
            Self::Lunch => "Almuerzo",
        }
    }

    #[cfg(test)]
    pub const fn economic_price(self) -> i64 {
        match self {
            Self::Breakfast => BREAKFAST_PRICE_CENTS,
            Self::Lunch => LUNCH_PRICE_CENTS,
        }
    }

    pub const fn for_hour(hour: u32) -> Self {
        match hour {
            0..=10 => Self::Breakfast,
            _ => Self::Lunch,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Menu {
    Economic,
    Special { unit_cents: i64 },
}

impl Menu {
    pub const fn label(self) -> &'static str {
        match self {
            Self::Economic => "Económico",
            Self::Special { .. } => "Especial",
        }
    }

    #[cfg(test)]
    pub const fn unit_cents(self, meal: Meal) -> i64 {
        match self {
            Self::Economic => meal.economic_price(),
            Self::Special { unit_cents } => unit_cents,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ConsumptionItem {
    pub meal: Meal,
    pub menu: Menu,
    pub portions: u32,
}

pub fn add_to_cart(cart: &mut Vec<ConsumptionItem>, item: ConsumptionItem) -> Option<()> {
    if let Some(line) = cart
        .iter_mut()
        .find(|line| line.meal == item.meal && line.menu == item.menu)
    {
        line.portions = line.portions.checked_add(item.portions)?;
    } else {
        cart.push(item);
    }
    Some(())
}

pub fn parse_ticket_count(text: &str) -> Option<u32> {
    let text = text.trim();
    if !text.is_empty() && text.bytes().all(|digit| digit.is_ascii_digit()) {
        text.parse().ok()
    } else {
        None
    }
}
