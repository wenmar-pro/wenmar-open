//! Labour estimates: the hours a job takes, by Wenmar, under CC BY 4.0.
//!
//! An estimate is for a scope, and a lookup takes the narrowest scope
//! that has a figure: an engine, then a run of model years, then a model,
//! then a class of vehicle. The answer always says which scope it was, so
//! that "typical for a small pickup" is never shown as if it had been
//! measured on that truck.

use serde::Serialize;
use wenmar_vehicles::{Source, Value};

use crate::dataset::Dataset;
use crate::error::ShopError;
use crate::shop::ShopData;

/// The credit every labour answer and page carries.
pub const ATTRIBUTION: &str = "Labour estimates by Wenmar Pro, CC BY 4.0";

/// The path of the page that says how every labour figure is made.
pub const METHOD_PATH: &str = "/jobs/how-estimates-are-made";

/// What every labour answer and page says.
pub const NOTE: &str =
    "An estimate, not a quote. Rust, seized fasteners and added equipment change the time.";

/// The fewest shops that must stand behind a measured figure for it to be
/// published.
pub const MIN_SHOPS: i64 = 5;

/// The fewest repair orders that must stand behind a measured figure.
pub const MIN_JOBS: i64 = 20;

/// The scopes, narrowest first: the order a lookup tries them in.
pub const SCOPES: [&str; 4] = ["engine", "generation", "model", "class"];

/// The kinds of figure: written by a person, measured from repair orders,
/// worked out by a model. Within one scope a lookup prefers them in the
/// order observed, expert, ai.
pub const BASES: [&str; 3] = ["expert", "observed", "ai"];

/// The fewest hundredths of an hour a figure may be: 0.01.
pub const LEAST_HUNDREDTHS: i64 = 1;

/// The most hundredths of an hour an `ai` figure may be: 40 hours.
pub const AI_MOST_HUNDREDTHS: i64 = 4_000;

/// An `ai` figure's high may be at most this many times its typical.
pub const AI_MOST_SPREAD: i64 = 3;

/// The most confidence an `ai` figure may claim, in percent.
pub const AI_MOST_CONFIDENCE: i64 = 60;

/// The kinds of input an `ai` figure may have been worked out from: the
/// vehicle's facts, the job's definition, Wenmar's own figures of the same
/// job, and a repair procedure's steps (never its numbers).
pub const AI_INPUTS: [&str; 4] = ["vehicle", "job", "wenmar-figures", "procedure"];

/// The classes of vehicle a class-scope estimate may be for: id and name.
/// The order decides a tie when a model year has two classes.
pub const CLASSES: [(&str, &str); 13] = [
    ("subcompact-car", "Subcompact car"),
    ("compact-car", "Compact car"),
    ("midsize-car", "Midsize car"),
    ("large-car", "Large car"),
    ("two-seater", "Two-seater"),
    ("station-wagon", "Station wagon"),
    ("small-suv", "Small SUV"),
    ("standard-suv", "Standard SUV"),
    ("suv", "SUV, size not given"),
    ("minivan", "Minivan"),
    ("van", "Van"),
    ("small-pickup", "Small pickup"),
    ("standard-pickup", "Standard pickup"),
];

/// The name of a class id, or `None` for one that is not in [`CLASSES`].
pub fn class_name(id: &str) -> Option<&'static str> {
    CLASSES
        .iter()
        .find(|(class, _)| *class == id)
        .map(|(_, name)| *name)
}

