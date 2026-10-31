//! The job list and the labour routes.

use axum::http::StatusCode;
use serde_json::{Value, json};

use crate::common::{self, TestApp};

const NOTE: &str =
    "An estimate, not a quote. Rust, seized fasteners and added equipment change the time.";

async fn ok(app: &TestApp, path: &str) -> Value {
    let (status, body) = app.json(path).await;
    assert_eq!(status, StatusCode::OK, "{path}: {body}");
    body
}

async fn refused(app: &TestApp, path: &str, code: &str) -> Value {
    let (status, body) = app.json(path).await;
    assert_eq!(status, StatusCode::NOT_FOUND, "{path}: {body}");
    assert_eq!(body["error"]["code"], code, "{path}: {body}");
    body
}

/// Job, scope and typical hours of each estimate.
fn figures(body: &Value) -> Vec<(String, String, f64)> {
    body["estimates"]
        .as_array()
        .unwrap_or_else(|| panic!("no estimates in {body}"))
        .iter()
        .map(|estimate| {
            (
                estimate["job"].as_str().unwrap().to_owned(),
                estimate["scope"].as_str().unwrap().to_owned(),
                estimate["hours"]["typical"].as_f64().unwrap(),
            )
        })
        .collect()
}

fn one(job: &str, scope: &str, hours: f64) -> (String, String, f64) {
    (job.to_owned(), scope.to_owned(), hours)
}

#[tokio::test]
async fn the_job_list_and_one_job() {
    let app = common::labor_app().await;
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
    // Its class figures, with the credit and the caution they need.
    assert_eq!(figures(&job), [one("test-job-a", "class", 98.0)]);
    assert_eq!(job["estimates"][0]["class_name"], "Small SUV");
    assert_eq!(job["note"], NOTE);
    assert_eq!(job["attribution"], json!([common::LABOR_CREDIT]));
    // A measured class figure is given before the written one.
    let job = ok(&app, "/v1/jobs/test-job-b").await;
    assert_eq!(figures(&job), [one("test-job-b", "class", 48.0)]);
    assert_eq!(job["estimates"][0]["basis"], "observed");
    assert_eq!(
        job["estimates"][0]["sample"],
        json!({ "jobs": 40, "shops": 6 })
    );
    for path in ["/v1/jobs/test-job-z", "/v1/jobs/Not%20A%20Job"] {
        let body = refused(&app, path, "not_found").await;
        assert_eq!(body["error"]["message"], "No job has that id.");
    }
}

#[tokio::test]
async fn a_vehicle_gets_the_narrowest_figure_of_each_job_and_which_scope_it_was() {
    let app = common::labor_app().await;
    let body = ok(&app, "/v1/vehicles/2023_hyundai_kona/labor").await;
    assert_eq!(
        body["vehicle"],
        json!({ "id": "2023_hyundai_kona", "year": 2023, "make": "Hyundai", "model": "Kona" })
    );
    assert_eq!(body["engine"], Value::Null);
    assert_eq!(
        body["class"],
        json!({ "id": "small-suv", "name": "Small SUV", "source": "epa" })
    );
    assert_eq!(body["as_of"], "2026-10-12");
    assert_eq!(body["note"], NOTE);
    assert_eq!(body["attribution"], json!([common::LABOR_CREDIT]));
    // Job A by its model, job B by the class and measured; job C has no
    // figure that reaches a small SUV.
    assert_eq!(
        figures(&body),
        [
            one("test-job-a", "model", 88.0),
            one("test-job-b", "class", 48.0)
        ]
    );
    assert_eq!(
        body["estimates"][0],
        json!({
            "job": "test-job-a",
            "name": "Test job A",
            "hours": { "low": 87.0, "typical": 88.0, "high": 89.0 },
            "basis": "expert",
            "scope": "model",
            "class": null,
            "class_name": null,
            "years": null,
            "engine": null,
            "sample": null,
            "confidence_pct": null,
            "model": null,
            "method": null,
            "inputs": null,
            "notes": null,
            "updated": "2026-10-10",
            "method_url": "https://open.example/jobs/how-estimates-are-made",
            "url": "https://open.example/jobs/test-job-a"
        })
    );
    let measured = &body["estimates"][1];
    assert_eq!(measured["basis"], "observed");
    assert_eq!(measured["class"], "small-suv");
    assert_eq!(measured["class_name"], "Small SUV");
    assert_eq!(measured["sample"], json!({ "jobs": 40, "shops": 6 }));
    assert_eq!(
        measured["hours"],
        json!({ "low": 47.0, "typical": 48.0, "high": 49.0 })
    );

    // One job: the same answer, narrowed.
    let body = ok(&app, "/v1/vehicles/2023_hyundai_kona/labor/test-job-b").await;
    assert_eq!(figures(&body), [one("test-job-b", "class", 48.0)]);
    // A job of the list with no figure for the vehicle is not an error.
    let body = ok(&app, "/v1/vehicles/2023_hyundai_kona/labor/test-job-c").await;
    assert_eq!(body["estimates"], json!([]));
    assert_eq!(body["attribution"], json!([common::LABOR_CREDIT]));
    // A year whose class is its neighbour's says so.
    let body = ok(&app, "/v1/vehicles/2022_hyundai_kona/labor").await;
    assert_eq!(body["class"]["source"], "nearest");
    // A vehicle with no class and no figure of its own has none.
    let body = ok(&app, "/v1/vehicles/2019_honda_civic/labor").await;
    assert_eq!(body["class"], Value::Null);
    assert_eq!(body["estimates"], json!([]));

    for (path, message) in [
        (
            "/v1/vehicles/2023_hyundai_kona/labor/test-job-z",
            "No job has that id.",
        ),
        (
            "/v1/vehicles/2019_honda_nothing/labor/test-job-z",
            "No vehicle has that id.",
        ),
    ] {
        let body = refused(&app, path, "not_found").await;
        assert_eq!(body["error"]["message"], message, "{path}");
    }
}

