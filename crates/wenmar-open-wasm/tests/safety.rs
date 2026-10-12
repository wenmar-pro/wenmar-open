//! The `safety` operation, run natively against the small data file and
//! the small shop file through `rusqlite`.

mod common;

use serde_json::{Value, json};

use common::{data, opened, opened_with_shop, run, run_with_shop, shop};

#[test]
fn a_model_year_gets_every_rating_nhtsa_gave_it() {
    let (connection, shop) = (data(), shop());
    let mut engine = opened_with_shop(&connection, &shop);
    let asked = run_with_shop(
        &mut engine,
        &connection,
        &shop,
        "safety",
        json!({ "id": "2023_hyundai_kona" }),
    );
    let answer = &asked.answer["ok"];
    assert_eq!(
        answer["vehicle"],
        json!({ "id": "2023_hyundai_kona", "year": 2023, "make": "Hyundai", "model": "Kona" })
    );
    assert_eq!(answer["as_of"], "2026-08-07");
    assert_eq!(answer["source"], "NHTSA New Car Assessment Program");
    assert_eq!(answer["source_url"], "https://www.nhtsa.gov/ratings");
    assert_eq!(answer["attribution"], json!([]));
    let described: Vec<&str> = answer["ratings"]
        .as_array()
        .unwrap()
        .iter()
        .map(|rating| rating["description"].as_str().unwrap())
        .collect();
    assert_eq!(
        described,
        [
            "KONA, SUV, AWD",
            "KONA, SUV, FWD",
            "KONA ELECTRIC, SUV, FWD"
        ]
    );
    let first = &answer["ratings"][0];
    assert_eq!(first["match"], "exact");
    assert_eq!(first["stars"]["overall"], 5);
    assert_eq!(first["stars"]["rollover"], 4);
    assert_eq!(first["rollover_risk_percent"], 14.8);
    assert_eq!(first["rollover_tip"], false);
    assert_eq!(first["curb_weight_kg"], 1463);
    assert_eq!(first["curb_weight_lb"], 3225);
    assert_eq!(first["features"]["esc"], Value::Null);
    assert_eq!(
        first["features"]["crash_imminent_braking_evaluation"],
        "does_not_meet"
    );
    assert_eq!(
        answer["ratings"][1]["notes"],
        json!([
            "These ratings do not apply to the N Line.",
            "The rear door opened in the side test."
        ])
    );
    // A vehicle NHTSA lists and has not tested: every star is null.
    for (name, stars) in answer["ratings"][2]["stars"].as_object().unwrap() {
        assert!(stars.is_null(), "{name}");
    }
    // The ratings are in the shop file: one statement, marked for it, and
    // within the limits of Cloudflare D1.
    let of_shop: Vec<&Value> = asked
        .statements
        .iter()
        .filter(|statement| statement.get("db").and_then(Value::as_str) == Some("shop"))
        .collect();
    assert_eq!(of_shop.len(), 1, "{of_shop:?}");
    let sql = of_shop[0]["sql"].as_str().unwrap();
    assert!(sql.contains("FROM rating_vehicle"), "{sql}");
    assert!(sql.trim_start().starts_with("SELECT"), "{sql}");
    assert_eq!(of_shop[0]["params"], json!([498, 900, 2023]));
    assert!(asked.steps <= 4, "{} steps", asked.steps);

    // Every case answers, or fails as the API does.
    for (name, op, args) in common::safety_cases() {
        let asked = run_with_shop(&mut engine, &connection, &shop, op, args);
        match asked.answer.get("error") {
            None => assert!(asked.answer["ok"]["ratings"].is_array(), "{name}"),
            Some(error) => assert_eq!(error["code"], "not_found", "{name}"),
        }
    }
    let none = run_with_shop(
        &mut engine,
        &connection,
        &shop,
        "safety",
        json!({ "id": "2019_honda_civic" }),
    );
    assert_eq!(none.answer["ok"]["ratings"], json!([]));
}

#[test]
fn with_no_shop_file_safety_is_dataset_unavailable_and_reads_nothing() {
    let connection = data();
    let mut engine = opened(&connection);
    for id in ["2023_hyundai_kona", "2019_honda_nothing", ""] {
        let asked = run(&mut engine, &connection, "safety", json!({ "id": id }));
        assert_eq!(asked.steps, 0, "{id}");
        assert_eq!(
            asked.answer["error"],
            json!({
                "code": "dataset_unavailable",
                "message": "The ratings dataset is not available.",
                "details": { "dataset": "ratings" }
            }),
            "{id}"
        );
    }
}
