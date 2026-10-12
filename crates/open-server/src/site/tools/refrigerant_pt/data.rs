pub use crate::site::tools::canada_invoice_tax::rates::Source;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Degrees {
    Celsius,
    Fahrenheit,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Pressure {
    Kilopascals,
    PoundsPerSquareInch,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Reading {
    Gauge,
    Absolute,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Refrigerant {
    pub key: &'static str,
    pub name: &'static str,
    pub as_of: &'static str,
    pub confirmed: bool,
    pub missing: &'static [&'static str],
    pub notes: &'static [&'static str],
    pub temperature: Degrees, // the unit the source prints
    pub pressure: Pressure,
    pub reading: Reading,
    pub rows: &'static [(i64, i64)], // temperature and pressure, in tenths
    pub sources: &'static [Source],
}

include!(concat!(env!("OUT_DIR"), "/refrigerant_pt.rs"));
