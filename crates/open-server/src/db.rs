//! The data file, read through `rusqlite`.
//!
//! The code is in the `wenmar-open-db` crate, which other programs use to
//! read a data file in-process. This module keeps the names the rest of the
//! server uses.

pub use wenmar_open_db::{Db, DbError, Meta, Worker};