/// The class EPA's size class puts a vehicle in. `None` for a size class
/// that is not mapped: `Special Purpose Vehicle` is a mixed bag.
pub fn class_of_epa(size_class: &str) -> Option<&'static str> {
    let text = size_class.trim().to_ascii_lowercase();
    let starts = |prefix: &str| text.starts_with(prefix);
    if starts("minicompact cars") || starts("subcompact cars") {
        Some("subcompact-car")
    } else if starts("compact cars") {
        Some("compact-car")
    } else if starts("midsize cars") {
        Some("midsize-car")
    } else if starts("large cars") {
        Some("large-car")
    } else if starts("two seaters") {
        Some("two-seater")
    } else if text.contains("station wagons") {
        Some("station-wagon")
    } else if starts("small sport utility vehicle") {
        Some("small-suv")
    } else if starts("standard sport utility vehicle") {
        Some("standard-suv")
    } else if starts("sport utility vehicle") {
        Some("suv")
    } else if starts("minivan") {
        Some("minivan")
    } else if starts("vans") {
        Some("van")
    } else if starts("small pickup trucks") {
        Some("small-pickup")
    } else if starts("standard pickup trucks") {
        Some("standard-pickup")
    } else {
        None
    }
}

/// One estimate, as the lookup gives it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct LaborEstimate {
    pub job_id: String,
    pub job_name: String,
    /// Which scope answered: one of [`SCOPES`].
    pub scope: String,
    /// The class, for a class-scope figure.
    pub class: Option<String>,
    /// The make and model the figure is for, for a figure narrower than
    /// class. `None` for a class-scope figure.
    pub make_id: Option<i64>,
    pub model_id: Option<i64>,
    /// The run of model years, for a generation or an engine.
    pub year_from: Option<u16>,
    pub year_to: Option<u16>,
    /// The engine's id form, for an engine-scope figure.
    pub engine: Option<String>,
    /// Whole hundredths of an hour.
    pub hours_low: i64,
    pub hours_typical: i64,
    pub hours_high: i64,
    /// `expert`, `observed` or `ai`.
    pub basis: String,
    /// How many jobs and shops stand behind an observed figure.
    pub sample_jobs: Option<i64>,
    pub sample_shops: Option<i64>,
    pub notes: Option<String>,
    /// The day the figure was last changed, as `2026-10-12`.
    pub updated: String,
    /// For an `ai` figure: the model's confidence in percent, the model,
    /// the prompt version, and the kinds of input it was given. `None` and
    /// empty for the others, and for a file without the `labor_ai` dataset.
    pub confidence_pct: Option<i64>,
    pub model: Option<String>,
    pub method: Option<String>,
    pub inputs: Vec<String>,
}

/// What a shop file holds about its labour estimates: the counts the
/// method page shows. Every field is counted from the file.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct LaborCounts {
    /// Every `labor_estimate` row.
    pub figures: i64,
    /// The figures of each basis, in [`BASES`] order, a basis with no
    /// figures left out.
    pub by_basis: Vec<(String, i64)>,
    /// The jobs that have at least one figure.
    pub jobs: i64,
    /// The distinct models of the figures narrower than class.
    pub models: i64,
    /// The distinct makes of the figures narrower than class.
    pub makes: i64,
    /// The `ai` figures worked out with a repair procedure's steps.
    pub ai_with_steps: i64,
    /// The models that answered an `ai` figure, sorted.
    pub ai_answered_by: Vec<String>,
    /// The prompt versions of the `ai` figures, sorted.
    pub ai_methods: Vec<String>,
    /// The newest day any figure was changed, as `2026-10-12`.
    pub newest: Option<String>,
}

/// The columns [`estimate`] reads. With `ai`, the `labor_ai` columns from
/// `a`; without, NULLs in their place (a file without the dataset).
fn columns(ai: bool) -> String {
    let extra = if ai {
        "a.confidence_pct, a.model, a.method, a.inputs"
    } else {
        "NULL, NULL, NULL, NULL"
    };
    format!(
        "e.job_id, j.name, e.scope, e.class, e.year_from, e.year_to, e.engine, e.hours_low, e.hours_typical, e.hours_high, e.basis, e.sample_jobs, e.sample_shops, e.notes, e.updated, {extra}, e.make_id, e.model_id"
    )
}

