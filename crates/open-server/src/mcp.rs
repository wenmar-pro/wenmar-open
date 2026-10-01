//! The remote MCP endpoint at `/mcp`, for AI agents.
//!
//! It speaks the Model Context Protocol's Streamable HTTP transport in its
//! simplest form: a client POSTs one JSON-RPC message and gets one JSON
//! answer. There are no sessions, no streams and no stored state, so there
//! is nothing to keep open and nothing to clean up.
//!
//! Two tools, one per noun, each taking an `action`. They call the same
//! functions as the `/v1` routes and return the same JSON.

use axum::Json;
use axum::extract::State;
use axum::extract::rejection::JsonRejection;
use axum::http::{HeaderValue, StatusCode, header};
use axum::response::{IntoResponse, Response};
use open_mcp::{
    INVALID_PARAMS, INVALID_REQUEST, METHOD_NOT_FOUND, PARSE_ERROR, rpc_error, rpc_result,
    tool_result,
};
use serde::Deserialize;
use serde::de::DeserializeOwned;
use serde_json::{Value, json};

use crate::api::{vehicles, vin};
use crate::error::ApiError;
use crate::state::AppState;

/// The tools, as `tools/list` returns them. They are defined once, for
/// this endpoint and for `wenmar-open mcp`.
pub use open_mcp::tools;

/// Protocol versions this server can speak, newest first.
pub const PROTOCOL_VERSIONS: [&str; 3] = ["2025-11-25", "2025-06-18", "2025-03-26"];

fn answer(status: StatusCode, body: Value) -> Response {
    let mut response = (status, Json(body)).into_response();
    response
        .headers_mut()
        .insert(header::CACHE_CONTROL, HeaderValue::from_static("no-store"));
    response
}

/// A tool call that failed in a way the model can read and correct.
fn tool_error(error: &ApiError) -> Value {
    let body = serde_json::to_value(error.body()).unwrap_or_else(|_| json!({}));
    open_mcp::tool_error(body)
}

fn arguments<T: DeserializeOwned>(arguments: &Value) -> Result<T, ApiError> {
    serde_json::from_value(arguments.clone())
        .map_err(|error| ApiError::validation(error.to_string()))
}

fn to_value<T: serde::Serialize>(value: T) -> Result<Value, ApiError> {
    serde_json::to_value(value).map_err(ApiError::internal)
}

#[derive(Debug, Deserialize)]
struct VinArguments {
    vin: Option<String>,
    vins: Option<Vec<String>>,
    #[serde(default, deserialize_with = "crate::api::blank_is_none")]
    year: Option<u16>,
}

async fn vin_tool(state: &AppState, action: &str, given: &Value) -> Result<Value, ApiError> {
    let given: VinArguments = arguments(given)?;
    match action {
        "decode" => {
            let Some(input) = given.vin else {
                return Err(ApiError::validation("vin is required for action=decode"));
            };
            let mut decoded = vin::decode_many(state, vec![input], given.year).await?;
            match decoded.pop() {
                Some(result) => to_value(result?),
                None => Err(ApiError::internal("a decode gave no answer")),
            }
        }
        "batch" => {
            let inputs = given.vins.unwrap_or_default();
            if inputs.len() > vin::MOST_VINS {
                return Err(ApiError::validation(format!(
                    "a batch holds at most {} VINs",
                    vin::MOST_VINS
                )));
            }
            let decoded = vin::decode_many(state, inputs, given.year).await?;
            let items: Vec<Value> = decoded
                .into_iter()
                .map(|result| match result {
                    Ok(decode) => to_value(decode),
                    Err(error) => to_value(error.body()),
                })
                .collect::<Result<_, _>>()?;
            Ok(Value::Array(items))
        }
        _ => Err(ApiError::validation("action must be decode or batch")),
    }
}

#[derive(Debug, Deserialize)]
struct Lookup {
    query: Option<String>,
    id: Option<String>,
}

