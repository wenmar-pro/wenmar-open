//! Compares this project's decodes with NHTSA's recorded answers.

use std::collections::{BTreeMap, BTreeSet};
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

/// How one field compared across the corpus. The VINs behind every
/// difference are kept, so a change that fixes one VIN and breaks another
/// cannot hide behind unchanged totals.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Outcome {
    /// VINs where both have the same value.
    pub agree: u32,
    /// VINs where both have a value and they are not the same.
    pub differ: Vec<String>,
    /// VINs where NHTSA has a value and we do not.
    pub missing: Vec<String>,
    /// VINs where we have a value and NHTSA does not.
    pub extra: Vec<String>,
    /// VINs where neither has a value.
    pub both_empty: u32,
}

/// The counts of an [`Outcome`].
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Tally {
    pub agree: u32,
    pub differ: u32,
    pub missing: u32,
    pub extra: u32,
    pub both_empty: u32,
}

fn count(vins: &[String]) -> u32 {
    u32::try_from(vins.len()).unwrap_or(u32::MAX)
}

impl Outcome {
    pub fn tally(&self) -> Tally {
        Tally {
            agree: self.agree,
            differ: count(&self.differ),
            missing: count(&self.missing),
            extra: count(&self.extra),
            both_empty: self.both_empty,
        }
    }
}

/// Agreement with NHTSA over a corpus.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Report {
    pub vins: u32,
    /// VINs this project could not decode at all.
    pub decode_errors: u32,
    pub fields: BTreeMap<String, Outcome>,
}

#[derive(Clone, Copy)]
enum Kind {
    /// Compared exactly, after trimming.
    Text,
    /// NHTSA's API upper-cases these; compared without regard to case.
    Upper,
    /// NHTSA spells drive types out; we keep the part before the slash.
    Drive,
    /// Compared after rounding both to one decimal place.
    Litres,
    /// Compared as whole numbers, allowing one either way for rounding.
    Cc,
    /// Compared as amounts of money, equal to the cent.
    Money,
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
        kind: Kind::Upper,
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
        name: "displacement_cc",
        nhtsa: "DisplacementCC",
        kind: Kind::Cc,
        ours: |d| text(d.engine.as_ref()?.displacement_cc),
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
        kind: Kind::Upper,
        ours: |d| d.manufacturer.vehicle_type.clone(),
    },
    Field {
        name: "manufacturer",
        nhtsa: "Manufacturer",
        kind: Kind::Upper,
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
    Field {
        name: "traction_control",
        nhtsa: "TractionControl",
        kind: Kind::Text,
        ours: |d| d.safety.as_ref()?.traction_control.clone(),
    },
    Field {
        name: "dynamic_brake_support",
        nhtsa: "DynamicBrakeSupport",
        kind: Kind::Text,
        ours: |d| d.safety.as_ref()?.dynamic_brake_support.clone(),
    },
    Field {
        name: "rear_cross_traffic",
        nhtsa: "RearCrossTrafficAlert",
        kind: Kind::Text,
        ours: |d| d.safety.as_ref()?.rear_cross_traffic.clone(),
    },
    Field {
        name: "park_assist",
        nhtsa: "ParkAssist",
        kind: Kind::Text,
        ours: |d| d.safety.as_ref()?.park_assist.clone(),
    },
    Field {
        name: "pedestrian_braking",
        nhtsa: "PedestrianAutomaticEmergencyBraking",
        kind: Kind::Text,
        ours: |d| d.safety.as_ref()?.pedestrian_braking.clone(),
    },
    Field {
        name: "lane_centering",
        nhtsa: "LaneCenteringAssistance",
        kind: Kind::Text,
        ours: |d| d.safety.as_ref()?.lane_centering.clone(),
    },
    Field {
        name: "wheel_size_front",
        nhtsa: "WheelSizeFront",
        kind: Kind::Text,
        ours: |d| text(d.wheel_size_front),
    },
    Field {
        name: "wheel_size_rear",
        nhtsa: "WheelSizeRear",
        kind: Kind::Text,
        ours: |d| text(d.wheel_size_rear),
    },
    Field {
        name: "seats",
        nhtsa: "Seats",
        kind: Kind::Text,
        ours: |d| text(d.seats),
    },
    Field {
        name: "seat_rows",
        nhtsa: "SeatRows",
        kind: Kind::Text,
        ours: |d| text(d.seat_rows),
    },
    Field {
        name: "gvwr",
        nhtsa: "GVWR",
        kind: Kind::Text,
        ours: |d| d.gvwr.clone(),
    },
    Field {
        name: "base_price_usd",
        nhtsa: "BasePrice",
        kind: Kind::Money,
        ours: |d| text(d.base_price_usd),
    },
];

