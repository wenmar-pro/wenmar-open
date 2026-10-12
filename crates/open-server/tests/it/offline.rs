//! The offline engine against this server: the same data file, the same
//! questions, the same JSON. `wenmar-open-wasm` is what the npm package's
//! offline mode runs; here it runs natively and reads the fixture through
//! `rusqlite`, the way the package reads it through a JavaScript database.

use axum::http::StatusCode;
use rusqlite::types::{Value as Sql, ValueRef};
use rusqlite::{Connection, params_from_iter};
use serde_json::{Value, json};
use wenmar_open_wasm::Engine;

use crate::common;

/// Runs one statement the engine asked for.
fn rows(connection: &Connection, statement: &Value) -> Value {
    let params: Vec<Sql> = statement["params"]
        .as_array()
        .unwrap()
        .iter()
        .map(|cell| match cell {
            Value::Null => Sql::Null,
            Value::Number(number) => Sql::Integer(number.as_i64().unwrap()),
            Value::String(text) => Sql::Text(text.clone()),
            other => panic!("a statement asked to bind {other}"),
        })
        .collect();
    let mut prepared = connection
        .prepare(statement["sql"].as_str().unwrap())
        .unwrap();
    let columns = prepared.column_count();
    let mut found = prepared.query(params_from_iter(params)).unwrap();
    let mut rows = Vec::new();
    while let Some(row) = found.next().unwrap() {
        let cells: Vec<Value> = (0..columns)
            .map(|column| match row.get_ref(column).unwrap() {
                ValueRef::Null => Value::Null,
                ValueRef::Integer(number) => json!(number),
                ValueRef::Real(number) => json!(number),
                ValueRef::Text(text) => json!(String::from_utf8_lossy(text)),
                ValueRef::Blob(_) => panic!("the data file holds no blobs"),
            })
            .collect();
        rows.push(Value::Array(cells));
    }
    Value::Array(rows)
}

/// Sends a request until it is answered, as the npm package does.
fn ask(engine: &mut Engine, connection: &Connection, op: &str, args: &Value) -> Value {
    let current_year = open_server::api::vin::current_year();
    let mut answers: Vec<Value> = Vec::new();
    for _ in 0..32 {
        let request =
            json!({ "op": op, "args": args, "current_year": current_year, "answers": answers });
        let answer: Value = serde_json::from_str(&engine.call(&request.to_string())).unwrap();
        let Some(need) = answer.get("need").and_then(Value::as_array) else {
            return answer;
        };
        for statement in need {
            answers.push(json!({
                "sql": statement["sql"],
                "params": statement["params"],
                "rows": rows(connection, statement),
            }));
        }
    }
    panic!("{op} did not finish");
}

