//! Compares this project's decodes with NHTSA's recorded answers.

use std::collections::BTreeMap;
use std::fmt::Write as _;

use serde::{Deserialize, Serialize};
use wenmar_vin::{DecodeOptions, Decoded, Decoder, VinData};

/// NHTSA's answers for a corpus of VINs, as recorded by `tools/corpus/record.py`.
#[derive(Debug, Clone, Deserialize)]
pub struct Fixtures {
    /// The calendar year the answers were recorded in. Decoding uses it as
    /// the current year, so the corpus decodes the same whenever it is run.
    pub recorded_year: u16,
    /// For each VIN, the non-empty fields of NHTSA's flat response.
    pub vins: BTreeMap<String, BTreeMap<String, String>>,
}

/// How one field compared across the corpus.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Tally {
    /// Both have the same value.
    pub agree: u32,
    /// Both have a value and they are not the same.
    pub differ: u32,
    /// NHTSA has a value and we do not.
    pub missing: u32,
    /// We have a value and NHTSA does not.
    pub extra: u32,
    /// Neither has a value.
    pub both_empty: u32,
}

/// Agreement with NHTSA over a corpus.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Report {
    pub vins: u32,
    /// VINs this project could not decode at all.
    pub decode_errors: u32,
    pub fields: BTreeMap<String, Tally>,
}

#[derive(Clone, Copy)]
enum Kind {
    Text,
    /// NHTSA spells drive types out; we keep the part before the slash.
    Drive,
    /// Compared after rounding to one decimal place.
    Litres,
}

struct Field {
    name: &'static str,
    nhtsa: &'static str,
    kind: Kind,
    ours: fn(&Decoded) -> Option<String>,
}

fn text<T: ToString>(value: Option<T>) -> Option<String> {
    value.map(|value| value.to_string())
}

