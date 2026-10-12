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
        (
            "/v1/recalls/recent?since=2019-01-01",
            "recalls_recent",
            json!({ "since": "2019-01-01" }),
        ),
        (
            "/v1/recalls/recent?since=2019-01-01&country=CA&make=chevy",
            "recalls_recent",
            json!({ "since": "2019-01-01", "country": "CA", "make": "chevy" }),
        ),
        (
            "/v1/recalls/recent?since=2019-01-01&make=nobody",
            "recalls_recent",
            json!({ "since": "2019-01-01", "make": "nobody" }),
        ),
        (
            "/v1/recalls/recent?since=1960-01-01&limit=2",
            "recalls_recent",
            json!({ "since": "1960-01-01", "limit": 2 }),
        ),
        (
            "/v1/recalls/recent?since=1960-01-01&limit=2&cursor=2023-03-10_9",
            "recalls_recent",
            json!({ "since": "1960-01-01", "limit": 2, "cursor": "2023-03-10_9" }),
        ),
        (
            "/v1/recalls/recent?since=2024-05-11",
            "recalls_recent",
            json!({ "since": "2024-05-11" }),
        ),
        ("/v1/recalls/recent", "recalls_recent", json!({})),
        (
            "/v1/recalls/recent?since=2026-9-1",
            "recalls_recent",
            json!({ "since": "2026-9-1" }),
        ),
        (
            "/v1/recalls/recent?since=2019-01-01&country=mx",
            "recalls_recent",
            json!({ "since": "2019-01-01", "country": "mx" }),
        ),
        (
            "/v1/recalls/recent?since=2019-01-01&cursor=nope",
            "recalls_recent",
            json!({ "since": "2019-01-01", "cursor": "nope" }),
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

/// The questions an overview answers with or without a shop file: the
/// API's address, then the engine's operation and arguments.
fn overview_questions() -> Vec<(&'static str, &'static str, Value)> {
    vec![
        (
            "/v1/vehicles/2023_hyundai_kona/overview",
            "overview_vehicle",
            json!({ "id": "2023_hyundai_kona" }),
        ),
        (
            "/v1/vehicles/2023_hyundai_kona_se/overview",
            "overview_vehicle",
            json!({ "id": "2023_hyundai_kona_se" }),
        ),
        (
            "/v1/vehicles/2019_chevrolet_silverado/overview",
            "overview_vehicle",
            json!({ "id": "2019_chevrolet_silverado" }),
        ),
        (
            "/v1/vehicles/2019_honda_civic/overview",
            "overview_vehicle",
            json!({ "id": "2019_honda_civic" }),
        ),
        (
            "/v1/vehicles/2019_honda_nothing/overview",
            "overview_vehicle",
            json!({ "id": "2019_honda_nothing" }),
        ),
        (
            "/v1/vin/KM8K2CAB4PU001140/overview",
            "overview_vin",
            json!({ "vin": common::KONA }),
        ),
        (
            "/v1/vin/km8k2-cab4pu001140/overview?year=2023",
            "overview_vin",
            json!({ "vin": "km8k2-cab4pu001140", "year": 2023 }),
        ),
        (
            "/v1/vin/KM8K2CAB0PU001140/overview",
            "overview_vin",
            json!({ "vin": common::KONA_BAD_CHECK }),
        ),
        (
            "/v1/vin/1M8PDMPA9SP000001/overview",
            "overview_vin",
            json!({ "vin": common::COACH }),
        ),
        (
            "/v1/vin/1A9100AA851881001/overview",
            "overview_vin",
            json!({ "vin": common::TRAILER }),
        ),
        (
            "/v1/vin/ZZZK2CAB4PU001140/overview",
            "overview_vin",
            json!({ "vin": common::UNKNOWN }),
        ),
        (
            "/v1/vin/KM8K2CAB4PUO01140/overview",
            "overview_vin",
            json!({ "vin": "KM8K2CAB4PUO01140" }),
        ),
        (
            "/v1/vin/KM8K2CAB4PU001140/overview?year=1900",
            "overview_vin",
            json!({ "vin": common::KONA, "year": 1900 }),
        ),
        (
            "/v1/models/hyundai/kona",
            "model_years",
            json!({ "make": "hyundai", "model": "kona" }),
        ),
        (
            "/v1/models/chevy/Silverado",
            "model_years",
            json!({ "make": "chevy", "model": "Silverado" }),
        ),
        (
            "/v1/models/ford/f150",
            "model_years",
            json!({ "make": "ford", "model": "f150" }),
        ),
        (
            "/v1/models/honda/civic",
            "model_years",
            json!({ "make": "honda", "model": "civic" }),
        ),
        (
            "/v1/models/honda/nothing",
            "model_years",
            json!({ "make": "honda", "model": "nothing" }),
        ),
        (
            "/v1/models/nobody/civic",
            "model_years",
            json!({ "make": "nobody", "model": "civic" }),
        ),
    ]
}

/// The blocks of an overview that the hosted API fills where its server
/// holds their data, and that the files do not give yet: from files they
/// are `null` whatever the shop file holds. `specs`, `fuel`, `fluids`,
/// `tires` and `complaints` are given from files now. `bulletins` is hosted
/// only by design: the offline shop file does not hold them.
const HOSTED_ONLY: [&str; 1] = ["bulletins"];

/// An answer of the API as the files give it. What is set aside before
/// two answers are compared, and why:
///
/// - `lookup.maker`, `recalls.lookup.maker` and `links.maker`: the makers'
///   own lookup and links are compiled into the server from
///   `data/catalog/makes.yaml` and are in neither file.
/// - the blocks of [`HOSTED_ONLY`], and each model year's `bulletins`
///   count, which is hosted only for the same reason.
///
/// Nothing else is: every other field, the other `null` blocks among them,
/// must be the same. The shop file of these tests holds recalls only, so
/// `attribution` is compared whole. A test over a shop file that holds the
/// data of a [`HOSTED_ONLY`] block must set that block's credit lines
/// aside too.
fn as_the_files_give_it(mut answer: Value) -> Value {
    for place in ["/lookup/maker", "/recalls/lookup/maker", "/links/maker"] {
        if let Some(maker) = answer.pointer_mut(place) {
            *maker = Value::Null;
        }
    }
    for block in HOSTED_ONLY {
        if let Some(held) = answer.get_mut(block) {
            *held = Value::Null;
        }
    }
    if let Some(years) = answer.get_mut("years").and_then(Value::as_array_mut) {
        for year in years {
            year["bulletins"] = Value::Null;
        }
    }
    answer
}

#[tokio::test]
async fn the_offline_engine_answers_overview_questions_as_the_api_does() {
    for with_shop in [true, false] {
        // The API says its pages are where the engine says they are.
        let app = if with_shop {
            common::recalls_app_at(wenmar_shopdata::answer::SITE).await
        } else {
            common::app_with(|config| {
                config.base_url = wenmar_shopdata::answer::SITE.to_owned();
            })
            .await
        };
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
        if with_shop {
            let opened = ask_with_shop(&mut engine, &connection, &shop, "open_shop", &json!({}));
            assert!(opened.get("ok").is_some(), "{opened}");
        }

        for (path, op, args) in &overview_questions() {
            let (status, served) = app.json(path).await;
            // With no shop file opened the engine asks for none of its rows:
            // `ask` would stop at a statement for a second database.
            let answer = if with_shop {
                ask_with_shop(&mut engine, &connection, &shop, op, args)
            } else {
                ask(&mut engine, &connection, op, args)
            };
            if status == StatusCode::OK {
                assert_eq!(
                    answer["ok"],
                    as_the_files_give_it(served.clone()),
                    "{path} {with_shop}"
                );
                if *op == "model_years" {
                    assert_eq!(answer["ok"]["may_apply"], true, "{path}");
                    assert_eq!(answer["ok"]["lookup"]["nhtsa"], served["lookup"]["nhtsa"]);
                    assert_eq!(answer["ok"]["lookup"]["tc"], served["lookup"]["tc"]);
                } else {
                    // A recall block only with a shop file and a model year.
                    assert_eq!(
                        answer["ok"]["recalls"].is_object(),
                        with_shop && served["vehicle"].is_object(),
                        "{path}"
                    );
                    for block in HOSTED_ONLY {
                        assert_eq!(answer["ok"][block], Value::Null, "{path} {block}");
                    }
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
}

#[tokio::test]
async fn the_offline_engine_answers_safety_questions_as_the_api_does() {
    // Three servers, and the files that go with each: the ratings beside
    // the recalls, the recalls alone, and no shop file.
    for held in ["ratings", "recalls", "nothing"] {
        let files = match held {
            "ratings" => Some((
                common::RECALL_AND_RATING_DATASETS,
                format!("{}{}", common::recall_rows(), common::rating_rows()),
            )),
            "recalls" => Some((common::RECALL_DATASETS, common::recall_rows())),
            _ => None,
        };
        let app = match &files {
            Some((datasets, rows)) => common::app_with_shop(datasets, rows).await,
            None => common::app().await,
        };
        let fixture = common::data_file();
        // A shop file is written either way. With none on the server it is
        // not opened, so the engine has none either.
        let (datasets, rows) = files
            .clone()
            .unwrap_or((common::RECALL_DATASETS, String::new()));
        common::add_shop_file(
            &fixture,
            wenmar_shopdata::schema::SCHEMA_VERSION,
            "2026.09",
            datasets,
            &rows,
        );
        let connection = Connection::open(fixture.path()).unwrap();
        let shop = Connection::open(fixture.shop_path()).unwrap();
        let mut engine = Engine::new();
        let opened = ask(&mut engine, &connection, "open", &json!({}));
        assert!(opened.get("ok").is_some(), "{opened}");
        // With no shop file on the server, none is opened here either.
        if files.is_some() {
            let opened = ask_with_shop(&mut engine, &connection, &shop, "open_shop", &json!({}));
            assert!(opened.get("ok").is_some(), "{opened}");
        }

        for id in [
            "2023_hyundai_kona",
            "2023_hyundai_kona_se",
            "2022_hyundai_kona",
            "2019_ford_f-150",
            "2019_honda_civic",
            "2019_honda_nothing",
        ] {
            let (status, served) = app.json(&format!("/v1/vehicles/{id}/safety")).await;
            let args = json!({ "id": id });
            let answer = if files.is_some() {
                ask_with_shop(&mut engine, &connection, &shop, "safety", &args)
            } else {
                ask(&mut engine, &connection, "safety", &args)
            };
            if status == StatusCode::OK {
                assert_eq!(held, "ratings", "{id}");
                // Nothing is set aside: the files give every field.
                assert_eq!(answer["ok"], served, "{id}");
            } else {
                assert_eq!(json!({ "error": answer["error"] }), served, "{id} {held}");
                assert_eq!(status, StatusCode::NOT_FOUND, "{id} {held}");
                if held != "ratings" {
                    assert_eq!(answer["error"]["code"], "dataset_unavailable", "{id}");
                }
            }
        }
    }
}

#[tokio::test]
async fn the_offline_engine_gives_an_overview_its_crash_test_ratings_as_the_api_does() {
    // A server and a shop file that hold the ratings beside the recalls.
    let app = common::ratings_and_recalls_app_at(wenmar_shopdata::answer::SITE).await;
    let fixture = common::data_file();
    common::add_shop_file(
        &fixture,
        wenmar_shopdata::schema::SCHEMA_VERSION,
        "2026.09",
        common::RECALL_AND_RATING_DATASETS,
        &format!("{}{}", common::recall_rows(), common::rating_rows()),
    );
    let connection = Connection::open(fixture.path()).unwrap();
    let shop = Connection::open(fixture.shop_path()).unwrap();
    let mut engine = Engine::new();
    let opened = ask(&mut engine, &connection, "open", &json!({}));
    assert!(opened.get("ok").is_some(), "{opened}");
    let opened = ask_with_shop(&mut engine, &connection, &shop, "open_shop", &json!({}));
    assert!(opened.get("ok").is_some(), "{opened}");

    for (path, op, args) in &overview_questions() {
        let (status, served) = app.json(path).await;
        let answer = ask_with_shop(&mut engine, &connection, &shop, op, args);
        if status == StatusCode::OK {
            // `safety` is compared, not set aside.
            assert_eq!(answer["ok"], as_the_files_give_it(served.clone()), "{path}");
            if *op != "model_years" {
                // The ratings come with a model year, and with nothing else.
                assert_eq!(
                    answer["ok"]["safety"].is_object(),
                    served["vehicle"].is_object(),
                    "{path}"
                );
            }
        } else {
            assert_eq!(json!({ "error": answer["error"] }), served, "{path}");
        }
    }
    // The block is the answer of the route, whole.
    let overview = ask_with_shop(
        &mut engine,
        &connection,
        &shop,
        "overview_vehicle",
        &json!({ "id": "2023_hyundai_kona" }),
    );
    let (_, safety) = app.json("/v1/vehicles/2023_hyundai_kona/safety").await;
    assert_eq!(overview["ok"]["safety"], safety);
    assert_eq!(safety["ratings"].as_array().unwrap().len(), 3);
}

#[tokio::test]
async fn the_offline_engine_answers_fuel_specs_fluids_and_tires_as_the_api_does() {
    let hand = format!("{}{}", common::fuel_rows(), common::hand_rows());
    // Three servers, and the files that go with each: everything, the
    // recalls alone, and no shop file.
    for held in ["everything", "recalls", "nothing"] {
        let files: Option<(&str, String)> = match held {
            "everything" => Some((common::HAND_DATASETS, hand.clone())),
            "recalls" => Some((common::RECALL_DATASETS, common::recall_rows())),
            _ => None,
        };
        let app = match &files {
            Some((datasets, rows)) => {
                common::app_with_rows_and_shop(common::TESTMAKE_ROWS, datasets, rows).await
            }
            None => common::app_with_rows(common::TESTMAKE_ROWS).await,
        };
        let fixture = common::data_file();
        let connection = Connection::open(fixture.path()).unwrap();
        connection.execute_batch(common::TESTMAKE_ROWS).unwrap();
        let (datasets, rows) = files
            .clone()
            .unwrap_or((common::RECALL_DATASETS, String::new()));
        common::add_shop_file(
            &fixture,
            wenmar_shopdata::schema::SCHEMA_VERSION,
            "2026.09",
            datasets,
            &rows,
        );
        let shop = Connection::open(fixture.shop_path()).unwrap();
        let mut engine = Engine::new();
        let opened = ask(&mut engine, &connection, "open", &json!({}));
        assert!(opened.get("ok").is_some(), "{opened}");
        if files.is_some() {
            let opened = ask_with_shop(&mut engine, &connection, &shop, "open_shop", &json!({}));
            assert!(opened.get("ok").is_some(), "{opened}");
        }

        // Each question as a path of the API, and as an operation.
        let mut questions: Vec<(String, String, Value)> = Vec::new();
        for topic in ["fuel", "specs", "fluids", "tires"] {
            for id in [
                "2023_hyundai_kona",
                "2023_hyundai_kona_se",
                "2001_testmake_sampler",
                "2019_honda_civic",
                "2019_ford_f-150",
                "2019_honda_nothing",
            ] {
                questions.push((
                    format!("/v1/vehicles/{id}/{topic}"),
                    format!("{topic}_vehicle"),
                    json!({ "id": id }),
                ));
            }
            for vin in [common::KONA, common::COACH, common::KONA_BAD_CHECK] {
                questions.push((
                    format!("/v1/vin/{vin}/{topic}"),
                    format!("{topic}_vin"),
                    json!({ "vin": vin }),
                ));
            }
            questions.push((
                format!("/v1/vin/{}/{topic}?year=2022", common::KONA),
                format!("{topic}_vin"),
                json!({ "vin": common::KONA, "year": 2022 }),
            ));
        }
        for country in ["us", "CA", "mx"] {
            questions.push((
                format!("/v1/vehicles/2023_hyundai_kona/fuel?country={country}"),
                "fuel_vehicle".to_owned(),
                json!({ "id": "2023_hyundai_kona", "country": country }),
            ));
            questions.push((
                format!("/v1/vin/{}/fuel?country={country}", common::KONA),
                "fuel_vin".to_owned(),
                json!({ "vin": common::KONA, "country": country }),
            ));
        }

        let mut answered = 0;
        for (path, op, args) in questions {
            let (status, served) = app.json(&path).await;
            let answer = if files.is_some() {
                ask_with_shop(&mut engine, &connection, &shop, &op, &args)
            } else {
                ask(&mut engine, &connection, &op, &args)
            };
            if status == StatusCode::OK {
                assert_eq!(held, "everything", "{path}");
                // Nothing is set aside: the files give every field.
                assert_eq!(answer["ok"], served, "{path}");
                answered += 1;
            } else {
                assert_eq!(json!({ "error": answer["error"] }), served, "{path} {held}");
            }
        }
        if held == "everything" {
            // All but the ids that name no vehicle and the country that is
            // neither.
            assert_eq!(answered, 40);

            // The overview's blocks, whole.
            for (path, op, args) in [
                (
                    "/v1/vehicles/2001_testmake_sampler/overview".to_owned(),
                    "overview_vehicle",
                    json!({ "id": "2001_testmake_sampler" }),
                ),
                (
                    "/v1/vehicles/2023_hyundai_kona/overview".to_owned(),
                    "overview_vehicle",
                    json!({ "id": "2023_hyundai_kona" }),
                ),
                (
                    format!("/v1/vin/{}/overview", common::KONA),
                    "overview_vin",
                    json!({ "vin": common::KONA }),
                ),
            ] {
                let (status, served) = app.json(&path).await;
                assert_eq!(status, StatusCode::OK, "{path}");
                let answer = ask_with_shop(&mut engine, &connection, &shop, op, &args);
                for block in ["fuel", "specs", "fluids", "tires", "attribution"] {
                    assert_eq!(answer["ok"][block], served[block], "{path} {block}");
                }
                assert!(answer["ok"]["fluids"].is_object(), "{path}");
            }
        }
    }
}

#[tokio::test]
async fn the_offline_engine_gives_what_owners_report_as_the_api_does() {
    // Three servers, and the files that go with each: the complaints beside
    // the recalls, the recalls alone, and no shop file.
    for held in ["complaints", "recalls", "nothing"] {
        let files = match held {
            "complaints" => Some((common::COMPLAINT_DATASETS, common::complaint_rows())),
            "recalls" => Some((common::RECALL_DATASETS, common::recall_rows())),
            _ => None,
        };
        // Its addresses are the engine's, as the overview test's are.
        let app = match (held, &files) {
            ("complaints", _) => common::complaints_app_at(wenmar_shopdata::answer::SITE).await,
            (_, Some((datasets, rows))) => common::app_with_shop(datasets, rows).await,
            (_, None) => common::app().await,
        };
        let fixture = common::data_file();
        // A shop file is written either way. With none on the server it is
        // not opened, so the engine has none either.
        let (datasets, rows) = files
            .clone()
            .unwrap_or((common::RECALL_DATASETS, String::new()));
        common::add_shop_file(
            &fixture,
            wenmar_shopdata::schema::SCHEMA_VERSION,
            "2026.09",
            datasets,
            &rows,
        );
        let connection = Connection::open(fixture.path()).unwrap();
        let shop = Connection::open(fixture.shop_path()).unwrap();
        let mut engine = Engine::new();
        let opened = ask(&mut engine, &connection, "open", &json!({}));
        assert!(opened.get("ok").is_some(), "{opened}");
        // With no shop file on the server, none is opened here either.
        if files.is_some() {
            let opened = ask_with_shop(&mut engine, &connection, &shop, "open_shop", &json!({}));
            assert!(opened.get("ok").is_some(), "{opened}");
        }

        for id in [
            "2023_hyundai_kona",
            "2022_hyundai_kona",
            "2019_honda_civic",
            "2019_honda_nothing",
        ] {
            let (status, served) = app.json(&format!("/v1/vehicles/{id}/complaints")).await;
            let args = json!({ "id": id });
            let answer = if files.is_some() {
                ask_with_shop(&mut engine, &connection, &shop, "complaints", &args)
            } else {
                ask(&mut engine, &connection, "complaints", &args)
            };
            if status == StatusCode::OK {
                assert_eq!(held, "complaints", "{id}");
                // Nothing is set aside: the files give every field.
                assert_eq!(answer["ok"], served, "{id}");
                // The overview's block is the same answer, in brief.
                let (_, overview) = app.json(&format!("/v1/vehicles/{id}/overview")).await;
                let asked =
                    ask_with_shop(&mut engine, &connection, &shop, "overview_vehicle", &args);
                assert_eq!(asked["ok"], overview, "overview {id}");
                assert!(overview["complaints"].is_object(), "{id}");
            } else {
                assert_eq!(json!({ "error": answer["error"] }), served, "{id} {held}");
                assert_eq!(status, StatusCode::NOT_FOUND, "{id} {held}");
                if held != "complaints" {
                    assert_eq!(answer["error"]["code"], "dataset_unavailable", "{id}");
                }
            }
        }
    }
}
