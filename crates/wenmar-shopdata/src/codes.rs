//! Trouble codes: the generic OBD-II codes this project has described in
//! its own words, from `data/codes/`.
//!
//! A code number and the system it belongs to are facts. The text of a
//! record is written for this project and comes from nowhere else.

use serde::Serialize;
use wenmar_vehicles::{Source, Value};

use crate::bulletins::code_form;
use crate::dataset::Dataset;
use crate::error::ShopError;
use crate::recalls::contains_pattern;
use crate::shop::ShopData;

/// The first two characters of every generic code. A `P3` code is generic
/// only from `P3400` up; see [`is_generic`].
pub const RANGES: [&str; 7] = ["P0", "P2", "P3", "B0", "C0", "U0", "U3"];

/// What a record's `system` may be, with the name a page shows. An id is
/// a group, or a group and an area of it. The group follows from the
/// code's letter; the areas are this project's own grouping.
pub const SYSTEMS: [(&str, &str); 11] = [
    ("powertrain", "Powertrain"),
    ("powertrain/fuel-air", "Fuel and air metering"),
    ("powertrain/ignition", "Ignition and misfire"),
    ("powertrain/emissions", "Emission controls"),
    ("powertrain/speed-idle", "Vehicle speed and idle control"),
    ("powertrain/computer", "Control module and its circuits"),
    ("powertrain/transmission", "Transmission"),
    ("powertrain/hybrid", "Hybrid and electric drive"),
    ("body", "Body"),
    ("chassis", "Chassis"),
    ("network", "Network and communication"),
];

/// Most codes one page of a list holds, whatever `limit` says.
pub const MOST_CODES_LISTED: usize = 200;

/// Whether `text` is a code as the tables store one: a capital `P`, `B`,
/// `C` or `U`, a digit `0` to `3`, and three of `0`-`9` and `A`-`F`.
pub fn is_code(text: &str) -> bool {
    text.len() == 5 && code_form(text).as_deref() == Some(text)
}

/// Whether a code is in a range every maker uses the same way, so that it
/// may have a record: `P0`, `P2`, `P3400` and up, `B0`, `C0`, `U0`, `U3`.
pub fn is_generic(code: &str) -> bool {
    if !is_code(code) {
        return false;
    }
    let bytes = code.as_bytes();
    match (bytes[0], bytes[1]) {
        (b'P', b'0' | b'2') | (b'B' | b'C' | b'U', b'0') | (b'U', b'3') => true,
        // `4` to `9` and `A` to `F` all sort at or after `4`.
        (b'P', b'3') => bytes[2] >= b'4',
        _ => false,
    }
}

/// The group a code's letter puts it in: `powertrain`, `body`, `chassis`
/// or `network`.
pub fn group(code: &str) -> Option<&'static str> {
    match code.as_bytes().first()? {
        b'P' => Some("powertrain"),
        b'B' => Some("body"),
        b'C' => Some("chassis"),
        b'U' => Some("network"),
        _ => None,
    }
}

/// The name of a system id, or `None` for one that is not in [`SYSTEMS`].
pub fn system_name(id: &str) -> Option<&'static str> {
    SYSTEMS
        .iter()
        .find(|(system, _)| *system == id)
        .map(|(_, name)| *name)
}

/// Whether a record for `code` may have this `system`: one of
/// [`SYSTEMS`], and the code's own group or an area of it.
pub fn system_fits(code: &str, system: &str) -> bool {
    let Some(group) = group(code) else {
        return false;
    };
    system_name(system).is_some() && system.split('/').next() == Some(group)
}

/// A code as a list shows it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct CodeSummary {
    pub code: String,
    pub system: String,
    pub title: String,
}

/// A component a code concerns: an entry of `data/components.yaml`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct CodeComponent {
    pub id: String,
    pub name: String,
    /// The system a part belongs to. `None` for a system.
    pub parent: Option<String>,
}

/// One record of `data/codes/`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct CodeRecord {
    pub code: String,
    pub system: String,
    pub title: String,
    pub meaning: String,
    pub common_causes: Vec<String>,
    pub checks: Vec<String>,
    /// In the order the record lists them.
    pub components: Vec<CodeComponent>,
    /// The related codes that have a record, in the record's order.
    pub related: Vec<CodeSummary>,
    pub author: String,
    pub reviewer: String,
    /// The day the reviewer approved the record, as `2026-10-12`.
    pub checked: String,
}

/// What a list of codes is narrowed by.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct CodeQuery {
    /// Only codes with this text in the code, the title or the meaning.
    pub q: Option<String>,
    /// Only this system: an id of [`SYSTEMS`]. A group holds its areas.
    pub system: Option<String>,
    /// At most this many, up to [`MOST_CODES_LISTED`]. 0 means the most.
    pub limit: usize,
    /// Only codes after this one: [`CodePage::next`] of the page before.
    pub after: Option<String>,
}