const FIELDS: &[Field] = &[
    Field {
        name: "year",
        nhtsa: "ModelYear",
        kind: Kind::Text,
        ours: |d| text(d.year),
    },
    Field {
        name: "make",
        nhtsa: "Make",
        kind: Kind::Text,
        ours: |d| d.make.clone(),
    },
    Field {
        name: "model",
        nhtsa: "Model",
        kind: Kind::Text,
        ours: |d| d.model.clone(),
    },
    Field {
        name: "series",
        nhtsa: "Series",
        kind: Kind::Text,
        ours: |d| d.series.clone(),
    },
    Field {
        name: "trim",
        nhtsa: "Trim",
        kind: Kind::Text,
        ours: |d| d.trim.clone(),
    },
    Field {
        name: "body",
        nhtsa: "BodyClass",
        kind: Kind::Text,
        ours: |d| d.body.clone(),
    },
    Field {
        name: "doors",
        nhtsa: "Doors",
        kind: Kind::Text,
        ours: |d| text(d.doors),
    },
    Field {
        name: "drivetrain",
        nhtsa: "DriveType",
        kind: Kind::Drive,
        ours: |d| d.drivetrain.clone(),
    },
    Field {
        name: "transmission",
        nhtsa: "TransmissionStyle",
        kind: Kind::Text,
        ours: |d| d.transmission.clone(),
    },
    Field {
        name: "transmission_speeds",
        nhtsa: "TransmissionSpeeds",
        kind: Kind::Text,
        ours: |d| text(d.transmission_speeds),
    },
    Field {
        name: "engine_model",
        nhtsa: "EngineModel",
        kind: Kind::Text,
        ours: |d| d.engine.as_ref()?.model.clone(),
    },
    Field {
        name: "engine_configuration",
        nhtsa: "EngineConfiguration",
        kind: Kind::Text,
        ours: |d| d.engine.as_ref()?.configuration.clone(),
    },
    Field {
        name: "cylinders",
        nhtsa: "EngineCylinders",
        kind: Kind::Text,
        ours: |d| text(d.engine.as_ref()?.cylinders),
    },
    Field {
        name: "displacement_l",
        nhtsa: "DisplacementL",
        kind: Kind::Litres,
        ours: |d| text(d.engine.as_ref()?.displacement_l),
    },
    Field {
        name: "fuel",
        nhtsa: "FuelTypePrimary",
        kind: Kind::Text,
        ours: |d| d.engine.as_ref()?.fuel.clone(),
    },
    Field {
        name: "electrification",
        nhtsa: "ElectrificationLevel",
        kind: Kind::Text,
        ours: |d| d.engine.as_ref()?.electrification.clone(),
    },
    Field {
        name: "plant_city",
        nhtsa: "PlantCity",
        kind: Kind::Text,
        ours: |d| d.plant.city.clone(),
    },
    Field {
        name: "plant_state",
        nhtsa: "PlantState",
        kind: Kind::Text,
        ours: |d| d.plant.state.clone(),
    },
    Field {
        name: "plant_country",
        nhtsa: "PlantCountry",
        kind: Kind::Text,
        ours: |d| d.plant.country.clone(),
    },
    Field {
        name: "plant_company",
        nhtsa: "PlantCompanyName",
        kind: Kind::Text,
        ours: |d| d.plant.company.clone(),
    },
    Field {
        name: "vehicle_type",
        nhtsa: "VehicleType",
        kind: Kind::Text,
        ours: |d| d.manufacturer.vehicle_type.clone(),
    },
    Field {
        name: "manufacturer",
        nhtsa: "Manufacturer",
        kind: Kind::Text,
        ours: |d| Some(d.manufacturer.name.clone()),
    },
    Field {
        name: "abs",
        nhtsa: "ABS",
        kind: Kind::Text,
        ours: |d| d.safety.as_ref()?.abs.clone(),
    },
    Field {
        name: "esc",
        nhtsa: "ESC",
        kind: Kind::Text,
        ours: |d| d.safety.as_ref()?.esc.clone(),
    },
    Field {
        name: "tpms",
        nhtsa: "TPMS",
        kind: Kind::Text,
        ours: |d| d.safety.as_ref()?.tpms.clone(),
    },
    Field {
        name: "adaptive_cruise",
        nhtsa: "AdaptiveCruiseControl",
        kind: Kind::Text,
        ours: |d| d.safety.as_ref()?.adaptive_cruise.clone(),
    },
    Field {
        name: "lane_keep",
        nhtsa: "LaneKeepSystem",
        kind: Kind::Text,
        ours: |d| d.safety.as_ref()?.lane_keep.clone(),
    },
    Field {
        name: "lane_departure",
        nhtsa: "LaneDepartureWarning",
        kind: Kind::Text,
        ours: |d| d.safety.as_ref()?.lane_departure.clone(),
    },
    Field {
        name: "blind_spot",
        nhtsa: "BlindSpotMon",
        kind: Kind::Text,
        ours: |d| d.safety.as_ref()?.blind_spot.clone(),
    },
    Field {
        name: "forward_collision",
        nhtsa: "ForwardCollisionWarning",
        kind: Kind::Text,
        ours: |d| d.safety.as_ref()?.forward_collision.clone(),
    },
    Field {
        name: "auto_brake",
        nhtsa: "CIB",
        kind: Kind::Text,
        ours: |d| d.safety.as_ref()?.auto_brake.clone(),
    },
    Field {
        name: "backup_camera",
        nhtsa: "RearVisibilitySystem",
        kind: Kind::Text,
        ours: |d| d.safety.as_ref()?.backup_camera.clone(),
    },
    Field {
        name: "airbags_front",
        nhtsa: "AirBagLocFront",
        kind: Kind::Text,
        ours: |d| d.safety.as_ref()?.airbags_front.clone(),
    },
    Field {
        name: "airbags_side",
        nhtsa: "AirBagLocSide",
        kind: Kind::Text,
        ours: |d| d.safety.as_ref()?.airbags_side.clone(),
    },
    Field {
        name: "airbags_curtain",
        nhtsa: "AirBagLocCurtain",
        kind: Kind::Text,
        ours: |d| d.safety.as_ref()?.airbags_curtain.clone(),
    },
    Field {
        name: "airbags_knee",
        nhtsa: "AirBagLocKnee",
        kind: Kind::Text,
        ours: |d| d.safety.as_ref()?.airbags_knee.clone(),
    },
];

/// Lowercased and trimmed; empty for a missing value or NHTSA's placeholder.
fn normal(value: Option<&str>) -> String {
    let value = value.unwrap_or_default().trim().to_lowercase();
    if value == "not applicable" {
        String::new()
    } else {
        value
    }
}

/// The decoder's own shortening of drive types, applied to NHTSA's value.
fn short_drive(value: &str) -> &str {
    match value.split_once('/') {
        Some((short, rest)) if !short.trim().is_empty() && rest.contains("wheel drive") => {
            short.trim()
        }
        _ => value,
    }
}

fn same(kind: Kind, ours: &str, theirs: &str) -> bool {
    match kind {
        Kind::Text => ours == theirs,
        Kind::Drive => ours == short_drive(theirs),
        Kind::Litres => match (ours.parse::<f64>(), theirs.parse::<f64>()) {
            (Ok(ours), Ok(theirs)) => ((ours * 10.0).round() - (theirs * 10.0).round()).abs() < 0.5,
            _ => ours == theirs,
        },
    }
}