/// Trimmed; empty for a missing value or NHTSA's placeholder.
fn normal(value: Option<&str>) -> &str {
    let value = value.unwrap_or_default().trim();
    if value.eq_ignore_ascii_case("not applicable") {
        ""
    } else {
        value
    }
}

/// The decoder's own shortening of drive types, applied to NHTSA's value.
fn short_drive(value: &str) -> &str {
    match value.split_once('/') {
        Some((short, rest)) if !short.trim().is_empty() && rest.contains("Wheel Drive") => {
            short.trim()
        }
        _ => value,
    }
}

fn numbers(ours: &str, theirs: &str) -> Option<(f64, f64)> {
    Some((ours.parse().ok()?, theirs.parse().ok()?))
}

fn same(kind: Kind, ours: &str, theirs: &str) -> bool {
    match kind {
        Kind::Text => ours == theirs,
        Kind::Upper => ours.to_lowercase() == theirs.to_lowercase(),
        Kind::Drive => ours == short_drive(theirs),
        Kind::Litres => numbers(ours, theirs).map_or(ours == theirs, |(ours, theirs)| {
            ((ours * 10.0).round() - (theirs * 10.0).round()).abs() < 0.5
        }),
        Kind::Cc => numbers(ours, theirs).map_or(ours == theirs, |(ours, theirs)| {
            (ours.round() - theirs.round()).abs() <= 1.0
        }),
        Kind::Money => numbers(ours, theirs).map_or(ours == theirs, |(ours, theirs)| {
            ((ours * 100.0).round() - (theirs * 100.0).round()).abs() < 0.5
        }),
    }
}

/// Decodes every VIN in the fixtures and records agreement field by field.
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
            .map(|field| (field.name.to_owned(), Outcome::default()))
            .collect(),
    };
    for (vin, answers) in &fixtures.vins {
        report.vins += 1;
        let Ok(decoded) = decoder.decode(vin, options) else {
            report.decode_errors += 1;
            continue;
        };
        for field in FIELDS {
            let ours = (field.ours)(&decoded);
            let ours = normal(ours.as_deref());
            let theirs = normal(answers.get(field.nhtsa).map(String::as_str));
            let outcome = report.fields.entry(field.name.to_owned()).or_default();
            match (ours.is_empty(), theirs.is_empty()) {
                (true, true) => outcome.both_empty += 1,
                (true, false) => outcome.missing.push(vin.clone()),
                (false, true) => outcome.extra.push(vin.clone()),
                (false, false) if same(field.kind, ours, theirs) => outcome.agree += 1,
                (false, false) => outcome.differ.push(vin.clone()),
            }
        }
    }
    report
}

/// VINs in `now` that were not in `was`.
fn newly(now: &[String], was: &[String]) -> Vec<String> {
    let was: BTreeSet<&String> = was.iter().collect();
    now.iter()
        .filter(|vin| !was.contains(vin))
        .cloned()
        .collect()
}

