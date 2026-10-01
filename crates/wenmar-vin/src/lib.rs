//! VIN decoding for auto repair shops.

pub mod error;
pub mod vin;

pub use error::{InvalidChar, VinError};
pub use vin::Vin;
pub mod check_digit;
pub use check_digit::CheckDigit;
pub mod model_year;
pub mod pattern;
pub mod suggest;
