//! The investigation routes: the open investigations of a model year, and
//! the investigations that led to a recall campaign.

use axum::http::StatusCode;
use serde_json::{Value, json};

use crate::common::{self, TestApp};

const NOTE: &str = "An investigation is NHTSA looking into a possible defect. It is not a finding that a defect exists, and it does not say that any one vehicle is affected.";

/// The action numbers of a list, in order.
fn actions(body: &Value) -> Vec<String> {
    body["investigations"]
        .as_array()
        .unwrap_or_else(|| panic!("no investigations in {body}"))
        .iter()
        .map(|found| found["action"].as_str().unwrap().to_owned())
        .collect()
}

async fn ok(app: &TestApp, path: &str) -> Value {
    let (status, body) = app.json(path).await;
    assert_eq!(status, StatusCode::OK, "{path}: {body}");
    body
}

#[tokio::test]
async fn a_model_year_lists_its_open_investigations_newest_first() {
    let app = common::investigations_app().await;
    let body = ok(&app, "/v1/vehicles/2023_hyundai_kona/investigations").await;
    assert_eq!(
        body["vehicle"],
        json!({ "id": "2023_hyundai_kona", "year": 2023, "make": "Hyundai", "model": "Kona" })
    );
    assert_eq!(body["as_of"], "2026-10-12");
    assert_eq!(body["note"], NOTE);
    assert_eq!(body["attribution"], json!([]));
    // The audit query is open and is not about the vehicle; PE22001 is
    // closed.
    assert_eq!(actions(&body), ["PE25010", "EA24003"]);
    assert_eq!(
        body["investigations"][0],
        json!({
            "action": "PE25010",
            "kind": "PE",
            "kind_name": "Preliminary evaluation",
            "opened": "2025-09-15",
            "closed": null,
            "open": true,
            "manufacturer": "Hyundai Motor America",
            "component": "ENGINE; FUEL SYSTEM, GASOLINE",
            "subject": "Loss of motive power",
            "summary": "The engine may stall while driving.",
            "campaigns": [],
            "match": "exact",
            "models": ["KONA", "KONA ELECTRIC"]
        })
    );
    assert_eq!(body["investigations"][1]["campaigns"], json!(["24V456000"]));

    // `status=open` is the default; `all` adds the audit query and the
    // closed one.
    let open = ok(
        &app,
        "/v1/vehicles/2023_hyundai_kona/investigations?status=open",
    )
    .await;
    assert_eq!(open, body);
    let all = ok(
        &app,
        "/v1/vehicles/2023_hyundai_kona/investigations?status=all",
    )
    .await;
    assert_eq!(actions(&all), ["PE25010", "EA24003", "AQ22002", "PE22001"]);
    assert_eq!(all["investigations"][2]["kind_name"], "Audit query");
    assert_eq!(all["investigations"][3]["open"], false);
    assert_eq!(all["investigations"][3]["closed"], "2022-12-01");

    // No day closed in the file, but opened in 1998: not open.
    let old = ok(&app, "/v1/vehicles/2019_chevrolet_silverado/investigations").await;
    assert_eq!(actions(&old), [] as [&str; 0]);
    let old = ok(
        &app,
        "/v1/vehicles/2019_chevrolet_silverado/investigations?status=all",
    )
    .await;
    assert_eq!(actions(&old), ["EA19009", "PE19004", "RQ98022"]);
    assert_eq!(old["investigations"][2]["open"], false);
    assert_eq!(old["investigations"][2]["closed"], Value::Null);

    // A model year no investigation lists; a submodel in the id is set aside.
    let none = ok(&app, "/v1/vehicles/2019_honda_civic_si/investigations").await;
    assert_eq!(none["vehicle"]["id"], "2019_honda_civic");
    assert_eq!(none["investigations"], json!([]));
    assert_eq!(none["note"], NOTE);
}

