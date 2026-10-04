//! The arithmetic behind the shop calculators of Wenmar Open.
//!
//! Money is whole cents ([`Money`]), a percentage is whole thousandths of a
//! percent ([`Percent`]) and hours are whole hundredths of an hour
//! ([`Hours`]). Nothing here uses floating point, and no input causes a
//! panic or an overflow: a field that cannot be read is a [`ParseError`],
//! and a result that does not fit is [`TooLarge`].
//!
//! The crate has no web code and reads no data. A page reads the text of
//! its form fields with the `parse` functions, calls one function for the
//! calculation, and shows the values of the result with their `Display`.

mod decimal;
pub mod gross_profit;
pub mod invoice_tax;
pub mod labor_rate;
mod money;
pub mod parts_matrix;
mod percent;
mod quantity;
pub mod targets;

pub use decimal::{ParseError, TooLarge};
pub use money::Money;
pub use percent::Percent;
pub use quantity::{Hours, MAX_COUNT, parse_count};
