//! The fuel, specification and fluid routes, by vehicle id and by VIN.

use axum::http::StatusCode;
use serde_json::{Value, json};

use crate::common::{self, KONA, OGL_CANADA, TestApp};

async fn ok(app: &TestApp, path: &str) -> Value {
    let (status, body) = app.json(path).await;
    assert_eq!(status, StatusCode::OK, "{path}: {body}");
    body
}

/// One field of every entry of a list, in order.
fn each(body: &Value, list: &str, field: &str) -> Vec<Value> {
    body[list]
        .as_array()
        .unwrap_or_else(|| panic!("no {list} in {body}"))
        .iter()
        .map(|entry| entry[field].clone())
        .collect()
}

#[tokio::test]
async fn a_model_year_lists_its_ratings_with_how_far_each_was_matched() {
    let app = common::fuel_app().await;
    let body = ok(&app, "/v1/vehicles/2023_hyundai_kona/fuel").await;
    assert_eq!(
        body["vehicle"],
        json!({ "id": "2023_hyundai_kona", "year": 2023, "make": "Hyundai", "model": "Kona" })
    );
    assert_eq!(
        body["as_of"],
        json!({ "us": "2026-09-29", "ca": "2026-10-06" })
    );
    assert_eq!(body["attribution"], json!([OGL_CANADA]));
    assert_eq!(
        each(&body, "ratings", "source_id"),
        [
            json!("46004"),
            json!("46003"),
            json!("46002"),
            json!("46001"),
            json!(null)
        ]
    );
    assert_eq!(
        body["ratings"][3],
        json!({
            "source": "epa", "country": "us", "source_id": "46001",
            "match": "exact", "confidence": "engine",
            "engine": { "id": "2-0l", "label": "2.0L", "vin8": "A" },
            "described_as": "Kona FWD",
            "displacement_l": 2.0, "cylinders": 4,
            "transmission": "Automatic (variable gear ratios)", "drive": "Front-Wheel Drive",
            "fuel_grade": "Regular Gasoline", "second_fuel": null,
            "l_100km": { "city": 7.8, "highway": 6.7, "combined": 7.4 },
            "mpg": { "city": 30, "highway": 35, "combined": 32 },
            "kwh_100km": null, "range_km": null, "range_mi": null, "co2_g_km": 172
        })
    );
    // Matched to the model year only: no engine is named, and an electric
    // rating has neither litres nor miles per gallon.
    let electric = &body["ratings"][0];
    assert_eq!(electric["confidence"], "model");
    assert_eq!(electric["engine"], json!(null));
    assert_eq!(electric["l_100km"], json!(null));
    assert_eq!(electric["mpg"], json!(null));
    assert_eq!(electric["kwh_100km"], 17.4);
    assert_eq!(
        (&electric["range_km"], &electric["range_mi"]),
        (&json!(415), &json!(258))
    );

    // One country; a submodel in the id is set aside.
    let body = ok(&app, "/v1/vehicles/2023_hyundai_kona_se/fuel?country=CA").await;
    assert_eq!(each(&body, "ratings", "source"), [json!("nrcan")]);
    assert_eq!(body["ratings"][0]["engine"]["label"], "2.0L");
    assert_eq!(body["attribution"], json!([OGL_CANADA]));
    let body = ok(&app, "/v1/vehicles/2023_hyundai_kona/fuel?country=us").await;
    assert_eq!(body["ratings"].as_array().unwrap().len(), 4);
    assert_eq!(body["attribution"], json!([]));
    // A model year with none is an empty list, not an error.
    let body = ok(&app, "/v1/vehicles/2019_ford_f-150/fuel").await;
    assert_eq!(body["ratings"], json!([]));
}

