//! The JSON API under `/v1`.

pub mod bulletins;
pub mod codes;
pub mod complaints;
pub mod fuel;
pub mod hand;
pub mod investigations;
pub mod labor;
pub mod meta;
pub mod overview;
pub mod parts_card;
pub mod payroll;
pub mod recalls;
pub mod safety;
pub mod types;
pub mod vehicles;
pub mod vin;

use std::fmt::Display;
use std::str::FromStr;

use axum::extract::rejection::{JsonRejection, QueryRejection};
use axum::http::StatusCode;
use serde::{Deserialize, Deserializer, de};
use utoipa::OpenApi;
use utoipa_axum::router::OpenApiRouter;
use utoipa_axum::routes;

use crate::error::ApiError;
use crate::state::AppState;

#[derive(OpenApi)]
#[openapi(
    info(
        title = "Wenmar Open",
        description = "Free vehicle data for auto repair shops: VIN decoding and a year, make, model, submodel and engine catalog. No API key and no account. Responses are plain JSON with no wrapper. Errors are `{ \"error\": { \"code\", \"message\", \"details\" } }`. Fields and endpoints are only ever added. One address may make 600 requests a minute.",
        license(name = "MIT", identifier = "MIT")
    ),
    servers((url = "https://open.wenmarpro.com")),
    tags(
        (name = "VIN", description = "Decode vehicle identification numbers."),
        (name = "Vehicles", description = "Years, makes, models, submodels and engines, each step limited to what is valid for the steps before it."),
        (name = "Recalls", description = "Recall campaigns from NHTSA and Transport Canada that may apply to a model year. Never a lookup by VIN: confirm with the manufacturer."),
        (name = "Investigations", description = "NHTSA defect investigations that list a model year or led to a recall. An investigation is not a finding of a defect."),
        (name = "Bulletins", description = "Manufacturer communications NHTSA publishes: service bulletins, service campaigns, warranty extensions and over-the-air updates, by model year. NHTSA's index and summary only; the document is linked at NHTSA."),
        (name = "Complaints", description = "What owners reported to NHTSA about a model year, as counts by component. Reports, not verified defects; counts, not rates."),
        (name = "Codes", description = "Generic OBD-II trouble codes, described in this project's own words, and what is held about a code for one model year. Manufacturer-specific codes are not described."),
        (name = "Labour", description = "Wenmar's own estimates of the hours a repair job takes, by job and vehicle, under CC BY 4.0. An estimate, not a quote, and never taken from a labour guide. Each figure says what it is for: an engine, a run of model years, a model, or a class of vehicle."),
        (name = "Payroll", description = "Ontario payroll deductions for one pay by CRA's T4127 formulas (Option 1): income tax, CPP, CPP2 and EI, the net pay and the employer's share, with every factor used. An estimate: the employer remains liable to CRA for what it deducts and remits; check with CRA's Payroll Deductions Online Calculator (PDOC)."),
        (name = "Data", description = "About the data being served.")
    )
)]
struct ApiDoc;

/// The `/v1` routes and their OpenAPI description, built from the same list
/// so one cannot name a route the other lacks.
pub fn router() -> (axum::Router<AppState>, utoipa::openapi::OpenApi) {
    let v1 = OpenApiRouter::new()
        .routes(routes!(vin::decode))
        .routes(routes!(vin::batch))
        .routes(routes!(vehicles::years))
        .routes(routes!(vehicles::makes))
        .routes(routes!(vehicles::models))
        .routes(routes!(vehicles::submodels))
        .routes(routes!(vehicles::trims))
        .routes(routes!(vehicles::engines))
        .routes(routes!(vehicles::search))
        .routes(routes!(vehicles::entry))
        .routes(routes!(safety::vehicle_safety))
        .routes(routes!(complaints::vehicle_complaints))
        .routes(routes!(recalls::vehicle_recalls))
        .routes(routes!(recalls::vin_recalls))
        .routes(routes!(recalls::recall_campaign))
        .routes(routes!(recalls::recall_search))
        .routes(routes!(bulletins::vehicle_bulletins))
        .routes(routes!(bulletins::vin_bulletins))
        .routes(routes!(bulletins::bulletin))
        .routes(routes!(bulletins::bulletin_search))
        .routes(routes!(codes::code))
        .routes(routes!(codes::code_search))
        .routes(routes!(codes::vehicle_code))
        .routes(routes!(labor::jobs))
        .routes(routes!(labor::job))
        .routes(routes!(labor::vehicle_labor))
        .routes(routes!(labor::vehicle_labor_job))
        .routes(routes!(labor::vin_labor))
        .routes(routes!(labor::vin_labor_job))
        .routes(routes!(investigations::vehicle_investigations))
        .routes(routes!(investigations::recall_investigations))
        .routes(routes!(overview::recent_recalls))
        .routes(routes!(overview::vehicle_overview))
        .routes(routes!(overview::vin_overview))
        .routes(routes!(overview::model_years))
        .routes(routes!(fuel::vehicle_fuel))
        .routes(routes!(fuel::vin_fuel))
        .routes(routes!(fuel::vehicle_specs))
        .routes(routes!(fuel::vin_specs))
        .routes(routes!(fuel::vehicle_fluids))
        .routes(routes!(fuel::vin_fluids))
        .routes(routes!(hand::vehicle_tires))
        .routes(routes!(hand::vin_tires))
        .routes(routes!(parts_card::parts_card))
        .routes(routes!(vehicles::make))
        .routes(routes!(payroll::payroll_deductions))
        .routes(routes!(meta::meta));
    OpenApiRouter::with_openapi(ApiDoc::openapi())
        .nest("/v1", v1)
        .split_for_parts()
}

/// A query string that could not be read.
pub fn bad_query(rejection: QueryRejection) -> ApiError {
    ApiError::validation(rejection.body_text())
}

/// A JSON body that could not be read.
pub fn bad_body(rejection: JsonRejection) -> ApiError {
    match rejection.status() {
        StatusCode::PAYLOAD_TOO_LARGE => ApiError::PayloadTooLarge,
        StatusCode::UNSUPPORTED_MEDIA_TYPE => {
            ApiError::validation("Send the body as JSON, with Content-Type: application/json.")
        }
        _ => ApiError::validation(rejection.body_text()),
    }
}

/// Reads an optional value that may arrive as text (a query string) or as a
/// number (JSON). Nothing and an empty string both mean it was not given, so
/// `?year=` from an empty form field is not an error.
pub fn blank_is_none<'de, D, T>(deserializer: D) -> Result<Option<T>, D::Error>
where
    D: Deserializer<'de>,
    T: FromStr,
    T::Err: Display,
{
    #[derive(Deserialize)]
    #[serde(untagged)]
    enum Given {
        Number(i64),
        Text(String),
    }
    let text = match Option::<Given>::deserialize(deserializer)? {
        None => return Ok(None),
        Some(Given::Number(number)) => number.to_string(),
        Some(Given::Text(text)) => text,
    };
    let text = text.trim();
    if text.is_empty() {
        return Ok(None);
    }
    text.parse().map(Some).map_err(de::Error::custom)
}
