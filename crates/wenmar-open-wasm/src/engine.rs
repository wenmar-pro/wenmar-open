//! One request in, one answer out.

use std::collections::HashMap;

use serde_json::{Map, Value, json};
use wenmar_vehicles::{Catalog, Source};

use crate::SCHEMA_VERSION;
use crate::cells::{from_json, to_json};
use crate::error::{DATA_INVALID, OpError};
use crate::ops;
use crate::replay::{Answer, Replay, Statement};

/// Every row of `meta`: the data file's versions.
pub const META_SQL: &str = "SELECT key, value FROM meta";

/// What `meta` says about an opened data file.
#[derive(Debug, Clone, PartialEq, Eq)]
struct Meta {
    data_version: String,
    vpic_release: String,
    built_at: String,
}

#[derive(Debug)]
struct Opened {
    catalog: Catalog<Replay>,
    meta: Meta,
}

/// The two libraries over a [`Replay`], answering requests written as JSON.
///
/// A request is `{ "op", "args", "current_year", "answers" }`. The answer is
/// one of:
///
/// - `{ "need": [{ "sql", "params" }, ...] }`: run these statements and send
///   the request again with their rows added to `answers`, each as
///   `{ "sql", "params", "rows" }`;
/// - `{ "ok": ... }`: the answer, as the hosted API would send it;
/// - `{ "error": { "code", "message", "details" } }`.
///
/// `open` must succeed before any other operation but `version`. It reads
/// the makes once and keeps them, as the server does at startup. Nothing
/// else is kept from one request to the next.
#[derive(Debug, Default)]
pub struct Engine {
    replay: Replay,
    opened: Option<Opened>,
}

fn statement_json(statement: &Statement) -> Value {
    json!({
        "sql": statement.sql,
        "params": statement.params.iter().map(to_json).collect::<Vec<_>>(),
    })
}

fn bad_request(what: &str) -> OpError {
    OpError::internal(format!("the request is not one this build reads: {what}"))
}

/// The rows the caller sends back. A value that is neither a number, text
/// nor null is refused: it came from a database that returns values oddly.
fn answers(request: &Map<String, Value>) -> Result<Vec<Answer>, OpError> {
    let Some(list) = request.get("answers") else {
        return Ok(Vec::new());
    };
    let list = list.as_array().ok_or_else(|| bad_request("answers"))?;
    let odd = || {
        OpError::new(
            DATA_INVALID,
            "The database returned a value that is not a number, text or null.",
        )
    };
    let mut found = Vec::with_capacity(list.len());
    for answer in list {
        let sql = answer
            .get("sql")
            .and_then(Value::as_str)
            .ok_or_else(|| bad_request("answers[].sql"))?;
        let params = answer
            .get("params")
            .and_then(Value::as_array)
            .ok_or_else(|| bad_request("answers[].params"))?
            .iter()
            .map(|cell| from_json(cell).ok_or_else(|| bad_request("answers[].params")))
            .collect::<Result<Vec<_>, _>>()?;
        let rows = answer
            .get("rows")
            .and_then(Value::as_array)
            .ok_or_else(odd)?
            .iter()
            .map(|row| {
                row.as_array()
                    .ok_or_else(odd)?
                    .iter()
                    .map(|cell| from_json(cell).ok_or_else(odd))
                    .collect::<Result<Vec<_>, _>>()
            })
            .collect::<Result<Vec<_>, _>>()?;
        found.push(Answer {
            statement: Statement {
                sql: sql.to_owned(),
                params,
            },
            rows,
        });
    }
    Ok(found)
}

impl Engine {
    pub fn new() -> Engine {
        Engine::default()
    }

    /// Answers one request. It never panics by intent, whatever the text.
    pub fn call(&mut self, request: &str) -> String {
        let answer = match self.answer(request) {
            Ok(Outcome::Done(value)) => json!({ "ok": value }),
            Ok(Outcome::Need(statements)) => {
                json!({ "need": statements.iter().map(statement_json).collect::<Vec<_>>() })
            }
            Err(error) => json!({ "error": error.body() }),
        };
        answer.to_string()
    }

