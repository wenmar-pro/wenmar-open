//! The state of charge of a battery from its open-circuit voltage, and the
//! correction of that voltage to the temperature its table was measured at.
//!
//! The correction is `corrected = reading + per degree × (reference − temperature)`,
//! in degrees Celsius, rounded once. The table and the correction figures
//! are data read from a published source (`data/reference/battery-soc.toml`);
//! nothing here holds a figure of a battery.
//!
//! Volts are whole millivolts, a temperature is whole tenths of a degree
//! Celsius, and the correction is whole microvolts a degree.

use crate::decimal::{self, ParseError, div_round, product};

/// The most a voltage field accepts: 99.999 V, in millivolts.
pub const MOST_MILLIVOLTS: i64 = 99_999;

/// What a source says to add to a reading for each degree Celsius the battery is below the
/// temperature its table was measured at.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Correction {
    pub reference_tenths_c: i64,
    pub microvolts_per_degree: i64,
}

/// Reads a voltage in millivolts: "12.6", "12.065 V" or "12.60v" are 12_600, 12_065 and 12_600.
pub fn parse_volts(text: &str) -> Result<i64, ParseError> {
    let text = text.trim();
    let text = text
        .strip_suffix('V')
        .or_else(|| text.strip_suffix('v'))
        .unwrap_or(text)
        .trim_end();
    let millivolts = decimal::read(text, 3)?;
    if millivolts > MOST_MILLIVOLTS {
        return Err(ParseError::OutOfRange {
            least: "0".to_owned(),
            most: volts(MOST_MILLIVOLTS),
        });
    }
    Ok(millivolts)
}

/// The voltage corrected to the reference temperature of `correction`, given the
/// reading in millivolts and the battery's temperature in tenths of a degree Celsius.
/// `None` when the result does not fit.
pub fn corrected(millivolts: i64, tenths_c: i64, correction: &Correction) -> Option<i64> {
    let degrees = i128::from(correction.reference_tenths_c) - i128::from(tenths_c);
    // Tenths of a degree × microvolts is ten-thousandths of a millivolt.
    let added = div_round(
        product(&[i128::from(correction.microvolts_per_degree), degrees]),
        Some(10_000),
    )?;
    millivolts.checked_add(added)
}

/// Writes millivolts as volts: two decimal places, or three when the third is not zero.
pub fn volts(millivolts: i64) -> String {
    let sign = if millivolts < 0 { "-" } else { "" };
    format!(
        "{sign}{}",
        decimal::write(millivolts.unsigned_abs(), 3, 2, false)
    )
}
