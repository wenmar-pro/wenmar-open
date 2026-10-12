//! Fuel ratings, specifications and the certified engine oil grade of a
//! model year, and the same narrowed by a VIN.
//!
//! Every entry says how far it was matched in `confidence`. An entry
//! names a catalog engine only when the match settled one, and the VIN
//! routes narrow to the decoded engine only when the decode settles it.
//! Nothing here is shown more exactly than that.
//!
//! Each route is a thin wrapper around a function ending in `_op`, as in
//! `recalls.rs`.

use std::collections::HashMap;

use axum::Json;
use axum::extract::rejection::{PathRejection, QueryRejection};
use axum::extract::{Path, Query, State};
use serde::Deserialize;
use serde_json::json;
use utoipa::IntoParams;
use wenmar_shopdata::Dataset;
use wenmar_shopdata::fuel::{self as shopdata, FuelSource};
use wenmar_vehicles::text::slug;
use wenmar_vehicles::{CatalogError, Source, Value};

use crate::api::recalls::{ModelYear, NO_CATALOG_VEHICLE, model_year};
use crate::api::types::{
    Consumption, EngineOil, EngineRef, FuelAsOf, FuelRating, Mpg, RecallDecode, RecallVehicle,
    Spec, SpecEmission, VehicleFluids, VehicleFuel, VehicleSpecs, VinFluids, VinFuel, VinSpecs,
    Warning,
};
use crate::api::vin::decode_many;
use crate::api::{bad_query, blank_is_none};
use crate::db::Worker;
use crate::error::{ApiError, ErrorBody};
use crate::shop::ShopDb;
use crate::state::AppState;

/// What every oil grade here is: the label a page and an answer give it.
pub const OIL_SOURCE: &str = "EPA fuel economy certification";

/// Kilometres in a mile.
const KM_PER_MILE: f64 = 1.609_344;

/// `?1` make id, `?2` model id, `?3` year.
const ENGINES_SQL: &str = "
SELECT e.id, e.label, e.vin8
FROM catalog_vehicle v
JOIN catalog_engine e ON e.detail_id = v.detail_id
WHERE v.make_id = ?1 AND v.model_id = ?2 AND v.year = ?3
ORDER BY e.id";

/// The catalog engines of a model year, by `catalog_engine.id`.
pub fn engines(
    worker: &Worker,
    found: &ModelYear,
) -> Result<HashMap<i64, EngineRef>, CatalogError> {
    let rows = worker
        .source
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
            Some((
                id,
                EngineRef {
                    id: slug(label),
                    label: label.to_owned(),
                    vin8: row.get(2).and_then(Value::text).map(str::to_owned),
                },
            ))
        })
        .collect())
}

/// `?country=`: `us`, `ca`, or nothing for both.
pub fn country(text: Option<&str>) -> Result<Option<FuelSource>, ApiError> {
    let Some(text) = text.map(str::trim).filter(|text| !text.is_empty()) else {
        return Ok(None);
    };
    FuelSource::from_country(&text.to_ascii_lowercase())
        .map(Some)
        .ok_or_else(|| ApiError::validation("country must be us or ca."))
}

/// The shop file, if it holds the fuel ratings asked for.
pub fn fuel_shop(state: &AppState, source: Option<FuelSource>) -> Result<&ShopDb, ApiError> {
    match source {
        Some(source) => state.dataset(source.dataset()),
        None => state
            .dataset(Dataset::FuelUs)
            .or_else(|_| state.dataset(Dataset::FuelCa))
            .map_err(|_| ApiError::DatasetUnavailable(Dataset::FuelUs)),
    }
}

fn as_of_of(shop: &ShopDb, dataset: Dataset) -> Option<String> {
    shop.datasets()
        .get(dataset)
        .map(|entry| entry.as_of.clone())
}

pub fn as_of(shop: &ShopDb) -> FuelAsOf {
    FuelAsOf {
        us: as_of_of(shop, Dataset::FuelUs),
        ca: as_of_of(shop, Dataset::FuelCa),
    }
}