/// The join that brings in `labor_ai`, when the file holds it.
fn ai_join(ai: bool) -> &'static str {
    if ai {
        "LEFT JOIN labor_ai a ON a.estimate_id = e.id"
    } else {
        ""
    }
}

/// Within one scope: a measured figure, then a written one, then a
/// model's.
const BASIS_ORDER: &str = "CASE e.basis WHEN 'observed' THEN 0 WHEN 'expert' THEN 1 ELSE 2 END";

/// Every estimate that covers one vehicle, the narrowest scope of each job
/// first: `?1` model id, `?2` model year, `?3` the engine's id form or
/// NULL, `?4` a job id or NULL for every job. The first row of each job is
/// the answer: within a scope a measured figure comes before a written
/// one, and a written one before a model's. Jobs come in the order of the
/// job list.
pub fn labor_for_vehicle_sql(ai: bool) -> String {
    format!(
        "
SELECT {columns}
FROM labor_estimate e
JOIN job j ON j.id = e.job_id
{join}
WHERE (?4 IS NULL OR e.job_id = ?4)
  AND (   (e.scope = 'engine' AND e.model_id = ?1 AND ?2 BETWEEN e.year_from AND e.year_to AND e.engine = ?3)
       OR (e.scope = 'generation' AND e.model_id = ?1 AND ?2 BETWEEN e.year_from AND e.year_to)
       OR (e.scope = 'model' AND e.model_id = ?1)
       OR (e.scope = 'class' AND e.class = (SELECT class FROM vehicle_class WHERE model_id = ?1 AND year = ?2)))
ORDER BY j.position,
         CASE e.scope WHEN 'engine' THEN 0 WHEN 'generation' THEN 1 WHEN 'model' THEN 2 ELSE 3 END,
         {BASIS_ORDER},
         e.id",
        columns = columns(ai),
        join = ai_join(ai),
    )
}

const TABLE: &str = "labor_estimate";

fn text(row: &[Value], column: usize) -> Result<String, ShopError> {
    row.get(column)
        .and_then(Value::text)
        .map(str::to_owned)
        .ok_or(ShopError::Shape(TABLE))
}

fn optional_text(row: &[Value], column: usize) -> Option<String> {
    row.get(column).and_then(Value::text).map(str::to_owned)
}

fn integer(row: &[Value], column: usize) -> Result<i64, ShopError> {
    row.get(column)
        .and_then(Value::integer)
        .ok_or(ShopError::Shape(TABLE))
}

fn year(row: &[Value], column: usize) -> Option<u16> {
    row.get(column)
        .and_then(Value::integer)
        .and_then(|year| u16::try_from(year).ok())
}

/// Reads a [`LaborEstimate`] from [`columns`].
fn estimate(row: &[Value]) -> Result<LaborEstimate, ShopError> {
    Ok(LaborEstimate {
        job_id: text(row, 0)?,
        job_name: text(row, 1)?,
        scope: text(row, 2)?,
        class: optional_text(row, 3),
        make_id: row.get(19).and_then(Value::integer),
        model_id: row.get(20).and_then(Value::integer),
        year_from: year(row, 4),
        year_to: year(row, 5),
        engine: optional_text(row, 6),
        hours_low: integer(row, 7)?,
        hours_typical: integer(row, 8)?,
        hours_high: integer(row, 9)?,
        basis: text(row, 10)?,
        sample_jobs: row.get(11).and_then(Value::integer),
        sample_shops: row.get(12).and_then(Value::integer),
        notes: optional_text(row, 13),
        updated: text(row, 14)?,
        confidence_pct: row.get(15).and_then(Value::integer),
        model: optional_text(row, 16),
        method: optional_text(row, 17),
        inputs: optional_text(row, 18)
            .map(|inputs| {
                inputs
                    .split(',')
                    .map(str::trim)
                    .filter(|input| !input.is_empty())
                    .map(str::to_owned)
                    .collect()
            })
            .unwrap_or_default(),
    })
}

