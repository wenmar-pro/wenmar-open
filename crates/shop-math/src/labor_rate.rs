//! The labor rate: the rate a shop needs, and the rate it is getting.
//!
//! The rate the shop needs:
//!
//! - hours billed = technicians × paid hours × productivity
//! - labor sales needed = (technician cost + overhead − parts gross profit)
//!   ÷ (1 − target profit)
//! - rate needed = labor sales needed ÷ hours billed
//! - break-even rate = the same with a target profit of zero
//! - rate from the labor target = technician cost ÷ hours billed
//!   ÷ (1 − target labor gross profit)
//!
//! The rate the shop is getting: effective rate = labor sales ÷ hours
//! billed.
//!
//! Each amount of money is worked from the inputs and rounded once. A
//! percentage is worked from the amounts as shown, so a reader's own
//! division agrees with the page.

use std::cmp::Ordering;

use crate::decimal::{TooLarge, div_round, product};
use crate::money::Money;
use crate::percent::Percent;
use crate::quantity::Hours;

/// Paid hours per technician per month when the field is left empty: 40
/// hours a week.
pub const DEFAULT_PAID_HOURS: Hours = Hours::from_hundredths(17_300);
/// Productivity when the field is left empty.
pub const DEFAULT_PRODUCTIVITY: Percent = Percent::whole(85);
/// Productivity is from 1% to 200%.
pub const LEAST_PRODUCTIVITY: Percent = Percent::whole(1);
pub const MOST_PRODUCTIVITY: Percent = Percent::whole(200);
/// Target profit when the field is left empty.
pub const DEFAULT_TARGET_PROFIT: Percent = Percent::whole(10);
/// A target profit, and a target labor gross profit, is from 0% to 99%.
pub const MOST_TARGET: Percent = Percent::whole(99);

const HUNDRED: i128 = 100_000;
/// Hundredths of an hour in an hour.
const PER_HOUR: i128 = 100;

/// A field of the labor rate form.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LaborField {
    Technicians,
    PaidHours,
    Productivity,
    TechnicianCost,
    Overhead,
    PartsGrossProfit,
    TargetProfit,
    TargetLaborGrossProfit,
    LaborSales,
    HoursBilled,
    PostedRate,
    PeriodTechnicianCost,
}