/// The credit lines of the datasets an answer drew on, each once.
pub fn attribution(shop: &ShopDb, datasets: &[Dataset]) -> Vec<String> {
    let mut lines: Vec<String> = Vec::new();
    for dataset in datasets {
        if let Some(entry) = shop.datasets().get(*dataset)
            && !entry.attribution.is_empty()
            && !lines.contains(&entry.attribution)
        {
            lines.push(entry.attribution.clone());
        }
    }
    lines
}

fn fuel_datasets(source: Option<FuelSource>) -> Vec<Dataset> {
    match source {
        Some(source) => vec![source.dataset()],
        None => vec![Dataset::FuelUs, Dataset::FuelCa],
    }
}

fn rating(rating: shopdata::FuelRating, engines: &HashMap<i64, EngineRef>) -> FuelRating {
    FuelRating {
        source: rating.source,
        country: rating.country,
        source_id: rating.source_id.map(|id| id.to_string()),
        matched: rating.matched,
        confidence: rating.confidence,
        engine: rating.engine_id.and_then(|id| engines.get(&id).cloned()),
        described_as: rating.model_text,
        displacement_l: rating.displacement_l,
        cylinders: rating.cylinders,
        transmission: rating.transmission,
        drive: rating.drive,
        fuel_grade: rating.fuel,
        second_fuel: rating.fuel2,
        l_100km: rating.l_100km.map(|figures| Consumption {
            city: figures.city,
            highway: figures.highway,
            combined: figures.combined,
        }),
        mpg: rating.mpg.map(|figures| Mpg {
            city: figures.city,
            highway: figures.highway,
            combined: figures.combined,
        }),
        kwh_100km: rating.kwh_100km,
        range_km: rating.range_km,
        range_mi: rating
            .range_km
            .map(|km| (f64::from(km) / KM_PER_MILE).round() as u32),
        co2_g_km: rating.co2_g_km,
    }
}

/// A model year by its vehicle id, or the `not_found` error.
async fn found_by_id(state: &AppState, id: String) -> Result<ModelYear, ApiError> {
    state
        .db()
        .run(move |worker| model_year(worker, &id))
        .await??
        .ok_or_else(|| ApiError::NotFound("No vehicle has that id.".to_owned()))
}

/// What a VIN decodes to, as far as these routes need it.
pub struct Decoded {
    pub summary: RecallDecode,
    pub found: Option<ModelYear>,
    /// The id form of the engine the decode settled, such as `2-0l`.
    pub engine: Option<String>,
    pub warnings: Vec<Warning>,
}

pub async fn decoded(
    state: &AppState,
    vin: String,
    year: Option<u16>,
) -> Result<Decoded, ApiError> {
    let mut decoded = decode_many(state, vec![vin], year).await?;
    let decode = match decoded.pop() {
        Some(result) => result?,
        None => return Err(ApiError::internal("a decode gave no answer")),
    };
    let selection = decode.catalog.as_ref();
    let engine = selection.and_then(|selection| selection.engine_id.clone());
    let found = match selection.map(|selection| selection.vehicle_id.clone()) {
        Some(id) => {
            state
                .db()
                .run(move |worker| model_year(worker, &id))
                .await??
        }
        None => None,
    };
    let mut warnings = decode.warnings;
    if found.is_none() {
        warnings.push(Warning {
            code: "no_catalog_vehicle".to_owned(),
            message: NO_CATALOG_VEHICLE.to_owned(),
            suggestions: Vec::new(),
        });
    }
    Ok(Decoded {
        summary: RecallDecode {
            vin: decode.vin,
            valid: decode.valid,
            year: decode.year,
            make: decode.make,
            model: decode.model,
            trim: decode.trim,
        },
        found,
        engine,
        warnings,
    })
}

/// The catalog engine a decode settled: its row id and how it is named.
pub fn settled(
    engines: &HashMap<i64, EngineRef>,
    engine: Option<&str>,
) -> Option<(i64, EngineRef)> {
    let wanted = engine?;
    engines
        .iter()
        .find(|(_, engine)| engine.id == wanted)
        .map(|(id, engine)| (*id, engine.clone()))
}

pub async fn engines_of(
    state: &AppState,
    found: &ModelYear,
) -> Result<HashMap<i64, EngineRef>, ApiError> {
    let found = found.clone();
    Ok(state
        .db()
        .run(move |worker| engines(worker, &found))
        .await??)
}

