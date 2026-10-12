//! What owners reported about a model year.

use axum::http::StatusCode;
use serde_json::{Value, json};

use crate::common;

#[tokio::test]
async fn a_model_year_gets_counts_and_the_words_that_go_with_them() {
    let app = common::complaints_app().await;
    let (status, body) = app.json("/v1/vehicles/2023_hyundai_kona/complaints").await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(
        body["vehicle"],
        json!({ "id": "2023_hyundai_kona", "year": 2023, "make": "Hyundai", "model": "Kona" })
    );
    assert_eq!(body["as_of"], "2026-10-11");
    assert_eq!(body["verified"], false);
    assert_eq!(body["complaints"], 60);
    assert_eq!(body["crashes"], 3);
    assert_eq!(body["fires"], 1);
    assert_eq!(body["injuries"], 2);
    assert_eq!(body["median_miles"], 30000);
    assert_eq!(body["median_km"], 48280);
    let ids: Vec<&str> = body["components"]
        .as_array()
        .unwrap()
        .iter()
        .map(|entry| entry["id"].as_str().unwrap())
        .collect();
    // Most reported first; brakes, with four, is not listed.
    assert_eq!(ids, ["electrical", "steering", "power-steering", "engine"]);
    assert_eq!(
        body["components"][2],
        json!({ "id": "power-steering", "name": "Power steering", "parent": "steering",
                "complaints": 12, "crashes": 0, "fires": 0, "injuries": 0,
                "median_km": null, "median_miles": null })
    );
    let note = body["note"].as_str().unwrap();
    assert!(note.contains("it is not verified"), "{note}");
    assert!(note.contains("A count is not a rate"), "{note}");
    assert_eq!(
        body["report_url"],
        "https://www.nhtsa.gov/report-a-safety-problem"
    );
    assert_eq!(body["attribution"], json!([]));
    // Nothing in the answer could be an owner's words, a place or a VIN.
    let keys: Vec<&String> = body.as_object().unwrap().keys().collect();
    for key in keys {
        for banned in [
            "description",
            "text",
            "city",
            "state",
            "vin",
            "dealer",
            "odi",
        ] {
            assert!(!key.contains(banned), "{key}");
        }
    }
}

#[tokio::test]
async fn few_or_no_complaints_are_zeros_and_an_empty_list() {
    let app = common::complaints_app().await;
    let (status, body) = app.json("/v1/vehicles/2022_hyundai_kona/complaints").await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["complaints"], 3);
    assert_eq!(body["median_miles"], Value::Null);
    assert_eq!(body["components"], json!([]));
    let (status, body) = app.json("/v1/vehicles/2019_honda_civic/complaints").await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["complaints"], 0);
    assert_eq!(body["components"], json!([]));
    assert_eq!(body["verified"], false);
    let (status, body) = app.json("/v1/vehicles/nonsense/complaints").await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    assert_eq!(body["error"]["code"], "not_found");
}

#[tokio::test]
async fn a_server_without_complaints_says_the_dataset_is_unavailable() {
    for app in [common::app().await, common::recalls_app().await] {
        let (status, body) = app.json("/v1/vehicles/2023_hyundai_kona/complaints").await;
        assert_eq!(status, StatusCode::NOT_FOUND);
        assert_eq!(
            body,
            json!({ "error": {
                "code": "dataset_unavailable",
                "message": "The complaints dataset is not available.",
                "details": { "dataset": "complaints" }
            }})
        );
        let (_, overview) = app.json("/v1/vehicles/2023_hyundai_kona/overview").await;
        assert_eq!(overview["complaints"], Value::Null);
    }
}

#[tokio::test]
async fn the_overview_has_the_counts_in_brief() {
    let app = common::complaints_app().await;
    let (status, body) = app.json("/v1/vehicles/2023_hyundai_kona/overview").await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let block = &body["complaints"];
    assert_eq!(block["verified"], false);
    assert_eq!(block["complaints"], 60);
    assert_eq!(block["median_km"], 48280);
    assert_eq!(block["as_of"], "2026-10-11");
    let top: Vec<&str> = block["top"]
        .as_array()
        .unwrap()
        .iter()
        .map(|entry| entry["id"].as_str().unwrap())
        .collect();
    assert_eq!(top, ["electrical", "steering", "power-steering"]);
    assert_eq!(
        block["url"],
        "https://open.example/makes/hyundai/kona/2023#owners-report"
    );
    // The recall block is untouched beside it.
    assert_eq!(body["recalls"]["may_apply"], true);
}
