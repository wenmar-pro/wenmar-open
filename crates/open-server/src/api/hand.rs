//! The facts a person read from the maker's own documents: the fluids of
//! a model year beside EPA's oil grade, and its tire fitments.
//!
//! Every entry carries the document it was read from and the day. A
//! model year with no record answers with empty lists: nothing is
//! estimated and nothing is carried over from another year or engine.

use std::collections::HashMap;

use axum::Json;
use axum::extract::rejection::{PathRejection, QueryRejection};
use axum::extract::{Path, Query, State};
use serde_json::json;
use wenmar_shopdata::Dataset;
use wenmar_shopdata::hand as shopdata;

use crate::api::bad_query;
use crate::api::fuel::{VinYearQuery, attribution, decoded, engines_of};
use crate::api::recalls::ModelYear;
use crate::api::types::{EngineRef, FluidRecord, FrontRear, TireFitment, VehicleTires, VinTires};
use crate::error::{ApiError, ErrorBody};
use crate::shop::ShopDb;
use crate::state::AppState;

/// The hand-read fluids of one model year, by kind.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Recorded {
    pub engine_oil: Vec<FluidRecord>,
    pub transmission: Vec<FluidRecord>,
    pub coolant: Vec<FluidRecord>,
    pub refrigerant: Vec<FluidRecord>,
    pub brake: Vec<FluidRecord>,
}

impl Recorded {
    pub fn is_empty(&self) -> bool {
        self.engine_oil.is_empty()
            && self.transmission.is_empty()
            && self.coolant.is_empty()
            && self.refrigerant.is_empty()
            && self.brake.is_empty()
    }

    /// Keeps the records for `engine` and those for every engine.
    pub fn narrow(&mut self, engine: &EngineRef) {
        for list in [
            &mut self.engine_oil,
            &mut self.transmission,
            &mut self.coolant,
            &mut self.refrigerant,
            &mut self.brake,
        ] {
            list.retain(|record| record.engine.as_ref().is_none_or(|named| named == engine));
        }
    }
}

fn record(fluid: shopdata::Fluid, engines: &HashMap<i64, EngineRef>) -> FluidRecord {
    FluidRecord {
        engine: fluid.engine_id.and_then(|id| engines.get(&id).cloned()),
        applies_to: fluid.applies_to,
        grade: fluid.grade,
        specification: fluid.specification,
        capacity_l: fluid.capacity_l,
        capacity_qt: fluid.capacity_qt,
        capacity_written_in: fluid.capacity_unit,
        with_filter: fluid.with_filter,
        charge_g: fluid.charge_g,
        charge_oz: fluid.charge_oz,
        charge_written_in: fluid.charge_unit,
        market: fluid.market,
        source: fluid.source,
        checked: fluid.checked,
    }
}

/// The day of the newest record of a hand-kept dataset.
pub fn as_of(shop: &ShopDb, dataset: Dataset) -> Option<String> {
    shop.datasets()
        .get(dataset)
        .map(|entry| entry.as_of.clone())
}

/// The hand-read fluids of one model year. `shop` holds `fluids`.
pub async fn recorded_of(
    state: &AppState,
    shop: &ShopDb,
    found: &ModelYear,
) -> Result<Recorded, ApiError> {
    let (make_id, model_id, year) = (found.make_id, found.model_id, found.vehicle.year);
    let fluids = shop
        .run(move |reader| reader.fluids_for_vehicle(make_id, model_id, year))
        .await??;
    let engines = engines_of(state, found).await?;
    let mut recorded = Recorded::default();
    for fluid in fluids {
        // A row whose engine the catalog no longer has is left out: it
        // cannot be shown under an engine, and must not be shown as if it
        // were for every engine.
        if fluid.engine_id.is_some_and(|id| !engines.contains_key(&id)) {
            continue;
        }
        let list = match fluid.kind.as_str() {
            "engine_oil" => &mut recorded.engine_oil,
            "transmission" => &mut recorded.transmission,
            "coolant" => &mut recorded.coolant,
            "refrigerant" => &mut recorded.refrigerant,
            "brake" => &mut recorded.brake,
            _ => continue,
        };
        list.push(record(fluid, &engines));
    }
    Ok(recorded)
}

fn pair(front: Option<i64>, rear: Option<i64>) -> Option<FrontRear> {
    (front.is_some() || rear.is_some()).then_some(FrontRear { front, rear })
}

fn fitment(tire: shopdata::TireFitment) -> TireFitment {
    TireFitment {
        size: tire.size,
        size_rear: tire.size_rear,
        trims: tire.trims,
        pressure_kpa: pair(tire.pressure_front_kpa, tire.pressure_rear_kpa),
        pressure_psi: pair(tire.pressure_front_psi, tire.pressure_rear_psi),
        pressure_written_in: tire.pressure_unit,
        lug_torque_nm: tire.lug_torque_nm,
        lug_torque_lbft: tire.lug_torque_lbft,
        lug_torque_written_in: tire.lug_torque_unit,
        bolt_pattern: tire.bolt_pattern,
        market: tire.market,
        source: tire.source,
        checked: tire.checked,
    }
}

