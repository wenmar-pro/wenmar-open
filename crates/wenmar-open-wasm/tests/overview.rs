//! The overview operations, run natively against the small data file,
//! with and without the small shop file, through `rusqlite`.

mod common;

use serde_json::{Value, json};

use common::{KONA, data, opened, opened_with_shop, run, run_with_shop, shop};

const SITE: &str = "https://open.wenmarpro.com";

/// Every key an overview has.
const KEYS: [&str; 13] = [
    "vehicle",
    "decode",
    "specs",
    "fuel",
    "fluids",
    "tires",
    "recalls",
    "safety",
    "labor",
    "bulletins",
    "links",
    "attribution",
    "warnings",
];

#[test]
fn with_no_shop_file_an_overview_answers_and_its_recalls_are_null() {
    let connection = data();
    let mut engine = opened(&connection);
    let asked = run(
        &mut engine,
        &connection,
        "overview_vehicle",
        json!({ "id": "2023_hyundai_kona" }),
    );
    let answer = &asked.answer["ok"];
    let object = answer.as_object().unwrap();
    for key in KEYS {
        assert!(object.contains_key(key), "no {key}");
    }
    assert_eq!(object.len(), KEYS.len());
    assert_eq!(
        answer["vehicle"],
        json!({ "id": "2023_hyundai_kona", "year": 2023, "make": "Hyundai", "model": "Kona" })
    );
    assert_eq!(answer["decode"], Value::Null);
    assert_eq!(answer["recalls"], Value::Null);
    assert_eq!(answer["safety"], Value::Null);
    assert_eq!(
        answer["links"],
        json!({ "page": format!("{SITE}/makes/hyundai/kona/2023"), "maker": null })
    );
    assert_eq!(answer["attribution"], json!([]));
    // Nothing was asked of a shop file.
    assert!(
        asked
            .statements
            .iter()
            .all(|statement| statement.get("db").is_none())
    );

    let asked = run(
        &mut engine,
        &connection,
        "overview_vin",
        json!({ "vin": KONA }),
    );
    let answer = &asked.answer["ok"];
    assert_eq!(answer["decode"]["vin"], KONA);
    assert_eq!(answer["decode"]["entry_id"], "2023_hyundai_kona_se_2-0l");
    assert_eq!(answer["vehicle"]["id"], "2023_hyundai_kona");
    assert_eq!(answer["recalls"], Value::Null);

    let asked = run(
        &mut engine,
        &connection,
        "model_years",
        json!({ "make": "hyundai", "model": "kona" }),
    );
    let answer = &asked.answer["ok"];
    assert_eq!(answer["may_apply"], true);
    assert_eq!(answer["as_of"], json!({ "us": null, "ca": null }));
    assert_eq!(answer["years"].as_array().unwrap().len(), 2);
    assert_eq!(answer["years"][0]["recalls"], Value::Null);

    // The recent list is recalls and nothing else. What the API refuses
    // before it looks for them is refused first here too.
    let asked = run(
        &mut engine,
        &connection,
        "recalls_recent",
        json!({ "since": "2023-03-05" }),
    );
    assert_eq!(asked.steps, 0);
    assert_eq!(
        asked.answer["error"],
        json!({
            "code": "dataset_unavailable",
            "message": "The recalls_us dataset is not available.",
            "details": { "dataset": "recalls_us" }
        })
    );
    let asked = run(
        &mut engine,
        &connection,
        "recalls_recent",
        json!({ "since": "2023-03-05", "country": "mx" }),
    );
    assert_eq!(asked.answer["error"]["details"]["field"], "country");
}

