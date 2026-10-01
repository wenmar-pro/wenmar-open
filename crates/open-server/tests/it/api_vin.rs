use axum::http::StatusCode;
use serde_json::{Value, json};

use crate::common::{self, body_json};

#[tokio::test]
async fn decodes_a_vin_with_its_catalog_entry() {
    let app = common::app().await;
    let (status, body) = app.json("/v1/vin/KM8K2CAB4PU001140").await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["vin"], "KM8K2CAB4PU001140");
    assert_eq!(body["valid"], true);
    assert_eq!(
        body["check_digit"],
        json!({ "valid": true, "expected": "4", "actual": "4" })
    );
    assert_eq!(body["year"], 2023);
    assert_eq!(body["make"], "Hyundai");
    assert_eq!(body["model"], "Kona");
    assert_eq!(body["trim"], "SE");
    assert_eq!(body["engine"]["label"], "2.0L");
    assert_eq!(body["safety"]["abs"], "Standard");
    assert_eq!(body["manufacturer"]["wmi"], "KM8");
    assert_eq!(body["plant"]["code"], "U");
    assert_eq!(body["warnings"], json!([]));
    // The catalog entry a vehicle form would have reached.
    assert_eq!(body["catalog"]["vehicle_id"], "2023_hyundai_kona");
    assert_eq!(body["catalog"]["submodel_id"], "se");
    assert_eq!(body["catalog"]["entry"]["id"], "2023_hyundai_kona_se_2-0l");
    // Unknown fields are left out, never null or empty.
    assert!(body.get("series").is_none());
    assert!(body.get("doors").is_none());
}

#[tokio::test]
async fn a_decode_carries_the_data_version_and_may_be_cached() {
    let app = common::app().await;
    let response = app.get("/v1/vin/KM8K2CAB4PU001140").await;
    assert_eq!(common::header(&response, "x-data-version"), "2026.09");
    assert_eq!(
        common::header(&response, "cache-control"),
        "public, max-age=86400"
    );
    assert!(response.headers().get("etag").is_some());
    // An error is not cached.
    let response = app.get("/v1/vin/nope").await;
    assert_eq!(common::header(&response, "cache-control"), "no-store");
    assert!(response.headers().get("etag").is_none());
    // `/v1/vin/batch` only takes POST.
    let response = app.get("/v1/vin/batch").await;
    assert_eq!(response.status(), StatusCode::METHOD_NOT_ALLOWED);
}

#[tokio::test]
async fn the_api_json_has_every_field_the_decoder_gives() {
    use wenmar_vin::sqlite::SqliteData;
    use wenmar_vin::{DecodeOptions, Decoder};

    let fixture = common::data_file();
    let direct = Decoder::new(SqliteData::open(fixture.path()).unwrap())
        .decode(common::KONA, DecodeOptions::default())
        .unwrap();
    let app = common::app().await;
    let (_, mut served) = app.json("/v1/vin/KM8K2CAB4PU001140").await;
    served.as_object_mut().unwrap().remove("catalog");
    assert_eq!(served, serde_json::to_value(&direct).unwrap());
}

#[tokio::test]
async fn spaces_dashes_and_lowercase_are_accepted() {
    let app = common::app().await;
    let (status, body) = app.json("/v1/vin/km8-k2cab4%20pu001140").await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["vin"], "KM8K2CAB4PU001140");
}

#[tokio::test]
async fn a_wrong_check_digit_is_not_an_error() {
    let app = common::app().await;
    let (status, body) = app.json("/v1/vin/KM8K2CAB0PU001140").await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["valid"], false);
    assert_eq!(body["model"], "Kona");
    assert_eq!(body["warnings"][0]["code"], "invalid_check_digit");
}

#[tokio::test]
async fn a_malformed_vin_is_400_with_suggestions() {
    let app = common::app().await;
    let (status, body) = app.json("/v1/vin/KM8K2CAB4PUO01140").await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert_eq!(body["error"]["code"], "invalid_vin");
    assert_eq!(
        body["error"]["details"]["suggestions"],
        json!(["KM8K2CAB4PU001140"])
    );
    let (status, body) = app.json("/v1/vin/KM8K2").await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert_eq!(
        body["error"]["message"],
        "a VIN has 17 characters, this has 5"
    );
}

#[tokio::test]
async fn an_unknown_manufacturer_is_404() {
    let app = common::app().await;
    let (status, body) = app.json("/v1/vin/ZZZK2CAB4PU001140").await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    assert_eq!(body["error"]["code"], "not_found");
    assert_eq!(
        body["error"]["message"],
        "No manufacturer is registered for ZZZ."
    );
    assert_eq!(body["error"]["details"], json!({}));
}

#[tokio::test]
async fn the_year_can_be_overridden() {
    let app = common::app().await;
    let (status, body) = app.json("/v1/vin/KM8K2CAB4PU001140?year=1993").await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["year"], 1993);
    assert_eq!(body["model"], "Old Model");
    // An empty value means no override.
    let (_, body) = app.json("/v1/vin/KM8K2CAB4PU001140?year=").await;
    assert_eq!(body["year"], 2023);
    for bad in ["abc", "1900", "9999", "-1", "20233"] {
        let (status, body) = app
            .json(&format!("/v1/vin/KM8K2CAB4PU001140?year={bad}"))
            .await;
        assert_eq!(status, StatusCode::BAD_REQUEST, "{bad}");
        assert_eq!(body["error"]["code"], "validation_failed", "{bad}");
    }
}

