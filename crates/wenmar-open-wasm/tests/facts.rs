//! The operations for fuel, specifications, fluids and tire fitments, run
//! natively against the small data file and the small shop file through
//! `rusqlite`.

mod common;

use serde_json::{Value, json};

use common::{KONA, data, opened, opened_with_shop, run, run_with_shop, shop};

const OPS: [&str; 8] = [
    "fuel_vehicle",
    "fuel_vin",
    "specs_vehicle",
    "specs_vin",
    "fluids_vehicle",
    "fluids_vin",
    "tires_vehicle",
    "tires_vin",
];

fn count(answer: &Value, list: &str) -> usize {
    answer[list]
        .as_array()
        .unwrap_or_else(|| panic!("no {list} in {answer}"))
        .len()
}

#[test]
fn a_model_year_gets_what_is_held_and_a_vin_only_what_is_for_its_engine() {
    let (connection, shop) = (data(), shop());
    let mut engine = opened_with_shop(&connection, &shop);
    let mut ask = |op: &str, args: Value| {
        let asked = run_with_shop(&mut engine, &connection, &shop, op, args);
        assert!(asked.answer.get("ok").is_some(), "{op}: {}", asked.answer);
        // A handful of round trips: the decode of a VIN takes most of them.
        assert!(asked.steps <= 12, "{op}: {} steps", asked.steps);
        asked.answer["ok"].clone()
    };
    let kona = json!({ "id": "2023_hyundai_kona" });
    let by_vin = json!({ "vin": KONA });

    let fuel = ask("fuel_vehicle", kona.clone());
    assert_eq!(
        fuel["as_of"],
        json!({ "us": "2026-09-29", "ca": "2026-10-06" })
    );
    assert_eq!(count(&fuel, "ratings"), 4);
    assert_eq!(fuel["ratings"][0]["confidence"], "model");
    assert_eq!(fuel["ratings"][0]["engine"], Value::Null);
    assert_eq!(
        fuel["ratings"][2]["engine"],
        json!({ "id": "2-0l", "label": "2.0L", "vin8": "A" })
    );
    let fuel = ask("fuel_vin", by_vin.clone());
    assert_eq!(fuel["engine_settled"], true);
    assert_eq!(fuel["engine"]["id"], "2-0l");
    // The EPA rating and the Canadian one of the 2.0L; not the turbo's,
    // and not the one matched to the model year alone.
    assert_eq!(count(&fuel, "ratings"), 2);
    let canada = ask(
        "fuel_vehicle",
        json!({ "id": "2023_hyundai_kona", "country": "CA" }),
    );
    assert_eq!(count(&canada, "ratings"), 1);

    let specs = ask("specs_vehicle", kona.clone());
    assert_eq!(count(&specs, "specs"), 3);
    assert_eq!(count(&ask("specs_vin", by_vin.clone()), "specs"), 1);

    let fluids = ask("fluids_vehicle", kona.clone());
    assert_eq!(count(&fluids, "engine_oil"), 2);
    assert_eq!(
        fluids["engine_oil"][0]["source"],
        "EPA fuel economy certification"
    );
    assert_eq!(count(&fluids, "engine_oil_checked"), 2);
    assert_eq!(fluids["checked_as_of"], "2026-10-09");
    let fluids = ask("fluids_vin", by_vin.clone());
    assert_eq!(count(&fluids, "engine_oil"), 1);
    assert_eq!(count(&fluids, "engine_oil_checked"), 1);
    assert_eq!(
        fluids["engine_oil_checked"][0]["source"],
        "Test manual K, page 1"
    );
    // Recorded for every engine, so it stays.
    assert_eq!(count(&fluids, "brake"), 1);

    let tires = ask("tires_vehicle", kona.clone());
    assert_eq!(tires["fitments"][0]["size"], "996/96R96");
    assert_eq!(tires["fitments"][0]["trims"], json!(["SE"]));
    assert_eq!(
        tires["fitments"][0]["pressure_kpa"],
        json!({ "front": 990, "rear": 980 })
    );
    // Never narrowed.
    assert_eq!(count(&ask("tires_vin", by_vin), "fitments"), 1);

    // The overview carries the same blocks.
    let overview = ask("overview_vehicle", kona);
    assert_eq!(overview["tires"], tires["fitments"]);
    assert_eq!(overview["fluids"]["brake"].as_array().unwrap().len(), 1);
    assert_eq!(overview["fuel"].as_array().unwrap().len(), 4);
    assert_eq!(overview["specs"].as_array().unwrap().len(), 3);
}

#[test]
fn every_case_answers_or_fails_as_the_api_does() {
    let (connection, shop) = (data(), shop());
    let mut engine = opened_with_shop(&connection, &shop);
    for (name, op, args) in common::facts_cases() {
        let asked = run_with_shop(&mut engine, &connection, &shop, op, args);
        match asked.answer.get("error") {
            None => assert!(asked.answer["ok"].is_object(), "{name}"),
            Some(error) if name.contains("unknown vehicle") => {
                assert_eq!(error["code"], "not_found", "{name}");
            }
            Some(error) => {
                assert_eq!(error["code"], "validation_failed", "{name}");
                assert_eq!(error["details"], json!({ "field": "country" }), "{name}");
            }
        }
    }
}

#[test]
fn with_no_shop_file_each_is_dataset_unavailable_and_reads_nothing() {
    let connection = data();
    let mut engine = opened(&connection);
    for op in OPS {
        let args = json!({ "id": "2023_hyundai_kona", "vin": KONA });
        let asked = run(&mut engine, &connection, op, args);
        assert_eq!(asked.steps, 0, "{op}");
        let dataset = match op.split('_').next().unwrap() {
            "fuel" => "fuel_us",
            "specs" => "specs",
            "fluids" => "oil_grade",
            _ => "tires",
        };
        assert_eq!(
            asked.answer["error"],
            json!({
                "code": "dataset_unavailable",
                "message": format!("The {dataset} dataset is not available."),
                "details": { "dataset": dataset }
            }),
            "{op}"
        );
    }
    let asked = run(
        &mut engine,
        &connection,
        "fuel_vehicle",
        json!({ "id": "2023_hyundai_kona", "country": "ca" }),
    );
    assert_eq!(asked.answer["error"]["details"]["dataset"], "fuel_ca");
    // A country that is neither is refused before the data is looked for.
    let asked = run(
        &mut engine,
        &connection,
        "fuel_vin",
        json!({ "vin": KONA, "country": "mx" }),
    );
    assert_eq!(asked.answer["error"]["code"], "validation_failed");
    assert_eq!(asked.steps, 0);
}
