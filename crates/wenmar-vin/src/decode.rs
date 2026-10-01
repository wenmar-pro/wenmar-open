use std::cmp::Reverse;
use std::collections::{HashMap, HashSet};
use std::time::{SystemTime, UNIX_EPOCH};

use crate::check_digit;
use crate::data::{DataError, Element, Manufacturer, Pattern, SpecRow, VinData};
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

/// The value chosen for an element, and where it ranks.
#[derive(Debug, Clone)]
struct Item {
    attribute: String,
    value: String,
    priority: i32,
    changed_on: String,
}

type Values = HashMap<Element, Item>;

/// The result of decoding against one candidate model year.
struct Attempt {
    year: u16,
    /// The patterns that matched the VIN.
    matched: Vec<Pattern>,
    /// One value per element, from every source: the patterns, then the
    /// engine model, then the specification sheets.
    values: Values,
}

impl Attempt {
    /// What NHTSA compares when the cycle is not settled: how much weight the
    /// resolved elements carry, whatever they came from, then how many
    /// elements the patterns resolved, then the later year.
    fn standing(&self) -> (u32, usize, u16) {
        let resolved: HashSet<Element> = self.values.keys().copied().collect();
        let from_patterns: HashSet<Element> = self.matched.iter().map(|row| row.element).collect();
        (element_weight(&resolved), from_patterns.len(), self.year)
    }
}

/// Rank of a value that came from an engine model: below any schema year.
const ENGINE_MODEL_PRIORITY: i32 = 50;
/// Rank of a value that came from a specification sheet: below everything else.
const SPECIFICATION_PRIORITY: i32 = -100;
/// vPIC's weight for displacement, which NHTSA converts between units
/// before scoring, so it counts once.
const DISPLACEMENT_WEIGHT: u32 = 98;

fn element_weight(elements: &HashSet<Element>) -> u32 {
    let has_displacement = [
        Element::DisplacementL,
        Element::DisplacementCc,
        Element::DisplacementCi,
    ]
    .iter()
    .any(|element| elements.contains(element));
    let weight: u32 = elements.iter().map(|element| element.weight()).sum();
    weight
        + if has_displacement {
            DISPLACEMENT_WEIGHT
        } else {
            0
        }
}

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
                model_year::candidates(&vin, current, manufacturer.light_vehicle)
            }
        };
        // For cars, MPVs and light trucks position 7 settles the cycle, so the
        // first candidate year that has any schema is final, as in NHTSA's
        // decoder. A new model missing from the data must not turn into a
        // 30-year-old one. For other vehicles the cycle is not settled, so
        // every candidate is decoded and the one that explains more is kept.
        let conclusive = manufacturer.light_vehicle;
        let key = vin.match_key();
        let mut best: Option<Attempt> = None;
        for candidate in &years {
            let schemas = self
                .data
                .schemas(&manufacturer.wmi, *candidate)
                .map_err(DecodeError::Data)?;
            if schemas.is_empty() {
                continue;
            }
            let mut year_from: HashMap<i64, u16> = HashMap::new();
            for schema in &schemas {
                let latest = year_from.entry(schema.id).or_insert(schema.year_from);
                *latest = (*latest).max(schema.year_from);
            }
            let schema_ids: Vec<i64> = year_from.keys().copied().collect();
            let matched: Vec<Pattern> = self
                .data
                .patterns(&schema_ids, &key)
                .map_err(DecodeError::Data)?
                .into_iter()
                .filter(|row| usable(&row.value) && pattern::matches(&row.keys, &key))
                .collect();
            let mut values = select(&matched, &year_from);
            add_engine_model(&self.data, &mut values).map_err(DecodeError::Data)?;
            add_specifications(&self.data, &manufacturer.wmi, *candidate, &mut values)
                .map_err(DecodeError::Data)?;
            let attempt = Attempt {
                year: *candidate,
                matched,
                values,
            };
            if conclusive {
                best = Some(attempt);
                break;
            }
            if best
                .as_ref()
                .is_none_or(|current| attempt.standing() > current.standing())
            {
                best = Some(attempt);
            }
        }
        let (year, matched, values) = match best {
            Some(attempt) => (Some(attempt.year), attempt.matched, attempt.values),
            None => (years.first().copied(), Vec::new(), Values::new()),
        };
        if year.is_none() {
            warnings.push(warning(
                WarningCode::ModelYearUnknown,
                "Position 10 of this VIN does not encode a model year.",
            ));
        }

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

