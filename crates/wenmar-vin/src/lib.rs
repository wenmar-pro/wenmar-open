//! VIN decoding for auto repair shops.

pub mod error;
pub mod vin;

pub use error::{InvalidChar, VinError};
pub use vin::Vin;
pub mod check_digit;
pub use check_digit::CheckDigit;
pub mod data;
pub mod memory;
pub mod model_year;
pub mod pattern;
pub mod suggest;

pub use data::{DataError, Element, Manufacturer, Pattern, VinData};
pub use memory::MemoryData;
