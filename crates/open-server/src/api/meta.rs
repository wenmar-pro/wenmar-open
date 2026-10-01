use axum::Json;
use axum::extract::State;
use serde::Serialize;
use serde_json::{Value, json};
use utoipa::ToSchema;

use crate::state::AppState;

/// What is being served.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, ToSchema)]
pub struct MetaResponse {
    /// The data release, such as `2026.09`. Also sent as `X-Data-Version`.
    #[schema(example = "2026.09")]
    pub data_version: String,
    /// The NHTSA vPIC release the data was built from.
    #[schema(example = "vPICList_lite_2026_09")]
    pub vpic_release: String,
    /// When the data file was built, in UTC.
    #[schema(example = "2026-10-01 04:25:57")]
    pub built_at: String,
    /// The version of the server.
    #[schema(example = "0.1.0")]
    pub server_version: String,
}

/// The data version, the vPIC release it came from, and when it was built.
#[utoipa::path(
    get,
    path = "/meta",
    tag = "Data",
    responses((status = 200, description = "About the data being served.", body = MetaResponse))
)]
pub async fn meta(State(state): State<AppState>) -> Json<MetaResponse> {
    let meta = state.db().meta();
    Json(MetaResponse {
        data_version: meta.data_version.clone(),
        vpic_release: meta.vpic_release.clone(),
        built_at: meta.built_at.clone(),
        server_version: env!("CARGO_PKG_VERSION").to_owned(),
    })
}

/// Liveness, for the deploy proxy. Reads nothing and is not in the OpenAPI
/// description.
pub async fn health(State(state): State<AppState>) -> Json<Value> {
    Json(json!({ "status": "ok", "data_version": state.db().meta().data_version }))
}
