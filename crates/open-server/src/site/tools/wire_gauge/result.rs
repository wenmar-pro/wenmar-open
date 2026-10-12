//! What the wire gauge page says and shows: the drop along each gauge in the
//! table, and the thinnest gauge that stays within the drop allowed.
//!
//! No resistance is written here. Each table comes from
//! `data/reference/wire-resistance.toml` through [`super::data::WIRE`], and
//! each drop is `shop_math`'s. Every function takes the table, so the tests
//! can use a made-up one.

use shop_math::voltage_drop::{Circuit, voltage_drop, volts};

use crate::site::markdown::Table;
use crate::site::tools::wire_gauge::data::WireTable;

pub const LIMITS: &str = "This is voltage drop only. A wire must also carry the current without overheating, which depends on its insulation, on how it is bundled and on the heat around it, and the circuit needs a fuse sized for the wire. Check the wire maker's rating. The length is every metre the current flows through: where a second wire brings the current back, count both.";
pub const NOT_CHECKED: &str = "Not yet checked. The resistance of each gauge has not been read from a published source yet, so nothing is worked out.";

/// What the page shows for a circuit: the drop along each gauge, and the
/// sentence that names the thinnest that stays within the drop allowed.
#[derive(Debug, Clone)]
pub struct Shown {
    pub table: Table,
    pub pick: String,
}

pub fn shown(circuit: &Circuit, wire: &WireTable) -> Shown {
    let mut rows = Vec::new();
    let mut pick = None;
    for gauge in wire.gauges {
        let Ok(drop) = voltage_drop(circuit, gauge.nano_ohms_per_metre) else {
            continue;
        };
        if drop.within && pick.is_none() {
            pick = Some(gauge.name);
        }
        rows.push(vec![
            gauge.name.to_owned(),
            format!("{} {}", gauge.resistance, wire.unit.label()),
            format!("{} V", volts(drop.millivolts)),
            drop.percent.to_string(),
            if drop.within { "Yes" } else { "No" }.to_owned(),
        ]);
    }
    let allowed = circuit.allowed;
    let pick = match pick {
        Some(name) => {
            format!("The thinnest wire in the table that stays within {allowed} is {name}.")
        }
        None => format!("No wire in the table stays within {allowed}."),
    };
    Shown {
        table: Table {
            caption: "Voltage drop for each gauge".to_owned(),
            head: vec![
                "Gauge".to_owned(),
                "Resistance".to_owned(),
                "Drop".to_owned(),
                format!("Percent of {} V", circuit.system_millivolts / 1_000),
                format!("Within {allowed}"),
            ],
            rows,
            prose: false,
        },
        pick,
    }
}

/// The day the table was read, as the page says it.
pub fn checked(wire: &WireTable) -> String {
    format!("Resistances for {} as of {}.", wire.name, wire.as_of)
}

/// The four lines that say how the result is worked out.
pub fn formulas() -> Vec<String> {
    vec![
        "Voltage drop = current \u{d7} resistance of the wire per metre \u{d7} length of wire.".to_owned(),
        "Percent = voltage drop \u{f7} system voltage, which is taken as 12 V or 24 V.".to_owned(),
        "A wire is within the drop allowed when its percent is no more than the percent allowed. One foot is 0.3048 metres.".to_owned(),
        "The resistance of each gauge is read from the sources listed. Each drop is rounded once, to a thousandth of a volt, and its percent is worked from the drop as shown.".to_owned(),
    ]
}

#[cfg(test)]
pub mod tests {
    use super::*;
    use crate::site::tools::wire_gauge::data::{Gauge, Source, Unit};
    use shop_math::Percent;

    /// Made up: no figure here is a real wire's.
    pub const FAKE: WireTable = WireTable {
        name: "Made-up wire",
        as_of: "2026-01-31",
        confirmed: true,
        missing: &[],
        notes: &[],
        conditions: &[],
        unit: Unit::OhmsPerKilometre,
        gauges: &[
            Gauge {
                name: "T1 (made up)",
                resistance: "10",
                nano_ohms_per_metre: 10_000_000,
            },
            Gauge {
                name: "T2 (made up)",
                resistance: "5",
                nano_ohms_per_metre: 5_000_000,
            },
            Gauge {
                name: "T3 (made up)",
                resistance: "2",
                nano_ohms_per_metre: 2_000_000,
            },
        ],
        sources: &[Source {
            title: "A made-up page",
            url: "https://example.org/wire",
            archived: None,
        }],
    };

    fn rows(shown: &Shown) -> Vec<String> {
        shown.table.rows.iter().map(|row| row.join(" | ")).collect()
    }

    #[test]
    fn every_gauge_is_shown_and_the_thinnest_that_stays_within_is_named() {
        let circuit = Circuit {
            current_tenths_amp: 200,
            length_micrometres: 5_000_000,
            system_millivolts: 12_000,
            allowed: Percent::whole(3),
        };
        let at_12 = shown(&circuit, &FAKE);
        assert_eq!(
            at_12.table.head,
            [
                "Gauge",
                "Resistance",
                "Drop",
                "Percent of 12 V",
                "Within 3%"
            ]
        );
        assert_eq!(
            rows(&at_12),
            [
                "T1 (made up) | 10 \u{3a9}/km | 1.000 V | 8.333% | No",
                "T2 (made up) | 5 \u{3a9}/km | 0.500 V | 4.167% | No",
                "T3 (made up) | 2 \u{3a9}/km | 0.200 V | 1.667% | Yes",
            ]
        );
        assert_eq!(
            at_12.pick,
            "The thinnest wire in the table that stays within 3% is T3 (made up)."
        );
        let at_24 = shown(
            &Circuit {
                system_millivolts: 24_000,
                ..circuit
            },
            &FAKE,
        );
        assert_eq!(
            rows(&at_24)[1],
            "T2 (made up) | 5 \u{3a9}/km | 0.500 V | 2.083% | Yes"
        );
        assert_eq!(
            at_24.pick,
            "The thinnest wire in the table that stays within 3% is T2 (made up)."
        );
        let feet = shown(
            &Circuit {
                current_tenths_amp: 155,
                length_micrometres: 3_048_000,
                ..circuit
            },
            &FAKE,
        );
        assert_eq!(
            rows(&feet),
            [
                "T1 (made up) | 10 \u{3a9}/km | 0.472 V | 3.933% | No",
                "T2 (made up) | 5 \u{3a9}/km | 0.236 V | 1.967% | Yes",
                "T3 (made up) | 2 \u{3a9}/km | 0.094 V | 0.783% | Yes",
            ]
        );
        let strict = shown(
            &Circuit {
                allowed: Percent::from_thousandths(100),
                ..circuit
            },
            &FAKE,
        );
        assert_eq!(strict.pick, "No wire in the table stays within 0.1%.");
        assert_eq!(
            checked(&FAKE),
            "Resistances for Made-up wire as of 2026-01-31."
        );
        assert_eq!(formulas().len(), 4);
    }
}
