//! One test binary for the whole crate: each binary links the database
//! engine again, and disk space on the build machines is limited.

mod api_vehicles;
mod api_vin;
mod common;
mod db;
mod http;
mod limit;
mod mcp;
mod openapi;
mod search;
mod vin_rows;
