//! The parts-ordering card: the route and the block on the VIN page.

use axum::http::StatusCode;
use serde_json::{Value, json};

use crate::common::{self, KONA, TestApp, page};

async fn card(app: &TestApp, path: &str) -> Value {
    let (status, body) = app.json(path).await;
    assert_eq!(status, StatusCode::OK, "{path}: {body}");
    body
}

/// The card's lines as `label|value|source`.
fn lines_of(body: &Value) -> Vec<String> {
    body["lines"]
        .as_array()
        .unwrap()
        .iter()
        .map(|line| {
            format!(
                "{}|{}|{}",
                line["label"].as_str().unwrap(),
                line["value"].as_str().unwrap(),
                line["source"].as_str().unwrap()
            )
        })
        .collect()
}

#[tokio::test]
async fn without_shop_data_the_card_is_the_decode_and_the_catalog_engine() {
    let app = common::app().await;
    let body = card(&app, &format!("/v1/vin/{KONA}/parts-card")).await;
    assert_eq!(body["vin"], KONA);
    assert_eq!(body["vehicle"]["id"], "2023_hyundai_kona");
    assert_eq!(body["engine_settled"], true);
    let lines = lines_of(&body);
    // Every line names its source, and the first is the VIN itself.
    assert_eq!(lines[0], format!("VIN|{KONA}|the VIN"));
    for expected in [
        "Model year|2023|VIN decode (NHTSA vPIC)",
        "Make|Hyundai|VIN decode (NHTSA vPIC)",
        "Model|Kona|VIN decode (NHTSA vPIC)",
        "Engine|2.0L|VIN decode (NHTSA vPIC)",
        "VIN position 8 (engine)|B|the VIN",
        "VIN position 11 (plant)|U|the VIN",
        "Catalog engine|2.0L (VIN position 8: A)|Wenmar Open catalog (NHTSA vPIC)",
    ] {
        assert!(
            lines.contains(&expected.to_owned()),
            "{expected} not in {lines:#?}"
        );
    }
    // Nothing from the shop file.
    assert!(
        !lines
            .iter()
            .any(|line| line.starts_with("Emission standard"))
    );
    assert!(!lines.iter().any(|line| line.starts_with("Build dates")));
    // The text is the lines, one each.
    let text = body["text"].as_str().unwrap();
    assert_eq!(text.lines().count(), lines.len());
    assert!(text.starts_with(&format!("VIN: {KONA}\n")));
    assert!(text.contains("\nEngine: 2.0L\n"));
    assert_eq!(body["attribution"], json!([]));
}

#[tokio::test]
async fn shop_data_adds_the_emission_standard_and_recall_build_dates() {
    let app = common::fuel_app().await;
    let body = card(&app, &format!("/v1/vin/{KONA}/parts-card")).await;
    let lines = lines_of(&body);
    // Only the standards of the ratings matched to the 2.0L.
    let standards: Vec<&String> = lines
        .iter()
        .filter(|line| line.starts_with("Emission standard"))
        .collect();
    assert_eq!(
        standards,
        [
            "Emission standard, EPA sales area 3|Federal Tier 3 Bin 30 (T3B30)|EPA emissions file, matched to this engine",
            "Emission standard, EPA sales area 7|California LEV-III SULEV30 (L3SULEV30)|EPA emissions file, matched to this engine",
        ]
    );

    let app = common::recalls_app().await;
    let body = card(&app, &format!("/v1/vin/{KONA}/parts-card")).await;
    let lines = lines_of(&body);
    assert!(
        lines.contains(
            &"Build dates covered by recall 23V123000|2022-09-01 to 2023-01-31|NHTSA recall 23V123000"
                .to_owned()
        ),
        "{lines:#?}"
    );
    assert_eq!(body["attribution"], json!([common::OGL_CANADA]));
}

#[tokio::test]
async fn a_vin_the_catalog_cannot_place_still_gets_what_the_decode_gives() {
    let app = common::fuel_app().await;
    let body = card(&app, &format!("/v1/vin/{}/parts-card", common::COACH)).await;
    assert_eq!(body["vehicle"], json!(null));
    assert_eq!(body["engine_settled"], false);
    let lines = lines_of(&body);
    assert!(lines[0].starts_with("VIN|"));
    assert!(!lines.iter().any(|line| line.starts_with("Catalog engine")));
    assert_eq!(
        body["warnings"].as_array().unwrap().last().unwrap()["code"],
        "no_catalog_vehicle"
    );
    for (path, status, code) in [
        (
            "/v1/vin/KM8K2CAB4PUO01140/parts-card",
            StatusCode::BAD_REQUEST,
            "invalid_vin",
        ),
        (
            "/v1/vin/ZZZK2CAB4PU001140/parts-card",
            StatusCode::NOT_FOUND,
            "not_found",
        ),
        (
            "/v1/vin/KM8K2CAB4PU001140/parts-card?year=x",
            StatusCode::BAD_REQUEST,
            "validation_failed",
        ),
    ] {
        let (found, body) = app.json(path).await;
        assert_eq!(found, status, "{path}: {body}");
        assert_eq!(body["error"]["code"], code, "{path}");
    }
}

#[tokio::test]
async fn the_vin_page_shows_the_card_as_text_that_can_be_selected() {
    let app = common::fuel_app().await;
    let (status, html) = page(&app, &format!("/vin/{KONA}")).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(
        html.matches(r#"<h2 id="parts-card">Parts-ordering card</h2>"#)
            .count(),
        1
    );
    // The text is on the page itself, so it can be selected and copied
    // with no JavaScript. The button is hidden until a script shows it.
    let start = html.find(r#"<pre class="card" tabindex="0">"#).unwrap();
    let end = start + html[start..].find("</pre>").unwrap();
    let text = &html[start..end];
    assert!(text.contains(&format!("VIN: {KONA}\n")), "{text}");
    assert!(text.contains("Engine: 2.0L\n"), "{text}");
    assert!(
        text.contains("Emission standard, EPA sales area 3: Federal Tier 3 Bin 30 (T3B30)"),
        "{text}"
    );
    assert!(html.contains(r#"hidden>Copy the card</button>"#));
    assert!(
        html.contains(r#"<span class="note">EPA emissions file, matched to this engine</span>"#)
    );
    // Before the recalls section when there is one, and always there.
    let app = common::app().await;
    let (_, html) = page(&app, &format!("/vin/{KONA}")).await;
    assert!(html.contains(r#"<h2 id="parts-card">"#));
    assert!(!html.contains("Emission standard"));
}