impl<S: Source> ShopData<S> {
    /// The newest day an estimate was changed, as `2026-10-12`. `None`
    /// when the file holds no labour estimates.
    pub fn labor_as_of(&self) -> Option<&str> {
        self.datasets()
            .get(Dataset::Labor)
            .map(|entry| entry.as_of.as_str())
    }

    /// Whether the file says how its `ai` figures were worked out (the
    /// `labor_ai` dataset). A file built before it existed does not.
    pub fn holds_labor_ai(&self) -> bool {
        self.datasets().contains(Dataset::LaborAi)
    }

    /// What the file holds about its labour estimates: how many figures,
    /// of which basis, for how many jobs, models and makes, and how the
    /// `ai` figures were worked out. Every count is read from the file.
    pub fn labor_counts(&self) -> Result<LaborCounts, ShopError> {
        self.require(Dataset::Labor)?;
        let count = |sql: &str| -> Result<i64, ShopError> {
            self.query(sql, &[])?
                .first()
                .and_then(|row| row.first())
                .and_then(Value::integer)
                .ok_or(ShopError::Shape(TABLE))
        };
        let texts = |sql: &str| -> Result<Vec<String>, ShopError> {
            self.query(sql, &[])?
                .iter()
                .map(|row| text(row, 0))
                .collect()
        };
        let figures = count("SELECT COUNT(*) FROM labor_estimate")?;
        let rows = self.query(
            "SELECT basis, COUNT(*) FROM labor_estimate GROUP BY basis",
            &[],
        )?;
        let mut held = Vec::with_capacity(rows.len());
        for row in &rows {
            held.push((text(row, 0)?, integer(row, 1)?));
        }
        let by_basis = BASES
            .iter()
            .filter_map(|basis| {
                held.iter()
                    .find(|(name, _)| name == basis)
                    .map(|(_, n)| ((*basis).to_owned(), *n))
            })
            .collect();
        let jobs = count("SELECT COUNT(DISTINCT job_id) FROM labor_estimate")?;
        let models = count(
            "SELECT COUNT(DISTINCT model_id) FROM labor_estimate
             WHERE scope <> 'class' AND model_id IS NOT NULL",
        )?;
        let makes = count(
            "SELECT COUNT(DISTINCT make_id) FROM labor_estimate
             WHERE scope <> 'class' AND make_id IS NOT NULL",
        )?;
        let newest = self
            .query("SELECT MAX(updated) FROM labor_estimate", &[])?
            .first()
            .and_then(|row| row.first())
            .and_then(Value::text)
            .map(str::to_owned);
        let (ai_with_steps, ai_answered_by, ai_methods) = if self.holds_labor_ai() {
            let with_steps = count(
                "SELECT COUNT(*) FROM labor_estimate e
                 JOIN labor_ai a ON a.estimate_id = e.id
                 WHERE e.basis = 'ai'
                   AND instr(',' || a.inputs || ',', ',procedure,') > 0",
            )?;
            let answered_by = texts(
                "SELECT DISTINCT a.model FROM labor_ai a
                 JOIN labor_estimate e ON e.id = a.estimate_id
                 WHERE e.basis = 'ai' ORDER BY a.model",
            )?;
            let methods = texts(
                "SELECT DISTINCT a.method FROM labor_ai a
                 JOIN labor_estimate e ON e.id = a.estimate_id
                 WHERE e.basis = 'ai' ORDER BY a.method",
            )?;
            (with_steps, answered_by, methods)
        } else {
            (0, Vec::new(), Vec::new())
        };
        Ok(LaborCounts {
            figures,
            by_basis,
            jobs,
            models,
            makes,
            ai_with_steps,
            ai_answered_by,
            ai_methods,
            newest,
        })
    }

