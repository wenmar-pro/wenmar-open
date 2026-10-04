//! The gross profit check: what labor, parts and sublet made in a period,
//! and how that stands against the targets.
//!
//! - gross profit = sales − cost; as a percent, gross profit ÷ sales.
//! - The overall line is the three lines added together.
//! - For labor or parts below its range: sales needed = cost ÷ (1 − usual
//!   target), and the difference from the sales made.

use crate::decimal::{TooLarge, div_round, product};
use crate::money::Money;
use crate::percent::Percent;
use crate::targets::{LABOR, OVERALL, PARTS, Standing, Target};

/// What one kind of work sold for in the period and what it cost.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Line {
    pub sales: Money,
    pub cost: Money,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct GrossProfitInput {
    /// Labor sales, and what the technicians cost.
    pub labor: Line,
    pub parts: Line,
    /// Work bought from another shop. Zero when there was none.
    pub sublet: Line,
}

/// A field of the gross profit form.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GrossProfitField {
    LaborSales,
    TechnicianCost,
    PartsSales,
    PartsCost,
    SubletSales,
    SubletCost,
}

/// Why the gross profit was not worked out. [`GrossProfitError::field`]
/// says which field to put the message beside; without one it belongs at
/// the form.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
#[non_exhaustive]
pub enum GrossProfitError {
    #[error("Enter sales for labor, parts or sublet.")]
    NoSales,
    #[error("This cannot be below zero.")]
    BelowZero { field: GrossProfitField },
    #[error(transparent)]
    TooLarge(#[from] TooLarge),
}

impl GrossProfitError {
    pub fn field(&self) -> Option<GrossProfitField> {
        match self {
            GrossProfitError::BelowZero { field } => Some(*field),
            _ => None,
        }
    }
}

/// What a line below its range would have to sell, at the same cost, to
/// reach the usual target.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Shortfall {
    pub sales_needed: Money,
    /// Sales needed less the sales made.
    pub difference: Money,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LineResult {
    pub sales: Money,
    pub cost: Money,
    pub profit: Money,
    /// Gross profit as a percent of sales. `None` when there were no sales.
    pub percent: Option<Percent>,
    /// Where the percent stands against the line's target. `None` for
    /// sublet, which has no target, and for a line with no percent.
    pub standing: Option<Standing>,
    /// Given for labor and for parts when the line is below its range.
    pub shortfall: Option<Shortfall>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct GrossProfit {
    pub labor: LineResult,
    pub parts: LineResult,
    pub sublet: LineResult,
    /// Labor, parts and sublet together.
    pub overall: LineResult,
}

/// One line's result. A shortfall is worked out only when `act` is true:
/// the overall line has a target, but no single cost to act on.
fn line(line: Line, target: Option<&Target>, act: bool) -> Result<LineResult, TooLarge> {
    let profit = line.sales.checked_sub(line.cost)?;
    let percent = Percent::of(profit, line.sales)?;
    let standing = target
        .zip(percent)
        .map(|(target, percent)| target.standing(percent));
    let shortfall = match (target, standing) {
        (Some(target), Some(Standing::Below)) if act => {
            let rest = Percent::HUNDRED.thousandths() - target.usual.thousandths();
            let cost = product(&[i128::from(line.cost.cents()), 100_000]);
            let needed = div_round(cost, Some(i128::from(rest))).ok_or(TooLarge)?;
            let sales_needed = Money::from_cents(needed);
            Some(Shortfall {
                sales_needed,
                difference: sales_needed.checked_sub(line.sales)?,
            })
        }
        _ => None,
    };
    Ok(LineResult {
        sales: line.sales,
        cost: line.cost,
        profit,
        percent,
        standing,
        shortfall,
    })
}

/// The gross profit of a period.
pub fn gross_profit(input: &GrossProfitInput) -> Result<GrossProfit, GrossProfitError> {
    use GrossProfitField as Field;
    let (labor, parts, sublet) = (input.labor, input.parts, input.sublet);
    for (amount, field) in [
        (labor.sales, Field::LaborSales),
        (labor.cost, Field::TechnicianCost),
        (parts.sales, Field::PartsSales),
        (parts.cost, Field::PartsCost),
        (sublet.sales, Field::SubletSales),
        (sublet.cost, Field::SubletCost),
    ] {
        if amount < Money::ZERO {
            return Err(GrossProfitError::BelowZero { field });
        }
    }
    let overall = Line {
        sales: labor
            .sales
            .checked_add(parts.sales)?
            .checked_add(sublet.sales)?,
        cost: labor
            .cost
            .checked_add(parts.cost)?
            .checked_add(sublet.cost)?,
    };
    if overall.sales == Money::ZERO {
        return Err(GrossProfitError::NoSales);
    }
    Ok(GrossProfit {
        labor: line(labor, Some(&LABOR), true)?,
        parts: line(parts, Some(&PARTS), true)?,
        sublet: line(sublet, None, false)?,
        overall: line(overall, Some(&OVERALL), false)?,
    })
}
