//! `data/labor/`: Wenmar's own estimates of the hours a job takes, built
//! into `labor_estimate`, and the class of each model year, built into
//! `vehicle_class` from EPA's size class.

use std::collections::{BTreeMap, HashMap, HashSet};
use std::path::Path;

use anyhow::{Context, Result, bail};
use rusqlite::{Connection, OptionalExtension, params};
use serde::Deserialize;
use wenmar_shopdata::labor::{
    AI_INPUTS, AI_MOST_CONFIDENCE, ATTRIBUTION, CLASSES, MIN_JOBS, MIN_SHOPS, class_name,
    class_of_epa,
};
use wenmar_shopdata::{Dataset, DatasetEntry, licence, schema};
use wenmar_vehicles::text::{is_slug, slug, squeeze};

use crate::shop::build::ShapeContext;
use crate::shop::codes::{files, is_day, is_github_name};
use crate::shop::components::Components;
use crate::shop::jobs::{Jobs, Status};

/// The most hours a figure may be, in hundredths: 999.99.
pub const MOST_HUNDREDTHS: i64 = 99_999;
/// The longest a note may be, in characters.
pub const LONGEST_NOTE: usize = 300;

/// Hours written as `1`, `1.5` or `0.25`, as whole hundredths. `None` for
/// anything else: no digits before the point, more than two after it, a
/// sign, zero, or more than [`MOST_HUNDREDTHS`].
pub fn hundredths(text: &str) -> Option<i64> {
    let (whole, part) = match text.split_once('.') {
        Some((whole, part)) => (whole, part),
        None => (text, "0"),
    };
    let digits = |text: &str| !text.is_empty() && text.bytes().all(|byte| byte.is_ascii_digit());
    if !digits(whole) || !digits(part) || whole.len() > 3 || part.len() > 2 {
        return None;
    }
    let part = if part.len() == 1 {
        part.parse::<i64>().ok()? * 10
    } else {
        part.parse::<i64>().ok()?
    };
    let total = whole.parse::<i64>().ok()? * 100 + part;
    (1..=MOST_HUNDREDTHS).contains(&total).then_some(total)
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
struct Written {
    scope: String,
    #[serde(default)]
    class: Option<String>,
    #[serde(default)]
    make: Option<String>,
    #[serde(default)]
    model: Option<String>,
    #[serde(default)]
    year_from: Option<u16>,
    #[serde(default)]
    year_to: Option<u16>,
    #[serde(default)]
    engine: Option<String>,
    low: String,
    typical: String,
    high: String,
    basis: String,
    #[serde(default)]
    notes: Option<String>,
    author: String,
    reviewer: String,
    updated: String,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct LaborFile {
    job: String,
    estimates: Vec<Written>,
}

/// One estimate of a file, checked for shape. The vehicle is still named
/// by id forms: the catalog has not been asked.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EstimateRecord {
    pub job: String,
    /// `class`, `model`, `generation` or `engine`.
    pub scope: String,
    pub class: Option<String>,
    pub make: Option<String>,
    pub model: Option<String>,
    pub year_from: Option<u16>,
    pub year_to: Option<u16>,
    pub engine: Option<String>,
    /// Whole hundredths of an hour.
    pub low: i64,
    pub typical: i64,
    pub high: i64,
    pub notes: Option<String>,
    pub author: String,
    pub reviewer: String,
    pub updated: String,
}

/// Reads the text of one file, `<name>.yaml`, and checks everything that
/// can be checked without the job list and the catalog.
pub fn parse(name: &str, yaml: &str) -> Result<Vec<EstimateRecord>> {
    let file = format!("{name}.yaml");
    let parsed: LaborFile =
        serde_saphyr::from_str(yaml).with_context(|| format!("reading data/labor/{file}"))?;
    if parsed.job != name {
        bail!(
            "data/labor/{file} holds the job {}; name the file after its job",
            parsed.job
        );
    }
    let mut records = Vec::with_capacity(parsed.estimates.len());
    for (index, written) in parsed.estimates.into_iter().enumerate() {
        let place = format!("data/labor/{file}, estimate {}", index + 1);
        let scope = written.scope.as_str();
        let vehicle = written.make.is_some() || written.model.is_some();
        let years = written.year_from.is_some() || written.year_to.is_some();
        match scope {
            "class" => {
                let Some(class) = written.class.as_deref() else {
                    bail!("{place}: a class estimate needs class");
                };
                if class_name(class).is_none() {
                    bail!(
                        "{place}: {class} is not a class; use one of {}",
                        CLASSES
                            .iter()
                            .map(|(id, _)| *id)
                            .collect::<Vec<_>>()
                            .join(", ")
                    );
                }
                if vehicle || years || written.engine.is_some() {
                    bail!("{place}: a class estimate takes no make, model, model year or engine");
                }
            }
            "model" | "generation" | "engine" => {
                if written.class.is_some() {
                    bail!("{place}: a {scope} estimate takes no class");
                }
                let named = written.make.as_deref().is_some_and(is_slug)
                    && written.model.as_deref().is_some_and(is_slug);
                if !named {
                    bail!("{place}: a {scope} estimate needs make and model, as id forms");
                }
                if scope == "model" {
                    if years || written.engine.is_some() {
                        bail!("{place}: a model estimate takes no model year and no engine");
                    }
                } else {
                    let (Some(from), Some(to)) = (written.year_from, written.year_to) else {
                        bail!("{place}: a {scope} estimate needs year_from and year_to");
                    };
                    if from > to {
                        bail!("{place}: year_from is after year_to");
                    }
                    match (scope, written.engine.as_deref()) {
                        ("engine", Some(engine)) if is_slug(engine) => {}
                        ("engine", _) => {
                            bail!("{place}: an engine estimate needs engine, as an id form")
                        }
                        (_, Some(_)) => bail!("{place}: a generation estimate takes no engine"),
                        (_, None) => {}
                    }
                }
            }
            other => bail!(
                "{place}: the scope {other} is not one of class, model, generation and engine"
            ),
        }
        let hours = |field: &str, text: &str| {
            hundredths(text).with_context(|| {
                format!(
                    "{place}: {field} must be hours like \"1.5\", from 0.01 to 999.99, not {text:?}"
                )
            })
        };
        let (low, typical, high) = (
            hours("low", &written.low)?,
            hours("typical", &written.typical)?,
            hours("high", &written.high)?,
        );
        if !(low <= typical && typical <= high) {
            bail!("{place}: low, typical and high must be in that order");
        }
        if written.basis != "expert" {
            bail!(
                "{place}: basis must be expert, not {:?}; measured figures are not written by hand",
                written.basis
            );
        }
        let notes = written
            .notes
            .as_deref()
            .map(squeeze)
            .filter(|notes| !notes.is_empty());
        if notes
            .as_ref()
            .is_some_and(|notes| notes.chars().count() > LONGEST_NOTE)
        {
            bail!("{place}: notes is longer than {LONGEST_NOTE} characters");
        }
        for (field, name) in [("author", &written.author), ("reviewer", &written.reviewer)] {
            if !is_github_name(name) {
                bail!("{place}: the {field} {name:?} is not a GitHub user name");
            }
        }
        if written.author.eq_ignore_ascii_case(&written.reviewer) {
            bail!("{place}: the reviewer is the author; someone else must review");
        }
        if !is_day(&written.updated) {
            bail!(
                "{place}: updated must be a day like 2026-10-12, not {:?}",
                written.updated
            );
        }
        records.push(EstimateRecord {
            job: parsed.job.clone(),
            scope: written.scope,
            class: written.class,
            make: written.make,
            model: written.model,
            year_from: written.year_from,
            year_to: written.year_to,
            engine: written.engine,
            low,
            typical,
            high,
            notes,
            author: written.author,
            reviewer: written.reviewer,
            updated: written.updated,
        });
    }
    Ok(records)
}

/// An estimate with its make and model as catalog ids.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Estimate {
    pub record: EstimateRecord,
    pub make_id: Option<i64>,
    pub model_id: Option<i64>,
}