/// Adds what the engine model implies, for elements the VIN left empty.
fn add_engine_model<D: VinData>(data: &D, values: &mut Values) -> Result<(), DataError> {
    let Some(name) = values
        .get(&Element::EngineModel)
        .map(|item| item.value.clone())
    else {
        return Ok(());
    };
    let mut rows = data.engine_model(&name)?;
    // Latest change first, then lowest id, so the first row seen per element wins.
    rows.sort_by(|a, b| b.changed_on.cmp(&a.changed_on).then(a.id.cmp(&b.id)));
    for row in rows {
        if usable(&row.value) && !values.contains_key(&row.element) {
            values.insert(
                row.element,
                Item {
                    attribute: row.attribute,
                    value: row.value,
                    priority: ENGINE_MODEL_PRIORITY,
                    changed_on: row.changed_on,
                },
            );
        }
    }
    Ok(())
}

/// Adds values from the specification sheets whose key rows all match what
/// has been decoded, for elements still empty.
fn add_specifications<D: VinData>(
    data: &D,
    wmi: &str,
    year: u16,
    values: &mut Values,
) -> Result<(), DataError> {
    let Some(model) = values
        .get(&Element::Model)
        .map(|item| item.attribute.clone())
    else {
        return Ok(());
    };
    let rows = data.specs(wmi, &model, year)?;

    // A sheet applies only if it has keys and every key matches.
    let mut keys: HashMap<i64, bool> = HashMap::new();
    for row in rows.iter().filter(|row| row.is_key) {
        let matches = values.get(&row.element).is_some_and(|item| {
            item.attribute
                .trim()
                .eq_ignore_ascii_case(row.attribute.trim())
        });
        let all = keys.entry(row.spec_pattern_id).or_insert(true);
        *all = *all && matches;
    }

    let mut additions: Vec<&SpecRow> = rows
        .iter()
        .filter(|row| !row.is_key && keys.get(&row.spec_pattern_id) == Some(&true))
        .filter(|row| usable(&row.value) && !values.contains_key(&row.element))
        .collect();
    // Latest change first, then lowest id, so the first row seen per element wins.
    additions.sort_by(|a, b| b.changed_on.cmp(&a.changed_on).then(a.id.cmp(&b.id)));
    for row in additions {
        values.entry(row.element).or_insert_with(|| Item {
            attribute: row.attribute.clone(),
            value: row.value.clone(),
            priority: SPECIFICATION_PRIORITY,
            changed_on: row.changed_on.clone(),
        });
    }
    Ok(())
}

/// Trims a value from the data and drops it if it is a placeholder.
fn clean(value: Option<String>) -> Option<String> {
    value
        .map(|value| value.trim().to_owned())
        .filter(|value| usable(value))
}

/// What NHTSA's decoder sorts on when several patterns give a value for the
/// same element. The greatest rank wins.
fn rank<'p>(
    row: &'p Pattern,
    year_from: &HashMap<i64, u16>,
) -> (u16, &'p str, Reverse<usize>, Reverse<String>, Reverse<i64>) {
    (
        year_from.get(&row.schema_id).copied().unwrap_or(0),
        row.changed_on.as_str(),
        Reverse(
            row.keys
                .chars()
                .filter(|character| *character != '*')
                .count(),
        ),
        Reverse(row.keys.replace(['[', ']'], "")),
        Reverse(row.id),
    )
}

