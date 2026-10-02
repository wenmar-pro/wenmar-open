//! A small data file and a database to read it with.
//!
//! The rows are the ones the server's and the adapter's tests use, so every
//! crate's tests describe the same vehicles, with a bus added whose model
//! year cannot be settled from the VIN alone.

#![allow(dead_code)]

use rusqlite::types::{Value as Sql, ValueRef};
use rusqlite::{Connection, params_from_iter};
use serde_json::{Value, json};
use wenmar_open_wasm::Engine;

/// The year the tests run in. A decode's candidate years depend on it.
pub const YEAR: u16 = 2026;

/// A Hyundai Kona, model year 2023. Its check digit is right.
pub const KONA: &str = "KM8K2CAB4PU001140";
/// The same VIN with a wrong check digit.
pub const KONA_BAD_CHECK: &str = "KM8K2CAB0PU001140";
/// No manufacturer is registered for `ZZZ`.
pub const UNKNOWN: &str = "ZZZK2CAB4PU001140";
/// A bus. Position 10 is `M`: 1991 or 2021, and a bus's position 7 does not
/// say which. The 2021 patterns explain more of this one.
pub const BUS_LATE: &str = "1M8PDMPA1MP000001";
/// The same, for a bus the 1991 patterns explain more of.
pub const BUS_EARLY: &str = "1M8PEMPA5MP000001";
/// A trailer from a low-volume maker, found by its six-character code.
pub const TRAILER: &str = "1A9100AB0L1881001";

const DECODER_ROWS: &str = "
INSERT INTO meta VALUES
  ('data_version', '2026.09'),
  ('vpic_release', 'vPICList_lite_2026_09'),
  ('built_at', '2026-10-01 04:25:57');
INSERT INTO wmi VALUES
  ('KM8', 'Hyundai Motor Co', 'Hyundai', 'South Korea', 'Multipurpose Passenger Vehicle (MPV)', 1, 7),
  ('1M8', 'Motor Coach Industries', NULL, 'United States (USA)', 'Bus', 0, 5),
  ('1A9', 'Many Small Makers', NULL, 'United States (USA)', 'Trailer', 0, 6),
  ('1A9881', 'Ranger Trailer Works', 'Ranger Trailers', 'United States (USA)', 'Trailer', 0, 6);
INSERT INTO wmi_make VALUES ('KM8', 498), ('1A9881', 5000);
INSERT INTO wmi_schema VALUES
  ('KM8', 1, 2022, NULL), ('KM8', 2, 1990, 1995),
  ('1M8', 3, 1990, 1999), ('1M8', 4, 2020, NULL),
  ('1A9881', 5, 2000, NULL);
INSERT INTO pattern VALUES
  (10, 1, 'K2***', 28, 'Kona', '2022-05-01 00:00:00', 'Hyundai', '900'),
  (11, 1, 'K[2-3]CA', 38, 'SE', '2022-05-01 00:00:00', NULL, 'SE'),
  (12, 1, 'K9***', 38, 'Does Not Match', '2022-05-01 00:00:00', NULL, 'x'),
  (13, 1, '*****|*U', 31, 'Ulsan', '2022-05-01 00:00:00', NULL, 'Ulsan'),
  (14, 1, 'K2***', 96, 'Internal Element', '2022-05-01 00:00:00', NULL, 'x'),
  (15, 2, 'K2***', 28, 'Old Model', '1995-01-01 00:00:00', 'Hyundai', '901'),
  (16, 1, 'K2***', 18, 'G4NH', '2022-05-01 00:00:00', NULL, 'G4NH'),
  (17, 1, 'K2***', 13, '2.0', '2022-05-01 00:00:00', NULL, '2.0'),
  (18, 1, 'K2***', 5, 'Sport Utility Vehicle (SUV)/Multi-Purpose Vehicle (MPV)', '2022-05-01 00:00:00', NULL, '7'),
  (20, 3, 'PD***', 28, 'D-Series', '1996-01-01 00:00:00', 'MCI', '950'),
  (21, 4, 'PD***', 28, 'D-Series', '2021-01-01 00:00:00', 'MCI', '950'),
  (22, 4, 'PDM**', 5, 'Bus', '2021-01-01 00:00:00', NULL, '16'),
  (23, 4, 'PDMP*', 15, '6x4', '2021-01-01 00:00:00', NULL, '6'),
  (25, 3, 'PE***', 28, 'E-Series', '1996-01-01 00:00:00', 'MCI', '951'),
  (26, 3, 'PE***', 5, 'Bus', '1996-01-01 00:00:00', NULL, '16'),
  (27, 3, 'PE***', 9, '6', '1996-01-01 00:00:00', NULL, '6'),
  (28, 4, 'PE***', 28, 'E-Series', '2021-01-01 00:00:00', 'MCI', '951'),
  (30, 5, '100**', 28, 'Tilt Deck', '2001-01-01 00:00:00', 'Ranger Trailers', '9100');