#[derive(Debug, Default, Deserialize, IntoParams)]
#[into_params(parameter_in = Query)]
pub struct VehicleFuelQuery {
    /// `us` for EPA's ratings, `ca` for Natural Resources Canada's.
    /// Leave out for both.
    pub country: Option<String>,
}

#[derive(Debug, Default, Deserialize, IntoParams)]
#[into_params(parameter_in = Query)]
pub struct VinFuelQuery {
    /// `us` or `ca`. Leave out for both.
    pub country: Option<String>,
    /// The model year, when the VIN's tenth character leaves two possible.
    #[serde(default, deserialize_with = "blank_is_none")]
    #[param(value_type = Option<u16>)]
    pub year: Option<u16>,
}

#[derive(Debug, Default, Deserialize, IntoParams)]
#[into_params(parameter_in = Query)]
pub struct VinYearQuery {
    /// The model year, when the VIN's tenth character leaves two possible.
    #[serde(default, deserialize_with = "blank_is_none")]
    #[param(value_type = Option<u16>)]
    pub year: Option<u16>,
}

/// The ratings of one model year, as the API and the pages give them.
pub async fn fuel_of(
    state: &AppState,
    shop: &ShopDb,
    found: &ModelYear,
    source: Option<FuelSource>,
) -> Result<VehicleFuel, ApiError> {
    let (make_id, model_id, year) = (found.make_id, found.model_id, found.vehicle.year);
    let ratings = shop
        .run(move |reader| reader.fuel_for_vehicle(make_id, model_id, year, source))
        .await??;
    let engines = engines_of(state, found).await?;
    Ok(VehicleFuel {
        vehicle: found.vehicle.clone(),
        as_of: as_of(shop),
        ratings: ratings
            .into_iter()
            .map(|found| rating(found, &engines))
            .collect(),
        attribution: attribution(shop, &fuel_datasets(source)),
    })
}

pub async fn vehicle_fuel_op(
    state: &AppState,
    id: String,
    query: VehicleFuelQuery,
) -> Result<VehicleFuel, ApiError> {
    let source = country(query.country.as_deref())?;
    let shop = fuel_shop(state, source)?;
    let found = found_by_id(state, id).await?;
    fuel_of(state, shop, &found, source).await
}

pub async fn vin_fuel_op(
    state: &AppState,
    vin: String,
    query: VinFuelQuery,
) -> Result<VinFuel, ApiError> {
    let source = country(query.country.as_deref())?;
    let shop = fuel_shop(state, source)?;
    let decoded = decoded(state, vin, query.year).await?;
    let Some(found) = decoded.found else {
        return Ok(VinFuel {
            decode: decoded.summary,
            vehicle: None,
            engine: None,
            engine_settled: false,
            as_of: as_of(shop),
            ratings: Vec::new(),
            attribution: attribution(shop, &fuel_datasets(source)),
            warnings: decoded.warnings,
        });
    };
    let mut answer = fuel_of(state, shop, &found, source).await?;
    let engines = engines_of(state, &found).await?;
    let engine = settled(&engines, decoded.engine.as_deref()).map(|(_, engine)| engine);
    if let Some(engine) = &engine {
        // Only what was matched to this very engine. A rating matched to
        // the model year alone cannot be said to be this vehicle's.
        answer
            .ratings
            .retain(|rating| rating.engine.as_ref() == Some(engine));
    }
    Ok(VinFuel {
        decode: decoded.summary,
        vehicle: Some(answer.vehicle),
        engine_settled: engine.is_some(),
        engine,
        as_of: answer.as_of,
        ratings: answer.ratings,
        attribution: answer.attribution,
        warnings: decoded.warnings,
    })
}

