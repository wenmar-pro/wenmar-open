//! What every response has in common: headers, caching, CORS and errors.

use axum::body::Body;
use axum::http::{Request, StatusCode};
use serde_json::json;

use crate::common::{self, body_json, body_text, header};

#[tokio::test]
async fn meta_says_what_is_being_served() {
    let app = common::app().await;
    let (status, body) = app.json("/v1/meta").await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(
        body,
        json!({
            "data_version": "2026.09",
            "vpic_release": "vPICList_lite_2026_09",
            "built_at": "2026-10-01 04:25:57",
            "server_version": env!("CARGO_PKG_VERSION")
        })
    );
}

#[tokio::test]
async fn health_answers_without_data_headers() {
    let app = common::app().await;
    let response = app.get("/health").await;
    assert_eq!(response.status(), StatusCode::OK);
    assert!(response.headers().get("etag").is_none());
    assert!(response.headers().get("x-data-version").is_none());
    assert_eq!(
        body_json(response).await,
        json!({ "status": "ok", "data_version": "2026.09" })
    );
}

#[tokio::test]
async fn a_data_response_carries_the_version_and_may_be_cached() {
    let app = common::app().await;
    for path in ["/v1/meta", "/v1/openapi.json"] {
        let response = app.get(path).await;
        assert_eq!(response.status(), StatusCode::OK, "{path}");
        assert_eq!(header(&response, "x-data-version"), "2026.09", "{path}");
        assert_eq!(
            header(&response, "cache-control"),
            "public, max-age=86400",
            "{path}"
        );
        assert_eq!(
            header(&response, "etag"),
            format!("W/\"2026.09-{}\"", env!("CARGO_PKG_VERSION")),
            "{path}"
        );
        assert_eq!(header(&response, "x-content-type-options"), "nosniff");
        // Nothing is ever stored on the caller's side.
        assert!(response.headers().get("set-cookie").is_none(), "{path}");
    }
}

#[tokio::test]
async fn an_unchanged_response_is_304_with_no_body() {
    let app = common::app().await;
    let first = app.get("/v1/meta").await;
    let etag = header(&first, "etag").to_owned();
    let again = app
        .send(
            Request::get("/v1/meta")
                .header("if-none-match", &etag)
                .body(Body::empty())
                .unwrap(),
        )
        .await;
    assert_eq!(again.status(), StatusCode::NOT_MODIFIED);
    assert_eq!(header(&again, "etag"), etag);
    assert_eq!(header(&again, "x-data-version"), "2026.09");
    assert_eq!(body_text(again).await, "");

    // The tag of another data version is not a match.
    let stale = app
        .send(
            Request::get("/v1/meta")
                .header("if-none-match", "W/\"2026.08-0.1.0\"")
                .body(Body::empty())
                .unwrap(),
        )
        .await;
    assert_eq!(stale.status(), StatusCode::OK);
}

#[tokio::test]
async fn errors_are_never_cached() {
    let app = common::app().await;
    let response = app.get("/v1/nothing").await;
    assert_eq!(response.status(), StatusCode::NOT_FOUND);
    assert_eq!(header(&response, "cache-control"), "no-store");
    assert_eq!(header(&response, "x-data-version"), "2026.09");
    assert!(response.headers().get("etag").is_none());
}

#[tokio::test]
async fn any_origin_may_call_the_api_without_credentials() {
    let app = common::app().await;
    let response = app
        .send(
            Request::get("/v1/meta")
                .header("origin", "https://shop.example")
                .body(Body::empty())
                .unwrap(),
        )
        .await;
    assert_eq!(header(&response, "access-control-allow-origin"), "*");
    assert!(
        response
            .headers()
            .get("access-control-allow-credentials")
            .is_none()
    );
    let exposed = header(&response, "access-control-expose-headers").to_lowercase();
    assert!(exposed.contains("x-data-version"), "{exposed}");

    // The preflight a browser sends before a JSON POST.
    let preflight = app
        .send(
            Request::options("/v1/vin/batch")
                .header("origin", "https://shop.example")
                .header("access-control-request-method", "POST")
                .header("access-control-request-headers", "content-type")
                .body(Body::empty())
                .unwrap(),
        )
        .await;
    assert_eq!(preflight.status(), StatusCode::OK);
    assert_eq!(header(&preflight, "access-control-allow-origin"), "*");
    assert!(header(&preflight, "access-control-allow-methods").contains("POST"));

    // An error can be read by a browser too.
    let error = app
        .send(
            Request::get("/v1/nothing")
                .header("origin", "https://shop.example")
                .body(Body::empty())
                .unwrap(),
        )
        .await;
    assert_eq!(header(&error, "access-control-allow-origin"), "*");
}

#[tokio::test]
async fn an_unknown_address_or_method_is_a_json_error() {
    let app = common::app().await;
    for path in [
        "/v1/nothing",
        "/v1",
        "/v1/a/b/c",
        "/v1/..%2F..%2Fetc%2Fpasswd",
    ] {
        let (status, body) = app.json(path).await;
        assert_eq!(status, StatusCode::NOT_FOUND, "{path}");
        assert_eq!(body["error"]["code"], "not_found", "{path}");
        assert_eq!(body["error"]["details"], json!({}), "{path}");
    }
    let response = app
        .send(Request::delete("/v1/meta").body(Body::empty()).unwrap())
        .await;
    assert_eq!(response.status(), StatusCode::METHOD_NOT_ALLOWED);
    assert_eq!(header(&response, "x-data-version"), "2026.09");
    assert_eq!(
        body_json(response).await["error"]["code"],
        "method_not_allowed"
    );
}

#[tokio::test]
async fn head_is_answered_like_get_without_a_body() {
    let app = common::app().await;
    let response = app
        .send(Request::head("/v1/meta").body(Body::empty()).unwrap())
        .await;
    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(header(&response, "x-data-version"), "2026.09");
    assert_eq!(body_text(response).await, "");
}

#[tokio::test]
async fn outside_v1_an_unknown_address_is_a_json_404_too() {
    let app = common::app().await;
    for path in ["/", "/nothing", "/makes", "/.env"] {
        let response = app.get(path).await;
        assert_eq!(response.status(), StatusCode::NOT_FOUND, "{path}");
        // Only data responses carry the data version.
        assert!(response.headers().get("x-data-version").is_none());
        assert_eq!(body_json(response).await["error"]["code"], "not_found");
    }
}