/// Why a labor rate was not worked out. [`LaborRateError::field`] says
/// which field to put the message beside.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
#[non_exhaustive]
pub enum LaborRateError {
    #[error("No hours would be billed with this.")]
    NoHours { field: LaborField },
    #[error("This cannot be zero.")]
    Zero { field: LaborField },
    #[error("This cannot be below zero.")]
    BelowZero { field: LaborField },
    #[error("This must be below 100%.")]
    NotBelowHundred { field: LaborField },
    #[error(transparent)]
    TooLarge(#[from] TooLarge),
}

impl LaborRateError {
    /// The field the refusal is about. `None` for a result too large.
    pub fn field(&self) -> Option<LaborField> {
        match self {
            LaborRateError::NoHours { field }
            | LaborRateError::Zero { field }
            | LaborRateError::BelowZero { field }
            | LaborRateError::NotBelowHundred { field } => Some(*field),
            LaborRateError::TooLarge(_) => None,
        }
    }
}

/// What the shop pays and expects in a month.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RateNeededInput {
    pub technicians: u32,
    /// Paid hours for one technician in the month.
    pub paid_hours: Hours,
    /// Hours billed as a percent of hours paid.
    pub productivity: Percent,
    /// Wages, and what the shop pays on top, for all technicians.
    pub technician_cost: Money,
    pub overhead: Money,
    pub parts_gross_profit: Money,
    /// Profit wanted, as a percent of labor sales.
    pub target_profit: Percent,
    pub target_labor_gross_profit: Percent,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RateNeeded {
    pub hours_billed: Hours,
    pub labor_sales_needed: Money,
    /// Per hour billed. Zero when parts gross profit covers every cost.
    pub rate_needed: Money,
    /// The rate needed with a target profit of zero.
    pub break_even_rate: Money,
    /// Parts gross profit is at least technician cost and overhead
    /// together, so labor has nothing left to cover.
    pub covered_by_parts: bool,
    /// Labor gross profit at the rate needed. `None` when labor sales
    /// needed are zero.
    pub labor_gross_profit: Option<Percent>,
    /// The rate the target labor gross profit gives by itself.
    pub target_rate: Money,
    /// The rate needed against the rate from the target: `Greater` when
    /// overhead asks for more than the target gives, `Less` when the target
    /// leaves more profit than was asked for.
    pub needed_against_target: Ordering,
}

fn money(cents: Option<i64>) -> Result<Money, TooLarge> {
    cents.map(Money::from_cents).ok_or(TooLarge)
}

fn not_below_zero(amount: i64, field: LaborField) -> Result<i128, LaborRateError> {
    if amount < 0 {
        return Err(LaborRateError::BelowZero { field });
    }
    Ok(i128::from(amount))
}

/// What is left of 100% after a target, which is from 0% to below 100%.
fn rest_after(target: Percent, field: LaborField) -> Result<i128, LaborRateError> {
    let target = not_below_zero(target.thousandths(), field)?;
    if target >= HUNDRED {
        return Err(LaborRateError::NotBelowHundred { field });
    }
    Ok(HUNDRED - target)
}

/// The rate the shop needs.
pub fn rate_needed(input: &RateNeededInput) -> Result<RateNeeded, LaborRateError> {
    use LaborField as Field;
    let technicians = i128::from(input.technicians);
    let paid = not_below_zero(input.paid_hours.hundredths(), Field::PaidHours)?;
    let productivity = not_below_zero(input.productivity.thousandths(), Field::Productivity)?;
    let cost = not_below_zero(input.technician_cost.cents(), Field::TechnicianCost)?;
    let overhead = not_below_zero(input.overhead.cents(), Field::Overhead)?;
    let parts = not_below_zero(input.parts_gross_profit.cents(), Field::PartsGrossProfit)?;
    let kept = rest_after(input.target_profit, Field::TargetProfit)?;
    let kept_by_target = rest_after(
        input.target_labor_gross_profit,
        Field::TargetLaborGrossProfit,
    )?;

    // Hours billed, in hundredths of an hour times 100,000: the paid hours
    // in hundredths times the productivity in thousandths of a percent.
    let hours = product(&[technicians, paid, productivity]).ok_or(TooLarge)?;
    let hours_billed = div_round(Some(hours), Some(HUNDRED)).ok_or(TooLarge)?;
    if hours_billed == 0 {
        // The first field that is zero, or paid hours when the hours are
        // only too few to count.
        let field = match (technicians, paid, productivity) {
            (0, _, _) => Field::Technicians,
            (_, _, 0) if paid != 0 => Field::Productivity,
            _ => Field::PaidHours,
        };
        return Err(LaborRateError::NoHours { field });
    }

    // What labor has to cover. Not below zero: parts cannot cover more
    // than there is.
    let to_cover = (cost + overhead - parts).max(0);
    let sales = div_round(product(&[to_cover, HUNDRED]), Some(kept));
    let labor_sales_needed = money(sales)?;
    // A rate is cents ÷ hours: the cents are multiplied by what the hours
    // were multiplied by.
    let per_hour = |cents: Option<i128>, kept: i128| {
        let cents = cents.and_then(|cents| product(&[cents, PER_HOUR, HUNDRED]));
        money(div_round(cents, product(&[kept, hours])))
    };
    let rate_needed = per_hour(product(&[to_cover, HUNDRED]), kept)?;
    let target_rate = per_hour(product(&[cost, HUNDRED]), kept_by_target)?;
    let gross_profit = labor_sales_needed.checked_sub(input.technician_cost)?;
    Ok(RateNeeded {
        hours_billed: Hours::from_hundredths(hours_billed),
        labor_sales_needed,
        rate_needed,
        break_even_rate: per_hour(Some(to_cover), 1)?,
        covered_by_parts: cost + overhead <= parts,
        labor_gross_profit: Percent::of(gross_profit, labor_sales_needed)?,
        target_rate,
        needed_against_target: rate_needed.cmp(&target_rate),
    })
}

/// What the shop sold in a period.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct EffectiveRateInput {
    pub labor_sales: Money,
    pub hours_billed: Hours,
    pub posted_rate: Option<Money>,
    pub technician_cost: Option<Money>,
}

/// The effective rate beside the posted rate.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AgainstPosted {
    /// The effective rate as a percent of the posted rate.
    pub percent_of_posted: Percent,
    /// The posted rate less the effective rate. Below zero when the shop
    /// is getting more than it posts.
    pub per_hour: Money,
    /// What the hours billed would have sold for at the posted rate, less
    /// what they did sell for.
    pub over_the_period: Money,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct EffectiveRate {
    pub rate: Money,
    /// `None` when no posted rate was given.
    pub against_posted: Option<AgainstPosted>,
    /// `None` when no technician cost was given, or labor sales are zero.
    pub labor_gross_profit: Option<Percent>,
}

/// The rate the shop is getting.
pub fn effective_rate(input: &EffectiveRateInput) -> Result<EffectiveRate, LaborRateError> {
    use LaborField as Field;
    let sales = not_below_zero(input.labor_sales.cents(), Field::LaborSales)?;
    let hours = not_below_zero(input.hours_billed.hundredths(), Field::HoursBilled)?;
    if hours == 0 {
        let field = Field::HoursBilled;
        return Err(LaborRateError::NoHours { field });
    }
    let rate = money(div_round(product(&[sales, PER_HOUR]), Some(hours)))?;
    let against_posted = match input.posted_rate {
        None => None,
        Some(posted) => {
            let field = Field::PostedRate;
            let cents = not_below_zero(posted.cents(), field)?;
            let percent = Percent::of(rate, posted)?;
            let at_posted = money(div_round(product(&[cents, hours]), Some(PER_HOUR)))?;
            Some(AgainstPosted {
                percent_of_posted: percent.ok_or(LaborRateError::Zero { field })?,
                per_hour: posted.checked_sub(rate)?,
                over_the_period: at_posted.checked_sub(input.labor_sales)?,
            })
        }
    };
    let labor_gross_profit = match input.technician_cost {
        None => None,
        Some(cost) => {
            not_below_zero(cost.cents(), Field::PeriodTechnicianCost)?;
            Percent::of(input.labor_sales.checked_sub(cost)?, input.labor_sales)?
        }
    };
    Ok(EffectiveRate {
        rate,
        against_posted,
        labor_gross_profit,
    })
}
