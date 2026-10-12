//! Metric tire sizes: the figures of one size, and the comparison of two.
//!
//! For a size such as 275/65R18, with the width in millimetres, the aspect
//! ratio as a percent of the width and the rim in inches:
//!
//! - sidewall height = width × aspect ratio ÷ 100
//! - overall diameter = rim diameter × 25.4 + 2 × sidewall height
//! - circumference = π × overall diameter, and revolutions per kilometre
//!   = 1,000,000 ÷ circumference in millimetres
//! - the comparison of two sizes: the difference in diameter, the difference
//!   as a percent of the first diameter, the change in ride height (half the
//!   difference in diameter), and the true speed at a reading of 100 km/h
//!   (100 × second diameter ÷ first diameter)
//!
//! Lengths are whole hundredths of a millimetre, so the sidewall and the
//! diameter are exact. Every other figure is worked from the exact diameter
//! and rounded once.

use std::fmt;

use crate::decimal::{self, div_round, product};
use crate::percent::Percent;

pub const LEAST_WIDTH: u32 = 125;
pub const MOST_WIDTH: u32 = 455;
pub const LEAST_ASPECT: u32 = 20;
pub const MOST_ASPECT: u32 = 95;
pub const LEAST_RIM: u32 = 10;
pub const MOST_RIM: u32 = 30;

const PI_NUMERATOR: i128 = 3_141_592_653_589_793;
const PI_DENOMINATOR: i128 = 1_000_000_000_000_000;

/// A length in whole hundredths of a millimetre. It can be below zero: a
/// difference is.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Default)]
pub struct Length(i64);

impl Length {
    pub const fn from_hundredths_mm(hundredths: i64) -> Length {
        Length(hundredths)
    }

    pub const fn hundredths_mm(self) -> i64 {
        self.0
    }

    /// In millimetres to one decimal place: `814.7`, `-99.5`.
    pub fn mm(self) -> String {
        signed(
            div_round(Some(i128::from(self.0)), Some(10)).unwrap_or(0),
            1,
        )
    }

    /// In inches to two decimal places: `32.07`.
    pub fn inches(self) -> String {
        let tenths_of_hundredth = i128::from(self.0) * 10;
        signed(
            div_round(Some(tenths_of_hundredth), Some(254)).unwrap_or(0),
            2,
        )
    }
}

/// A figure in whole tenths. Its display is one decimal place: `390.7`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Default)]
pub struct Tenths(i64);

impl Tenths {
    pub const fn from_tenths(tenths: i64) -> Tenths {
        Tenths(tenths)
    }

    pub const fn tenths(self) -> i64 {
        self.0
    }
}

impl fmt::Display for Tenths {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{}", signed(self.0, 1))
    }
}

/// Why a tire size was not read. Each message is written for the person
/// filling in the form.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
#[non_exhaustive]
pub enum TireSizeError {
    #[error("Enter a tire size.")]
    Empty,
    #[error("This is not a tire size this page reads. Write it like 275/65R18 or LT285/70R17.")]
    NotASize,
    #[error("The width must be from {LEAST_WIDTH} to {MOST_WIDTH} millimetres.")]
    Width,
    #[error("The aspect ratio must be from {LEAST_ASPECT} to {MOST_ASPECT}.")]
    Aspect,
    #[error("The rim must be from {LEAST_RIM} to {MOST_RIM} inches.")]
    Rim,
}

/// The letters in front of a size. They say what the tire is built for and
/// change no figure.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Prefix {
    None,
    Passenger,
    LightTruck,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TireSize {
    pub prefix: Prefix,
    pub width_mm: u32,
    /// Sidewall height as a percent of the width.
    pub aspect: u32,
    /// The rim in tenths of an inch: 18 inches is 180, 19.5 inches is 195.
    pub rim_tenths_inch: u32,
}

impl TireSize {
    /// Reads a size as it is written on a sidewall. Spaces and the case of
    /// letters do not matter. Flotation sizes and bias-ply letters are not
    /// read.
    pub fn parse(text: &str) -> Result<TireSize, TireSizeError> {
        let text: String = text
            .chars()
            .filter(|character| !character.is_whitespace())
            .map(|character| character.to_ascii_uppercase())
            .collect();
        if text.is_empty() {
            return Err(TireSizeError::Empty);
        }
        let size = read(&text).ok_or(TireSizeError::NotASize)?;
        if !(LEAST_WIDTH..=MOST_WIDTH).contains(&size.width_mm) {
            return Err(TireSizeError::Width);
        }
        if !(LEAST_ASPECT..=MOST_ASPECT).contains(&size.aspect) {
            return Err(TireSizeError::Aspect);
        }
        if !(LEAST_RIM * 10..=MOST_RIM * 10).contains(&size.rim_tenths_inch) {
            return Err(TireSizeError::Rim);
        }
        Ok(size)
    }
}