INSERT INTO spec_schema VALUES (50, 498, 7), (51, 498, 7), (52, 999, 7), (53, 498, 5);
INSERT INTO spec_schema_model VALUES (50, 900), (51, 900), (52, 900), (53, 900);
INSERT INTO spec_schema_year VALUES (50, 2023), (51, 2021);
INSERT INTO spec_row VALUES
  (1, 10, 50, 1, 38, 'SE', 'SE', '2023-01-01 00:00:00'),
  (2, 10, 50, 0, 86, '1', 'Standard', '2023-01-01 00:00:00'),
  (3, 11, 51, 0, 86, '2', 'Other Year', '2023-01-01 00:00:00'),
  (4, 12, 52, 0, 86, '2', 'Other Make', '2023-01-01 00:00:00'),
  (5, 13, 53, 0, 86, '2', 'Other Vehicle Type', '2023-01-01 00:00:00'),
  (6, 10, 50, 0, 168, '1', 'Direct', '2023-01-01 00:00:00'),
  (7, 10, 50, 0, 37, '2', 'Automatic', '2023-01-01 00:00:00');
INSERT INTO engine_model_row VALUES
  (1, 'g4nh', 9, '4', '4', '2020-01-01 00:00:00'),
  (2, 'g4nh', 24, '4', 'Gasoline', '2020-01-01 00:00:00'),
  (3, 'other', 9, '8', '8', '2020-01-01 00:00:00');
";

const CATALOG_ROWS: &str = "
INSERT INTO catalog_type VALUES
  (2, 'Passenger Car'), (3, 'Truck'), (5, 'Bus'), (6, 'Trailer'),
  (7, 'Multipurpose Passenger Vehicle (MPV)');
INSERT INTO catalog_make VALUES
  (460, 'ford', 'Ford', 'ford', 2, 8, 1),
  (467, 'chevrolet', 'Chevrolet', 'chevrolet', 3, 8, 1),
  (474, 'honda', 'Honda', 'honda', 4, 132, 1),
  (498, 'hyundai', 'Hyundai', 'hyundai', 12, 132, 1),
  (5000, 'ranger-trailers', 'Ranger Trailers', 'rangertrailers', NULL, 64, 0);
INSERT INTO catalog_alias VALUES ('chevy', 467);
INSERT INTO catalog_rename VALUES ('se plus', 'SE');
INSERT INTO catalog_model VALUES
  (900, 498, 'kona', 'Kona', 'kona', 2022, 2023, 128, 1),
  (1801, 460, 'f-150', 'F-150', 'f150', 2019, 2019, 8, 1),
  (1850, 467, 'silverado', 'Silverado', 'silverado', 2019, 2019, 8, 1),
  (1863, 474, 'civic', 'Civic', 'civic', 2018, 2020, 4, 1),
  (1865, 474, 'cr-v', 'CR-V', 'crv', 2019, 2019, 128, 1),
  (9100, 5000, 'tilt-deck', 'Tilt Deck', 'tiltdeck', 2019, 2020, 64, 0);
INSERT INTO catalog_vehicle VALUES
  (1, 2018, 474, 1863, 4, 1, 1),
  (2, 2019, 460, 1801, 8, 1, 2),
  (3, 2019, 467, 1850, 8, 1, 3),
  (4, 2019, 474, 1863, 4, 1, 1),
  (5, 2019, 474, 1865, 128, 1, NULL),
  (6, 2019, 5000, 9100, 64, 0, NULL),
  (7, 2020, 474, 1863, 4, 1, 4),
  (8, 2022, 498, 900, 128, 1, 5),
  (9, 2023, 498, 900, 128, 1, 5),
  (11, 2020, 5000, 9100, 64, 0, NULL);