async fn vehicles_tool(state: &AppState, action: &str, given: &Value) -> Result<Value, ApiError> {
    match action {
        "years" => to_value(vehicles::years_op(state, arguments(given)?).await?),
        "makes" => to_value(vehicles::makes_op(state, arguments(given)?).await?),
        "models" => to_value(vehicles::models_op(state, arguments(given)?).await?),
        "submodels" => to_value(vehicles::submodels_op(state, arguments(given)?).await?),
        "engines" => to_value(vehicles::engines_op(state, arguments(given)?).await?),
        "search" => {
            let lookup: Lookup = arguments(given)?;
            let mut query: vehicles::SearchQuery = arguments(given)?;
            query.q = lookup.query.or(query.q);
            to_value(vehicles::search_op(state, query).await?)
        }
        "entry" => {
            let lookup: Lookup = arguments(given)?;
            let Some(id) = lookup.id else {
                return Err(ApiError::validation("id is required for action=entry"));
            };
            to_value(vehicles::entry_op(state, id).await?)
        }
        _ => Err(ApiError::validation(
            "action must be years, makes, models, submodels, engines, search or entry",
        )),
    }
}

/// Runs one `tools/call`. `Err` is a protocol error; a tool that fails
/// answers `Ok` with `isError`.
async fn call(state: &AppState, params: &Value) -> Result<Value, (i64, String)> {
    let name = params
        .get("name")
        .and_then(Value::as_str)
        .unwrap_or_default();
    let empty = json!({});
    let given = match params.get("arguments") {
        None | Some(Value::Null) => &empty,
        Some(given) if given.is_object() => given,
        Some(_) => return Err((INVALID_PARAMS, "arguments must be an object".to_owned())),
    };
    let action = given
        .get("action")
        .and_then(Value::as_str)
        .unwrap_or_default();
    let outcome = match name {
        "wenmar_vin" => vin_tool(state, action, given).await,
        "wenmar_vehicles" => vehicles_tool(state, action, given).await,
        _ => return Err((INVALID_PARAMS, format!("Unknown tool: {name:.60}"))),
    };
    Ok(match outcome {
        Ok(value) => tool_result(value),
        Err(error) => tool_error(&error),
    })
}

fn initialize(params: &Value) -> Value {
    open_mcp::initialize(params, &PROTOCOL_VERSIONS, env!("CARGO_PKG_VERSION"))
}

/// `POST /mcp`.
pub async fn post(
    State(state): State<AppState>,
    body: Result<Json<Value>, JsonRejection>,
) -> Response {
    let message = match body {
        Ok(Json(message)) => message,
        Err(rejection) if rejection.status() == StatusCode::PAYLOAD_TOO_LARGE => {
            return ApiError::PayloadTooLarge.into_response();
        }
        Err(_) => {
            return answer(
                StatusCode::BAD_REQUEST,
                rpc_error(&Value::Null, PARSE_ERROR, "The body is not JSON."),
            );
        }
    };
    if message.is_array() {
        return answer(
            StatusCode::BAD_REQUEST,
            rpc_error(
                &Value::Null,
                INVALID_REQUEST,
                "Batches are not supported. Send one message.",
            ),
        );
    }
    let method = message.get("method").and_then(Value::as_str);
    let id = message.get("id").filter(|id| !id.is_null());
    let (Some(method), true) = (method, message.is_object()) else {
        return answer(
            StatusCode::BAD_REQUEST,
            rpc_error(&Value::Null, INVALID_REQUEST, "Not a JSON-RPC request."),
        );
    };
    // A message with no id is a notification, such as
    // `notifications/initialized`. It is accepted and gets no answer.
    let Some(id) = id else {
        return StatusCode::ACCEPTED.into_response();
    };
    let null = Value::Null;
    let params = message.get("params").unwrap_or(&null);
    let body = match method {
        "initialize" => rpc_result(id, initialize(params)),
        "ping" => rpc_result(id, json!({})),
        "tools/list" => rpc_result(id, json!({ "tools": tools() })),
        "tools/call" => match call(&state, params).await {
            Ok(result) => rpc_result(id, result),
            Err((code, message)) => rpc_error(id, code, &message),
        },
        _ => rpc_error(id, METHOD_NOT_FOUND, "Method not found."),
    };
    answer(StatusCode::OK, body)
}

/// `GET` and `DELETE /mcp`: this server has no streams and no sessions.
pub async fn not_allowed() -> Response {
    let mut response = ApiError::MethodNotAllowed.into_response();
    response
        .headers_mut()
        .insert(header::ALLOW, HeaderValue::from_static("POST"));
    response
}
