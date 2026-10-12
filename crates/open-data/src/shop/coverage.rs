//! How much of each dataset matched the catalog.
//!
//! Unmatched names are how `shop/models.yaml` grows, and a matched share
//! that falls is how a change in a source's naming is noticed.

use std::collections::BTreeMap;
use std::fmt::Write as _;
use std::path::Path;

use anyhow::{Context, Result, bail};
use rusqlite::Connection;
use serde::{Deserialize, Serialize};
use wenmar_shopdata::{Dataset, ShopData};
use wenmar_vehicles::{Source, Value};

use crate::shop::matcher::NO_YEAR;

/// The first model year measured. Older vehicles are rarely in a shop.
pub const FIRST_YEAR: i64 = 2000;

/// How many unmatched names a report lists for a dataset.
pub const MOST_UNMATCHED: usize = 25;

/// The top of the thresholds file. `data/coverage.yaml` starts with it.
const HEADER: &str = "\
# Minimum share of each dataset's vehicle rows that must match a catalog
# model, in percent. Measured over rows whose make is a light make in the
# catalog (cars, MPVs and trucks) and whose model year is 2000 or later.
#
# Check:   open-data coverage --shop <shop file> --catalog <data file>
# Record:  the same command with --write, after reading the report.
# Raise a figure by hand when matching improves; never lower one to make a
# build pass without saying why in the commit.
";

/// A dataset's vehicle rows: a table, or a parenthesised `SELECT`, with the
/// columns `year`, `make_id`, `model_id`, `make_text`, `model_text` and
/// `"match"`.
#[derive(Debug, Clone, Copy)]
pub struct CoverageTable {
    pub dataset: Dataset,
    pub table: &'static str,
}

/// Every dataset with vehicle rows. A plan that adds one adds an entry.
pub const TABLES: &[CoverageTable] = &[
    CoverageTable {
        dataset: Dataset::RecallsUs,
        table: "(SELECT v.* FROM recall_vehicle v JOIN recall r ON r.id = v.recall_id WHERE r.source = 'nhtsa')",
    },
    CoverageTable {
        dataset: Dataset::RecallsCa,
        table: "(SELECT v.* FROM recall_vehicle v JOIN recall r ON r.id = v.recall_id WHERE r.source = 'tc')",
    },
    CoverageTable {
        dataset: Dataset::Investigations,
        table: "investigation_vehicle",
    },
];

/// A name that matched no catalog model, and how many rows carry it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Unmatched {
    pub make_text: String,
    pub model_text: String,
    pub rows: u64,
}

/// How one dataset matched.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
pub struct DatasetCoverage {
    pub dataset: String,
    /// Every row of the table.
    pub rows: u64,
    /// Rows with a light make and a model year from [`FIRST_YEAR`] on.
    pub measured: u64,
    pub exact: u64,
    pub base: u64,
    pub curated: u64,
    pub unmatched: u64,
    /// The commonest unmatched names among the measured rows.
    pub top_unmatched: Vec<Unmatched>,
}

fn tenths(value: u64) -> String {
    format!("{}.{}", value / 10, value % 10)
}

impl DatasetCoverage {
    /// The matched share of the measured rows, in tenths of a percent,
    /// rounded down. Zero when nothing was measured.
    pub fn share_tenths(&self) -> u64 {
        ((self.exact + self.base + self.curated) * 1000)
            .checked_div(self.measured)
            .unwrap_or(0)
    }

    /// The share as text, such as `62.5`.
    pub fn share(&self) -> String {
        tenths(self.share_tenths())
    }
}

/// How every measured dataset matched, in registry order.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
pub struct Report {
    pub datasets: Vec<DatasetCoverage>,
}

/// The committed minimums: dataset id to percent.
#[derive(Debug, Clone, Default, PartialEq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Thresholds {
    pub datasets: BTreeMap<String, f64>,
}

impl Thresholds {
    pub fn load(path: &Path) -> Result<Thresholds> {
        let text =
            std::fs::read_to_string(path).with_context(|| format!("reading {}", path.display()))?;
        Thresholds::parse(&text)
    }

    pub fn parse(yaml: &str) -> Result<Thresholds> {
        let thresholds: Thresholds =
            serde_saphyr::from_str(yaml).context("reading coverage.yaml")?;
        for (id, minimum) in &thresholds.datasets {
            if Dataset::parse(id).is_none() {
                bail!("coverage.yaml: {id} is not a dataset");
            }
            if !(0.0..=100.0).contains(minimum) {
                bail!("coverage.yaml: {id} must be from 0 to 100");
            }
        }
        Ok(thresholds)
    }
}

/// The ids of the catalog's light makes: cars, MPVs and trucks.
pub fn light_makes(catalog: &Connection) -> Result<Vec<i64>> {
    let mut statement =
        catalog.prepare("SELECT id FROM catalog_make WHERE light = 1 ORDER BY id")?;
    let ids = statement
        .query_map([], |row| row.get(0))?
        .collect::<rusqlite::Result<Vec<i64>>>()?;
    Ok(ids)
}

fn count(row: &[Value], column: usize) -> u64 {
    row.get(column)
        .and_then(Value::integer)
        .and_then(|number| u64::try_from(number).ok())
        .unwrap_or(0)
}