/// The tire fitments of one model year. `shop` holds `tires`.
pub async fn tires_of(shop: &ShopDb, found: &ModelYear) -> Result<VehicleTires, ApiError> {
    let (make_id, model_id, year) = (found.make_id, found.model_id, found.vehicle.year);
    let tires = shop
        .run(move |reader| reader.tires_for_vehicle(make_id, model_id, year))
        .await??;
    Ok(VehicleTires {
        vehicle: found.vehicle.clone(),
        as_of: as_of(shop, Dataset::Tires),
        fitments: tires.into_iter().map(fitment).collect(),
        attribution: attribution(shop, &[Dataset::Tires]),
    })
}

pub async fn vehicle_tires_op(state: &AppState, id: String) -> Result<VehicleTires, ApiError> {
    let shop = state.dataset(Dataset::Tires)?;
    let found = state
        .db()
        .run(move |worker| crate::api::recalls::model_year(worker, &id))
        .await??
        .ok_or_else(|| ApiError::NotFound("No vehicle has that id.".to_owned()))?;
    tires_of(shop, &found).await
}

pub async fn vin_tires_op(
    state: &AppState,
    vin: String,
    year: Option<u16>,
) -> Result<VinTires, ApiError> {
    let shop = state.dataset(Dataset::Tires)?;
    let decoded = decoded(state, vin, year).await?;
    let Some(found) = decoded.found else {
        return Ok(VinTires {
            decode: decoded.summary,
            vehicle: None,
            as_of: as_of(shop, Dataset::Tires),
            fitments: Vec::new(),
            attribution: attribution(shop, &[Dataset::Tires]),
            warnings: decoded.warnings,
        });
    };
    let answer = tires_of(shop, &found).await?;
    Ok(VinTires {
        decode: decoded.summary,
        vehicle: Some(answer.vehicle),
        as_of: answer.as_of,
        fitments: answer.fitments,
        attribution: answer.attribution,
        warnings: decoded.warnings,
    })
}

/// Original-equipment tire fitments of a model year.
///
/// Every size the maker fitted that a person has recorded, each with the
/// trims it covers, the cold pressures, the wheel nut torque and the bolt
/// pattern, the document it was read from (`source`) and the day
/// (`checked`). A model year usually has several; none is picked. An
/// empty list means nobody has recorded this model year, not that it has
/// no tires on file anywhere.
#[utoipa::path(
    get,
    path = "/vehicles/{id}/tires",
    tag = "Vehicles",
    params(("id" = String, Path, description = "A vehicle id. Fitments are listed by year, make and model; a submodel or an engine in the id is set aside.", example = "2018_ford_f-150")),
    responses(
        (status = 200, description = "The fitments, in the order they were recorded.", body = VehicleTires),
        (status = 404, description = "`not_found`: no vehicle has that id. `dataset_unavailable`: this server holds no tire fitments.", body = ErrorBody)
    )
)]
pub async fn vehicle_tires(
    State(state): State<AppState>,
    path: Result<Path<String>, PathRejection>,
) -> Result<Json<VehicleTires>, ApiError> {
    let Ok(Path(id)) = path else {
        return Err(ApiError::NotFound("No vehicle has that id.".to_owned()));
    };
    Ok(Json(vehicle_tires_op(&state, id).await?))
}

/// Tire fitments for the vehicle a VIN decodes to.
///
/// Every fitment of the model year: a VIN does not say which wheels a
/// vehicle was built with, so the list is not narrowed. Read the size on
/// the door placard.
#[utoipa::path(
    get,
    path = "/vin/{vin}/tires",
    tag = "VIN",
    params(
        ("vin" = String, Path, description = "A 17-character VIN.", example = "KM8K2CAB4PU001140"),
        VinYearQuery
    ),
    responses(
        (status = 200, description = "The fitments.", body = VinTires),
        (status = 400, description = "`invalid_vin`, or `validation_failed` for `year`.", body = ErrorBody),
        (status = 404, description = "`not_found`: no manufacturer is registered for the first three characters. `dataset_unavailable`: this server holds no tire fitments.", body = ErrorBody)
    )
)]
pub async fn vin_tires(
    State(state): State<AppState>,
    path: Result<Path<String>, PathRejection>,
    query: Result<Query<VinYearQuery>, QueryRejection>,
) -> Result<Json<VinTires>, ApiError> {
    let Ok(Path(vin)) = path else {
        return Err(ApiError::InvalidVin {
            message: "a VIN uses only digits and letters other than I, O and Q".to_owned(),
            details: json!({ "suggestions": [] }),
        });
    };
    let Query(query) = query.map_err(bad_query)?;
    Ok(Json(vin_tires_op(&state, vin, query.year).await?))
}
