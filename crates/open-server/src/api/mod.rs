//! The JSON API under `/v1`.

pub mod meta;
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
