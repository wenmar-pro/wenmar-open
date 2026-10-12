//! Readers for fuel ratings, specifications, the certified engine oil
//! grade and emission standards: the tables of `schema::FUEL`.
//!
//! Every list is for one model year of the catalog. A row says how far it
//! was matched in `confidence`: `model` (the model year only), `engine`
//! (one catalog engine) or `submodel` (an engine and a trim). A caller
//! must not show a row as more exact than that.

use std::fmt;

use serde::Serialize;
use wenmar_vehicles::{Source, Value};

use crate::{Dataset, ShopData, ShopError};

const TABLE: &str = "fuel_rating";

/// Where a rating comes from.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum FuelSource {
    /// EPA, the United States.
    Epa,
    /// Natural Resources Canada.
    Nrcan,
}

impl FuelSource {
    pub const ALL: [FuelSource; 2] = [FuelSource::Epa, FuelSource::Nrcan];

    /// `fuel_rating.source`.
    pub fn id(self) -> &'static str {
        match self {
            FuelSource::Epa => "epa",
            FuelSource::Nrcan => "nrcan",
        }
    }

    pub fn country(self) -> &'static str {
        match self {
            FuelSource::Epa => "us",
            FuelSource::Nrcan => "ca",
        }
    }

    pub fn dataset(self) -> Dataset {
        match self {
            FuelSource::Epa => Dataset::FuelUs,
            FuelSource::Nrcan => Dataset::FuelCa,
        }
    }

    pub fn parse(id: &str) -> Option<FuelSource> {
        FuelSource::ALL.into_iter().find(|source| source.id() == id)
    }

    pub fn from_country(country: &str) -> Option<FuelSource> {
        FuelSource::ALL
            .into_iter()
            .find(|source| source.country() == country)
    }
}

impl fmt::Display for FuelSource {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.id())
    }
}

/// City, highway and combined consumption in L/100 km.
#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
pub struct Consumption {
    pub city: Option<f64>,
    pub highway: Option<f64>,
    pub combined: Option<f64>,
}

/// City, highway and combined economy in miles per US gallon.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct Mpg {
    pub city: Option<u16>,
    pub highway: Option<u16>,
    pub combined: Option<u16>,
}

/// One rating of a model year.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct FuelRating {
    /// `epa` or `nrcan`.
    pub source: String,
    /// `us` or `ca`.
    pub country: String,
    /// EPA's vehicle id. None for an NRCan rating.
    pub source_id: Option<u32>,
    /// How the model was found: `exact`, `base` or `curated`.
    #[serde(rename = "match")]
    pub matched: String,
    /// How far the row was matched: `model`, `engine` or `submodel`.
    pub confidence: String,
    /// `catalog_engine.id`, when one engine was settled.
    pub engine_id: Option<i64>,
    /// `catalog_submodel.id`, when the name spelled one.
    pub submodel_id: Option<i64>,
    /// The source's own name for the vehicle: `F150 Pickup 2WD`.
    pub model_text: String,
    pub drive_text: Option<String>,
    pub body_text: Option<String>,
    pub note_text: Option<String>,
    pub displacement_l: Option<f64>,
    pub cylinders: Option<u8>,
    pub transmission: Option<String>,
    pub drive: Option<String>,
    /// The fuel the figures are for, which is also the recommended grade:
    /// `Regular Gasoline`, `Premium Gasoline`, `Diesel`, `Electricity`.
    pub fuel: String,
    pub fuel2: Option<String>,
    /// None when the fuel is not sold by the litre.
    pub l_100km: Option<Consumption>,
    pub mpg: Option<Mpg>,
    pub kwh_100km: Option<f64>,
    pub range_km: Option<u32>,
    pub co2_g_km: Option<u32>,
}

/// The specifications that come with one EPA rating.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct VehicleSpec {
    pub epa_id: u32,
    pub confidence: String,
    pub engine_id: Option<i64>,
    pub submodel_id: Option<i64>,
    pub model_text: String,
    pub transmission: Option<String>,
    pub drive: Option<String>,
    pub fuel: String,
    pub size_class: String,
    pub turbo: bool,
    pub supercharger: bool,
    pub start_stop: Option<bool>,
    pub electrification: Option<String>,
    pub ev_motor: Option<String>,
    pub passenger_ft3: Option<u16>,
    pub luggage_ft3: Option<u16>,
    pub gears: Option<u8>,
    pub transmission_type: Option<String>,
    pub fuel_usage: Option<String>,
    pub cylinder_deactivation: Option<bool>,
    pub cylinder_deactivation_name: Option<String>,
    pub variable_valve_timing: Option<bool>,
    pub variable_valve_lift: Option<bool>,
    pub intake_valves: Option<u8>,
    pub exhaust_valves: Option<u8>,
    pub battery_type: Option<String>,
    pub battery_voltage: Option<u16>,
    pub motor_kw: Option<String>,
}

