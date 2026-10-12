//! What the refrigerant chart says and shows: the table of one refrigerant,
//! the units it was read in, and how the other units are worked out.
//!
//! No figure is written here. Each one comes from `data/reference/refrigerant-pt.toml`
//! through [`data::REFRIGERANTS`], and each conversion is `shop_math::chart`'s.

use shop_math::chart::{
    celsius_from_fahrenheit, fahrenheit_from_celsius, kpa_from_psi, psi_from_kpa, show_tenths,
};

use crate::site::markdown::Table;
use crate::site::tools::refrigerant_pt::data::{Degrees, Pressure, Reading, Refrigerant};

pub const LIMITS: &str = "Each pressure is that of the refrigerant alone, at rest, at the temperature beside it. A gauge on a running system reads something else, and air or another refrigerant in the system changes the reading. This chart is not a charging procedure: follow the vehicle maker's.";

/// What the page says while a refrigerant has no checked table.
pub fn not_checked(refrigerant: &Refrigerant) -> String {
    format!(
        "Not yet checked. No figure is shown for {} until each one has been read from a published source.",
        refrigerant.name
    )
}

/// The day the table was read, as the page says it.
pub fn checked(refrigerant: &Refrigerant) -> String {
    format!(
        "Figures for {} as of {}.",
        refrigerant.name, refrigerant.as_of
    )
}

/// Which units the source printed, and which columns are worked from them.
pub fn units(refrigerant: &Refrigerant) -> String {
    let (given_temperature, other_temperature) = match refrigerant.temperature {
        Degrees::Celsius => ("\u{b0}C", "\u{b0}F"),
        Degrees::Fahrenheit => ("\u{b0}F", "\u{b0}C"),
    };
    let (given_pressure, other_pressure) = match refrigerant.pressure {
        Pressure::Kilopascals => ("kPa", "psi"),
        Pressure::PoundsPerSquareInch => ("psi", "kPa"),
    };
    format!(
        "The source gives {given_temperature} and {given_pressure}. The {other_temperature} and {other_pressure} columns are worked from them and rounded to one decimal place."
    )
}

/// The table of one refrigerant: each row of the entry, in four columns.
pub fn table(refrigerant: &Refrigerant) -> Table {
    let reading = match refrigerant.reading {
        Reading::Gauge => "gauge",
        Reading::Absolute => "absolute",
    };
    let rows = refrigerant
        .rows
        .iter()
        .map(|&(temperature, pressure)| {
            let (celsius, fahrenheit) = match refrigerant.temperature {
                Degrees::Celsius => (temperature, fahrenheit_from_celsius(temperature)),
                Degrees::Fahrenheit => (celsius_from_fahrenheit(temperature), temperature),
            };
            let (kpa, psi) = match refrigerant.pressure {
                Pressure::Kilopascals => (pressure, psi_from_kpa(pressure)),
                Pressure::PoundsPerSquareInch => (kpa_from_psi(pressure), pressure),
            };
            vec![
                show_tenths(celsius),
                show_tenths(fahrenheit),
                show_tenths(kpa),
                show_tenths(psi),
            ]
        })
        .collect();
    Table {
        caption: format!("{}: pressure at each temperature", refrigerant.name),
        head: vec![
            "\u{b0}C".to_owned(),
            "\u{b0}F".to_owned(),
            format!("kPa ({reading})"),
            format!("psi ({reading})"),
        ],
        rows,
        prose: false,
    }
}

/// The three lines that say how the columns are worked out.
pub fn conversions() -> Vec<String> {
    vec![
        "\u{b0}F = \u{b0}C \u{d7} 9 \u{f7} 5 + 32, and \u{b0}C = (\u{b0}F \u{2212} 32) \u{d7} 5 \u{f7} 9.".to_owned(),
        "1 psi = 6.894757 kPa.".to_owned(),
        "A gauge pressure is what a gauge reads above the air around it; an absolute pressure is measured from a vacuum. Each column's heading says which it is. No figure is worked out between two rows: the rows are the ones the source gives.".to_owned(),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::site::tools::refrigerant_pt::data::{
        Degrees, Pressure, Reading, Refrigerant, Source,
    };

    /// Made up: no figure here is a real pressure.
    const FAKE: Refrigerant = Refrigerant {
        key: "x000",
        name: "X-000 (made up)",
        as_of: "2026-01-31",
        confirmed: true,
        missing: &[],
        notes: &[],
        temperature: Degrees::Celsius,
        pressure: Pressure::Kilopascals,
        reading: Reading::Gauge,
        rows: &[(-100, 0), (0, 1_000), (100, 2_000)],
        sources: &[Source {
            title: "A made-up page",
            url: "https://example.org/pt",
            archived: None,
        }],
    };

    fn rows(table: &Table) -> Vec<String> {
        table.rows.iter().map(|row| row.join(" | ")).collect()
    }

    #[test]
    fn a_table_shows_the_sources_figures_and_the_converted_ones() {
        let table = table(&FAKE);
        assert_eq!(
            table.caption,
            "X-000 (made up): pressure at each temperature"
        );
        assert_eq!(
            table.head,
            ["\u{b0}C", "\u{b0}F", "kPa (gauge)", "psi (gauge)"]
        );
        assert_eq!(
            rows(&table),
            [
                "-10.0 | 14.0 | 0.0 | 0.0",
                "0.0 | 32.0 | 100.0 | 14.5",
                "10.0 | 50.0 | 200.0 | 29.0"
            ]
        );
        assert_eq!(
            units(&FAKE),
            "The source gives \u{b0}C and kPa. The \u{b0}F and psi columns are worked from them and rounded to one decimal place."
        );
        let other = Refrigerant {
            temperature: Degrees::Fahrenheit,
            pressure: Pressure::PoundsPerSquareInch,
            reading: Reading::Absolute,
            rows: &[(320, 100), (500, 1_000)],
            ..FAKE
        };
        let table = super::table(&other);
        assert_eq!(
            table.head,
            ["\u{b0}C", "\u{b0}F", "kPa (absolute)", "psi (absolute)"]
        );
        assert_eq!(
            rows(&table),
            ["0.0 | 32.0 | 68.9 | 10.0", "10.0 | 50.0 | 689.5 | 100.0"]
        );
        assert_eq!(
            units(&other),
            "The source gives \u{b0}F and psi. The \u{b0}C and kPa columns are worked from them and rounded to one decimal place."
        );
        assert_eq!(
            checked(&FAKE),
            "Figures for X-000 (made up) as of 2026-01-31."
        );
        assert_eq!(
            not_checked(&FAKE),
            "Not yet checked. No figure is shown for X-000 (made up) until each one has been read from a published source."
        );
    }
}
