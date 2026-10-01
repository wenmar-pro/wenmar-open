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
mod site;
mod site_catalog;
mod site_home;
mod site_vin;
mod vin_rows;
