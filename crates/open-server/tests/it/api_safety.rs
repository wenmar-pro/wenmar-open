//! `GET /v1/vehicles/{id}/safety`: NHTSA's crash test ratings for a model
//! year.

use axum::http::StatusCode;
use serde_json::{Value, json};

use crate::common::{self, TestApp};

async fn ok(app: &TestApp, path: &str) -> Value {
    let (status, body) = app.json(path).await;
    assert_eq!(status, StatusCode::OK, "{path}: {body}");
    body
}

/// The descriptions of an answer's ratings, in order.
fn described(body: &Value) -> Vec<String> {
    body["ratings"]
        .as_array()
        .unwrap_or_else(|| panic!("no ratings in {body}"))
        .iter()
        .map(|rating| rating["description"].as_str().unwrap().to_owned())
        .collect()
}

#[tokio::test]
async fn a_model_year_lists_every_rating_nhtsa_gave_it() {
    let app = common::ratings_app().await;
    let body = ok(&app, "/v1/vehicles/2023_hyundai_kona/safety").await;
    assert_eq!(
        body["vehicle"],
        json!({ "id": "2023_hyundai_kona", "year": 2023, "make": "Hyundai", "model": "Kona" })
    );
    assert_eq!(body["as_of"], "2026-08-07");
    assert_eq!(body["source"], "NHTSA New Car Assessment Program");
    assert_eq!(body["source_url"], "https://www.nhtsa.gov/ratings");
    assert_eq!(body["attribution"], json!([]));
    // By NHTSA's model name, then body style, then drive. None is picked.
    assert_eq!(
        described(&body),
        [
            "KONA, SUV, AWD",
            "KONA, SUV, FWD",
            "KONA ELECTRIC, SUV, FWD"
        ]
    );
    assert_eq!(
        body["ratings"][0],
        json!({
            "description": "KONA, SUV, AWD",
            "model": "KONA",
            "body_style": "SUV",
            "vehicle_type": "MPV",
            "drive": "AWD",
            "release": 1,
            "match": "exact",
            "stars": {
                "overall": 5,
                "frontal": 4,
                "frontal_driver": 4,
                "frontal_passenger": 4,
                "side": 5,
                "side_driver": 5,
                "side_passenger": 5,
                "side_barrier": 5,
                "side_front_seat": 5,
                "side_rear_seat": 5,
                "side_pole": 5,
                "rollover": 4
            },
            "rollover_risk_percent": 14.8,
            "rollover_tip": false,
            "curb_weight_kg": 1463,
            "curb_weight_lb": 3225,
            "features": {
                "esc": null,
                "forward_collision_warning": "standard",
                "forward_collision_warning_evaluation": "meets",
                "lane_departure_warning": "standard",
                "lane_departure_warning_evaluation": "meets",
                "crash_imminent_braking": "standard",
                "crash_imminent_braking_evaluation": "does_not_meet",
                "dynamic_brake_support": "standard",
                "dynamic_brake_support_evaluation": "pending",
                "backup_camera": null,
                "blind_spot_detection": "optional",
                "head_side_airbag": "standard",
                "torso_side_airbag": "standard"
            },
            "notes": []
        })
    );
    let front = &body["ratings"][1];
    assert_eq!(front["stars"]["overall"], 4);
    assert_eq!(front["curb_weight_kg"], 1363);
    assert_eq!(front["features"]["crash_imminent_braking"], "both");
    assert_eq!(
        front["notes"],
        json!([
            "These ratings do not apply to the N Line.",
            "The rear door opened in the side test."
        ])
    );
    // A vehicle NHTSA lists and has not tested: every star is null.
    let electric = &body["ratings"][2];
    assert_eq!(electric["match"], "base");
    for (name, stars) in electric["stars"].as_object().unwrap() {
        assert!(stars.is_null(), "{name}");
    }
    assert!(electric["rollover_risk_percent"].is_null());
    assert!(electric["rollover_tip"].is_null());
    assert!(electric["curb_weight_kg"].is_null());
    assert!(electric["curb_weight_lb"].is_null());

    // A submodel or an engine in the id is set aside.
    let trim = ok(&app, "/v1/vehicles/2023_hyundai_kona_se/safety").await;
    assert_eq!(trim, body);

    // A later release says so, and a truck is described by its cab.
    let earlier = ok(&app, "/v1/vehicles/2022_hyundai_kona/safety").await;
    assert_eq!(described(&earlier), ["KONA, SUV, FWD, later release"]);
    assert_eq!(earlier["ratings"][0]["release"], 2);
    let truck = ok(&app, "/v1/vehicles/2019_ford_f-150/safety").await;
    assert_eq!(described(&truck), ["F-150 SUPER CREW, PU/CC, 4x4"]);
    assert_eq!(truck["ratings"][0]["match"], "base");
    assert_eq!(truck["ratings"][0]["rollover_risk_percent"], 19.1);
    assert_eq!(truck["ratings"][0]["curb_weight_kg"], 2295);
    assert!(truck["ratings"][0]["features"]["forward_collision_warning_evaluation"].is_null());
}

