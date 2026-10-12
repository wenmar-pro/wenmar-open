//! Readers for the facts collected by hand: fluids (`schema::FLUIDS`) and
//! tire fitments (`schema::TIRES`).
//!
//! Every row was read from the maker's own document by a person, and
//! carries the document, the day it was read and who compared it. A model
//! year with no row has nothing on record: a reader gives an empty list,
//! and never a figure from another model year or another engine.

use serde::Serialize;
use wenmar_vehicles::{Source, Value};

use crate::{Dataset, ShopData, ShopError};

/// The kinds of fluid a record may be about, in the order they are listed.
pub const FLUID_KINDS: [&str; 5] = [
    "engine_oil",
    "transmission",
    "coolant",
    "refrigerant",
    "brake",
];

/// One fluid of one model year.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Fluid {
    /// One of [`FLUID_KINDS`].
    pub kind: String,
    /// `catalog_engine.id`. None when the record is for every engine.
    pub engine_id: Option<i64>,
    /// Which transmission or system the record is for, when it says.
    pub applies_to: Option<String>,
    /// Engine oil viscosity, such as `5W-30`.
    pub grade: Option<String>,
    /// The maker's specification, or the type of fluid.
    pub specification: Option<String>,
    pub capacity_l: Option<f64>,
    /// US liquid quarts.
    pub capacity_qt: Option<f64>,
    /// The unit the source printed: `l` or `qt`.
    pub capacity_unit: Option<String>,
    pub with_filter: Option<bool>,
    pub charge_g: Option<i64>,
    pub charge_oz: Option<f64>,
    /// `g`, `kg`, `oz` or `lb`.
    pub charge_unit: Option<String>,
    /// `us` or `ca`. None when the source is for both.
    pub market: Option<String>,
    pub source: String,
    pub checked: String,
    pub reviewer: String,
}

/// One original-equipment tire fitment of one model year.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct TireFitment {
    pub size: String,
    /// The rear size, only when it differs from the front.
    pub size_rear: Option<String>,
    /// The trims the source ties the size to. Empty when it names none.
    pub trims: Vec<String>,
    pub pressure_front_kpa: Option<i64>,
    pub pressure_rear_kpa: Option<i64>,
    pub pressure_front_psi: Option<i64>,
    pub pressure_rear_psi: Option<i64>,
    /// The unit the source printed: `kpa`, `bar` or `psi`.
    pub pressure_unit: Option<String>,
    pub lug_torque_nm: Option<i64>,
    pub lug_torque_lbft: Option<i64>,
    /// `nm` or `lbft`.
    pub lug_torque_unit: Option<String>,
    pub bolt_pattern: Option<String>,
    pub market: Option<String>,
    pub source: String,
    pub checked: String,
    pub reviewer: String,
}

/// `?1` make id, `?2` model id, `?3` year. In the order of
/// [`FLUID_KINDS`], then by engine.
pub const FLUIDS_FOR_VEHICLE: &str = "
SELECT kind, engine_id, applies_to, grade, specification, capacity_l, capacity_qt,
       capacity_unit, with_filter, charge_g, charge_oz, charge_unit, market, source,
       checked, reviewer
FROM fluid
WHERE make_id = ?1 AND model_id = ?2 AND year = ?3
ORDER BY CASE kind WHEN 'engine_oil' THEN 0 WHEN 'transmission' THEN 1 WHEN 'coolant' THEN 2
                   WHEN 'refrigerant' THEN 3 ELSE 4 END,
         engine_id IS NULL, engine_id, applies_to, market, id";

/// `?1` make id, `?2` model id, `?3` year. In the order of the file.
pub const TIRES_FOR_VEHICLE: &str = "
SELECT size, size_rear, trims, pressure_front_kpa, pressure_rear_kpa, pressure_front_psi,
       pressure_rear_psi, pressure_unit, lug_torque_nm, lug_torque_lbft, lug_torque_unit,
       bolt_pattern, market, source, checked, reviewer
FROM tire_fitment
WHERE make_id = ?1 AND model_id = ?2 AND year = ?3
ORDER BY id";

fn text(row: &[Value], column: usize, table: &'static str) -> Result<String, ShopError> {
    row.get(column)
        .and_then(Value::text)
        .map(str::to_owned)
        .ok_or(ShopError::Shape(table))
}

fn optional_text(row: &[Value], column: usize) -> Option<String> {
    row.get(column).and_then(Value::text).map(str::to_owned)
}

fn integer(row: &[Value], column: usize) -> Option<i64> {
    row.get(column).and_then(Value::integer)
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
    /// Every fluid on record for one model year. Empty when nobody has
    /// recorded one.
    pub fn fluids_for_vehicle(
        &self,
        make_id: i64,
        model_id: i64,
        year: u16,
    ) -> Result<Vec<Fluid>, ShopError> {
        self.require(Dataset::Fluids)?;
        let rows = self.query(FLUIDS_FOR_VEHICLE, &model_year(make_id, model_id, year))?;
        rows.iter()
            .map(|row| {
                Ok(Fluid {
                    kind: text(row, 0, "fluid")?,
                    engine_id: integer(row, 1),
                    applies_to: optional_text(row, 2),
                    grade: optional_text(row, 3),
                    specification: optional_text(row, 4),
                    capacity_l: real(row, 5),
                    capacity_qt: real(row, 6),
                    capacity_unit: optional_text(row, 7),
                    with_filter: integer(row, 8).map(|flag| flag != 0),
                    charge_g: integer(row, 9),
                    charge_oz: real(row, 10),
                    charge_unit: optional_text(row, 11),
                    market: optional_text(row, 12),
                    source: text(row, 13, "fluid")?,
                    checked: text(row, 14, "fluid")?,
                    reviewer: text(row, 15, "fluid")?,
                })
            })
            .collect()
    }

    /// Every original-equipment tire fitment on record for one model
    /// year. Empty when nobody has recorded one.
    pub fn tires_for_vehicle(
        &self,
        make_id: i64,
        model_id: i64,
        year: u16,
    ) -> Result<Vec<TireFitment>, ShopError> {
        self.require(Dataset::Tires)?;
        let rows = self.query(TIRES_FOR_VEHICLE, &model_year(make_id, model_id, year))?;
        rows.iter()
            .map(|row| {
                let table = "tire_fitment";
                let trims: Vec<String> = serde_json::from_str(&text(row, 2, table)?)
                    .map_err(|_| ShopError::Shape(table))?;
                Ok(TireFitment {
                    size: text(row, 0, table)?,
                    size_rear: optional_text(row, 1),
                    trims,
                    pressure_front_kpa: integer(row, 3),
                    pressure_rear_kpa: integer(row, 4),
                    pressure_front_psi: integer(row, 5),
                    pressure_rear_psi: integer(row, 6),
                    pressure_unit: optional_text(row, 7),
                    lug_torque_nm: integer(row, 8),
                    lug_torque_lbft: integer(row, 9),
                    lug_torque_unit: optional_text(row, 10),
                    bolt_pattern: optional_text(row, 11),
                    market: optional_text(row, 12),
                    source: text(row, 13, table)?,
                    checked: text(row, 14, table)?,
                    reviewer: text(row, 15, table)?,
                })
            })
            .collect()
    }
}
