//! Decimal numbers held as whole units: reading them from text and writing
//! them back.
//!
//! Money is whole cents and a percentage is whole thousandths of a percent,
//! so nothing here or anywhere else in the crate uses floating point.

/// Why the text of a field could not be read as a number. The message is
/// written for the person filling in the form.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[non_exhaustive]
pub enum ParseError {
    #[error("Enter a number.")]
    Empty,
    #[error("This is not a number. Write it like 1,234.50.")]
    NotANumber,
    #[error("This cannot be below zero.")]
    BelowZero,
    #[error("Use at most {0} decimal places.")]
    TooManyDecimals(u32),
    #[error("Use a whole number.")]
    NotWhole,
    #[error("This must be from {least} to {most}.")]
    OutOfRange { least: String, most: String },
}

/// A result that does not fit in the numbers this crate holds.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
#[error("The result is too large to show.")]
pub struct TooLarge;

/// Reads a number that is not below zero and has at most `places` decimal
/// places, and gives it in units of the last place: `read("12.5", 2)` is
/// 1250. A number too long to hold comes back as `i64::MAX`, which every
/// caller's own limit then refuses.
pub(crate) fn read(text: &str, places: u32) -> Result<i64, ParseError> {
    let text = text.trim();
    if text.is_empty() {
        return Err(ParseError::Empty);
    }
    let (below_zero, text) = match text.strip_prefix('-') {
        Some(rest) => (true, rest),
        None => (false, text),
    };
    let (whole, fraction) = text.split_once('.').unwrap_or((text, ""));
    if !well_grouped(whole) || !all_digits(fraction) || text == "." || text.is_empty() {
        return Err(ParseError::NotANumber);
    }
    let fraction = fraction.trim_end_matches('0');
    let mut value: i64 = 0;
    let mut given: u32 = 0;
    for digit in whole.bytes().filter(|byte| *byte != b',') {
        value = value
            .saturating_mul(10)
            .saturating_add(i64::from(digit - b'0'));
    }
    for digit in fraction.bytes() {
        value = value
            .saturating_mul(10)
            .saturating_add(i64::from(digit - b'0'));
        given = given.saturating_add(1);
    }
    if given > places {
        return Err(match places {
            0 => ParseError::NotWhole,
            _ => ParseError::TooManyDecimals(places),
        });
    }
    let value = value.saturating_mul(10_i64.saturating_pow(places - given));
    if below_zero && value != 0 {
        return Err(ParseError::BelowZero);
    }
    Ok(value)
}

fn all_digits(text: &str) -> bool {
    text.bytes().all(|byte| byte.is_ascii_digit())
}

/// Digits alone, or digits in groups of three after a first group of one to
/// three. `1,5` is not a number here: read as fifteen it would be wrong for
/// someone who writes a comma for the decimal point.
fn well_grouped(whole: &str) -> bool {
    let mut groups = whole.split(',');
    let first = groups.next().unwrap_or("");
    if !whole.contains(',') {
        return all_digits(first);
    }
    (1..=3).contains(&first.len())
        && all_digits(first)
        && groups.all(|group| group.len() == 3 && all_digits(group))
}

/// Writes a number held in units of the last of `places` decimal places.
/// At least `keep` decimal places are written, and zeros after those are
/// left off. `grouped` puts a comma between thousands.
pub(crate) fn write(units: u64, places: u32, keep: u32, grouped: bool) -> String {
    let scale = 10_u64.saturating_pow(places);
    let whole = (units / scale).to_string();
    let mut text = String::new();
    for (index, digit) in whole.chars().enumerate() {
        if grouped && index > 0 && (whole.len() - index).is_multiple_of(3) {
            text.push(',');
        }
        text.push(digit);
    }
    let fraction = format!("{:0width$}", units % scale, width = places as usize);
    let length = fraction.trim_end_matches('0').len().max(keep as usize);
    if places > 0 && length > 0 {
        text.push('.');
        text.extend(fraction.chars().take(length));
    }
    text
}

/// The factors multiplied together, or `None` if the product does not fit.
pub(crate) fn product(factors: &[i128]) -> Option<i128> {
    factors
        .iter()
        .try_fold(1_i128, |product, factor| product.checked_mul(*factor))
}

/// `numerator ÷ denominator` to the nearest whole number, halves away from
/// zero. This is the one place the crate rounds. `None` when either number
/// is missing, the denominator is zero, or the result does not fit.
pub(crate) fn div_round(numerator: Option<i128>, denominator: Option<i128>) -> Option<i64> {
    let (numerator, denominator) = (numerator?, denominator?);
    if denominator == 0 {
        return None;
    }
    let below_zero = (numerator < 0) != (denominator < 0);
    let (numerator, denominator) = (numerator.unsigned_abs(), denominator.unsigned_abs());
    let quotient = numerator / denominator;
    let remainder = numerator % denominator;
    let rounded = if remainder >= denominator - remainder {
        quotient.checked_add(1)?
    } else {
        quotient
    };
    let rounded = i128::try_from(rounded).ok()?;
    i64::try_from(if below_zero { -rounded } else { rounded }).ok()
}
