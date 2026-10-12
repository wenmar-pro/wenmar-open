//! The facts read by hand on the API: the fluids beside EPA's oil grade,
//! the tire fitments, and both in the overview.

use axum::http::StatusCode;
use serde_json::{Value, json};

use crate::common::{self, KONA, TestApp};

async fn ok(app: &TestApp, path: &str) -> Value {
    let (status, body) = app.json(path).await;
    assert_eq!(status, StatusCode::OK, "{path}: {body}");
    body
}

/// One field of every entry of a list, in order.
fn each(list: &Value, field: &str) -> Vec<Value> {
    list.as_array()
        .unwrap_or_else(|| panic!("not a list: {list}"))
        .iter()
        .map(|entry| entry[field].clone())
        .collect()
}

const HAND_LISTS: [&str; 5] = [
    "engine_oil_checked",
    "transmission",
    "coolant",
    "refrigerant",
    "brake",
];

#[tokio::test]
async fn a_model_year_lists_its_hand_read_fluids_each_with_its_source() {
    let app = common::hand_app().await;
    let body = ok(&app, "/v1/vehicles/2001_testmake_sampler/fluids").await;
    assert_eq!(body["vehicle"]["id"], "2001_testmake_sampler");
    // EPA has nothing for a vehicle that does not exist.
    assert_eq!(body["as_of"], "2026-09-29");
    assert_eq!(body["engine_oil"], json!([]));
    assert_eq!(body["checked_as_of"], "2026-10-12");
    assert_eq!(body["attribution"], json!([]));
    assert_eq!(
        body["engine_oil_checked"],
        json!([
            {
                "engine": { "id": "9-9l-test", "label": "9.9L Test", "vin8": null },
                "applies_to": null, "grade": "0W-99", "specification": "TEST-SPEC-1",
                "capacity_l": 99.9, "capacity_qt": 105.6, "capacity_written_in": "l",
                "with_filter": true,
                "charge_g": null, "charge_oz": null, "charge_written_in": null,
                "market": null, "source": "Test manual, page 1", "checked": "2026-10-12"
            },
            {
                "engine": { "id": "8-8l-test-diesel", "label": "8.8L Test Diesel", "vin8": null },
                "applies_to": null, "grade": null, "specification": "TEST-SPEC-2",
                "capacity_l": 83.3, "capacity_qt": 88.0, "capacity_written_in": "qt",
                "with_filter": false,
                "charge_g": null, "charge_oz": null, "charge_written_in": null,
                "market": "ca", "source": "Test manual, page 2", "checked": "2026-10-11"
            }
        ])
    );
    assert_eq!(
        each(&body["transmission"], "applies_to"),
        [json!("Test automatic")]
    );
    assert_eq!(body["transmission"][0]["engine"], json!(null));
    assert_eq!(body["coolant"][0]["engine"]["id"], "9-9l-test");
    assert_eq!(
        body["refrigerant"],
        json!([{
            "engine": null, "applies_to": null, "grade": null, "specification": "R-TEST",
            "capacity_l": null, "capacity_qt": null, "capacity_written_in": null,
            "with_filter": null,
            "charge_g": 9990, "charge_oz": 352.4, "charge_written_in": "kg",
            "market": null, "source": "Test under-hood label", "checked": "2026-10-10"
        }])
    );
    assert_eq!(
        each(&body["brake"], "specification"),
        [json!("TEST BRAKE 9")]
    );
    // Every record carries where it was read and when; none names a reviewer.
    for list in HAND_LISTS {
        for record in body[list].as_array().unwrap() {
            assert!(
                record["source"]
                    .as_str()
                    .is_some_and(|text| !text.is_empty())
            );
            assert_eq!(record["checked"].as_str().unwrap().len(), 10);
            assert!(record.get("reviewer").is_none());
        }
    }
}

