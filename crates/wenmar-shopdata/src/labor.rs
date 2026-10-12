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

/// What every labour answer and page says.
pub const NOTE: &str =
    "An estimate, not a quote. Rust, seized fasteners and added equipment change the time.";

/// The scopes, narrowest first: the order a lookup tries them in.
pub const SCOPES: [&str; 4] = ["engine", "generation", "model", "class"];

/// The two kinds of figure.
pub const BASES: [&str; 2] = ["expert", "observed"];

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
    /// The run of model years, for a generation or an engine.
    pub year_from: Option<u16>,
    pub year_to: Option<u16>,
    /// The engine's id form, for an engine-scope figure.
    pub engine: Option<String>,
    /// Whole hundredths of an hour.
    pub hours_low: i64,
    pub hours_typical: i64,
    pub hours_high: i64,
    /// `expert` or `observed`.
    pub basis: String,
    /// How many jobs and shops stand behind an observed figure.
    pub sample_jobs: Option<i64>,
    pub sample_shops: Option<i64>,
    pub notes: Option<String>,
    /// The day the figure was last changed, as `2026-10-12`.
    pub updated: String,
}

/// The columns [`estimate`] reads.
const COLUMNS: &str = "e.job_id, j.name, e.scope, e.class, e.year_from, e.year_to, e.engine, e.hours_low, e.hours_typical, e.hours_high, e.basis, e.sample_jobs, e.sample_shops, e.notes, e.updated";

/// Every estimate that covers one vehicle, the narrowest scope of each job
/// first: `?1` model id, `?2` model year, `?3` the engine's id form or
/// NULL, `?4` a job id or NULL for every job. The first row of each job is
/// the answer: within a scope a measured figure comes before a written
/// one. Jobs come in the order of the job list.
pub const LABOR_FOR_VEHICLE: &str = "
SELECT e.job_id, j.name, e.scope, e.class, e.year_from, e.year_to, e.engine, e.hours_low, e.hours_typical, e.hours_high, e.basis, e.sample_jobs, e.sample_shops, e.notes, e.updated
FROM labor_estimate e
JOIN job j ON j.id = e.job_id
WHERE (?4 IS NULL OR e.job_id = ?4)
  AND (   (e.scope = 'engine' AND e.model_id = ?1 AND ?2 BETWEEN e.year_from AND e.year_to AND e.engine = ?3)
       OR (e.scope = 'generation' AND e.model_id = ?1 AND ?2 BETWEEN e.year_from AND e.year_to)
       OR (e.scope = 'model' AND e.model_id = ?1)
       OR (e.scope = 'class' AND e.class = (SELECT class FROM vehicle_class WHERE model_id = ?1 AND year = ?2)))
ORDER BY j.position,
         CASE e.scope WHEN 'engine' THEN 0 WHEN 'generation' THEN 1 WHEN 'model' THEN 2 ELSE 3 END,
         CASE e.basis WHEN 'observed' THEN 0 ELSE 1 END,
         e.id";

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

/// Reads a [`LaborEstimate`] from [`COLUMNS`].
fn estimate(row: &[Value]) -> Result<LaborEstimate, ShopError> {
    Ok(LaborEstimate {
        job_id: text(row, 0)?,
        job_name: text(row, 1)?,
        scope: text(row, 2)?,
        class: optional_text(row, 3),
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
            LABOR_FOR_VEHICLE,
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

    /// The class-scope figures of one job, in the order of [`CLASSES`]:
    /// what a job's own page shows. Where a class has a measured figure
    /// and a written one, the measured one is given.
    pub fn class_estimates_for_job(&self, job: &str) -> Result<Vec<LaborEstimate>, ShopError> {
        self.require(Dataset::Labor)?;
        let rows = self.query(
            &format!(
                "SELECT {COLUMNS} FROM labor_estimate e JOIN job j ON j.id = e.job_id
                 WHERE e.job_id = ?1 AND e.scope = 'class'
                 ORDER BY CASE e.basis WHEN 'observed' THEN 0 ELSE 1 END, e.id"
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
