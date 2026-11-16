//! `/feedback`. GitHub is a stub on a local port: no test reaches GitHub.

use std::net::SocketAddr;
use std::sync::{Arc, Mutex};

use axum::Router;
use axum::body::Body;
use axum::http::{Request, Response, StatusCode};
use http_body_util::BodyExt;
use open_server::config::Secret;
use open_server::serve::{Limits, serve};
use serde_json::{Value, json};
use tokio::net::TcpListener;
use tokio::sync::oneshot;

use crate::common::{self, TestApp, assert_basics, assert_no_injection, body_text, header, page};

/// A stand-in for GitHub's API. `seen` holds, for each request, its path,
/// its `authorization` header and its JSON body.
struct Stub {
    address: SocketAddr,
    seen: Arc<Mutex<Vec<(String, String, Value)>>>,
    _stop: oneshot::Sender<()>,
}

async fn stub(status: StatusCode) -> Stub {
    let seen = Arc::new(Mutex::new(Vec::new()));
    let record = Arc::clone(&seen);
    let router = Router::new().fallback(move |request: Request<Body>| {
        let record = Arc::clone(&record);
        async move {
            let path = request.uri().path().to_owned();
            let authorization = request
                .headers()
                .get("authorization")
                .and_then(|value| value.to_str().ok())
                .unwrap_or_default()
                .to_owned();
            let bytes = request.into_body().collect().await.unwrap().to_bytes();
            let body = serde_json::from_slice(&bytes).unwrap_or(Value::Null);
            record.lock().unwrap().push((path, authorization, body));
            (
                status,
                axum::Json(json!({ "number": 7, "html_url": "https://github.com/wenmar-pro/wenmar-open/issues/7" })),
            )
        }
    });
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    let (stop, stopped) = oneshot::channel::<()>();
    let until = async move {
        let _ = stopped.await;
    };
    tokio::spawn(serve(listener, router, until, Limits::default()));
    Stub {
        address,
        seen,
        _stop: stop,
    }
}

async fn app_over(stub: &Stub) -> TestApp {
    let api = format!("http://{}", stub.address);
    common::app_with(move |config| {
        config.github_token = Some(Secret::new("t0ken"));
        config.github_api = api;
    })
    .await
}

async fn post(app: &TestApp, body: &str) -> Response<Body> {
    let request = Request::post("/feedback")
        .header("content-type", "application/x-www-form-urlencoded")
        .body(Body::from(body.to_owned()))
        .unwrap();
    app.send(request).await
}

const ASKED: &str = "/feedback?page=https%3A%2F%2Fopen.example%2Fmakes%2Fhonda%2Fcivic%2F2019&vehicle=2019%20Honda%20Civic";
const REPORT: &str = "page=https%3A%2F%2Fopen.example%2Fmakes%2Fhonda%2Fcivic%2F2019&vehicle=2019+Honda+Civic&wrong=The+drive+is+AWD&correct=&website=";

#[tokio::test]
async fn without_a_token_the_page_offers_the_links_and_no_form() {
    let app = common::app().await;
    let response = app.get(ASKED).await;
    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(header(&response, "x-robots-tag"), "noindex");
    let html = body_text(response).await;
    assert_basics(&html, "/feedback");
    assert!(html.contains("<h1>Report an error</h1>"));
    assert!(!html.contains("<form"), "{html}");
    assert!(html.contains("&#38;page=https%3A%2F%2Fopen.example%2Fmakes%2Fhonda%2Fcivic%2F2019&#38;vehicle=2019%20Honda%20Civic\">Report it on GitHub</a>"), "{html}");
    // The pages still link straight to GitHub.
    let (_, html) = page(&app, "/makes/honda/civic/2019").await;
    assert!(!html.contains("/feedback"));
    // A report posted anyway files nothing and shows the same page.
    let response = post(&app, REPORT).await;
    assert_eq!(response.status(), StatusCode::OK);
    assert!(!body_text(response).await.contains("<form"));
}

