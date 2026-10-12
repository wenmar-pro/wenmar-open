//! Fuel ratings, specifications, fluids and tire fitments as the hosted
//! API words them, built from files.
//!
//! These are the steps of `open-server/src/api/fuel.rs` and
//! `api/hand.rs`, written once more over any [`Source`]. They build JSON
//! directly: the hosted server's types describe themselves to OpenAPI and
//! live in that crate, and a test there asks both the same questions and
//! compares the answers.
//!
//! The rules are the server's. A figure is never shown more exactly than
//! its match: a catalog engine is named only where the match settled one.
//! An answer for a VIN is narrowed to the decoded engine only when the
//! decode settles it. A hand-read record made for every engine is kept
//! in a narrowed answer; a tire list is never narrowed.

use std::collections::HashMap;

use serde_json::{Value as Json, json};
use wenmar_vehicles::text::slug;
use wenmar_vehicles::{CatalogError, Source, Value};

use super::{AnswerError, Answers, ModelYear, NO_CATALOG_VEHICLE};
use crate::dataset::Dataset;
use crate::error::ShopError;
use crate::fuel::FuelSource;

/// What every EPA oil grade is labelled with.
pub const OIL_SOURCE: &str = "EPA fuel economy certification";

/// Kilometres in a mile.
const KM_PER_MILE: f64 = 1.609_344;

/// Litres in a cubic foot.
const LITRES_PER_FT3: f64 = 28.316_846_592;

/// `?1` make id, `?2` model id, `?3` year. On the core data file.
pub const ENGINES_SQL: &str = "
SELECT e.id, e.label, e.vin8
FROM catalog_vehicle v
JOIN catalog_engine e ON e.detail_id = v.detail_id
WHERE v.make_id = ?1 AND v.model_id = ?2 AND v.year = ?3
ORDER BY e.id";

/// The four blocks of an overview this module fills, and their credit
/// lines. A block is `None` when the shop file does not hold its data.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Blocks {
    pub fuel: Option<Json>,
    pub specs: Option<Json>,
    pub fluids: Option<Json>,
    pub tires: Option<Json>,
    pub attribution: Vec<String>,
}

/// `country` of a fuel question: `us`, `ca` (either case), or nothing
/// for both.
pub fn fuel_country(text: Option<&str>) -> Result<Option<FuelSource>, AnswerError> {
    match text.map(str::trim).filter(|text| !text.is_empty()) {
        None => Ok(None),
        Some(text) => FuelSource::from_country(&text.to_ascii_lowercase())
            .map(Some)
            .ok_or(AnswerError::Validation {
                field: "country",
                message: "country must be us or ca",
            }),
    }
}

/// The dataset a question about `topic` names as missing when there is no
/// shop file at all: what the hosted API names on a server without one.
pub fn dataset_for(topic: &str, country: Option<&str>) -> Dataset {
    match topic {
        "specs" => Dataset::Specs,
        "fluids" => Dataset::OilGrade,
        "tires" => Dataset::Tires,
        _ => match country
            .map(|text| text.trim().to_ascii_lowercase())
            .as_deref()
        {
            Some("ca") => Dataset::FuelCa,
            _ => Dataset::FuelUs,
        },
    }
}

/// What a VIN decoded to, as these answers give it, and the decode's
/// warnings. `decode` is the JSON of `GET /v1/vin/{vin}`.
fn summary(decode: &Json) -> (Json, Vec<Json>) {
    let field = |key: &str| decode.get(key).cloned().unwrap_or(Json::Null);
    let warnings = decode
        .get("warnings")
        .and_then(Json::as_array)
        .cloned()
        .unwrap_or_default();
    (
        json!({
            "vin": field("vin"),
            "valid": decode.get("valid").and_then(Json::as_bool).unwrap_or(false),
            "year": field("year"),
            "make": field("make"),
            "model": field("model"),
            "trim": field("trim"),
        }),
        warnings,
    )
}

fn no_catalog_vehicle() -> Json {
    json!({ "code": "no_catalog_vehicle", "message": NO_CATALOG_VEHICLE })
}

/// Adds each line once, in order.
fn add(credit: &mut Vec<String>, lines: Vec<String>) {
    for line in lines {
        if !credit.contains(&line) {
            credit.push(line);
        }
    }
}

