use std::cmp::Reverse;
use std::collections::HashMap;
use std::time::{SystemTime, UNIX_EPOCH};

use crate::check_digit;
use crate::data::{DataError, Element, Manufacturer, Pattern, VinData};
use crate::engine::{self, Engine};
use crate::error::VinError;
use crate::model_year;
use crate::pattern;
use crate::result::{Decoded, ManufacturerInfo, Plant, Safety, Warning, WarningCode};
use crate::suggest;
use crate::vin::Vin;

/// Why a VIN could not be decoded at all.
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum DecodeError {
    /// The input is not a well-formed VIN.
    #[error("{error}")]
    InvalidVin {
        error: VinError,
        /// Well-formed VINs the caller may have meant.
        suggestions: Vec<String>,
    },
    /// No manufacturer is registered for the first three characters.
    #[error("no manufacturer is registered for {wmi}")]
    UnknownManufacturer { wmi: String },
    /// The data store failed.
    #[error("vehicle data could not be read")]
    Data(#[source] DataError),
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct DecodeOptions {
    /// Use this model year instead of working it out from the VIN.
    pub model_year: Option<u16>,
    /// The current calendar year. `None` reads the system clock.
    pub current_year: Option<u16>,
}

/// Decodes VINs against a data source.
#[derive(Debug, Clone)]
pub struct Decoder<D> {
    data: D,
}

type Values<'a> = HashMap<Element, &'a str>;

impl<D: VinData> Decoder<D> {
    pub fn new(data: D) -> Self {
        Self { data }
    }

    pub fn decode(&self, input: &str, options: DecodeOptions) -> Result<Decoded, DecodeError> {
        let vin = Vin::parse(input).map_err(|error| DecodeError::InvalidVin {
            error,
            suggestions: suggest::for_malformed(input),
        })?;

        let check_digit = check_digit::check(&vin);
        let mut warnings = Vec::new();
        if !check_digit.valid {
            warnings.push(Warning {
                code: WarningCode::InvalidCheckDigit,
                message: format!(
                    "Position 9 should be {} for this VIN, but it is {}.",
                    check_digit.expected, check_digit.actual
                ),
                suggestions: suggest::for_check_digit(&vin),
            });
        }

        let manufacturer = self.find_manufacturer(&vin)?;

        let years = match options.model_year {
            Some(year) => vec![year],
            None => {
                let current = options.current_year.unwrap_or_else(current_year);
                model_year::candidates(&vin, current)
            }
        };
        // Position 7 does not settle the cycle for every vehicle type, so each
        // candidate year is decoded and the best is kept: one that resolves a
        // model, else one that resolves anything, else the likeliest year.
        let key = vin.match_key();
        let mut best: Option<(u8, u16, Vec<Pattern>)> = None;
        for candidate in &years {
            let schema_ids = self
                .data
                .schemas(&manufacturer.wmi, *candidate)
                .map_err(DecodeError::Data)?;
            if schema_ids.is_empty() {
                continue;
            }
            let matched: Vec<Pattern> = self
                .data
                .patterns(&schema_ids, &key)
                .map_err(DecodeError::Data)?
                .into_iter()
                .filter(|row| usable(&row.value) && pattern::matches(&row.keys, &key))
                .collect();
            let score = if matched.iter().any(|row| row.element == Element::Model) {
                2
            } else {
                u8::from(!matched.is_empty())
            };
            if best.as_ref().is_none_or(|(current, _, _)| score > *current) {
                best = Some((score, *candidate, matched));
            }
            if score == 2 {
                break;
            }
        }
        let (year, matched) = match best {
            Some((_, year, matched)) => (Some(year), matched),
            None => (years.first().copied(), Vec::new()),
        };
        if year.is_none() {
            warnings.push(warning(
                WarningCode::ModelYearUnknown,
                "Position 10 of this VIN does not encode a model year.",
            ));
        }

        let values = select(&matched);

        if matched.is_empty() {
            warnings.push(warning(
                WarningCode::NoPatterns,
                "The manufacturer is known, but no vehicle details are on file for this VIN.",
            ));
        } else if !values.contains_key(&Element::Model) {
            warnings.push(warning(
                WarningCode::ModelUnresolved,
                "Some details were found, but the model could not be determined.",
            ));
        }

        let text = |element: Element| text(&values, element);
        let count = |element: Element| count(&values, element);

        Ok(Decoded {
            vin: vin.as_str().to_owned(),
            valid: check_digit.valid,
            check_digit,
            year,
            make: text(Element::Make).or_else(|| clean(manufacturer.make.clone())),
            model: text(Element::Model),
            series: text(Element::Series),
            trim: text(Element::Trim),
            body: text(Element::BodyClass),
            doors: count(Element::Doors),
            drivetrain: text(Element::DriveType).map(|value| short_form(&value)),
            transmission: text(Element::TransmissionStyle),
            transmission_speeds: count(Element::TransmissionSpeeds),
            engine: build_engine(&values),
            safety: build_safety(&values),
            manufacturer: ManufacturerInfo {
                wmi: manufacturer.wmi,
                name: manufacturer.name.trim().to_owned(),
                country: clean(manufacturer.country),
                vehicle_type: clean(manufacturer.vehicle_type),
            },
            plant: Plant {
                code: vin.plant_char(),
                city: text(Element::PlantCity),
                state: text(Element::PlantState),
                country: text(Element::PlantCountry),
                company: text(Element::PlantCompany),
            },
            warnings,
        })
    }

    /// Low-volume manufacturers share a three-character code and are told
    /// apart by a six-character one, so that is tried first.
    fn find_manufacturer(&self, vin: &Vin) -> Result<Manufacturer, DecodeError> {
        if let Some(extended) = vin.extended_wmi()
            && let Some(found) = self
                .data
                .manufacturer(&extended)
                .map_err(DecodeError::Data)?
        {
            return Ok(found);
        }
        self.data
            .manufacturer(vin.wmi())
            .map_err(DecodeError::Data)?
            .ok_or_else(|| DecodeError::UnknownManufacturer {
                wmi: vin.wmi().to_owned(),
            })
    }
}

fn warning(code: WarningCode, message: &str) -> Warning {
    Warning {
        code,
        message: message.to_owned(),
        suggestions: Vec::new(),
    }
}

/// vPIC marks attributes that do not apply with a placeholder value.
fn usable(value: &str) -> bool {
    let value = value.trim();
    !value.is_empty() && !value.eq_ignore_ascii_case("not applicable")
}

/// Trims a value from the data and drops it if it is a placeholder.
fn clean(value: Option<String>) -> Option<String> {
    value
        .map(|value| value.trim().to_owned())
        .filter(|value| usable(value))
}

/// The schema this VIN most likely belongs to.
fn primary_schema(matched: &[Pattern]) -> Option<i64> {
    let matches_in = |schema_id: i64| {
        matched
            .iter()
            .filter(|candidate| candidate.schema_id == schema_id)
            .count()
    };
    let best_model = matched
        .iter()
        .filter(|candidate| candidate.element == Element::Model)
        .max_by_key(|candidate| {
            (
                pattern::specificity(&candidate.keys),
                matches_in(candidate.schema_id),
                Reverse(candidate.schema_id),
            )
        });
    match best_model {
        Some(model) => Some(model.schema_id),
        None => matched
            .iter()
            .map(|candidate| candidate.schema_id)
            .max_by_key(|schema_id| (matches_in(*schema_id), Reverse(*schema_id))),
    }
}

/// One value per element: primary schema first, then the most specific
/// pattern, then the lowest pattern id.
fn select(matched: &[Pattern]) -> Values<'_> {
    let primary = primary_schema(matched);
    let rank = |candidate: &Pattern| {
        (
            Some(candidate.schema_id) == primary,
            pattern::specificity(&candidate.keys),
            Reverse(candidate.id),
        )
    };
    let mut best: HashMap<Element, &Pattern> = HashMap::new();
    for candidate in matched {
        let better = best
            .get(&candidate.element)
            .is_none_or(|current| rank(candidate) > rank(current));
        if better {
            best.insert(candidate.element, candidate);
        }
    }
    // vPIC stores the make against the model, so the make must come from the
    // pattern that supplied the model, not be chosen on its own.
    if let Some(model) = best.get(&Element::Model).copied()
        && let Some(make) = matched.iter().find(|row| {
            row.element == Element::Make
                && row.schema_id == model.schema_id
                && row.keys == model.keys
        })
    {
        best.insert(Element::Make, make);
    }
    best.into_iter()
        .map(|(element, chosen)| (element, chosen.value.as_str()))
        .collect()
}

fn text(values: &Values<'_>, element: Element) -> Option<String> {
    values.get(&element).map(|value| value.trim().to_owned())
}

fn count(values: &Values<'_>, element: Element) -> Option<u8> {
    values
        .get(&element)
        .and_then(|value| value.trim().parse().ok())
}

/// vPIC writes drive types as `FWD/Front-Wheel Drive`. Shops use the part
/// before the slash. Values where the slash separates alternatives, such as
/// `2WD/4WD`, are kept whole.
fn short_form(value: &str) -> String {
    match value.split_once('/') {
        Some((short, rest)) if !short.trim().is_empty() && rest.contains("Wheel Drive") => {
            short.trim().to_owned()
        }
        _ => value.to_owned(),
    }
}

fn build_engine(values: &Values<'_>) -> Option<Engine> {
    let displacement_l = values
        .get(&Element::DisplacementL)
        .and_then(|value| value.trim().parse::<f64>().ok())
        .filter(|litres| litres.is_finite() && *litres > 0.0);
    let mut engine = Engine {
        label: None,
        model: text(values, Element::EngineModel),
        displacement_l,
        cylinders: count(values, Element::EngineCylinders),
        configuration: text(values, Element::EngineConfiguration),
        fuel: text(values, Element::FuelTypePrimary),
        turbo: values
            .get(&Element::Turbo)
            .map(|value| value.trim().eq_ignore_ascii_case("yes")),
        electrification: text(values, Element::ElectrificationLevel),
    };
    if engine == Engine::default() {
        return None;
    }
    engine.label = engine::label(&engine);
    Some(engine)
}

fn build_safety(values: &Values<'_>) -> Option<Safety> {
    let safety = Safety {
        abs: text(values, Element::Abs),
        esc: text(values, Element::Esc),
        tpms: text(values, Element::TpmsType),
        adaptive_cruise: text(values, Element::AdaptiveCruise),
        lane_keep: text(values, Element::LaneKeep),
        lane_departure: text(values, Element::LaneDeparture),
        blind_spot: text(values, Element::BlindSpot),
        forward_collision: text(values, Element::ForwardCollision),
        auto_brake: text(values, Element::AutoBrake),
        backup_camera: text(values, Element::BackupCamera),
        airbags_front: text(values, Element::AirbagsFront),
        airbags_side: text(values, Element::AirbagsSide),
        airbags_curtain: text(values, Element::AirbagsCurtain),
        airbags_knee: text(values, Element::AirbagsKnee),
    };
    (safety != Safety::default()).then_some(safety)
}

/// The calendar year from the system clock. Only used as an upper bound on
/// model years, so being off by a day at New Year does not matter.
fn current_year() -> u16 {
    const SECONDS_PER_YEAR: u64 = 31_556_952;
    let seconds = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |elapsed| elapsed.as_secs());
    u16::try_from(1970 + seconds / SECONDS_PER_YEAR).unwrap_or(u16::MAX)
}