#[tokio::test]
async fn the_offline_engine_answers_every_question_as_the_api_does() {
    let app = common::app().await;
    // The same rows the application was opened over.
    let fixture = common::data_file();
    let connection = Connection::open(fixture.path()).unwrap();
    let mut engine = Engine::new();
    let opened = ask(&mut engine, &connection, "open", &json!({}));
    assert!(opened.get("ok").is_some(), "{opened}");

    let questions = [
        (
            "/v1/vin/KM8K2CAB4PU001140",
            "decode",
            json!({ "vin": common::KONA }),
        ),
        (
            "/v1/vin/km8k2-cab4pu001140",
            "decode",
            json!({ "vin": "km8k2-cab4pu001140" }),
        ),
        (
            "/v1/vin/KM8K2CAB0PU001140",
            "decode",
            json!({ "vin": common::KONA_BAD_CHECK }),
        ),
        (
            "/v1/vin/KM8K2CAB4PU001140?year=1993",
            "decode",
            json!({ "vin": common::KONA, "year": 1993 }),
        ),
        (
            "/v1/vin/KM8K2CAB4PU001140?year=1900",
            "decode",
            json!({ "vin": common::KONA, "year": 1900 }),
        ),
        (
            "/v1/vin/1M8PDMPA9SP000001",
            "decode",
            json!({ "vin": common::COACH }),
        ),
        (
            "/v1/vin/1A9100AA851881001",
            "decode",
            json!({ "vin": common::TRAILER }),
        ),
        (
            "/v1/vin/ZZZK2CAB4PU001140",
            "decode",
            json!({ "vin": common::UNKNOWN }),
        ),
        (
            "/v1/vin/KM8K2CAB4PUO01140",
            "decode",
            json!({ "vin": "KM8K2CAB4PUO01140" }),
        ),
        ("/v1/vin/KM8", "decode", json!({ "vin": "KM8" })),
        ("/v1/vehicles/years", "years", json!({})),
        (
            "/v1/vehicles/years?scope=all&term=201",
            "years",
            json!({ "scope": "all", "term": "201" }),
        ),
        ("/v1/vehicles/makes", "makes", json!({})),
        (
            "/v1/vehicles/makes?year=2019&term=ch&limit=5",
            "makes",
            json!({ "year": 2019, "term": "ch", "limit": 5 }),
        ),
        (
            "/v1/vehicles/makes?scope=nope",
            "makes",
            json!({ "scope": "nope" }),
        ),
        (
            "/v1/vehicles/models?make=honda",
            "models",
            json!({ "make": "honda" }),
        ),
        (
            "/v1/vehicles/models?make=chevy&year=2019",
            "models",
            json!({ "make": "chevy", "year": 2019 }),
        ),
        ("/v1/vehicles/models", "models", json!({})),
        (
            "/v1/vehicles/submodels?make=honda&model=civic&year=2019",
            "submodels",
            json!({ "make": "honda", "model": "civic", "year": 2019 }),
        ),
        (
            "/v1/vehicles/trims?make=honda&model=civic",
            "submodels",
            json!({ "make": "honda", "model": "civic" }),
        ),
        (
            "/v1/vehicles/engines?make=honda&model=civic&year=2019&submodel=si",
            "engines",
            json!({ "make": "honda", "model": "civic", "year": 2019, "submodel": "si" }),
        ),
        (
            "/v1/vehicles/engines?make=ford&model=f150&year=2019",
            "engines",
            json!({ "make": "ford", "model": "f150", "year": 2019 }),
        ),
        (
            "/v1/vehicles/search?q=2019%20civic%20si",
            "search",
            json!({ "q": "2019 civic si" }),
        ),
        (
            "/v1/vehicles/search?q=chevy%201500&limit=50",
            "search",
            json!({ "q": "chevy 1500", "limit": 50 }),
        ),
        (
            "/v1/vehicles/search?q=honda",
            "search",
            json!({ "q": "honda" }),
        ),
        ("/v1/vehicles/search", "search", json!({})),
        (
            "/v1/vehicles/2019_honda_civic_si",
            "vehicle",
            json!({ "id": "2019_honda_civic_si" }),
        ),
        (
            "/v1/vehicles/2019_honda_nothing",
            "vehicle",
            json!({ "id": "2019_honda_nothing" }),
        ),
        ("/v1/meta", "meta", json!({})),
        ("/v1/vehicles/makes?limit=0", "makes", json!({ "limit": 0 })),
        (
            "/v1/vehicles/search?q=honda&limit=0",
            "search",
            json!({ "q": "honda", "limit": 0 }),
        ),
        (
            "/v1/vehicles/makes?year=%2B2019",
            "makes",
            json!({ "year": "+2019" }),
        ),
        (
            "/v1/vehicles/search?q=honda&limit=%2B5",
            "search",
            json!({ "q": "honda", "limit": "+5" }),
        ),
    ];
    for (path, op, args) in &questions {
        let (status, served) = app.json(path).await;
        let answer = ask(&mut engine, &connection, op, args);
        if status == StatusCode::OK {
            assert_eq!(answer["ok"], served, "{path}");
        } else {
            // The API wraps its error in `{ "error": ... }`; so does this.
            assert_eq!(json!({ "error": answer["error"] }), served, "{path}");
            let code = answer["error"]["code"].as_str().unwrap();
            let expected = match code {
                "not_found" => StatusCode::NOT_FOUND,
                _ => StatusCode::BAD_REQUEST,
            };
            assert_eq!(status, expected, "{path}");
        }
    }
}

/// [`ask`] with a shop file: a statement marked `"db": "shop"` is run on
/// `shop`, and its rows are sent back with the same mark.
fn ask_with_shop(
    engine: &mut Engine,
    connection: &Connection,
    shop: &Connection,
    op: &str,
    args: &Value,
) -> Value {
    let current_year = open_server::api::vin::current_year();
    let mut answers: Vec<Value> = Vec::new();
    for _ in 0..32 {
        let request =
            json!({ "op": op, "args": args, "current_year": current_year, "answers": answers });
        let answer: Value = serde_json::from_str(&engine.call(&request.to_string())).unwrap();
        let Some(need) = answer.get("need").and_then(Value::as_array) else {
            return answer;
        };
        for statement in need {
            let mut answered = statement.clone();
            answered["rows"] = match statement.get("db").and_then(Value::as_str) {
                None => rows(connection, statement),
                Some("shop") => rows(shop, statement),
                Some(other) => panic!("a statement for a database called {other}"),
            };
            answers.push(answered);
        }
    }
    panic!("{op} did not finish");
}

