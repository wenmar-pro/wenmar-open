//! The engine, run natively against a small data file through `rusqlite`.

mod common;

use std::panic::{AssertUnwindSafe, catch_unwind};

use rusqlite::Connection;
use serde_json::{Value, json};
use wenmar_open_wasm::Engine;
use wenmar_vehicles::Catalog;
use wenmar_vehicles::sqlite::SqliteSource;
use wenmar_vin::sqlite::SqliteData;
use wenmar_vin::{DecodeOptions, Decoder};

use common::{BUS_EARLY, BUS_LATE, KONA, TRAILER, YEAR, data, data_at, opened, run};

fn call(engine: &mut Engine, request: Value) -> Value {
    serde_json::from_str(&engine.call(&request.to_string())).unwrap()
}

/// The decode the two libraries give when they read the file themselves.
fn direct(vin: &str, year: Option<u16>) -> Value {
    let decoder = Decoder::new(SqliteData::from_connection(data()).unwrap());
    let catalog = Catalog::new(SqliteSource::from_connection(data()).unwrap()).unwrap();
    let options = DecodeOptions {
        model_year: year,
        current_year: Some(YEAR),
    };
    let decoded = decoder.decode(vin, options).unwrap();
    let selection = catalog.selection(&decoded).unwrap();
    let mut answer = serde_json::to_value(&decoded).unwrap();
    if let Some(selection) = selection {
        answer["catalog"] = serde_json::to_value(selection).unwrap();
    }
    answer
}

// ----- opening -----

#[test]
fn opening_asks_for_meta_alone_then_for_the_catalog_and_reports_the_versions() {
    let connection = data();
    let mut engine = Engine::new();
    // The schema version is checked before a table that an older layout
    // lacks is named.
    let first = call(&mut engine, json!({ "op": "open" }));
    assert_eq!(
        first,
        json!({ "need": [{ "sql": "SELECT key, value FROM meta", "params": [] }] })
    );

    let run = run(&mut engine, &connection, "open", json!({}));
    assert_eq!(run.steps, 2);
    assert_eq!(run.statements.len(), 6);
    assert_eq!(run.statements[0], first["need"][0]);
    assert_eq!(
        run.answer,
        json!({ "ok": {
            "data_version": "2026.09",
            "vpic_release": "vPICList_lite_2026_09",
            "built_at": "2026-10-01 04:25:57",
            "schema_version": "3",
        } })
    );
}

// Review Focus 1.
#[test]
fn a_data_file_of_another_schema_version_is_refused_and_both_versions_are_named() {
    let connection = data_at("2");
    let mut engine = Engine::new();
    let run = run(&mut engine, &connection, "open", json!({}));
    assert_eq!(run.answer["error"]["code"], "data_invalid");
    assert_eq!(
        run.answer["error"]["message"],
        "The data file has schema version 2. This version of wenmar-open reads schema version 3."
    );
    assert_eq!(
        run.answer["error"]["details"],
        json!({ "schema_version": "2", "expected": "3" })
    );
    // Nothing but meta was asked for: the tables the catalog reads may not
    // be there in a file of another layout.
    assert_eq!(
        run.statements,
        vec![json!({ "sql": "SELECT key, value FROM meta", "params": [] })]
    );
    // And nothing can be read from it afterwards.
    let after = call(
        &mut engine,
        json!({ "op": "decode", "args": { "vin": KONA }, "current_year": YEAR }),
    );
    assert_eq!(after["error"]["code"], "internal_error");
}

#[test]
fn a_database_with_no_schema_version_is_not_a_data_file() {
    let connection = Connection::open_in_memory().unwrap();
    connection
        .execute_batch(
            &common::fixture_sql("3").replace("('schema_version', '3')", "('other', 'x')"),
        )
        .unwrap();
    let run = run(&mut Engine::new(), &connection, "open", json!({}));
    assert_eq!(run.answer["error"]["code"], "data_invalid");
    assert!(
        run.answer["error"]["message"]
            .as_str()
            .unwrap()
            .starts_with("This is not a Wenmar Open data file"),
        "{}",
        run.answer
    );
}

