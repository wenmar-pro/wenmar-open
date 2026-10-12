//! NHTSA's crash test ratings for a model year.
//!
//! NHTSA rates a vehicle by body style and drive, so a model year often
//! has several ratings. The answer gives every one with NHTSA's own words
//! for the vehicle it tested, and picks none.
//!
//! The route is a thin wrapper around a function ending in `_op`, as in
//! `vehicles.rs`.

use axum::Json;
use axum::extract::rejection::PathRejection;
use axum::extract::{Path, State};
use wenmar_shopdata::Dataset;
use wenmar_shopdata::ratings::VehicleRating;

use crate::api::recalls::{ModelYear, model_year};
use crate::api::types::{SafetyFeatures, SafetyRating, SafetyStars, VehicleSafety};
use crate::error::{ApiError, ErrorBody};
use crate::shop::ShopDb;
use crate::state::AppState;

/// Who rates the vehicles, as an answer names it.
pub const SOURCE: &str = "NHTSA New Car Assessment Program";

/// NHTSA's own pages of ratings, where a rating can be read in full.
pub const SOURCE_URL: &str = "https://www.nhtsa.gov/ratings";

/// Pounds as whole kilograms. A pound is 0.45359237 kg exactly.
pub fn kilograms(pounds: u32) -> u32 {
    (f64::from(pounds) * 0.453_592_37).round() as u32
}

fn rating(found: VehicleRating) -> SafetyRating {
    let description = found.description();
    let (stars, features) = (found.stars, found.features);
    SafetyRating {
        description,
        model: found.model,
        body_style: found.body_style,
        vehicle_type: found.vehicle_type,
        drive: found.drive,
        release: found.release,
        matched: found.matched,
        stars: SafetyStars {
            overall: stars.overall,
            frontal: stars.frontal,
            frontal_driver: stars.frontal_driver,
            frontal_passenger: stars.frontal_passenger,
            side: stars.side,
            side_driver: stars.side_driver,
            side_passenger: stars.side_passenger,
            side_barrier: stars.side_barrier,
            side_front_seat: stars.side_front_seat,
            side_rear_seat: stars.side_rear_seat,
            side_pole: stars.side_pole,
            rollover: stars.rollover,
        },
        rollover_risk_percent: found
            .rollover_risk_permille
            .map(|share| f64::from(share) / 10.0),
        rollover_tip: found.rollover_tip,
        curb_weight_kg: found.curb_weight_lb.map(kilograms),
        curb_weight_lb: found.curb_weight_lb,
        features: SafetyFeatures {
            esc: features.esc,
            forward_collision_warning: features.forward_collision_warning,
            forward_collision_warning_evaluation: features.forward_collision_warning_evaluation,
            lane_departure_warning: features.lane_departure_warning,
            lane_departure_warning_evaluation: features.lane_departure_warning_evaluation,
            crash_imminent_braking: features.crash_imminent_braking,
            crash_imminent_braking_evaluation: features.crash_imminent_braking_evaluation,
            dynamic_brake_support: features.dynamic_brake_support,
            dynamic_brake_support_evaluation: features.dynamic_brake_support_evaluation,
            backup_camera: features.backup_camera,
            blind_spot_detection: features.blind_spot_detection,
            head_side_airbag: features.head_side_airbag,
            torso_side_airbag: features.torso_side_airbag,
        },
        notes: found.notes,
    }
}

/// The ratings of one model year, as the API and the pages give them.
/// `shop` is the shop file, known to hold the ratings.
pub async fn safety_of(shop: &ShopDb, found: &ModelYear) -> Result<VehicleSafety, ApiError> {
    let (make_id, model_id, year) = (found.make_id, found.model_id, found.vehicle.year);
    let ratings = shop
        .run(move |reader| reader.ratings_for_vehicle(make_id, model_id, year))
        .await??;
    let entry = shop.datasets().get(Dataset::Ratings);
    Ok(VehicleSafety {
        vehicle: found.vehicle.clone(),
        as_of: entry.map(|entry| entry.as_of.clone()).unwrap_or_default(),
        ratings: ratings.into_iter().map(rating).collect(),
        source: SOURCE.to_owned(),
        source_url: SOURCE_URL.to_owned(),
        attribution: entry
            .map(|entry| entry.attribution.clone())
            .filter(|credit| !credit.is_empty())
            .into_iter()
            .collect(),
    })
}

pub async fn vehicle_safety_op(state: &AppState, id: String) -> Result<VehicleSafety, ApiError> {
    let shop = state.dataset(Dataset::Ratings)?;
    let found = state
        .db()
        .run(move |worker| model_year(worker, &id))
        .await??;
    let Some(found) = found else {
        return Err(ApiError::NotFound("No vehicle has that id.".to_owned()));
    };
    safety_of(shop, &found).await
}

/// NHTSA's crash test ratings for a model year.
///
/// NHTSA rates by body style and drive, so `ratings` often has several
/// entries and their stars can differ: each says which vehicle it is for,
/// in NHTSA's words. An entry with no stars is a vehicle NHTSA lists and
/// has not tested. `ratings` is empty when NHTSA lists none for the model
/// year.
#[utoipa::path(
    get,
    path = "/vehicles/{id}/safety",
    tag = "Vehicles",
    params(("id" = String, Path, description = "A vehicle id. Ratings are listed by year, make and model; a submodel or an engine in the id is set aside.", example = "2018_ford_f-150")),
    responses(
        (status = 200, description = "Every rating NHTSA gave a vehicle of the model year.", body = VehicleSafety),
        (status = 404, description = "`not_found`: no vehicle has that id. `dataset_unavailable`: this server holds no crash test ratings.", body = ErrorBody)
    )
)]
pub async fn vehicle_safety(
    State(state): State<AppState>,
    path: Result<Path<String>, PathRejection>,
) -> Result<Json<VehicleSafety>, ApiError> {
    let Ok(Path(id)) = path else {
        return Err(ApiError::NotFound("No vehicle has that id.".to_owned()));
    };
    Ok(Json(vehicle_safety_op(&state, id).await?))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pounds_become_whole_kilograms() {
        assert_eq!(kilograms(5060), 2295);
        assert_eq!(kilograms(2735), 1241);
        assert_eq!(kilograms(1000), 454);
        assert_eq!(kilograms(0), 0);
    }
}
