//! The Wenmar Open website and JSON API.

pub mod api;
pub mod config;
pub mod db;
pub mod error;
pub mod headers;
pub mod limit;
pub mod log;
pub mod state;
pub mod vin_rows;

use std::sync::Arc;
use std::time::Duration;

use axum::Router;
use axum::extract::{DefaultBodyLimit, Request};
use axum::http::{HeaderName, HeaderValue, Method, header};
use axum::middleware::{self, Next};
use axum::response::{IntoResponse, Response};
use axum::routing::get;
use tower_http::catch_panic::CatchPanicLayer;
use tower_http::cors::{Any, CorsLayer};

use crate::error::ApiError;
use crate::state::AppState;

/// Largest request body read: a batch of 50 VINs is under 2 KB.
pub const BODY_LIMIT: usize = 16 * 1024;

/// Longest a request may take before it is answered with 503.
pub const REQUEST_TIMEOUT: Duration = Duration::from_secs(10);

async fn not_found() -> ApiError {
    ApiError::NotFound("There is nothing at this address.".to_owned())
}

async fn method_not_allowed() -> ApiError {
    ApiError::MethodNotAllowed
}

async fn timeout(request: Request, next: Next) -> Response {
    match tokio::time::timeout(REQUEST_TIMEOUT, next.run(request)).await {
        Ok(response) => response,
        Err(_) => ApiError::Unavailable.into_response(),
    }
}

/// Any origin may call the API from a browser. No credentials are involved:
/// the service has no cookies and no keys.
fn cors() -> CorsLayer {
    CorsLayer::new()
        .allow_origin(Any)
        .allow_methods([Method::GET, Method::HEAD, Method::POST, Method::OPTIONS])
        .allow_headers(Any)
        .expose_headers([
            HeaderName::from_static(headers::X_DATA_VERSION),
            header::ETAG,
            header::RETRY_AFTER,
        ])
        .max_age(Duration::from_secs(86_400))
}

/// The whole application.
pub fn app(state: AppState) -> Router {
    let (v1, description) = api::router();
    let description =
        Arc::new(serde_json::to_string_pretty(&description).unwrap_or_else(|_| "{}".to_owned()));
    let openapi = move || {
        let description = Arc::clone(&description);
        async move {
            (
                [(
                    header::CONTENT_TYPE,
                    HeaderValue::from_static("application/json"),
                )],
                description.as_str().to_owned(),
            )
        }
    };

    // Layers run from the bottom of this list to the top: a request is
    // logged, then guarded against panics, counted against the ceiling,
    // given a time limit and CORS headers, and only then routed.
    Router::new()
        .merge(v1)
        .route("/v1/openapi.json", get(openapi))
        .route("/health", get(api::meta::health))
        .fallback(not_found)
        .method_not_allowed_fallback(method_not_allowed)
        .layer(DefaultBodyLimit::max(BODY_LIMIT))
        .layer(middleware::from_fn_with_state(
            state.clone(),
            headers::data_headers,
        ))
        .layer(cors())
        .layer(middleware::from_fn(timeout))
        .layer(middleware::from_fn_with_state(state.clone(), limit::limit))
        .layer(CatchPanicLayer::custom(
            |_: Box<dyn std::any::Any + Send>| ApiError::Internal.into_response(),
        ))
        .layer(middleware::from_fn(log::log))
        .with_state(state)
}

/// The OpenAPI description, as `/v1/openapi.json` serves it.
pub fn openapi_json() -> String {
    let (_, description) = api::router();
    serde_json::to_string_pretty(&description).unwrap_or_else(|_| "{}".to_owned())
}

#[cfg(test)]
mod tests {
    use axum::body::Body;
    use axum::http::{Request, StatusCode};
    use tower::ServiceExt;

    use super::*;

    #[tokio::test(start_paused = true)]
    async fn a_request_that_takes_too_long_is_answered_with_503() {
        let router = Router::new()
            .route(
                "/slow",
                get(|| async {
                    tokio::time::sleep(REQUEST_TIMEOUT * 2).await;
                    "late"
                }),
            )
            .route("/quick", get(|| async { "in time" }))
            .layer(middleware::from_fn(timeout));
        let slow = router
            .clone()
            .oneshot(Request::get("/slow").body(Body::empty()).unwrap())
            .await
            .unwrap();
        assert_eq!(slow.status(), StatusCode::SERVICE_UNAVAILABLE);
        assert_eq!(slow.headers()["retry-after"], "1");
        let quick = router
            .oneshot(Request::get("/quick").body(Body::empty()).unwrap())
            .await
            .unwrap();
        assert_eq!(quick.status(), StatusCode::OK);
    }
}