INSERT INTO catalog_detail VALUES
  (1, NULL, 'FWD', NULL),
  (2, 'Pickup', NULL, NULL),
  (3, NULL, NULL, NULL),
  (4, 'Sedan', 'FWD', 'CVT'),
  (5, 'SUV', NULL, NULL);
INSERT INTO catalog_submodel VALUES
  (1, 1, 'LX', 'lx', 'trim', 1, NULL, NULL, NULL),
  (2, 1, 'Si', 'si', 'trim', 1, 'Sedan', NULL, 'Manual'),
  (3, 1, 'Touring', 'touring', 'trim', 1, NULL, NULL, 'CVT'),
  (4, 2, 'Raptor', 'raptor', 'trim', 1, NULL, '4WD', NULL),
  (5, 2, 'XLT', 'xlt', 'preset', 1, NULL, NULL, NULL),
  (7, 3, 'LT', 'lt', 'trim', 1, NULL, NULL, NULL),
  (8, 3, '1500', '1500', 'series', 0, NULL, NULL, NULL),
  (10, 4, 'LX', 'lx', 'trim', 1, NULL, NULL, NULL),
  (11, 5, 'SE', 'se', 'trim', 1, NULL, NULL, NULL),
  (12, 5, 'Limited', 'limited', 'trim', 1, NULL, NULL, NULL);
INSERT INTO catalog_engine VALUES
  (1, 1, '1.5L Turbo', NULL, 'vpic'),
  (2, 1, '2.0L', NULL, 'vpic'),
  (3, 2, '3.5L Turbo V6', '4G', 'vpic'),
  (4, 2, '5.0L V8', '5', 'vpic'),
  (5, 3, '5.3L V8', 'CR', 'vpic'),
  (7, 4, '2.0L', NULL, 'preset'),
  (8, 5, '2.0L', 'A', 'vpic'),
  (9, 5, '1.6L Turbo', NULL, 'vpic');
INSERT INTO catalog_submodel_engine VALUES (2, 1);
";

/// The whole data file as SQL: the two libraries' own table definitions,
/// then the rows. `clients/js` loads the same text into `node:sqlite`.
pub fn fixture_sql(schema_version: &str) -> String {
    format!(
        "{}\nINSERT INTO meta VALUES ('schema_version', '{schema_version}');\n{DECODER_ROWS}\n{}\n{CATALOG_ROWS}",
        wenmar_vin::sqlite::SCHEMA.trim(),
        wenmar_vehicles::schema::SCHEMA.trim(),
    )
}

/// The data file, in memory, at this build's schema version.
pub fn data() -> Connection {
    data_at(wenmar_open_wasm::SCHEMA_VERSION)
}

pub fn data_at(schema_version: &str) -> Connection {
    let connection = Connection::open_in_memory().unwrap();
    connection
        .execute_batch(&fixture_sql(schema_version))
        .unwrap();
    connection
}

fn bind(cell: &Value) -> Sql {
    match cell {
        Value::Null => Sql::Null,
        Value::Number(number) => number
            .as_i64()
            .map_or_else(|| Sql::Real(number.as_f64().unwrap_or(0.0)), Sql::Integer),
        Value::String(text) => Sql::Text(text.clone()),
        other => panic!("a statement asked to bind {other}"),
    }
}

