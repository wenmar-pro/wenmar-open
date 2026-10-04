//! Quantities that are neither money nor a percentage: hours, and a count.

use std::fmt;

use crate::decimal::{self, ParseError};

/// A number of hours in whole hundredths of an hour.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Default)]
pub struct Hours(i64);

impl Hours {
    pub const ZERO: Hours = Hours(0);
    /// The most a form field accepts: 99,999,999.99.
    pub const MAX_INPUT: Hours = Hours(9_999_999_999);

    pub const fn from_hundredths(hundredths: i64) -> Hours {
        Hours(hundredths)
    }

    pub const fn hundredths(self) -> i64 {
        self.0
    }

    /// Reads what was typed into an hours field: from 0 to
    /// [`Hours::MAX_INPUT`], with at most two decimal places.
    pub fn parse(text: &str) -> Result<Hours, ParseError> {
        let hundredths = decimal::read(text, 2)?;
        if hundredths > Hours::MAX_INPUT.0 {
            return Err(ParseError::OutOfRange {
                least: "0".to_owned(),
                most: Hours::MAX_INPUT.to_string(),
            });
        }
        Ok(Hours(hundredths))
    }

    /// The hours as a form field or a query string holds them: `1470.5`.
    pub fn input(self) -> String {
        let sign = if self.0 < 0 { "-" } else { "" };
        format!(
            "{sign}{}",
            decimal::write(self.0.unsigned_abs(), 2, 0, false)
        )
    }
}

/// The hours as a page shows them: `173`, `441.15`, `1,470.5`.
impl fmt::Display for Hours {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        let sign = if self.0 < 0 { "-" } else { "" };
        let amount = decimal::write(self.0.unsigned_abs(), 2, 0, true);
        write!(formatter, "{sign}{amount}")
    }
}

/// The most [`parse_count`] accepts.
pub const MAX_COUNT: u32 = 1_000;

/// Reads a whole number from 0 to [`MAX_COUNT`], such as a number of
/// technicians.
pub fn parse_count(text: &str) -> Result<u32, ParseError> {
    let count = decimal::read(text, 0)?;
    match u32::try_from(count) {
        Ok(count) if count <= MAX_COUNT => Ok(count),
        _ => Err(ParseError::OutOfRange {
            least: "0".to_owned(),
            most: decimal::write(u64::from(MAX_COUNT), 0, 0, true),
        }),
    }
}
