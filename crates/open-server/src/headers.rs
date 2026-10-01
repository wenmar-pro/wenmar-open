//! Headers every `/v1` response carries: the data version, and caching.
//!
//! An answer is the same until the data file or the server changes, so
//! successful `GET`s may be cached for a day and carry an `ETag` made of
//! the data version, the server's version and the build id. Everything
//! else is `no-store`.

use axum::body::Body;
use axum::extract::{Request, State};
use axum::http::{HeaderValue, Method, StatusCode, header};
use axum::middleware::Next;
use axum::response::Response;

use crate::db::Meta;
use crate::state::AppState;

pub const X_DATA_VERSION: &str = "x-data-version";
pub const CACHE_FOR_A_DAY: &str = "public, max-age=86400";

/// The `ETag` of every cacheable response of this data file and this build.
///
/// The version number alone does not name a build: a page is reworded or
/// the stylesheet fixed without it changing. [`crate::BUILD_ID`] does.
pub fn etag(meta: &Meta) -> String {
    format!(
        "W/\"{}-{}-{}\"",
        meta.data_version,
        env!("CARGO_PKG_VERSION"),
        crate::BUILD_ID
    )
}

/// Whether an `If-None-Match` header names `etag`. Weak and strong forms of
/// the same tag match, as does `*`.
pub fn names(header: &HeaderValue, etag: &str) -> bool {
    let wanted = etag.trim_start_matches("W/");
    header.to_str().is_ok_and(|value| {
        value
            .split(',')
            .map(|tag| tag.trim().trim_start_matches("W/"))
            .any(|tag| tag == "*" || tag == wanted)
    })
}

/// Adds the headers to every response under `/v1`, including errors and
/// addresses that do not exist. Other paths pass through untouched.
pub async fn data_headers(State(state): State<AppState>, request: Request, next: Next) -> Response {
    if !request.uri().path().starts_with("/v1/") {
        return next.run(request).await;
    }
    let cacheable = matches!(*request.method(), Method::GET | Method::HEAD);
    let unchanged = cacheable
        && request
            .headers()
            .get(header::IF_NONE_MATCH)
            .is_some_and(|value| names(value, state.etag_text()));

    let mut response = if unchanged {
        let mut response = Response::new(Body::empty());
        *response.status_mut() = StatusCode::NOT_MODIFIED;
        response
    } else {
        next.run(request).await
    };

    let status = response.status();
    let headers = response.headers_mut();
    headers.insert(X_DATA_VERSION, state.data_version().clone());
    // JSON is never to be guessed at as HTML, whatever text the data holds.
    headers.insert(
        header::X_CONTENT_TYPE_OPTIONS,
        HeaderValue::from_static("nosniff"),
    );
    if cacheable && (status == StatusCode::OK || status == StatusCode::NOT_MODIFIED) {
        headers.insert(header::ETAG, state.etag().clone());
        headers.insert(
            header::CACHE_CONTROL,
            HeaderValue::from_static(CACHE_FOR_A_DAY),
        );
    } else {
        headers.insert(header::CACHE_CONTROL, HeaderValue::from_static("no-store"));
    }
    response
}

#[cfg(test)]
mod tests {
    use super::*;

    fn value(text: &str) -> HeaderValue {
        HeaderValue::from_str(text).unwrap()
    }

    #[test]
    fn the_etag_names_the_data_and_the_build() {
        let meta = Meta {
            data_version: "2026.09".to_owned(),
            vpic_release: String::new(),
            built_at: String::new(),
        };
        assert_eq!(
            etag(&meta),
            format!(
                "W/\"2026.09-{}-{}\"",
                env!("CARGO_PKG_VERSION"),
                crate::BUILD_ID
            )
        );
        // The build is named by more than the version number, which stays
        // the same from one deploy to the next.
        assert_ne!(
            etag(&meta),
            format!("W/\"2026.09-{}\"", env!("CARGO_PKG_VERSION"))
        );
    }

    #[test]
    fn if_none_match_is_compared_without_regard_to_weakness() {
        let etag = "W/\"2026.09-0.1.0\"";
        assert!(names(&value("W/\"2026.09-0.1.0\""), etag));
        assert!(names(&value("\"2026.09-0.1.0\""), etag));
        assert!(names(&value("\"other\", W/\"2026.09-0.1.0\""), etag));
        assert!(names(&value("*"), etag));
        assert!(!names(&value("W/\"2026.08-0.1.0\""), etag));
        assert!(!names(&value(""), etag));
    }
}
