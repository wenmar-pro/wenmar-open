//! Builds the shop file: the datasets beside the catalog, such as recalls.
//!
//! `build` creates the file. Each source has a module of its own with a
//! `parse` function (file to rows, no database) and a `shape` function
//! (rows to tables).

pub mod build;
pub mod coverage;
pub mod epa_guide;
pub mod epa_match;
pub mod epa_vehicles;
pub mod matcher;
pub mod nhtsa_investigations;
pub mod nhtsa_ratings;
pub mod nhtsa_recall_reports;
pub mod nhtsa_recalls;
pub mod nhtsa_vin_guides;
pub mod oil_grade;
pub mod recalls;
pub mod tc_recalls;
pub mod xlsx;
