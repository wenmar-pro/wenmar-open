//! One test binary for the whole crate: each binary links the database
//! engine again, and disk space on the build machines is limited.

mod api_vehicles;
mod api_vin;
mod capture;
mod common;
mod db;
mod http;
mod limit;
mod log;
mod mcp;
mod openapi;
mod search;
mod search_speed;
mod serve;
mod vin_rows;
