//! The voltage drop along a wire, and the thinnest wire that stays within a
//! drop allowed.
//!
//! The drop is `drop = current × resistance per metre × length`, in
//! millivolts, rounded once. The percent is `percent = drop ÷ system
//! voltage`, worked from the drop as shown, to a thousandth of a percent.
//! A wire is within the drop when its percent is no more than the percent
//! allowed. The resistance of each gauge is data read from a published
//! source (`data/reference/wire-resistance.toml`); nothing here holds a
//! figure of a wire.
//!
//! Current is whole tenths of an ampere, length is whole micrometres, and
//! resistance is whole nano-ohms per metre. A length in feet is converted
//! exactly: a hundredth of a foot is 3,048 micrometres.

use crate::decimal::{self, ParseError, TooLarge, div_round, product};
use crate::percent::Percent;

/// A hundredth of a metre, in micrometres.
pub const PER_HUNDREDTH_METRE: i64 = 10_000;
/// A hundredth of a foot, in micrometres: a foot is 0.3048 metres.
pub const PER_HUNDREDTH_FOOT: i64 = 3_048;
/// 500 A, in tenths of an ampere.
pub const MOST_CURRENT: i64 = 5_000;
/// 1,000 metres or feet, in hundredths.
pub const MOST_LENGTH: i64 = 100_000;
/// 1,000 ohms, in ten-thousandths of an ohm.
pub const MOST_RESISTANCE: i64 = 10_000_000;
pub const LEAST_ALLOWED: Percent = Percent::from_thousandths(100);
pub const MOST_ALLOWED: Percent = Percent::whole(20);
/// The allowed drop when the field is left empty: an example to start from, not a standard.
pub const DEFAULT_ALLOWED: Percent = Percent::whole(3);

/// Reads a current in amperes: tenths of an ampere.
pub fn parse_current(text: &str) -> Result<i64, ParseError> {
    between(text, 1, MOST_CURRENT)
}

/// Reads a length in metres or feet: hundredths of the unit.
pub fn parse_length(text: &str) -> Result<i64, ParseError> {
    between(text, 2, MOST_LENGTH)
}

/// Reads a resistance in ohms per unit: ten-thousandths of an ohm.
pub fn parse_resistance(text: &str) -> Result<i64, ParseError> {
    between(text, 4, MOST_RESISTANCE)
}

/// Ohms per kilometre, in ten-thousandths, as nano-ohms per metre.
pub fn from_ohms_per_km(ten_thousandths: i64) -> i64 {
    ten_thousandths.saturating_mul(100)
}

/// Ohms per 1000 feet, in ten-thousandths, as nano-ohms per metre. A
/// thousand feet is 304.8 metres, so the figure is `× 10^6 × 1250 ÷ 381`.
/// `i64::MAX` when it does not fit.
pub fn from_ohms_per_1000_ft(ten_thousandths: i64) -> i64 {
    div_round(product(&[i128::from(ten_thousandths), 125_000]), Some(381)).unwrap_or(i64::MAX)
}

/// One run of wire at one current, as the form reads it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Circuit {
    pub current_tenths_amp: i64,
    /// Every metre the current flows through.
    pub length_micrometres: i64,
    /// 12_000 or 24_000.
    pub system_millivolts: i64,
    pub allowed: Percent,
}

/// The drop along one gauge of wire.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Drop {
    pub millivolts: i64,
    pub percent: Percent,
    pub within: bool,
}

/// The drop along a wire of `nano_ohms_per_metre` for `circuit`.
pub fn voltage_drop(circuit: &Circuit, nano_ohms_per_metre: i64) -> Result<Drop, TooLarge> {
    // Tenths of an ampere × nano-ohms × micrometres is 10^-16 volts, so
    // dividing by 10^13 gives millivolts.
    let millivolts = div_round(
        product(&[
            i128::from(circuit.current_tenths_amp),
            i128::from(nano_ohms_per_metre),
            i128::from(circuit.length_micrometres),
        ]),
        Some(10_000_000_000_000),
    )
    .ok_or(TooLarge)?;
    let system = i128::from(circuit.system_millivolts);
    let percent = Percent::from_thousandths(
        div_round(product(&[i128::from(millivolts), 100_000]), Some(system)).ok_or(TooLarge)?,
    );
    let drop = product(&[i128::from(millivolts), 100_000]).ok_or(TooLarge)?;
    let limit = product(&[system, i128::from(circuit.allowed.thousandths())]).ok_or(TooLarge)?;
    Ok(Drop {
        millivolts,
        percent,
        within: drop <= limit,
    })
}

/// A voltage in millivolts, written in volts with three decimal places: "0.236".
pub fn volts(millivolts: i64) -> String {
    let sign = if millivolts < 0 { "-" } else { "" };
    format!(
        "{sign}{}",
        decimal::write(millivolts.unsigned_abs(), 3, 3, false)
    )
}

/// A number that is above zero, at most `most` in the unit of `places`.
fn between(text: &str, places: u32, most: i64) -> Result<i64, ParseError> {
    let value = decimal::read(text, places)?;
    if value == 0 || value > most {
        return Err(ParseError::OutOfRange {
            least: decimal::write(1, places, 0, false),
            most: decimal::write(most.unsigned_abs(), places, 0, true),
        });
    }
    Ok(value)
}
