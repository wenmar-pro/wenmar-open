//! A Wenmar Open shop file: the datasets beside the vehicle catalog, such
//! as recalls, each matched to catalog ids.
//!
//! [`ShopData`] reads through [`wenmar_vehicles::Source`], so it does not
//! care which SQLite engine holds the file. [`sqlite::ShopSource`] is the
//! `rusqlite` one, behind the `sqlite` feature.

pub mod answer;
pub mod bulletins;
pub mod codes;
pub mod complaints;
pub mod dataset;
pub mod error;
pub mod fuel;
pub mod hand;
pub mod investigations;
pub mod licence;
pub mod ratings;
pub mod recall_reports;
pub mod recalls;
pub mod schema;
pub mod shop;
pub mod siblings;
pub mod vin_guides;

pub use dataset::{Dataset, DatasetEntry, Datasets};
pub use error::ShopError;
pub use shop::{ShopData, ShopMeta, check_pairing};

#[cfg(feature = "sqlite")]
pub mod sqlite;