/// The specifications of one model year.
pub async fn specs_of(
    state: &AppState,
    shop: &ShopDb,
    found: &ModelYear,
) -> Result<VehicleSpecs, ApiError> {
    let (make_id, model_id, year) = (found.make_id, found.model_id, found.vehicle.year);
    let (specs, emissions) = shop
        .run(move |reader| {
            Ok::<_, wenmar_shopdata::ShopError>((
                reader.specs_for_vehicle(make_id, model_id, year)?,
                reader.emissions_for_vehicle(make_id, model_id, year)?,
            ))
        })
        .await??;
    let engines = engines_of(state, found).await?;
    Ok(VehicleSpecs {
        vehicle: found.vehicle.clone(),
        as_of: as_of_of(shop, Dataset::Specs),
        specs: specs
            .into_iter()
            .map(|spec| Spec {
                emission_standards: emissions
                    .iter()
                    .filter(|standard| standard.epa_id == spec.epa_id)
                    .map(|standard| SpecEmission {
                        sales_area: standard.sales_area,
                        standard: standard.standard.clone(),
                        standard_text: standard.standard_text.clone(),
                        smog_score: standard.smog_score,
                        engine_family: standard.engine_family.clone(),
                    })
                    .collect(),
                source: "epa".to_owned(),
                source_id: spec.epa_id.to_string(),
                confidence: spec.confidence,
                engine: spec.engine_id.and_then(|id| engines.get(&id).cloned()),
                described_as: spec.model_text,
                transmission: spec.transmission,
                transmission_type: spec.transmission_type,
                gears: spec.gears,
                drive: spec.drive,
                fuel_grade: spec.fuel,
                fuel_usage: spec.fuel_usage,
                size_class: spec.size_class,
                turbo: spec.turbo,
                supercharger: spec.supercharger,
                start_stop: spec.start_stop,
                electrification: spec.electrification,
                ev_motor: spec.ev_motor,
                cylinder_deactivation: spec.cylinder_deactivation,
                cylinder_deactivation_name: spec.cylinder_deactivation_name,
                variable_valve_timing: spec.variable_valve_timing,
                variable_valve_lift: spec.variable_valve_lift,
                intake_valves: spec.intake_valves,
                exhaust_valves: spec.exhaust_valves,
                passenger_volume_l: spec.passenger_ft3.map(litres),
                passenger_volume_ft3: spec.passenger_ft3,
                luggage_volume_l: spec.luggage_ft3.map(litres),
                luggage_volume_ft3: spec.luggage_ft3,
                battery_type: spec.battery_type,
                battery_voltage: spec.battery_voltage,
                motor_kw: spec.motor_kw,
            })
            .collect(),
        attribution: attribution(shop, &[Dataset::Specs]),
    })
}

/// Litres in a volume of cubic feet, to a whole number.
fn litres(cubic_feet: u16) -> u32 {
    (f64::from(cubic_feet) * 28.316_846_592).round() as u32
}

pub async fn vehicle_specs_op(state: &AppState, id: String) -> Result<VehicleSpecs, ApiError> {
    let shop = state.dataset(Dataset::Specs)?;
    let found = found_by_id(state, id).await?;
    specs_of(state, shop, &found).await
}

pub async fn vin_specs_op(
    state: &AppState,
    vin: String,
    year: Option<u16>,
) -> Result<VinSpecs, ApiError> {
    let shop = state.dataset(Dataset::Specs)?;
    let decoded = decoded(state, vin, year).await?;
    let Some(found) = decoded.found else {
        return Ok(VinSpecs {
            decode: decoded.summary,
            vehicle: None,
            engine: None,
            engine_settled: false,
            as_of: as_of_of(shop, Dataset::Specs),
            specs: Vec::new(),
            attribution: attribution(shop, &[Dataset::Specs]),
            warnings: decoded.warnings,
        });
    };
    let mut answer = specs_of(state, shop, &found).await?;
    let engines = engines_of(state, &found).await?;
    let engine = settled(&engines, decoded.engine.as_deref()).map(|(_, engine)| engine);
    if let Some(engine) = &engine {
        answer
            .specs
            .retain(|spec| spec.engine.as_ref() == Some(engine));
    }
    Ok(VinSpecs {
        decode: decoded.summary,
        vehicle: Some(answer.vehicle),
        engine_settled: engine.is_some(),
        engine,
        as_of: answer.as_of,
        specs: answer.specs,
        attribution: answer.attribution,
        warnings: decoded.warnings,
    })
}

