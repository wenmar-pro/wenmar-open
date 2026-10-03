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