/// The catalog ids of a make and a model, by their id forms.
fn model_ids(catalog: &Connection, make: &str, model: &str) -> Result<(i64, i64)> {
    let make_id: Option<i64> = catalog
        .query_row(
            "SELECT id FROM catalog_make WHERE slug = ?1",
            [make],
            |row| row.get(0),
        )
        .optional()?;
    let Some(make_id) = make_id else {
        bail!("{make} is not a make of the catalog");
    };
    let model_id: Option<i64> = catalog
        .query_row(
            "SELECT id FROM catalog_model WHERE make_id = ?1 AND slug = ?2",
            params![make_id, model],
            |row| row.get(0),
        )
        .optional()?;
    match model_id {
        Some(model_id) => Ok((make_id, model_id)),
        None => bail!("{make} has no model {model} in the catalog"),
    }
}

/// The id forms of the engines the catalog lists for one model year.
/// `None` when the catalog has no such model year.
fn engines_of(catalog: &Connection, model_id: i64, year: u16) -> Result<Option<Vec<String>>> {
    let listed: Option<Option<i64>> = catalog
        .query_row(
            "SELECT detail_id FROM catalog_vehicle WHERE model_id = ?1 AND year = ?2",
            params![model_id, year],
            |row| row.get(0),
        )
        .optional()?;
    let Some(detail_id) = listed else {
        return Ok(None);
    };
    let mut engines = Vec::new();
    if let Some(detail_id) = detail_id {
        let mut statement =
            catalog.prepare("SELECT label FROM catalog_engine WHERE detail_id = ?1")?;
        let labels = statement.query_map([detail_id], |row| row.get::<_, String>(0))?;
        for label in labels {
            engines.push(slug(&label?));
        }
    }
    Ok(Some(engines))
}

