//! The trouble code routes: one code, the list, and a code on a vehicle.

use axum::http::StatusCode;
use serde_json::{Value, json};

use crate::common::{self, TestApp};

async fn ok(app: &TestApp, path: &str) -> Value {
    let (status, body) = app.json(path).await;
    assert_eq!(status, StatusCode::OK, "{path}: {body}");
    body
}

async fn refused(app: &TestApp, path: &str, status: StatusCode, code: &str) -> Value {
    let (found, body) = app.json(path).await;
    assert_eq!(found, status, "{path}: {body}");
    assert_eq!(body["error"]["code"], code, "{path}: {body}");
    body
}

fn codes(body: &Value) -> Vec<&str> {
    body["codes"]
        .as_array()
        .unwrap_or_else(|| panic!("no codes in {body}"))
        .iter()
        .map(|code| code["code"].as_str().unwrap())
        .collect()
}

fn ids(list: &Value) -> Vec<i64> {
    list.as_array()
        .unwrap_or_else(|| panic!("not a list: {list}"))
        .iter()
        .map(|bulletin| bulletin["nhtsa_id"].as_i64().unwrap())
        .collect()
}

#[tokio::test]
async fn a_code_is_given_with_everything_its_record_holds() {
    let app = common::codes_app().await;
    let body = ok(&app, "/v1/codes/P0420").await;
    assert_eq!(
        body,
        json!({
            "code": "P0420",
            "generic": true,
            "system": "powertrain/emissions",
            "system_name": "Emission controls",
            "title": "Made-up title two",
            "meaning": "Made-up meaning two.",
            "common_causes": ["Made-up cause B", "Made-up cause C"],
            "checks": ["Made-up check B"],
            "components": [
                { "id": "catalytic-converter", "name": "Catalytic converter", "parent": "exhaust-emissions" },
                { "id": "electrical", "name": "Electrical system", "parent": null }
            ],
            "related": [{
                "code": "P0300",
                "system": "powertrain/ignition",
                "title": "Made-up title one",
                "url": "https://open.example/codes/P0300"
            }],
            "reviewer": "test-reviewer",
            "checked": "2026-10-12",
            "url": "https://open.example/codes/P0420",
            "note": "Written for Wenmar Open in its own words, and general: it is not about one make. A code says which circuit or system reported a fault, not which part to replace. Diagnose before replacing anything.",
            "attribution": []
        })
    );
    // Either case, and the longer forms a scan tool prints.
    for path in ["/v1/codes/p0420", "/v1/codes/P0420:00", "/v1/codes/P042000"] {
        assert_eq!(ok(&app, path).await["code"], "P0420", "{path}");
    }
}

#[tokio::test]
async fn a_code_with_no_record_says_which_kind_of_none_it_is() {
    let app = common::codes_app().await;
    for (path, words) in [
        ("/v1/codes/zebra", "No trouble code is written that way"),
        ("/v1/codes/P1234", "specific to a manufacturer"),
        ("/v1/codes/B3000", "specific to a manufacturer"),
        ("/v1/codes/P0001", "Nobody has written a description"),
    ] {
        let body = refused(&app, path, StatusCode::NOT_FOUND, "not_found").await;
        let message = body["error"]["message"].as_str().unwrap();
        assert!(message.contains(words), "{path}: {message}");
    }
}

#[tokio::test]
async fn the_list_is_by_code_and_narrowed_by_system_and_text() {
    let app = common::codes_app().await;
    let body = ok(&app, "/v1/codes").await;
    assert_eq!(codes(&body), ["P0300", "P0420", "U0001"]);
    assert_eq!(body["as_of"], "2026-10-12");
    assert_eq!(body["next_cursor"], Value::Null);
    assert_eq!(
        body["codes"][0],
        json!({
            "code": "P0300",
            "system": "powertrain/ignition",
            "title": "Made-up title one",
            "url": "https://open.example/codes/P0300"
        })
    );
    for (query, expected) in [
        ("system=powertrain", vec!["P0300", "P0420"]),
        ("system=powertrain%2Femissions", vec!["P0420"]),
        ("system=NETWORK", vec!["U0001"]),
        ("system=body", vec![]),
        ("q=title+three", vec!["U0001"]),
        ("q=p04", vec!["P0420"]),
        ("q=&system=", vec!["P0300", "P0420", "U0001"]),
    ] {
        let body = ok(&app, &format!("/v1/codes?{query}")).await;
        assert_eq!(codes(&body), expected, "{query}");
    }
    let first = ok(&app, "/v1/codes?limit=2").await;
    assert_eq!(codes(&first), ["P0300", "P0420"]);
    assert_eq!(first["next_cursor"], "P0420");
    let second = ok(&app, "/v1/codes?limit=2&cursor=P0420").await;
    assert_eq!(codes(&second), ["U0001"]);
    assert_eq!(second["next_cursor"], Value::Null);
    for (query, field) in [("system=engine", "system"), ("cursor=zebra", "cursor")] {
        let body = refused(
            &app,
            &format!("/v1/codes?{query}"),
            StatusCode::BAD_REQUEST,
            "validation_failed",
        )
        .await;
        assert_eq!(body["error"]["details"]["field"], field, "{query}");
    }
}

