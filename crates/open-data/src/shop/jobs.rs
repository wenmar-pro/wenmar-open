//! `data/jobs.yaml`: the repair jobs a labour time can be about, built
//! into the `job` and `job_component` tables once the list is reviewed.

use std::collections::HashSet;
use std::path::Path;

use anyhow::{Context, Result, bail};
use rusqlite::params;
use serde::Deserialize;
use wenmar_shopdata::{Dataset, DatasetEntry, licence, schema};
use wenmar_vehicles::text::{is_slug, squeeze};

use crate::shop::build::ShapeContext;
use crate::shop::codes::is_day;
use crate::shop::components::Components;

/// The longest a job's name may be, in characters.
pub const LONGEST_NAME: usize = 80;
/// The longest an entry of `includes` or `excludes` may be.
pub const LONGEST_ENTRY: usize = 120;
/// The most entries `includes` and `excludes` may each have.
pub const MOST_ENTRIES: usize = 12;
/// The most components a job may name.
pub const MOST_COMPONENTS: usize = 8;
/// The most related jobs a job may name.
pub const MOST_RELATED: usize = 10;

/// Whether the list is complete.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Status {
    /// Checked and not built: some job does not yet say what it covers.
    Draft,
    /// Built into the shop file.
    Reviewed,
}

/// One job of the file, with its text tidied.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct JobRecord {
    pub id: String,
    pub name: String,
    pub system: String,
    #[serde(default)]
    pub components: Vec<String>,
    #[serde(default)]
    pub includes: Vec<String>,
    #[serde(default)]
    pub excludes: Vec<String>,
    #[serde(default)]
    pub related: Vec<String>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct JobsFile {
    status: String,
    updated: String,
    jobs: Vec<JobRecord>,
}

/// Everything `jobs.yaml` says, checked.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Jobs {
    pub status: Status,
    /// The day of the last change, as `2026-10-12`.
    pub updated: String,
    pub list: Vec<JobRecord>,
}

fn entries(id: &str, field: &str, list: &[String]) -> Result<Vec<String>> {
    if list.len() > MOST_ENTRIES {
        bail!("jobs.yaml: {id} has more than {MOST_ENTRIES} entries in {field}");
    }
    let mut tidy = Vec::with_capacity(list.len());
    for entry in list {
        let entry = squeeze(entry);
        if entry.is_empty() {
            bail!("jobs.yaml: {id} has an empty entry in {field}");
        }
        if entry.chars().count() > LONGEST_ENTRY {
            bail!("jobs.yaml: {id}: an entry in {field} is longer than {LONGEST_ENTRY} characters");
        }
        tidy.push(entry);
    }
    Ok(tidy)
}

impl Jobs {
    /// Reads `jobs.yaml` under `directory`. `None` when there is no such
    /// file.
    pub fn load(directory: &Path, components: &Components) -> Result<Option<Jobs>> {
        let path = directory.join("jobs.yaml");
        if !path.is_file() {
            return Ok(None);
        }
        let text = std::fs::read_to_string(&path)
            .with_context(|| format!("reading {}", path.display()))?;
        Jobs::parse(&text, components).map(Some)
    }