/// Every estimate of `<curated>/labor`, checked against the job list and
/// the catalog: the job exists, the vehicle and the engine exist for
/// every model year named, and no job has two figures for one scope.
pub fn load(curated: &Path, catalog: &Connection) -> Result<Vec<Estimate>> {
    let found = files(&curated.join("labor"))?;
    if found.is_empty() {
        return Ok(Vec::new());
    }
    let components = Components::load(curated)?;
    let jobs = Jobs::load(curated, &components)?;
    let Some(jobs) = jobs.filter(|jobs| jobs.status == Status::Reviewed) else {
        bail!(
            "data/labor has estimates but data/jobs.yaml is still a draft; review the job list first"
        );
    };
    let mut estimates = Vec::new();
    // What each job already has a figure for.
    let mut held: HashSet<String> = HashSet::new();
    for (name, yaml) in found {
        if !jobs.list.iter().any(|job| job.id == name) {
            bail!("data/labor/{name}.yaml: {name} is not a job of jobs.yaml");
        }
        for record in parse(&name, &yaml)? {
            let job = &record.job;
            let (Some(make), Some(model)) = (record.make.as_deref(), record.model.as_deref())
            else {
                let class = record.class.clone().unwrap_or_default();
                if !held.insert(format!("{job}|class|{class}")) {
                    bail!("data/labor: {job} has two class estimates for {class}");
                }
                estimates.push(Estimate {
                    record,
                    make_id: None,
                    model_id: None,
                });
                continue;
            };
            let (make_id, model_id) = model_ids(catalog, make, model)
                .with_context(|| format!("data/labor/{name}.yaml"))?;
            let scope = record.scope.as_str();
            match (record.year_from, record.year_to) {
                (Some(from), Some(to)) => {
                    for year in from..=to {
                        let Some(engines) = engines_of(catalog, model_id, year)? else {
                            bail!(
                                "data/labor/{name}.yaml: the catalog has no {year} {make} {model}"
                            );
                        };
                        let engine = record.engine.clone().unwrap_or_default();
                        if scope == "engine" && !engines.contains(&engine) {
                            bail!(
                                "data/labor/{name}.yaml: the {year} {make} {model} has no engine {engine}"
                            );
                        }
                        if !held.insert(format!("{job}|{scope}|{model_id}|{year}|{engine}")) {
                            bail!(
                                "data/labor: {job} has two {scope} estimates that cover the {year} {make} {model}"
                            );
                        }
                    }
                }
                _ => {
                    if !held.insert(format!("{job}|model|{model_id}")) {
                        bail!("data/labor: {job} has two model estimates for {make} {model}");
                    }
                }
            }
            estimates.push(Estimate {
                record,
                make_id: Some(make_id),
                model_id: Some(model_id),
            });
        }
    }
    Ok(estimates)
}

// ----- measured times, from Wenmar Pro -----

/// What the build says of a field an observed table may not have.
pub const NOTHING_ELSE: &str = "an observed table holds only generated and rows, and a row only job, scope, class, make, model, year_from, year_to, engine, median, low, high, jobs and shops: nothing that could identify a shop, a customer, a VIN or a date may be in it";

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ObservedRow {
    pub job: String,
    pub scope: String,
    #[serde(default)]
    pub class: Option<String>,
    #[serde(default)]
    pub make: Option<String>,
    #[serde(default)]
    pub model: Option<String>,
    #[serde(default)]
    pub year_from: Option<u16>,
    #[serde(default)]
    pub year_to: Option<u16>,
    #[serde(default)]
    pub engine: Option<String>,
    /// Whole hundredths of an hour.
    pub median: i64,
    pub low: i64,
    pub high: i64,
    /// Repair orders counted, and the shops they came from.
    pub jobs: i64,
    pub shops: i64,
}

/// A table of measured times, checked for shape.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ObservedTable {
    /// The day the table was produced, as `2027-02-01`.
    pub generated: String,
    pub rows: Vec<ObservedRow>,
}

/// Whether `text` is 17 characters a VIN may hold.
fn looks_like_a_vin(text: &str) -> bool {
    text.len() == 17
        && text.bytes().all(|byte| {
            byte.is_ascii_alphanumeric() && !matches!(byte.to_ascii_uppercase(), b'I' | b'O' | b'Q')
        })
}

