//! What owners reported to NHTSA about a model year, as counts.
//!
//! The data holds no complaint, only how many there are by component.
//! Every answer says the reports are not verified and that a count is not
//! a rate.
//!
//! The route is a thin wrapper around a function ending in `_op`, as in
//! `safety.rs`.

use axum::Json;
use axum::extract::rejection::PathRejection;
use axum::extract::{Path, State};
use wenmar_shopdata::Dataset;
use wenmar_shopdata::complaints::{self as shopdata, NHTSA_COMPLAINTS, NOTE};

use crate::api::recalls::{ModelYear, model_year};
use crate::api::types::{ComponentComplaints, VehicleComplaints};
use crate::error::{ApiError, ErrorBody};
use crate::shop::ShopDb;
use crate::state::AppState;

pub fn component(found: shopdata::ComponentComplaints) -> ComponentComplaints {
    ComponentComplaints {
        id: found.id,
        name: found.name,
        parent: found.parent,
        complaints: found.figures.complaints,
        crashes: found.figures.crashes,
        fires: found.figures.fires,
        injuries: found.figures.injuries,
        median_km: found.figures.median_km,
        median_miles: found.figures.median_miles,
    }
}

/// The complaints of one model year, as the API and the pages give them.
/// `shop` is the shop file, known to hold the complaints.
pub async fn complaints_of(
    shop: &ShopDb,
    found: &ModelYear,
) -> Result<VehicleComplaints, ApiError> {
    let (model_id, year) = (found.model_id, found.vehicle.year);
    let counts = shop
        .run(move |reader| reader.complaints_for_vehicle(model_id, year))
        .await??;
    let entry = shop.datasets().get(Dataset::Complaints);
    Ok(VehicleComplaints {
        vehicle: found.vehicle.clone(),
        as_of: entry.map(|entry| entry.as_of.clone()),
        verified: false,
        complaints: counts.total.complaints,
        crashes: counts.total.crashes,
        fires: counts.total.fires,
        injuries: counts.total.injuries,
        median_km: counts.total.median_km,
        median_miles: counts.total.median_miles,
        components: counts.components.into_iter().map(component).collect(),
        note: NOTE.to_owned(),
        report_url: NHTSA_COMPLAINTS.to_owned(),
        attribution: entry
            .map(|entry| entry.attribution.clone())
            .filter(|credit| !credit.is_empty())
            .into_iter()
            .collect(),
    })
}

pub async fn vehicle_complaints_op(
    state: &AppState,
    id: String,
) -> Result<VehicleComplaints, ApiError> {
    let shop = state.dataset(Dataset::Complaints)?;
    let found = state
        .db()
        .run(move |worker| model_year(worker, &id))
        .await??;
    let Some(found) = found else {
        return Err(ApiError::NotFound("No vehicle has that id.".to_owned()));
    };
    complaints_of(shop, &found).await
}

/// What owners reported to NHTSA about a model year, as counts by
/// component.
///
/// These are owners' reports. They are not verified, and a count is not a
/// rate. No complaint itself is available here: only how many there are.
#[utoipa::path(
    get,
    path = "/vehicles/{id}/complaints",
    tag = "Complaints",
    params(
        ("id" = String, Path, description = "A vehicle id. Complaints are counted by year, make and model; a submodel or an engine in the id is set aside.", example = "2016_honda_civic")
    ),
    responses(
        (status = 200, description = "The counts. All zero when no complaint names the model year.", body = VehicleComplaints),
        (status = 404, description = "`not_found`: no vehicle has that id. `dataset_unavailable`: this server holds no complaints.", body = ErrorBody)
    )
)]
pub async fn vehicle_complaints(
    State(state): State<AppState>,
    path: Result<Path<String>, PathRejection>,
) -> Result<Json<VehicleComplaints>, ApiError> {
    let Ok(Path(id)) = path else {
        return Err(ApiError::NotFound("No vehicle has that id.".to_owned()));
    };
    Ok(Json(vehicle_complaints_op(&state, id).await?))
}
