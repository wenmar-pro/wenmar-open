//! NHTSA defect investigations: the open ones that list a model year, and
//! the ones that led to a recall campaign.
//!
//! An investigation is not a finding of a defect, and it lists a make, a
//! model and a model year, never one vehicle. Every answer here carries a
//! `note` that says so.
//!
//! Each route is a thin wrapper around a function ending in `_op`, as in
//! `recalls.rs`.

use axum::Json;
use axum::extract::rejection::{PathRejection, QueryRejection};
use axum::extract::{Path, Query, State};
use serde::Deserialize;
use serde_json::json;
use utoipa::IntoParams;
use wenmar_shopdata::Dataset;
use wenmar_shopdata::investigations::{self as shopdata, NOT_A_FINDING};
use wenmar_shopdata::recalls::RecallSource;

use crate::api::bad_query;
use crate::api::recalls::{ModelYear, campaign_url, model_year};
use crate::api::types::{Investigation, RecallInvestigations, VehicleInvestigations};
use crate::error::{ApiError, ErrorBody};
use crate::shop::ShopDb;
use crate::state::AppState;

/// Longest campaign number read from an address.
const LONGEST_CAMPAIGN: usize = 20;

fn listed(found: shopdata::Investigation) -> Investigation {
    Investigation {
        action: found.action,
        kind: found.kind,
        kind_name: found.kind_name,
        opened: found.opened,
        closed: found.closed,
        open: found.open,
        manufacturer: found.manufacturer,
        component: found.component,
        subject: found.subject,
        summary: found.summary,
        campaigns: found.campaigns,
        matched: None,
        models: Vec::new(),
    }
}

fn for_vehicle(found: shopdata::VehicleInvestigation) -> Investigation {
    let mut investigation = listed(found.investigation);
    investigation.matched = Some(found.matched);
    investigation.models = found.models;
    investigation
}

/// The day the investigations were current.
fn as_of(shop: &ShopDb) -> String {
    shop.datasets()
        .get(Dataset::Investigations)
        .map(|entry| entry.as_of.clone())
        .unwrap_or_default()
}

/// The credit line of the dataset, when its licence asks for one.
fn attribution(shop: &ShopDb) -> Vec<String> {
    shop.datasets()
        .get(Dataset::Investigations)
        .map(|entry| entry.attribution.clone())
        .filter(|credit| !credit.is_empty())
        .into_iter()
        .collect()
}

#[derive(Debug, Default, Deserialize, IntoParams)]
#[into_params(parameter_in = Query)]
pub struct VehicleInvestigationsQuery {
    /// `open` (the default): only investigations that are open and about a
    /// possible defect in the vehicles they list. `all`: every
    /// investigation that lists the model year, closed ones and audit
    /// queries included.
    #[param(example = "open")]
    pub status: Option<String>,
}

/// Reads `status`: whether only open investigations are wanted.
pub fn open_only(text: Option<&str>) -> Result<bool, ApiError> {
    match text.map(str::trim).filter(|text| !text.is_empty()) {
        None => Ok(true),
        Some(text) if text.eq_ignore_ascii_case("open") => Ok(true),
        Some(text) if text.eq_ignore_ascii_case("all") => Ok(false),
        Some(_) => Err(ApiError::Validation {
            message: "status must be open or all".to_owned(),
            details: json!({ "field": "status" }),
        }),
    }
}

/// The investigations of one model year, as the API and the pages give
/// them.
pub async fn investigations_of(
    shop: &ShopDb,
    found: &ModelYear,
    open_only: bool,
) -> Result<VehicleInvestigations, ApiError> {
    let (make_id, model_id, year) = (found.make_id, found.model_id, found.vehicle.year);
    let investigations = shop
        .run(move |reader| reader.investigations_for_vehicle(make_id, model_id, year, open_only))
        .await??;
    Ok(VehicleInvestigations {
        vehicle: found.vehicle.clone(),
        as_of: as_of(shop),
        investigations: investigations.into_iter().map(for_vehicle).collect(),
        note: NOT_A_FINDING.to_owned(),
        attribution: attribution(shop),
    })
}

pub async fn vehicle_investigations_op(
    state: &AppState,
    id: String,
    query: VehicleInvestigationsQuery,
) -> Result<VehicleInvestigations, ApiError> {
    let open_only = open_only(query.status.as_deref())?;
    let shop = state.dataset(Dataset::Investigations)?;
    let found = state
        .db()
        .run(move |worker| model_year(worker, &id))
        .await??;
    let Some(found) = found else {
        return Err(ApiError::NotFound("No vehicle has that id.".to_owned()));
    };
    investigations_of(shop, &found, open_only).await
}

