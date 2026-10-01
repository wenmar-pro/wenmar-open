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
use serde::Deserialize;
use serde::de::DeserializeOwned;
use serde_json::{Value, json};

use crate::api::{vehicles, vin};
use crate::error::ApiError;
use crate::state::AppState;

/// Protocol versions this server can speak, newest first.
pub const PROTOCOL_VERSIONS: [&str; 3] = ["2025-11-25", "2025-06-18", "2025-03-26"];

const PARSE_ERROR: i64 = -32700;
const INVALID_REQUEST: i64 = -32600;
const METHOD_NOT_FOUND: i64 = -32601;
const INVALID_PARAMS: i64 = -32602;

const INSTRUCTIONS: &str = "Wenmar Open is free vehicle data for auto repair shops, from NHTSA's vPIC. Use wenmar_vin to decode a VIN. Use wenmar_vehicles to step through year, make, model, submodel and engine, to search by free text, or to look up a vehicle id. Every call is read-only and needs no key.";

/// The tools, as `tools/list` returns them.
pub fn tools() -> Value {
    json!([
        {
            "name": "wenmar_vin",
            "title": "VIN decoder",
            "description": "Decode vehicle identification numbers into year, make, model, trim, engine, drivetrain, safety equipment and the matching catalog entry. action=decode takes `vin` (and optionally `year` to override the model year). action=batch takes `vins`, at most 50, and returns one result per VIN in order. A wrong check digit is not an error: the result has valid=false and a warning.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "action": { "type": "string", "enum": ["decode", "batch"] },
                    "vin": { "type": "string", "description": "A 17-character VIN. Spaces and dashes are ignored." },
                    "vins": { "type": "array", "items": { "type": "string" }, "maxItems": 50 },
                    "year": { "type": "integer", "description": "Model year to use instead of the one worked out from the VIN." }
                },
                "required": ["action"]
            },
            "annotations": { "readOnlyHint": true, "openWorldHint": false }
        },
        {
            "name": "wenmar_vehicles",
            "title": "Vehicle catalog",
            "description": "Look up vehicles by year, make, model, submodel and engine. Actions: years; makes (optional `year`); models (`make`, optional `year`); submodels (`make`, `model`, `year`); engines (`make`, `model`, `year`, optional `submodel`); search (`query`, such as \"2019 civic si\" or \"chevy 1500\"); entry (`id`, such as \"2019_honda_civic_si\"). `make` and `model` take a name, an alias or an id. `term` narrows a list to names starting with it. `scope` is light (cars, MPVs and trucks; the default), all, or a vPIC vehicle type id.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "action": { "type": "string", "enum": ["years", "makes", "models", "submodels", "engines", "search", "entry"] },
                    "year": { "type": "integer" },
                    "make": { "type": "string" },
                    "model": { "type": "string" },
                    "submodel": { "type": "string" },
                    "term": { "type": "string" },
                    "query": { "type": "string" },
                    "id": { "type": "string" },
                    "scope": { "type": "string" },
                    "limit": { "type": "integer" }
                },
                "required": ["action"]
            },
            "annotations": { "readOnlyHint": true, "openWorldHint": false }
        }
    ])
}

fn rpc_result(id: &Value, result: Value) -> Value {
    json!({ "jsonrpc": "2.0", "id": id, "result": result })
}

fn rpc_error(id: &Value, code: i64, message: &str) -> Value {
    json!({ "jsonrpc": "2.0", "id": id, "error": { "code": code, "message": message } })
}

fn answer(status: StatusCode, body: Value) -> Response {
    let mut response = (status, Json(body)).into_response();
    response
        .headers_mut()
        .insert(header::CACHE_CONTROL, HeaderValue::from_static("no-store"));
    response
}

/// A tool's result: the JSON as text, and as structured content when it is
/// an object. Lists are wrapped, because structured content must be an
/// object.
fn tool_result(value: Value) -> Value {
    let text = value.to_string();
    let structured = match value {
        Value::Object(_) => value,
        other => json!({ "items": other }),
    };
    json!({
        "content": [{ "type": "text", "text": text }],
        "structuredContent": structured,
        "isError": false
    })
}

/// A tool call that failed in a way the model can read and correct.
fn tool_error(error: &ApiError) -> Value {
    let body = serde_json::to_value(error.body()).unwrap_or_else(|_| json!({}));
    json!({
        "content": [{ "type": "text", "text": body.to_string() }],
        "structuredContent": body,
        "isError": true
    })
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
    let asked = params.get("protocolVersion").and_then(Value::as_str);
    let version = asked
        .and_then(|asked| PROTOCOL_VERSIONS.iter().find(|known| **known == asked))
        .unwrap_or(&PROTOCOL_VERSIONS[0]);
    json!({
        "protocolVersion": version,
        "capabilities": { "tools": { "listChanged": false } },
        "serverInfo": {
            "name": "wenmar-open",
            "title": "Wenmar Open",
            "version": env!("CARGO_PKG_VERSION")
        },
        "instructions": INSTRUCTIONS
    })
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