/// The engine oil viscosity a maker certified an engine with, as EPA's
/// Fuel Economy Guide records it.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct OilGrade {
    pub confidence: String,
    pub engine_id: Option<i64>,
    /// The engine as the source describes it, for a row with no engine id.
    pub displacement_l: Option<f64>,
    pub cylinders: Option<u8>,
    /// Standard grades, such as `5W-30`. More than one when the maker
    /// listed more than one.
    pub grades: Vec<String>,
    /// The maker's own spellings the grades were read from.
    pub texts: Vec<String>,
}

/// One emission certification of an EPA rating.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct EmissionStandard {
    pub epa_id: u32,
    pub confidence: String,
    pub engine_id: Option<i64>,
    /// EPA's sales area code, a number. Its meaning is not given a name.
    pub sales_area: u8,
    pub standard: String,
    pub standard_text: Option<String>,
    pub smog_score: Option<u8>,
    pub engine_family: String,
}

/// `?1` make id, `?2` model id, `?3` year, `?4` and `?5` the sources.
pub const FUEL_FOR_VEHICLE: &str = "
SELECT source, source_id, \"match\", confidence, engine_id, submodel_id, model_text,
       drive_text, body_text, note_text, displacement_l, cylinders, transmission, drive,
       fuel, fuel2, city_l_100km, highway_l_100km, combined_l_100km, city_mpg, highway_mpg,
       combined_mpg, kwh_100km, range_km, co2_g_km
FROM fuel_rating
WHERE make_id = ?1 AND model_id = ?2 AND year = ?3 AND source IN (?4, ?5)
ORDER BY source, displacement_l, cylinders, model_text, transmission, fuel, id";

/// `?1` make id, `?2` model id, `?3` year.
pub const SPECS_FOR_VEHICLE: &str = "
SELECT s.epa_id, f.confidence, f.engine_id, f.submodel_id, f.model_text, f.transmission,
       f.drive, f.fuel, s.size_class, s.turbo, s.supercharger, s.start_stop,
       s.electrification, s.ev_motor,
       COALESCE(s.passenger_ft3_4door, s.passenger_ft3_2door, s.passenger_ft3_hatch),
       COALESCE(s.luggage_ft3_4door, s.luggage_ft3_2door, s.luggage_ft3_hatch),
       s.gears, s.transmission_type, s.fuel_usage, s.cylinder_deactivation,
       s.cylinder_deactivation_name, s.variable_valve_timing, s.variable_valve_lift,
       s.intake_valves, s.exhaust_valves, s.battery_type, s.battery_voltage, s.motor_kw
FROM fuel_rating f
JOIN vehicle_spec s ON s.epa_id = f.source_id
WHERE f.make_id = ?1 AND f.model_id = ?2 AND f.year = ?3 AND f.source = 'epa'
ORDER BY f.displacement_l, f.cylinders, f.model_text, f.transmission, f.id";

/// `?1` make id, `?2` model id, `?3` year. One row per rating and oil
/// text; the reader groups them by engine.
pub const OIL_FOR_VEHICLE: &str = "
SELECT f.engine_id, f.displacement_l, f.cylinders, o.grades, o.grade_text
FROM fuel_rating f
JOIN oil_grade o ON o.epa_id = f.source_id
WHERE f.make_id = ?1 AND f.model_id = ?2 AND f.year = ?3 AND f.source = 'epa'
ORDER BY f.engine_id IS NULL, f.engine_id, f.displacement_l, f.cylinders, o.grades, o.grade_text";

/// `?1` make id, `?2` model id, `?3` year.
pub const EMISSIONS_FOR_VEHICLE: &str = "
SELECT e.epa_id, f.confidence, f.engine_id, e.sales_area, e.standard, e.standard_text,
       e.smog_score, e.engine_family
FROM fuel_rating f
JOIN emission_standard e ON e.epa_id = f.source_id
WHERE f.make_id = ?1 AND f.model_id = ?2 AND f.year = ?3 AND f.source = 'epa'
ORDER BY e.epa_id, e.sales_area, e.standard, e.engine_family";

fn text(row: &[Value], column: usize) -> Result<String, ShopError> {
    row.get(column)
        .and_then(Value::text)
        .map(str::to_owned)
        .ok_or(ShopError::Shape(TABLE))
}