#[test]
fn nothing_but_version_is_answered_before_the_data_is_opened() {
    let mut engine = Engine::new();
    assert_eq!(
        call(&mut engine, json!({ "op": "version" })),
        json!({ "ok": { "version": env!("CARGO_PKG_VERSION"), "schema_version": "3" } })
    );
    for op in [
        "meta",
        "decode",
        "years",
        "makes",
        "models",
        "submodels",
        "engines",
        "search",
        "vehicle",
    ] {
        let answer = call(&mut engine, json!({ "op": op, "current_year": YEAR }));
        assert_eq!(answer["error"]["code"], "internal_error", "{op}: {answer}");
    }
}

// ----- answers -----

#[test]
fn a_decode_is_what_the_libraries_give_when_they_read_the_file_themselves() {
    let connection = data();
    let mut engine = opened(&connection);
    for (vin, year) in [
        (KONA, None),
        (KONA, Some(1993)),
        ("km8k2-cab4 pu001140", None),
        (common::KONA_BAD_CHECK, None),
        (BUS_LATE, None),
        (BUS_EARLY, None),
        (TRAILER, None),
    ] {
        let run = run(
            &mut engine,
            &connection,
            "decode",
            json!({ "vin": vin, "year": year }),
        );
        assert_eq!(run.answer["ok"], direct(vin, year), "{vin} {year:?}");
    }
}

#[test]
fn the_kona_decodes_with_its_specification_sheet_engine_model_and_catalog_entry() {
    let connection = data();
    let mut engine = opened(&connection);
    let run = run(&mut engine, &connection, "decode", json!({ "vin": KONA }));
    let decode = &run.answer["ok"];
    assert_eq!(decode["year"], 2023);
    assert_eq!(decode["make"], "Hyundai");
    assert_eq!(decode["model"], "Kona");
    assert_eq!(decode["trim"], "SE");
    // From the engine model the pattern names.
    assert_eq!(decode["engine"]["cylinders"], 4);
    assert_eq!(decode["engine"]["fuel"], "Gasoline");
    // From the specification sheet for the model, the year and the trim.
    assert_eq!(decode["safety"]["abs"], "Standard");
    assert_eq!(decode["transmission"], "Automatic");
    assert_eq!(decode["catalog"]["vehicle_id"], "2023_hyundai_kona");
    assert_eq!(decode["catalog"]["submodel_id"], "se");
    assert_eq!(
        decode["catalog"]["entry"]["id"],
        "2023_hyundai_kona_se_2-0l"
    );
}

// Review Focus 4.
#[test]
fn a_vin_whose_cycle_is_not_settled_is_decoded_in_both_and_the_better_year_is_kept() {
    let connection = data();
    let mut engine = opened(&connection);

    let late = run(
        &mut engine,
        &connection,
        "decode",
        json!({ "vin": BUS_LATE }),
    );
    assert_eq!(late.answer["ok"]["year"], 2021, "{}", late.answer);
    assert_eq!(late.answer["ok"]["drivetrain"], "6x4");
    // The patterns of both years were read in one statement.
    let patterns: Vec<&Value> = late
        .statements
        .iter()
        .filter(|statement| statement["sql"].as_str().unwrap().contains("FROM pattern"))
        .collect();
    assert_eq!(patterns.len(), 1);
    assert_eq!(patterns[0]["params"], json!([3, 4, "PDMPA|MP000001"]));

    let early = run(
        &mut engine,
        &connection,
        "decode",
        json!({ "vin": BUS_EARLY }),
    );
    assert_eq!(early.answer["ok"]["year"], 1991, "{}", early.answer);
    assert_eq!(early.answer["ok"]["engine"]["cylinders"], 6);
    assert_eq!(early.answer["ok"], direct(BUS_EARLY, None));
}

