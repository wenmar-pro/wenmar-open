//! A Wenmar Open data file, read through the `rusqlite` crate.
//!
//! [`Db::open`] opens a data file read-only. Everything that reads it runs
//! on tokio's blocking thread pool, through [`Db::run`], so an async caller
//! is never held up by a query.
//!
//! The file is only ever read. It is opened read-only, so no journal, WAL
//! or lock file is created beside it, a write is refused, and SQLite takes
//! no lock other readers would notice: other programs may read the same
//! file at the same time. Nothing in this crate hands out a
//! `rusqlite::Connection`.
//!
//! A `rusqlite::Connection` is `Send` but not `Sync`: it may move between
//! threads but may not be shared between tasks. That is why this crate
//! keeps a fixed pool of connections behind [`Db::run`], and why every
//! query has a connection of its own for as long as it runs.
//!
//! ```no_run
//! # async fn example() -> Result<(), Box<dyn std::error::Error>> {
//! use std::path::Path;
//! use wenmar_open_db::Db;
//! use wenmar_vehicles::Scope;
//! use wenmar_vin::DecodeOptions;
//!
//! let db = Db::open(Path::new("wenmar-open-2026.09.sqlite3"), 4).await?;
//! let decode = db.decode_vin("KM8K2CAB4PU001140", DecodeOptions::default()).await?;
//! println!("{:?} {:?}", decode.decoded.year, decode.decoded.make);
//!
//! let makes = db.catalog(|catalog| catalog.makes(Some(2019), Scope::Light, "", 50)).await??;
//! println!("{} makes", makes.len());
//! # Ok(())
//! # }
//! ```

pub mod db;
pub mod decode;
pub mod vin_rows;

pub use db::{Db, DbError, Meta, Worker};
pub use decode::{Decode, DecodeFailure, LONGEST_INPUT, current_year};
