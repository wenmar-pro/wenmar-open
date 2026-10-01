//! The abuse ceiling, through the whole application.

use axum::body::Body;
use axum::http::{Request, StatusCode};

use crate::common::{self, body_json, header};

#[tokio::test]
async fn the_request_over_the_ceiling_is_429_with_retry_after() {
    let app = common::app_with(|config| config.requests_per_minute = 3).await;
    for _ in 0..3 {
        assert_eq!(app.get("/v1/meta").await.status(), StatusCode::OK);
    }
    let refused = app.get("/v1/meta").await;
    assert_eq!(refused.status(), StatusCode::TOO_MANY_REQUESTS);
    let seconds: u64 = header(&refused, "retry-after").parse().unwrap();
    assert!((1..=60).contains(&seconds), "{seconds}");
    assert_eq!(header(&refused, "cache-control"), "no-store");
    let body = body_json(refused).await;
    assert_eq!(body["error"]["code"], "rate_limited");
    assert_eq!(body["error"]["details"]["retry_after"], seconds);
    // The deploy proxy's health check is never refused.
    for _ in 0..10 {
        assert_eq!(app.get("/health").await.status(), StatusCode::OK);
    }
}

#[tokio::test]
async fn every_kind_of_request_counts_including_ones_that_fail() {
    let app = common::app_with(|config| config.requests_per_minute = 3).await;
    assert_eq!(app.get("/v1/nothing").await.status(), StatusCode::NOT_FOUND);
    assert_eq!(
        app.post_json("/v1/nothing", "{}").await.status(),
        StatusCode::NOT_FOUND
    );
    assert_eq!(app.get("/v1/meta").await.status(), StatusCode::OK);
    assert_eq!(
        app.get("/v1/nothing").await.status(),
        StatusCode::TOO_MANY_REQUESTS
    );
}

#[tokio::test]
async fn a_forged_forwarded_header_does_not_escape_the_ceiling() {
    // No proxy is trusted, so every request below is from the same peer
    // whatever the header claims.
    let app = common::app_with(|config| config.requests_per_minute = 2).await;
    let from = |address: &'static str| {
        Request::get("/v1/meta")
            .header("x-forwarded-for", address)
            .body(Body::empty())
            .unwrap()
    };
    assert_eq!(app.send(from("203.0.113.1")).await.status(), StatusCode::OK);
    assert_eq!(app.send(from("203.0.113.2")).await.status(), StatusCode::OK);
    assert_eq!(
        app.send(from("203.0.113.3")).await.status(),
        StatusCode::TOO_MANY_REQUESTS
    );

    // Behind one proxy, the entry the proxy added is the client. What the
    // client put in front of it changes nothing.
    let app = common::app_with(|config| {
        config.requests_per_minute = 2;
        config.trusted_proxies = 1;
    })
    .await;
    assert_eq!(
        app.send(from("10.1.1.1, 203.0.113.9")).await.status(),
        StatusCode::OK
    );
    assert_eq!(
        app.send(from("10.2.2.2, 203.0.113.9")).await.status(),
        StatusCode::OK
    );
    assert_eq!(
        app.send(from("10.3.3.3, 203.0.113.9")).await.status(),
        StatusCode::TOO_MANY_REQUESTS
    );
    // Another client behind the same proxy is counted on its own.
    assert_eq!(
        app.send(from("10.3.3.3, 203.0.113.10")).await.status(),
        StatusCode::OK
    );
}
