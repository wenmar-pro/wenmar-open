//! What the battery page says and shows: the reading, the temperature
//! correction, where the voltage falls in the table, and the table itself.
//!
//! No figure is written here. Each table and each correction comes from
//! `data/reference/battery-soc.toml` through [`super::data::BATTERIES`], and
//! each conversion and correction is `shop_math`'s.

use shop_math::Percent;
use shop_math::battery_soc::{corrected, volts};
use shop_math::chart::{Place, fahrenheit_from_celsius, place, show_tenths};

use crate::site::markdown::Table;
use crate::site::tools::battery_soc::data::Battery;
use crate::site::tools::battery_soc::form::Reading;

pub const LIMITS: &str = "A voltage says how charged a battery is, not how healthy it is: a worn battery can read full. The table is for the kind of battery named. A battery that was just charged or just used does not read true.";
pub const NONE_CHECKED: &str = "Not yet checked. No table has been read from a published source yet, so nothing is worked out.";

/// What the page shows for a reading.
#[derive(Debug, Clone)]
pub struct Shown {
    /// What was entered, and the corrected voltage when there is one.
    pub reading: Table,
    /// What was done about the temperature.
    pub correction: String,
    /// Where the voltage falls in the table.
    pub finding: String,
}

/// A voltage in millivolts, as the page writes it with its unit.
fn v(millivolts: i64) -> String {
    format!("{} V", volts(millivolts))
}

/// A percent of charge, as the page writes it.
fn p(percent: u32) -> String {
    Percent::whole(i64::from(percent)).to_string()
}

pub fn shown(reading: &Reading) -> Shown {
    let battery = reading.battery;
    let fixed = match (reading.tenths_c, battery.correction) {
        (Some(tenths), Some(correction)) => corrected(reading.millivolts, tenths, &correction),
        _ => None,
    };
    let used = fixed.unwrap_or(reading.millivolts);

    let mut rows = vec![vec!["Voltage read".to_owned(), v(reading.millivolts)]];
    if let Some(tenths) = reading.tenths_c {
        rows.push(vec![
            "Battery temperature".to_owned(),
            format!(
                "{} \u{b0}C ({} \u{b0}F)",
                show_tenths(tenths),
                show_tenths(fahrenheit_from_celsius(tenths))
            ),
        ]);
    }
    if let (Some(value), Some(correction)) = (fixed, battery.correction) {
        rows.push(vec![
            format!(
                "Voltage corrected to {} \u{b0}C",
                show_tenths(correction.reference_tenths_c)
            ),
            v(value),
        ]);
    }
    let reading_table = Table {
        caption: "The reading".to_owned(),
        head: vec!["Figure".to_owned(), "Amount".to_owned()],
        rows,
        prose: false,
    };

    let correction = match (reading.tenths_c, battery.correction) {
        (Some(_), Some(correction)) => format!(
            "The table was measured at {} \u{b0}C. The source adds {} mV for each degree Celsius the battery is below that, and takes the same off for each degree above.",
            show_tenths(correction.reference_tenths_c),
            volts(correction.microvolts_per_degree)
        ),
        (Some(_), None) => "No temperature correction has been read from a source for this kind of battery, so the reading is used as it is.".to_owned(),
        (None, _) => "No temperature was entered, so the reading is used as it is.".to_owned(),
    };

    let millivolts: Vec<i64> = battery
        .rows
        .iter()
        .map(|&(millivolts, _)| millivolts)
        .collect();
    let finding = match place(&millivolts, used) {
        Some(Place::At(index)) => format!(
            "{} is on the table's row for {}.",
            v(used),
            p(battery.rows[index].1)
        ),
        Some(Place::Between(low, high)) => format!(
            "{} is between the table's rows for {} ({}) and {} ({}).",
            v(used),
            v(battery.rows[low].0),
            p(battery.rows[low].1),
            v(battery.rows[high].0),
            p(battery.rows[high].1)
        ),
        Some(Place::Below) => {
            let (first, percent) = battery.rows[0];
            format!(
                "{} is below the table's lowest row, {} ({}).",
                v(used),
                v(first),
                p(percent)
            )
        }
        Some(Place::Above) => {
            let (last, percent) = battery.rows[battery.rows.len() - 1];
            format!(
                "{} is above the table's highest row, {} ({}).",
                v(used),
                v(last),
                p(percent)
            )
        }
        None => "The table has no rows.".to_owned(),
    };

    Shown {
        reading: reading_table,
        correction,
        finding,
    }
}