/// The questions about recalls: the API's address, then the engine's
/// operation and arguments.
fn recall_questions() -> Vec<(&'static str, &'static str, Value)> {
    vec![
        (
            "/v1/vehicles/2023_hyundai_kona/recalls",
            "recalls_vehicle",
            json!({ "id": "2023_hyundai_kona" }),
        ),
        (
            "/v1/vehicles/2023_hyundai_kona_se/recalls?country=ca",
            "recalls_vehicle",
            json!({ "id": "2023_hyundai_kona_se", "country": "ca" }),
        ),
        (
            "/v1/vehicles/2019_chevrolet_silverado/recalls?country=US",
            "recalls_vehicle",
            json!({ "id": "2019_chevrolet_silverado", "country": "US" }),
        ),
        (
            "/v1/vehicles/2019_honda_civic/recalls",
            "recalls_vehicle",
            json!({ "id": "2019_honda_civic" }),
        ),
        (
            "/v1/vehicles/2019_honda_nothing/recalls",
            "recalls_vehicle",
            json!({ "id": "2019_honda_nothing" }),
        ),
        (
            "/v1/vehicles/2023_hyundai_kona/recalls?country=mx",
            "recalls_vehicle",
            json!({ "id": "2023_hyundai_kona", "country": "mx" }),
        ),
        (
            "/v1/vin/KM8K2CAB4PU001140/recalls",
            "recalls_vin",
            json!({ "vin": common::KONA }),
        ),
        (
            "/v1/vin/km8k2-cab4pu001140/recalls?year=2023&country=us",
            "recalls_vin",
            json!({ "vin": "km8k2-cab4pu001140", "year": 2023, "country": "us" }),
        ),
        (
            "/v1/vin/KM8K2CAB0PU001140/recalls",
            "recalls_vin",
            json!({ "vin": common::KONA_BAD_CHECK }),
        ),
        (
            "/v1/vin/1M8PDMPA9SP000001/recalls",
            "recalls_vin",
            json!({ "vin": common::COACH }),
        ),
        (
            "/v1/vin/1A9100AA851881001/recalls",
            "recalls_vin",
            json!({ "vin": common::TRAILER }),
        ),
        (
            "/v1/vin/ZZZK2CAB4PU001140/recalls",
            "recalls_vin",
            json!({ "vin": common::UNKNOWN }),
        ),
        (
            "/v1/vin/KM8K2CAB4PUO01140/recalls",
            "recalls_vin",
            json!({ "vin": "KM8K2CAB4PUO01140" }),
        ),
        (
            "/v1/vin/KM8K2CAB4PU001140/recalls?year=1900",
            "recalls_vin",
            json!({ "vin": common::KONA, "year": 1900 }),
        ),
        (
            "/v1/recalls/nhtsa/19V888000",
            "recall",
            json!({ "source": "nhtsa", "campaign": "19V888000" }),
        ),
        (
            "/v1/recalls/nhtsa/23v123000",
            "recall",
            json!({ "source": "nhtsa", "campaign": "23v123000" }),
        ),
        (
            "/v1/recalls/tc/2023101",
            "recall",
            json!({ "source": "tc", "campaign": "2023101" }),
        ),
        (
            "/v1/recalls/nhtsa/68V023000",
            "recall",
            json!({ "source": "nhtsa", "campaign": "68V023000" }),
        ),
        (
            "/v1/recalls/nhtsa/19V600000",
            "recall",
            json!({ "source": "nhtsa", "campaign": "19V600000" }),
        ),
        (
            "/v1/recalls/nhtsa/00V000000",
            "recall",
            json!({ "source": "nhtsa", "campaign": "00V000000" }),
        ),
        (
            "/v1/recalls/epa/19V888000",
            "recall",
            json!({ "source": "epa", "campaign": "19V888000" }),
        ),
        ("/v1/recalls", "recalls_search", json!({})),
        (
            "/v1/recalls?limit=2",
            "recalls_search",
            json!({ "limit": 2 }),
        ),
        (
            "/v1/recalls?limit=4&cursor=2022-11-20_2",
            "recalls_search",
            json!({ "limit": 4, "cursor": "2022-11-20_2" }),
        ),
        (
            "/v1/recalls?make=hyundai&year=2023",
            "recalls_search",
            json!({ "make": "hyundai", "year": 2023 }),
        ),
        (
            "/v1/recalls?make=chevy&model=silverado&country=ca",
            "recalls_search",
            json!({ "make": "chevy", "model": "silverado", "country": "ca" }),
        ),
        (
            "/v1/recalls?q=wiring%20harness&component=electrical",
            "recalls_search",
            json!({ "q": "wiring harness", "component": "electrical" }),
        ),
        (
            "/v1/recalls?make=delorean",
            "recalls_search",
            json!({ "make": "delorean" }),
        ),
        (
            "/v1/recalls?model=kona",
            "recalls_search",
            json!({ "model": "kona" }),
        ),
        (
            "/v1/recalls?cursor=nonsense",
            "recalls_search",
            json!({ "cursor": "nonsense" }),
        ),
        (
            "/v1/recalls?limit=0",
            "recalls_search",
            json!({ "limit": 0 }),
        ),
        ("/v1/meta", "meta", json!({})),
    ]
}

