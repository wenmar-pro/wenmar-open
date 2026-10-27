use std::cmp::Reverse;
use std::collections::{BTreeMap, HashMap, HashSet};
use std::time::{SystemTime, UNIX_EPOCH};

use crate::check_digit;
use crate::data::{DataError, Element, Manufacturer, Pattern, SpecRow, VinData};
use crate::engine::{self, Engine};
use crate::error::VinError;
use crate::generation;
use crate::model_year;
use crate::pattern;
use crate::result::{
    Attribute, AttributeSource, BodyStyle, Decoded, Drive, Electric, ManufacturerInfo, Plant,
    Safety, Warning, WarningCode,
};
use crate::suggest;
use crate::vin::Vin;
use crate::vpic;

/// vPIC's ids of the elements this crate shows but does not interpret. They
/// stay [`Element::Other`], so the decoder weighs them as before.
mod id {
    pub const BATTERY_INFO: i64 = 1;
    pub const BATTERY_TYPE: i64 = 2;
    pub const BED_TYPE: i64 = 3;
    pub const CAB_TYPE: i64 = 4;
    pub const DESTINATION_MARKET: i64 = 10;
    pub const ENGINE_CYCLES: i64 = 17;
    pub const ENGINE_KW: i64 = 21;
    pub const STEERING: i64 = 36;
    pub const AXLES: i64 = 41;
    pub const BRAKE_SYSTEM: i64 = 42;
    pub const BED_LENGTH: i64 = 49;
    pub const BRAKE_DESCRIPTION: i64 = 52;
    pub const CURB_WEIGHT: i64 = 54;
    pub const AIRBAGS_SEAT_CUSHION: i64 = 56;
    pub const BATTERY_VOLTS: i64 = 58;
    pub const BATTERY_KWH: i64 = 59;
    pub const VALVE_TRAIN: i64 = 62;
    pub const FUEL_SECONDARY: i64 = 66;
    pub const FUEL_INJECTION: i64 = 67;
    pub const ENGINE_HP: i64 = 71;
    pub const EV_DRIVE_UNIT: i64 = 72;
    pub const PRETENSIONER: i64 = 78;
    pub const SEAT_BELTS: i64 = 79;
    pub const ADAPTIVE_HEADLIGHTS: i64 = 82;
    pub const TRIM2: i64 = 109;
    pub const SERIES2: i64 = 110;
    pub const WHEELBASE: i64 = 111;
    pub const WHEELBASE_TO: i64 = 112;
    pub const WHEELS: i64 = 115;
    pub const COOLING: i64 = 122;
    pub const ENGINE_HP_TO: i64 = 125;
    pub const CHARGER_LEVEL: i64 = 127;
    pub const CHARGER_KW: i64 = 128;
    pub const BATTERY_VOLTS_TO: i64 = 133;
    pub const BATTERY_KWH_TO: i64 = 134;
    pub const TOP_SPEED: i64 = 139;
    pub const AXLE_CONFIGURATION: i64 = 145;
    pub const ENGINE_MANUFACTURER: i64 = 146;
    pub const TRACK_WIDTH: i64 = 159;
    pub const AUTO_REVERSE: i64 = 172;
    pub const PEDESTRIAN_ALERT: i64 = 173;
    pub const CRASH_NOTIFICATION: i64 = 174;
    pub const EVENT_DATA_RECORDER: i64 = 175;
    pub const KEYLESS_IGNITION: i64 = 176;
    pub const DAYTIME_RUNNING_LIGHTS: i64 = 177;
    pub const HEADLAMP_SOURCE: i64 = 178;
    pub const AUTOMATIC_HIGH_BEAMS: i64 = 179;
    pub const ADAPTIVE_DRIVING_BEAM: i64 = 180;
    pub const GVWR_TO: i64 = 190;
}

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

/// Where the value chosen for an element came from.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Source {
    /// A pattern that matched the VIN.
    Pattern,
    /// The engine model a pattern named.
    EngineModel,
    /// A specification sheet.
    Specification,
}

/// The value chosen for an element, and where it ranks.
#[derive(Debug, Clone)]
struct Item {
    source: Source,
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
    /// How many rows from the patterns and the engine model are notes, of
    /// which NHTSA keeps every one instead of choosing a value.
    notes: usize,
    /// The engine model's note rows.
    engine_notes: Vec<(Element, String)>,
    /// Where the year is among the candidates, first being 0.
    index: usize,
}

