//! The recall operations, run natively against the small data file and a
//! small shop file through `rusqlite`.

mod common;

use serde_json::{Value, json};
use wenmar_open_wasm::Engine;

use common::{KONA, data, opened, opened_with_shop, run, run_with_shop, shop, shop_at};

#[test]
fn opening_the_shop_file_reads_its_meta_alone_and_reports_what_it_holds() {
    let (connection, shop) = (data(), shop());
    let mut engine = opened(&connection);
    let opened = run_with_shop(&mut engine, &connection, &shop, "open_shop", json!({}));
    assert_eq!(opened.steps, 1);
    assert_eq!(
        opened.statements,
        [json!({ "sql": "SELECT key, value FROM meta", "params": [], "db": "shop" })]
    );
    assert_eq!(
        opened.answer["ok"],
        json!({
            "data_version": "2026.10.12",
            "catalog_data_version": "2026.09",
            "built_at": "2026-10-12 06:00:00",
            "schema_version": "1",
            "datasets": serde_json::from_str::<Value>(common::SHOP_DATASETS).unwrap()
        })
    );
    // `meta` then lists the datasets, as the API does on a server with a
    // shop file.
    let meta = run_with_shop(&mut engine, &connection, &shop, "meta", json!({}));
    assert_eq!(
        meta.answer["ok"]["datasets"]["recalls_ca"]["as_of"],
        "2026-10-06"
    );
    assert_eq!(meta.steps, 0);
    // The version says which shop files this build reads.
    let version: Value = serde_json::from_str(&Engine::new().call(r#"{"op":"version"}"#)).unwrap();
    assert_eq!(version["ok"]["shop_schema_version"], "1");
}

#[test]
fn a_shop_file_that_does_not_go_with_the_data_file_is_refused() {
    let connection = data();
    let cases = [
        (
            shop_at("1", "2026.08"),
            "The shop file cannot be used: shop file was matched against data version 2026.08, but the data file is version 2026.09; rebuild it with open-data build-shop --catalog <the data file>.",
        ),
        (
            shop_at("2", "2026.09"),
            "The shop file cannot be used: shop file has schema version 2, this build reads version 1; rebuild it with open-data build-shop.",
        ),
    ];
    for (shop, message) in cases {
        let mut engine = opened(&connection);
        let refused = run_with_shop(&mut engine, &connection, &shop, "open_shop", json!({}));
        assert_eq!(
            refused.answer["error"],
            json!({ "code": "data_invalid", "message": message, "details": {} })
        );
        // The data file still answers, and recalls are as with no shop file.
        let decoded = run(&mut engine, &connection, "decode", json!({ "vin": KONA }));
        assert_eq!(decoded.answer["ok"]["model"], "Kona");
        let recalls = run(
            &mut engine,
            &connection,
            "recalls_vehicle",
            json!({ "id": "2023_hyundai_kona" }),
        );
        assert_eq!(recalls.answer["error"]["code"], "dataset_unavailable");
    }
    // A database that is not a shop file: the data file, say.
    let mut engine = opened(&connection);
    let refused = run_with_shop(
        &mut engine,
        &connection,
        &connection,
        "open_shop",
        json!({}),
    );
    assert_eq!(refused.answer["error"]["code"], "data_invalid");
    // The shop file cannot come first.
    let early: Value =
        serde_json::from_str(&Engine::new().call(r#"{"op":"open_shop","answers":[]}"#)).unwrap();
    assert_eq!(early["error"]["code"], "internal_error");
}

#[test]
fn with_no_shop_file_every_recall_question_is_dataset_unavailable() {
    let connection = data();
    let mut engine = opened(&connection);
    let cases = [
        (
            "recalls_vehicle",
            json!({ "id": "2023_hyundai_kona" }),
            "recalls_us",
        ),
        ("recalls_vin", json!({ "vin": KONA }), "recalls_us"),
        ("recalls_vin", json!({ "vin": "nope" }), "recalls_us"),
        (
            "recall",
            json!({ "source": "nhtsa", "campaign": "19V888000" }),
            "recalls_us",
        ),
        (
            "recall",
            json!({ "source": "tc", "campaign": "2023101" }),
            "recalls_ca",
        ),
        ("recalls_search", json!({}), "recalls_us"),
        ("recalls_search", json!({ "country": "CA" }), "recalls_ca"),
    ];
    for (op, args, dataset) in cases {
        let asked = run(&mut engine, &connection, op, args.clone());
        assert_eq!(asked.steps, 0, "{op} {args}");
        assert_eq!(
            asked.answer["error"],
            json!({
                "code": "dataset_unavailable",
                "message": format!("The {dataset} dataset is not available."),
                "details": { "dataset": dataset }
            }),
            "{op} {args}"
        );
    }
    // A country that is not one is refused first, as the API refuses it.
    let asked = run(
        &mut engine,
        &connection,
        "recalls_search",
        json!({ "country": "mx" }),
    );
    assert_eq!(asked.answer["error"]["code"], "validation_failed");
    assert_eq!(asked.answer["error"]["details"]["field"], "country");
    // And `meta` lists no datasets.
    let meta = run(&mut engine, &connection, "meta", json!({}));
    assert_eq!(meta.answer["ok"]["datasets"], json!({}));
}

#[test]
fn a_model_year_gets_the_recalls_that_may_apply_and_never_more_than_that() {
    let (connection, shop) = (data(), shop());
    let mut engine = opened_with_shop(&connection, &shop);
    let asked = run_with_shop(
        &mut engine,
        &connection,
        &shop,
        "recalls_vin",
        json!({ "vin": KONA }),
    );
    let answer = &asked.answer["ok"];
    assert_eq!(answer["may_apply"], true);
    assert_eq!(answer["vehicle"]["id"], "2023_hyundai_kona");
    assert_eq!(answer["decode"]["vin"], KONA);
    let campaigns: Vec<&str> = answer["recalls"]
        .as_array()
        .unwrap()
        .iter()
        .map(|recall| recall["campaign"].as_str().unwrap())
        .collect();
    assert_eq!(campaigns, ["24V456000", "2023101", "23V123000"]);
    assert_eq!(
        answer["recalls"][0]["url"],
        "https://open.wenmarpro.com/recalls/nhtsa/24V456000"
    );
    assert_eq!(answer["lookup"]["nhtsa"], "https://www.nhtsa.gov/recalls");
    assert_eq!(answer["lookup"]["maker"], Value::Null);
    // Every answer of every case says "may apply" and gives the lookups.
    for (name, op, args) in common::shop_cases() {
        let asked = run_with_shop(&mut engine, &connection, &shop, op, args);
        if let Some(answer) = asked.answer.get("ok").filter(|_| op != "meta") {
            assert_eq!(answer["may_apply"], true, "{name}");
            assert!(answer["lookup"]["nhtsa"].is_string(), "{name}");
            assert!(answer["lookup"]["tc"].is_string(), "{name}");
        } else if op != "meta" {
            let code = asked.answer["error"]["code"].as_str().unwrap();
            assert!(
                ["validation_failed", "not_found", "invalid_vin"].contains(&code),
                "{name}: {code}"
            );
        }
    }
}

#[test]
fn recall_statements_say_which_file_they_are_for_and_fit_cloudflare_d1() {
    let (connection, shop) = (data(), shop());
    let mut engine = opened_with_shop(&connection, &shop);
    let mut all = Vec::new();
    for (_, op, args) in common::shop_cases() {
        all.extend(run_with_shop(&mut engine, &connection, &shop, op, args).statements);
    }
    assert!(all.len() > 20, "only {} statements", all.len());
    let mut of_shop = 0;
    for statement in &all {
        let sql = statement["sql"].as_str().unwrap();
        let params = statement["params"].as_array().unwrap();
        // The recall tables are in the shop file and nothing else is.
        let names_recalls = sql.contains("recall");
        assert_eq!(
            statement.get("db").and_then(Value::as_str) == Some("shop"),
            names_recalls,
            "{sql}"
        );
        of_shop += usize::from(names_recalls);
        // D1: at most 100 bound parameters and 100,000 bytes a statement.
        assert!(params.len() <= 100, "{} parameters: {sql}", params.len());
        assert!(sql.len() < 100_000);
        assert!(sql.trim_start().starts_with("SELECT"), "{sql}");
    }
    assert!(of_shop >= 8, "{of_shop} statements for the shop file");
    // How often each question goes back to the databases.
    let mut steps =
        |op: &str, args: Value| run_with_shop(&mut engine, &connection, &shop, op, args).steps;
    assert!(steps("recalls_vehicle", json!({ "id": "2023_hyundai_kona" })) <= 5);
    assert!(
        steps(
            "recall",
            json!({ "source": "nhtsa", "campaign": "19V888000" })
        ) <= 3
    );
    assert!(steps("recalls_search", json!({})) <= 1);
    assert!(steps("recalls_vin", json!({ "vin": KONA })) <= 12);
}