/// The engine oil of one model year: one entry per engine.
pub async fn oil_of(
    state: &AppState,
    shop: &ShopDb,
    found: &ModelYear,
) -> Result<Vec<EngineOil>, ApiError> {
    let (make_id, model_id, year) = (found.make_id, found.model_id, found.vehicle.year);
    let oil = shop
        .run(move |reader| reader.oil_for_vehicle(make_id, model_id, year))
        .await??;
    let engines = engines_of(state, found).await?;
    Ok(oil
        .into_iter()
        .map(|oil| EngineOil {
            confidence: oil.confidence,
            engine: oil.engine_id.and_then(|id| engines.get(&id).cloned()),
            displacement_l: oil.displacement_l,
            cylinders: oil.cylinders,
            grades: oil.grades,
            as_written: oil.texts,
            source: OIL_SOURCE.to_owned(),
        })
        .collect())
}

fn fluids(shop: &ShopDb, vehicle: RecallVehicle, engine_oil: Vec<EngineOil>) -> VehicleFluids {
    VehicleFluids {
        vehicle,
        as_of: as_of_of(shop, Dataset::OilGrade),
        engine_oil,
        attribution: attribution(shop, &[Dataset::OilGrade]),
    }
}

pub async fn vehicle_fluids_op(state: &AppState, id: String) -> Result<VehicleFluids, ApiError> {
    let shop = state.dataset(Dataset::OilGrade)?;
    let found = found_by_id(state, id).await?;
    let engine_oil = oil_of(state, shop, &found).await?;
    Ok(fluids(shop, found.vehicle, engine_oil))
}

pub async fn vin_fluids_op(
    state: &AppState,
    vin: String,
    year: Option<u16>,
) -> Result<VinFluids, ApiError> {
    let shop = state.dataset(Dataset::OilGrade)?;
    let decoded = decoded(state, vin, year).await?;
    let Some(found) = decoded.found else {
        return Ok(VinFluids {
            decode: decoded.summary,
            vehicle: None,
            engine: None,
            engine_settled: false,
            as_of: as_of_of(shop, Dataset::OilGrade),
            engine_oil: Vec::new(),
            attribution: attribution(shop, &[Dataset::OilGrade]),
            warnings: decoded.warnings,
        });
    };
    let mut engine_oil = oil_of(state, shop, &found).await?;
    let engines = engines_of(state, &found).await?;
    let engine = settled(&engines, decoded.engine.as_deref()).map(|(_, engine)| engine);
    if let Some(engine) = &engine {
        engine_oil.retain(|oil| oil.engine.as_ref() == Some(engine));
    }
    let answer = fluids(shop, found.vehicle, engine_oil);
    Ok(VinFluids {
        decode: decoded.summary,
        vehicle: Some(answer.vehicle),
        engine_settled: engine.is_some(),
        engine,
        as_of: answer.as_of,
        engine_oil: answer.engine_oil,
        attribution: answer.attribution,
        warnings: decoded.warnings,
    })
}

/// The `fuel`, `specs` and `fluids` blocks of an overview, and their
/// credit lines. A block is `None` when the server does not hold its
/// dataset. With `engine`, the label of the engine a decode settled, each
/// block holds only what was matched to that engine.
pub async fn overview_blocks(
    state: &AppState,
    found: &ModelYear,
    engine: Option<&str>,
) -> Result<
    (
        Option<serde_json::Value>,
        Option<serde_json::Value>,
        Option<serde_json::Value>,
        Vec<String>,
    ),
    ApiError,
> {
    let is_engine = |named: &Option<EngineRef>| match engine {
        Some(label) => named.as_ref().is_some_and(|named| named.label == label),
        None => true,
    };
    let block = |value: Result<serde_json::Value, serde_json::Error>| {
        value.map_err(|error| ApiError::internal(error.to_string()))
    };
    let mut credit: Vec<String> = Vec::new();
    let mut add = |lines: Vec<String>| {
        for line in lines {
            if !credit.contains(&line) {
                credit.push(line);
            }
        }
    };
    let fuel = match fuel_shop(state, None) {
        Ok(shop) => {
            let mut answer = fuel_of(state, shop, found, None).await?;
            answer.ratings.retain(|rating| is_engine(&rating.engine));
            add(answer.attribution);
            Some(block(serde_json::to_value(&answer.ratings))?)
        }
        Err(_) => None,
    };
    let specs = match state.dataset(Dataset::Specs) {
        Ok(shop) => {
            let mut answer = specs_of(state, shop, found).await?;
            answer.specs.retain(|spec| is_engine(&spec.engine));
            add(answer.attribution);
            Some(block(serde_json::to_value(&answer.specs))?)
        }
        Err(_) => None,
    };
    let fluids = match state.dataset(Dataset::OilGrade) {
        Ok(shop) => {
            let mut oil = oil_of(state, shop, found).await?;
            oil.retain(|oil| is_engine(&oil.engine));
            add(attribution(shop, &[Dataset::OilGrade]));
            Some(json!({ "engine_oil": block(serde_json::to_value(&oil))? }))
        }
        Err(_) => None,
    };
    Ok((fuel, specs, fluids, credit))
}

