//! The abuse ceiling covers every route there is. No route has a limit of
//! its own and none is exempt but `/health`.
//!
//! The API's routes are read from the OpenAPI description, so a route that
//! is added later is walked here without this file changing.

use axum::body::Body;
use axum::http::{Method, Request, Response, StatusCode};
use serde_json::Value;

use crate::common::{self, TestApp, body_json, header};

/// A value for each parameter a path may name.
fn filled(path: &str) -> String {
    let mut address = String::new();
    for part in path.split('/').skip(1) {
        address.push('/');
        address.push_str(match part {
            "{vin}" => common::KONA,
            "{id}" => "2023_hyundai_kona",
            "{make}" => "hyundai",
            "{model}" => "kona",
            "{year}" => "2023",
            "{source}" => "nhtsa",
            "{campaign}" => "19V888000",
            // A parameter added later. The ceiling is reached before the
            // route is, so any value will do.
            other if other.starts_with('{') => "x",
            other => other,
        });
    }
    address
}

/// Every route of the API, as a method and an address.
fn api_routes() -> Vec<(Method, String)> {
    let description: Value = serde_json::from_str(&open_server::openapi_json()).unwrap();
    let mut routes = Vec::new();
    for (path, methods) in description["paths"].as_object().unwrap() {
        for method in methods.as_object().unwrap().keys() {
            let method = Method::from_bytes(method.to_uppercase().as_bytes()).unwrap();
            routes.push((method, filled(path)));
        }
    }
    routes.sort_by(|left, right| left.1.cmp(&right.1));
    routes
}

/// Addresses outside the description: the MCP endpoint, the discovery
/// files and the recall pages. A page that this build does not have is
/// refused all the same: the ceiling does not know the routes.
fn other_routes() -> Vec<(Method, String)> {
    let get = |path: &str| (Method::GET, path.to_owned());
    vec![
        (Method::POST, "/mcp".to_owned()),
        get("/v1/openapi.json"),
        get("/llms.txt"),
        get("/recalls"),
        get("/recalls/nhtsa/19V888000"),
        get("/recalls/nhtsa/19V888000.md"),
        get("/makes/hyundai/kona/2023"),
        get("/makes/hyundai/kona/2023/recalls"),
        get("/makes/hyundai/kona/2023/recalls.md"),
        get("/vin/KM8K2CAB4PU001140"),
    ]
}

async fn send(app: &TestApp, method: &Method, address: &str) -> Response<Body> {
    let request = Request::builder().method(method.clone()).uri(address);
    let request = if *method == Method::POST {
        request
            .header("content-type", "application/json")
            .body(Body::from("{}"))
    } else {
        request.body(Body::empty())
    };
    app.send(request.unwrap()).await
}

#[test]
fn the_routes_added_with_the_shop_data_are_among_those_walked() {
    let walked: Vec<String> = api_routes()
        .into_iter()
        .map(|(_, address)| address)
        .collect();
    for address in [
        "/v1/vehicles/2023_hyundai_kona/recalls",
        "/v1/vin/KM8K2CAB4PU001140/recalls",
        "/v1/recalls/nhtsa/19V888000",
        "/v1/recalls",
        "/v1/makes/hyundai",
        "/v1/vin/batch",
        "/v1/meta",
    ] {
        assert!(
            walked.iter().any(|found| found == address),
            "{address} in {walked:?}"
        );
    }
    assert!(
        !walked.iter().any(|address| address.contains('{')),
        "{walked:?}"
    );
}

#[tokio::test]
async fn every_route_is_refused_over_the_ceiling_as_meta_is() {
    let app = common::app_with(|config| config.requests_per_minute = 1).await;
    assert_eq!(app.get("/v1/meta").await.status(), StatusCode::OK);
    // What `/v1/meta` itself gets, to hold every other route to.
    let meta = app.get("/v1/meta").await;
    assert_eq!(meta.status(), StatusCode::TOO_MANY_REQUESTS);
    assert_eq!(header(&meta, "cache-control"), "no-store");
    let seconds: u64 = header(&meta, "retry-after").parse().unwrap();
    assert!((1..=60).contains(&seconds), "{seconds}");
    let expected = body_json(meta).await;
    assert_eq!(expected["error"]["code"], "rate_limited");

    for (method, address) in api_routes() {
        let refused = send(&app, &method, &address).await;
        assert_eq!(
            refused.status(),
            StatusCode::TOO_MANY_REQUESTS,
            "{method} {address}"
        );
        assert_eq!(header(&refused, "cache-control"), "no-store", "{address}");
        let seconds: u64 = header(&refused, "retry-after").parse().unwrap();
        assert!((1..=60).contains(&seconds), "{address}: {seconds}");
        let mut body = body_json(refused).await;
        // The seconds left may have ticked down by one since `/v1/meta`.
        body["error"]["details"]["retry_after"] =
            expected["error"]["details"]["retry_after"].clone();
        assert_eq!(body, expected, "{method} {address}");
    }
    for (method, address) in other_routes() {
        let refused = send(&app, &method, &address).await;
        assert_eq!(
            refused.status(),
            StatusCode::TOO_MANY_REQUESTS,
            "{method} {address}"
        );
        assert_eq!(header(&refused, "cache-control"), "no-store", "{address}");
        let seconds: u64 = header(&refused, "retry-after").parse().unwrap();
        assert!((1..=60).contains(&seconds), "{address}: {seconds}");
    }
    // The one exemption there has always been.
    assert_eq!(app.get("/health").await.status(), StatusCode::OK);
}

#[tokio::test]
async fn every_route_counts_against_the_ceiling() {
    let mut routes = api_routes();
    routes.extend(other_routes());
    let ceiling = u32::try_from(routes.len()).unwrap();
    let app = common::app_with(move |config| config.requests_per_minute = ceiling).await;
    // One request to each route uses the allowance up exactly: none is
    // refused, whatever else it answers, and none is free.
    for (method, address) in &routes {
        let answered = send(&app, method, address).await;
        assert_ne!(
            answered.status(),
            StatusCode::TOO_MANY_REQUESTS,
            "{method} {address}"
        );
    }
    assert_eq!(
        app.get("/v1/meta").await.status(),
        StatusCode::TOO_MANY_REQUESTS
    );
}
