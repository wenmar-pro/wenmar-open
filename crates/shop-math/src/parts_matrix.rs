//! The parts matrix: tiers of cost, each with a markup or a margin, and
//! what a part sells for under them.
//!
//! - price = cost × (1 + markup), or cost ÷ (1 − margin) when the tiers are
//!   given as margins. A price is worked from the percent that was entered,
//!   never from one converted and rounded.
//! - margin = markup ÷ (1 + markup); markup = margin ÷ (1 − margin).
//! - A cost belongs to the first tier whose "cost up to" is at or above it,
//!   and to the last tier when it is above them all.
//! - Blended margin: with share s of parts spend and markup m in each tier,
//!   sales = Σ s × (1 + m), profit = Σ s × m, blended margin = profit ÷ sales.

use crate::decimal::{TooLarge, div_round, product};
use crate::money::Money;
use crate::percent::Percent;

/// The most tiers a matrix has.
pub const MAX_TIERS: usize = 8;

const HUNDRED: i128 = 100_000;

/// How the percent of every tier is read.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum RateKind {
    /// Profit as a percent of cost.
    #[default]
    Markup,
    /// Profit as a percent of the selling price.
    Margin,
}

/// One row of the form that was filled in. Rows left empty are not passed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TierInput {
    /// The row's number on the form, from 1. Refusals name it.
    pub row: usize,
    pub cost_up_to: Money,
    /// A markup or a margin, as [`RateKind`] says.
    pub rate: Percent,
    /// This tier's share of what the shop spends on parts.
    pub share: Option<Percent>,
}

/// One tier of the result.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TierResult {
    pub row: usize,
    /// The lowest cost in the tier: zero, or a cent above the tier before.
    pub cost_from: Money,
    /// The highest cost in the tier. `None` for the last tier, which has
    /// no upper bound.
    pub cost_up_to: Option<Money>,
    pub markup: Percent,
    pub margin: Percent,
    /// The "cost up to" figure as entered, for the last tier too.
    pub sample_cost: Money,
    pub sample_price: Money,
    pub sample_profit: Money,
    pub share: Option<Percent>,
}

/// One part priced by the matrix.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PartResult {
    /// The row of the tier the cost belongs to.
    pub row: usize,
    pub cost: Money,
    pub price: Money,
    pub profit: Money,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MatrixResult {
    pub tiers: Vec<TierResult>,
    /// The part priced, when a cost was given.
    pub part: Option<PartResult>,
    /// Total profit over total sales, when shares were given.
    pub blended_margin: Option<Percent>,
}