fn optional_text(row: &[Value], column: usize) -> Option<String> {
    row.get(column).and_then(Value::text).map(str::to_owned)
}

fn integer(row: &[Value], column: usize) -> Option<i64> {
    row.get(column).and_then(Value::integer)
}

fn small<T: TryFrom<i64>>(row: &[Value], column: usize) -> Option<T> {
    integer(row, column).and_then(|number| T::try_from(number).ok())
}

fn flag(row: &[Value], column: usize) -> Option<bool> {
    integer(row, column).map(|number| number != 0)
}

/// A REAL column. SQLite hands back an integer when one was stored.
fn real(row: &[Value], column: usize) -> Option<f64> {
    match row.get(column) {
        Some(Value::Real(number)) => Some(*number),
        Some(Value::Integer(number)) => Some(*number as f64),
        _ => None,
    }
}

fn model_year(make_id: i64, model_id: i64, year: u16) -> [Value; 3] {
    [make_id.into(), model_id.into(), i64::from(year).into()]
}

impl<S: Source> ShopData<S> {
    /// The fuel sources a reader may read: the one asked for, or every
    /// one the file holds. Fails with the dataset that is missing when
    /// the one asked for is not held, or when the file holds neither.
    pub fn require_fuel(&self, source: Option<FuelSource>) -> Result<Vec<FuelSource>, ShopError> {
        if let Some(source) = source {
            self.require(source.dataset())?;
            return Ok(vec![source]);
        }
        let held: Vec<FuelSource> = FuelSource::ALL
            .into_iter()
            .filter(|source| self.require(source.dataset()).is_ok())
            .collect();
        if held.is_empty() {
            return Err(ShopError::DatasetUnavailable(Dataset::FuelUs));
        }
        Ok(held)
    }

    /// The day a source's ratings were current. `None` when the file
    /// does not hold that source.
    pub fn fuel_as_of(&self, source: FuelSource) -> Option<&str> {
        self.datasets()
            .get(source.dataset())
            .map(|entry| entry.as_of.as_str())
    }

    /// Every rating matched to one model year: EPA's first, then NRCan's,
    /// each by engine size.
    pub fn fuel_for_vehicle(
        &self,
        make_id: i64,
        model_id: i64,
        year: u16,
        source: Option<FuelSource>,
    ) -> Result<Vec<FuelRating>, ShopError> {
        let sources = self.require_fuel(source)?;
        let (first, last) = (sources[0], sources[sources.len() - 1]);
        let [make, model, year] = model_year(make_id, model_id, year);
        let rows = self.query(
            FUEL_FOR_VEHICLE,
            &[make, model, year, first.id().into(), last.id().into()],
        )?;
        rows.iter()
            .map(|row| {
                let source = FuelSource::parse(&text(row, 0)?).ok_or(ShopError::Shape(TABLE))?;
                let consumption = Consumption {
                    city: real(row, 16),
                    highway: real(row, 17),
                    combined: real(row, 18),
                };
                let mpg = Mpg {
                    city: small(row, 19),
                    highway: small(row, 20),
                    combined: small(row, 21),
                };
                Ok(FuelRating {
                    source: source.id().to_owned(),
                    country: source.country().to_owned(),
                    source_id: small(row, 1),
                    matched: text(row, 2)?,
                    confidence: text(row, 3)?,
                    engine_id: integer(row, 4),
                    submodel_id: integer(row, 5),
                    model_text: text(row, 6)?,
                    drive_text: optional_text(row, 7),
                    body_text: optional_text(row, 8),
                    note_text: optional_text(row, 9),
                    displacement_l: real(row, 10),
                    cylinders: small(row, 11),
                    transmission: optional_text(row, 12),
                    drive: optional_text(row, 13),
                    fuel: text(row, 14)?,
                    fuel2: optional_text(row, 15),
                    l_100km: consumption.combined.is_some().then_some(consumption),
                    mpg: mpg.combined.is_some().then_some(mpg),
                    kwh_100km: real(row, 22),
                    range_km: small(row, 23),
                    co2_g_km: small(row, 24),
                })
            })
            .collect()
    }