fn no_vehicle() -> ApiError {
    ApiError::NotFound("No vehicle has that id.".to_owned())
}

fn bad_vin() -> ApiError {
    ApiError::InvalidVin {
        message: "a VIN uses only digits and letters other than I, O and Q".to_owned(),
        details: json!({ "suggestions": [] }),
    }
}

/// Fuel consumption ratings of a model year.
///
/// EPA's ratings and Natural Resources Canada's, each matched to the
/// model year. `confidence` says how far: `model`, `engine` or
/// `submodel`. `engine` is the catalog engine only when the match settled
/// one. `l_100km` and `mpg` (US gallons) are the same figures in two
/// units; both are null for a fuel not sold by the litre.
#[utoipa::path(
    get,
    path = "/vehicles/{id}/fuel",
    tag = "Vehicles",
    params(
        ("id" = String, Path, description = "A vehicle id. Ratings are listed by year, make and model; a submodel or an engine in the id is set aside.", example = "2018_ford_f-150"),
        VehicleFuelQuery
    ),
    responses(
        (status = 200, description = "The ratings, EPA's first, each by engine size.", body = VehicleFuel),
        (status = 400, description = "`validation_failed`: `country` is not `us` or `ca`.", body = ErrorBody),
        (status = 404, description = "`not_found`: no vehicle has that id. `dataset_unavailable`: this server holds no fuel ratings, or none for the country asked for.", body = ErrorBody)
    )
)]
pub async fn vehicle_fuel(
    State(state): State<AppState>,
    path: Result<Path<String>, PathRejection>,
    query: Result<Query<VehicleFuelQuery>, QueryRejection>,
) -> Result<Json<VehicleFuel>, ApiError> {
    let Ok(Path(id)) = path else {
        return Err(no_vehicle());
    };
    let Query(query) = query.map_err(bad_query)?;
    Ok(Json(vehicle_fuel_op(&state, id, query).await?))
}

/// Fuel consumption ratings for the vehicle a VIN decodes to.
///
/// When the decode settles the engine, `engine_settled` is `true` and
/// `ratings` holds only the ratings matched to that engine. Otherwise it
/// holds every rating of the model year, as the vehicle route does.
#[utoipa::path(
    get,
    path = "/vin/{vin}/fuel",
    tag = "VIN",
    params(
        ("vin" = String, Path, description = "A 17-character VIN.", example = "KM8K2CAB4PU001140"),
        VinFuelQuery
    ),
    responses(
        (status = 200, description = "The ratings.", body = VinFuel),
        (status = 400, description = "`invalid_vin`, or `validation_failed` for `country` or `year`.", body = ErrorBody),
        (status = 404, description = "`not_found`: no manufacturer is registered for the first three characters. `dataset_unavailable`: this server holds no fuel ratings, or none for the country asked for.", body = ErrorBody)
    )
)]
pub async fn vin_fuel(
    State(state): State<AppState>,
    path: Result<Path<String>, PathRejection>,
    query: Result<Query<VinFuelQuery>, QueryRejection>,
) -> Result<Json<VinFuel>, ApiError> {
    let Ok(Path(vin)) = path else {
        return Err(bad_vin());
    };
    let Query(query) = query.map_err(bad_query)?;
    Ok(Json(vin_fuel_op(&state, vin, query).await?))
}

