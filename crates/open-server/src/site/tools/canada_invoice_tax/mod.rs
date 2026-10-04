//! `/tools/canada-invoice-tax`: the sales taxes and the new-tire fees on a
//! repair invoice in a Canadian province or territory.
//!
//! No rate, rule or fee is written here: each is in `data/rates/canada.toml`
//! and reaches this code through [`rates`].

pub mod form;
pub mod rates;
pub mod result;

/// The page's address, its heading and its one sentence.
pub const PATH: &str = "/tools/canada-invoice-tax";
pub const TITLE: &str = "Canadian invoice tax and tire fee calculator";
pub const SUMMARY: &str = "Works out the GST, HST, PST or QST and the new-tire fees on a repair invoice in a Canadian province or territory, and the invoice's total.";