/// The whole table of a battery: each row's voltage and percent.
pub fn chart(battery: &Battery) -> Table {
    Table {
        caption: format!("{}: state of charge by open-circuit voltage", battery.name),
        head: vec![
            "Open-circuit voltage".to_owned(),
            "State of charge".to_owned(),
        ],
        rows: battery
            .rows
            .iter()
            .map(|&(millivolts, percent)| vec![v(millivolts), p(percent)])
            .collect(),
        prose: false,
    }
}

/// The day the table was read, as the page says it.
pub fn checked(battery: &Battery) -> String {
    format!("Table for {} as of {}.", battery.name, battery.as_of)
}

/// The three lines that say how the result is worked out.
pub fn formulas() -> Vec<String> {
    vec![
        "Corrected voltage = voltage read + correction per degree \u{d7} (table temperature \u{2212} battery temperature), in degrees Celsius. \u{b0}C = (\u{b0}F \u{2212} 32) \u{d7} 5 \u{f7} 9.".to_owned(),
        "The corrected voltage is then found in the table. No percent is worked out between two rows: the page names the row the voltage is on, or the two rows it is between.".to_owned(),
        "The table and the correction are read from the sources listed, and each voltage is rounded once, to a thousandth of a volt.".to_owned(),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::site::tools::battery_soc::form::tests::FAKES;

    #[test]
    fn a_reading_is_corrected_and_found_in_the_table() {
        let at = |millivolts, tenths_c| {
            shown(&Reading {
                battery: &FAKES[0],
                millivolts,
                tenths_c,
            })
        };
        let rows = |table: &Table| -> Vec<String> {
            table.rows.iter().map(|row| row.join(" | ")).collect()
        };
        let cold = at(11_400, Some(50));
        assert_eq!(
            rows(&cold.reading),
            [
                "Voltage read | 11.40 V",
                "Battery temperature | 5.0 \u{b0}C (41.0 \u{b0}F)",
                "Voltage corrected to 25.0 \u{b0}C | 11.60 V"
            ]
        );
        assert_eq!(
            cold.correction,
            "The table was measured at 25.0 \u{b0}C. The source adds 10.00 mV for each degree Celsius the battery is below that, and takes the same off for each degree above."
        );
        assert_eq!(
            cold.finding,
            "11.60 V is between the table's rows for 11.00 V (50%) and 12.00 V (100%)."
        );
        let plain = at(11_000, None);
        assert_eq!(rows(&plain.reading), ["Voltage read | 11.00 V"]);
        assert_eq!(
            plain.correction,
            "No temperature was entered, so the reading is used as it is."
        );
        assert_eq!(plain.finding, "11.00 V is on the table's row for 50%.");
        assert_eq!(
            at(9_500, None).finding,
            "9.50 V is below the table's lowest row, 10.00 V (0%)."
        );
        assert_eq!(
            at(12_500, None).finding,
            "12.50 V is above the table's highest row, 12.00 V (100%)."
        );
        assert_eq!(
            at(12_050, Some(350)).finding,
            "11.95 V is between the table's rows for 11.00 V (50%) and 12.00 V (100%)."
        );
        let bare = Battery {
            correction: None,
            ..FAKES[0]
        };
        let uncorrected = shown(&Reading {
            battery: Box::leak(Box::new(bare)),
            millivolts: 11_400,
            tenths_c: Some(50),
        });
        assert_eq!(uncorrected.reading.rows.len(), 2);
        assert!(
            uncorrected
                .correction
                .starts_with("No temperature correction has been read")
        );
        assert_eq!(
            rows(&chart(&FAKES[0])),
            ["10.00 V | 0%", "11.00 V | 50%", "12.00 V | 100%"]
        );
        assert_eq!(
            checked(&FAKES[0]),
            "Table for Made-up battery as of 2026-01-31."
        );
    }
}
