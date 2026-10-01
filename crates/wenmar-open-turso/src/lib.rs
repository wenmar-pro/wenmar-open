//! A Wenmar Open data file, read through the `turso` crate.
//!
//! [`Db::open`] opens a data file read-only. Everything that reads it runs
//! on tokio's blocking thread pool, through [`Db::run`], so an async caller
//! is never held up by a query.
//!
//! The file is only ever read. It is opened with turso's `read_only`
//! setting, which creates no journal, WAL or lock file beside it and takes
//! no lock, so other programs may read the same file at the same time.
//! Nothing in this crate hands out a turso connection.
//!
//! turso does not honour SQLite's file locks. Never rewrite a data file in
//! place while it is open: write the new file beside it, rename it over the
//! old one, and open it again.
//!
//! ```no_run
//! # async fn example() -> Result<(), Box<dyn std::error::Error>> {
//! use std::path::Path;
//! use wenmar_open_turso::Db;
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

pub use db::{Db, DbError, Meta, TursoSource, Worker};
pub use decode::{Decode, DecodeFailure, LONGEST_INPUT, current_year};