#[tokio::test]
async fn a_model_year_nhtsa_lists_nothing_for_has_an_empty_list() {
    let app = common::ratings_app().await;
    let body = ok(&app, "/v1/vehicles/2019_honda_civic/safety").await;
    assert_eq!(body["vehicle"]["id"], "2019_honda_civic");
    assert_eq!(body["ratings"], json!([]));
    assert_eq!(body["as_of"], "2026-08-07");
    // A trailer is a vehicle of the catalog too.
    let body = ok(&app, "/v1/vehicles/2019_ranger-trailers_tilt-deck/safety").await;
    assert_eq!(body["ratings"], json!([]));
}

#[tokio::test]
async fn an_id_no_vehicle_has_is_not_found() {
    let app = common::ratings_app().await;
    for path in [
        "/v1/vehicles/2019_honda_nothing/safety",
        "/v1/vehicles/1999_toyota_corolla/safety",
        "/v1/vehicles/nonsense/safety",
        "/v1/vehicles/2023_hyundai_kona_nothing/safety",
    ] {
        let (status, body) = app.json(path).await;
        assert_eq!(status, StatusCode::NOT_FOUND, "{path}");
        assert_eq!(
            body,
            json!({ "error": { "code": "not_found", "message": "No vehicle has that id.", "details": {} } }),
            "{path}"
        );
    }
}

#[tokio::test]
async fn a_server_without_ratings_says_the_dataset_is_unavailable() {
    // No shop file, a shop file with recalls only, and one with nothing.
    let none = common::app().await;
    let recalls = common::recalls_app().await;
    let empty = common::app_with_shop("{}", "").await;
    for app in [&none, &recalls, &empty] {
        // Whether or not the id names a vehicle: the dataset comes first.
        for path in [
            "/v1/vehicles/2023_hyundai_kona/safety",
            "/v1/vehicles/nonsense/safety",
        ] {
            let (status, body) = app.json(path).await;
            assert_eq!(status, StatusCode::NOT_FOUND, "{path}");
            assert_eq!(
                body,
                json!({ "error": {
                    "code": "dataset_unavailable",
                    "message": "The ratings dataset is not available.",
                    "details": { "dataset": "ratings" }
                } }),
                "{path}"
            );
        }
    }
    // The routes beside it are as they were.
    let (status, _) = none.json("/v1/vehicles/2023_hyundai_kona").await;
    assert_eq!(status, StatusCode::OK);
    let (status, _) = recalls.json("/v1/vehicles/2023_hyundai_kona/recalls").await;
    assert_eq!(status, StatusCode::OK);
}

#[tokio::test]
async fn text_from_the_file_is_given_as_it_is() {
    // JSON carries the words as data; the page test holds that they are
    // never markup.
    let app = common::ratings_app().await;
    let body = ok(&app, "/v1/vehicles/2019_b-b-script_model-img/safety").await;
    assert_eq!(
        body["ratings"][0]["description"],
        "Model \"<img src=x onerror=alert(1)>, <b>Bold</b>"
    );
    assert_eq!(body["ratings"][0]["rollover_tip"], true);
    assert_eq!(body["ratings"][0]["stars"]["frontal_driver"], 4);
    assert!(body["ratings"][0]["stars"]["frontal"].is_null());
}
