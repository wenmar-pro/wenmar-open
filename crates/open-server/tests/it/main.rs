//! One test binary for the whole crate: each binary links the database
//! engine again, and disk space on the build machines is limited.

mod common;
mod db;
mod http;
mod limit;
mod openapi;
mod vin_rows;