/// The text of a size, upper case and without spaces, read in order.
/// `None` when the text is not the shape of a size.
fn read(text: &str) -> Option<TireSize> {
    let (prefix, rest) = if let Some(rest) = text.strip_prefix("LT") {
        (Prefix::LightTruck, rest)
    } else if let Some(rest) = text.strip_prefix('P') {
        (Prefix::Passenger, rest)
    } else {
        (Prefix::None, text)
    };
    let (width_mm, rest) = take_digits(rest, 3)?;
    let rest = rest.strip_prefix('/')?;
    let (aspect, rest) = take_digits(rest, 2)?;
    let rest = rest.strip_prefix("ZR").or_else(|| rest.strip_prefix('R'))?;
    let (rim, rest) = take_digits(rest, 2)?;
    let (half, rest) = match rest.strip_prefix(".5") {
        Some(rest) => (5, rest),
        None => (0, rest),
    };
    let prefix = match (rest, prefix) {
        ("", prefix) => prefix,
        ("C", Prefix::None) => Prefix::None,
        ("LT", Prefix::None) => Prefix::LightTruck,
        _ => return None,
    };
    Some(TireSize {
        prefix,
        width_mm,
        aspect,
        rim_tenths_inch: rim * 10 + half,
    })
}

/// The first `count` characters of `text` as a number, when they are all
/// ASCII digits, and the text after them.
fn take_digits(text: &str, count: usize) -> Option<(u32, &str)> {
    let head = text.get(..count)?;
    if !head.bytes().all(|byte| byte.is_ascii_digit()) {
        return None;
    }
    Some((head.parse().ok()?, text.get(count..)?))
}

impl fmt::Display for TireSize {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        let prefix = match self.prefix {
            Prefix::None => "",
            Prefix::Passenger => "P",
            Prefix::LightTruck => "LT",
        };
        let rim = decimal::write(u64::from(self.rim_tenths_inch), 1, 0, false);
        write!(formatter, "{prefix}{}/{}R{rim}", self.width_mm, self.aspect)
    }
}

/// The figures of one size.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Dimensions {
    pub sidewall: Length,
    pub diameter: Length,
    /// In millimetres.
    pub circumference: Tenths,
    pub revs_per_km: Tenths,
}

pub fn dimensions(size: &TireSize) -> Dimensions {
    let sidewall = i64::from(size.width_mm) * i64::from(size.aspect);
    let rim = i64::from(size.rim_tenths_inch) * 254;
    let diameter = rim + 2 * sidewall;
    let diameter_wide = i128::from(diameter);
    // Circumference in tenths of a millimetre: π × diameter ÷ 10.
    let circumference = div_round(
        product(&[diameter_wide, PI_NUMERATOR]),
        product(&[PI_DENOMINATOR, 10]),
    )
    .unwrap_or(0);
    // Revolutions per kilometre in tenths: 1,000,000,000 ÷ (π × diameter).
    let revs_per_km = div_round(
        product(&[1_000_000_000, PI_DENOMINATOR]),
        product(&[diameter_wide, PI_NUMERATOR]),
    )
    .unwrap_or(0);
    Dimensions {
        sidewall: Length(sidewall),
        diameter: Length(diameter),
        circumference: Tenths(circumference),
        revs_per_km: Tenths(revs_per_km),
    }
}

/// Two sizes compared. The second is measured against the first.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Comparison {
    pub first: Dimensions,
    pub second: Dimensions,
    /// The second diameter less the first.
    pub diameter_difference: Length,
    /// The same, as a percent of the first diameter.
    pub diameter_change: Percent,
    /// Half the difference: the change in ride height.
    pub ride_height_change: Length,
    /// The true speed in km/h when a speedometer set for the first size
    /// reads 100.
    pub actual_at_100: Tenths,
}

pub fn compare(first: &TireSize, second: &TireSize) -> Comparison {
    let first_dimensions = dimensions(first);
    let second_dimensions = dimensions(second);
    let (d1, d2) = (
        first_dimensions.diameter.hundredths_mm(),
        second_dimensions.diameter.hundredths_mm(),
    );
    let difference = d2 - d1;
    let diameter_change = div_round(
        product(&[i128::from(difference), 100_000]),
        Some(i128::from(d1)),
    )
    .unwrap_or(0);
    let actual_at_100 =
        div_round(product(&[i128::from(d2), 1_000]), Some(i128::from(d1))).unwrap_or(0);
    Comparison {
        first: first_dimensions,
        second: second_dimensions,
        diameter_difference: Length(difference),
        diameter_change: Percent::from_thousandths(diameter_change),
        ride_height_change: Length(difference / 2),
        actual_at_100: Tenths(actual_at_100),
    }
}

/// A number in units of `places` decimal places, with a minus sign when it
/// is below zero and all the places written.
fn signed(units: i64, places: u32) -> String {
    let sign = if units < 0 { "-" } else { "" };
    let amount = decimal::write(units.unsigned_abs(), places, places, false);
    format!("{sign}{amount}")
}