    fn answer(&mut self, request: &str) -> Result<Outcome, OpError> {
        let request: Value =
            serde_json::from_str(request).map_err(|_| bad_request("it is not JSON"))?;
        let request = request
            .as_object()
            .ok_or_else(|| bad_request("it is not an object"))?;
        let op = request
            .get("op")
            .and_then(Value::as_str)
            .ok_or_else(|| bad_request("op"))?;
        let empty = json!({});
        let args = request.get("args").unwrap_or(&empty);
        self.replay.load(answers(request)?);
        let result = self.run(op, args, request.get("current_year"));
        // A statement went unanswered, so whatever came out was worked out
        // on partial rows. Only the list of what is missing is of any use.
        let missing = self.replay.finish();
        if !missing.is_empty() {
            return Ok(Outcome::Need(missing));
        }
        result.map(Outcome::Done)
    }

    fn run(
        &mut self,
        op: &str,
        args: &Value,
        current_year: Option<&Value>,
    ) -> Result<Value, OpError> {
        match op {
            "version" => {
                return Ok(json!({
                    "version": env!("CARGO_PKG_VERSION"),
                    "schema_version": SCHEMA_VERSION,
                }));
            }
            "open" => return self.open(),
            _ => {}
        }
        let Some(opened) = &self.opened else {
            return Err(OpError::internal(
                "the data must be opened before it is read",
            ));
        };
        let catalog = &opened.catalog;
        match op {
            "meta" => Ok(json!({
                "data_version": opened.meta.data_version,
                "vpic_release": opened.meta.vpic_release,
                "built_at": opened.meta.built_at,
                "server_version": env!("CARGO_PKG_VERSION"),
            })),
            "decode" => {
                // This build has no clock, and the decoder would stop the
                // whole module if it reached for one.
                let current_year = current_year
                    .and_then(Value::as_u64)
                    .and_then(|year| u16::try_from(year).ok())
                    .ok_or_else(|| bad_request("current_year"))?;
                ops::decode(catalog, &self.replay, args, current_year)
            }
            "years" => ops::years(catalog, args),
            "makes" => ops::makes(catalog, args),
            "models" => ops::models(catalog, args),
            "submodels" => ops::submodels(catalog, args),
            "engines" => ops::engines(catalog, args),
            "search" => ops::search(catalog, args),
            "vehicle" => ops::vehicle(catalog, args),
            _ => Err(bad_request("op")),
        }
    }

    /// Checks the data file's layout and reads what the catalog keeps in
    /// memory. The statements of both are asked for together.
    fn open(&mut self) -> Result<Value, OpError> {
        self.opened = None;
        let rows = self
            .replay
            .query(META_SQL, &[])
            .map_err(|error| OpError::new(DATA_INVALID, error.to_string()))?;
        let catalog = Catalog::new(self.replay.clone());
        if self.replay.incomplete() {
            return Ok(Value::Null);
        }
        let mut meta: HashMap<&str, &str> = HashMap::new();
        for row in &rows {
            if let (Some(key), Some(value)) = (
                row.first().and_then(|cell| cell.text()),
                row.get(1).and_then(|cell| cell.text()),
            ) {
                meta.insert(key, value);
            }
        }
        match meta.get("schema_version").copied() {
            Some(SCHEMA_VERSION) => {}
            Some(other) => {
                return Err(OpError::new(
                    DATA_INVALID,
                    format!(
                        "The data file has schema version {other}. This version of wenmar-open reads schema version {SCHEMA_VERSION}."
                    ),
                )
                .with_details(json!({ "schema_version": other, "expected": SCHEMA_VERSION })));
            }
            None => {
                return Err(OpError::new(
                    DATA_INVALID,
                    "This is not a Wenmar Open data file: its meta table has no schema_version.",
                )
                .with_details(json!({ "expected": SCHEMA_VERSION })));
            }
        }
        let field = |key: &str| meta.get(key).copied().unwrap_or_default().to_owned();
        let meta = Meta {
            data_version: field("data_version"),
            vpic_release: field("vpic_release"),
            built_at: field("built_at"),
        };
        let answer = json!({
            "data_version": meta.data_version,
            "vpic_release": meta.vpic_release,
            "built_at": meta.built_at,
            "schema_version": SCHEMA_VERSION,
        });
        self.opened = Some(Opened {
            catalog: catalog?,
            meta,
        });
        Ok(answer)
    }
}

enum Outcome {
    Done(Value),
    Need(Vec<Statement>),
}