#[test]
fn errors_have_the_codes_messages_and_details_of_the_api() {
    let connection = data();
    let mut engine = opened(&connection);
    let mut error =
        |op: &str, args: Value| run(&mut engine, &connection, op, args).answer["error"].clone();
    assert_eq!(
        error("decode", json!({ "vin": "KM8K2CAB4PUO01140" })),
        json!({
            "code": "invalid_vin",
            "message": "a VIN uses only digits and letters other than I, O and Q",
            "details": {
                "suggestions": ["KM8K2CAB4PU001140"],
                "invalid_characters": [{ "position": 12, "character": "O" }],
            },
        })
    );
    assert_eq!(
        error("decode", json!({ "vin": "KM8" })),
        json!({
            "code": "invalid_vin",
            "message": "a VIN has 17 characters, this has 3",
            "details": { "suggestions": [] },
        })
    );
    assert_eq!(
        error("decode", json!({ "vin": "K".repeat(70) })),
        json!({
            "code": "invalid_vin",
            "message": "a VIN has 17 characters, this is far longer",
            "details": { "suggestions": [] },
        })
    );
    assert_eq!(
        error("decode", json!({ "vin": common::UNKNOWN })),
        json!({ "code": "not_found", "message": "No manufacturer is registered for ZZZ.", "details": {} })
    );
    assert_eq!(
        error("decode", json!({ "vin": KONA, "year": 1900 })),
        json!({
            "code": "validation_failed",
            "message": "year must be between 1980 and 2028",
            "details": { "field": "year", "min": 1980, "max": 2028 },
        })
    );
    assert_eq!(
        error("models", json!({})),
        json!({ "code": "validation_failed", "message": "make is required", "details": { "field": "make" } })
    );
    assert_eq!(
        error("submodels", json!({ "make": "honda", "model": "civic" })),
        json!({ "code": "validation_failed", "message": "year is required", "details": { "field": "year" } })
    );
    assert_eq!(
        error("makes", json!({ "scope": "nope" })),
        json!({
            "code": "validation_failed",
            "message": "scope must be light, all, or a vehicle type id",
            "details": { "field": "scope" },
        })
    );
    assert_eq!(
        error("search", json!({ "q": "  " })),
        json!({ "code": "validation_failed", "message": "q is required", "details": { "field": "q" } })
    );
    assert_eq!(
        error("vehicle", json!({ "id": "2019_honda_nothing" })),
        json!({ "code": "not_found", "message": "No vehicle has that id.", "details": {} })
    );
}

#[test]
fn the_catalog_answers_as_the_api_does() {
    let connection = data();
    let mut engine = opened(&connection);
    let mut ok =
        |op: &str, args: Value| run(&mut engine, &connection, op, args).answer["ok"].clone();
    assert_eq!(
        ok("years", json!({})),
        json!([2023, 2022, 2020, 2019, 2018])
    );
    assert_eq!(
        ok("years", json!({ "scope": "all", "term": "201" })),
        json!([2019, 2018])
    );
    assert_eq!(
        ok("makes", json!({ "year": 2019, "term": "ch" })),
        json!([{ "id": "chevrolet", "name": "Chevrolet", "popular": true }])
    );
    assert_eq!(
        ok("models", json!({ "make": "chevy", "year": "2019" })),
        json!([{ "id": "silverado", "name": "Silverado", "year_from": 2019, "year_to": 2019 }])
    );
    assert_eq!(
        ok(
            "engines",
            json!({ "make": "honda", "model": "civic", "year": 2019, "submodel": "si" })
        ),
        json!([{ "id": "1-5l-turbo", "label": "1.5L Turbo", "preset": false }])
    );
    let found = ok("search", json!({ "q": "2019 civic si" }));
    assert_eq!(found[0]["id"], "2019_honda_civic_si");
    assert_eq!(ok("search", json!({ "q": "zzzz" })), json!([]));
    assert_eq!(
        ok("meta", json!({})),
        json!({
            "data_version": "2026.09",
            "vpic_release": "vPICList_lite_2026_09",
            "built_at": "2026-10-01 04:25:57",
            "server_version": env!("CARGO_PKG_VERSION"),
        })
    );
}

// ----- the protocol -----

