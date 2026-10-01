//! Builds the vehicle catalog tables of the data file.

pub mod curated;
pub mod vehicles;

use anyhow::{Context, Result, bail};
use rusqlite::Transaction;

use crate::catalog::curated::Curated;

/// Row counts of the catalog, and what the curated lists named that vPIC
/// does not have.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct CatalogSummary {
    pub makes: u64,
    pub models: u64,
    pub vehicles: u64,
    pub light_vehicles: u64,
    pub details: u64,
    pub submodels: u64,
    pub engines: u64,
    /// Combinations of positions 4 to 8 that were worked through.
    pub cells: u64,
    /// Models left without submodels and engines because they had too many
    /// combinations.
    pub capped: u64,
    /// Curated entries with no counterpart in vPIC, such as `make Scion`.
    pub unmatched: Vec<String>,
}

/// The last model year in the catalog: the year after the build.
pub fn last_year(built_at: &str) -> Result<u16> {
    let year: u16 = built_at
        .get(..4)
        .and_then(|year| year.parse().ok())
        .filter(|year| (wenmar_vehicles::FIRST_YEAR..9999).contains(year))
        .with_context(|| format!("cannot read a year from the build time {built_at:?}"))?;
    Ok(year + 1)
}

/// Fills the catalog tables from the shaped tables and the staging tables.
/// Must run before the staging tables are dropped.
pub fn build(
    transaction: &Transaction<'_>,
    curated: &Curated,
    last_year: u16,
) -> Result<CatalogSummary> {
    if last_year < wenmar_vehicles::FIRST_YEAR {
        bail!("the catalog cannot end in {last_year}");
    }
    let mut summary = CatalogSummary::default();
    vehicles::build(transaction, curated, last_year, &mut summary)?;
    Ok(summary)
}