    /// The specifications of every EPA rating matched to one model year.
    pub fn specs_for_vehicle(
        &self,
        make_id: i64,
        model_id: i64,
        year: u16,
    ) -> Result<Vec<VehicleSpec>, ShopError> {
        self.require(Dataset::Specs)?;
        let rows = self.query(SPECS_FOR_VEHICLE, &model_year(make_id, model_id, year))?;
        rows.iter()
            .map(|row| {
                Ok(VehicleSpec {
                    epa_id: small(row, 0).ok_or(ShopError::Shape("vehicle_spec"))?,
                    confidence: text(row, 1)?,
                    engine_id: integer(row, 2),
                    submodel_id: integer(row, 3),
                    model_text: text(row, 4)?,
                    transmission: optional_text(row, 5),
                    drive: optional_text(row, 6),
                    fuel: text(row, 7)?,
                    size_class: text(row, 8)?,
                    turbo: flag(row, 9).unwrap_or(false),
                    supercharger: flag(row, 10).unwrap_or(false),
                    start_stop: flag(row, 11),
                    electrification: optional_text(row, 12),
                    ev_motor: optional_text(row, 13),
                    passenger_ft3: small(row, 14),
                    luggage_ft3: small(row, 15),
                    gears: small(row, 16),
                    transmission_type: optional_text(row, 17),
                    fuel_usage: optional_text(row, 18),
                    cylinder_deactivation: flag(row, 19),
                    cylinder_deactivation_name: optional_text(row, 20),
                    variable_valve_timing: flag(row, 21),
                    variable_valve_lift: flag(row, 22),
                    intake_valves: small(row, 23),
                    exhaust_valves: small(row, 24),
                    battery_type: optional_text(row, 25),
                    battery_voltage: small(row, 26),
                    motor_kw: optional_text(row, 27),
                })
            })
            .collect()
    }

    /// The certified oil grades of one model year: one entry per catalog
    /// engine that has any, then one per engine size among the ratings
    /// whose engine was not settled. An entry lists every grade and every
    /// spelling its ratings carry, each once.
    pub fn oil_for_vehicle(
        &self,
        make_id: i64,
        model_id: i64,
        year: u16,
    ) -> Result<Vec<OilGrade>, ShopError> {
        self.require(Dataset::OilGrade)?;
        let rows = self.query(OIL_FOR_VEHICLE, &model_year(make_id, model_id, year))?;
        let mut found: Vec<OilGrade> = Vec::new();
        for row in &rows {
            let engine_id = integer(row, 0);
            let (litres, cylinders) = (real(row, 1), small::<u8>(row, 2));
            let grades = text(row, 3).map_err(|_| ShopError::Shape("oil_grade"))?;
            let spelling = text(row, 4).map_err(|_| ShopError::Shape("oil_grade"))?;
            let same = |entry: &OilGrade| match engine_id {
                Some(_) => entry.engine_id == engine_id,
                None => {
                    entry.engine_id.is_none()
                        && entry.displacement_l == litres
                        && entry.cylinders == cylinders
                }
            };
            let position = match found.iter().position(same) {
                Some(position) => position,
                None => {
                    found.push(OilGrade {
                        confidence: if engine_id.is_some() {
                            "engine"
                        } else {
                            "model"
                        }
                        .to_owned(),
                        engine_id,
                        displacement_l: litres,
                        cylinders,
                        grades: Vec::new(),
                        texts: Vec::new(),
                    });
                    found.len() - 1
                }
            };
            let entry = &mut found[position];
            for grade in grades.split(", ").filter(|grade| !grade.is_empty()) {
                if !entry.grades.iter().any(|known| known == grade) {
                    entry.grades.push(grade.to_owned());
                }
            }
            if !entry.texts.contains(&spelling) {
                entry.texts.push(spelling);
            }
        }
        Ok(found)
    }

    /// The emission certifications of every EPA rating matched to one
    /// model year. They are part of the `specs` dataset; the list is
    /// empty when the build had no emissions file.
    pub fn emissions_for_vehicle(
        &self,
        make_id: i64,
        model_id: i64,
        year: u16,
    ) -> Result<Vec<EmissionStandard>, ShopError> {
        self.require(Dataset::Specs)?;
        let rows = self.query(EMISSIONS_FOR_VEHICLE, &model_year(make_id, model_id, year))?;
        rows.iter()
            .map(|row| {
                let shape = ShopError::Shape("emission_standard");
                Ok(EmissionStandard {
                    epa_id: small(row, 0).ok_or(shape)?,
                    confidence: text(row, 1)?,
                    engine_id: integer(row, 2),
                    sales_area: small(row, 3).ok_or(ShopError::Shape("emission_standard"))?,
                    standard: text(row, 4)?,
                    standard_text: optional_text(row, 5),
                    smog_score: small(row, 6),
                    engine_family: text(row, 7)?,
                })
            })
            .collect()
    }
}