impl Attempt {
    /// What NHTSA compares when the cycle is not settled: how much weight the
    /// resolved elements carry, whatever they came from, then how many items
    /// the patterns and the engine model gave, then the later year. An item
    /// is a resolved element or a note. The make is not one: NHTSA takes it
    /// from the model. Nor is anything from a specification sheet.
    fn standing(&self) -> (u32, usize, u16) {
        let resolved: HashSet<Element> = self.values.keys().copied().collect();
        let elements = self
            .values
            .iter()
            .filter(|(element, item)| {
                **element != Element::Make
                    && !is_note(**element)
                    && item.source != Source::Specification
            })
            .count();
        (element_weight(&resolved), elements + self.notes, self.year)
    }
}

/// Rank of a value that came from an engine model: below any schema year.
const ENGINE_MODEL_PRIORITY: i32 = 50;
/// Rank of a value that came from a specification sheet: below everything else.
const SPECIFICATION_PRIORITY: i32 = -100;
/// vPIC's weight for displacement, which NHTSA converts between units
/// before scoring, so it counts once.
const DISPLACEMENT_WEIGHT: u32 = 98;

/// vPIC's note elements: note, other restraint system, engine, bus,
/// motorcycle and trailer information, active safety system note, NCSA note.
const NOTE_ELEMENT_IDS: [i64; 8] = [114, 121, 129, 150, 154, 155, 169, 186];

