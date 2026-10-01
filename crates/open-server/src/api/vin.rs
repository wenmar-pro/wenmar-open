//! `GET /v1/vin/{vin}` and `POST /v1/vin/batch`.

use std::time::{SystemTime, UNIX_EPOCH};

use axum::Json;
use axum::extract::rejection::{JsonRejection, PathRejection, QueryRejection};
use axum::extract::{Path, Query, State};
use serde::{Deserialize, Serialize};
use serde_json::json;
use utoipa::{IntoParams, ToSchema};
use wenmar_vin::{DecodeOptions, Decoder, Vin};

use crate::api::types::VinDecode;
use crate::api::{bad_body, bad_query, blank_is_none};
use crate::db::Worker;
use crate::error::{ApiError, ErrorBody};
use crate::state::AppState;
use crate::vin_rows::{self, VinRows};

/// Most VINs in one batch.
pub const MOST_VINS: usize = 50;

/// Longest text read as a VIN. A VIN has 17 characters; spaces and dashes
/// are allowed, so there is some room. Anything longer is refused unread.
pub const LONGEST_INPUT: usize = 64;

/// The first model year a 17-character VIN can have.
const FIRST_YEAR: u16 = 1980;

#[derive(Debug, Default, Deserialize, IntoParams)]
#[into_params(parameter_in = Query)]
pub struct DecodeQuery {
    /// Use this model year instead of working it out from the VIN.
    #[serde(default, deserialize_with = "blank_is_none")]
    #[param(value_type = Option<u16>, example = 2023)]
    pub year: Option<u16>,
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct BatchRequest {
    /// At most 50 VINs.
    #[schema(max_items = 50, example = json!(["KM8K2CAB4PU001140", "1HGCM82633A004352"]))]
    pub vins: Vec<String>,
}

/// One answer of a batch: a decode, or the error a single request for that
/// VIN would have given.
#[derive(Debug, Clone, PartialEq, Serialize, ToSchema)]
#[serde(untagged)]
pub enum BatchItem {
    Decode(Box<VinDecode>),
    Error(ErrorBody),
}

/// The calendar year, worked out the way `wenmar-vin` does. It is only an
/// upper bound on model years, so a day's error at New Year does not matter.
pub fn current_year() -> u16 {
    const SECONDS_PER_YEAR: u64 = 31_556_952;
    let seconds = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |elapsed| elapsed.as_secs());
    u16::try_from(1970 + seconds / SECONDS_PER_YEAR).unwrap_or(u16::MAX)
}

fn too_long() -> ApiError {
    ApiError::InvalidVin {
        message: "a VIN has 17 characters, this is far longer".to_owned(),
        details: json!({ "suggestions": [] }),
    }
}

/// Decodes one VIN on a blocking thread: fetch its rows, decode, then ask
/// the catalog which entry it is.
fn decode_one(
    worker: &Worker,
    input: &str,
    model_year: Option<u16>,
    current_year: u16,
) -> Result<VinDecode, ApiError> {
    if input.len() > LONGEST_INPUT {
        return Err(too_long());
    }
    let rows = match Vin::parse(input) {
        Ok(vin) => vin_rows::fetch(&worker.source, &vin, model_year, current_year)
            .map_err(ApiError::internal)?,
        // The decoder says what is wrong with it and suggests corrections.
        Err(_) => VinRows::default(),
    };
    let options = DecodeOptions {
        model_year,
        current_year: Some(current_year),
    };
    let decoded = Decoder::new(&rows).decode(input, options)?;
    let selection = worker.catalog.selection(&decoded)?;
    Ok(VinDecode::new(decoded, selection))
}

/// Decodes each input in order. The outer error is for the request as a
/// whole; each inner one is for a single VIN.
pub async fn decode_many(
    state: &AppState,
    inputs: Vec<String>,
    model_year: Option<u16>,
) -> Result<Vec<Result<VinDecode, ApiError>>, ApiError> {
    let current_year = current_year();
    let latest = current_year.saturating_add(2);
    if let Some(year) = model_year
        && !(FIRST_YEAR..=latest).contains(&year)
    {
        return Err(ApiError::Validation {
            message: format!("year must be between {FIRST_YEAR} and {latest}"),
            details: json!({ "field": "year", "min": FIRST_YEAR, "max": latest }),
        });
    }
    Ok(state
        .db()
        .run(move |worker| {
            inputs
                .iter()
                .map(|input| decode_one(worker, input, model_year, current_year))
                .collect()
        })
        .await?)
}

/// Decode one VIN.
///
/// Spaces and dashes in the VIN are ignored and letters may be in either
/// case. A wrong check digit is not an error: `valid` is `false` and a
/// warning says so.
#[utoipa::path(
    get,
    path = "/vin/{vin}",
    tag = "VIN",
    params(
        ("vin" = String, Path, description = "A 17-character VIN.", example = "KM8K2CAB4PU001140"),
        DecodeQuery
    ),
    responses(
        (status = 200, description = "The decode.", body = VinDecode),
        (status = 400, description = "`invalid_vin`: wrong length or illegal characters. `details.suggestions` lists likely corrections.", body = ErrorBody),
        (status = 404, description = "`not_found`: no manufacturer is registered for the first three characters.", body = ErrorBody),
        (status = 429, description = "`rate_limited`: over 600 requests a minute. See `Retry-After`.", body = ErrorBody)
    )
)]
pub async fn decode(
    State(state): State<AppState>,
    path: Result<Path<String>, PathRejection>,
    query: Result<Query<DecodeQuery>, QueryRejection>,
) -> Result<Json<VinDecode>, ApiError> {
    let Ok(Path(input)) = path else {
        return Err(ApiError::InvalidVin {
            message: "a VIN uses only digits and letters other than I, O and Q".to_owned(),
            details: json!({ "suggestions": [] }),
        });
    };
    let Query(query) = query.map_err(bad_query)?;
    let mut decoded = decode_many(&state, vec![input], query.year).await?;
    match decoded.pop() {
        Some(result) => Ok(Json(result?)),
        None => Err(ApiError::internal("a decode gave no answer")),
    }
}

/// Decode up to 50 VINs.
///
/// The answer is an array in the order asked. Each item is a decode, or the
/// error object a single request for that VIN would have returned.
#[utoipa::path(
    post,
    path = "/vin/batch",
    tag = "VIN",
    request_body = BatchRequest,
    responses(
        (status = 200, description = "One item per VIN, in order.", body = Vec<BatchItem>),
        (status = 400, description = "`validation_failed`: the body is not `{ \"vins\": [...] }` or lists more than 50.", body = ErrorBody),
        (status = 413, description = "`payload_too_large`.", body = ErrorBody),
        (status = 429, description = "`rate_limited`.", body = ErrorBody)
    )
)]
pub async fn batch(
    State(state): State<AppState>,
    body: Result<Json<BatchRequest>, JsonRejection>,
) -> Result<Json<Vec<BatchItem>>, ApiError> {
    let Json(request) = body.map_err(bad_body)?;
    if request.vins.len() > MOST_VINS {
        return Err(ApiError::Validation {
            message: format!("a batch holds at most {MOST_VINS} VINs"),
            details: json!({ "field": "vins", "max": MOST_VINS, "received": request.vins.len() }),
        });
    }
    let decoded = decode_many(&state, request.vins, None).await?;
    Ok(Json(
        decoded
            .into_iter()
            .map(|result| match result {
                Ok(decode) => BatchItem::Decode(Box::new(decode)),
                Err(error) => BatchItem::Error(error.body()),
            })
            .collect(),
    ))
}