#[test]
fn with_the_shop_file_an_overview_counts_the_recalls_that_may_apply() {
    let (connection, shop) = (data(), shop());
    let mut engine = opened_with_shop(&connection, &shop);
    let asked = run_with_shop(
        &mut engine,
        &connection,
        &shop,
        "overview_vin",
        json!({ "vin": KONA }),
    );
    let answer = &asked.answer["ok"];
    assert_eq!(answer.as_object().unwrap().len(), KEYS.len());
    let recalls = &answer["recalls"];
    assert_eq!(recalls["may_apply"], true);
    assert_eq!(recalls["count"], json!({ "us": 2, "ca": 1 }));
    assert_eq!(recalls["do_not_drive"], true);
    assert_eq!(recalls["park_outside"], true);
    let newest: Vec<&str> = recalls["newest"]
        .as_array()
        .unwrap()
        .iter()
        .map(|recall| recall["campaign"].as_str().unwrap())
        .collect();
    assert_eq!(newest, ["24V456000", "2023101", "23V123000"]);
    assert_eq!(
        recalls["url"],
        format!("{SITE}/makes/hyundai/kona/2023/recalls")
    );
    assert_eq!(recalls["lookup"]["nhtsa"], "https://www.nhtsa.gov/recalls");
    assert_eq!(recalls["lookup"]["maker"], Value::Null);
    assert_eq!(answer["links"]["maker"], Value::Null);
    assert_eq!(
        answer["attribution"],
        json!(["Contains information licensed under the Open Government Licence - Canada."])
    );
    // The newest are the first of the whole list.
    let whole = run_with_shop(
        &mut engine,
        &connection,
        &shop,
        "recalls_vehicle",
        json!({ "id": "2023_hyundai_kona" }),
    );
    assert_eq!(recalls["newest"], whole.answer["ok"]["recalls"]);
    // The safety block is the answer of the `safety` operation, whole.
    let safety = run_with_shop(
        &mut engine,
        &connection,
        &shop,
        "safety",
        json!({ "id": "2023_hyundai_kona" }),
    );
    assert_eq!(answer["safety"], safety.answer["ok"]);
    assert_eq!(answer["safety"]["ratings"].as_array().unwrap().len(), 3);
    // A VIN that reaches no model year has no ratings to give.
    let coach = run_with_shop(
        &mut engine,
        &connection,
        &shop,
        "overview_vin",
        json!({ "vin": common::BUS_LATE }),
    );
    assert_eq!(coach.answer["ok"]["vehicle"], Value::Null);
    assert_eq!(coach.answer["ok"]["safety"], Value::Null);

    let asked = run_with_shop(
        &mut engine,
        &connection,
        &shop,
        "model_years",
        json!({ "make": "hyundai", "model": "kona" }),
    );
    assert_eq!(
        asked.answer["ok"]["years"],
        json!([
            {
                "id": "2023_hyundai_kona", "year": 2023,
                "recalls": { "us": 2, "ca": 1 }, "bulletins": null,
                "url": format!("{SITE}/makes/hyundai/kona/2023")
            },
            {
                "id": "2022_hyundai_kona", "year": 2022,
                "recalls": { "us": 0, "ca": 0 }, "bulletins": null,
                "url": format!("{SITE}/makes/hyundai/kona/2022")
            }
        ])
    );

    let asked = run_with_shop(
        &mut engine,
        &connection,
        &shop,
        "recalls_recent",
        json!({ "since": "2019-01-01", "make": "chevy" }),
    );
    let answer = &asked.answer["ok"];
    assert_eq!(answer["may_apply"], true);
    assert_eq!(answer["since"], "2019-01-01");
    assert_eq!(answer["recalls"].as_array().unwrap().len(), 1);
    let listed = &answer["recalls"][0];
    assert_eq!(listed["recall"]["campaign"], "19V888000");
    assert_eq!(listed["vehicles_listed"], 2);
    assert_eq!(
        listed["vehicles"][0]["vehicle_id"],
        "2019_chevrolet_silverado"
    );
    assert_eq!(listed["vehicles"][1]["vehicle_id"], Value::Null);
}

#[test]
fn overview_statements_say_which_file_they_are_for_and_fit_cloudflare_d1() {
    let (connection, shop) = (data(), shop());
    let mut engine = opened_with_shop(&connection, &shop);
    let mut all = Vec::new();
    for (name, op, args) in common::overview_cases() {
        let asked = run_with_shop(&mut engine, &connection, &shop, op, args);
        // An answer, or one of the errors the API gives for the question.
        if let Some(error) = asked.answer.get("error") {
            let code = error["code"].as_str().unwrap();
            assert!(
                ["validation_failed", "not_found", "invalid_vin"].contains(&code),
                "{name}: {code}"
            );
        }
        all.extend(asked.statements);
    }
    assert!(all.len() > 30, "only {} statements", all.len());
    for statement in &all {
        let sql = statement["sql"].as_str().unwrap();
        let params = statement["params"].as_array().unwrap();
        // The recall and rating tables are in the shop file and nothing
        // else is.
        assert_eq!(
            statement.get("db").and_then(Value::as_str) == Some("shop"),
            sql.contains("recall") || sql.contains("rating"),
            "{sql}"
        );
        // D1: at most 100 bound parameters and 100,000 bytes a statement.
        assert!(params.len() <= 100, "{} parameters: {sql}", params.len());
        assert!(sql.len() < 100_000);
        assert!(sql.trim_start().starts_with("SELECT"), "{sql}");
    }
    // How often each question goes back to the databases.
    let mut steps =
        |op: &str, args: Value| run_with_shop(&mut engine, &connection, &shop, op, args).steps;
    assert!(steps("overview_vehicle", json!({ "id": "2023_hyundai_kona" })) <= 5);
    assert!(steps("overview_vin", json!({ "vin": KONA })) <= 12);
    assert!(steps("model_years", json!({ "make": "hyundai", "model": "kona" })) <= 3);
    assert!(steps("recalls_recent", json!({ "since": "2019-01-01" })) <= 5);
}
