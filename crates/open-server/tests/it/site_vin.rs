//! The result page, `/vin/{vin}`.

use axum::body::Body;
use axum::http::{Request, StatusCode};

use crate::common::{self, assert_basics, assert_no_injection, body_text, header, page, redirect};

#[tokio::test]
async fn a_result_page_shows_the_decode_and_is_not_indexed() {
    let app = common::app().await;
    let response = app.get("/vin/KM8K2CAB4PU001140").await;
    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(header(&response, "x-robots-tag"), "noindex");
    assert_eq!(header(&response, "cache-control"), "private, max-age=3600");
    let html = body_text(response).await;
    assert_basics(&html, "/vin/KM8K2CAB4PU001140");
    assert!(html.contains(r#"<meta name="robots" content="noindex">"#));
    assert!(!html.contains(r#"rel="canonical""#));
    assert!(
        html.contains(r#"<h1 id="summary">2023 Hyundai Kona SE</h1>"#),
        "{html}"
    );
    assert!(html.contains("<title>2023 Hyundai Kona SE - Wenmar Open</title>"));
    assert!(html.contains(r#"<tr><th scope="row">Engine</th><td>2.0L</td></tr>"#));
    assert!(html.contains(r#"<tr><th scope="row">ABS</th><td>Standard</td></tr>"#));
    assert!(
        html.contains(r#"<tr><th scope="row">Tire pressure monitoring</th><td>Direct</td></tr>"#)
    );
    assert!(html.contains(r#"<tr><th scope="row">Plant city</th><td>Ulsan</td></tr>"#));
    // The vehicle first, then the engine, then safety equipment.
    let vehicle = html.find("<caption>Vehicle</caption>").unwrap();
    let engine = html.find("<caption>Engine</caption>").unwrap();
    let safety = html.find("<caption>Safety equipment</caption>").unwrap();
    assert!(vehicle < engine && engine < safety);
    // Unknown fields have no row.
    assert!(!html.contains("Doors"));
    assert!(html.contains(
        r#"<a href="/makes/hyundai/kona/2023">All trims and engines for the 2023 Hyundai Kona</a>"#
    ));
    // Copy and print are conveniences that need the script; they start hidden.
    assert!(html.contains(r#"<button type="button" data-copy="summary" hidden>Copy</button>"#));
    assert!(html.contains(r#"<button type="button" data-print hidden>Print</button>"#));
    // One line about Wenmar Pro, and its link says nothing about the vehicle.
    assert_eq!(html.matches("utm_campaign=vin-result").count(), 1);
    for link in html.split("https://wenmarpro.com").skip(1) {
        let address = link.split('"').next().unwrap();
        assert!(!address.contains("KM8"), "{address}");
    }
}

#[tokio::test]
async fn a_vin_typed_untidily_is_sent_to_its_one_address() {
    let app = common::app().await;
    let (status, to) = redirect(&app, "/vin/km8k2cab4pu001140").await;
    assert_eq!(status, StatusCode::PERMANENT_REDIRECT);
    assert_eq!(to, "/vin/KM8K2CAB4PU001140");
    let (_, to) = redirect(&app, "/vin/KM8-K2CAB4-PU001140").await;
    assert_eq!(to, "/vin/KM8K2CAB4PU001140");
}

#[tokio::test]
async fn a_wrong_check_digit_shows_the_decode_with_a_warning() {
    let app = common::app().await;
    let (status, html) = page(&app, "/vin/KM8K2CAB0PU001140").await;
    assert_eq!(status, StatusCode::OK);
    assert!(
        html.contains(r#"<p class="warning">Position 9 should be 4 for this VIN, but it is 0."#),
        "{html}"
    );
    assert!(html.contains("2023 Hyundai Kona SE"));
}

#[tokio::test]
async fn html_in_the_data_is_shown_as_text() {
    let app = common::app().await;
    let (status, html) = page(&app, &format!("/vin/{}", common::HOSTILE)).await;
    assert_eq!(status, StatusCode::OK);
    assert_no_injection(&html);
    // It is there, as text.
    assert!(
        html.contains("&#60;script&#62;alert(1)&#60;/script&#62;"),
        "{html}"
    );
    assert!(
        html.contains("&#34;&#62;&#60;img src=x onerror=alert(1)&#62;"),
        "{html}"
    );
}

#[tokio::test]
async fn what_is_typed_in_the_address_is_never_shown_back_as_markup() {
    let app = common::app().await;
    for path in [
        "/vin/%3Cscript%3Ealert(1)%3C%2Fscript%3E",
        "/vin/%22%3E%3Cimg%20src=x%20onerror=alert(1)%3E",
        "/vin/KM8K2CAB4PU00114%3Cb%3EBold",
        "/vin/%FF%FE",
    ] {
        let (status, html) = page(&app, path).await;
        assert_eq!(status, StatusCode::BAD_REQUEST, "{path}");
        assert_no_injection(&html);
        assert_basics(&html, path);
    }
    let long = format!("/vin/{}", "A".repeat(60_000));
    // The server refuses an address over 8 KB before routing, so this never
    // reaches the page; what matters is that nothing large comes back.
    let (status, html) = page(&app, &long).await;
    assert_eq!(status, StatusCode::URI_TOO_LONG);
    assert!(html.len() < 10_000, "{} bytes", html.len());
}

#[tokio::test]
async fn a_malformed_vin_gets_a_page_that_helps() {
    let app = common::app().await;
    let response = app.get("/vin/KM8K2CAB4PUO01140").await;
    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    assert_eq!(header(&response, "x-robots-tag"), "noindex");
    let html = body_text(response).await;
    assert!(html.contains("<h1>That is not a VIN</h1>"));
    assert!(
        html.contains("A VIN uses only digits and letters other than I, O and Q."),
        "{html}"
    );
    assert!(
        html.contains(r#"<a class="mono" href="/vin/KM8K2CAB4PU001140">KM8K2CAB4PU001140</a>"#)
    );
    // The box is offered again.
    assert!(html.contains(r#"<form class="vin" action="/vin" method="get">"#));

    let (status, html) = page(&app, "/vin/KM8K2").await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert!(
        html.contains("A VIN has 17 characters, this has 5."),
        "{html}"
    );
}

#[tokio::test]
async fn an_unknown_manufacturer_gets_a_page_that_says_so() {
    let app = common::app().await;
    let (status, html) = page(&app, "/vin/ZZZK2CAB4PU001140").await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    assert!(html.contains("<h1>We do not know this manufacturer</h1>"));
    assert!(html.contains("No manufacturer is registered for ZZZ."));
    assert_basics(&html, "/vin/ZZZK2CAB4PU001140");
}

#[tokio::test]
async fn a_result_page_stays_private_when_revalidated() {
    let app = common::app().await;
    let etag = header(&app.get("/").await, "etag").to_owned();
    // A result page has no tag of its own to send back.
    let first = app.get("/vin/KM8K2CAB4PU001140").await;
    assert!(first.headers().get("etag").is_none());
    // The tag of a public page, or `*`, must not turn it into a public 304.
    for tag in [etag.as_str(), "*"] {
        let response = app
            .send(
                Request::get("/vin/KM8K2CAB4PU001140")
                    .header("if-none-match", tag)
                    .body(Body::empty())
                    .unwrap(),
            )
            .await;
        assert_eq!(response.status(), StatusCode::OK, "{tag}");
        assert_eq!(header(&response, "cache-control"), "private, max-age=3600");
        assert_eq!(header(&response, "x-robots-tag"), "noindex");
        assert!(response.headers().get("etag").is_none());
    }
}

#[tokio::test]
async fn a_failure_on_a_result_page_is_not_kept() {
    let app = common::app().await;
    for (path, status) in [
        ("/vin/KM8K2", StatusCode::BAD_REQUEST),
        ("/vin/ZZZK2CAB4PU001140", StatusCode::NOT_FOUND),
        ("/vin/%FF%FE", StatusCode::BAD_REQUEST),
    ] {
        let response = app.get(path).await;
        assert_eq!(response.status(), status, "{path}");
        assert_eq!(header(&response, "cache-control"), "no-store", "{path}");
        assert_eq!(header(&response, "x-robots-tag"), "noindex", "{path}");
    }
}
