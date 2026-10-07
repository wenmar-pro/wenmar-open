//! The Wenmar Open website and JSON API.

pub mod api;
pub mod config;
pub mod db;
pub mod error;
pub mod headers;
pub mod limit;
pub mod llms;
pub mod log;
pub mod mcp;
pub mod search_index;
pub mod serve;
pub mod site;
pub mod state;
pub mod vin_rows;

#[cfg(test)]
#[path = "../build_id.rs"]
mod build_id;
#[cfg(test)]
#[path = "../rates_build.rs"]
mod rates_build;

use std::sync::Arc;
use std::time::Duration;

use axum::Router;
use axum::extract::{DefaultBodyLimit, Request, State};
use axum::http::{HeaderName, HeaderValue, Method, header};
use axum::middleware::{self, Next};
use axum::response::{IntoResponse, Response};
use axum::routing::get;
use tower_http::catch_panic::CatchPanicLayer;
use tower_http::cors::{Any, CorsLayer};

use crate::error::ApiError;
use crate::state::AppState;

/// A name for this build: 16 hexadecimal digits worked out from the
/// templates, the assets and the code when the crate is compiled. It is in
/// every `ETag` and in the address of the stylesheet and the script, so
/// that a deploy which changes a page is seen by a browser or a CDN that
/// holds the old one, whether or not the crate's version number changed.
pub const BUILD_ID: &str = env!("OPEN_BUILD_ID");

/// Largest request body read: a batch of 50 VINs is under 2 KB.
pub const BODY_LIMIT: usize = 16 * 1024;

/// Longest address read: the path and the query string together. The longest
/// address the API has a use for is well under 1 KB.
pub const MOST_URI: usize = 8 * 1024;

/// Most requests being answered at once. One more is answered 503 at once.
/// With [`serve::HEAD_LIMIT`] this bounds the memory requests can hold while
/// they wait: 512 heads of 32 KB are 16 MB.
pub const MOST_IN_FLIGHT: usize = 512;

/// Longest a request may take before it is answered with 503.
pub const REQUEST_TIMEOUT: Duration = Duration::from_secs(10);

async fn method_not_allowed() -> ApiError {
    ApiError::MethodNotAllowed
}

async fn timeout(request: Request, next: Next) -> Response {
    match tokio::time::timeout(REQUEST_TIMEOUT, next.run(request)).await {
        Ok(response) => response,
        Err(_) => ApiError::Unavailable.into_response(),
    }
}

/// Refuses an address longer than [`MOST_URI`] before anything else looks at
/// it.
async fn short_enough(request: Request, next: Next) -> Response {
    let length = request
        .uri()
        .path_and_query()
        .map_or(0, |address| address.as_str().len());
    if length > MOST_URI {
        return ApiError::UriTooLong { max: MOST_URI }.into_response();
    }
    next.run(request).await
}

/// Answers 503 at once when [`MOST_IN_FLIGHT`] requests are already being
/// answered, so requests that wait cannot pile up without limit. `/health`
/// always gets through: the deploy proxy calls it every few seconds.
async fn in_flight(State(state): State<AppState>, request: Request, next: Next) -> Response {
    if request.uri().path() == "/health" {
        return next.run(request).await;
    }
    let Ok(_place) = state.places().try_acquire() else {
        return ApiError::Unavailable.into_response();
    };
    next.run(request).await
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
    // logged, then guarded against panics, refused if its address is too
    // long, counted against the ceiling, given a place among the requests
    // in flight, a time limit, CORS headers and a page's or the API's own
    // headers, and only then handled.
    // On the way back, a refusal made on a page's address becomes a page.
    Router::new()
        .merge(v1)
        .route("/v1/openapi.json", get(openapi))
        .route("/health", get(api::meta::health))
        .route("/llms.txt", get(llms::llms_txt))
        .route("/llms-full.txt", get(llms::llms_full_txt))
        .route("/.well-known/api-catalog", get(site::seo::api_catalog))
        .route(
            "/mcp",
            axum::routing::post(mcp::post)
                .get(mcp::not_allowed)
                .delete(mcp::not_allowed),
        )
        // The same address with a trailing slash, as it is often pasted.
        .route(
            "/mcp/",
            axum::routing::post(mcp::post)
                .get(mcp::not_allowed)
                .delete(mcp::not_allowed),
        )
        .merge(site::router())
        .fallback(site::fallback)
        .method_not_allowed_fallback(method_not_allowed)
        .layer(DefaultBodyLimit::max(BODY_LIMIT))
        .layer(middleware::from_fn_with_state(
            state.clone(),
            headers::data_headers,
        ))
        .layer(middleware::from_fn_with_state(
            state.clone(),
            site::page_headers,
        ))
        .layer(cors())
        .layer(middleware::from_fn(timeout))
        .layer(middleware::from_fn_with_state(state.clone(), in_flight))
        .layer(middleware::from_fn_with_state(state.clone(), limit::limit))
        .layer(middleware::from_fn(short_enough))
        .layer(CatchPanicLayer::custom(
            |_: Box<dyn std::any::Any + Send>| ApiError::Internal.into_response(),
        ))
        // Outside everything that can refuse a request, so that a person
        // who is refused sees a page.
        .layer(middleware::from_fn_with_state(
            state.clone(),
            site::page_refusals,
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
