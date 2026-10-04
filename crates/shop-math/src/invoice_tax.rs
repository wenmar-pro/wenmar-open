//! The taxes and the tire fees on a repair invoice.
//!
//! This module holds no rate, no fee and no rule of any place: each comes
//! in as data. What it does with them:
//!
//! - subtotal = labour + parts + shop supplies
//! - a tire fee line = the fee on one tire × the number of new tires
//! - a tax's base = each of labour, parts, shop supplies and the tire
//!   fees that the tax applies to, added together
//! - a tax = its base × its rate
//! - total = subtotal + the tire fee lines + the taxes
//!
//! A tax is never calculated on another tax. Every amount but a tax is
//! exact to the cent. Each tax is rounded once, on its whole base, to the
//! nearest cent with a half cent going up, and the total is the sum of the
//! lines as they are shown.

use crate::decimal::{TooLarge, div_round, product};
use crate::money::Money;
use crate::percent::Percent;

/// Thousandths of a percent in the whole.
const HUNDRED: i128 = 100_000;

/// One tax: its rate, and which lines of an invoice it is charged on.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Tax {
    pub rate: Percent,
    pub on_labour: bool,
    pub on_parts: bool,
    pub on_supplies: bool,
    /// Whether the tax is charged on the fee on a new tire.
    pub on_tire_fee: bool,
}

/// New tires of one class: the fee on each, and how many were sold.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Tires {
    pub fee: Money,
    pub count: u32,
}

/// A repair invoice before fees and taxes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Invoice<'a> {
    pub labour: Money,
    pub parts: Money,
    /// The shop supplies charge. Zero when there is none.
    pub supplies: Money,
    /// The new tires sold, one entry for each class.
    pub tires: &'a [Tires],
}

/// A field of the invoice form.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InvoiceField {
    Labour,
    Parts,
    Supplies,
}

/// Why the invoice was not worked out. [`InvoiceTaxError::field`] says
/// which field to put the message beside; without one it belongs at the
/// form.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
#[non_exhaustive]
pub enum InvoiceTaxError {
    #[error("Enter labour, parts, shop supplies or a number of new tires.")]
    Nothing,
    #[error("This cannot be below zero.")]
    BelowZero { field: InvoiceField },
    /// The rates given were not ones to calculate with.
    #[error("A rate or a fee is below zero.")]
    Rule,
    #[error(transparent)]
    TooLarge(#[from] TooLarge),
}

impl InvoiceTaxError {
    pub fn field(&self) -> Option<InvoiceField> {
        match self {
            InvoiceTaxError::BelowZero { field } => Some(*field),
            _ => None,
        }
    }
}

/// The fee on the new tires of one class.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FeeLine {
    pub fee: Money,
    pub count: u32,
    /// The fee × the count.
    pub amount: Money,
}

/// One tax as an invoice shows it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TaxLine {
    pub rate: Percent,
    /// What the tax was calculated on.
    pub base: Money,
    pub amount: Money,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InvoiceTax {
    /// Labour, parts and shop supplies.
    pub subtotal: Money,
    /// One line for each entry of [`Invoice::tires`], in the same order. A
    /// class with no tires has a line of nothing.
    pub fees: Vec<FeeLine>,
    /// One line for each tax given, in the same order.
    pub taxes: Vec<TaxLine>,
    /// The subtotal, the fee lines and the tax lines added together.
    pub total: Money,
}

/// The fee lines, the tax lines and the total of an invoice under `taxes`.
pub fn invoice_tax(taxes: &[Tax], invoice: &Invoice<'_>) -> Result<InvoiceTax, InvoiceTaxError> {
    for (amount, field) in [
        (invoice.labour, InvoiceField::Labour),
        (invoice.parts, InvoiceField::Parts),
        (invoice.supplies, InvoiceField::Supplies),
    ] {
        if amount < Money::ZERO {
            return Err(InvoiceTaxError::BelowZero { field });
        }
    }
    if invoice.tires.iter().any(|tires| tires.fee < Money::ZERO)
        || taxes.iter().any(|tax| tax.rate < Percent::ZERO)
    {
        return Err(InvoiceTaxError::Rule);
    }
    let subtotal = invoice
        .labour
        .checked_add(invoice.parts)?
        .checked_add(invoice.supplies)?;
    let mut fees = Vec::with_capacity(invoice.tires.len());
    let mut fee_total = Money::ZERO;
    for tires in invoice.tires {
        let amount = product(&[i128::from(tires.fee.cents()), i128::from(tires.count)]);
        let amount = div_round(amount, Some(1)).ok_or(TooLarge)?;
        let amount = Money::from_cents(amount);
        fee_total = fee_total.checked_add(amount)?;
        fees.push(FeeLine {
            fee: tires.fee,
            count: tires.count,
            amount,
        });
    }
    if subtotal == Money::ZERO && invoice.tires.iter().all(|tires| tires.count == 0) {
        return Err(InvoiceTaxError::Nothing);
    }
    let mut total = subtotal.checked_add(fee_total)?;
    let mut lines = Vec::with_capacity(taxes.len());
    for tax in taxes {
        let mut base = Money::ZERO;
        for (applies, amount) in [
            (tax.on_labour, invoice.labour),
            (tax.on_parts, invoice.parts),
            (tax.on_supplies, invoice.supplies),
            (tax.on_tire_fee, fee_total),
        ] {
            if applies {
                base = base.checked_add(amount)?;
            }
        }
        let amount = product(&[i128::from(base.cents()), i128::from(tax.rate.thousandths())]);
        let amount = div_round(amount, Some(HUNDRED)).ok_or(TooLarge)?;
        let amount = Money::from_cents(amount);
        total = total.checked_add(amount)?;
        lines.push(TaxLine {
            rate: tax.rate,
            base,
            amount,
        });
    }
    Ok(InvoiceTax {
        subtotal,
        fees,
        taxes: lines,
        total,
    })
}