#[tokio::test]
async fn a_code_on_a_vehicle_brings_its_bulletins_and_recalls() {
    let app = common::codes_app().await;
    let body = ok(&app, "/v1/vehicles/2023_hyundai_kona/codes/p0420").await;
    assert_eq!(
        body["vehicle"],
        json!({ "id": "2023_hyundai_kona", "year": 2023, "make": "Hyundai", "model": "Kona" })
    );
    assert_eq!(body["code"], "P0420");
    assert_eq!(body["generic"], true);
    // The description is the answer of the code's own route.
    assert_eq!(body["description"], ok(&app, "/v1/codes/P0420").await);
    // One bulletin of the 2023 Kona names the code; 11000006 names it too
    // but lists the 2022 Kona only.
    assert_eq!(body["bulletins"]["count"], 1);
    assert_eq!(body["bulletins"]["as_of"], "2026-10-11");
    assert_eq!(ids(&body["bulletins"]["bulletins"]), [11000001]);
    assert_eq!(
        body["bulletins"]["url"],
        "https://open.example/makes/hyundai/kona/2023/bulletins?code=P0420"
    );
    assert_eq!(
        body["bulletins"]["bulletins"][0],
        ok(&app, "/v1/vehicles/2023_hyundai_kona/bulletins?code=P0420").await["bulletins"][0]
    );
    // The record concerns the catalytic converter and the electrical
    // system. 11000003 is about the electrical system and does not name
    // the code; 11000001 is too, and is already listed above.
    assert_eq!(
        body["related"]["components"],
        body["description"]["components"]
    );
    assert_eq!(ids(&body["related"]["bulletins"]), [11000003]);
    let recalls = &body["related"]["recalls"];
    assert_eq!(recalls["may_apply"], true);
    let campaigns: Vec<&str> = recalls["recalls"]
        .as_array()
        .unwrap()
        .iter()
        .map(|recall| recall["campaign"].as_str().unwrap())
        .collect();
    assert_eq!(campaigns, ["2023101", "23V123000"]);
    assert_eq!(recalls["recalls"][1]["match"], "exact");
    assert_eq!(recalls["lookup"]["nhtsa"], "https://www.nhtsa.gov/recalls");
    assert_eq!(
        body["url"],
        "https://open.example/makes/hyundai/kona/2023/codes/P0420"
    );
    assert!(
        body["note"]
            .as_str()
            .unwrap()
            .contains("may apply to the model year")
    );
    assert_eq!(body["attribution"], json!([common::OGL_CANADA]));
}

#[tokio::test]
async fn a_code_with_no_record_still_answers_for_a_vehicle() {
    let app = common::codes_app().await;
    // A manufacturer-specific code a bulletin names.
    let body = ok(&app, "/v1/vehicles/2023_hyundai_kona/codes/C1234").await;
    assert_eq!(body["generic"], false);
    assert_eq!(body["description"], Value::Null);
    assert_eq!(body["bulletins"]["count"], 1);
    assert_eq!(ids(&body["bulletins"]["bulletins"]), [11000004]);
    assert_eq!(body["related"]["components"], json!([]));
    assert_eq!(body["related"]["bulletins"], json!([]));
    assert_eq!(body["related"]["recalls"]["recalls"], json!([]));
    assert_eq!(body["related"]["recalls"]["may_apply"], true);
    // A generic code nobody has described and no bulletin names.
    let body = ok(&app, "/v1/vehicles/2023_hyundai_kona/codes/P0001").await;
    assert_eq!(body["generic"], true);
    assert_eq!(body["description"], Value::Null);
    assert_eq!(body["bulletins"]["count"], 0);
    // A code whose record names a component nothing of this vehicle is about.
    let body = ok(&app, "/v1/vehicles/2019_chevrolet_silverado/codes/U0001").await;
    assert_eq!(body["description"]["title"], "Made-up title three");
    assert_eq!(body["bulletins"]["count"], 0);
    assert_eq!(body["related"]["recalls"]["recalls"], json!([]));
    for (path, words) in [
        (
            "/v1/vehicles/2023_hyundai_kona/codes/zebra",
            "No trouble code is written that way",
        ),
        (
            "/v1/vehicles/1899_nobody_nothing/codes/P0420",
            "No vehicle has that id",
        ),
    ] {
        let body = refused(&app, path, StatusCode::NOT_FOUND, "not_found").await;
        assert!(
            body["error"]["message"].as_str().unwrap().contains(words),
            "{path}: {body}"
        );
    }
}

#[tokio::test]
async fn each_part_is_null_without_its_dataset() {
    // As offline users have it: codes and recalls, no bulletins.
    let app = common::codes_offline_app_at("https://open.example").await;
    let body = ok(&app, "/v1/vehicles/2023_hyundai_kona/codes/P0420").await;
    assert_eq!(body["description"]["title"], "Made-up title two");
    assert_eq!(body["bulletins"], Value::Null);
    assert_eq!(body["related"]["bulletins"], Value::Null);
    assert_eq!(
        body["related"]["recalls"]["recalls"]
            .as_array()
            .unwrap()
            .len(),
        2
    );
    // Bulletins and recalls, no codes: the vehicle route still answers.
    let app = common::bulletins_app().await;
    let body = ok(&app, "/v1/vehicles/2023_hyundai_kona/codes/P0420").await;
    assert_eq!(body["description"], Value::Null);
    assert_eq!(body["bulletins"]["count"], 1);
    assert_eq!(body["related"]["bulletins"], json!([]));
    for path in ["/v1/codes/P0420", "/v1/codes"] {
        let body = refused(&app, path, StatusCode::NOT_FOUND, "dataset_unavailable").await;
        assert_eq!(
            body["error"]["message"],
            "The codes dataset is not available."
        );
    }
    // Recalls only, and no shop file at all: nothing to answer from.
    for app in [common::recalls_app().await, common::app().await] {
        for path in [
            "/v1/codes/P0420",
            "/v1/codes",
            "/v1/vehicles/2023_hyundai_kona/codes/P0420",
        ] {
            let body = refused(&app, path, StatusCode::NOT_FOUND, "dataset_unavailable").await;
            assert_eq!(
                body["error"]["message"],
                "The codes dataset is not available."
            );
        }
    }
}
