//! The remote MCP endpoint at `/mcp`, for AI agents.
//!
//! It speaks the Model Context Protocol's Streamable HTTP transport in its
//! simplest form: a client POSTs one JSON-RPC message and gets one JSON
//! answer. There are no sessions, no streams and no stored state, so there
//! is nothing to keep open and nothing to clean up.
//!
//! It speaks both kinds of protocol revision on the one address. A client
//! that opens with `initialize` gets the handshake of the revisions up to
//! 2025-11-25. A client that names its version in each request's `_meta`,
//! as 2026-07-28 does, is answered without one, and must say the same in
//! its headers.
//!
//! Three tools, one per noun, each taking an `action`. They call the same
//! functions as the `/v1` routes and return the same JSON.

use axum::Json;
use axum::extract::State;
use axum::extract::rejection::JsonRejection;
use axum::http::{HeaderMap, HeaderValue, Method, StatusCode, header};
use axum::response::{IntoResponse, Response};
use open_mcp::{
    Era, HEADER_MISMATCH, INVALID_PARAMS, INVALID_REQUEST, METHOD_NOT_FOUND, MODERN_VERSIONS,
    PARSE_ERROR, UNSUPPORTED_PROTOCOL_VERSION, VERSION_KEY, rpc_error, rpc_failure, rpc_result,
    tool_result,
};
use serde::Deserialize;
use serde::de::DeserializeOwned;
use serde_json::{Value, json};

use crate::api::{bulletins, codes, fuel, hand, labor, overview, recalls, vehicles, vin};
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
    /// For action=fuel.
    country: Option<String>,
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
        "overview" => {
            let Some(input) = given.vin else {
                return Err(ApiError::validation("vin is required for action=overview"));
            };
            to_value(overview::vin_overview_op(state, input, given.year).await?)
        }
        "fuel" | "specs" | "fluids" | "tires" => {
            let Some(input) = given.vin else {
                return Err(ApiError::validation(format!(
                    "vin is required for action={action}"
                )));
            };
            match action {
                "fuel" => {
                    let query = fuel::VinFuelQuery {
                        country: given.country,
                        year: given.year,
                    };
                    to_value(fuel::vin_fuel_op(state, input, query).await?)
                }
                "specs" => to_value(fuel::vin_specs_op(state, input, given.year).await?),
                "fluids" => to_value(fuel::vin_fluids_op(state, input, given.year).await?),
                _ => to_value(hand::vin_tires_op(state, input, given.year).await?),
            }
        }
        _ => Err(ApiError::validation(
            "action must be decode, batch, overview, fuel, specs, fluids or tires",
        )),
    }
}

#[derive(Debug, Deserialize)]
struct Lookup {
    query: Option<String>,
    id: Option<String>,
    make: Option<String>,
    model: Option<String>,
    /// For action=fuel.
    country: Option<String>,
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
        "safety" => {
            let lookup: Lookup = arguments(given)?;
            let Some(id) = lookup.id else {
                return Err(ApiError::validation("id is required for action=safety"));
            };
            to_value(crate::api::safety::vehicle_safety_op(state, id).await?)
        }
        "complaints" => {
            let lookup: Lookup = arguments(given)?;
            let Some(id) = lookup.id else {
                return Err(ApiError::validation("id is required for action=complaints"));
            };
            to_value(crate::api::complaints::vehicle_complaints_op(state, id).await?)
        }
        "overview" => {
            let lookup: Lookup = arguments(given)?;
            let Some(id) = lookup.id else {
                return Err(ApiError::validation("id is required for action=overview"));
            };
            to_value(overview::vehicle_overview_op(state, id).await?)
        }
        "model_years" => {
            let lookup: Lookup = arguments(given)?;
            let (Some(make), Some(model)) = (lookup.make, lookup.model) else {
                return Err(ApiError::validation(
                    "make and model are required for action=model_years",
                ));
            };
            to_value(overview::model_years_op(state, make, model).await?)
        }
        "fuel" | "specs" | "fluids" | "tires" => {
            let lookup: Lookup = arguments(given)?;
            let Some(id) = lookup.id else {
                return Err(ApiError::validation(format!(
                    "id is required for action={action}"
                )));
            };
            match action {
                "fuel" => {
                    let query = fuel::VehicleFuelQuery {
                        country: lookup.country,
                    };
                    to_value(fuel::vehicle_fuel_op(state, id, query).await?)
                }
                "specs" => to_value(fuel::vehicle_specs_op(state, id).await?),
                "fluids" => to_value(fuel::vehicle_fluids_op(state, id).await?),
                _ => to_value(hand::vehicle_tires_op(state, id).await?),
            }
        }
        _ => Err(ApiError::validation(
            "action must be years, makes, models, submodels, engines, search, entry, overview, model_years, safety, complaints, fuel, specs, fluids or tires",
        )),
    }
}

