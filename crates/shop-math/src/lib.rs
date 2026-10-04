//! The arithmetic behind the shop calculators of Wenmar Open.
//!
//! Money is whole cents ([`Money`]). Nothing here uses floating point, and
//! no input causes a panic or an overflow: a field that cannot be read is
//! a [`ParseError`], and a result that does not fit is [`TooLarge`].
//!
//! The crate has no web code and reads no data. A page reads the text of
//! its form fields with the `parse` functions, calls one function for the
//! calculation, and shows the values of the result with their `Display`.

mod decimal;
mod money;

pub use decimal::{ParseError, TooLarge};
pub use money::Money;
