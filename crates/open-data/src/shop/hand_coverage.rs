//! Which models have hand-read records and which still have none.
//!
//! The repository holds no sales figures, so this cannot say which model
//! sells best. What it has is the order of the makes in
//! `data/catalog/makes.yaml`, which the core data file carries as
//! `catalog_make.rank`: the report goes through the ranked makes in that
//! order and lists, for each, the light models that have no record.

use std::collections::BTreeSet;
use std::fmt::Write as _;
use std::path::Path;

use anyhow::Result;
use rusqlite::Connection;

use crate::shop::hand::{load_fluids, load_tires};

/// One ranked make of the catalog.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MakeCoverage {
    pub make: String,
    pub rank: i64,
    /// Light models with a model year of `from_year` or later.
    pub models: usize,
    pub with_fluids: usize,
    pub with_tires: usize,
    /// The names of the models with neither, by name.
    pub missing: Vec<String>,
}

/// The ranked makes, in rank order, each with its models from `from_year`
/// on and whether `curated/fluids` and `curated/tires` have a file with a
/// record for them. A record is counted by its file: the report does not
/// check it against the catalog, which the build does.
pub fn report(catalog: &Connection, curated: &Path, from_year: u16) -> Result<Vec<MakeCoverage>> {
    let have = |files: Vec<(String, String, bool)>| -> BTreeSet<(String, String)> {
        files
            .into_iter()
            .filter(|(_, _, any)| *any)
            .map(|(make, model, _)| (make, model))
            .collect()
    };
    let fluids = have(
        load_fluids(curated)?
            .into_iter()
            .map(|file| (file.make, file.model, !file.records.is_empty()))
            .collect(),
    );
    let tires = have(
        load_tires(curated)?
            .into_iter()
            .map(|file| (file.make, file.model, !file.records.is_empty()))
            .collect(),
    );
    let mut makes = catalog.prepare(
        "SELECT id, slug, name, rank FROM catalog_make WHERE rank IS NOT NULL ORDER BY rank, name",
    )?;
    let mut models = catalog.prepare(
        "SELECT slug, name FROM catalog_model
         WHERE make_id = ?1 AND light = 1 AND year_to >= ?2 ORDER BY name, slug",
    )?;
    let mut found = Vec::new();
    let rows = makes.query_map([], |row| {
        Ok((
            row.get::<_, i64>(0)?,
            row.get::<_, String>(1)?,
            row.get::<_, String>(2)?,
            row.get::<_, i64>(3)?,
        ))
    })?;
    for row in rows {
        let (make_id, make_slug, make, rank) = row?;
        let mut coverage = MakeCoverage {
            make,
            rank,
            models: 0,
            with_fluids: 0,
            with_tires: 0,
            missing: Vec::new(),
        };
        let listed = models.query_map(rusqlite::params![make_id, from_year], |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
        })?;
        for model in listed {
            let (slug, name) = model?;
            let key = (make_slug.clone(), slug);
            let (has_fluids, has_tires) = (fluids.contains(&key), tires.contains(&key));
            coverage.models += 1;
            coverage.with_fluids += usize::from(has_fluids);
            coverage.with_tires += usize::from(has_tires);
            if !has_fluids && !has_tires {
                coverage.missing.push(name);
            }
        }
        found.push(coverage);
    }
    Ok(found)
}

/// The report as text: one line per make, then up to `list` of its models
/// with no record.
pub fn table(report: &[MakeCoverage], from_year: u16, list: usize) -> String {
    let mut text = format!(
        "Hand-read records by make, in the order of data/catalog/makes.yaml. Light models with a model year from {from_year} on.\n"
    );
    for make in report {
        let _ = writeln!(
            text,
            "{:>3}. {}: {} models, {} with fluids, {} with tires, {} with neither",
            make.rank,
            make.make,
            make.models,
            make.with_fluids,
            make.with_tires,
            make.missing.len()
        );
        if list > 0 && !make.missing.is_empty() {
            let shown: Vec<&str> = make.missing.iter().take(list).map(String::as_str).collect();
            let more = make.missing.len().saturating_sub(list);
            let _ = writeln!(
                text,
                "     none yet: {}{}",
                shown.join(", "),
                if more > 0 {
                    format!(", and {more} more")
                } else {
                    String::new()
                }
            );
        }
    }
    text
}