#[derive(Debug, Deserialize)]
struct RecallArguments {
    id: Option<String>,
    vin: Option<String>,
    source: Option<String>,
    campaign: Option<String>,
    query: Option<String>,
    country: Option<String>,
    #[serde(default, deserialize_with = "crate::api::blank_is_none")]
    year: Option<u16>,
}

/// The recalls that may apply. Each action is the function behind a `/v1`
/// recall route, so a server with no recall data answers with the same
/// `dataset_unavailable` error, as a tool error the model can read.
async fn recalls_tool(state: &AppState, action: &str, given: &Value) -> Result<Value, ApiError> {
    let asked: RecallArguments = arguments(given)?;
    match action {
        "vehicle" => {
            let Some(id) = asked.id else {
                return Err(ApiError::validation("id is required for action=vehicle"));
            };
            let query = recalls::VehicleRecallsQuery {
                country: asked.country,
            };
            to_value(recalls::vehicle_recalls_op(state, id, query).await?)
        }
        "vin" => {
            let Some(vin) = asked.vin else {
                return Err(ApiError::validation("vin is required for action=vin"));
            };
            let query = recalls::VinRecallsQuery {
                country: asked.country,
                year: asked.year,
            };
            to_value(recalls::vin_recalls_op(state, vin, query).await?)
        }
        "campaign" => {
            let (Some(source), Some(campaign)) = (asked.source, asked.campaign) else {
                return Err(ApiError::validation(
                    "source and campaign are required for action=campaign",
                ));
            };
            to_value(recalls::recall_campaign_op(state, &source, &campaign).await?)
        }
        "search" => {
            let mut query: recalls::RecallSearchQuery = arguments(given)?;
            query.q = asked.query.or(query.q);
            to_value(recalls::recall_search_op(state, query).await?)
        }
        "recent" => {
            let query: overview::RecentQuery = arguments(given)?;
            to_value(overview::recent_recalls_op(state, query).await?)
        }
        _ => Err(ApiError::validation(
            "action must be vehicle, vin, campaign, search or recent",
        )),
    }
}

/// What a `wenmar_bulletins` call may carry besides the filter.
#[derive(Debug, Default, Deserialize)]
struct BulletinArguments {
    id: Option<String>,
    vin: Option<String>,
    nhtsa_id: Option<i64>,
    query: Option<String>,
}

/// Manufacturer bulletins. Each action is the function behind a `/v1`
/// bulletin route, so a server with no bulletin data answers with the same
/// `dataset_unavailable` error, as a tool error the model can read.
async fn bulletins_tool(state: &AppState, action: &str, given: &Value) -> Result<Value, ApiError> {
    let asked: BulletinArguments = arguments(given)?;
    match action {
        "vehicle" => {
            let Some(id) = asked.id else {
                return Err(ApiError::validation("id is required for action=vehicle"));
            };
            let mut filter: bulletins::BulletinFilter = arguments(given)?;
            filter.q = asked.query.or(filter.q);
            to_value(bulletins::vehicle_bulletins_op(state, id, filter).await?)
        }
        "vin" => {
            let Some(vin) = asked.vin else {
                return Err(ApiError::validation("vin is required for action=vin"));
            };
            let mut query: bulletins::VinBulletinsQuery = arguments(given)?;
            query.q = asked.query.or(query.q);
            to_value(bulletins::vin_bulletins_op(state, vin, query).await?)
        }
        "get" => {
            let Some(nhtsa_id) = asked.nhtsa_id else {
                return Err(ApiError::validation("nhtsa_id is required for action=get"));
            };
            to_value(bulletins::bulletin_op(state, &nhtsa_id.to_string()).await?)
        }
        "search" => {
            let mut query: bulletins::BulletinSearchQuery = arguments(given)?;
            query.q = asked.query.or(query.q);
            to_value(bulletins::bulletin_search_op(state, query).await?)
        }
        _ => Err(ApiError::validation(
            "action must be vehicle, vin, get or search",
        )),
    }
}

