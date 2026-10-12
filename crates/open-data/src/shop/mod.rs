//! Builds the shop file: the datasets beside the catalog, such as recalls.
//!
//! `build` creates the file. Each source has a module of its own with a
//! `parse` function (file to rows, no database) and a `shape` function
//! (rows to tables).

pub mod build;
pub mod coverage;
pub mod matcher;
pub mod nhtsa_recalls;
pub mod tc_recalls;