/// Reads the text of `observed.yaml` and checks everything that can be
/// checked without the job list and the catalog: no field but the agreed
/// ones, nothing that looks like a VIN, every row at or over the
/// publishing threshold, and figures in order.
pub fn parse_observed(yaml: &str) -> Result<ObservedTable> {
    let table: ObservedTable = serde_saphyr::from_str(yaml)
        .with_context(|| format!("reading data/labor/observed/observed.yaml; {NOTHING_ELSE}"))?;
    if !is_day(&table.generated) {
        bail!(
            "data/labor/observed: generated must be a day like 2027-02-01, not {:?}",
            table.generated
        );
    }
    for (index, row) in table.rows.iter().enumerate() {
        let place = format!("data/labor/observed: row {}", index + 1);
        let texts = [
            Some(&row.job),
            Some(&row.scope),
            row.class.as_ref(),
            row.make.as_ref(),
            row.model.as_ref(),
            row.engine.as_ref(),
        ];
        if texts
            .into_iter()
            .flatten()
            .any(|text| looks_like_a_vin(text))
        {
            bail!("{place} holds what looks like a VIN; {NOTHING_ELSE}");
        }
        if row.shops < MIN_SHOPS {
            bail!(
                "{place} has {} shops behind it; the least is {MIN_SHOPS}",
                row.shops
            );
        }
        if row.jobs < MIN_JOBS {
            bail!(
                "{place} has {} repair orders behind it; the least is {MIN_JOBS}",
                row.jobs
            );
        }
        if row.shops > row.jobs {
            bail!("{place} has more shops than repair orders");
        }
        for (field, value) in [("low", row.low), ("median", row.median), ("high", row.high)] {
            if !(1..=MOST_HUNDREDTHS).contains(&value) {
                bail!("{place}: {field} must be hundredths of an hour from 1 to {MOST_HUNDREDTHS}");
            }
        }
        if !(row.low <= row.median && row.median <= row.high) {
            bail!("{place}: low, median and high must be in that order");
        }
        let vehicle = row.make.is_some() || row.model.is_some();
        let years = row.year_from.is_some() || row.year_to.is_some();
        let named =
            row.make.as_deref().is_some_and(is_slug) && row.model.as_deref().is_some_and(is_slug);
        let good = match row.scope.as_str() {
            "class" => {
                let Some(class) = row.class.as_deref() else {
                    bail!("{place}: a class row needs class");
                };
                if class_name(class).is_none() {
                    bail!("{place}: {class} is not a class");
                }
                !vehicle && !years && row.engine.is_none()
            }
            "model" => row.class.is_none() && named && !years && row.engine.is_none(),
            "generation" | "engine" => {
                let ordered =
                    matches!((row.year_from, row.year_to), (Some(from), Some(to)) if from <= to);
                let engine = row.engine.as_deref().is_some_and(is_slug);
                row.class.is_none()
                    && named
                    && ordered
                    && (engine == (row.scope == "engine"))
                    && (row.engine.is_some() == (row.scope == "engine"))
            }
            other => bail!(
                "{place}: the scope {other} is not one of class, model, generation and engine"
            ),
        };
        if !good {
            bail!(
                "{place}: a {} row has the wrong fields; see data/labor/README.md",
                row.scope
            );
        }
    }
    Ok(table)
}

/// A measured row with its make and model as catalog ids.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Observed {
    pub row: ObservedRow,
    pub make_id: Option<i64>,
    pub model_id: Option<i64>,
}

/// The measured table of `<curated>/labor/observed`, checked against the
/// job list and the catalog as an expert estimate is. `None` when there is
/// no table. The directory may hold one: a new table replaces the old.
pub fn load_observed(
    curated: &Path,
    catalog: &Connection,
) -> Result<Option<(String, Vec<Observed>)>> {
    let found = files(&curated.join("labor").join("observed"))?;
    let yaml = match found.as_slice() {
        [] => return Ok(None),
        [(_, yaml)] => yaml,
        several => bail!(
            "data/labor/observed holds {} tables; a new table replaces the old one",
            several.len()
        ),
    };
    let table = parse_observed(yaml)?;
    let components = Components::load(curated)?;
    let jobs = Jobs::load(curated, &components)?;
    let Some(jobs) = jobs.filter(|jobs| jobs.status == Status::Reviewed) else {
        bail!("data/labor/observed has a table but data/jobs.yaml is still a draft");
    };
    let mut held: HashSet<String> = HashSet::new();
    let mut rows = Vec::with_capacity(table.rows.len());
    for row in table.rows {
        let job = &row.job;
        if !jobs.list.iter().any(|listed| &listed.id == job) {
            bail!("data/labor/observed: {job} is not a job of jobs.yaml");
        }
        let scope = row.scope.as_str();
        let (Some(make), Some(model)) = (row.make.as_deref(), row.model.as_deref()) else {
            let class = row.class.clone().unwrap_or_default();
            if !held.insert(format!("{job}|class|{class}")) {
                bail!("data/labor/observed: {job} has two measured class rows for {class}");
            }
            rows.push(Observed {
                row,
                make_id: None,
                model_id: None,
            });
            continue;
        };
        let (make_id, model_id) = model_ids(catalog, make, model).context("data/labor/observed")?;
        match (row.year_from, row.year_to) {
            (Some(from), Some(to)) => {
                for year in from..=to {
                    let Some(engines) = engines_of(catalog, model_id, year)? else {
                        bail!("data/labor/observed: the catalog has no {year} {make} {model}");
                    };
                    let engine = row.engine.clone().unwrap_or_default();
                    if scope == "engine" && !engines.contains(&engine) {
                        bail!(
                            "data/labor/observed: the {year} {make} {model} has no engine {engine}"
                        );
                    }
                    if !held.insert(format!("{job}|{scope}|{model_id}|{year}|{engine}")) {
                        bail!(
                            "data/labor/observed: {job} has two measured {scope} rows that cover the {year} {make} {model}"
                        );
                    }
                }
            }
            _ => {
                if !held.insert(format!("{job}|model|{model_id}")) {
                    bail!(
                        "data/labor/observed: {job} has two measured model rows for {make} {model}"
                    );
                }
            }
        }
        rows.push(Observed {
            row,
            make_id: Some(make_id),
            model_id: Some(model_id),
        });
    }
    Ok(Some((table.generated, rows)))
}