/// Whether NHTSA keeps every row of this element, not only the best one.
fn is_note(element: Element) -> bool {
    matches!(element, Element::Other(id) if NOTE_ELEMENT_IDS.contains(&id))
}

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
        let mut with_schemas = 0usize;
        for (index, candidate) in years.iter().enumerate() {
            let schemas = self
                .data
                .schemas(&manufacturer.wmi, *candidate)
                .map_err(DecodeError::Data)?;
            if schemas.is_empty() {
                continue;
            }
            with_schemas += 1;
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
            let engine_notes =
                add_engine_model(&self.data, &mut values).map_err(DecodeError::Data)?;
            let notes =
                matched.iter().filter(|row| is_note(row.element)).count() + engine_notes.len();
            add_specifications(&self.data, &manufacturer.wmi, *candidate, &mut values)
                .map_err(DecodeError::Data)?;
            let attempt = Attempt {
                year: *candidate,
                matched,
                values,
                notes,
                engine_notes,
                index,
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
        let (year, matched, values, engine_notes, index) = match best {
            Some(attempt) => (
                Some(attempt.year),
                attempt.matched,
                attempt.values,
                attempt.engine_notes,
                attempt.index,
            ),
            None => (
                years.first().copied(),
                Vec::new(),
                Values::new(),
                Vec::new(),
                0,
            ),
        };
        // Position 7 settles the cycle of a light vehicle, unless the year it
        // points to had no data. For another vehicle the year is settled
        // only when one candidate alone had data.
        let year_conclusive = year.is_some()
            && with_schemas > 0
            && (options.model_year.is_some()
                || if conclusive {
                    index == 0
                } else {
                    with_schemas == 1
                });
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
        let other = |id: i64| text(Element::Other(id));
        let number = |id: i64| number(&values, Element::Other(id));
        let notes = collect_notes(&matched, &engine_notes, &values);
        let make = text(Element::Make).or_else(|| clean(manufacturer.make.clone()));
        let model = text(Element::Model);
        let body = text(Element::BodyClass);
        let drivetrain = text(Element::DriveType);
        let generation = match (&make, &model, year) {
            (Some(make), Some(model), Some(year)) => generation::find(make, model, year),
            _ => None,
        };

        Ok(Decoded {
            vin: vin.as_str().to_owned(),
            valid: check_digit.valid,
            check_digit,
            year,
            make,
            model,
            series: text(Element::Series),
            trim: text(Element::Trim),
            body_style: body.as_deref().and_then(BodyStyle::from_vpic),
            body,
            doors: count(Element::Doors),
            drive: drivetrain.as_deref().and_then(Drive::from_vpic),
            drivetrain: drivetrain.map(|value| short_form(&value)),
            transmission: text(Element::TransmissionStyle),
            transmission_speeds: count(Element::TransmissionSpeeds),
            engine: build_engine(&values),
            safety: build_safety(&values),
            wheel_size_front: count(Element::WheelSizeFront),
            wheel_size_rear: count(Element::WheelSizeRear),
            seats: count(Element::Seats),
            seat_rows: count(Element::SeatRows),
            gvwr: text(Element::Gvwr),
            base_price_usd: text(Element::BasePrice).and_then(|value| price(&value)),
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
            year_conclusive,
            generation,
            series2: other(id::SERIES2),
            trim2: other(id::TRIM2),
            cab_type: other(id::CAB_TYPE),
            bed_type: other(id::BED_TYPE),
            bed_length_in: number(id::BED_LENGTH),
            curb_weight_lb: number(id::CURB_WEIGHT),
            gvwr_to: other(id::GVWR_TO),
            wheelbase_in: number(id::WHEELBASE),
            wheelbase_to_in: number(id::WHEELBASE_TO),
            track_width_in: number(id::TRACK_WIDTH),
            wheels: count(Element::Other(id::WHEELS)),
            axles: count(Element::Other(id::AXLES)),
            axle_configuration: other(id::AXLE_CONFIGURATION),
            steering: other(id::STEERING),
            brake_system: other(id::BRAKE_SYSTEM),
            brake_system_description: other(id::BRAKE_DESCRIPTION),
            destination_market: other(id::DESTINATION_MARKET),
            top_speed_mph: number(id::TOP_SPEED),
            electric: build_electric(&values),
            attributes: attributes(&values, &notes),
            notes: notes.into_iter().map(|(_, note)| note).collect(),
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
/// Returns its note rows.
fn add_engine_model<D: VinData>(
    data: &D,
    values: &mut Values,
) -> Result<Vec<(Element, String)>, DataError> {
    let Some(name) = values
        .get(&Element::EngineModel)
        .map(|item| item.value.clone())
    else {
        return Ok(Vec::new());
    };
    let mut rows = data.engine_model(&name)?;
    let notes = rows
        .iter()
        .filter(|row| is_note(row.element) && usable(&row.value))
        .map(|row| (row.element, row.value.trim().to_owned()))
        .collect();
    // Latest change first, then lowest id, so the first row seen per element wins.
    rows.sort_by(|a, b| b.changed_on.cmp(&a.changed_on).then(a.id.cmp(&b.id)));
    for row in rows {
        if usable(&row.value) && !values.contains_key(&row.element) {
            values.insert(
                row.element,
                Item {
                    source: Source::EngineModel,
                    attribute: row.attribute,
                    value: row.value,
                    priority: ENGINE_MODEL_PRIORITY,
                    changed_on: row.changed_on,
                },
            );
        }
    }
    Ok(notes)
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
            source: Source::Specification,
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
                source: Source::Pattern,
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

/// A finite amount above zero.
fn number(values: &Values, element: Element) -> Option<f64> {
    values
        .get(&element)
        .and_then(|item| item.value.trim().parse::<f64>().ok())
        .filter(|amount| amount.is_finite() && *amount > 0.0)
}

/// Every note, each once, by element and then as the rows come: those of
/// the matched patterns in id order, the engine model's, then one from a
/// specification sheet.
fn collect_notes(
    matched: &[Pattern],
    engine_notes: &[(Element, String)],
    values: &Values,
) -> Vec<(Element, String)> {
    let mut rows: Vec<&Pattern> = matched
        .iter()
        .filter(|row| is_note(row.element) && usable(&row.value))
        .collect();
    rows.sort_by_key(|row| (row.element.vpic_id(), row.id));
    let mut found: Vec<(Element, String)> = rows
        .into_iter()
        .map(|row| (row.element, row.value.trim().to_owned()))
        .collect();
    found.extend(engine_notes.iter().cloned());
    found.extend(
        values
            .iter()
            .filter(|(element, item)| is_note(**element) && item.source == Source::Specification)
            .map(|(element, item)| (*element, item.value.trim().to_owned())),
    );
    found.sort_by_key(|(element, _)| element.vpic_id());
    let mut seen: HashSet<String> = HashSet::new();
    found.retain(|(_, note)| seen.insert(note.clone()));
    found
}

/// Every resolved element by vPIC's code. A note element carries all its
/// notes.
fn attributes(values: &Values, notes: &[(Element, String)]) -> BTreeMap<String, Attribute> {
    values
        .iter()
        .map(|(element, item)| {
            let id = element.vpic_id();
            let info = vpic::element(id);
            let code = info.map_or_else(|| format!("Element{id}"), |info| info.code.to_owned());
            let name = info.map_or_else(|| code.clone(), |info| info.name.to_owned());
            let value = if is_note(*element) {
                let all: Vec<&str> = notes
                    .iter()
                    .filter(|(noted, _)| noted == element)
                    .map(|(_, note)| note.as_str())
                    .collect();
                if all.is_empty() {
                    item.value.trim().to_owned()
                } else {
                    all.join("; ")
                }
            } else {
                item.value.trim().to_owned()
            };
            let source = match item.source {
                Source::Pattern => AttributeSource::VinPattern,
                Source::EngineModel => AttributeSource::EngineModel,
                Source::Specification => AttributeSource::SpecificationSheet,
            };
            (
                code,
                Attribute {
                    name,
                    value,
                    source,
                },
            )
        })
        .collect()
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
        hp: number(Element::Other(id::ENGINE_HP)),
        hp_to: number(Element::Other(id::ENGINE_HP_TO)),
        kw: number(Element::Other(id::ENGINE_KW)),
        manufacturer: text(values, Element::Other(id::ENGINE_MANUFACTURER)),
        valve_train: text(values, Element::Other(id::VALVE_TRAIN)),
        fuel_injection: text(values, Element::Other(id::FUEL_INJECTION)),
        cooling: text(values, Element::Other(id::COOLING)),
        fuel_secondary: text(values, Element::Other(id::FUEL_SECONDARY)),
        stroke_cycles: count(values, Element::Other(id::ENGINE_CYCLES)),
    };
    if engine == Engine::default() {
        return None;
    }
    engine.label = engine::label(&engine);
    Some(engine)
}

/// A base price in dollars. Anything that is not a finite amount above zero
/// and at most 100,000,000 is not a price.
fn price(value: &str) -> Option<f64> {
    let amount: f64 = value.trim().parse().ok()?;
    (amount.is_finite() && amount > 0.0 && amount <= 100_000_000.0).then_some(amount)
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
        traction_control: text(values, Element::TractionControl),
        dynamic_brake_support: text(values, Element::DynamicBrakeSupport),
        rear_cross_traffic: text(values, Element::RearCrossTraffic),
        park_assist: text(values, Element::ParkAssist),
        pedestrian_braking: text(values, Element::PedestrianBraking),
        lane_centering: text(values, Element::LaneCentering),
        keyless_ignition: text(values, Element::Other(id::KEYLESS_IGNITION)),
        headlamp_light_source: text(values, Element::Other(id::HEADLAMP_SOURCE)),
        adaptive_driving_beam: text(values, Element::Other(id::ADAPTIVE_DRIVING_BEAM)),
        adaptive_headlights: text(values, Element::Other(id::ADAPTIVE_HEADLIGHTS)),
        automatic_high_beams: text(values, Element::Other(id::AUTOMATIC_HIGH_BEAMS)),
        daytime_running_lights: text(values, Element::Other(id::DAYTIME_RUNNING_LIGHTS)),
        auto_reverse_windows: text(values, Element::Other(id::AUTO_REVERSE)),
        crash_notification: text(values, Element::Other(id::CRASH_NOTIFICATION)),
        event_data_recorder: text(values, Element::Other(id::EVENT_DATA_RECORDER)),
        pedestrian_alert_sound: text(values, Element::Other(id::PEDESTRIAN_ALERT)),
        seat_belts: text(values, Element::Other(id::SEAT_BELTS)),
        pretensioner: text(values, Element::Other(id::PRETENSIONER)),
        airbags_seat_cushion: text(values, Element::Other(id::AIRBAGS_SEAT_CUSHION)),
    };
    (safety != Safety::default()).then_some(safety)
}

fn build_electric(values: &Values) -> Option<Electric> {
    let text = |id: i64| text(values, Element::Other(id));
    let number = |id: i64| number(values, Element::Other(id));
    let electric = Electric {
        battery_type: text(id::BATTERY_TYPE),
        battery_kwh: number(id::BATTERY_KWH),
        battery_kwh_to: number(id::BATTERY_KWH_TO),
        battery_volts: number(id::BATTERY_VOLTS),
        battery_volts_to: number(id::BATTERY_VOLTS_TO),
        battery_info: text(id::BATTERY_INFO),
        drive_unit: text(id::EV_DRIVE_UNIT),
        charger_level: text(id::CHARGER_LEVEL),
        charger_power_kw: number(id::CHARGER_KW),
    };
    (electric != Electric::default()).then_some(electric)
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
