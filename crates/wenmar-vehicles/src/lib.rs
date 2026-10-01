//! The vehicle catalog of a Wenmar Open data file: years, makes, models,
//! submodels and engines, with stable ids and free-text search.
//!
//! [`Catalog`] answers the questions a vehicle-entry form asks, one step at
//! a time, and turns free text or a decoded VIN into catalog entries. It
//! reads through [`Source`], which is one method, so it does not care which
//! SQLite engine holds the data file. [`sqlite::SqliteSource`] is the
//! `rusqlite` one, behind the `sqlite` feature.
//!
//! [`text`], [`id`], [`summary`], [`index`] and [`parse`] need no database.

pub mod catalog;
pub mod id;
pub mod index;
pub mod parse;
pub mod schema;
pub mod search;
pub mod selection;
pub mod source;
pub mod sql;
pub mod summary;
pub mod text;
pub mod vin_rows;

pub use catalog::{Catalog, CatalogError, EngineOption, Entry, Make, Model, Submodel};
pub use id::VehicleId;
pub use index::{MakeIndex, MakeRef, Scope};
pub use selection::Selection;
pub use source::{Source, SourceError, Value};

#[cfg(feature = "sqlite")]
pub mod sqlite;

/// The first model year in the catalog: the year the 17-character VIN became
/// standard.
pub const FIRST_YEAR: u16 = 1981;