#[tokio::test]
async fn a_vin_narrows_to_its_engine_only_when_the_decode_settles_it() {
    let app = common::fuel_app().await;
    let body = ok(&app, &format!("/v1/vin/{KONA}/fuel")).await;
    assert_eq!(body["decode"]["vin"], KONA);
    assert_eq!(body["vehicle"]["id"], "2023_hyundai_kona");
    assert_eq!(body["engine_settled"], true);
    assert_eq!(
        body["engine"],
        json!({ "id": "2-0l", "label": "2.0L", "vin8": "A" })
    );
    // The two EPA ratings and the Canadian one on the 2.0L. Not the
    // turbo's, and not the one matched to the model year alone.
    assert_eq!(
        each(&body, "ratings", "source_id"),
        [json!("46002"), json!("46001"), json!(null)]
    );
    assert!(
        each(&body, "ratings", "confidence")
            .iter()
            .all(|found| found == "engine")
    );
    assert_eq!(body["warnings"], json!([]));

    let fluids = ok(&app, &format!("/v1/vin/{KONA}/fluids")).await;
    assert_eq!(fluids["engine_settled"], true);
    assert_eq!(
        fluids["engine_oil"],
        json!([{
            "confidence": "engine",
            "engine": { "id": "2-0l", "label": "2.0L", "vin8": "A" },
            "displacement_l": 2.0, "cylinders": 4,
            "grades": ["0W-20"], "as_written": ["0W-20", "SAE 0W-20"],
            "source": "EPA fuel economy certification"
        }])
    );
    let specs = ok(&app, &format!("/v1/vin/{KONA}/specs")).await;
    assert_eq!(
        each(&specs, "specs", "source_id"),
        [json!("46002"), json!("46001")]
    );

    // A VIN that decodes to no vehicle in the catalog: empty, with a warning.
    let body = ok(&app, &format!("/v1/vin/{}/fuel", common::COACH)).await;
    assert_eq!(body["vehicle"], json!(null));
    assert_eq!(body["engine_settled"], false);
    assert_eq!(body["ratings"], json!([]));
    assert_eq!(
        body["warnings"].as_array().unwrap().last().unwrap()["code"],
        "no_catalog_vehicle"
    );
}

#[tokio::test]
async fn specs_and_fluids_are_listed_by_engine() {
    let app = common::fuel_app().await;
    let specs = ok(&app, "/v1/vehicles/2023_hyundai_kona/specs").await;
    assert_eq!(specs["as_of"], "2026-09-29");
    assert_eq!(specs["attribution"], json!([]));
    assert_eq!(
        each(&specs, "specs", "source_id"),
        [
            json!("46004"),
            json!("46003"),
            json!("46002"),
            json!("46001")
        ]
    );
    assert_eq!(
        specs["specs"][3],
        json!({
            "source": "epa", "source_id": "46001", "confidence": "engine",
            "engine": { "id": "2-0l", "label": "2.0L", "vin8": "A" },
            "described_as": "Kona FWD",
            "transmission": "Automatic (variable gear ratios)",
            "transmission_type": "Continuously Variable", "gears": 1,
            "drive": "Front-Wheel Drive",
            "fuel_grade": "Regular Gasoline",
            "fuel_usage": "Gasoline (Regular Unleaded Recommended)",
            "size_class": "Small Sport Utility Vehicle 2WD",
            "turbo": false, "supercharger": false, "start_stop": true,
            "electrification": null, "ev_motor": null,
            "cylinder_deactivation": false, "cylinder_deactivation_name": null,
            "variable_valve_timing": true, "variable_valve_lift": false,
            "intake_valves": 2, "exhaust_valves": 2,
            "passenger_volume_l": 2662, "passenger_volume_ft3": 94,
            "luggage_volume_l": 538, "luggage_volume_ft3": 19,
            "battery_type": null, "battery_voltage": null, "motor_kw": null,
            "emission_standards": [
                { "sales_area": 3, "standard": "T3B30", "standard_text": "Federal Tier 3 Bin 30", "smog_score": 7, "engine_family": "PHYXV02.0AAA" },
                { "sales_area": 7, "standard": "L3SULEV30", "standard_text": "California LEV-III SULEV30", "smog_score": 7, "engine_family": "PHYXV02.0AAA" }
            ]
        })
    );
    assert_eq!(
        specs["specs"][1]["cylinder_deactivation_name"],
        "Half Engine Mode"
    );
    assert_eq!(specs["specs"][2]["emission_standards"], json!([]));

    let fluids = ok(&app, "/v1/vehicles/2023_hyundai_kona/fluids").await;
    assert_eq!(fluids["as_of"], "2026-09-29");
    assert_eq!(
        each(&fluids, "engine_oil", "grades"),
        [json!(["0W-20"]), json!(["5W-30", "0W-30"])]
    );
    assert_eq!(fluids["engine_oil"][1]["engine"]["label"], "1.6L Turbo");
    // No engine settled for the Civic's rating: the grade is given for
    // the engine size the source names, marked `model`.
    let fluids = ok(&app, "/v1/vehicles/2019_honda_civic/fluids").await;
    assert_eq!(
        fluids["engine_oil"],
        json!([{
            "confidence": "model", "engine": null, "displacement_l": 1.8, "cylinders": 4,
            "grades": ["0W-20"], "as_written": ["0W20"],
            "source": "EPA fuel economy certification"
        }])
    );
}

