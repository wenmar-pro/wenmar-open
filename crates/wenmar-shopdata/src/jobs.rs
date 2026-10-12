//! The repair jobs a labour time can be about, from `data/jobs.yaml`.
//!
//! A job's id is what makes two shops' times for it comparable. It never
//! changes once used.

use serde::Serialize;
use wenmar_vehicles::{Source, Value};

use crate::dataset::Dataset;
use crate::error::ShopError;
use crate::shop::ShopData;

/// A job as a list shows it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct JobSummary {
    pub id: String,
    pub name: String,
    /// The id of the system of `components.yaml` the job belongs to.
    pub system: String,
}

/// A component a job acts on: an entry of `data/components.yaml`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct JobComponent {
    pub id: String,
    pub name: String,
    /// The system a part belongs to. `None` for a system.
    pub parent: Option<String>,
}

/// One job of `data/jobs.yaml`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Job {
    pub id: String,
    pub name: String,
    pub system: String,
    /// The name of the system, as `components.yaml` has it.
    pub system_name: String,
    /// In the order the record lists them.
    pub components: Vec<JobComponent>,
    /// What the job covers.
    pub includes: Vec<String>,
    /// What it does not, so that two shops' times mean the same work.
    pub excludes: Vec<String>,
    /// In the record's order.
    pub related: Vec<JobSummary>,
}

const TABLE: &str = "job";

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

fn summary(row: &[Value]) -> Result<JobSummary, ShopError> {
    Ok(JobSummary {
        id: text(row, 0)?,
        name: text(row, 1)?,
        system: text(row, 2)?,
    })
}

impl<S: Source> ShopData<S> {
    /// The day the job list was last changed, as `2026-10-12`. `None` when
    /// the file holds no jobs.
    pub fn jobs_as_of(&self) -> Option<&str> {
        self.datasets()
            .get(Dataset::Jobs)
            .map(|entry| entry.as_of.as_str())
    }

    /// Every job, in the order of the file.
    pub fn jobs(&self) -> Result<Vec<JobSummary>, ShopError> {
        self.require(Dataset::Jobs)?;
        self.query("SELECT id, name, system FROM job ORDER BY position", &[])?
            .iter()
            .map(|row| summary(row))
            .collect()
    }

    /// The components one job acts on, in the record's order. Empty for
    /// an id that is not a job.
    pub fn job_components(&self, id: &str) -> Result<Vec<JobComponent>, ShopError> {
        self.require(Dataset::Jobs)?;
        self.query(
            "SELECT k.id, k.name, k.parent FROM job_component c JOIN component k ON k.id = c.component_id WHERE c.job_id = ?1 ORDER BY c.position",
            &[id.into()],
        )?
        .iter()
        .map(|row| {
            Ok(JobComponent {
                id: text(row, 0)?,
                name: text(row, 1)?,
                parent: row.get(2).and_then(Value::text).map(str::to_owned),
            })
        })
        .collect()
    }

    /// One job. `None` when the list has none of that id.
    pub fn job(&self, id: &str) -> Result<Option<Job>, ShopError> {
        self.require(Dataset::Jobs)?;
        let rows = self.query(
            "SELECT j.id, j.name, j.system, j.includes, j.excludes, j.related, COALESCE(k.name, j.system) FROM job j LEFT JOIN component k ON k.id = j.system WHERE j.id = ?1",
            &[id.into()],
        )?;
        let Some(row) = rows.first() else {
            return Ok(None);
        };
        // One statement for all the related jobs, then the record's order.
        let wanted = list(row, 5)?;
        let mut related = Vec::with_capacity(wanted.len());
        if !wanted.is_empty() {
            let marks: Vec<String> = (1..=wanted.len())
                .map(|number| format!("?{number}"))
                .collect();
            let params: Vec<Value> = wanted.iter().map(|id| id.as_str().into()).collect();
            let found = self.query(
                &format!(
                    "SELECT id, name, system FROM job WHERE id IN ({})",
                    marks.join(", ")
                ),
                &params,
            )?;
            for id in &wanted {
                let row = found
                    .iter()
                    .find(|row| row.first().and_then(Value::text) == Some(id.as_str()));
                if let Some(row) = row {
                    related.push(summary(row)?);
                }
            }
        }
        Ok(Some(Job {
            id: text(row, 0)?,
            name: text(row, 1)?,
            system: text(row, 2)?,
            system_name: text(row, 6)?,
            components: self.job_components(id)?,
            includes: list(row, 3)?,
            excludes: list(row, 4)?,
            related,
        }))
    }
}