#[tokio::test]
async fn a_campaign_lists_the_investigations_that_led_to_it_oldest_first() {
    let app = common::investigations_app().await;
    let body = ok(&app, "/v1/recalls/nhtsa/19V888000/investigations").await;
    assert_eq!(body["source"], "nhtsa");
    assert_eq!(body["campaign"], "19V888000");
    assert_eq!(body["as_of"], "2026-10-12");
    assert_eq!(body["note"], NOTE);
    assert_eq!(body["url"], "https://open.example/recalls/nhtsa/19V888000");
    assert_eq!(actions(&body), ["PE19004", "EA19009"]);
    assert_eq!(
        body["investigations"][0],
        json!({
            "action": "PE19004",
            "kind": "PE",
            "kind_name": "Preliminary evaluation",
            "opened": "2019-02-11",
            "closed": "2019-06-20",
            "open": false,
            "manufacturer": "General Motors, LLC",
            "component": "ELECTRICAL SYSTEM:12V/24V/48V BATTERY:CABLES",
            "subject": "Battery cable connection",
            "summary": "The cable nut may be loose.",
            "campaigns": ["19V888000"],
            "match": null,
            "models": []
        })
    );
    // Letters in either case.
    let lower = ok(&app, "/v1/recalls/nhtsa/19v888000/investigations").await;
    assert_eq!(lower, body);
    // A campaign that followed none, one the recall data does not hold, and
    // a Transport Canada campaign: an empty list each.
    for path in [
        "/v1/recalls/nhtsa/22V789000/investigations",
        "/v1/recalls/nhtsa/94E013001/investigations",
        "/v1/recalls/tc/2019631/investigations",
    ] {
        let body = ok(&app, path).await;
        assert_eq!(body["investigations"], json!([]), "{path}");
    }
    // An investigation with nothing but its campaigns.
    let stub = ok(&app, "/v1/recalls/nhtsa/94E013000/investigations").await;
    assert_eq!(actions(&stub), ["ID94001"]);
    assert_eq!(stub["investigations"][0]["kind_name"], Value::Null);
    assert_eq!(stub["investigations"][0]["subject"], Value::Null);
}

#[tokio::test]
async fn what_cannot_be_answered_is_an_error_with_its_code() {
    let app = common::investigations_app().await;
    for (path, status, code) in [
        (
            "/v1/vehicles/2023_hyundai_kona/investigations?status=closed",
            StatusCode::BAD_REQUEST,
            "validation_failed",
        ),
        (
            "/v1/vehicles/2019_honda_nothing/investigations",
            StatusCode::NOT_FOUND,
            "not_found",
        ),
        (
            "/v1/recalls/epa/19V888000/investigations",
            StatusCode::NOT_FOUND,
            "not_found",
        ),
        (
            "/v1/recalls/nhtsa/19V-888/investigations",
            StatusCode::NOT_FOUND,
            "not_found",
        ),
        (
            "/v1/recalls/nhtsa/123456789012345678901/investigations",
            StatusCode::NOT_FOUND,
            "not_found",
        ),
    ] {
        let (found, body) = app.json(path).await;
        assert_eq!(found, status, "{path}: {body}");
        assert_eq!(body["error"]["code"], code, "{path}");
    }
    let (_, body) = app
        .json("/v1/vehicles/2023_hyundai_kona/investigations?status=closed")
        .await;
    assert_eq!(body["error"]["message"], "status must be open or all");
    assert_eq!(body["error"]["details"], json!({ "field": "status" }));
}

#[tokio::test]
async fn a_server_without_investigations_says_so_and_one_with_only_them_answers() {
    // Recalls, and no investigations.
    let app = common::recalls_app().await;
    for path in [
        "/v1/vehicles/2023_hyundai_kona/investigations",
        "/v1/recalls/nhtsa/19V888000/investigations",
    ] {
        let (status, body) = app.json(path).await;
        assert_eq!(status, StatusCode::NOT_FOUND, "{path}");
        assert_eq!(
            body,
            json!({ "error": {
                "code": "dataset_unavailable",
                "message": "The investigations dataset is not available.",
                "details": { "dataset": "investigations" }
            } }),
            "{path}"
        );
    }
    // No shop file at all.
    let (status, body) = common::app()
        .await
        .json("/v1/vehicles/2023_hyundai_kona/investigations")
        .await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    assert_eq!(body["error"]["code"], "dataset_unavailable");

    // Investigations, and no recalls: both routes answer.
    let rows = format!(
        "{}{}",
        wenmar_shopdata::schema::INVESTIGATIONS,
        common::INVESTIGATION_INSERTS
    );
    let app = common::app_with_shop(common::INVESTIGATION_DATASETS_ALONE, &rows).await;
    let body = ok(&app, "/v1/vehicles/2023_hyundai_kona/investigations").await;
    assert_eq!(actions(&body), ["PE25010", "EA24003"]);
    let body = ok(&app, "/v1/recalls/nhtsa/24V456000/investigations").await;
    assert_eq!(actions(&body), ["EA24003"]);
    let (_, meta) = app.json("/v1/meta").await;
    assert_eq!(meta["datasets"]["investigations"]["rows"], 9);
}