#[tokio::test]
async fn the_epa_grade_is_kept_as_it_was_beside_the_records() {
    let app = common::hand_app().await;
    let body = ok(&app, "/v1/vehicles/2023_hyundai_kona/fluids").await;
    // What the route answered before the records existed.
    let before = ok(
        &common::fuel_app().await,
        "/v1/vehicles/2023_hyundai_kona/fluids",
    )
    .await;
    for field in ["vehicle", "as_of", "engine_oil", "attribution"] {
        assert_eq!(body[field], before[field], "{field}");
    }
    assert_eq!(
        each(&body["engine_oil_checked"], "specification"),
        [json!("TEST-SPEC-K"), json!("TEST-SPEC-T")]
    );
    assert_eq!(body["brake"].as_array().unwrap().len(), 1);
    // A server with the grades and no records: the lists are empty and
    // the day is null.
    assert_eq!(before["checked_as_of"], json!(null));
    for list in HAND_LISTS {
        assert_eq!(before[list], json!([]), "{list}");
    }
    // A model year with no record has empty lists, not an error.
    let body = ok(&app, "/v1/vehicles/2019_honda_civic/fluids").await;
    for list in HAND_LISTS {
        assert_eq!(body[list], json!([]), "{list}");
    }
    assert_eq!(body["engine_oil"].as_array().unwrap().len(), 1);
}

#[tokio::test]
async fn a_vin_narrows_the_records_to_its_engine_and_keeps_those_for_every_engine() {
    let app = common::hand_app().await;
    let body = ok(&app, &format!("/v1/vin/{KONA}/fluids")).await;
    assert_eq!(body["engine_settled"], true);
    assert_eq!(body["engine"]["id"], "2-0l");
    assert_eq!(
        each(&body["engine_oil_checked"], "specification"),
        [json!("TEST-SPEC-K")]
    );
    assert_eq!(body["engine_oil_checked"][0]["engine"]["id"], "2-0l");
    // Recorded for every engine, so it stays.
    assert_eq!(
        each(&body["brake"], "source"),
        [json!("Test manual K, page 3")]
    );
    assert_eq!(body["engine_oil"].as_array().unwrap().len(), 1);
    assert_eq!(body["checked_as_of"], "2026-10-12");
}

#[tokio::test]
async fn the_fluids_routes_answer_with_either_dataset_and_refuse_with_neither() {
    // The records alone: no EPA grade and no day for it.
    let app = common::hand_alone_app().await;
    let body = ok(&app, "/v1/vehicles/2001_testmake_sampler/fluids").await;
    assert_eq!(body["as_of"], json!(null));
    assert_eq!(body["engine_oil"], json!([]));
    assert_eq!(body["engine_oil_checked"].as_array().unwrap().len(), 2);
    let body = ok(&app, &format!("/v1/vin/{KONA}/fluids")).await;
    assert_eq!(body["engine_oil_checked"].as_array().unwrap().len(), 1);
    // Neither: the error the route always gave.
    let app = common::recalls_app().await;
    for path in [
        "/v1/vehicles/2023_hyundai_kona/fluids".to_owned(),
        format!("/v1/vin/{KONA}/fluids"),
    ] {
        let (status, body) = app.json(&path).await;
        assert_eq!(status, StatusCode::NOT_FOUND, "{path}");
        assert_eq!(body["error"]["code"], "dataset_unavailable");
        assert_eq!(
            body["error"]["message"],
            "The oil_grade dataset is not available."
        );
    }
}

#[tokio::test]
async fn a_model_year_lists_every_tire_fitment_and_picks_none() {
    let app = common::hand_app().await;
    let body = ok(&app, "/v1/vehicles/2001_testmake_sampler/tires").await;
    assert_eq!(
        body,
        json!({
            "vehicle": { "id": "2001_testmake_sampler", "year": 2001, "make": "Testmake", "model": "Sampler" },
            "as_of": "2026-10-11",
            "fitments": [
                {
                    "size": "999/99R99", "size_rear": null, "trims": ["Base", "Deluxe"],
                    "pressure_kpa": { "front": 990, "rear": 980 },
                    "pressure_psi": { "front": 144, "rear": 142 },
                    "pressure_written_in": "kpa",
                    "lug_torque_nm": 999, "lug_torque_lbft": 737, "lug_torque_written_in": "nm",
                    "bolt_pattern": "9x999", "market": null,
                    "source": "Test door placard", "checked": "2026-10-11"
                },
                {
                    "size": "998/98R98", "size_rear": "997/97R97", "trims": [],
                    "pressure_kpa": { "front": 990, "rear": 990 },
                    "pressure_psi": { "front": 144, "rear": 144 },
                    "pressure_written_in": "psi",
                    "lug_torque_nm": null, "lug_torque_lbft": null, "lug_torque_written_in": null,
                    "bolt_pattern": null, "market": "us",
                    "source": "Test manual, page 6", "checked": "2026-10-10"
                }
            ],
            "attribution": []
        })
    );
    // A submodel or an engine in the id is set aside.
    let body = ok(&app, "/v1/vehicles/2001_testmake_sampler__9-9l-test/tires").await;
    assert_eq!(body["fitments"].as_array().unwrap().len(), 2);
    // A model year with no record is an empty list.
    let body = ok(&app, "/v1/vehicles/2019_honda_civic/tires").await;
    assert_eq!(body["fitments"], json!([]));
    assert_eq!(body["as_of"], "2026-10-11");
}

