//! `/tools/parts-matrix`: what a part sells for under a matrix of tiers,
//! and the margin the matrix makes on a shop's mix of parts.

pub mod doc;
pub mod form;
pub mod presets;
pub mod result;

/// The page's address, its heading and its one sentence.
pub const PATH: &str = "/tools/parts-matrix";
pub const TITLE: &str = "Parts markup matrix calculator";
pub const SUMMARY: &str = "Works out what a part sells for under a parts matrix, and the margin the matrix makes across a shop's mix of parts.";