/// The one thing the files cannot say: the maker's own lookup is compiled
/// into the server. It is set aside before two answers are compared.
fn without_maker(mut answer: Value) -> Value {
    if let Some(lookup) = answer.get_mut("lookup").and_then(Value::as_object_mut) {
        lookup.insert("maker".to_owned(), Value::Null);
    }
    answer
}

#[tokio::test]
async fn the_offline_engine_answers_recall_questions_as_the_api_does() {
    // The API says its pages are where the engine says they are.
    let app = common::recalls_app_at(wenmar_shopdata::answer::SITE).await;
    // The same rows the application was opened over, in two files.
    let fixture = common::data_file();
    common::add_shop_file(
        &fixture,
        wenmar_shopdata::schema::SCHEMA_VERSION,
        "2026.09",
        common::RECALL_DATASETS,
        &common::recall_rows(),
    );
    let connection = Connection::open(fixture.path()).unwrap();
    let shop = Connection::open(fixture.shop_path()).unwrap();
    let mut engine = Engine::new();
    let opened = ask(&mut engine, &connection, "open", &json!({}));
    assert!(opened.get("ok").is_some(), "{opened}");
    let opened = ask_with_shop(&mut engine, &connection, &shop, "open_shop", &json!({}));
    assert!(opened.get("ok").is_some(), "{opened}");

    for (path, op, args) in &recall_questions() {
        let (status, served) = app.json(path).await;
        let answer = ask_with_shop(&mut engine, &connection, &shop, op, args);
        if status == StatusCode::OK {
            assert_eq!(answer["ok"], without_maker(served.clone()), "{path}");
            if *op != "meta" {
                // Never more than "may apply", and always where to check.
                assert_eq!(answer["ok"]["may_apply"], true, "{path}");
                assert_eq!(answer["ok"]["lookup"]["nhtsa"], served["lookup"]["nhtsa"]);
                assert_eq!(answer["ok"]["lookup"]["tc"], served["lookup"]["tc"]);
            }
        } else {
            assert_eq!(json!({ "error": answer["error"] }), served, "{path}");
            let code = answer["error"]["code"].as_str().unwrap();
            let expected = match code {
                "not_found" => StatusCode::NOT_FOUND,
                _ => StatusCode::BAD_REQUEST,
            };
            assert_eq!(status, expected, "{path}");
        }
    }
}

#[tokio::test]
async fn with_no_shop_file_the_engine_and_the_api_refuse_recall_questions_alike() {
    let app = common::app().await;
    let fixture = common::data_file();
    let connection = Connection::open(fixture.path()).unwrap();
    let mut engine = Engine::new();
    let opened = ask(&mut engine, &connection, "open", &json!({}));
    assert!(opened.get("ok").is_some(), "{opened}");
    for (path, op, args) in &recall_questions() {
        let (status, served) = app.json(path).await;
        let answer = ask(&mut engine, &connection, op, args);
        if *op == "meta" {
            assert_eq!(answer["ok"], served, "{path}");
            continue;
        }
        // 400 for a country that is not one; 404 `not_found` for a publisher
        // that is not one; and otherwise 404 naming the dataset that is
        // missing, whatever else is wrong with the question.
        assert_eq!(json!({ "error": answer["error"] }), served, "{path}");
        let code = answer["error"]["code"].as_str().unwrap();
        let expected = match code {
            "dataset_unavailable" | "not_found" => StatusCode::NOT_FOUND,
            _ => StatusCode::BAD_REQUEST,
        };
        assert_eq!(status, expected, "{path}");
    }
}
