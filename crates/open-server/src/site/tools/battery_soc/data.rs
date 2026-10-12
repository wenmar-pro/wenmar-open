pub use crate::site::tools::canada_invoice_tax::rates::Source;
pub use shop_math::battery_soc::Correction;

/// One kind of battery as the page shows it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Battery {
    pub key: &'static str,
    pub name: &'static str,
    pub as_of: &'static str,
    pub confirmed: bool,
    pub missing: &'static [&'static str],
    pub notes: &'static [&'static str],
    pub conditions: &'static [&'static str],
    pub rows: &'static [(i64, u32)], // millivolts and percent, from least to most
    pub correction: Option<Correction>,
    pub correction_sources: &'static [Source],
    pub sources: &'static [Source],
}

include!(concat!(env!("OUT_DIR"), "/battery_soc.rs"));