#[test]
fn how_often_each_question_goes_back_to_the_database() {
    let connection = data();
    let mut engine = opened(&connection);
    let mut steps = |op: &str, args: Value| {
        let run = run(&mut engine, &connection, op, args);
        (run.steps, run.statements.len())
    };
    // (times the database is gone back to, statements run)
    assert_eq!(steps("decode", json!({ "vin": KONA })), (7, 15));
    assert_eq!(steps("decode", json!({ "vin": BUS_LATE })), (4, 5));
    assert_eq!(steps("decode", json!({ "vin": common::UNKNOWN })), (1, 1));
    assert_eq!(steps("decode", json!({ "vin": "KM8" })), (0, 0));
    assert_eq!(steps("years", json!({})), (1, 1));
    assert_eq!(steps("makes", json!({})), (0, 0));
    assert_eq!(steps("makes", json!({ "year": 2019 })), (1, 1));
    assert_eq!(steps("models", json!({ "make": "honda" })), (1, 1));
    assert_eq!(
        steps(
            "submodels",
            json!({ "make": "honda", "model": "civic", "year": 2019 })
        ),
        (3, 3)
    );
    assert_eq!(
        steps("vehicle", json!({ "id": "2019_honda_civic_si" })),
        (3, 3)
    );
    let (search, _) = steps("search", json!({ "q": "2019 civic si" }));
    assert!(search <= 5, "search took {search} steps");
}

#[test]
fn two_questions_asked_in_turns_do_not_mix() {
    let connection = data();
    let mut engine = opened(&connection);
    let mut answers: [Vec<Value>; 2] = [Vec::new(), Vec::new()];
    let vins = [KONA, BUS_LATE];
    let mut done: [Option<Value>; 2] = [None, None];
    for _ in 0..32 {
        for index in 0..2 {
            if done[index].is_some() {
                continue;
            }
            let request = json!({
                "op": "decode", "args": { "vin": vins[index] }, "current_year": YEAR, "answers": answers[index],
            });
            let answer = call(&mut engine, request);
            match answer.get("need").and_then(Value::as_array) {
                Some(need) => {
                    for statement in need {
                        answers[index].push(json!({
                            "sql": statement["sql"],
                            "params": statement["params"],
                            "rows": common::rows(&connection, statement),
                        }));
                    }
                }
                None => done[index] = Some(answer),
            }
        }
    }
    assert_eq!(done[0].as_ref().unwrap()["ok"], direct(KONA, None));
    assert_eq!(done[1].as_ref().unwrap()["ok"], direct(BUS_LATE, None));
}

#[test]
fn every_statement_fits_cloudflare_d1() {
    let connection = data();
    let mut engine = opened(&connection);
    let mut all = Vec::new();
    for (_, op, args) in common::cases() {
        all.extend(run(&mut engine, &connection, op, args).statements);
    }
    // Six words of forty characters: the longest text a search reads.
    let long = vec!["a".repeat(60); 6].join(" ");
    all.extend(
        run(
            &mut engine,
            &connection,
            "search",
            json!({ "q": format!("honda {long}") }),
        )
        .statements,
    );
    all.extend(
        run(
            &mut engine,
            &connection,
            "search",
            json!({ "q": format!("civic {long}") }),
        )
        .statements,
    );
    assert!(all.len() > 80, "only {} statements", all.len());
    for statement in &all {
        let sql = statement["sql"].as_str().unwrap();
        let params = statement["params"].as_array().unwrap();
        // D1: at most 100 bound parameters and 100,000 bytes a statement.
        assert!(params.len() <= 100, "{} parameters: {sql}", params.len());
        assert!(sql.len() < 100_000);
        assert!(sql.trim_start().starts_with("SELECT"), "{sql}");
        // D1: a LIKE pattern is at most 50 bytes. Every pattern here is a
        // text parameter with `%` added.
        if sql.contains("LIKE") {
            for param in params.iter().filter_map(Value::as_str) {
                assert!(
                    param.len() < 50,
                    "a {}-byte LIKE parameter: {sql}",
                    param.len()
                );
            }
        }
    }
}

// ----- what must never stop the module -----