/// Measures every table in `tables` whose dataset the shop file holds.
pub fn run<S: Source>(
    shop: &ShopData<S>,
    light_makes: &[i64],
    tables: &[CoverageTable],
) -> Result<Report> {
    // An id no make has, so the list is never empty.
    let mut ids = String::from("-1");
    for id in light_makes {
        write!(ids, ", {id}")?;
    }
    let measured = format!(
        "t.year BETWEEN {FIRST_YEAR} AND {} AND t.make_id IN ({ids})",
        NO_YEAR - 1
    );
    let mut report = Report::default();
    for entry in tables {
        if !shop.datasets().contains(entry.dataset) {
            continue;
        }
        let table = entry.table;
        let reading = || format!("measuring {}", entry.dataset);
        let mut coverage = DatasetCoverage {
            dataset: entry.dataset.id().to_owned(),
            ..DatasetCoverage::default()
        };
        let total = shop
            .query(&format!("SELECT COUNT(*) FROM {table} AS t"), &[])
            .with_context(reading)?;
        coverage.rows = total.first().map_or(0, |row| count(row, 0));
        let kinds = shop
            .query(
                &format!(
                    "SELECT COALESCE(t.\"match\", ''), COUNT(*) FROM {table} AS t
                     WHERE {measured} GROUP BY 1"
                ),
                &[],
            )
            .with_context(reading)?;
        for row in &kinds {
            let rows = count(row, 1);
            match row.first().and_then(Value::text) {
                Some("exact") => coverage.exact = rows,
                Some("base") => coverage.base = rows,
                Some("curated") => coverage.curated = rows,
                Some("") => coverage.unmatched = rows,
                other => bail!(
                    "{} has the match value {:?}; it must be exact, base, curated or NULL",
                    entry.dataset,
                    other.unwrap_or_default()
                ),
            }
            coverage.measured += rows;
        }
        let top = shop
            .query(
                &format!(
                    "SELECT t.make_text, t.model_text, COUNT(*) FROM {table} AS t
                     WHERE {measured} AND t.\"match\" IS NULL
                     GROUP BY 1, 2 ORDER BY 3 DESC, 1, 2 LIMIT {MOST_UNMATCHED}"
                ),
                &[],
            )
            .with_context(reading)?;
        for row in &top {
            coverage.top_unmatched.push(Unmatched {
                make_text: row
                    .first()
                    .and_then(Value::text)
                    .unwrap_or_default()
                    .to_owned(),
                model_text: row
                    .get(1)
                    .and_then(Value::text)
                    .unwrap_or_default()
                    .to_owned(),
                rows: count(row, 2),
            });
        }
        report.datasets.push(coverage);
    }
    Ok(report)
}

/// The report as text.
pub fn table(report: &Report) -> String {
    let mut text = String::new();
    if report.datasets.is_empty() {
        text.push_str("no dataset in this shop file has vehicle rows to measure\n");
    }
    for coverage in &report.datasets {
        let _ = writeln!(
            text,
            "{}: {} rows, {} measured (light makes, model year {FIRST_YEAR} on)",
            coverage.dataset, coverage.rows, coverage.measured
        );
        let _ = writeln!(
            text,
            "  matched {}%: {} exact, {} base, {} curated; {} unmatched",
            coverage.share(),
            coverage.exact,
            coverage.base,
            coverage.curated,
            coverage.unmatched
        );
        if !coverage.top_unmatched.is_empty() {
            text.push_str("  most common unmatched:\n");
        }
        for name in &coverage.top_unmatched {
            let _ = writeln!(
                text,
                "  {:>8}  {} / {}",
                name.rows, name.make_text, name.model_text
            );
        }
    }
    text
}

/// One sentence for each way the report falls short of the minimums.
pub fn failures(report: &Report, thresholds: &Thresholds) -> Vec<String> {
    let mut problems = Vec::new();
    for coverage in &report.datasets {
        match thresholds.datasets.get(&coverage.dataset) {
            None => problems.push(format!(
                "{} has no minimum in coverage.yaml; run open-data coverage with --write and commit the file",
                coverage.dataset
            )),
            Some(minimum) => {
                // The file holds one decimal; compare in tenths.
                let minimum = (minimum * 10.0).round() as u64;
                if coverage.share_tenths() < minimum {
                    problems.push(format!(
                        "{}: {}% of measured rows matched, below the minimum of {}%",
                        coverage.dataset,
                        coverage.share(),
                        tenths(minimum)
                    ));
                }
            }
        }
    }
    for id in thresholds.datasets.keys() {
        if !report
            .datasets
            .iter()
            .any(|coverage| &coverage.dataset == id)
        {
            problems.push(format!(
                "{id} has a minimum in coverage.yaml but is not in this shop file"
            ));
        }
    }
    problems
}

/// The thresholds file that records this report's shares as the minimums.
pub fn yaml(report: &Report) -> String {
    let mut text = String::from(HEADER);
    if report.datasets.is_empty() {
        text.push_str("datasets: {}\n");
        return text;
    }
    text.push_str("datasets:\n");
    for coverage in &report.datasets {
        let _ = writeln!(text, "  {}: {}", coverage.dataset, coverage.share());
    }
    text
}