// ----- figures worked out by a model -----

/// The most hours an `ai` figure may be, in hundredths: 40.
pub const AI_MOST_HUNDREDTHS: i64 = 4_000;
/// An `ai` figure's high may be at most this many times its typical.
pub const AI_MOST_SPREAD: i64 = 3;
/// The longest a model's or a method's name may be, in characters.
pub const AI_LONGEST_NAME: usize = 80;

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
struct AiWritten {
    scope: String,
    #[serde(default)]
    class: Option<String>,
    #[serde(default)]
    make: Option<String>,
    #[serde(default)]
    model: Option<String>,
    #[serde(default)]
    year_from: Option<u16>,
    #[serde(default)]
    year_to: Option<u16>,
    #[serde(default)]
    engine: Option<String>,
    low: String,
    typical: String,
    high: String,
    confidence: i64,
    inputs: Vec<String>,
    #[serde(default)]
    notes: Option<String>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct AiFile {
    job: String,
    method: String,
    model: String,
    generated: String,
    estimates: Vec<AiWritten>,
}

/// One figure of an AI file, checked for shape.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AiRecord {
    pub job: String,
    pub scope: String,
    pub class: Option<String>,
    pub make: Option<String>,
    pub model: Option<String>,
    pub year_from: Option<u16>,
    pub year_to: Option<u16>,
    pub engine: Option<String>,
    /// Whole hundredths of an hour.
    pub low: i64,
    pub typical: i64,
    pub high: i64,
    pub confidence_pct: i64,
    pub inputs: Vec<String>,
    pub notes: Option<String>,
    /// The model that answered, the prompt version, the day.
    pub llm: String,
    pub method: String,
    pub generated: String,
}

/// Whether `text` is a name a file may give a model or a method: not
/// empty, no longer than [`AI_LONGEST_NAME`], no control characters.
fn is_ai_name(text: &str) -> bool {
    !text.trim().is_empty()
        && text.chars().count() <= AI_LONGEST_NAME
        && !text.chars().any(char::is_control)
}

/// The scope fields of one figure: the ones its scope needs and no other.
/// The same rules as a written estimate's.
#[allow(clippy::too_many_arguments)]
fn check_scope_fields(
    place: &str,
    scope: &str,
    class: Option<&str>,
    make: Option<&str>,
    model: Option<&str>,
    year_from: Option<u16>,
    year_to: Option<u16>,
    engine: Option<&str>,
) -> Result<()> {
    let vehicle = make.is_some() || model.is_some();
    let years = year_from.is_some() || year_to.is_some();
    match scope {
        "class" => {
            let Some(class) = class else {
                bail!("{place}: a class figure needs class");
            };
            if class_name(class).is_none() {
                bail!("{place}: {class} is not a class");
            }
            if vehicle || years || engine.is_some() {
                bail!("{place}: a class figure takes no make, model, model year or engine");
            }
        }
        "model" | "generation" | "engine" => {
            if class.is_some() {
                bail!("{place}: a {scope} figure takes no class");
            }
            if !(make.is_some_and(is_slug) && model.is_some_and(is_slug)) {
                bail!("{place}: a {scope} figure needs make and model, as id forms");
            }
            if scope == "model" {
                if years || engine.is_some() {
                    bail!("{place}: a model figure takes no model year and no engine");
                }
            } else {
                let (Some(from), Some(to)) = (year_from, year_to) else {
                    bail!("{place}: a {scope} figure needs year_from and year_to");
                };
                if from > to {
                    bail!("{place}: year_from is after year_to");
                }
                match (scope, engine) {
                    ("engine", Some(engine)) if is_slug(engine) => {}
                    ("engine", _) => bail!("{place}: an engine figure needs engine, as an id form"),
                    (_, Some(_)) => bail!("{place}: a generation figure takes no engine"),
                    (_, None) => {}
                }
            }
        }
        other => {
            bail!("{place}: the scope {other} is not one of class, model, generation and engine")
        }
    }
    Ok(())
}

