//! `wenmar-vin` and `wenmar-vehicles` behind one call that takes JSON and
//! gives JSON: what the `wenmar-open` npm package runs, as WebAssembly, when
//! it works offline.
//!
//! The two libraries read a database in the middle of their work, and a
//! WebAssembly call cannot wait for a database that answers later. So the
//! database here is [`Replay`]: it holds the rows the caller has already
//! read, and a statement it has no rows for is written down and answered
//! with nothing. [`Engine::call`] then says which statements it needs, the
//! caller runs them with whatever database it has, and calls again.
//!
//! The crate has no clock, no file and no network. The current year is in
//! every request.

pub mod abi;
pub mod cells;
pub mod engine;
pub mod error;
pub mod ops;
pub mod replay;

pub use engine::Engine;
pub use error::OpError;
pub use replay::{Replay, Statement};

/// The data file layout this build reads.
pub const SCHEMA_VERSION: &str = wenmar_vehicles::schema::SCHEMA_VERSION;