/// Decodes every VIN in the fixtures and tallies agreement field by field.
pub fn run<D: VinData>(data: D, fixtures: &Fixtures) -> Report {
    let decoder = Decoder::new(data);
    let options = DecodeOptions {
        model_year: None,
        current_year: Some(fixtures.recorded_year),
    };
    let mut report = Report {
        vins: 0,
        decode_errors: 0,
        fields: FIELDS
            .iter()
            .map(|field| (field.name.to_owned(), Tally::default()))
            .collect(),
    };
    for (vin, answers) in &fixtures.vins {
        report.vins += 1;
        let Ok(decoded) = decoder.decode(vin, options) else {
            report.decode_errors += 1;
            continue;
        };
        for field in FIELDS {
            let ours = normal((field.ours)(&decoded).as_deref());
            let theirs = normal(answers.get(field.nhtsa).map(String::as_str));
            let tally = report.fields.entry(field.name.to_owned()).or_default();
            match (ours.is_empty(), theirs.is_empty()) {
                (true, true) => tally.both_empty += 1,
                (true, false) => tally.missing += 1,
                (false, true) => tally.extra += 1,
                (false, false) if same(field.kind, &ours, &theirs) => tally.agree += 1,
                (false, false) => tally.differ += 1,
            }
        }
    }
    report
}

/// Ways `report` is worse than `baseline`. Empty when agreement held.
pub fn regressions(report: &Report, baseline: &Report) -> Vec<String> {
    let mut problems = Vec::new();
    if report.decode_errors > baseline.decode_errors {
        problems.push(format!(
            "decode errors rose from {} to {}",
            baseline.decode_errors, report.decode_errors
        ));
    }
    for (name, was) in &baseline.fields {
        let Some(now) = report.fields.get(name) else {
            problems.push(format!("{name}: in the baseline but no longer compared"));
            continue;
        };
        if now.agree < was.agree {
            problems.push(format!(
                "{name}: agree fell from {} to {}",
                was.agree, now.agree
            ));
        }
        if now.differ > was.differ {
            problems.push(format!(
                "{name}: differ rose from {} to {}",
                was.differ, now.differ
            ));
        }
        if now.extra > was.extra {
            problems.push(format!(
                "{name}: extra rose from {} to {}",
                was.extra, now.extra
            ));
        }
    }
    for name in report.fields.keys() {
        if !baseline.fields.contains_key(name) {
            problems.push(format!("{name}: compared but not in the baseline"));
        }
    }
    problems
}

/// The report as a text table, fields in comparison order.
pub fn table(report: &Report) -> String {
    let mut text = format!(
        "{} VINs, {} could not be decoded\n{:<22}{:>7}{:>8}{:>9}{:>7}{:>12}\n",
        report.vins,
        report.decode_errors,
        "field",
        "agree",
        "differ",
        "missing",
        "extra",
        "both empty"
    );
    for field in FIELDS {
        let tally = report.fields.get(field.name).copied().unwrap_or_default();
        let _ = writeln!(
            text,
            "{:<22}{:>7}{:>8}{:>9}{:>7}{:>12}",
            field.name, tally.agree, tally.differ, tally.missing, tally.extra, tally.both_empty
        );
    }
    text
}

#[cfg(test)]
mod tests {
    use super::*;
    use wenmar_vin::{Element, Manufacturer, MemoryData};

    const KONA: &str = "KM8K2CAB4PU001140";

    fn data() -> MemoryData {
        MemoryData::new()
            .with_manufacturer(Manufacturer {
                make: Some("Hyundai".to_owned()),
                vehicle_type: Some("Multipurpose Passenger Vehicle (MPV)".to_owned()),
                ..Manufacturer::new("KM8", "HYUNDAI MOTOR CO")
            })
            .with_schema("KM8", 1, 2022, None)
            .with_pattern(1, "K2***", Element::Model, "Kona")
            .with_pattern(1, "K2***", Element::Trim, "SE")
            .with_pattern(1, "K2***", Element::DriveType, "FWD/Front-Wheel Drive")
            .with_pattern(1, "K2***", Element::DisplacementCi, "122")
            .with_pattern(1, "K2***", Element::Doors, "5")
    }

    fn fixtures(answers: &[(&str, &[(&str, &str)])]) -> Fixtures {
        Fixtures {
            recorded_year: 2026,
            vins: answers
                .iter()
                .map(|(vin, fields)| {
                    let fields = fields
                        .iter()
                        .map(|(key, value)| ((*key).to_owned(), (*value).to_owned()))
                        .collect();
                    ((*vin).to_owned(), fields)
                })
                .collect(),
        }
    }