#[tokio::test]
async fn what_the_server_does_not_hold_is_dataset_unavailable() {
    let app = common::app_with_shop(common::FUEL_DATASETS_US, &common::fuel_rows()).await;
    let body = ok(&app, "/v1/vehicles/2023_hyundai_kona/fuel").await;
    assert_eq!(body["as_of"], json!({ "us": "2026-09-29", "ca": null }));
    assert_eq!(body["ratings"].as_array().unwrap().len(), 4);
    for (path, dataset) in [
        ("/v1/vehicles/2023_hyundai_kona/fuel?country=ca", "fuel_ca"),
        ("/v1/vehicles/2023_hyundai_kona/specs", "specs"),
        ("/v1/vehicles/2023_hyundai_kona/fluids", "oil_grade"),
        ("/v1/vin/KM8K2CAB4PU001140/specs", "specs"),
        ("/v1/vin/KM8K2CAB4PU001140/fluids", "oil_grade"),
    ] {
        let (status, body) = app.json(path).await;
        assert_eq!(status, StatusCode::NOT_FOUND, "{path}");
        assert_eq!(body["error"]["code"], "dataset_unavailable", "{path}");
        assert_eq!(body["error"]["details"]["dataset"], dataset, "{path}");
    }
    // No shop file at all.
    let app = common::app().await;
    let (status, body) = app.json("/v1/vehicles/2023_hyundai_kona/fuel").await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    assert_eq!(body["error"]["details"]["dataset"], "fuel_us");
}

#[tokio::test]
async fn bad_requests_are_named() {
    let app = common::fuel_app().await;
    for (path, status, code) in [
        (
            "/v1/vehicles/2023_hyundai_nothing/fuel",
            StatusCode::NOT_FOUND,
            "not_found",
        ),
        (
            "/v1/vehicles/2023_hyundai_nothing/specs",
            StatusCode::NOT_FOUND,
            "not_found",
        ),
        (
            "/v1/vehicles/2023_hyundai_nothing/fluids",
            StatusCode::NOT_FOUND,
            "not_found",
        ),
        (
            "/v1/vehicles/2023_hyundai_kona/fuel?country=mx",
            StatusCode::BAD_REQUEST,
            "validation_failed",
        ),
        (
            "/v1/vin/KM8K2CAB4PUO01140/fuel",
            StatusCode::BAD_REQUEST,
            "invalid_vin",
        ),
        (
            "/v1/vin/KM8K2CAB4PU001140/fluids?year=abc",
            StatusCode::BAD_REQUEST,
            "validation_failed",
        ),
        (
            "/v1/vin/ZZZK2CAB4PU001140/specs",
            StatusCode::NOT_FOUND,
            "not_found",
        ),
    ] {
        let (found, body) = app.json(path).await;
        assert_eq!(found, status, "{path}: {body}");
        assert_eq!(body["error"]["code"], code, "{path}");
    }
}