/// A few VINs for a message, with a count of the rest.
fn sample(vins: &[String]) -> String {
    const SHOWN: usize = 10;
    let mut text = vins
        .iter()
        .take(SHOWN)
        .cloned()
        .collect::<Vec<_>>()
        .join(", ");
    if vins.len() > SHOWN {
        let _ = write!(text, " and {} more", vins.len() - SHOWN);
    }
    text
}

/// Ways `report` is worse than `baseline`. Empty when agreement held.
///
/// The comparison is by VIN, not by total: a VIN that newly differs, is newly
/// missing, or newly has a value NHTSA lacks is a regression even if another
/// VIN improved.
pub fn regressions(report: &Report, baseline: &Report) -> Vec<String> {
    let mut problems = Vec::new();
    if report.vins != baseline.vins {
        problems.push(format!(
            "the corpus has {} VINs but the baseline was recorded with {}; update the baseline",
            report.vins, baseline.vins
        ));
    }
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
        for (what, now, was) in [
            ("newly differ", &now.differ, &was.differ),
            ("newly have a value NHTSA lacks", &now.extra, &was.extra),
            ("are newly missing", &now.missing, &was.missing),
        ] {
            let new = newly(now, was);
            if !new.is_empty() {
                problems.push(format!(
                    "{name}: {} VINs {what}: {}",
                    new.len(),
                    sample(&new)
                ));
            }
        }
    }
    for name in report.fields.keys() {
        if !baseline.fields.contains_key(name) {
            problems.push(format!("{name}: compared but not in the baseline"));
        }
    }
    problems
}