/// What a `wenmar_codes` call may carry.
#[derive(Debug, Default, Deserialize)]
struct CodeArguments {
    id: Option<String>,
    code: Option<String>,
    query: Option<String>,
    system: Option<String>,
    limit: Option<usize>,
    cursor: Option<String>,
}

/// Trouble codes. Each action is the function behind a `/v1` code route,
/// so a server without the data answers with the same
/// `dataset_unavailable` error, as a tool error the model can read.
async fn codes_tool(state: &AppState, action: &str, given: &Value) -> Result<Value, ApiError> {
    let asked: CodeArguments = arguments(given)?;
    match action {
        "get" => {
            let Some(code) = asked.code else {
                return Err(ApiError::validation("code is required for action=get"));
            };
            to_value(codes::code_op(state, code).await?)
        }
        "search" => {
            let query = codes::CodeSearchQuery {
                q: asked.query,
                system: asked.system,
                limit: asked.limit,
                cursor: asked.cursor,
            };
            to_value(codes::code_search_op(state, query).await?)
        }
        "vehicle" => {
            let Some(id) = asked.id else {
                return Err(ApiError::validation("id is required for action=vehicle"));
            };
            let Some(code) = asked.code else {
                return Err(ApiError::validation("code is required for action=vehicle"));
            };
            to_value(codes::vehicle_code_op(state, id, code).await?)
        }
        _ => Err(ApiError::validation(
            "action must be get, search or vehicle",
        )),
    }
}

/// What a `wenmar_labor` call may carry.
#[derive(Debug, Default, Deserialize)]
struct LaborArguments {
    job: Option<String>,
    id: Option<String>,
    vin: Option<String>,
    year: Option<u16>,
}

/// The job list and the labour estimates. Each action is the function
/// behind a `/v1` route.
async fn labor_tool(state: &AppState, action: &str, given: &Value) -> Result<Value, ApiError> {
    let asked: LaborArguments = arguments(given)?;
    match action {
        "jobs" => match asked.job {
            Some(job) => to_value(labor::job_op(state, job).await?),
            None => to_value(labor::jobs_op(state).await?),
        },
        "vehicle" => {
            let Some(id) = asked.id else {
                return Err(ApiError::validation("id is required for action=vehicle"));
            };
            to_value(labor::vehicle_labor_op(state, id, asked.job).await?)
        }
        "vin" => {
            let Some(vin) = asked.vin else {
                return Err(ApiError::validation("vin is required for action=vin"));
            };
            to_value(labor::vin_labor_op(state, vin, asked.year, asked.job).await?)
        }
        _ => Err(ApiError::validation("action must be jobs, vehicle or vin")),
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
        "wenmar_recalls" => recalls_tool(state, action, given).await,
        "wenmar_bulletins" => bulletins_tool(state, action, given).await,
        "wenmar_codes" => codes_tool(state, action, given).await,
        "wenmar_labor" => labor_tool(state, action, given).await,
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

fn header_text<'h>(headers: &'h HeaderMap, name: &str) -> Option<&'h str> {
    headers
        .get(name)
        .and_then(|value| value.to_str().ok())
        .map(str::trim)
}

/// Refuses a request whose headers do not say what its body says.
fn mismatch(id: &Value, what: &str) -> Response {
    answer(
        StatusCode::BAD_REQUEST,
        rpc_failure(id, json!({ "code": HEADER_MISMATCH, "message": what })),
    )
}

/// Refuses a protocol version this server does not speak, naming the ones
/// it does.
fn unsupported(id: &Value, requested: &str) -> Response {
    answer(
        StatusCode::BAD_REQUEST,
        rpc_failure(
            id,
            json!({
                "code": UNSUPPORTED_PROTOCOL_VERSION,
                "message": "Unsupported protocol version",
                "data": {
                    "supported": open_mcp::supported(&PROTOCOL_VERSIONS),
                    "requested": requested
                }
            }),
        ),
    )
}