pub async fn recall_investigations_op(
    state: &AppState,
    source: &str,
    campaign: &str,
) -> Result<RecallInvestigations, ApiError> {
    let missing = || ApiError::NotFound("No recall campaign has that number.".to_owned());
    let Some(source) = RecallSource::parse(source) else {
        return Err(missing());
    };
    let shop = state.dataset(Dataset::Investigations)?;
    let number = campaign.trim().to_ascii_uppercase();
    if number.is_empty()
        || number.len() > LONGEST_CAMPAIGN
        || !number.bytes().all(|byte| byte.is_ascii_alphanumeric())
    {
        return Err(missing());
    }
    // Only NHTSA's campaigns come out of NHTSA's investigations.
    let investigations = match source {
        RecallSource::Nhtsa => {
            let asked = number.clone();
            shop.run(move |reader| reader.investigations_for_campaign(&asked))
                .await??
        }
        RecallSource::Tc => Vec::new(),
    };
    Ok(RecallInvestigations {
        url: campaign_url(&state.config().base_url, source.id(), &number),
        source: source.id().to_owned(),
        campaign: number,
        as_of: as_of(shop),
        investigations: investigations.into_iter().map(listed).collect(),
        note: NOT_A_FINDING.to_owned(),
        attribution: attribution(shop),
    })
}

/// NHTSA investigations that list a model year.
///
/// By default only the open ones about a possible defect in the vehicles
/// listed. An investigation is not a finding that a defect exists, and it
/// lists a year, make and model, never one vehicle: `note` says so.
#[utoipa::path(
    get,
    path = "/vehicles/{id}/investigations",
    tag = "Investigations",
    params(
        ("id" = String, Path, description = "A vehicle id. Investigations are listed by year, make and model; a submodel or an engine in the id is set aside.", example = "2019_ram_1500"),
        VehicleInvestigationsQuery
    ),
    responses(
        (status = 200, description = "The investigations, newest first. An empty list when none lists the model year.", body = VehicleInvestigations),
        (status = 400, description = "`validation_failed`: `status` is not `open` or `all`.", body = ErrorBody),
        (status = 404, description = "`not_found`: no vehicle has that id. `dataset_unavailable`: this server holds no investigations.", body = ErrorBody)
    )
)]
pub async fn vehicle_investigations(
    State(state): State<AppState>,
    path: Result<Path<String>, PathRejection>,
    query: Result<Query<VehicleInvestigationsQuery>, QueryRejection>,
) -> Result<Json<VehicleInvestigations>, ApiError> {
    let Ok(Path(id)) = path else {
        return Err(ApiError::NotFound("No vehicle has that id.".to_owned()));
    };
    let Query(query) = query.map_err(bad_query)?;
    Ok(Json(vehicle_investigations_op(&state, id, query).await?))
}

/// The NHTSA investigations that led to a recall campaign.
///
/// Open and closed, oldest first. Most campaigns followed no investigation
/// and give an empty list, as every Transport Canada campaign does. The
/// campaign number is not checked against the recall data.
#[utoipa::path(
    get,
    path = "/recalls/{source}/{campaign}/investigations",
    tag = "Investigations",
    params(
        ("source" = String, Path, description = "Who published the campaign: `nhtsa` or `tc` (Transport Canada).", example = "nhtsa"),
        ("campaign" = String, Path, description = "The publisher's campaign number. Letters may be in either case.", example = "18V568000")
    ),
    responses(
        (status = 200, description = "The investigations, oldest first.", body = RecallInvestigations),
        (status = 404, description = "`not_found`: that is not a campaign number. `dataset_unavailable`: this server holds no investigations.", body = ErrorBody)
    )
)]
pub async fn recall_investigations(
    State(state): State<AppState>,
    path: Result<Path<(String, String)>, PathRejection>,
) -> Result<Json<RecallInvestigations>, ApiError> {
    let Ok(Path((source, campaign))) = path else {
        return Err(ApiError::NotFound(
            "No recall campaign has that number.".to_owned(),
        ));
    };
    Ok(Json(
        recall_investigations_op(&state, &source, &campaign).await?,
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn status_is_open_unless_all_is_asked_for() {
        assert_eq!(open_only(None), Ok(true));
        assert_eq!(open_only(Some("")), Ok(true));
        assert_eq!(open_only(Some("open")), Ok(true));
        assert_eq!(open_only(Some(" ALL ")), Ok(false));
        assert!(matches!(
            open_only(Some("closed")),
            Err(ApiError::Validation { .. })
        ));
    }
}