    /// Reads the text of the file and checks it against the components.
    pub fn parse(yaml: &str, components: &Components) -> Result<Jobs> {
        let file: JobsFile = serde_saphyr::from_str(yaml).context("reading jobs.yaml")?;
        let status = match file.status.as_str() {
            "draft" => Status::Draft,
            "reviewed" => Status::Reviewed,
            other => bail!("jobs.yaml: status must be draft or reviewed, not {other:?}"),
        };
        if !is_day(&file.updated) {
            bail!(
                "jobs.yaml: updated must be a day like 2026-10-12, not {:?}",
                file.updated
            );
        }
        let mut ids: HashSet<String> = HashSet::new();
        let mut list = Vec::with_capacity(file.jobs.len());
        for record in file.jobs {
            let id = record.id.clone();
            if !is_slug(&id) {
                bail!("jobs.yaml: {id:?} is not an id; use lowercase words joined by hyphens");
            }
            if !ids.insert(id.clone()) {
                bail!("jobs.yaml lists {id} twice");
            }
            let name = squeeze(&record.name);
            if name.is_empty() {
                bail!("jobs.yaml: {id} has no name");
            }
            if name.chars().count() > LONGEST_NAME {
                bail!("jobs.yaml: {id}: the name is longer than {LONGEST_NAME} characters");
            }
            let is_system = components
                .list
                .iter()
                .any(|entry| entry.id == record.system && entry.parent.is_none());
            if !is_system {
                bail!(
                    "jobs.yaml: {id}: {} is not a system of components.yaml",
                    record.system
                );
            }
            if record.components.len() > MOST_COMPONENTS {
                bail!("jobs.yaml: {id} names more than {MOST_COMPONENTS} components");
            }
            for (index, component) in record.components.iter().enumerate() {
                if !components.list.iter().any(|entry| &entry.id == component) {
                    bail!("jobs.yaml: {id}: {component} is not a component of components.yaml");
                }
                if record.components[..index].contains(component) {
                    bail!("jobs.yaml: {id} names the component {component} twice");
                }
            }
            let includes = entries(&id, "includes", &record.includes)?;
            let excludes = entries(&id, "excludes", &record.excludes)?;
            if status == Status::Reviewed && includes.is_empty() {
                bail!(
                    "jobs.yaml: {id} has no includes; a reviewed list says what every job covers"
                );
            }
            if record.related.len() > MOST_RELATED {
                bail!("jobs.yaml: {id} has more than {MOST_RELATED} related jobs");
            }
            list.push(JobRecord {
                name,
                includes,
                excludes,
                ..record
            });
        }
        // A related job is in the list, whatever the order of the file.
        for record in &list {
            let id = &record.id;
            for (index, other) in record.related.iter().enumerate() {
                if other == id {
                    bail!("jobs.yaml: {id} is related to itself");
                }
                if !ids.contains(other) {
                    bail!("jobs.yaml: {id}: the related job {other} is not in the list");
                }
                if record.related[..index].contains(other) {
                    bail!("jobs.yaml: {id} lists the related job {other} twice");
                }
            }
        }
        Ok(Jobs {
            status,
            updated: file.updated,
            list,
        })
    }
}

/// Checks `<curated>/jobs.yaml` and, when it is reviewed, fills `job` and
/// `job_component` and registers the `jobs` dataset. With no file, or a
/// draft, it builds nothing. Runs in every build.
pub fn shape(context: &mut ShapeContext<'_>) -> Result<()> {
    if !context.curated.join("jobs.yaml").is_file() {
        return Ok(());
    }
    let components = Components::load(context.curated)?;
    let Some(jobs) = Jobs::load(context.curated, &components)? else {
        return Ok(());
    };
    if jobs.status == Status::Draft || jobs.list.is_empty() {
        return Ok(());
    }
    let transaction = context.transaction;
    transaction.execute_batch(schema::JOBS)?;
    {
        let mut job = transaction.prepare(
            "INSERT INTO job (id, name, system, includes, excludes, related, position)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
        )?;
        let mut component = transaction.prepare(
            "INSERT INTO job_component (job_id, component_id, position) VALUES (?1, ?2, ?3)",
        )?;
        for (position, record) in jobs.list.iter().enumerate() {
            job.execute(params![
                record.id,
                record.name,
                record.system,
                serde_json::to_string(&record.includes)?,
                serde_json::to_string(&record.excludes)?,
                serde_json::to_string(&record.related)?,
                position as i64,
            ])?;
            for (order, id) in record.components.iter().enumerate() {
                component.execute(params![record.id, id, order as i64])?;
            }
        }
    }
    context.register(
        Dataset::Jobs,
        DatasetEntry {
            as_of: jobs.updated,
            rows: jobs.list.len() as u64,
            licence: licence::MIT.to_owned(),
            attribution: String::new(),
        },
    )
}
