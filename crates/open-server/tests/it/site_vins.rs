//! `/vins`: a pasted list of VINs, as a table or a CSV file.

use axum::body::Body;
use axum::http::{Request, StatusCode};

use crate::common::{self, body_text, header};

fn form(body: &str) -> Request<Body> {
    Request::post("/vins")
        .header("content-type", "application/x-www-form-urlencoded")
        .body(Body::from(body.to_owned()))
        .unwrap()
}

#[tokio::test]
async fn the_form_is_offered_and_never_indexed() {
    let app = common::app().await;
    let response = app.get("/vins").await;
    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(header(&response, "x-robots-tag"), "noindex");
    let html = body_text(response).await;
    assert!(html.contains(r#"<form class="feedback" action="/vins" method="post">"#));
    // The single-VIN form leads here.
    let home = body_text(app.get("/").await).await;
    assert!(home.contains(r#"<a href="/vins">Decode many at once</a>"#));
}

#[tokio::test]
async fn a_pasted_list_is_a_table_in_the_order_pasted() {
    let app = common::app().await;
    // A spreadsheet column: a header, a VIN in lowercase, and a VIN no
    // manufacturer is registered for.
    let response = app
        .send(form(
            "vins=unit%2Cvin%0A1%2Ckm8k2cab4pu001140%0A2%2CZZZK2CAB4PU001140&format=table",
        ))
        .await;
    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(header(&response, "cache-control"), "no-store");
    let html = body_text(response).await;
    assert!(html.contains("<caption>2 VINs</caption>"), "{html}");
    let kona = html
        .find(r#"<a href="/vin/KM8K2CAB4PU001140">"#)
        .expect("the Kona links");
    let unknown = html
        .find("ZZZK2CAB4PU001140</td>")
        .expect("the unknown VIN is listed");
    assert!(kona < unknown);
    assert!(html.contains("2023 Hyundai Kona SE"));
    assert!(
        html.contains("No manufacturer is registered for ZZZ"),
        "{html}"
    );
}

#[tokio::test]
async fn a_pasted_list_can_be_downloaded_as_csv() {
    let app = common::app().await;
    let response = app
        .send(form("vins=KM8K2CAB4PU001140%0Anope&format=csv"))
        .await;
    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(header(&response, "content-type"), "text/csv; charset=utf-8");
    let text = body_text(response).await;
    let mut lines = text.lines();
    assert!(
        lines
            .next()
            .unwrap()
            .starts_with("vin,valid,year,make,model,")
    );
    assert!(
        lines
            .next()
            .unwrap()
            .starts_with("KM8K2CAB4PU001140,true,2023,Hyundai,Kona,")
    );
    // "nope" is not a VIN, so it was passed over.
    assert_eq!(lines.next(), None);
}

#[tokio::test]
async fn a_paste_with_no_vin_says_so() {
    let app = common::app().await;
    let response = app.send(form("vins=hello&format=table")).await;
    assert_eq!(response.status(), StatusCode::OK);
    let html = body_text(response).await;
    assert!(html.contains("No 17-character VIN was found"));
}