    /// The class of one model year and where it came from: `epa`, or
    /// `nearest` when it is the class of the nearest model year that has
    /// one. `None` when the model has no class at all.
    pub fn vehicle_class(
        &self,
        model_id: i64,
        year: u16,
    ) -> Result<Option<(String, String)>, ShopError> {
        self.require(Dataset::Labor)?;
        let rows = self.query(
            "SELECT class, source FROM vehicle_class WHERE model_id = ?1 AND year = ?2",
            &[model_id.into(), i64::from(year).into()],
        )?;
        match rows.first() {
            Some(row) => Ok(Some((text(row, 0)?, text(row, 1)?))),
            None => Ok(None),
        }
    }

    /// One estimate for each job that has a figure covering the vehicle:
    /// the narrowest scope's. `engine` is the engine's id form, when the
    /// vehicle's engine is known; `job` narrows the answer to one job.
    /// Jobs come in the order of the job list.
    pub fn labor_for_vehicle(
        &self,
        model_id: i64,
        year: u16,
        engine: Option<&str>,
        job: Option<&str>,
    ) -> Result<Vec<LaborEstimate>, ShopError> {
        self.require(Dataset::Labor)?;
        let optional = |text: Option<&str>| text.map_or(Value::Null, Value::from);
        let rows = self.query(
            &labor_for_vehicle_sql(self.holds_labor_ai()),
            &[
                model_id.into(),
                i64::from(year).into(),
                optional(engine),
                optional(job),
            ],
        )?;
        let mut found: Vec<LaborEstimate> = Vec::new();
        for row in &rows {
            let next = estimate(row)?;
            // The rows of a job are together, its answer first.
            if found.last().is_none_or(|last| last.job_id != next.job_id) {
                found.push(next);
            }
        }
        Ok(found)
    }

    /// The figures of one job that are narrower than class: the model,
    /// generation and engine figures, which are the vehicles that have
    /// their own time for it. In make, model, year and engine order. A
    /// class-scope figure is not a vehicle's own.
    pub fn job_vehicles(
        &self,
        job: &str,
        offset: usize,
        limit: usize,
    ) -> Result<Vec<LaborEstimate>, ShopError> {
        self.require(Dataset::Labor)?;
        let ai = self.holds_labor_ai();
        let rows = self.query(
            &format!(
                "SELECT {columns} FROM labor_estimate e JOIN job j ON j.id = e.job_id {join}
                 WHERE e.job_id = ?1 AND e.scope <> 'class'
                 ORDER BY e.make_id, e.model_id, e.year_from, e.engine, e.id
                 LIMIT ?2 OFFSET ?3",
                columns = columns(ai),
                join = ai_join(ai),
            ),
            &[
                job.into(),
                i64::try_from(limit).unwrap_or(i64::MAX).into(),
                i64::try_from(offset).unwrap_or(i64::MAX).into(),
            ],
        )?;
        rows.iter().map(|row| estimate(row.as_slice())).collect()
    }

    /// The class-scope figures of one job, in the order of [`CLASSES`]:
    /// what a job's own page shows. Where a class has a measured figure
    /// and a written one, the measured one is given.
    pub fn class_estimates_for_job(&self, job: &str) -> Result<Vec<LaborEstimate>, ShopError> {
        self.require(Dataset::Labor)?;
        let ai = self.holds_labor_ai();
        let rows = self.query(
            &format!(
                "SELECT {columns} FROM labor_estimate e JOIN job j ON j.id = e.job_id {join}
                 WHERE e.job_id = ?1 AND e.scope = 'class'
                 ORDER BY {BASIS_ORDER}, e.id",
                columns = columns(ai),
                join = ai_join(ai),
            ),
            &[job.into()],
        )?;
        let mut found: Vec<LaborEstimate> = Vec::new();
        for row in &rows {
            let next = estimate(row)?;
            if !found.iter().any(|held| held.class == next.class) {
                found.push(next);
            }
        }
        let place = |estimate: &LaborEstimate| {
            CLASSES
                .iter()
                .position(|(id, _)| Some(*id) == estimate.class.as_deref())
                .unwrap_or(CLASSES.len())
        };
        found.sort_by_key(place);
        Ok(found)
    }
}