#[tokio::test]
async fn hostile_and_oversized_vins_are_refused_without_harm() {
    let app = common::app().await;
    // An address may be 8 KB, so this is about the longest that can reach
    // the handler. A longer one is refused before it gets there.
    let long = "A".repeat(8_000);
    let inputs = [
        long.as_str(),
        "%00%00%00",
        "..%2F..%2Fetc%2Fpasswd",
        "%27%3B%20DROP%20TABLE%20wmi%3B--",
        "%F0%9F%9A%97%F0%9F%9A%97",
        "%FF%FE",
        "KM8K2CAB4PU00114%",
        "%3Cscript%3Ealert(1)%3C%2Fscript%3E",
    ];
    for input in inputs {
        let (status, body) = app.json(&format!("/v1/vin/{input}")).await;
        assert_eq!(status, StatusCode::BAD_REQUEST, "{:.40}", input);
        assert_eq!(body["error"]["code"], "invalid_vin", "{:.40}", input);
        // The input is never repeated back.
        let text = body.to_string();
        assert!(!text.contains("script"), "{text}");
        assert!(text.len() < 2_000, "{} bytes", text.len());
    }
    // The data file still answers.
    let (status, _) = app.json("/v1/vin/KM8K2CAB4PU001140").await;
    assert_eq!(status, StatusCode::OK);
}

#[tokio::test]
async fn html_in_the_data_is_sent_as_it_is_in_json_that_no_browser_renders() {
    let app = common::app().await;
    let response = app.get(&format!("/v1/vin/{}", common::HOSTILE)).await;
    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(
        common::header(&response, "content-type"),
        "application/json"
    );
    assert_eq!(
        common::header(&response, "x-content-type-options"),
        "nosniff"
    );
    let body = body_json(response).await;
    // JSON carries the text exactly. Showing it safely is the reader's job,
    // and the pages of this site are tested for it on their own.
    assert_eq!(body["model"], "<script>alert(1)</script>");
    assert_eq!(body["trim"], "\"><img src=x onerror=alert(1)>");
}

#[tokio::test]
async fn a_batch_answers_in_order_with_decodes_and_errors() {
    let app = common::app().await;
    let response = app
        .post_json(
            "/v1/vin/batch",
            r#"{ "vins": ["KM8K2CAB4PU001140", "nope", "ZZZK2CAB4PU001140", "1A9100AA851881001"] }"#,
        )
        .await;
    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(common::header(&response, "x-data-version"), "2026.09");
    assert_eq!(common::header(&response, "cache-control"), "no-store");
    let body = body_json(response).await;
    let items = body.as_array().unwrap();
    assert_eq!(items.len(), 4);
    assert_eq!(items[0]["model"], "Kona");
    assert_eq!(items[1]["error"]["code"], "invalid_vin");
    assert_eq!(items[2]["error"]["code"], "not_found");
    assert_eq!(items[3]["manufacturer"]["wmi"], "1A9881");
}

#[tokio::test]
async fn a_batch_of_fifty_is_accepted_and_fifty_one_is_not() {
    let app = common::app().await;
    let vins = |count: usize| {
        serde_json::to_string(&json!({ "vins": vec![common::KONA; count] })).unwrap()
    };
    let response = app.post_json("/v1/vin/batch", &vins(50)).await;
    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(body_json(response).await.as_array().unwrap().len(), 50);

    let response = app.post_json("/v1/vin/batch", &vins(51)).await;
    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    let body = body_json(response).await;
    assert_eq!(body["error"]["code"], "validation_failed");
    assert_eq!(
        body["error"]["details"],
        json!({ "field": "vins", "max": 50, "received": 51 })
    );

    let response = app.post_json("/v1/vin/batch", r#"{ "vins": [] }"#).await;
    assert_eq!(body_json(response).await, json!([]));
}

#[tokio::test]
async fn a_batch_body_that_is_wrong_is_a_json_error() {
    let app = common::app().await;
    let bodies = [
        "",
        "not json",
        "[]",
        "{}",
        r#"{ "vins": "KM8K2CAB4PU001140" }"#,
        r#"{ "vins": [1, 2] }"#,
        r#"{ "vins": [null] }"#,
        r#"{ "vins": [["KM8K2CAB4PU001140"]] }"#,
    ];
    for body in bodies {
        let response = app.post_json("/v1/vin/batch", body).await;
        assert_eq!(response.status(), StatusCode::BAD_REQUEST, "{body}");
        let answer: Value = body_json(response).await;
        assert_eq!(answer["error"]["code"], "validation_failed", "{body}");
    }
    // Without a JSON content type.
    let response = app
        .send(
            axum::http::Request::post("/v1/vin/batch")
                .body(axum::body::Body::from(r#"{ "vins": [] }"#))
                .unwrap(),
        )
        .await;
    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    assert_eq!(
        body_json(response).await["error"]["code"],
        "validation_failed"
    );
}

#[tokio::test]
async fn an_oversized_batch_body_is_413() {
    let app = common::app().await;
    let huge = format!(r#"{{ "vins": ["{}"] }}"#, "A".repeat(1_000_000));
    let response = app.post_json("/v1/vin/batch", &huge).await;
    assert_eq!(response.status(), StatusCode::PAYLOAD_TOO_LARGE);
    assert_eq!(
        body_json(response).await["error"]["code"],
        "payload_too_large"
    );
}
