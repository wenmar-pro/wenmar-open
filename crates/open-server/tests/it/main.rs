//! One test binary for the whole crate: each binary links the database
//! engine again, and disk space on the build machines is limited.

mod api_vehicles;
mod api_vin;
mod capture;
mod common;
mod db;
mod discovery;
mod http;
mod limit;
mod log;
mod mcp;
mod offline;
mod openapi;
mod search;
mod search_speed;
mod serve;
mod site;
mod site_assets;
mod site_catalog;
mod site_design;
mod site_guides;
mod site_head;
mod site_home;
mod site_labor_rate;
mod site_pages;
mod site_seo;
mod site_tools;
mod site_vin;
mod site_wmi;
mod vin_rows;