/// Reads the text of one AI file, `ai/<name>.yaml`, and checks everything
/// that can be checked without the job list and the catalog, with the
/// guardrails a model's answer must pass: hours from 0.01 to 40 and in
/// order, a high at most three times the typical, a confidence of at most
/// 60, inputs from [`AI_INPUTS`].
pub fn parse_ai(name: &str, yaml: &str) -> Result<Vec<AiRecord>> {
    let file = format!("data/labor/ai/{name}.yaml");
    let parsed: AiFile = serde_saphyr::from_str(yaml).with_context(|| format!("reading {file}"))?;
    if parsed.job != name {
        bail!(
            "{file} holds the job {}; name the file after its job",
            parsed.job
        );
    }
    if !is_day(&parsed.generated) {
        bail!(
            "{file}: generated must be a day like 2026-10-30, not {:?}",
            parsed.generated
        );
    }
    for (field, value) in [("model", &parsed.model), ("method", &parsed.method)] {
        if !is_ai_name(value) {
            bail!("{file}: {field} must be a name of 1 to {AI_LONGEST_NAME} characters");
        }
    }
    let mut records = Vec::with_capacity(parsed.estimates.len());
    for (index, written) in parsed.estimates.into_iter().enumerate() {
        let place = format!("{file}, figure {}", index + 1);
        check_scope_fields(
            &place,
            &written.scope,
            written.class.as_deref(),
            written.make.as_deref(),
            written.model.as_deref(),
            written.year_from,
            written.year_to,
            written.engine.as_deref(),
        )?;
        let hours = |field: &str, text: &str| {
            hundredths(text)
                .filter(|value| *value <= AI_MOST_HUNDREDTHS)
                .with_context(|| {
                    format!(
                        "{place}: {field} must be hours like \"1.5\", from 0.01 to 40, not {text:?}"
                    )
                })
        };
        let (low, typical, high) = (
            hours("low", &written.low)?,
            hours("typical", &written.typical)?,
            hours("high", &written.high)?,
        );
        if !(low <= typical && typical <= high) {
            bail!("{place}: low, typical and high must be in that order");
        }
        if high > AI_MOST_SPREAD * typical {
            bail!("{place}: high is more than {AI_MOST_SPREAD} times typical");
        }
        if !(0..=AI_MOST_CONFIDENCE).contains(&written.confidence) {
            bail!("{place}: confidence must be from 0 to {AI_MOST_CONFIDENCE}");
        }
        let mut seen = HashSet::new();
        if written.inputs.is_empty()
            || written
                .inputs
                .iter()
                .any(|input| !AI_INPUTS.contains(&input.as_str()) || !seen.insert(input))
        {
            bail!(
                "{place}: inputs must be one or more of {}, each once",
                AI_INPUTS.join(", ")
            );
        }
        let notes = written
            .notes
            .as_deref()
            .map(squeeze)
            .filter(|notes| !notes.is_empty());
        if notes
            .as_ref()
            .is_some_and(|notes| notes.chars().count() > LONGEST_NOTE)
        {
            bail!("{place}: notes is longer than {LONGEST_NOTE} characters");
        }
        records.push(AiRecord {
            job: parsed.job.clone(),
            scope: written.scope,
            class: written.class,
            make: written.make,
            model: written.model,
            year_from: written.year_from,
            year_to: written.year_to,
            engine: written.engine,
            low,
            typical,
            high,
            confidence_pct: written.confidence,
            inputs: written.inputs,
            notes,
            llm: parsed.model.clone(),
            method: parsed.method.clone(),
            generated: parsed.generated.clone(),
        });
    }
    Ok(records)
}

/// An AI figure with its make and model as catalog ids.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AiFigure {
    pub record: AiRecord,
    pub make_id: Option<i64>,
    pub model_id: Option<i64>,
}