/// `GET <streamable-http-url>/server-card`: the Server Card of SEP-2127.
///
/// The same document is served at `/.well-known/mcp/server-card.json` and
/// `/.well-known/mcp`. The SEP reserves the first and recommends against the
/// other two, but a card may sit at any unreserved URI, and serving them
/// costs nothing: a client that only looks under `.well-known` finds the
/// server either way.
///
/// It is static public metadata, so it carries the `ETag` and the
/// `If-None-Match` answer the SEP asks a host for, and any origin may read
/// it. The CORS layer already allows every origin.
pub async fn server_card(
    State(state): State<AppState>,
    method: Method,
    headers: HeaderMap,
) -> Response {
    let card = open_mcp::server_card(
        &state.config().base_url,
        &PROTOCOL_VERSIONS,
        env!("CARGO_PKG_VERSION"),
    );
    document(
        &state,
        method,
        headers,
        open_mcp::CARD_TYPE,
        card,
        Some(&format!("{}/mcp/server-card", state.config().base_url)),
    )
}

/// `GET /.well-known/ai-catalog.json`: how a client holding only a domain
/// learns that this domain serves an MCP server, and where its card is.
pub async fn ai_catalog(State(state): State<AppState>) -> Response {
    let catalog = open_mcp::ai_catalog(&state.config().base_url);
    let bytes = serde_json::to_vec_pretty(&catalog).unwrap_or_else(|_| b"{}".to_vec());
    (
        [
            (
                header::CONTENT_TYPE,
                HeaderValue::from_static(open_mcp::CATALOG_TYPE),
            ),
            (
                header::CACHE_CONTROL,
                HeaderValue::from_static("public, max-age=3600"),
            ),
        ],
        bytes,
    )
        .into_response()
}

/// A public JSON document: the media type it is served under, an hour of
/// caching, and a `304` for a visitor that already holds this build.
///
/// The validator names the document, not the data file behind it. A card is
/// static metadata, so a new month of data must not cost every client that
/// polls a fresh download.
fn document(
    _state: &AppState,
    method: Method,
    headers: HeaderMap,
    media_type: &'static str,
    body: Value,
    link_to: Option<&String>,
) -> Response {
    let bytes = serde_json::to_vec_pretty(&body).unwrap_or_else(|_| b"{}".to_vec());
    let etag = document_etag(&bytes);
    let cacheable = matches!(method, Method::GET | Method::HEAD);
    let unchanged = cacheable
        && headers
            .get(header::IF_NONE_MATCH)
            .is_some_and(|tag| crate::headers::names(tag, etag.to_str().unwrap_or_default()));
    let mut response = if unchanged {
        let mut response = Response::new(axum::body::Body::empty());
        *response.status_mut() = StatusCode::NOT_MODIFIED;
        response
    } else {
        let mut response = bytes.into_response();
        response
            .headers_mut()
            .insert(header::CONTENT_TYPE, HeaderValue::from_static(media_type));
        response
    };
    let headers = response.headers_mut();
    if cacheable {
        headers.insert(
            header::CACHE_CONTROL,
            HeaderValue::from_static("public, max-age=3600"),
        );
        headers.insert(header::ETAG, etag);
        headers.insert(
            header::X_CONTENT_TYPE_OPTIONS,
            HeaderValue::from_static("nosniff"),
        );
        if let Some(address) = link_to
            && let Ok(value) = HeaderValue::from_str(&format!(
                "<{address}>; rel=\"http://modelcontextprotocol.io/server-card\""
            ))
        {
            headers.insert(header::LINK, value);
        }
    } else {
        headers.insert(header::CACHE_CONTROL, HeaderValue::from_static("no-store"));
    }
    response
}