/// Runs one statement the engine asked for, as a store would.
pub fn rows(connection: &Connection, statement: &Value) -> Value {
    let sql = statement["sql"].as_str().unwrap();
    let params: Vec<Sql> = statement["params"]
        .as_array()
        .unwrap()
        .iter()
        .map(bind)
        .collect();
    let mut prepared = connection.prepare(sql).unwrap();
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

/// What a request came to, and what it took.
pub struct Run {
    /// `{ "ok": ... }` or `{ "error": ... }`.
    pub answer: Value,
    /// How many times the database was gone back to.
    pub steps: usize,
    /// Every statement that was run, in order.
    pub statements: Vec<Value>,
}

/// Sends a request until it is answered, running what it needs in between,
/// the way the npm package does.
pub fn run(engine: &mut Engine, connection: &Connection, op: &str, args: Value) -> Run {
    let mut answers: Vec<Value> = Vec::new();
    let mut statements = Vec::new();
    for steps in 0..32 {
        let request = json!({ "op": op, "args": args, "current_year": YEAR, "answers": answers });
        let answer: Value = serde_json::from_str(&engine.call(&request.to_string())).unwrap();
        let Some(need) = answer.get("need").and_then(Value::as_array) else {
            return Run {
                answer,
                steps,
                statements,
            };
        };
        assert!(!need.is_empty(), "asked for nothing");
        for statement in need {
            answers.push(json!({
                "sql": statement["sql"],
                "params": statement["params"],
                "rows": rows(connection, statement),
            }));
            statements.push(statement.clone());
        }
    }
    panic!("{op} did not finish in 32 steps");
}

/// An engine with the data opened.
pub fn opened(connection: &Connection) -> Engine {
    let mut engine = Engine::new();
    let run = run(&mut engine, connection, "open", json!({}));
    assert!(run.answer.get("ok").is_some(), "{}", run.answer);
    engine
}

/// Questions with a name each, asked of the fixture by every test that
/// compares answers: here against the libraries, and in `clients/js`
/// against the hosted client.
pub fn cases() -> Vec<(&'static str, &'static str, Value)> {
    let long = "K".repeat(70);
    vec![
        ("decode", "decode", json!({ "vin": KONA })),
        (
            "decode, typed loosely",
            "decode",
            json!({ "vin": "km8k2-cab4 pu001140" }),
        ),
        (
            "decode, wrong check digit",
            "decode",
            json!({ "vin": KONA_BAD_CHECK }),
        ),
        (
            "decode, year given",
            "decode",
            json!({ "vin": KONA, "year": 1993 }),
        ),
        (
            "decode, year out of range",
            "decode",
            json!({ "vin": KONA, "year": 1900 }),
        ),
        (
            "decode, unknown manufacturer",
            "decode",
            json!({ "vin": UNKNOWN }),
        ),
        (
            "decode, illegal character",
            "decode",
            json!({ "vin": "KM8K2CAB4PUO01140" }),
        ),
        ("decode, too short", "decode", json!({ "vin": "KM8" })),
        ("decode, far too long", "decode", json!({ "vin": long })),
        ("decode, bus of 2021", "decode", json!({ "vin": BUS_LATE })),
        ("decode, bus of 1991", "decode", json!({ "vin": BUS_EARLY })),
        (
            "decode, low-volume maker",
            "decode",
            json!({ "vin": TRAILER }),
        ),
        ("years", "years", json!({})),
        (
            "years, narrowed",
            "years",
            json!({ "scope": "all", "term": "201" }),
        ),
        ("makes", "makes", json!({})),
        (
            "makes, of a year",
            "makes",
            json!({ "year": 2019, "term": "ch", "limit": 5 }),
        ),
        ("makes, bad scope", "makes", json!({ "scope": "nope" })),
        ("models", "models", json!({ "make": "honda" })),
        (
            "models, by alias and year",
            "models",
            json!({ "make": "chevy", "year": 2019 }),
        ),
        ("models, no make", "models", json!({})),
        (
            "submodels",
            "submodels",
            json!({ "make": "honda", "model": "civic", "year": 2019 }),
        ),
        (
            "submodels, no year",
            "submodels",
            json!({ "make": "honda", "model": "civic" }),
        ),
        (
            "engines, of a submodel",
            "engines",
            json!({ "make": "honda", "model": "civic", "year": 2019, "submodel": "si" }),
        ),
        (
            "engines",
            "engines",
            json!({ "make": "ford", "model": "f150", "year": 2019 }),
        ),
        ("search", "search", json!({ "q": "2019 civic si" })),
        (
            "search, a series of a make",
            "search",
            json!({ "q": "chevy 1500", "limit": 50 }),
        ),
        ("search, a make alone", "search", json!({ "q": "honda" })),
        ("search, nothing", "search", json!({ "q": "zzzz" })),
        ("search, no text", "search", json!({})),
        ("vehicle", "vehicle", json!({ "id": "2019_honda_civic_si" })),
        (
            "vehicle, unknown",
            "vehicle",
            json!({ "id": "2019_honda_nothing" }),
        ),
        ("meta", "meta", json!({})),
    ]
}
