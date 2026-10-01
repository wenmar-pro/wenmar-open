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

pub mod db;
pub mod vin_rows;

pub use db::{Db, DbError, Meta, TursoSource, Worker};