#[tokio::test]
async fn with_a_token_the_form_is_shown_filled_in_and_pages_lead_to_it() {
    let github = stub(StatusCode::CREATED).await;
    let app = app_over(&github).await;
    let (status, html) = page(&app, ASKED).await;
    assert_eq!(status, StatusCode::OK);
    assert_basics(&html, "/feedback");
    assert!(html.contains(r#"<form class="feedback" action="/feedback" method="post">"#));
    assert!(html.contains(r#"name="page" type="text" maxlength="300" autocomplete="off" value="https://open.example/makes/honda/civic/2019">"#), "{html}");
    assert!(html.contains(r#"value="2019 Honda Civic">"#));
    assert!(
        html.contains(r#"<label for="website">"#)
            && html.contains(r#"<div class="trap" aria-hidden="true">"#)
    );
    assert!(!html.contains("t0ken"));
    for path in [
        "/makes/honda/civic/2019",
        "/makes/honda/civic/2019.md",
        "/vin/KM8K2CAB4PU001140",
    ] {
        let (_, body) = page(&app, path).await;
        assert!(
            body.contains("/feedback?page=https%3A%2F%2Fopen.example%2F"),
            "{path}"
        );
        assert!(!body.contains("issues/new"), "{path}");
    }
    assert!(github.seen.lock().unwrap().is_empty());
}

#[tokio::test]
async fn a_report_is_filed_as_an_issue_and_a_trapped_one_is_not() {
    let github = stub(StatusCode::CREATED).await;
    let app = app_over(&github).await;
    let response = post(&app, REPORT).await;
    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(header(&response, "cache-control"), "no-store");
    let html = body_text(response).await;
    assert!(html.contains("<h1>Thank you</h1>"));
    assert!(html.contains(r#"<a href="https://github.com/wenmar-pro/wenmar-open/issues/7">See the report on GitHub</a>"#), "{html}");
    {
        let seen = github.seen.lock().unwrap();
        assert_eq!(seen.len(), 1);
        let (path, authorization, body) = &seen[0];
        assert_eq!(path, "/repos/wenmar-pro/wenmar-open/issues");
        assert_eq!(authorization, "Bearer t0ken");
        assert_eq!(body["title"], "Data correction: 2019 Honda Civic");
        assert_eq!(body["labels"], json!(["data"]));
        assert!(
            body["body"]
                .as_str()
                .unwrap()
                .contains("### What is wrong\n\nThe drive is AWD\n")
        );
    }
    // The trap: thanked, and nothing more is sent.
    let response = post(&app, &REPORT.replace("website=", "website=x")).await;
    assert_eq!(response.status(), StatusCode::OK);
    let html = body_text(response).await;
    assert!(html.contains("<h1>Thank you</h1>") && !html.contains("issues/7"));
    assert_eq!(github.seen.lock().unwrap().len(), 1);
}

#[tokio::test]
async fn a_report_that_cannot_be_sent_keeps_what_was_typed() {
    let github = stub(StatusCode::UNAUTHORIZED).await;
    let app = app_over(&github).await;
    // Nothing to say, and too much to say: refused before GitHub is asked.
    let long = format!("wrong={}", "a".repeat(2001));
    for body in ["wrong=+&vehicle=x", long.as_str()] {
        let response = post(&app, body).await;
        assert_eq!(response.status(), StatusCode::BAD_REQUEST, "{body:.20}");
        assert!(body_text(response).await.contains(r#"<p class="warning">"#));
    }
    // A body that is not a form at all.
    let request = Request::post("/feedback")
        .header("content-type", "application/json")
        .body(Body::from("{}"))
        .unwrap();
    assert_eq!(app.send(request).await.status(), StatusCode::BAD_REQUEST);
    assert!(github.seen.lock().unwrap().is_empty());
    // GitHub refuses the token: the visitor's text is still in the form.
    let response = post(
        &app,
        "wrong=%3Cscript%3Ealert(1)%3C%2Fscript%3E&vehicle=2019+Honda+Civic",
    )
    .await;
    assert_eq!(response.status(), StatusCode::BAD_GATEWAY);
    let html = body_text(response).await;
    assert_no_injection(&html);
    assert!(html.contains("The report could not be sent."));
    assert!(
        html.contains("&#60;script&#62;alert(1)&#60;/script&#62;</textarea>"),
        "{html}"
    );
    assert!(html.contains(r#"value="2019 Honda Civic">"#));
    assert!(!html.contains("t0ken"));
    assert_eq!(github.seen.lock().unwrap().len(), 1);
}

#[tokio::test]
async fn the_fourth_report_in_a_minute_is_refused() {
    let github = stub(StatusCode::CREATED).await;
    let app = app_over(&github).await;
    for _ in 0..3 {
        assert_eq!(post(&app, REPORT).await.status(), StatusCode::OK);
    }
    let response = post(&app, REPORT).await;
    assert_eq!(response.status(), StatusCode::TOO_MANY_REQUESTS);
    assert!(
        body_text(response)
            .await
            .contains("<h1>Too many requests</h1>")
    );
    assert_eq!(github.seen.lock().unwrap().len(), 3);
}
