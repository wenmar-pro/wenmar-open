//! The vehicle catalog of a Wenmar Open data file: years, makes, models,
//! submodels and engines, with stable ids and free-text search.
//!
//! The modules here need no database. [`text`] and [`id`] define how names
//! become ids, and [`summary`] writes the one-line description of a vehicle.

pub mod id;
pub mod summary;
pub mod text;

pub use id::VehicleId;

/// The first model year in the catalog: the year the 17-character VIN became
/// standard.
pub const FIRST_YEAR: u16 = 1981;
