//! Builds the shop file: the datasets beside the catalog, such as recalls.
//!
//! `build` creates the file. Each source has a module of its own with a
//! `parse` function (file to rows, no database) and a `shape` function
//! (rows to tables).

pub mod build;
pub mod bulletin_text;
pub mod complaints;
pub mod components;
pub mod coverage;
pub mod epa_emissions;
pub mod epa_guide;
pub mod epa_match;
pub mod epa_vehicles;
pub mod fuel;
pub mod hand;
pub mod hand_coverage;
pub mod matcher;
pub mod mfr_comms;
pub mod nhtsa_investigations;
pub mod nhtsa_ratings;
pub mod nhtsa_recall_reports;
pub mod nhtsa_recalls;
pub mod nhtsa_vin_guides;
pub mod nrcan_fuel;
pub mod oil_grade;
pub mod recalls;
pub mod siblings;
pub mod tc_recalls;
pub mod xlsx;
