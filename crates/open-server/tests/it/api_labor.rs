//! The job list and one job's definition.

use axum::http::StatusCode;
use serde_json::{Value, json};

use crate::common::{self, TestApp};

async fn ok(app: &TestApp, path: &str) -> Value {
    let (status, body) = app.json(path).await;
    assert_eq!(status, StatusCode::OK, "{path}: {body}");
    body
}

#[tokio::test]
async fn the_job_list_and_a_job_definition_still_answer() {
    let app = common::jobs_app().await;
    let body = ok(&app, "/v1/jobs").await;
    assert_eq!(body["as_of"], "2026-10-11");
    assert_eq!(body["attribution"], json!([]));
    assert_eq!(
        body["jobs"],
        json!([
            { "id": "test-job-a", "name": "Test job A", "system": "engine", "url": "https://open.example/jobs/test-job-a" },
            { "id": "test-job-b", "name": "Test job B", "system": "brakes", "url": "https://open.example/jobs/test-job-b" },
            { "id": "test-job-c", "name": "Test job C", "system": "engine", "url": "https://open.example/jobs/test-job-c" }
        ])
    );
    let job = ok(&app, "/v1/jobs/test-job-a").await;
    assert_eq!(job["name"], "Test job A");
    assert_eq!(job["system_name"], "Engine");
    assert_eq!(
        job["includes"],
        json!(["made-up step one", "made-up step two"])
    );
    assert_eq!(job["excludes"], json!(["made-up exclusion"]));
    assert_eq!(
        job["components"],
        json!([
            { "id": "timing-belt", "name": "Timing belt or chain", "parent": "engine" },
            { "id": "engine", "name": "Engine", "parent": null }
        ])
    );
    assert_eq!(job["related"][0]["id"], "test-job-b");
    assert_eq!(job["url"], "https://open.example/jobs/test-job-a");
    assert!(!job.as_object().unwrap().contains_key("estimates"));
    assert!(!job.as_object().unwrap().contains_key("note"));
    assert!(!job.as_object().unwrap().contains_key("attribution"));
}

#[tokio::test]
async fn labour_figures_are_no_longer_served() {
    let app = common::jobs_app().await;
    let kona = common::KONA;
    for path in [
        "/v1/vehicles/2023_hyundai_kona/labor".to_owned(),
        "/v1/vehicles/2023_hyundai_kona/labor/test-job-a".to_owned(),
        "/v1/vehicles/2019_honda_nothing/labor".to_owned(),
        format!("/v1/vin/{kona}/labor"),
        format!("/v1/vin/{kona}/labor/test-job-a"),
        "/v1/jobs/test-job-a/vehicles".to_owned(),
    ] {
        let (status, body) = app.json(&path).await;
        assert_eq!(status, StatusCode::NOT_FOUND, "{path}: {body}");
    }
}