/// Why a matrix was refused. [`MatrixError::row`] says which row to put the
/// message beside; without one it belongs at the form.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[non_exhaustive]
pub enum MatrixError {
    #[error("Enter at least one tier: a cost and a percent.")]
    NoTiers,
    #[error("A matrix has at most 8 tiers.")]
    TooManyTiers,
    #[error("Row {row}: \"cost up to\" must be higher than in the row above.")]
    NotRising { row: usize },
    #[error("Row {row}: a margin must be below 100%.")]
    MarginTooHigh { row: usize },
    #[error("Row {row}: a figure is below zero.")]
    BelowZero { row: usize },
    #[error("The shares of parts spend add to {total}. They must add to 100%.")]
    SharesNotHundred { total: Percent },
    #[error("The part's cost is below zero.")]
    PartCostBelowZero,
    #[error(transparent)]
    TooLarge(#[from] TooLarge),
}

impl MatrixError {
    /// The row of the form the refusal is about, if it is about one.
    pub fn row(&self) -> Option<usize> {
        match self {
            MatrixError::NotRising { row }
            | MatrixError::MarginTooHigh { row }
            | MatrixError::BelowZero { row } => Some(*row),
            _ => None,
        }
    }
}

/// The margin a markup gives, to the nearest thousandth of a percent.
/// `None` for a markup below zero.
pub fn markup_to_margin(markup: Percent) -> Option<Percent> {
    let markup = i128::from(markup.thousandths());
    if markup < 0 {
        return None;
    }
    div_round(product(&[markup, HUNDRED]), HUNDRED.checked_add(markup))
        .map(Percent::from_thousandths)
}

/// The markup a margin needs, to the nearest thousandth of a percent.
/// `None` for a margin below zero, or of 100% or more.
pub fn margin_to_markup(margin: Percent) -> Option<Percent> {
    let margin = i128::from(margin.thousandths());
    if !(0..HUNDRED).contains(&margin) {
        return None;
    }
    div_round(product(&[margin, HUNDRED]), HUNDRED.checked_sub(margin))
        .map(Percent::from_thousandths)
}

/// What a cost sells for, to the nearest cent.
fn price(kind: RateKind, rate: Percent, cost: Money) -> Result<Money, TooLarge> {
    let (cost, rate) = (i128::from(cost.cents()), i128::from(rate.thousandths()));
    let cents = match kind {
        RateKind::Markup => div_round(product(&[cost, HUNDRED + rate]), Some(HUNDRED)),
        RateKind::Margin => div_round(product(&[cost, HUNDRED]), Some(HUNDRED - rate)),
    };
    cents.map(Money::from_cents).ok_or(TooLarge)
}

/// Works out a matrix: every tier, the one part if a cost was given, and
/// the blended margin if any share was given. A tier with no share counts
/// as a share of zero.
pub fn work_out(
    kind: RateKind,
    tiers: &[TierInput],
    part_cost: Option<Money>,
) -> Result<MatrixResult, MatrixError> {
    if tiers.is_empty() {
        return Err(MatrixError::NoTiers);
    }
    if tiers.len() > MAX_TIERS {
        return Err(MatrixError::TooManyTiers);
    }
    let mut results: Vec<TierResult> = Vec::with_capacity(tiers.len());
    for (index, tier) in tiers.iter().enumerate() {
        let row = tier.row;
        let share = tier.share.unwrap_or(Percent::ZERO);
        if tier.cost_up_to < Money::ZERO || tier.rate < Percent::ZERO || share < Percent::ZERO {
            return Err(MatrixError::BelowZero { row });
        }
        let cost_from = match results.last() {
            None => Money::ZERO,
            Some(before) if tier.cost_up_to <= before.sample_cost => {
                return Err(MatrixError::NotRising { row });
            }
            Some(before) => before.sample_cost.checked_add(Money::from_cents(1))?,
        };
        let (markup, margin) = match kind {
            RateKind::Markup => (tier.rate, markup_to_margin(tier.rate).ok_or(TooLarge)?),
            RateKind::Margin => {
                let markup = margin_to_markup(tier.rate);
                (markup.ok_or(MatrixError::MarginTooHigh { row })?, tier.rate)
            }
        };
        let sample_price = price(kind, tier.rate, tier.cost_up_to)?;
        results.push(TierResult {
            row,
            cost_from,
            cost_up_to: (index + 1 < tiers.len()).then_some(tier.cost_up_to),
            markup,
            margin,
            sample_cost: tier.cost_up_to,
            sample_price,
            sample_profit: sample_price.checked_sub(tier.cost_up_to)?,
            share: tier.share,
        });
    }
    let part = match part_cost {
        None => None,
        Some(cost) if cost < Money::ZERO => return Err(MatrixError::PartCostBelowZero),
        Some(cost) => {
            let tier = tiers.iter().find(|tier| cost <= tier.cost_up_to);
            let tier = tier.or(tiers.last()).ok_or(MatrixError::NoTiers)?;
            let price = price(kind, tier.rate, cost)?;
            Some(PartResult {
                row: tier.row,
                cost,
                price,
                profit: price.checked_sub(cost)?,
            })
        }
    };
    let blended_margin = blended_margin(&results)?;
    Ok(MatrixResult {
        tiers: results,
        part,
        blended_margin,
    })
}

/// Total profit over total sales, each tier weighted by its share and
/// using its markup as shown. `None` when no tier has a share.
fn blended_margin(tiers: &[TierResult]) -> Result<Option<Percent>, MatrixError> {
    if tiers.iter().all(|tier| tier.share.is_none()) {
        return Ok(None);
    }
    let (mut total, mut sales, mut profit) = (0_i128, Some(0_i128), Some(0_i128));
    for tier in tiers {
        let share = i128::from(tier.share.unwrap_or(Percent::ZERO).thousandths());
        let markup = i128::from(tier.markup.thousandths());
        total += share;
        let sold = product(&[share, HUNDRED + markup]);
        let made = product(&[share, markup]);
        sales = sales
            .zip(sold)
            .and_then(|(sum, sold)| sum.checked_add(sold));
        profit = profit
            .zip(made)
            .and_then(|(sum, made)| sum.checked_add(made));
    }
    if total != HUNDRED {
        let total = Percent::from_thousandths(i64::try_from(total).unwrap_or(i64::MAX));
        return Err(MatrixError::SharesNotHundred { total });
    }
    let profit = profit.and_then(|profit| profit.checked_mul(HUNDRED));
    let margin = div_round(profit, sales).ok_or(TooLarge)?;
    Ok(Some(Percent::from_thousandths(margin)))
}
