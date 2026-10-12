//! Reference tables read from a published source, where a reading falls
//! among a table's rows, and the unit conversions a page shows. Nothing
//! here holds a table.
//!
//! Temperatures and pressures are whole tenths of their unit. A conversion
//! is worked exactly and rounded once, as the rest of the crate does.

use crate::decimal::{self, ParseError, div_round, product};

/// The most, and below zero the least, that parse_tenths reads: 999.9.
pub const MOST_TENTHS: i64 = 9_999;
const PSI_NUMERATOR: i128 = 8_896_443_230_521; // pascals in a psi, as a fraction
const PSI_DENOMINATOR: i128 = 1_290_320_000;

/// Where a reading falls among rows sorted from least to most. The numbers are places in the list.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Place {
    Below,
    At(usize),
    Between(usize, usize),
    Above,
}

/// Where `reading` falls among `rows`, which are sorted from least to most.
/// `None` when there are no rows.
pub fn place(rows: &[i64], reading: i64) -> Option<Place> {
    let first = *rows.first()?;
    let last = *rows.last()?;
    if reading < first {
        return Some(Place::Below);
    }
    if reading > last {
        return Some(Place::Above);
    }
    let index = rows.iter().position(|row| *row >= reading)?;
    if rows[index] == reading {
        Some(Place::At(index))
    } else {
        Some(Place::Between(index - 1, index))
    }
}

/// Reads a temperature or a pressure in tenths: "-12.5" is -125.
pub fn parse_tenths(text: &str) -> Result<i64, ParseError> {
    let text = text.trim();
    let (below_zero, rest) = match text.strip_prefix('-') {
        Some(rest) => (true, rest),
        None => (false, text),
    };
    if rest.starts_with('-') {
        return Err(ParseError::NotANumber);
    }
    let tenths = decimal::read(rest, 1)?;
    if tenths > MOST_TENTHS {
        return Err(ParseError::OutOfRange {
            least: show_tenths(-MOST_TENTHS),
            most: show_tenths(MOST_TENTHS),
        });
    }
    Ok(if below_zero { -tenths } else { tenths })
}

/// Writes tenths with one decimal place: -125 is "-12.5", 0 is "0.0".
pub fn show_tenths(tenths: i64) -> String {
    let sign = if tenths < 0 { "-" } else { "" };
    format!(
        "{sign}{}",
        decimal::write(tenths.unsigned_abs(), 1, 1, false)
    )
}

/// A Fahrenheit temperature in tenths, as Celsius in tenths.
pub fn celsius_from_fahrenheit(tenths: i64) -> i64 {
    div_round(product(&[i128::from(tenths) - 320, 5]), Some(9)).unwrap_or(0)
}

/// A Celsius temperature in tenths, as Fahrenheit in tenths.
pub fn fahrenheit_from_celsius(tenths: i64) -> i64 {
    div_round(product(&[i128::from(tenths), 9]), Some(5))
        .map(|degrees| degrees + 320)
        .unwrap_or(0)
}

/// A kilopascal pressure in tenths, as psi in tenths.
pub fn psi_from_kpa(tenths: i64) -> i64 {
    div_round(
        product(&[i128::from(tenths), 1_000, PSI_DENOMINATOR]),
        Some(PSI_NUMERATOR),
    )
    .unwrap_or(0)
}

/// A psi pressure in tenths, as kilopascals in tenths.
pub fn kpa_from_psi(tenths: i64) -> i64 {
    div_round(
        product(&[i128::from(tenths), PSI_NUMERATOR]),
        Some(PSI_DENOMINATOR * 1_000),
    )
    .unwrap_or(0)
}