/// Every figure of `<curated>/labor/ai`, checked against the job list and
/// the catalog as a written estimate is: the job exists, the vehicle and
/// the engine exist for every model year named, and no job has two AI
/// figures for one scope.
pub fn load_ai(curated: &Path, catalog: &Connection) -> Result<Vec<AiFigure>> {
    let found = files(&curated.join("labor").join("ai"))?;
    if found.is_empty() {
        return Ok(Vec::new());
    }
    let components = Components::load(curated)?;
    let jobs = Jobs::load(curated, &components)?;
    let Some(jobs) = jobs.filter(|jobs| jobs.status == Status::Reviewed) else {
        bail!("data/labor/ai has figures but data/jobs.yaml is still a draft");
    };
    let mut figures = Vec::new();
    let mut held: HashSet<String> = HashSet::new();
    for (name, yaml) in found {
        let file = format!("data/labor/ai/{name}.yaml");
        if !jobs.list.iter().any(|job| job.id == name) {
            bail!("{file}: {name} is not a job of jobs.yaml");
        }
        for record in parse_ai(&name, &yaml)? {
            let job = &record.job;
            let (Some(make), Some(model)) = (record.make.as_deref(), record.model.as_deref())
            else {
                let class = record.class.clone().unwrap_or_default();
                if !held.insert(format!("{job}|class|{class}")) {
                    bail!("{file}: two class figures for {class}");
                }
                figures.push(AiFigure {
                    record,
                    make_id: None,
                    model_id: None,
                });
                continue;
            };
            let (make_id, model_id) =
                model_ids(catalog, make, model).with_context(|| file.clone())?;
            let scope = record.scope.as_str();
            match (record.year_from, record.year_to) {
                (Some(from), Some(to)) => {
                    for year in from..=to {
                        let Some(engines) = engines_of(catalog, model_id, year)? else {
                            bail!("{file}: the catalog has no {year} {make} {model}");
                        };
                        let engine = record.engine.clone().unwrap_or_default();
                        if scope == "engine" && !engines.contains(&engine) {
                            bail!("{file}: the {year} {make} {model} has no engine {engine}");
                        }
                        if !held.insert(format!("{job}|{scope}|{model_id}|{year}|{engine}")) {
                            bail!("{file}: two {scope} figures cover the {year} {make} {model}");
                        }
                    }
                }
                _ => {
                    if !held.insert(format!("{job}|model|{model_id}")) {
                        bail!("{file}: two model figures for {make} {model}");
                    }
                }
            }
            figures.push(AiFigure {
                record,
                make_id: Some(make_id),
                model_id: Some(model_id),
            });
        }
    }
    Ok(figures)
}