#[test]
fn a_request_that_is_not_one_is_an_error() {
    let mut engine = Engine::new();
    for request in [
        "",
        "not json",
        "[]",
        "null",
        "{}",
        r#"{"op":7}"#,
        r#"{"op":"nope"}"#,
        r#"{"op":"open","answers":7}"#,
        r#"{"op":"open","answers":[7]}"#,
        r#"{"op":"open","answers":[{"sql":"x","params":[[1]],"rows":[]}]}"#,
        "{\"op\":\"open\",\"answers\":[{\"sql\":\"\u{0}\",\"params\":[],\"rows\":[]}]}",
    ] {
        let answer: Value = serde_json::from_str(&engine.call(request)).unwrap();
        assert_eq!(
            answer["error"]["code"], "internal_error",
            "{request}: {answer}"
        );
    }
}

#[test]
fn a_decode_with_no_current_year_is_an_error_and_never_reads_a_clock() {
    let connection = data();
    let mut engine = opened(&connection);
    let answer = call(
        &mut engine,
        json!({ "op": "decode", "args": { "vin": KONA } }),
    );
    assert_eq!(answer["error"]["code"], "internal_error", "{answer}");
    let answer = call(
        &mut engine,
        json!({ "op": "decode", "args": { "vin": KONA }, "current_year": 70000 }),
    );
    assert_eq!(answer["error"]["code"], "internal_error", "{answer}");
}

/// The answer with each of its values replaced by `odd`.
fn spoil_answer(answer: &Value, odd: &Value) -> Value {
    let rows: Vec<Value> = answer["rows"]
        .as_array()
        .unwrap()
        .iter()
        .map(|row| Value::Array(vec![odd.clone(); row.as_array().unwrap().len()]))
        .collect();
    json!({ "sql": answer["sql"], "params": answer["params"], "rows": rows })
}

/// Every answer with each of its values replaced by `odd`.
fn spoiled(answers: &[Value], odd: &Value) -> Vec<Value> {
    answers
        .iter()
        .map(|answer| spoil_answer(answer, odd))
        .collect()
}

/// The answers from `from` on spoiled, the ones before kept as they are.
fn spoiled_from(answers: &[Value], from: usize, odd: &Value) -> Vec<Value> {
    let mut out = answers.to_vec();
    for answer in &mut out[from..] {
        *answer = spoil_answer(answer, odd);
    }
    out
}

/// The answers with the one cell at `row`, `column` of answer `index` replaced.
fn spoiled_cell(
    answers: &[Value],
    index: usize,
    row: usize,
    column: usize,
    odd: &Value,
) -> Vec<Value> {
    let mut out = answers.to_vec();
    out[index]["rows"][row][column] = odd.clone();
    out
}

/// Runs the request. The engine must not panic and must give exactly one of
/// `need`, `ok` or `error`. Returns the answer.
fn well_formed(engine: &mut Engine, what: &str, request: &Value) -> Value {
    let text = request.to_string();
    let outcome = catch_unwind(AssertUnwindSafe(|| engine.call(&text)));
    let answer: Value =
        serde_json::from_str(&outcome.unwrap_or_else(|_| panic!("{what} panicked"))).unwrap();
    let kinds = ["need", "ok", "error"]
        .iter()
        .filter(|key| answer.get(**key).is_some())
        .count();
    assert_eq!(kinds, 1, "{what}: {answer}");
    answer
}

