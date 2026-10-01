//! The home page and the two forms on it.

use axum::http::StatusCode;

use crate::common::{self, assert_basics, assert_no_injection, body_text, header, redirect};

#[tokio::test]
async fn the_home_page_has_one_vin_box_and_a_picker_that_need_no_javascript() {
    let app = common::app().await;
    let response = app.get("/").await;
    assert_eq!(response.status(), StatusCode::OK);
    assert!(header(&response, "content-type").starts_with("text/html"));
    assert_eq!(header(&response, "cache-control"), "public, max-age=3600");
    assert!(response.headers().get("etag").is_some());
    assert!(response.headers().get("set-cookie").is_none());
    let html = body_text(response).await;
    assert_basics(&html, "/");
    assert_no_injection(&html);
    assert!(html.contains(r#"<link rel="canonical" href="https://open.example/">"#));
    assert!(
        !html.contains(r#"name="robots""#),
        "the home page is indexed"
    );

    // The VIN box is an ordinary form that sends a GET.
    assert_eq!(html.matches(r#"name="vin""#).count(), 1, "one VIN box");
    assert!(
        html.contains(r#"<form class="vin" action="/vin" method="get">"#),
        "{html}"
    );
    assert!(html.contains(r#"<label for="vin">VIN</label>"#));
    assert!(html.contains(r#"<input id="vin" name="vin" type="text" required"#));
    // So is the picker, beneath it.
    let vin_box = html.find(r#"action="/vin""#).unwrap();
    let picker = html.find(r#"action="/pick""#).unwrap();
    assert!(vin_box < picker);
    assert!(html.contains(r#"<form class="pick" action="/pick" method="get""#));
    assert!(html.contains(r#"<label for="year">Year</label>"#));
    assert!(html.contains(r#"<option value="2023">2023</option>"#));
    assert!(html.contains(r#"<label for="make">Make</label>"#));
    assert!(html.contains(r#"<option value="ford">Ford</option>"#));
    // Newest year first, popular makes first, and no trailers.
    assert!(html.find(r#"value="2023""#) < html.find(r#"value="2018""#));
    assert!(html.find(r#"value="ford""#) < html.find(r#"value="hyundai""#));
    assert!(!html.contains("ranger-trailers"));
}

#[tokio::test]
async fn the_vin_box_sends_the_visitor_to_a_tidy_address() {
    let app = common::app().await;
    let cases = [
        ("/vin?vin=km8-k2cab4+pu001140", "/vin/KM8K2CAB4PU001140"),
        ("/vin?vin=KM8K2CAB4PU001140", "/vin/KM8K2CAB4PU001140"),
        ("/vin?vin=", "/"),
        ("/vin", "/"),
        ("/vin?vin=%20%20", "/"),
        ("/vin?other=1", "/"),
        // Whatever is typed, only letters and digits reach the address.
        ("/vin?vin=..%2F..%2Fetc%2Fpasswd", "/vin/ETCPASSWD"),
        ("/vin?vin=%3Cscript%3E", "/vin/SCRIPT"),
        ("/vin?vin=%2F%2Fevil.example%2Fx", "/vin/EVILEXAMPLEX"),
        ("/vin?vin=%0d%0aSet-Cookie:x", "/vin/SETCOOKIEX"),
    ];
    for (path, expected) in cases {
        let (status, to) = redirect(&app, path).await;
        assert_eq!(status, StatusCode::SEE_OTHER, "{path}");
        assert_eq!(to, expected, "{path}");
    }
    let long = format!("/vin?vin={}", "A".repeat(5_000));
    let (_, to) = redirect(&app, &long).await;
    assert_eq!(to.len(), "/vin/".len() + 64);
}

#[tokio::test]
async fn the_picker_sends_the_visitor_to_a_make_or_to_the_makes() {
    let app = common::app().await;
    let cases = [
        ("/pick?year=2019&make=honda", "/makes/honda?year=2019"),
        ("/pick?year=2019&make=", "/makes?year=2019"),
        ("/pick?make=honda", "/makes/honda"),
        ("/pick", "/makes"),
        ("/pick?year=soon&make=honda", "/makes/honda"),
        // A make that is not an id form is dropped, never put in an address.
        ("/pick?year=2019&make=..%2F..%2Fetc", "/makes?year=2019"),
        (
            "/pick?year=2019&make=%2F%2Fevil.example",
            "/makes?year=2019",
        ),
        ("/pick?make=Honda%20Civic", "/makes"),
    ];
    for (path, expected) in cases {
        let (status, to) = redirect(&app, path).await;
        assert_eq!(status, StatusCode::SEE_OTHER, "{path}");
        assert_eq!(to, expected, "{path}");
    }
}