/// Fills `vehicle_class`: for each light model year of the catalog, the
/// class EPA's size class puts it in (the one with the most EPA rows), or
/// that of the nearest model year of the same model that has one. Does
/// nothing when the file holds no specifications.
fn shape_classes(context: &ShapeContext<'_>) -> Result<()> {
    if !context.datasets().contains(Dataset::Specs) {
        return Ok(());
    }
    let transaction = context.transaction;
    // (model, year) to rows per class.
    let mut counted: BTreeMap<(i64, u16), HashMap<&'static str, i64>> = BTreeMap::new();
    {
        let mut statement = transaction.prepare(
            "SELECT f.model_id, f.year, s.size_class, COUNT(*)
             FROM vehicle_spec s
             JOIN fuel_rating f ON f.source = 'epa' AND f.source_id = s.epa_id
             WHERE f.model_id IS NOT NULL
             GROUP BY f.model_id, f.year, s.size_class",
        )?;
        let mut rows = statement.query([])?;
        while let Some(row) = rows.next()? {
            let (model_id, year, size_class, count): (i64, u16, String, i64) =
                (row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?);
            if let Some(class) = class_of_epa(&size_class) {
                *counted
                    .entry((model_id, year))
                    .or_default()
                    .entry(class)
                    .or_insert(0) += count;
            }
        }
    }
    // The class of each model year EPA rated: the most rows, and on a tie
    // the first of `CLASSES`.
    let mut known: BTreeMap<i64, Vec<(u16, &'static str)>> = BTreeMap::new();
    for ((model_id, year), classes) in &counted {
        let best = CLASSES
            .iter()
            .map(|(id, _)| *id)
            .filter(|id| classes.contains_key(id))
            .max_by_key(|id| {
                let place = CLASSES.iter().position(|(other, _)| other == id);
                (classes[id], std::cmp::Reverse(place))
            });
        if let Some(class) = best {
            known.entry(*model_id).or_default().push((*year, class));
        }
    }
    let mut vehicles = context.catalog.prepare(
        "SELECT model_id, year FROM catalog_vehicle WHERE light = 1 ORDER BY model_id, year",
    )?;
    let mut insert = transaction.prepare(
        "INSERT INTO vehicle_class (model_id, year, class, source) VALUES (?1, ?2, ?3, ?4)",
    )?;
    let mut rows = vehicles.query([])?;
    while let Some(row) = rows.next()? {
        let (model_id, year): (i64, u16) = (row.get(0)?, row.get(1)?);
        let Some(years) = known.get(&model_id) else {
            continue;
        };
        // The nearest year that has a class; the earlier one on a tie.
        let Some((from, class)) = years
            .iter()
            .min_by_key(|(other, _)| (other.abs_diff(year), *other))
        else {
            continue;
        };
        let source = if *from == year { "epa" } else { "nearest" };
        insert.execute(params![model_id, year, class, source])?;
    }
    Ok(())
}

/// Fills `labor_estimate` from `<curated>/labor` and `vehicle_class` from
/// the fuel tables, and registers the `labor` dataset. With no estimate
/// it does nothing at all. Runs in every build, after the fuel steps and
/// the job list.
pub fn shape(context: &mut ShapeContext<'_>) -> Result<()> {
    let estimates = load(context.curated, context.catalog)?;
    let observed = load_observed(context.curated, context.catalog)?;
    let ai = load_ai(context.curated, context.catalog)?;
    let (generated, measured) = match observed {
        Some((generated, rows)) => (Some(generated), rows),
        None => (None, Vec::new()),
    };
    let Some(as_of) = estimates
        .iter()
        .map(|estimate| estimate.record.updated.clone())
        .chain(generated.clone().filter(|_| !measured.is_empty()))
        .chain(ai.iter().map(|figure| figure.record.generated.clone()))
        .max()
    else {
        return Ok(());
    };
    let transaction = context.transaction;
    transaction.execute_batch(schema::LABOR)?;
    {
        let mut insert = transaction.prepare(
            "INSERT INTO labor_estimate (job_id, scope, class, make_id, model_id, year_from, year_to, engine, hours_low, hours_typical, hours_high, basis, notes, author, reviewer, updated)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, 'expert', ?12, ?13, ?14, ?15)",
        )?;
        for estimate in &estimates {
            let record = &estimate.record;
            insert.execute(params![
                record.job,
                record.scope,
                record.class,
                estimate.make_id,
                estimate.model_id,
                record.year_from,
                record.year_to,
                record.engine,
                record.low,
                record.typical,
                record.high,
                record.notes,
                record.author,
                record.reviewer,
                record.updated,
            ])?;
        }
    }
    if let Some(generated) = &generated {
        // A measured row has no author: it is nobody's estimate.
        let mut insert = transaction.prepare(
            "INSERT INTO labor_estimate (job_id, scope, class, make_id, model_id, year_from, year_to, engine, hours_low, hours_typical, hours_high, basis, sample_jobs, sample_shops, updated)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, 'observed', ?12, ?13, ?14)",
        )?;
        for found in &measured {
            let row = &found.row;
            insert.execute(params![
                row.job,
                row.scope,
                row.class,
                found.make_id,
                found.model_id,
                row.year_from,
                row.year_to,
                row.engine,
                row.low,
                row.median,
                row.high,
                row.jobs,
                row.shops,
                generated,
            ])?;
        }
    }
    if !ai.is_empty() {
        // A model's figure has no author either; `labor_ai` says how it was
        // worked out.
        transaction.execute_batch(schema::LABOR_AI)?;
        let mut insert = transaction.prepare(
            "INSERT INTO labor_estimate (job_id, scope, class, make_id, model_id, year_from, year_to, engine, hours_low, hours_typical, hours_high, basis, notes, updated)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, 'ai', ?12, ?13)",
        )?;
        let mut how = transaction.prepare(
            "INSERT INTO labor_ai (estimate_id, confidence_pct, model, method, inputs) VALUES (?1, ?2, ?3, ?4, ?5)",
        )?;
        for figure in &ai {
            let record = &figure.record;
            insert.execute(params![
                record.job,
                record.scope,
                record.class,
                figure.make_id,
                figure.model_id,
                record.year_from,
                record.year_to,
                record.engine,
                record.low,
                record.typical,
                record.high,
                record.notes,
                record.generated,
            ])?;
            how.execute(params![
                transaction.last_insert_rowid(),
                record.confidence_pct,
                record.llm,
                record.method,
                record.inputs.join(","),
            ])?;
        }
    }
    shape_classes(context)?;
    if let Some(ai_as_of) = ai
        .iter()
        .map(|figure| figure.record.generated.clone())
        .max()
    {
        context.register(
            Dataset::LaborAi,
            DatasetEntry {
                as_of: ai_as_of,
                rows: ai.len() as u64,
                licence: licence::CC_BY_4.to_owned(),
                attribution: ATTRIBUTION.to_owned(),
            },
        )?;
    }
    context.register(
        Dataset::Labor,
        DatasetEntry {
            as_of,
            rows: (estimates.len() + measured.len() + ai.len()) as u64,
            licence: licence::CC_BY_4.to_owned(),
            attribution: ATTRIBUTION.to_owned(),
        },
    )
}
