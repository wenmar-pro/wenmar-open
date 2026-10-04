//! `/tools/gross-profit`: what labor, parts and sublet made in a period,
//! each beside its target.

pub mod form;
pub mod result;

/// The page's address, its heading and its one sentence.
pub const PATH: &str = "/tools/gross-profit";
pub const TITLE: &str = "Gross profit calculator for auto repair shops";
pub const SUMMARY: &str = "Works out a repair shop's gross profit on labor, on parts, on sublet work and on all three together, and shows labor, parts and the overall figure beside a target.";