/// A weak validator for a document this server built, from its own bytes.
///
/// A card is the same whatever the data file holds, so the validator must not
/// name the data version: that would make every monthly data build cost each
/// client that polls one useless download.
fn document_etag(bytes: &[u8]) -> HeaderValue {
    // FNV-1a over the bytes. Two builds that produce the same card produce
    // the same value; two cards that differ produce different ones. A
    // collision costs a client one extra fetch and nothing else.
    let mut hash: u64 = 0xcbf2_9ce4_8422_2325;
    for byte in bytes {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
    }
    let text = format!("W/\"{}-{hash:x}\"", crate::BUILD_ID);
    HeaderValue::from_str(&text).unwrap_or_else(|_| HeaderValue::from_static("W/\"card\""))
}

/// `POST /mcp`.
pub async fn post(
    State(state): State<AppState>,
    headers: HeaderMap,
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
    let version_header = header_text(&headers, "mcp-protocol-version");

    // `initialize` is the handshake, whatever else the request carries.
    let era = if method == "initialize" {
        Era::Handshake
    } else {
        match open_mcp::era(params, &PROTOCOL_VERSIONS) {
            Ok(era) => era,
            Err(error) => return answer(StatusCode::BAD_REQUEST, rpc_failure(id, error)),
        }
    };
    match era {
        // Every request of this kind names its version, its method and,
        // for a tool call, the tool, in headers as well as in the body.
        Era::NoHandshake => {
            let asked = params["_meta"][VERSION_KEY].as_str();
            if version_header.is_none() || version_header != asked {
                return mismatch(
                    id,
                    "The MCP-Protocol-Version header must name the version in _meta.",
                );
            }
            if header_text(&headers, "mcp-method") != Some(method) {
                return mismatch(id, "The Mcp-Method header must name the method.");
            }
            if method == "tools/call" {
                let tool = params
                    .get("name")
                    .and_then(Value::as_str)
                    .unwrap_or_default();
                let named = header_text(&headers, "mcp-name").and_then(open_mcp::header_name);
                if named.as_deref() != Some(tool) {
                    return mismatch(id, "The Mcp-Name header must name the tool.");
                }
            }
        }
        // A client that has shaken hands sends the version it was given.
        // No header at all is an older client, and is let through.
        Era::Handshake if method != "initialize" => {
            if let Some(version) = version_header {
                if MODERN_VERSIONS.contains(&version) {
                    return answer(
                        StatusCode::BAD_REQUEST,
                        rpc_error(
                            id,
                            INVALID_PARAMS,
                            &format!("_meta must carry {VERSION_KEY}."),
                        ),
                    );
                }
                if !PROTOCOL_VERSIONS.contains(&version) {
                    return unsupported(id, version);
                }
            }
        }
        Era::Handshake => {}
    }

    let modern = era == Era::NoHandshake;
    let outcome: Result<Value, (StatusCode, i64, String)> = match method {
        "initialize" => Ok(initialize(params)),
        "server/discover" if modern => Ok(open_mcp::discover(&PROTOCOL_VERSIONS)),
        // A client of both kinds takes this as its cue to send `initialize`.
        "server/discover" => Err((
            StatusCode::BAD_REQUEST,
            INVALID_PARAMS,
            format!("_meta must carry {VERSION_KEY}."),
        )),
        "ping" => Ok(json!({})),
        "tools/list" => Ok(open_mcp::tools_list(era)),
        "tools/call" => call(&state, params)
            .await
            .map_err(|(code, message)| (StatusCode::OK, code, message)),
        // The revision with no handshake asks for 404 here.
        _ if modern => Err((
            StatusCode::NOT_FOUND,
            METHOD_NOT_FOUND,
            "Method not found.".to_owned(),
        )),
        _ => Err((
            StatusCode::OK,
            METHOD_NOT_FOUND,
            "Method not found.".to_owned(),
        )),
    };
    match outcome {
        Ok(result) if modern => answer(
            StatusCode::OK,
            rpc_result(id, open_mcp::complete(result, env!("CARGO_PKG_VERSION"))),
        ),
        Ok(result) => answer(StatusCode::OK, rpc_result(id, result)),
        Err((status, code, message)) => answer(status, rpc_error(id, code, &message)),
    }
}

/// `GET` and `DELETE /mcp`: this server has no streams and no sessions.
pub async fn not_allowed() -> Response {
    let mut response = ApiError::MethodNotAllowed.into_response();
    response
        .headers_mut()
        .insert(header::ALLOW, HeaderValue::from_static("POST"));
    response
}