#[tokio::test]
async fn a_vin_gets_the_figures_of_the_vehicle_it_decodes_to() {
    let app = common::labor_app().await;
    let body = ok(&app, &format!("/v1/vin/{}/labor", common::KONA)).await;
    assert_eq!(body["decode"]["vin"], common::KONA);
    assert_eq!(body["vehicle"]["id"], "2023_hyundai_kona");
    assert_eq!(body["class"]["id"], "small-suv");
    assert_eq!(body["attribution"], json!([common::LABOR_CREDIT]));
    assert_eq!(body["note"], NOTE);
    assert_eq!(body["estimates"].as_array().unwrap().len(), 2);
    let body = ok(&app, &format!("/v1/vin/{}/labor/test-job-b", common::KONA)).await;
    assert_eq!(figures(&body), [one("test-job-b", "class", 48.0)]);
    // A VIN that reaches no catalog vehicle is not an error.
    let body = ok(&app, &format!("/v1/vin/{}/labor", common::TRAILER)).await;
    if body["vehicle"].is_null() {
        assert_eq!(body["estimates"], json!([]));
        assert_eq!(body["warnings"][0]["code"], "no_catalog_vehicle");
    }
    let (status, body) = app.json("/v1/vin/NOTAVIN/labor").await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert_eq!(body["error"]["code"], "invalid_vin");
    let body = refused(
        &app,
        &format!("/v1/vin/{}/labor/test-job-z", common::KONA),
        "not_found",
    )
    .await;
    assert_eq!(body["error"]["message"], "No job has that id.");
}

#[tokio::test]
async fn the_overview_has_a_labor_block_where_the_estimates_are_held() {
    let app = common::labor_app().await;
    let overview = ok(&app, "/v1/vehicles/2023_hyundai_kona/overview").await;
    let labor = ok(&app, "/v1/vehicles/2023_hyundai_kona/labor").await;
    assert_eq!(overview["labor"], labor);
    assert!(
        overview["attribution"]
            .as_array()
            .unwrap()
            .contains(&json!(common::LABOR_CREDIT))
    );
    let overview = ok(&app, &format!("/v1/vin/{}/overview", common::KONA)).await;
    assert_eq!(overview["labor"], labor);
    // Without the estimates the block is null, and still there.
    let app = common::recalls_app().await;
    let overview = ok(&app, "/v1/vehicles/2023_hyundai_kona/overview").await;
    assert_eq!(overview["labor"], Value::Null);
    assert!(overview.as_object().unwrap().contains_key("labor"));
}

#[tokio::test]
async fn each_route_needs_its_dataset() {
    // The job list alone.
    let app = common::app_with_shop(common::JOB_DATASETS, &common::job_rows()).await;
    assert_eq!(
        ok(&app, "/v1/jobs").await["jobs"].as_array().unwrap().len(),
        3
    );
    let job = ok(&app, "/v1/jobs/test-job-a").await;
    assert_eq!(job["estimates"], Value::Null);
    assert_eq!(job["attribution"], json!([]));
    for path in [
        "/v1/vehicles/2023_hyundai_kona/labor".to_owned(),
        "/v1/vehicles/2023_hyundai_kona/labor/test-job-a".to_owned(),
        "/v1/vehicles/2019_honda_nothing/labor".to_owned(),
        format!("/v1/vin/{}/labor", common::KONA),
        format!("/v1/vin/{}/labor/test-job-a", common::KONA),
    ] {
        let body = refused(&app, &path, "dataset_unavailable").await;
        assert_eq!(
            body["error"]["message"],
            "The labor dataset is not available."
        );
    }
    // Recalls alone, and no shop file.
    for app in [common::recalls_app().await, common::app().await] {
        for path in ["/v1/jobs", "/v1/jobs/test-job-a"] {
            let body = refused(&app, path, "dataset_unavailable").await;
            assert_eq!(
                body["error"]["message"],
                "The jobs dataset is not available."
            );
        }
        refused(
            &app,
            "/v1/vehicles/2023_hyundai_kona/labor",
            "dataset_unavailable",
        )
        .await;
    }
}

#[tokio::test]
async fn an_ai_figure_says_how_it_was_made() {
    let app = common::labor_ai_app().await;
    let body = ok(&app, "/v1/vehicles/2023_hyundai_kona/labor/test-job-c").await;
    let figure = &body["estimates"][0];
    assert_eq!(figure["basis"], "ai");
    assert_eq!(figure["scope"], "generation");
    assert_eq!(figure["confidence_pct"], 45);
    assert_eq!(figure["model"], "test-model");
    assert_eq!(figure["method"], "open-labour-ai-1");
    assert_eq!(figure["inputs"], json!(["vehicle", "job", "procedure"]));
    assert_eq!(figure["notes"], "Made-up assumption.");
    assert_eq!(
        figure["method_url"],
        "https://open.example/jobs/how-estimates-are-made"
    );
}

#[tokio::test]
async fn every_figure_says_where_its_method_is_explained() {
    let app = common::labor_app().await;
    for path in [
        "/v1/jobs/test-job-a",
        "/v1/vehicles/2023_hyundai_kona/labor",
        "/v1/vehicles/2023_hyundai_kona/labor/test-job-b",
        &format!("/v1/vin/{}/labor", common::KONA),
    ] {
        let body = ok(&app, path).await;
        for figure in body["estimates"].as_array().unwrap() {
            assert_eq!(
                figure["method_url"], "https://open.example/jobs/how-estimates-are-made",
                "{path}: {figure}"
            );
        }
    }
}