/// The report as a text table, fields in comparison order, followed by the
/// VINs behind each difference.
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
    let empty = Outcome::default();
    for field in FIELDS {
        let tally = report.fields.get(field.name).unwrap_or(&empty).tally();
        let _ = writeln!(
            text,
            "{:<22}{:>7}{:>8}{:>9}{:>7}{:>12}",
            field.name, tally.agree, tally.differ, tally.missing, tally.extra, tally.both_empty
        );
    }
    for field in FIELDS {
        let outcome = report.fields.get(field.name).unwrap_or(&empty);
        if !outcome.differ.is_empty() {
            let _ = writeln!(text, "{} differs: {}", field.name, sample(&outcome.differ));
        }
        if !outcome.extra.is_empty() {
            let _ = writeln!(text, "{} extra: {}", field.name, sample(&outcome.extra));
        }
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

    fn one(answers: &[(&str, &str)]) -> Report {
        run(data(), &fixtures(&[(KONA, answers)]))
    }

    fn outcome(report: &Report, field: &str) -> Tally {
        report.fields[field].tally()
    }

    const AGREE: Tally = Tally {
        agree: 1,
        differ: 0,
        missing: 0,
        extra: 0,
        both_empty: 0,
    };
    const DIFFER: Tally = Tally {
        agree: 0,
        differ: 1,
        missing: 0,
        extra: 0,
        both_empty: 0,
    };

    #[test]
    fn sorts_each_field_into_one_outcome() {
        let report = one(&[
            ("ModelYear", "2023"),
            ("Model", " Kona "),
            ("Trim", "Limited"),
            ("Series", "Not Applicable"),
            ("TransmissionStyle", "Automatic"),
            ("DriveType", "FWD/Front-Wheel Drive"),
            ("DisplacementL", "1.999221808"),
            ("DisplacementCC", "1999.221808"),
        ]);
        assert_eq!((report.vins, report.decode_errors), (1, 0));
        for field in [
            "year",
            "model",
            "drivetrain",
            "displacement_l",
            "displacement_cc",
        ] {
            assert_eq!(outcome(&report, field), AGREE, "{field}");
        }
        assert_eq!(outcome(&report, "trim"), DIFFER);
        assert_eq!(
            outcome(&report, "transmission"),
            Tally {
                missing: 1,
                ..Tally::default()
            }
        );
        assert_eq!(
            outcome(&report, "doors"),
            Tally {
                extra: 1,
                ..Tally::default()
            }
        );
        assert_eq!(
            outcome(&report, "series"),
            Tally {
                both_empty: 1,
                ..Tally::default()
            }
        );
        assert_eq!(report.fields["trim"].differ, vec![KONA]);
        assert_eq!(report.fields["transmission"].missing, vec![KONA]);
        assert_eq!(report.fields["doors"].extra, vec![KONA]);
    }

    #[test]
    fn only_the_fields_nhtsa_upper_cases_ignore_case() {
        let report = one(&[
            ("Make", "HYUNDAI"),
            ("VehicleType", "MULTIPURPOSE PASSENGER VEHICLE (MPV)"),
            ("Manufacturer", "hyundai motor co"),
            ("Model", "KONA"),
            ("Trim", "se"),
        ]);
        for field in ["make", "vehicle_type", "manufacturer"] {
            assert_eq!(outcome(&report, field), AGREE, "{field}");
        }
        for field in ["model", "trim"] {
            assert_eq!(outcome(&report, field), DIFFER, "{field}");
        }
    }

    #[test]
    fn engine_sizes_are_compared_with_a_small_tolerance() {
        // Ours is 2.0 L and 1999 cc.
        let litres = |theirs: &str| outcome(&one(&[("DisplacementL", theirs)]), "displacement_l");
        assert_eq!(litres("2"), AGREE);
        assert_eq!(litres("2.04"), AGREE);
        assert_eq!(litres("2.1"), DIFFER);
        assert_eq!(litres("1.9"), DIFFER);
        let cc = |theirs: &str| outcome(&one(&[("DisplacementCC", theirs)]), "displacement_cc");
        assert_eq!(cc("1999.2"), AGREE);
        assert_eq!(cc("2000"), AGREE);
        assert_eq!(cc("2002"), DIFFER);
    }

    #[test]
    fn a_base_price_is_compared_to_the_cent() {
        let priced = data().with_pattern(1, "K2***", Element::BasePrice, "34195");
        let price = |theirs: &str| {
            let report = run(
                priced.clone(),
                &fixtures(&[(KONA, &[("BasePrice", theirs)])]),
            );
            outcome(&report, "base_price_usd")
        };
        assert_eq!(price("34195.00"), AGREE);
        assert_eq!(price("34195"), AGREE);
        assert_eq!(price("34195.01"), DIFFER);
        assert_eq!(price("34196"), DIFFER);
    }

    #[test]
    fn the_specification_fields_are_compared() {
        let equipped = data()
            .with_pattern(1, "K2***", Element::WheelSizeFront, "17")
            .with_pattern(1, "K2***", Element::Seats, "5");
        let report = run(
            equipped,
            &fixtures(&[(
                KONA,
                &[
                    ("WheelSizeFront", "17"),
                    ("Seats", "7"),
                    ("TractionControl", "Standard"),
                ],
            )]),
        );
        assert_eq!(outcome(&report, "wheel_size_front"), AGREE);
        assert_eq!(outcome(&report, "seats"), DIFFER);
        assert_eq!(report.fields["traction_control"].missing, vec![KONA]);
    }

    #[test]
    fn a_litre_figure_we_leave_out_is_missing_not_agreed() {
        let scooter = MemoryData::new()
            .with_manufacturer(Manufacturer::new("KM8", "Maker"))
            .with_schema("KM8", 1, 2022, None)
            .with_pattern(1, "K2***", Element::DisplacementCc, "49");
        let report = run(scooter, &fixtures(&[(KONA, &[("DisplacementL", "0.049")])]));
        assert_eq!(
            outcome(&report, "displacement_l"),
            Tally {
                missing: 1,
                ..Tally::default()
            }
        );
    }

    #[test]
    fn a_vin_nhtsa_flagged_as_an_error_is_still_compared() {
        let report = one(&[
            ("ErrorCode", "5,14"),
            ("Model", "Kona"),
            ("Trim", "Limited"),
        ]);
        assert_eq!(outcome(&report, "model"), AGREE);
        assert_eq!(outcome(&report, "trim"), DIFFER);
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
        assert_eq!(outcome(&report, "make").agree, 1);
    }

    #[test]
    fn the_recorded_year_bounds_model_years() {
        // In 2026 the Y code can only be 2000; a fixture recorded in 2030
        // allows 2030. The corpus must decode the same in any year.
        let recorded_2030 = Fixtures {
            recorded_year: 2030,
            ..fixtures(&[("KM8K2CAB4YU001140", &[("ModelYear", "2030")])])
        };
        assert_eq!(outcome(&run(data(), &recorded_2030), "year").agree, 1);
    }

    fn report(agree: u32, differ: &[&str], extra: &[&str], missing: &[&str]) -> Report {
        let owned = |vins: &[&str]| vins.iter().map(|vin| (*vin).to_owned()).collect();
        let mut fields = BTreeMap::new();
        fields.insert(
            "make".to_owned(),
            Outcome {
                agree,
                differ: owned(differ),
                extra: owned(extra),
                missing: owned(missing),
                both_empty: 0,
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
        let baseline = report(8, &["A"], &[], &["B"]);
        assert!(regressions(&baseline, &baseline).is_empty());
        assert!(regressions(&report(10, &[], &[], &[]), &baseline).is_empty());
    }

    #[test]
    fn a_vin_that_newly_differs_is_named() {
        let problems = regressions(
            &report(8, &["A", "C"], &[], &["B"]),
            &report(8, &["A"], &[], &["B"]),
        );
        assert_eq!(problems.len(), 1, "{problems:?}");
        assert!(
            problems[0].contains("make") && problems[0].contains('C'),
            "{problems:?}"
        );
    }

    #[test]
    fn a_swap_that_leaves_the_counts_unchanged_is_still_caught() {
        // One VIN fixed, another broken: same totals, different VINs.
        let problems = regressions(&report(8, &["C"], &[], &[]), &report(8, &["A"], &[], &[]));
        assert_eq!(problems.len(), 1, "{problems:?}");
    }

    #[test]
    fn newly_extra_and_newly_missing_vins_are_regressions() {
        let baseline = report(8, &[], &[], &[]);
        assert_eq!(
            regressions(&report(8, &[], &["E"], &[]), &baseline).len(),
            1
        );
        assert_eq!(
            regressions(&report(8, &[], &[], &["M"]), &baseline).len(),
            1
        );
    }

    #[test]
    fn a_different_corpus_size_cannot_be_compared() {
        let mut other = report(8, &[], &[], &[]);
        other.vins = 11;
        let problems = regressions(&other, &report(8, &[], &[], &[]));
        assert!(
            problems
                .iter()
                .any(|problem| problem.contains("11") && problem.contains("10")),
            "{problems:?}"
        );
    }

    #[test]
    fn fields_on_one_side_only_are_reported() {
        let mut changed = report(8, &[], &[], &[]);
        changed
            .fields
            .insert("new_field".to_owned(), Outcome::default());
        assert_eq!(regressions(&changed, &report(8, &[], &[], &[])).len(), 1);
        assert_eq!(regressions(&report(8, &[], &[], &[]), &changed).len(), 1);
    }

    #[test]
    fn more_decode_errors_is_a_regression() {
        let mut worse = report(8, &[], &[], &[]);
        worse.decode_errors = 1;
        assert_eq!(regressions(&worse, &report(8, &[], &[], &[])).len(), 1);
    }

    #[test]
    fn the_table_lists_every_field_and_names_the_differences() {
        let text = table(&one(&[("Make", "HYUNDAI"), ("Trim", "Limited")]));
        assert!(text.contains("make"), "{text}");
        assert!(text.contains("airbags_knee"), "{text}");
        assert!(text.contains("wheel_size_front"), "{text}");
        assert!(text.contains(&format!("trim differs: {KONA}")), "{text}");
    }
}
