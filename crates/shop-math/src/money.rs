//! Money, held as whole cents.

use std::fmt;

use crate::decimal::{self, ParseError, TooLarge};

/// An amount of money in whole cents. It can be below zero: a loss is one.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Default)]
pub struct Money(i64);

impl Money {
    pub const ZERO: Money = Money(0);
    /// The most a form field accepts: 99,999,999.99.
    pub const MAX_INPUT: Money = Money(9_999_999_999);

    pub const fn from_cents(cents: i64) -> Money {
        Money(cents)
    }

    pub const fn cents(self) -> i64 {
        self.0
    }

    /// Reads what was typed into a money field: `1234.5`, `1,234.50` or
    /// `$1,234.50`. The amount is from 0 to [`Money::MAX_INPUT`] and has at
    /// most two decimal places.
    pub fn parse(text: &str) -> Result<Money, ParseError> {
        let text = text.trim();
        let (below_zero, text) = match text.strip_prefix('-') {
            Some(rest) => (true, rest),
            None => (false, text),
        };
        let text = text.strip_prefix('$').unwrap_or(text).trim_start();
        let cents = decimal::read(text, 2)?;
        if below_zero && cents != 0 {
            return Err(ParseError::BelowZero);
        }
        if cents > Money::MAX_INPUT.0 {
            return Err(ParseError::OutOfRange {
                least: "0".to_owned(),
                most: decimal::write(Money::MAX_INPUT.0.unsigned_abs(), 2, 2, true),
            });
        }
        Ok(Money(cents))
    }

    /// The amount as a form field or a query string holds it: `1234.50`.
    pub fn input(self) -> String {
        let sign = if self.0 < 0 { "-" } else { "" };
        format!(
            "{sign}{}",
            decimal::write(self.0.unsigned_abs(), 2, 2, false)
        )
    }

    pub fn checked_add(self, other: Money) -> Result<Money, TooLarge> {
        self.0.checked_add(other.0).map(Money).ok_or(TooLarge)
    }

    pub fn checked_sub(self, other: Money) -> Result<Money, TooLarge> {
        self.0.checked_sub(other.0).map(Money).ok_or(TooLarge)
    }
}

/// The amount as a page shows it: `$1,234.50`, and `-$1,234.50` for a loss.
impl fmt::Display for Money {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        let sign = if self.0 < 0 { "-" } else { "" };
        let amount = decimal::write(self.0.unsigned_abs(), 2, 2, true);
        write!(formatter, "{sign}${amount}")
    }
}