    fn tally(report: &Report, field: &str) -> Tally {
        report.fields[field]
    }

    #[test]
    fn sorts_each_field_into_one_outcome() {
        let report = run(
            data(),
            &fixtures(&[(
                KONA,
                &[
                    ("ModelYear", "2023"),
                    ("Make", "HYUNDAI"),
                    ("Model", " Kona "),
                    ("Trim", "Limited"),
                    ("Series", "Not Applicable"),
                    ("TransmissionStyle", "Automatic"),
                    ("DriveType", "FWD/Front-Wheel Drive"),
                    ("DisplacementL", "1.999221808"),
                    ("VehicleType", "MULTIPURPOSE PASSENGER VEHICLE (MPV)"),
                    ("Manufacturer", "HYUNDAI MOTOR CO"),
                ],
            )]),
        );
        assert_eq!((report.vins, report.decode_errors), (1, 0));
        let agree = Tally {
            agree: 1,
            ..Tally::default()
        };
        for field in [
            "year",
            "make",
            "model",
            "drivetrain",
            "displacement_l",
            "vehicle_type",
            "manufacturer",
        ] {
            assert_eq!(tally(&report, field), agree, "{field}");
        }
        assert_eq!(
            tally(&report, "trim"),
            Tally {
                differ: 1,
                ..Tally::default()
            }
        );
        assert_eq!(
            tally(&report, "transmission"),
            Tally {
                missing: 1,
                ..Tally::default()
            }
        );
        assert_eq!(
            tally(&report, "doors"),
            Tally {
                extra: 1,
                ..Tally::default()
            }
        );
        assert_eq!(
            tally(&report, "series"),
            Tally {
                both_empty: 1,
                ..Tally::default()
            }
        );
    }

    #[test]
    fn a_vin_we_cannot_decode_is_counted_not_fatal() {
        let report = run(
            data(),
            &fixtures(&[
                (
                    "1HGCM82633A004352",
                    &[("ErrorCode", "0"), ("Make", "HONDA")],
                ),
                ("NOT A VIN", &[("ErrorCode", "7")]),
                (KONA, &[("Make", "HYUNDAI")]),
            ]),
        );
        assert_eq!((report.vins, report.decode_errors), (3, 2));
        assert_eq!(tally(&report, "make").agree, 1);
    }

    #[test]
    fn the_recorded_year_bounds_model_years() {
        // In 2026 the Y code can only be 2000; a fixture recorded in 2030
        // would allow 2030. The corpus must decode the same in any year.
        let recorded_2030 = Fixtures {
            recorded_year: 2030,
            ..fixtures(&[("KM8K2CAB4YU001140", &[("ModelYear", "2030")])])
        };
        assert_eq!(tally(&run(data(), &recorded_2030), "year").agree, 1);
    }

    fn report(agree: u32, differ: u32, extra: u32) -> Report {
        let mut fields = BTreeMap::new();
        fields.insert(
            "make".to_owned(),
            Tally {
                agree,
                differ,
                extra,
                ..Tally::default()
            },
        );
        Report {
            vins: 10,
            decode_errors: 0,
            fields,
        }
    }

    #[test]
    fn equal_or_better_reports_are_not_regressions() {
        assert!(regressions(&report(8, 1, 0), &report(8, 1, 0)).is_empty());
        assert!(regressions(&report(9, 0, 0), &report(8, 1, 0)).is_empty());
    }

    #[test]
    fn worse_reports_name_what_got_worse() {
        let problems = regressions(&report(7, 2, 1), &report(8, 1, 0));
        assert_eq!(problems.len(), 3);
        assert!(
            problems[0].contains("make") && problems[0].contains("agree"),
            "{problems:?}"
        );
    }

    #[test]
    fn fields_on_one_side_only_are_reported() {
        let mut changed = report(8, 1, 0);
        changed
            .fields
            .insert("new_field".to_owned(), Tally::default());
        assert_eq!(regressions(&changed, &report(8, 1, 0)).len(), 1);
        assert_eq!(regressions(&report(8, 1, 0), &changed).len(), 1);
    }

    #[test]
    fn more_decode_errors_is_a_regression() {
        let mut worse = report(8, 1, 0);
        worse.decode_errors = 1;
        assert_eq!(regressions(&worse, &report(8, 1, 0)).len(), 1);
    }

    #[test]
    fn the_table_lists_every_field() {
        let text = table(&run(data(), &fixtures(&[(KONA, &[("Make", "HYUNDAI")])])));
        assert!(text.contains("make"), "{text}");
        assert!(text.contains("airbags_knee"), "{text}");
        assert!(text.lines().count() > 30, "{text}");
    }
}
