//! `GET /v1/vin/{vin}` and `POST /v1/vin/batch`.

use axum::Json;
use axum::extract::rejection::{JsonRejection, PathRejection, QueryRejection};
use axum::extract::{Path, Query, State};
use axum::http::header;
use axum::response::{IntoResponse, Response};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use utoipa::{IntoParams, ToSchema};
use wenmar_open_db::DecodeFailure;
use wenmar_vin::DecodeOptions;

use crate::api::batch_out;
use crate::api::types::VinDecode;
use crate::api::{bad_body, bad_query, blank_is_none};
use crate::db::Worker;
use crate::error::{ApiError, ErrorBody};
use crate::state::AppState;

pub use wenmar_open_db::current_year;

/// Most VINs in one batch.
pub const MOST_VINS: usize = 250;

/// Most VINs in one batch asked over MCP. An agent reads every answer, so
/// a batch there stays small.
pub const MOST_MCP_VINS: usize = 50;

/// Longest text read as a VIN. A VIN has 17 characters; spaces and dashes
/// are allowed, so there is some room. Anything longer is refused unread.
pub const LONGEST_INPUT: usize = wenmar_open_db::LONGEST_INPUT;

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

#[derive(Debug, Default, Deserialize, IntoParams)]
#[into_params(parameter_in = Query)]
pub struct BatchQuery {
    /// `recalls` adds to each decode the number of recall campaigns that
    /// may apply to the model year it reaches. Left out: nothing is added.
    #[param(example = "recalls")]
    pub include: Option<String>,
    /// Only these fields of each decode, separated by commas: dotted paths
    /// such as `year`, `engine.hp` or `attributes.EngineHP.value`. `vin` is
    /// always given. An error item is given whole. Left out: every field.
    #[param(example = "year,make,model,engine.label,engine.hp")]
    pub fields: Option<String>,
    /// `csv` answers with one row per VIN, in order: `vin`, the fields asked
    /// for (or a set for a fleet list), then `error` and `error_message`.
    /// Left out or `json`: the JSON array.
    #[param(example = "csv")]
    pub format: Option<String>,
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct BatchRequest {
    /// At most 250 VINs.
    #[schema(max_items = 250, example = json!(["KM8K2CAB4PU001140", "1HGCM82633A004352"]))]
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

fn too_long() -> ApiError {
    ApiError::InvalidVin {
        message: "a VIN has 17 characters, this is far longer".to_owned(),
        details: json!({ "suggestions": [] }),
    }
}

/// Decodes one VIN on a blocking thread: fetch its rows, decode, then ask
/// the catalog which entry it is. The steps are `Worker::decode`'s; this
/// turns each way it can fail into the API's error.
fn decode_one(
    worker: &Worker,
    input: &str,
    model_year: Option<u16>,
    current_year: u16,
) -> Result<VinDecode, ApiError> {
    let options = DecodeOptions {
        model_year,
        current_year: Some(current_year),
    };
    match worker.decode(input, options) {
        Ok(decode) => Ok(VinDecode::new(decode.decoded, decode.catalog)),
        Err(DecodeFailure::TooLong) => Err(too_long()),
        Err(DecodeFailure::Decode(error)) => Err(ApiError::from(error)),
        Err(DecodeFailure::Catalog(error)) => Err(ApiError::from(error)),
        Err(other) => Err(ApiError::internal(other)),
    }
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
    // A long list is split over up to half the connections, so it finishes
    // sooner and the other half is always there for single decodes.
    let pieces = state
        .db()
        .connections()
        .div_ceil(2)
        .min(inputs.len().div_ceil(VINS_PER_PIECE))
        .max(1);
    if pieces == 1 {
        return Ok(state
            .db()
            .run(move |worker| {
                inputs
                    .iter()
                    .map(|input| decode_one(worker, input, model_year, current_year))
                    .collect()
            })
            .await?);
    }
    let size = inputs.len().div_ceil(pieces);
    let mut tasks = tokio::task::JoinSet::new();
    for (index, piece) in inputs.chunks(size).enumerate() {
        let state = state.clone();
        let piece = piece.to_vec();
        tasks.spawn(async move {
            let decoded = state
                .db()
                .run(move |worker| {
                    piece
                        .iter()
                        .map(|input| decode_one(worker, input, model_year, current_year))
                        .collect::<Vec<_>>()
                })
                .await;
            (index, decoded)
        });
    }
    let mut done: Vec<(usize, Vec<Result<VinDecode, ApiError>>)> = Vec::with_capacity(pieces);
    while let Some(finished) = tasks.join_next().await {
        let (index, decoded) = finished
            .map_err(|_| ApiError::internal("a batch decode stopped before it finished"))?;
        done.push((index, decoded?));
    }
    done.sort_by_key(|(index, _)| *index);
    Ok(done.into_iter().flat_map(|(_, decoded)| decoded).collect())
}

/// The fewest VINs worth a connection of their own in a batch.
const VINS_PER_PIECE: usize = 10;

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

/// Decode up to 250 VINs.
///
/// The answer is an array in the order asked. Each item is a decode, or the
/// error object a single request for that VIN would have returned.
///
/// `fields` narrows each decode to the fields named, and `format=csv`
/// answers with a spreadsheet of one row per VIN.
///
/// With `?include=recalls`, each decode that reaches a model year of the
/// catalog also carries `recalls`: how many campaigns may apply to that
/// model year. It is a count for the year, make and model, never a lookup
/// by VIN.
#[utoipa::path(
    post,
    path = "/vin/batch",
    tag = "VIN",
    params(BatchQuery),
    request_body = BatchRequest,
    responses(
        (status = 200, description = "One item per VIN, in order. With `fields`, each decode holds only those fields and `vin`. With `format=csv`, the same as CSV.", content(
            (Vec<BatchItem> = "application/json"),
            (String = "text/csv")
        )),
        (status = 400, description = "`validation_failed`: the body is not `{ \"vins\": [...] }` or lists more than 250, `include` is not `recalls`, `format` is not `json` or `csv`, or `fields` names something that is not a field.", body = ErrorBody),
        (status = 404, description = "`dataset_unavailable`: `include=recalls` was asked of a server that holds no recalls.", body = ErrorBody),
        (status = 413, description = "`payload_too_large`.", body = ErrorBody),
        (status = 429, description = "`rate_limited`.", body = ErrorBody)
    )
)]
pub async fn batch(
    State(state): State<AppState>,
    query: Result<Query<BatchQuery>, QueryRejection>,
    body: Result<Json<BatchRequest>, JsonRejection>,
) -> Result<Response, ApiError> {
    let Query(query) = query.map_err(bad_query)?;
    let with_recalls = crate::api::overview::include_recalls(query.include.as_deref())?;
    let fields = batch_out::parse_fields(query.fields.as_deref())?;
    let as_csv = match query.format.as_deref().map(str::trim) {
        None | Some("") | Some("json") => false,
        Some("csv") => true,
        Some(other) => {
            return Err(ApiError::Validation {
                message: format!("format must be json or csv, not {other:?}"),
                details: json!({ "field": "format", "allowed": ["json", "csv"] }),
            });
        }
    };
    let Json(request) = body.map_err(bad_body)?;
    if request.vins.len() > MOST_VINS {
        return Err(ApiError::Validation {
            message: format!("a batch holds at most {MOST_VINS} VINs"),
            details: json!({ "field": "vins", "max": MOST_VINS, "received": request.vins.len() }),
        });
    }
    let inputs = as_csv.then(|| request.vins.clone());
    let mut decoded = decode_many(&state, request.vins, None).await?;
    if with_recalls {
        crate::api::overview::add_recall_counts(&state, &mut decoded).await?;
    }
    let items: Vec<BatchItem> = decoded
        .into_iter()
        .map(|result| match result {
            Ok(decode) => BatchItem::Decode(Box::new(decode)),
            Err(error) => BatchItem::Error(error.body()),
        })
        .collect();
    if fields.is_none() && !as_csv {
        return Ok(Json(items).into_response());
    }
    let values: Vec<Value> = items
        .iter()
        .map(|item| serde_json::to_value(item).map_err(ApiError::internal))
        .collect::<Result<_, _>>()?;
    if let Some(inputs) = inputs {
        let text = batch_out::csv(&values, &inputs, fields.as_deref());
        return Ok((
            [
                (header::CONTENT_TYPE, "text/csv; charset=utf-8"),
                (
                    header::CONTENT_DISPOSITION,
                    "attachment; filename=\"vins.csv\"",
                ),
            ],
            text,
        )
            .into_response());
    }
    let fields = fields.unwrap_or_default();
    Ok(Json(
        values
            .into_iter()
            .map(|value| batch_out::project(value, &fields))
            .collect::<Vec<Value>>(),
    )
    .into_response())
}
