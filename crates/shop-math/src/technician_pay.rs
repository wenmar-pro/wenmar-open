//! A technician's gross pay from the hours they worked and the rates they
//! are paid, in whole cents.
//!
//! ```text
//! gross = flagged hours × flat rate
//!       + hourly hours × hourly rate
//!       + overtime hours × overtime rate
//! ```
//!
//! Each term is hours times a rate, to the cent, and the terms are added.
//! The deductions are not worked here: the page takes the gross to
//! `payroll-calc` and shows what the engine gives. Nothing here uses
//! floating point, and a result that does not fit is [`TooLarge`].

use crate::decimal::{TooLarge, div_round, product};
use crate::money::Money;
use crate::quantity::Hours;

/// Hundredths of an hour in an hour.
const PER_HOUR: i128 = 100;

fn money(cents: Option<i64>) -> Result<Money, TooLarge> {
    cents.map(Money::from_cents).ok_or(TooLarge)
}

/// What `hours` of work come to at `rate` an hour, to the cent.
fn charge(rate: Money, hours: Hours) -> Result<Money, TooLarge> {
    let cents = i128::from(rate.cents());
    let hundredths = i128::from(hours.hundredths());
    money(div_round(product(&[cents, hundredths]), Some(PER_HOUR)))
}

/// The three parts of the gross, each to the cent, and their total.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Gross {
    /// Flagged hours at the flat rate.
    pub flagged: Money,
    /// Hourly hours at the hourly rate.
    pub hourly: Money,
    /// Overtime hours at the overtime rate.
    pub overtime: Money,
    /// The three parts added.
    pub total: Money,
}

/// The technician's gross pay: the flagged hours at the flat rate, the
/// hourly hours at the hourly rate, and the overtime hours at the overtime
/// rate. Each part is worked to the cent and the parts are added.
pub fn gross(
    flagged_hours: Hours,
    flat_rate: Money,
    hourly_hours: Hours,
    hourly_rate: Money,
    overtime_hours: Hours,
    overtime_rate: Money,
) -> Result<Gross, TooLarge> {
    let flagged = charge(flat_rate, flagged_hours)?;
    let hourly = charge(hourly_rate, hourly_hours)?;
    let overtime = charge(overtime_rate, overtime_hours)?;
    let total = flagged.checked_add(hourly)?.checked_add(overtime)?;
    Ok(Gross {
        flagged,
        hourly,
        overtime,
        total,
    })
}
