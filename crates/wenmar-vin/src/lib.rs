//! VIN decoding for auto repair shops.

pub mod error;
pub mod vin;

pub use error::{InvalidChar, VinError};
pub use vin::Vin;