/// One page of a list, by code.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CodePage {
    pub codes: Vec<CodeSummary>,
    /// What to pass as `after` for the next page. `None` on the last.
    pub next: Option<String>,
}

const TABLE: &str = "code";

fn text(row: &[Value], column: usize) -> Result<String, ShopError> {
    row.get(column)
        .and_then(Value::text)
        .map(str::to_owned)
        .ok_or(ShopError::Shape(TABLE))
}

fn list(row: &[Value], column: usize) -> Result<Vec<String>, ShopError> {
    let json = row
        .get(column)
        .and_then(Value::text)
        .ok_or(ShopError::Shape(TABLE))?;
    serde_json::from_str(json).map_err(|_| ShopError::Shape(TABLE))
}

fn summary(row: &[Value]) -> Result<CodeSummary, ShopError> {
    Ok(CodeSummary {
        code: text(row, 0)?,
        system: text(row, 1)?,
        title: text(row, 2)?,
    })
}

impl<S: Source> ShopData<S> {
    /// The newest day a record was approved, as `2026-10-12`. `None` when
    /// the file holds no codes.
    pub fn codes_as_of(&self) -> Option<&str> {
        self.datasets()
            .get(Dataset::Codes)
            .map(|entry| entry.as_of.as_str())
    }

    /// The components one code concerns, in the record's order. Empty for
    /// a code with no record.
    pub fn code_components(&self, code: &str) -> Result<Vec<CodeComponent>, ShopError> {
        self.require(Dataset::Codes)?;
        self.query(
            "SELECT k.id, k.name, k.parent FROM code_component c JOIN component k ON k.id = c.component_id WHERE c.code = ?1 ORDER BY c.position",
            &[code.into()],
        )?
        .iter()
        .map(|row| {
            Ok(CodeComponent {
                id: text(row, 0)?,
                name: text(row, 1)?,
                parent: row.get(2).and_then(Value::text).map(str::to_owned),
            })
        })
        .collect()
    }

    /// One record. `None` when nobody has written one for the code.
    pub fn code(&self, code: &str) -> Result<Option<CodeRecord>, ShopError> {
        self.require(Dataset::Codes)?;
        let rows = self.query(
            "SELECT code, system, title, meaning, common_causes, checks, related, author, reviewer, checked FROM code WHERE code = ?1",
            &[code.into()],
        )?;
        let Some(row) = rows.first() else {
            return Ok(None);
        };
        // One statement for all of them, then the record's own order.
        let wanted = list(row, 6)?;
        let mut related = Vec::with_capacity(wanted.len());
        if !wanted.is_empty() {
            let marks: Vec<String> = (1..=wanted.len())
                .map(|number| format!("?{number}"))
                .collect();
            let params: Vec<Value> = wanted.iter().map(|code| code.as_str().into()).collect();
            let found = self.query(
                &format!(
                    "SELECT code, system, title FROM code WHERE code IN ({})",
                    marks.join(", ")
                ),
                &params,
            )?;
            for code in &wanted {
                let row = found
                    .iter()
                    .find(|row| row.first().and_then(Value::text) == Some(code.as_str()));
                if let Some(row) = row {
                    related.push(summary(row)?);
                }
            }
        }
        Ok(Some(CodeRecord {
            code: text(row, 0)?,
            system: text(row, 1)?,
            title: text(row, 2)?,
            meaning: text(row, 3)?,
            common_causes: list(row, 4)?,
            checks: list(row, 5)?,
            components: self.code_components(code)?,
            related,
            author: text(row, 7)?,
            reviewer: text(row, 8)?,
            checked: text(row, 9)?,
        }))
    }