#[tokio::test]
async fn a_vin_gets_every_fitment_of_its_model_year() {
    let app = common::hand_app().await;
    let body = ok(&app, &format!("/v1/vin/{KONA}/tires")).await;
    assert_eq!(body["decode"]["vin"], KONA);
    assert_eq!(body["vehicle"]["id"], "2023_hyundai_kona");
    assert_eq!(each(&body["fitments"], "size"), [json!("996/96R96")]);
    assert_eq!(body["fitments"][0]["pressure_kpa"], json!(null));
    assert_eq!(body["warnings"], json!([]));
    // A VIN that reaches no model year: no vehicle, an empty list, a warning.
    let body = ok(&app, &format!("/v1/vin/{}/tires", common::COACH)).await;
    assert_eq!(body["vehicle"], json!(null));
    assert_eq!(body["fitments"], json!([]));
    assert!(
        each(&body["warnings"], "code").contains(&json!("no_catalog_vehicle")),
        "{body}"
    );
}

#[tokio::test]
async fn the_tire_routes_refuse_what_they_cannot_answer() {
    let app = common::hand_app().await;
    let (status, body) = app.json("/v1/vehicles/2001_nomake_nomodel/tires").await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    assert_eq!(body["error"]["code"], "not_found");
    let (status, body) = app.json("/v1/vin/NOTAVIN/tires").await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert_eq!(body["error"]["code"], "invalid_vin");
    // A server with fuel data and no fitments.
    let app = common::fuel_app().await;
    for path in [
        "/v1/vehicles/2023_hyundai_kona/tires".to_owned(),
        format!("/v1/vin/{KONA}/tires"),
    ] {
        let (status, body) = app.json(&path).await;
        assert_eq!(status, StatusCode::NOT_FOUND, "{path}");
        assert_eq!(body["error"]["code"], "dataset_unavailable");
        assert_eq!(
            body["error"]["message"],
            "The tires dataset is not available."
        );
    }
}

#[tokio::test]
async fn the_overview_carries_the_records_and_the_fitments() {
    let app = common::hand_app().await;
    let body = ok(&app, "/v1/vehicles/2001_testmake_sampler/overview").await;
    let fluids = ok(&app, "/v1/vehicles/2001_testmake_sampler/fluids").await;
    let tires = ok(&app, "/v1/vehicles/2001_testmake_sampler/tires").await;
    assert_eq!(body["fluids"]["engine_oil"], json!([]));
    for list in HAND_LISTS {
        assert_eq!(body["fluids"][list], fluids[list], "{list}");
    }
    assert_eq!(body["tires"], tires["fitments"]);

    // By VIN: the records of the settled engine and those for every engine.
    let body = ok(&app, &format!("/v1/vin/{KONA}/overview")).await;
    assert_eq!(
        each(&body["fluids"]["engine_oil_checked"], "specification"),
        [json!("TEST-SPEC-K")]
    );
    assert_eq!(body["fluids"]["brake"].as_array().unwrap().len(), 1);
    assert_eq!(body["fluids"]["engine_oil"].as_array().unwrap().len(), 1);
    assert_eq!(each(&body["tires"], "size"), [json!("996/96R96")]);

    // Without the datasets: tires is null, and fluids keeps EPA's grades
    // with the record lists empty.
    let body = ok(
        &common::fuel_app().await,
        "/v1/vehicles/2023_hyundai_kona/overview",
    )
    .await;
    assert_eq!(body["tires"], json!(null));
    assert_eq!(body["fluids"]["engine_oil"].as_array().unwrap().len(), 2);
    assert_eq!(body["fluids"]["engine_oil_checked"], json!([]));
    // With the records alone, the fluids block is there without EPA's.
    let body = ok(
        &common::hand_alone_app().await,
        "/v1/vehicles/2001_testmake_sampler/overview",
    )
    .await;
    assert_eq!(body["fluids"]["engine_oil"], json!([]));
    assert_eq!(body["fluids"]["brake"].as_array().unwrap().len(), 1);
    assert_eq!(body["fuel"], json!(null));
}