/// Whether an entry is for the engine a decode settled. `wanted` is that
/// engine as [`engines`] gives it, or `None` when nothing is narrowed.
fn for_engine(entry: &Json, wanted: Option<&Json>) -> bool {
    wanted.is_none_or(|wanted| entry.get("engine") == Some(wanted))
}

/// The same for a hand-read record, which may be for every engine.
fn for_engine_or_all(entry: &Json, wanted: Option<&Json>) -> bool {
    entry.get("engine").is_none_or(Json::is_null) || for_engine(entry, wanted)
}

impl<S: Source, C: Source> Answers<'_, S, C> {
    /// The catalog engines of a model year, by `catalog_engine.id`, each
    /// as the API names it: `{ "id", "label", "vin8" }`.
    fn engines(&self, found: &ModelYear) -> Result<HashMap<i64, Json>, AnswerError> {
        let rows = self
            .core
            .query(
                ENGINES_SQL,
                &[
                    found.make_id.into(),
                    found.model_id.into(),
                    i64::from(found.vehicle.year).into(),
                ],
            )
            .map_err(CatalogError::Source)?;
        Ok(rows
            .iter()
            .filter_map(|row| {
                let id = row.first().and_then(Value::integer)?;
                let label = row.get(1).and_then(Value::text)?;
                let vin8 = row.get(2).and_then(Value::text);
                Some((
                    id,
                    json!({ "id": slug(label), "label": label, "vin8": vin8 }),
                ))
            })
            .collect())
    }

    /// The engine a decode settled, as [`Self::engines`] names it.
    fn settled(&self, found: &ModelYear, decode: &Json) -> Result<Option<Json>, AnswerError> {
        let Some(wanted) = decode
            .get("catalog")
            .and_then(|catalog| catalog.get("engine_id"))
            .and_then(Json::as_str)
        else {
            return Ok(None);
        };
        Ok(self
            .engines(found)?
            .into_values()
            .find(|engine| engine["id"] == wanted))
    }

    /// The model year a decode reached.
    fn decoded(&self, decode: &Json) -> Result<Option<ModelYear>, AnswerError> {
        match decode
            .get("catalog")
            .and_then(|catalog| catalog.get("vehicle_id"))
            .and_then(Json::as_str)
        {
            Some(id) => self.model_year(id),
            None => Ok(None),
        }
    }

    fn vehicle_by_id(&self, id: &str) -> Result<ModelYear, AnswerError> {
        self.model_year(id)?
            .ok_or(AnswerError::NotFound("No vehicle has that id."))
    }

    fn day(&self, dataset: Dataset) -> Option<String> {
        self.shop
            .datasets()
            .get(dataset)
            .map(|entry| entry.as_of.clone())
    }

    /// The credit lines of the datasets an answer drew on, each once.
    fn credit(&self, datasets: &[Dataset]) -> Vec<String> {
        let mut lines = Vec::new();
        for dataset in datasets {
            if let Some(entry) = self.shop.datasets().get(*dataset)
                && !entry.attribution.is_empty()
                && !lines.contains(&entry.attribution)
            {
                lines.push(entry.attribution.clone());
            }
        }
        lines
    }

    /// Whether the file holds what a question about `topic` (`fuel`,
    /// `specs`, `fluids` or `tires`) needs. The API checks this before it
    /// decodes a VIN, so a caller that decodes does the same.
    pub fn require(&self, topic: &str, country: Option<&str>) -> Result<(), AnswerError> {
        match topic {
            "fuel" => {
                self.shop.require_fuel(fuel_country(country)?)?;
            }
            "specs" => self.shop.require(Dataset::Specs)?,
            "fluids" => {
                self.fluid_datasets()?;
            }
            "tires" => self.shop.require(Dataset::Tires)?,
            _ => {}
        }
        Ok(())
    }

    // ----- fuel -----

    fn fuel_days(&self) -> Json {
        json!({ "us": self.day(Dataset::FuelUs), "ca": self.day(Dataset::FuelCa) })
    }

    fn fuel_credit(&self, source: Option<FuelSource>) -> Vec<String> {
        match source {
            Some(source) => self.credit(&[source.dataset()]),
            None => self.credit(&[Dataset::FuelUs, Dataset::FuelCa]),
        }
    }

    fn ratings(
        &self,
        found: &ModelYear,
        source: Option<FuelSource>,
    ) -> Result<Vec<Json>, AnswerError> {
        let ratings = self.shop.fuel_for_vehicle(
            found.make_id,
            found.model_id,
            found.vehicle.year,
            source,
        )?;
        let engines = self.engines(found)?;
        Ok(ratings
            .into_iter()
            .map(|rating| {
                json!({
                    "source": rating.source,
                    "country": rating.country,
                    "source_id": rating.source_id.map(|id| id.to_string()),
                    "match": rating.matched,
                    "confidence": rating.confidence,
                    "engine": rating.engine_id.and_then(|id| engines.get(&id)),
                    "described_as": rating.model_text,
                    "displacement_l": rating.displacement_l,
                    "cylinders": rating.cylinders,
                    "transmission": rating.transmission,
                    "drive": rating.drive,
                    "fuel_grade": rating.fuel,
                    "second_fuel": rating.fuel2,
                    "l_100km": rating.l_100km,
                    "mpg": rating.mpg,
                    "kwh_100km": rating.kwh_100km,
                    "range_km": rating.range_km,
                    "range_mi": rating
                        .range_km
                        .map(|km| (f64::from(km) / KM_PER_MILE).round() as u32),
                    "co2_g_km": rating.co2_g_km,
                })
            })
            .collect())
    }

    /// `GET /v1/vehicles/{id}/fuel`.
    pub fn vehicle_fuel(&self, id: &str, country: Option<&str>) -> Result<Json, AnswerError> {
        let source = fuel_country(country)?;
        self.shop.require_fuel(source)?;
        let found = self.vehicle_by_id(id)?;
        Ok(json!({
            "vehicle": found.vehicle,
            "as_of": self.fuel_days(),
            "ratings": self.ratings(&found, source)?,
            "attribution": self.fuel_credit(source),
        }))
    }

    /// `GET /v1/vin/{vin}/fuel`. `decode` is the JSON of the decode, as
    /// `GET /v1/vin/{vin}` gives it, with its `catalog` when it has one.
    pub fn vin_fuel(&self, decode: &Json, country: Option<&str>) -> Result<Json, AnswerError> {
        let source = fuel_country(country)?;
        self.shop.require_fuel(source)?;
        let (summary, mut warnings) = summary(decode);
        let Some(found) = self.decoded(decode)? else {
            warnings.push(no_catalog_vehicle());
            return Ok(json!({
                "decode": summary, "vehicle": null, "engine": null, "engine_settled": false,
                "as_of": self.fuel_days(), "ratings": [],
                "attribution": self.fuel_credit(source), "warnings": warnings,
            }));
        };
        let engine = self.settled(&found, decode)?;
        let mut ratings = self.ratings(&found, source)?;
        ratings.retain(|rating| for_engine(rating, engine.as_ref()));
        Ok(json!({
            "decode": summary, "vehicle": found.vehicle,
            "engine_settled": engine.is_some(), "engine": engine,
            "as_of": self.fuel_days(), "ratings": ratings,
            "attribution": self.fuel_credit(source), "warnings": warnings,
        }))
    }

    // ----- specifications -----

    fn specs(&self, found: &ModelYear) -> Result<Vec<Json>, AnswerError> {
        let (make, model, year) = (found.make_id, found.model_id, found.vehicle.year);
        let specs = self.shop.specs_for_vehicle(make, model, year)?;
        let emissions = self.shop.emissions_for_vehicle(make, model, year)?;
        let engines = self.engines(found)?;
        let litres = |cubic_feet: u16| (f64::from(cubic_feet) * LITRES_PER_FT3).round() as u32;
        Ok(specs
            .into_iter()
            .map(|spec| {
                let standards: Vec<Json> = emissions
                    .iter()
                    .filter(|standard| standard.epa_id == spec.epa_id)
                    .map(|standard| {
                        json!({
                            "sales_area": standard.sales_area,
                            "standard": standard.standard,
                            "standard_text": standard.standard_text,
                            "smog_score": standard.smog_score,
                            "engine_family": standard.engine_family,
                        })
                    })
                    .collect();
                json!({
                    "emission_standards": standards,
                    "source": "epa",
                    "source_id": spec.epa_id.to_string(),
                    "confidence": spec.confidence,
                    "engine": spec.engine_id.and_then(|id| engines.get(&id)),
                    "described_as": spec.model_text,
                    "transmission": spec.transmission,
                    "transmission_type": spec.transmission_type,
                    "gears": spec.gears,
                    "drive": spec.drive,
                    "fuel_grade": spec.fuel,
                    "fuel_usage": spec.fuel_usage,
                    "size_class": spec.size_class,
                    "turbo": spec.turbo,
                    "supercharger": spec.supercharger,
                    "start_stop": spec.start_stop,
                    "electrification": spec.electrification,
                    "ev_motor": spec.ev_motor,
                    "cylinder_deactivation": spec.cylinder_deactivation,
                    "cylinder_deactivation_name": spec.cylinder_deactivation_name,
                    "variable_valve_timing": spec.variable_valve_timing,
                    "variable_valve_lift": spec.variable_valve_lift,
                    "intake_valves": spec.intake_valves,
                    "exhaust_valves": spec.exhaust_valves,
                    "passenger_volume_l": spec.passenger_ft3.map(litres),
                    "passenger_volume_ft3": spec.passenger_ft3,
                    "luggage_volume_l": spec.luggage_ft3.map(litres),
                    "luggage_volume_ft3": spec.luggage_ft3,
                    "battery_type": spec.battery_type,
                    "battery_voltage": spec.battery_voltage,
                    "motor_kw": spec.motor_kw,
                })
            })
            .collect())
    }

    /// `GET /v1/vehicles/{id}/specs`.
    pub fn vehicle_specs(&self, id: &str) -> Result<Json, AnswerError> {
        self.shop.require(Dataset::Specs)?;
        let found = self.vehicle_by_id(id)?;
        Ok(json!({
            "vehicle": found.vehicle,
            "as_of": self.day(Dataset::Specs),
            "specs": self.specs(&found)?,
            "attribution": self.credit(&[Dataset::Specs]),
        }))
    }

    /// `GET /v1/vin/{vin}/specs`.
    pub fn vin_specs(&self, decode: &Json) -> Result<Json, AnswerError> {
        self.shop.require(Dataset::Specs)?;
        let (summary, mut warnings) = summary(decode);
        let Some(found) = self.decoded(decode)? else {
            warnings.push(no_catalog_vehicle());
            return Ok(json!({
                "decode": summary, "vehicle": null, "engine": null, "engine_settled": false,
                "as_of": self.day(Dataset::Specs), "specs": [],
                "attribution": self.credit(&[Dataset::Specs]), "warnings": warnings,
            }));
        };
        let engine = self.settled(&found, decode)?;
        let mut specs = self.specs(&found)?;
        specs.retain(|spec| for_engine(spec, engine.as_ref()));
        Ok(json!({
            "decode": summary, "vehicle": found.vehicle,
            "engine_settled": engine.is_some(), "engine": engine,
            "as_of": self.day(Dataset::Specs), "specs": specs,
            "attribution": self.credit(&[Dataset::Specs]), "warnings": warnings,
        }))
    }

    // ----- fluids -----

    /// Whether the file holds EPA's oil grades, and the hand-read records.
    /// Fails as the API does when it holds neither.
    fn fluid_datasets(&self) -> Result<(bool, bool), AnswerError> {
        let oil = self.shop.require(Dataset::OilGrade).is_ok();
        let recorded = self.shop.require(Dataset::Fluids).is_ok();
        if !oil && !recorded {
            return Err(ShopError::DatasetUnavailable(Dataset::OilGrade).into());
        }
        Ok((oil, recorded))
    }

    /// EPA's grade of each engine of a model year.
    fn engine_oil(&self, found: &ModelYear) -> Result<Vec<Json>, AnswerError> {
        let oil = self
            .shop
            .oil_for_vehicle(found.make_id, found.model_id, found.vehicle.year)?;
        let engines = self.engines(found)?;
        Ok(oil
            .into_iter()
            .map(|oil| {
                json!({
                    "confidence": oil.confidence,
                    "engine": oil.engine_id.and_then(|id| engines.get(&id)),
                    "displacement_l": oil.displacement_l,
                    "cylinders": oil.cylinders,
                    "grades": oil.grades,
                    "as_written": oil.texts,
                    "source": OIL_SOURCE,
                })
            })
            .collect())
    }

    /// The hand-read records of a model year, in the five lists of the
    /// API: engine oil, transmission, coolant, refrigerant, brake.
    fn recorded(&self, found: &ModelYear) -> Result<[Vec<Json>; 5], AnswerError> {
        let fluids =
            self.shop
                .fluids_for_vehicle(found.make_id, found.model_id, found.vehicle.year)?;
        let engines = self.engines(found)?;
        let mut lists: [Vec<Json>; 5] = Default::default();
        for fluid in fluids {
            // A record whose engine the catalog no longer has is left
            // out, as the server leaves it out.
            if fluid.engine_id.is_some_and(|id| !engines.contains_key(&id)) {
                continue;
            }
            let Some(list) = crate::hand::FLUID_KINDS
                .iter()
                .position(|kind| *kind == fluid.kind)
            else {
                continue;
            };
            lists[list].push(json!({
                "engine": fluid.engine_id.and_then(|id| engines.get(&id)),
                "applies_to": fluid.applies_to,
                "grade": fluid.grade,
                "specification": fluid.specification,
                "capacity_l": fluid.capacity_l,
                "capacity_qt": fluid.capacity_qt,
                "capacity_written_in": fluid.capacity_unit,
                "with_filter": fluid.with_filter,
                "charge_g": fluid.charge_g,
                "charge_oz": fluid.charge_oz,
                "charge_written_in": fluid.charge_unit,
                "market": fluid.market,
                "source": fluid.source,
                "checked": fluid.checked,
            }));
        }
        Ok(lists)
    }

    /// The lists of a fluids answer for one model year, narrowed to
    /// `engine` when a decode settled one: EPA's grades, then the five
    /// hand-read lists.
    fn fluid_lists(
        &self,
        found: &ModelYear,
        engine: Option<&Json>,
    ) -> Result<(Vec<Json>, [Vec<Json>; 5]), AnswerError> {
        let (oil, recorded) = self.fluid_datasets()?;
        let mut grades = if oil {
            self.engine_oil(found)?
        } else {
            Vec::new()
        };
        grades.retain(|grade| for_engine(grade, engine));
        let mut lists = if recorded {
            self.recorded(found)?
        } else {
            Default::default()
        };
        for list in &mut lists {
            list.retain(|record| for_engine_or_all(record, engine));
        }
        Ok((grades, lists))
    }

    /// The fields every fluids answer has, around what the caller adds.
    fn fluids_answer(&self, mut answer: Json, grades: Vec<Json>, lists: [Vec<Json>; 5]) -> Json {
        let [oil, transmission, coolant, refrigerant, brake] = lists;
        if let Some(fields) = answer.as_object_mut() {
            fields.insert("as_of".to_owned(), json!(self.day(Dataset::OilGrade)));
            fields.insert("engine_oil".to_owned(), json!(grades));
            fields.insert("checked_as_of".to_owned(), json!(self.day(Dataset::Fluids)));
            fields.insert("engine_oil_checked".to_owned(), json!(oil));
            fields.insert("transmission".to_owned(), json!(transmission));
            fields.insert("coolant".to_owned(), json!(coolant));
            fields.insert("refrigerant".to_owned(), json!(refrigerant));
            fields.insert("brake".to_owned(), json!(brake));
            fields.insert(
                "attribution".to_owned(),
                json!(self.credit(&[Dataset::OilGrade, Dataset::Fluids])),
            );
        }
        answer
    }

    /// `GET /v1/vehicles/{id}/fluids`.
    pub fn vehicle_fluids(&self, id: &str) -> Result<Json, AnswerError> {
        self.fluid_datasets()?;
        let found = self.vehicle_by_id(id)?;
        let (grades, lists) = self.fluid_lists(&found, None)?;
        Ok(self.fluids_answer(json!({ "vehicle": found.vehicle }), grades, lists))
    }

    /// `GET /v1/vin/{vin}/fluids`.
    pub fn vin_fluids(&self, decode: &Json) -> Result<Json, AnswerError> {
        self.fluid_datasets()?;
        let (summary, mut warnings) = summary(decode);
        let Some(found) = self.decoded(decode)? else {
            warnings.push(no_catalog_vehicle());
            let head = json!({
                "decode": summary, "vehicle": null, "engine": null, "engine_settled": false,
                "warnings": warnings,
            });
            return Ok(self.fluids_answer(head, Vec::new(), Default::default()));
        };
        let engine = self.settled(&found, decode)?;
        let (grades, lists) = self.fluid_lists(&found, engine.as_ref())?;
        let head = json!({
            "decode": summary, "vehicle": found.vehicle,
            "engine_settled": engine.is_some(), "engine": engine,
            "warnings": warnings,
        });
        Ok(self.fluids_answer(head, grades, lists))
    }

    // ----- tires -----

    fn fitments(&self, found: &ModelYear) -> Result<Vec<Json>, AnswerError> {
        let tires =
            self.shop
                .tires_for_vehicle(found.make_id, found.model_id, found.vehicle.year)?;
        let pair = |front: Option<i64>, rear: Option<i64>| {
            (front.is_some() || rear.is_some()).then(|| json!({ "front": front, "rear": rear }))
        };
        Ok(tires
            .into_iter()
            .map(|tire| {
                json!({
                    "size": tire.size,
                    "size_rear": tire.size_rear,
                    "trims": tire.trims,
                    "pressure_kpa": pair(tire.pressure_front_kpa, tire.pressure_rear_kpa),
                    "pressure_psi": pair(tire.pressure_front_psi, tire.pressure_rear_psi),
                    "pressure_written_in": tire.pressure_unit,
                    "lug_torque_nm": tire.lug_torque_nm,
                    "lug_torque_lbft": tire.lug_torque_lbft,
                    "lug_torque_written_in": tire.lug_torque_unit,
                    "bolt_pattern": tire.bolt_pattern,
                    "market": tire.market,
                    "source": tire.source,
                    "checked": tire.checked,
                })
            })
            .collect())
    }

    /// `GET /v1/vehicles/{id}/tires`.
    pub fn vehicle_tires(&self, id: &str) -> Result<Json, AnswerError> {
        self.shop.require(Dataset::Tires)?;
        let found = self.vehicle_by_id(id)?;
        Ok(json!({
            "vehicle": found.vehicle,
            "as_of": self.day(Dataset::Tires),
            "fitments": self.fitments(&found)?,
            "attribution": self.credit(&[Dataset::Tires]),
        }))
    }

    /// `GET /v1/vin/{vin}/tires`. Every fitment of the model year: a VIN
    /// does not name the wheels.
    pub fn vin_tires(&self, decode: &Json) -> Result<Json, AnswerError> {
        self.shop.require(Dataset::Tires)?;
        let (summary, mut warnings) = summary(decode);
        let found = self.decoded(decode)?;
        if found.is_none() {
            warnings.push(no_catalog_vehicle());
        }
        let fitments = match &found {
            Some(found) => self.fitments(found)?,
            None => Vec::new(),
        };
        Ok(json!({
            "decode": summary,
            "vehicle": found.map(|found| found.vehicle),
            "as_of": self.day(Dataset::Tires),
            "fitments": fitments,
            "attribution": self.credit(&[Dataset::Tires]),
            "warnings": warnings,
        }))
    }

    // ----- the overview -----

    /// The `fuel`, `specs`, `fluids` and `tires` blocks of an overview.
    /// With `engine`, the label of the engine a decode settled, the first
    /// three hold only what is for that engine.
    pub(super) fn blocks(
        &self,
        found: &ModelYear,
        engine: Option<&str>,
    ) -> Result<Blocks, AnswerError> {
        let fuel = self.shop.require_fuel(None).is_ok();
        let specs = self.shop.require(Dataset::Specs).is_ok();
        let fluids = self.fluid_datasets().is_ok();
        let tires = self.shop.require(Dataset::Tires).is_ok();
        // Nothing is read for a file that holds none of them.
        if !(fuel || specs || fluids || tires) {
            return Ok(Blocks::default());
        }
        let wanted = match engine {
            Some(label) => self
                .engines(found)?
                .into_values()
                .find(|named| named["label"] == label),
            None => None,
        };
        // A decode that settled an engine the catalog does not list for
        // the model year narrows to nothing, as on the server.
        let nothing = json!({ "label": engine });
        let wanted = match (engine, &wanted) {
            (Some(_), None) => Some(&nothing),
            _ => wanted.as_ref(),
        };
        let mut blocks = Blocks::default();
        if fuel {
            let mut ratings = self.ratings(found, None)?;
            ratings.retain(|rating| for_engine(rating, wanted));
            add(&mut blocks.attribution, self.fuel_credit(None));
            blocks.fuel = Some(json!(ratings));
        }
        if specs {
            let mut list = self.specs(found)?;
            list.retain(|spec| for_engine(spec, wanted));
            add(&mut blocks.attribution, self.credit(&[Dataset::Specs]));
            blocks.specs = Some(json!(list));
        }
        if fluids {
            let (grades, lists) = self.fluid_lists(found, wanted)?;
            let [oil, transmission, coolant, refrigerant, brake] = lists;
            add(
                &mut blocks.attribution,
                self.credit(&[Dataset::OilGrade, Dataset::Fluids]),
            );
            blocks.fluids = Some(json!({
                "engine_oil": grades,
                "engine_oil_checked": oil,
                "transmission": transmission,
                "coolant": coolant,
                "refrigerant": refrigerant,
                "brake": brake,
            }));
        }
        if tires {
            add(&mut blocks.attribution, self.credit(&[Dataset::Tires]));
            blocks.tires = Some(json!(self.fitments(found)?));
        }
        Ok(blocks)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_country_of_a_fuel_question_is_us_or_ca_or_nothing() {
        assert_eq!(fuel_country(None).unwrap(), None);
        assert_eq!(fuel_country(Some("  ")).unwrap(), None);
        assert_eq!(fuel_country(Some("US")).unwrap(), Some(FuelSource::Epa));
        assert_eq!(fuel_country(Some(" ca ")).unwrap(), Some(FuelSource::Nrcan));
        let error = fuel_country(Some("mx")).unwrap_err();
        assert!(
            matches!(error, AnswerError::Validation { field: "country", message } if message == "country must be us or ca")
        );
    }

    #[test]
    fn the_dataset_named_as_missing_follows_the_question() {
        assert_eq!(dataset_for("fuel", None), Dataset::FuelUs);
        assert_eq!(dataset_for("fuel", Some(" CA ")), Dataset::FuelCa);
        assert_eq!(dataset_for("specs", Some("ca")), Dataset::Specs);
        assert_eq!(dataset_for("fluids", None), Dataset::OilGrade);
        assert_eq!(dataset_for("tires", None), Dataset::Tires);
    }

    #[test]
    fn an_answer_is_narrowed_to_an_engine_and_a_record_for_all_is_kept() {
        let wanted = json!({ "id": "2-0l", "label": "2.0L", "vin8": "A" });
        let same = json!({ "engine": wanted.clone() });
        let other =
            json!({ "engine": { "id": "1-6l-turbo", "label": "1.6L Turbo", "vin8": null } });
        let all = json!({ "engine": null });
        assert!(for_engine(&same, Some(&wanted)));
        assert!(!for_engine(&other, Some(&wanted)));
        assert!(!for_engine(&all, Some(&wanted)));
        assert!(for_engine(&other, None) && for_engine(&all, None));
        assert!(for_engine_or_all(&all, Some(&wanted)));
        assert!(!for_engine_or_all(&other, Some(&wanted)));
        let (summary, warnings) = summary(&json!({
            "vin": "KM8K2CAB4PU001140", "valid": true, "year": 2023, "make": "Hyundai",
            "warnings": [{ "code": "x" }], "engine": { "label": "2.0L" },
        }));
        assert_eq!(
            summary,
            json!({ "vin": "KM8K2CAB4PU001140", "valid": true, "year": 2023, "make": "Hyundai", "model": null, "trim": null })
        );
        assert_eq!(warnings, [json!({ "code": "x" })]);
    }
}