/// Specifications of a model year from EPA's files.
///
/// One entry per EPA rating: transmission and gears, drive, recommended
/// fuel, forced induction, stop-start, cylinder deactivation, valves,
/// passenger and luggage volume, hybrid battery, and the emission
/// standards it was certified to. A field EPA does not give is null.
#[utoipa::path(
    get,
    path = "/vehicles/{id}/specs",
    tag = "Vehicles",
    params(("id" = String, Path, description = "A vehicle id.", example = "2018_ford_f-150")),
    responses(
        (status = 200, description = "The specifications, by engine size.", body = VehicleSpecs),
        (status = 404, description = "`not_found`: no vehicle has that id. `dataset_unavailable`: this server holds no specifications.", body = ErrorBody)
    )
)]
pub async fn vehicle_specs(
    State(state): State<AppState>,
    path: Result<Path<String>, PathRejection>,
) -> Result<Json<VehicleSpecs>, ApiError> {
    let Ok(Path(id)) = path else {
        return Err(no_vehicle());
    };
    Ok(Json(vehicle_specs_op(&state, id).await?))
}

/// Specifications for the vehicle a VIN decodes to.
///
/// Narrowed to the decoded engine when the decode settles it, as the
/// fuel route is.
#[utoipa::path(
    get,
    path = "/vin/{vin}/specs",
    tag = "VIN",
    params(
        ("vin" = String, Path, description = "A 17-character VIN.", example = "KM8K2CAB4PU001140"),
        VinYearQuery
    ),
    responses(
        (status = 200, description = "The specifications.", body = VinSpecs),
        (status = 400, description = "`invalid_vin`, or `validation_failed` for `year`.", body = ErrorBody),
        (status = 404, description = "`not_found`: no manufacturer is registered for the first three characters. `dataset_unavailable`: this server holds no specifications.", body = ErrorBody)
    )
)]
pub async fn vin_specs(
    State(state): State<AppState>,
    path: Result<Path<String>, PathRejection>,
    query: Result<Query<VinYearQuery>, QueryRejection>,
) -> Result<Json<VinSpecs>, ApiError> {
    let Ok(Path(vin)) = path else {
        return Err(bad_vin());
    };
    let Query(query) = query.map_err(bad_query)?;
    Ok(Json(vin_specs_op(&state, vin, query.year).await?))
}

/// Fluids of a model year. Today: the engine oil viscosity grade.
///
/// The grade is the one the maker certified the engine with, as EPA's
/// Fuel Economy Guide records it. It is not read from the owner's manual,
/// and it gives no capacity and no specification. One entry per engine.
#[utoipa::path(
    get,
    path = "/vehicles/{id}/fluids",
    tag = "Vehicles",
    params(("id" = String, Path, description = "A vehicle id.", example = "2018_ford_f-150")),
    responses(
        (status = 200, description = "The engine oil grades, by engine.", body = VehicleFluids),
        (status = 404, description = "`not_found`: no vehicle has that id. `dataset_unavailable`: this server holds no oil grades.", body = ErrorBody)
    )
)]
pub async fn vehicle_fluids(
    State(state): State<AppState>,
    path: Result<Path<String>, PathRejection>,
) -> Result<Json<VehicleFluids>, ApiError> {
    let Ok(Path(id)) = path else {
        return Err(no_vehicle());
    };
    Ok(Json(vehicle_fluids_op(&state, id).await?))
}

/// Fluids for the vehicle a VIN decodes to.
///
/// When the decode settles the engine, `engine_oil` holds the one entry
/// of that engine, or none. Otherwise it holds every engine's.
#[utoipa::path(
    get,
    path = "/vin/{vin}/fluids",
    tag = "VIN",
    params(
        ("vin" = String, Path, description = "A 17-character VIN.", example = "KM8K2CAB4PU001140"),
        VinYearQuery
    ),
    responses(
        (status = 200, description = "The engine oil grades.", body = VinFluids),
        (status = 400, description = "`invalid_vin`, or `validation_failed` for `year`.", body = ErrorBody),
        (status = 404, description = "`not_found`: no manufacturer is registered for the first three characters. `dataset_unavailable`: this server holds no oil grades.", body = ErrorBody)
    )
)]
pub async fn vin_fluids(
    State(state): State<AppState>,
    path: Result<Path<String>, PathRejection>,
    query: Result<Query<VinYearQuery>, QueryRejection>,
) -> Result<Json<VinFluids>, ApiError> {
    let Ok(Path(vin)) = path else {
        return Err(bad_vin());
    };
    let Query(query) = query.map_err(bad_query)?;
    Ok(Json(vin_fluids_op(&state, vin, query.year).await?))
}
