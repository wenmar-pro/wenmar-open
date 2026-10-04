//! Percentages, held as whole thousandths of a percent.

use std::fmt;

use crate::decimal::{self, ParseError, TooLarge};
use crate::money::Money;

/// A percentage in whole thousandths of a percent, so 9.975% is exact. It
/// can be below zero: the gross profit of a line sold at a loss is.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Default)]
pub struct Percent(i64);

impl Percent {
    pub const ZERO: Percent = Percent(0);
    pub const HUNDRED: Percent = Percent(100_000);
    /// The most a form field accepts unless it has a range of its own.
    pub const MAX_INPUT: Percent = Percent(1_000_000);

    pub const fn from_thousandths(thousandths: i64) -> Percent {
        Percent(thousandths)
    }

    /// A whole percentage: `Percent::whole(70)` is 70%.
    pub const fn whole(percent: i64) -> Percent {
        Percent(percent.saturating_mul(1_000))
    }

    pub const fn thousandths(self) -> i64 {
        self.0
    }

    /// Reads what was typed into a percent field, with or without `%`:
    /// from 0 to 1,000, with at most three decimal places.
    pub fn parse(text: &str) -> Result<Percent, ParseError> {
        Percent::parse_between(text, Percent::ZERO, Percent::MAX_INPUT)
    }

    /// Reads a percent field that has a range of its own, both ends
    /// included.
    pub fn parse_between(text: &str, least: Percent, most: Percent) -> Result<Percent, ParseError> {
        let text = text.trim();
        let text = text.strip_suffix('%').unwrap_or(text);
        let percent = Percent(decimal::read(text, 3)?);
        if percent < least || percent > most {
            return Err(ParseError::OutOfRange {
                least: least.to_string(),
                most: most.to_string(),
            });
        }
        Ok(percent)
    }

    /// The percentage as a form field or a query string holds it: `9.975`.
    pub fn input(self) -> String {
        let sign = if self.0 < 0 { "-" } else { "" };
        format!(
            "{sign}{}",
            decimal::write(self.0.unsigned_abs(), 3, 0, false)
        )
    }

    /// The percentage to the nearest tenth, halves away from zero, for a
    /// place where three decimals are more than a reader wants.
    pub fn nearest_tenth(self) -> Percent {
        let tenths = decimal::div_round(Some(i128::from(self.0)), Some(100));
        Percent(tenths.unwrap_or(0).saturating_mul(100))
    }

    /// `part` as a percentage of `whole`, to the nearest thousandth.
    /// `Ok(None)` when `whole` is zero: there is no percentage of nothing.
    pub fn of(part: Money, whole: Money) -> Result<Option<Percent>, TooLarge> {
        if whole == Money::ZERO {
            return Ok(None);
        }
        let numerator = decimal::product(&[i128::from(part.cents()), 100_000]);
        let percent = decimal::div_round(numerator, Some(i128::from(whole.cents())));
        percent
            .map(|thousandths| Some(Percent(thousandths)))
            .ok_or(TooLarge)
    }
}

/// The percentage as a page shows it: `50%`, `9.975%`, `-12.5%`.
impl fmt::Display for Percent {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        let sign = if self.0 < 0 { "-" } else { "" };
        let amount = decimal::write(self.0.unsigned_abs(), 3, 0, true);
        write!(formatter, "{sign}{amount}%")
    }
}
