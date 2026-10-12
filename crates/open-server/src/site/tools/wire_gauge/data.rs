pub use crate::site::tools::canada_invoice_tax::rates::Source;

/// The unit a table's resistances are printed in.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Unit {
    OhmsPerKilometre,
    OhmsPerThousandFeet,
}

impl Unit {
    /// The unit as the page writes it after a resistance.
    pub fn label(self) -> &'static str {
        match self {
            Unit::OhmsPerKilometre => "\u{3a9}/km",
            Unit::OhmsPerThousandFeet => "\u{3a9} per 1000 ft",
        }
    }
}

/// One gauge of wire: its name as a shop says it, its resistance as the
/// source prints it, and that resistance in nano-ohms per metre.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Gauge {
    pub name: &'static str,
    pub resistance: &'static str,
    pub nano_ohms_per_metre: i64,
}

/// The resistance table of one kind of wire as the page shows it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct WireTable {
    pub name: &'static str,
    pub as_of: &'static str,
    pub confirmed: bool,
    pub missing: &'static [&'static str],
    pub notes: &'static [&'static str],
    pub conditions: &'static [&'static str],
    pub unit: Unit,
    pub gauges: &'static [Gauge], // from the thinnest to the thickest
    pub sources: &'static [Source],
}

include!(concat!(env!("OUT_DIR"), "/wire_resistance.rs"));
