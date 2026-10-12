//! One request in, one answer out.

use std::collections::HashMap;

use serde_json::{Map, Value, json};
use wenmar_shopdata::ShopData;
use wenmar_shopdata::answer::SITE;
use wenmar_vehicles::{Catalog, Source};

use crate::cells::{from_json, to_json};
use crate::error::{DATA_INVALID, OpError, shop_invalid};
use crate::ops;
use crate::replay::{Answer, Replay, Statement};
use crate::{SCHEMA_VERSION, SHOP_SCHEMA_VERSION};

/// Every row of `meta`: the data file's versions.
pub const META_SQL: &str = "SELECT key, value FROM meta";

/// What a statement for the shop file is marked with, and the rows sent
/// back for it: `"db": "shop"`. A statement for the data file has no `db`.
pub const SHOP_DB: &str = "shop";

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
///   `{ "sql", "params", "rows" }`. A statement with `"db": "shop"` is run
///   on the shop file, and its rows are sent back with the same `db`;
/// - `{ "ok": ... }`: the answer, as the hosted API would send it;
/// - `{ "error": { "code", "message", "details" } }`.
///
/// `open` must succeed before any other operation but `version`. It reads
/// `meta` first and checks the schema version, then reads the makes once
/// and keeps them, as the server does at startup. Nothing
/// else is kept from one request to the next.
///
/// `open_shop`, after `open`, reads the shop file's `meta` and keeps what
/// it says. Without it the recall operations answer `dataset_unavailable`,
/// as a server with no shop file does, and the overview operations answer
/// with their recall parts `null`.
#[derive(Debug, Default)]
pub struct Engine {
    replay: Replay,
    /// The rows of the shop file. It is a database of its own.
    shop_replay: Replay,
    opened: Option<Opened>,
    shop: Option<ShopData<Replay>>,
}

fn statement_json(statement: &Statement) -> Value {
    json!({
        "sql": statement.sql,
        "params": statement.params.iter().map(to_json).collect::<Vec<_>>(),
    })
}

fn shop_statement_json(statement: &Statement) -> Value {
    let mut value = statement_json(statement);
    value["db"] = json!(SHOP_DB);
    value
}

fn bad_request(what: &str) -> OpError {
    OpError::internal(format!("the request is not one this build reads: {what}"))
}

/// The rows the caller sends back, for the data file and for the shop
/// file. An answer says which it is of with `"db": "shop"`; one with no
/// `db` is of the data file. A value that is neither a number, text nor
/// null is refused: it came from a database that returns values oddly.
fn answers(request: &Map<String, Value>) -> Result<(Vec<Answer>, Vec<Answer>), OpError> {
    let Some(list) = request.get("answers") else {
        return Ok((Vec::new(), Vec::new()));
    };
    let list = list.as_array().ok_or_else(|| bad_request("answers"))?;
    let odd = || {
        OpError::new(
            DATA_INVALID,
            "The database returned a value that is not a number, text or null.",
        )
    };
    let (mut core, mut shop) = (Vec::with_capacity(list.len()), Vec::new());
    for answer in list {
        let of_shop = match answer.get("db") {
            None | Some(Value::Null) => false,
            Some(Value::String(name)) if name == SHOP_DB => true,
            Some(_) => return Err(bad_request("answers[].db")),
        };
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
        let found = Answer {
            statement: Statement {
                sql: sql.to_owned(),
                params,
            },
            rows,
        };
        if of_shop {
            shop.push(found);
        } else {
            core.push(found);
        }
    }
    Ok((core, shop))
}

impl Engine {
    pub fn new() -> Engine {
        Engine::default()
    }