    /// One page of records narrowed by `query`, by code.
    pub fn codes(&self, query: &CodeQuery) -> Result<CodePage, ShopError> {
        self.require(Dataset::Codes)?;
        let limit = match query.limit {
            0 => MOST_CODES_LISTED,
            limit => limit.min(MOST_CODES_LISTED),
        };
        let mut wheres: Vec<String> = Vec::new();
        let mut params: Vec<Value> = Vec::new();
        let mut bind = |value: Value| {
            params.push(value);
            params.len()
        };
        if let Some(system) = query.system.as_deref() {
            let exact = bind(system.into());
            let below = bind(format!("{system}/").as_str().into());
            wheres.push(format!(
                "(system = ?{exact} OR substr(system, 1, length(?{below})) = ?{below})"
            ));
        }
        if let Some(pattern) = query.q.as_deref().and_then(contains_pattern) {
            let number = bind(pattern.as_str().into());
            wheres.push(format!(
                "(code LIKE ?{number} ESCAPE '\\' OR title LIKE ?{number} ESCAPE '\\' OR meaning LIKE ?{number} ESCAPE '\\')"
            ));
        }
        if let Some(after) = query.after.as_deref() {
            wheres.push(format!("code > ?{}", bind(after.into())));
        }
        // One more than asked for, to learn whether a page follows.
        let limit_number = bind(Value::Integer(i64::try_from(limit + 1).unwrap_or(i64::MAX)));
        let filter = if wheres.is_empty() {
            String::new()
        } else {
            format!("WHERE {}", wheres.join(" AND "))
        };
        let rows = self.query(
            &format!(
                "SELECT code, system, title FROM code {filter} ORDER BY code LIMIT ?{limit_number}"
            ),
            &params,
        )?;
        let mut codes = Vec::with_capacity(limit);
        for row in rows.iter().take(limit) {
            codes.push(summary(row)?);
        }
        let next = (rows.len() > limit)
            .then(|| codes.last().map(|code| code.code.clone()))
            .flatten();
        Ok(CodePage { codes, next })
    }

    /// Every record, by code.
    pub fn all_codes(&self) -> Result<Vec<CodeSummary>, ShopError> {
        self.require(Dataset::Codes)?;
        self.query("SELECT code, system, title FROM code ORDER BY code", &[])?
            .iter()
            .map(|row| summary(row))
            .collect()
    }
}

impl<S: Source> ShopData<S> {
    /// The campaigns that may apply to one model year and are about one of
    /// `components` (ids of `components.yaml`), newest first. Empty when
    /// `components` is, or when the file was built before recalls had
    /// components. The file must hold a recall dataset.
    pub fn recalls_about_for_vehicle(
        &self,
        make_id: i64,
        model_id: i64,
        year: u16,
        components: &[String],
    ) -> Result<Vec<crate::recalls::VehicleRecall>, ShopError> {
        let all = self.recalls_for_vehicle(make_id, model_id, year, None)?;
        if components.is_empty() || all.is_empty() || !self.has_table("recall_component")? {
            return Ok(Vec::new());
        }
        let mut params: Vec<Value> = vec![make_id.into(), model_id.into(), i64::from(year).into()];
        let mut marks: Vec<String> = Vec::with_capacity(components.len());
        for component in components {
            params.push(component.as_str().into());
            marks.push(format!("?{}", params.len()));
        }
        let sql = format!(
            "SELECT DISTINCT r.source, r.campaign FROM recall_vehicle v
             JOIN recall r ON r.id = v.recall_id
             JOIN recall_component c ON c.recall_id = r.id
             WHERE v.make_id = ?1 AND v.model_id = ?2 AND v.year = ?3 AND c.component_id IN ({})",
            marks.join(", ")
        );
        let mut about: Vec<(String, String)> = Vec::new();
        for row in self.query(&sql, &params)? {
            about.push((text(&row, 0)?, text(&row, 1)?));
        }
        Ok(all
            .into_iter()
            .filter(|found| {
                about.iter().any(|(source, campaign)| {
                    *source == found.recall.source && *campaign == found.recall.campaign
                })
            })
            .collect())
    }
}

/// What every answer and page about a code says.
pub const NOTE: &str = "Written for Wenmar Open in its own words, and general: it is not about one make. A code says which circuit or system reported a fault, not which part to replace. Diagnose before replacing anything.";

/// What a vehicle's answer about a code says.
pub const VEHICLE_NOTE: &str = "The bulletins listed name this code in NHTSA's summary, or are about the same components. A bulletin or a recall listed here may apply to the model year. None of it is a diagnosis.";

/// `not_found`: the text asked for is not a code.
pub const NOT_A_CODE: &str = "No trouble code is written that way. A code is P, B, C or U and four characters, such as P0420.";

/// `not_found`: the code is outside the ranges [`is_generic`] allows.
pub const NOT_GENERIC: &str = "That code is specific to a manufacturer, so it has no description here. Ask for it on a vehicle to see the bulletins that name it.";

/// `not_found`: a generic code nobody has described.
pub const NOT_WRITTEN: &str = "Nobody has written a description of that code yet.";

/// `validation_failed` for `system`.
pub const BAD_SYSTEM: &str = "system must be powertrain, powertrain/fuel-air, powertrain/ignition, powertrain/emissions, powertrain/speed-idle, powertrain/computer, powertrain/transmission, powertrain/hybrid, body, chassis or network";

/// `validation_failed` for `cursor`.
pub const BAD_CURSOR: &str = "cursor is not one this API gave out";

/// Codes in a list when `limit` is not given.
pub const LIST_DEFAULT: usize = 50;