// Review Focus 2, on this side of the boundary.
#[test]
fn rows_of_the_wrong_kind_are_an_error_or_an_answer_and_never_a_panic() {
    let connection = data();
    let odd_values = [
        json!(null),
        json!("text where a number belongs"),
        json!(-1),
        json!(9_007_199_254_740_993_i64),
        json!(1.5),
        json!(1e300),
        json!(true),
        json!(""),
    ];
    // One cell at a time, over this many rows of each answer a step adds. A
    // row's cells are the same kind in every row of a table, so the first few
    // rows reach every parser and keep the run to a few seconds.
    const CELL_ROWS: usize = 3;
    for (name, op, args) in common::cases() {
        // The real rows, as far as each step got, then spoiled.
        let mut engine = opened(&connection);
        let mut answers: Vec<Value> = Vec::new();
        for _ in 0..32 {
            let request =
                json!({ "op": op, "args": args, "current_year": YEAR, "answers": answers });
            let answer = call(&mut engine, request);
            let Some(need) = answer.get("need").and_then(Value::as_array) else {
                break;
            };
            let added = answers.len();
            for statement in need {
                answers.push(json!({
                    "sql": statement["sql"],
                    "params": statement["params"],
                    "rows": common::rows(&connection, statement),
                }));
            }
            let request = |answers: Vec<Value>| json!({ "op": op, "args": args, "current_year": YEAR, "answers": answers });
            for odd in &odd_values {
                // Every answer so far, the first statement's rows too.
                let what = format!("{name} with every answer of {odd}");
                well_formed(&mut engine, &what, &request(spoiled(&answers, odd)));
                // Only the answers this step added, the earlier ones real, so
                // the parsers of this step's tables are reached.
                let what = format!("{name} with the new answers of {odd}");
                well_formed(
                    &mut engine,
                    &what,
                    &request(spoiled_from(&answers, added, odd)),
                );
                // One cell of one new answer.
                for index in added..answers.len() {
                    let rows = answers[index]["rows"].as_array().unwrap();
                    for (row, cells) in rows.iter().enumerate().take(CELL_ROWS) {
                        for column in 0..cells.as_array().unwrap().len() {
                            let what = format!(
                                "{name} with {odd} at answer {index} row {row} column {column}"
                            );
                            let spoiled = spoiled_cell(&answers, index, row, column, odd);
                            well_formed(&mut engine, &what, &request(spoiled));
                        }
                    }
                }
            }
        }
    }
    // The catalog's own rows, spoiled while the data is being opened.
    for odd in &odd_values {
        let mut engine = Engine::new();
        // Everything opening reads, meta and then the catalog's statements.
        let answers: Vec<Value> = run(&mut engine, &connection, "open", json!({}))
            .statements
            .iter()
            .map(|statement| {
                json!({
                    "sql": statement["sql"],
                    "params": statement["params"],
                    "rows": common::rows(&connection, statement),
                })
            })
            .collect();
        // With the meta rows spoiled too: no schema_version to be found.
        let request = json!({ "op": "open", "answers": spoiled(&answers, odd) });
        let answer = well_formed(
            &mut engine,
            &format!("open with every answer of {odd}"),
            &request,
        );
        assert_eq!(answer["error"]["code"], "data_invalid", "{odd}: {answer}");
        // With meta real and the catalog's answers spoiled, whole and by cell.
        let catalog: Vec<usize> = (0..answers.len())
            .filter(|&index| !answers[index]["sql"].as_str().unwrap().contains("meta"))
            .collect();
        assert!(!catalog.is_empty(), "no catalog statements in {answers:?}");
        let mut spoil = answers.clone();
        for &index in &catalog {
            spoil[index] = spoil_answer(&answers[index], odd);
        }
        let request = json!({ "op": "open", "answers": spoil });
        well_formed(
            &mut engine,
            &format!("open with the catalog rows of {odd}"),
            &request,
        );
        for &index in &catalog {
            let rows = answers[index]["rows"].as_array().unwrap();
            for (row, cells) in rows.iter().enumerate().take(CELL_ROWS) {
                for column in 0..cells.as_array().unwrap().len() {
                    let what =
                        format!("open with {odd} at answer {index} row {row} column {column}");
                    let spoil = spoiled_cell(&answers, index, row, column, odd);
                    let request = json!({ "op": "open", "answers": spoil });
                    well_formed(&mut engine, &what, &request);
                }
            }
        }
    }
}

#[test]
fn a_value_no_database_holds_is_refused_by_name() {
    let mut engine = Engine::new();
    let answer = call(
        &mut engine,
        json!({ "op": "open", "answers": [
            { "sql": "SELECT key, value FROM meta", "params": [], "rows": [[{ "0": 1 }, [1, 2]]] },
        ] }),
    );
    assert_eq!(
        answer["error"],
        json!({
            "code": "data_invalid",
            "message": "The database returned a value that is not a number, text or null.",
            "details": {},
        })
    );
}