    /// Answers one request. It never panics by intent, whatever the text.
    pub fn call(&mut self, request: &str) -> String {
        let answer = match self.answer(request) {
            Ok(Outcome::Done(value)) => json!({ "ok": value }),
            Ok(Outcome::Need(core, shop)) => {
                let need: Vec<Value> = core
                    .iter()
                    .map(statement_json)
                    .chain(shop.iter().map(shop_statement_json))
                    .collect();
                json!({ "need": need })
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
        let (core, shop) = answers(request)?;
        self.replay.load(core);
        self.shop_replay.load(shop);
        let result = self.run(op, args, request.get("current_year"));
        // A statement went unanswered, so whatever came out was worked out
        // on partial rows. Only the list of what is missing is of any use.
        let missing = self.replay.finish();
        let missing_shop = self.shop_replay.finish();
        if !missing.is_empty() || !missing_shop.is_empty() {
            return Ok(Outcome::Need(missing, missing_shop));
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
                    "shop_schema_version": SHOP_SCHEMA_VERSION,
                }));
            }
            "open" => return self.open(),
            "open_shop" => return self.open_shop(),
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
                // The datasets of the shop file, when one has been opened:
                // what the API lists for a server that has one.
                "datasets": self.shop.as_ref().map_or_else(|| json!({}), |shop| json!(shop.datasets())),
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
            "safety" => {
                let Some(shop) = &self.shop else {
                    return Err(ops::no_ratings());
                };
                let answers = ops::RecallAnswers {
                    shop,
                    catalog,
                    core: &self.replay,
                    base: SITE,
                };
                ops::safety(&answers, args)
            }
            "complaints" => {
                let Some(shop) = &self.shop else {
                    return Err(ops::no_complaints());
                };
                let answers = ops::RecallAnswers {
                    shop,
                    catalog,
                    core: &self.replay,
                    base: SITE,
                };
                ops::complaints(&answers, args)
            }
            _ if ops::topic_of(op).is_some() => {
                let (topic, by_vin) = ops::topic_of(op).unwrap_or(("fuel", false));
                let Some(shop) = &self.shop else {
                    return Err(ops::no_facts(topic, args));
                };
                let answers = ops::RecallAnswers {
                    shop,
                    catalog,
                    core: &self.replay,
                    base: SITE,
                };
                let current_year = current_year
                    .and_then(Value::as_u64)
                    .and_then(|year| u16::try_from(year).ok());
                if by_vin && current_year.is_none() {
                    return Err(bad_request("current_year"));
                }
                ops::facts(&answers, topic, by_vin, args, current_year)
            }
            "overview_vehicle" | "overview_vin" | "model_years" => {
                // These answer with no shop file opened: `shop` is then
                // `None`, and nothing is asked of a second database.
                let overviews = ops::OverviewAnswers {
                    shop: self.shop.as_ref(),
                    catalog,
                    core: &self.replay,
                    base: SITE,
                };
                match op {
                    "overview_vehicle" => ops::overview_vehicle(&overviews, args),
                    "overview_vin" => {
                        let current_year = current_year
                            .and_then(Value::as_u64)
                            .and_then(|year| u16::try_from(year).ok())
                            .ok_or_else(|| bad_request("current_year"))?;
                        ops::overview_vin(&overviews, args, current_year)
                    }
                    _ => ops::model_years(&overviews, args),
                }
            }
            "recalls_vehicle" | "recalls_vin" | "recall" | "recalls_search" | "recalls_recent" => {
                let Some(shop) = &self.shop else {
                    return Err(ops::no_recalls(op, args));
                };
                let answers = ops::RecallAnswers {
                    shop,
                    catalog,
                    core: &self.replay,
                    base: SITE,
                };
                match op {
                    "recalls_vehicle" => ops::recalls_vehicle(&answers, args),
                    "recalls_vin" => {
                        let current_year = current_year
                            .and_then(Value::as_u64)
                            .and_then(|year| u16::try_from(year).ok())
                            .ok_or_else(|| bad_request("current_year"))?;
                        ops::recalls_vin(&answers, args, current_year)
                    }
                    "recall" => ops::recall(&answers, args),
                    "recalls_recent" => ops::recalls_recent(&answers, args),
                    _ => ops::recalls_search(&answers, args),
                }
            }
            _ => Err(bad_request("op")),
        }
    }

    /// Checks the data file's layout and reads what the catalog keeps in
    /// memory, in two reads of the database: `meta` alone, then the
    /// catalog's statements. The catalog's tables came with the schema
    /// version this build reads, so a file of another version is refused
    /// by its version before a table it may lack is named.
    fn open(&mut self) -> Result<Value, OpError> {
        self.opened = None;
        // A shop file goes with one data file, so it is opened again too.
        self.shop = None;
        let rows = self
            .replay
            .query(META_SQL, &[])
            .map_err(|error| OpError::new(DATA_INVALID, error.to_string()))?;
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
        let catalog = Catalog::new(self.replay.clone());
        if self.replay.incomplete() {
            return Ok(Value::Null);
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

    /// Checks the shop file's layout and that it goes with the data file
    /// that is open, in one read of the shop database: its `meta` table.
    /// The data file must be opened first.
    fn open_shop(&mut self) -> Result<Value, OpError> {
        self.shop = None;
        let Some(opened) = &self.opened else {
            return Err(OpError::internal(
                "the data must be opened before the shop file is",
            ));
        };
        // Asked for here first: with no rows yet, the reader would take
        // the file for one that is not a shop file.
        self.shop_replay
            .query(META_SQL, &[])
            .map_err(|error| OpError::new(DATA_INVALID, error.to_string()))?;
        if self.shop_replay.incomplete() {
            return Ok(Value::Null);
        }
        let shop = ShopData::new(self.shop_replay.clone()).map_err(|error| shop_invalid(&error))?;
        shop.check_catalog(&opened.meta.data_version)
            .map_err(|error| shop_invalid(&error))?;
        let answer = json!({
            "data_version": shop.data_version(),
            "catalog_data_version": shop.catalog_data_version(),
            "built_at": shop.built_at(),
            "schema_version": wenmar_shopdata::schema::SCHEMA_VERSION,
            "datasets": shop.datasets(),
        });
        self.shop = Some(shop);
        Ok(answer)
    }
}

enum Outcome {
    Done(Value),
    /// Statements for the data file, then statements for the shop file.
    Need(Vec<Statement>, Vec<Statement>),
}