/// One value per element, chosen in NHTSA's order: latest schema, latest
/// change, fewest fixed characters, keys in text order, lowest id.
fn select(matched: &[Pattern], year_from: &HashMap<i64, u16>) -> Values {
    let mut best: HashMap<Element, &Pattern> = HashMap::new();
    for candidate in matched {
        let better = best
            .get(&candidate.element)
            .is_none_or(|current| rank(candidate, year_from) > rank(current, year_from));
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
        .map(|(element, row)| {
            let priority = i32::from(year_from.get(&row.schema_id).copied().unwrap_or(0));
            let item = Item {
                attribute: row.attribute.clone(),
                value: row.value.clone(),
                priority,
                changed_on: row.changed_on.clone(),
            };
            (element, item)
        })
        .collect()
}

fn text(values: &Values, element: Element) -> Option<String> {
    values
        .get(&element)
        .map(|item| item.value.trim().to_owned())
}

fn count(values: &Values, element: Element) -> Option<u8> {
    values
        .get(&element)
        .and_then(|item| item.value.trim().parse().ok())
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

fn build_engine(values: &Values) -> Option<Engine> {
    let number = |element: Element| {
        values
            .get(&element)
            .and_then(|item| item.value.trim().parse::<f64>().ok())
            .filter(|amount| amount.is_finite() && *amount > 0.0)
    };
    // NHTSA converts between units from whichever pattern ranks first: the
    // later schema, then the more recent change.
    let recency = |element: Element| {
        values
            .get(&element)
            .map(|item| (item.priority, item.changed_on.as_str()))
    };
    const CC_PER_CUBIC_INCH: f64 = 16.387_064;
    // No road vehicle in vPIC comes close; anything larger is bad data.
    const LARGEST_LITRES: f64 = 100.0;
    let stated_cc = number(Element::DisplacementCc);
    let stated_litres = number(Element::DisplacementL);
    let from_cubic_inches = number(Element::DisplacementCi).map(|ci| ci * CC_PER_CUBIC_INCH);

    // With equal recency NHTSA converts cubic inches to cc before litres.
    let litres_first = recency(Element::DisplacementL) > recency(Element::DisplacementCi);
    let cubic_centimetres = stated_cc
        .or(match (from_cubic_inches, stated_litres) {
            (Some(_), Some(litres)) if litres_first => Some(litres * 1000.0),
            (Some(cc), _) => Some(cc),
            (None, litres) => litres.map(|litres| litres * 1000.0),
        })
        .filter(|cc| *cc <= LARGEST_LITRES * 1000.0);

    // With equal recency NHTSA converts cc to litres before cubic inches.
    let cubic_inches_first = recency(Element::DisplacementCi) > recency(Element::DisplacementCc);
    let litres = stated_litres
        .or(match (stated_cc, from_cubic_inches) {
            (Some(_), Some(cc)) if cubic_inches_first => Some(cc / 1000.0),
            (Some(cc), _) => Some(cc / 1000.0),
            (None, cc) => cc.map(|cc| cc / 1000.0),
        })
        .filter(|litres| *litres <= LARGEST_LITRES);
    // An engine under a twentieth of a litre would round to 0.0; it is
    // described by its cubic centimetres alone.
    let displacement_l = litres
        .map(|litres| (litres * 10.0).round() / 10.0)
        .filter(|litres| *litres >= 0.1);
    let displacement_cc = cubic_centimetres
        .map(f64::round)
        .filter(|cc| *cc >= 1.0)
        .map(|cc| cc as u32);
    let mut engine = Engine {
        label: None,
        model: text(values, Element::EngineModel),
        displacement_l,
        displacement_cc,
        cylinders: count(values, Element::EngineCylinders),
        configuration: text(values, Element::EngineConfiguration),
        fuel: text(values, Element::FuelTypePrimary),
        turbo: values
            .get(&Element::Turbo)
            .map(|item| item.value.trim().eq_ignore_ascii_case("yes")),
        electrification: text(values, Element::ElectrificationLevel),
    };
    if engine == Engine::default() {
        return None;
    }
    engine.label = engine::label(&engine);
    Some(engine)
}

fn build_safety(values: &Values) -> Option<Safety> {
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
