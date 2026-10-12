//! Creates a shop file: one shape step per dataset given, then `meta`.

use std::path::{Path, PathBuf};

use anyhow::{Context, Result, bail};
use rusqlite::{Connection, OpenFlags, OptionalExtension, Transaction};
use wenmar_shopdata::{Dataset, DatasetEntry, Datasets, licence, schema};

use crate::shop::matcher::{CuratedModels, Matcher};

/// What gets stamped into the shop file.
#[derive(Debug, Clone)]
pub struct ShopBuildInfo {
    /// The build date, such as `2026.10.12`.
    pub data_version: String,
    /// Build time as `YYYY-MM-DD HH:MM:SS`, UTC.
    pub built_at: String,
}

/// The fetched sources to build from: one optional directory per source.
/// A dataset whose source is not given is left out of the file.
#[derive(Debug, Clone, Default)]
pub struct ShopInputs {
    /// `data/build/sources/nhtsa-recalls`.
    pub nhtsa_recalls: Option<PathBuf>,
    /// `data/build/sources/tc-recalls`.
    pub tc_recalls: Option<PathBuf>,
    /// `data/build/sources/nhtsa-investigations`.
    pub nhtsa_investigations: Option<PathBuf>,
    /// `data/build/sources/nhtsa-recall-reports`.
    pub nhtsa_recall_reports: Option<PathBuf>,
    /// `data/build/sources/nhtsa-ratings`.
    pub nhtsa_ratings: Option<PathBuf>,
    /// `data/build/sources/nhtsa-vin-guides`.
    pub nhtsa_vin_guides: Option<PathBuf>,
    /// `data/build/sources/epa-vehicles`.
    pub epa_vehicles: Option<PathBuf>,
    /// `data/build/sources/epa-guide`. Needs `epa_vehicles`.
    pub epa_guide: Option<PathBuf>,
    /// `data/build/sources/epa-emissions`. Needs `epa_vehicles`.
    pub epa_emissions: Option<PathBuf>,
    /// `data/build/sources/nrcan-fuel`.
    pub nrcan_fuel: Option<PathBuf>,
    /// `data/build/sources/nhtsa-mfr-comms`.
    pub nhtsa_mfr_comms: Option<PathBuf>,
    /// `data/build/sources/nhtsa-complaints`.
    pub nhtsa_complaints: Option<PathBuf>,
}

/// What the finished file holds.
#[derive(Debug, Clone)]
pub struct ShopSummary {
    pub data_version: String,
    pub catalog_data_version: String,
    pub datasets: Datasets,
}

/// What a shape step works with.
pub struct ShapeContext<'a> {
    /// The shop file being built. A step creates its tables and inserts
    /// its rows through this.
    pub transaction: &'a Transaction<'a>,
    /// The core data file the rows are matched against. Read-only.
    pub catalog: &'a Connection,
    /// The directory of curated files.
    pub curated: &'a Path,
    /// Matches a source's make and model text to the catalog.
    pub matcher: &'a Matcher,
    pub info: &'a ShopBuildInfo,
    datasets: Datasets,
}

/// Whether `text` is ten characters of digits with `separator` after the
/// year and the month.
fn dated(text: &str, separator: u8) -> bool {
    let bytes = text.as_bytes();
    bytes.len() == 10
        && bytes.iter().enumerate().all(|(index, byte)| match index {
            4 | 7 => *byte == separator,
            _ => byte.is_ascii_digit(),
        })
}

/// Whether `text` is a shop data version, such as `2026.10.12`.
pub fn is_data_version(text: &str) -> bool {
    dated(text, b'.')
}

impl ShapeContext<'_> {
    /// Records that the file now holds `dataset`. A step calls this once
    /// for each dataset it filled, after inserting the rows.
    pub fn register(&mut self, dataset: Dataset, entry: DatasetEntry) -> Result<()> {
        if !licence::ALL.contains(&entry.licence.as_str()) {
            bail!(
                "{dataset}: {} is not a licence; use one of {}",
                entry.licence,
                licence::ALL.join(", ")
            );
        }
        if !dated(&entry.as_of, b'-') {
            bail!(
                "{dataset}: as_of must be a date like 2026-10-11, not {:?}",
                entry.as_of
            );
        }
        if self.datasets.insert(dataset, entry).is_some() {
            bail!("{dataset} was registered twice");
        }
        Ok(())
    }

    /// The datasets registered so far.
    pub fn datasets(&self) -> &Datasets {
        &self.datasets
    }
}

/// Opens a core data file read-only and reads its `data_version`.
pub fn open_catalog(path: &Path) -> Result<(Connection, String)> {
    // rusqlite would report a missing file less clearly.
    if !path.is_file() {
        bail!("the data file {} does not exist", path.display());
    }
    let connection = Connection::open_with_flags(
        path,
        OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_NO_MUTEX,
    )
    .with_context(|| format!("opening {}", path.display()))?;
    let value = |key: &str| -> Result<Option<String>> {
        connection
            .query_row("SELECT value FROM meta WHERE key = ?1", [key], |row| {
                row.get(0)
            })
            .optional()
            .with_context(|| format!("{} is not a Wenmar Open data file", path.display()))
    };
    let schema_version = value("schema_version")?;
    if schema_version.as_deref() != Some(wenmar_vehicles::schema::SCHEMA_VERSION) {
        bail!(
            "{} has schema version {}, this build reads version {}",
            path.display(),
            schema_version.as_deref().unwrap_or("none"),
            wenmar_vehicles::schema::SCHEMA_VERSION
        );
    }
    let Some(data_version) = value("data_version")? else {
        bail!("{} has no data_version in meta", path.display());
    };
    Ok((connection, data_version))
}

/// Runs the shape step of every source that was given, in this order.
///
/// A plan that adds a dataset names its new field in the pattern below and
/// adds one `if let Some(directory) = ... { module::shape(context, directory)?; }`.
fn shape_all(context: &mut ShapeContext<'_>, inputs: &ShopInputs) -> Result<()> {
    let ShopInputs {
        nhtsa_recalls,
        tc_recalls,
        nhtsa_investigations,
        nhtsa_recall_reports,
        nhtsa_ratings,
        nhtsa_vin_guides,
        epa_vehicles,
        epa_guide,
        epa_emissions,
        nrcan_fuel,
        nhtsa_mfr_comms,
        nhtsa_complaints,
    } = inputs;
    if let Some(directory) = nhtsa_recalls {
        crate::shop::nhtsa_recalls::shape(context, directory)?;
    }
    if let Some(directory) = tc_recalls {
        crate::shop::tc_recalls::shape(context, directory)?;
    }
    if let Some(directory) = nhtsa_investigations {
        crate::shop::nhtsa_investigations::shape(context, directory)?;
    }
    if let Some(directory) = nhtsa_recall_reports {
        crate::shop::nhtsa_recall_reports::shape(context, directory)?;
    }
    if let Some(directory) = nhtsa_ratings {
        crate::shop::nhtsa_ratings::shape(context, directory)?;
    }
    if let Some(directory) = nhtsa_vin_guides {
        crate::shop::nhtsa_vin_guides::shape(context, directory)?;
    }
    match epa_vehicles {
        Some(directory) => crate::shop::fuel::shape_epa(
            context,
            directory,
            epa_guide.as_deref(),
            epa_emissions.as_deref(),
        )?,
        None if epa_guide.is_some() || epa_emissions.is_some() => {
            bail!("--epa-guide and --epa-emissions need --epa-vehicles")
        }
        None => {}
    }
    if let Some(directory) = nrcan_fuel {
        crate::shop::fuel::shape_nrcan(context, directory)?;
    }
    if let Some(directory) = nhtsa_mfr_comms {
        crate::shop::mfr_comms::shape(context, directory)?;
    }
    if let Some(directory) = nhtsa_complaints {
        crate::shop::complaints::shape(context, directory)?;
    }
    // The hand-kept records are in the curated directory, not fetched.
    crate::shop::hand::shape(context)?;
    // The hand-written trouble codes are in the curated directory, not
    // fetched.
    crate::shop::codes::shape(context)?;
    // So is the job list.
    crate::shop::jobs::shape(context)?;
    after_sources(context)
}

/// The steps that read what the sources stored: which components each
/// recall and bulletin is about, and what each bulletin's summary names.
/// They run only for the datasets the file holds.
pub fn after_sources(context: &mut ShapeContext<'_>) -> Result<()> {
    let datasets = context.datasets();
    let recalls = datasets.contains(Dataset::RecallsUs) || datasets.contains(Dataset::RecallsCa);
    let bulletins = datasets.contains(Dataset::Bulletins);
    let complaints = datasets.contains(Dataset::Complaints);
    // A code names its components, so the file must be able to name them.
    let codes = datasets.contains(Dataset::Codes);
    // A job names its components and its system too.
    let jobs = datasets.contains(Dataset::Jobs);
    if recalls || bulletins || complaints || codes || jobs {
        crate::shop::components::shape(context)?;
    }
    if bulletins {
        crate::shop::bulletin_text::shape(context)?;
    }
    if recalls || bulletins {
        crate::shop::siblings::shape(context)?;
    }
    Ok(())
}

/// Builds a shop file at `output`, matched against the core file `catalog`.
pub fn build_shop(
    catalog: &Path,
    output: &Path,
    curated: &Path,
    info: &ShopBuildInfo,
    inputs: &ShopInputs,
) -> Result<ShopSummary> {
    build_with(catalog, output, curated, info, |context| {
        shape_all(context, inputs)
    })
}

/// Builds a shop file whose tables are whatever `shape` creates. This is
/// how one shape step is tested alone.
pub fn build_with(
    catalog: &Path,
    output: &Path,
    curated: &Path,
    info: &ShopBuildInfo,
    shape: impl FnOnce(&mut ShapeContext<'_>) -> Result<()>,
) -> Result<ShopSummary> {
    if !is_data_version(&info.data_version) {
        bail!(
            "{} is not a shop data version like 2026.10.12",
            info.data_version
        );
    }
    if output.exists() {
        bail!(
            "{} already exists; remove it or choose another path",
            output.display()
        );
    }
    let (catalog, catalog_data_version) = open_catalog(catalog)?;
    let result = build_into(
        &catalog,
        &catalog_data_version,
        output,
        curated,
        info,
        shape,
    );
    if result.is_err() {
        // Never leave a half-built file that could be mistaken for a good one.
        let _ = std::fs::remove_file(output);
    }
    result
}

fn build_into(
    catalog: &Connection,
    catalog_data_version: &str,
    output: &Path,
    curated: &Path,
    info: &ShopBuildInfo,
    shape: impl FnOnce(&mut ShapeContext<'_>) -> Result<()>,
) -> Result<ShopSummary> {
    let mut connection =
        Connection::open(output).with_context(|| format!("creating {}", output.display()))?;
    connection.execute_batch("PRAGMA journal_mode = OFF; PRAGMA synchronous = OFF;")?;
    connection.execute_batch(schema::META)?;
    let matcher = Matcher::load(catalog, &CuratedModels::load(curated)?)?;

    let transaction = connection.transaction()?;
    let mut context = ShapeContext {
        transaction: &transaction,
        catalog,
        curated,
        matcher: &matcher,
        info,
        datasets: Datasets::default(),
    };
    shape(&mut context)?;
    let datasets = context.datasets;
    let datasets_json = datasets.to_json();
    for (key, value) in [
        ("schema_version", schema::SCHEMA_VERSION),
        ("data_version", info.data_version.as_str()),
        ("catalog_data_version", catalog_data_version),
        ("built_at", info.built_at.as_str()),
        ("datasets", datasets_json.as_str()),
    ] {
        transaction.execute(
            "INSERT INTO meta (key, value) VALUES (?1, ?2)",
            [key, value],
        )?;
    }
    transaction.commit()?;
    connection.execute_batch("VACUUM;")?;

    Ok(ShopSummary {
        data_version: info.data_version.clone(),
        catalog_data_version: catalog_data_version.to_owned(),
        datasets,
    })
}

/// Writes a copy of the shop file `full` at `output` without the datasets
/// only the hosted service carries: their tables are dropped and their
/// entries leave `datasets`. Everything else is the same, byte for byte in
/// meaning: the same `data_version`, `catalog_data_version` and `built_at`.
/// Returns the datasets the copy holds.
pub fn offline_copy(full: &Path, output: &Path) -> Result<Datasets> {
    if output.exists() {
        bail!(
            "{} already exists; remove it or choose another path",
            output.display()
        );
    }
    let result = offline_copy_into(full, output);
    if result.is_err() {
        let _ = std::fs::remove_file(output);
    }
    result
}

fn offline_copy_into(full: &Path, output: &Path) -> Result<Datasets> {
    let source = Connection::open_with_flags(
        full,
        OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_NO_MUTEX,
    )
    .with_context(|| format!("opening {}", full.display()))?;
    let target = output
        .to_str()
        .with_context(|| format!("{} is not a UTF-8 path", output.display()))?;
    source
        .execute("VACUUM INTO ?1", [target])
        .with_context(|| format!("copying {} to {}", full.display(), output.display()))?;
    drop(source);

    let connection =
        Connection::open(output).with_context(|| format!("opening {}", output.display()))?;
    connection.execute_batch("PRAGMA journal_mode = OFF; PRAGMA synchronous = OFF;")?;
    let json: String = connection
        .query_row("SELECT value FROM meta WHERE key = 'datasets'", [], |row| {
            row.get(0)
        })
        .with_context(|| format!("{} is not a shop file", full.display()))?;
    let held = Datasets::parse(&json)?;
    let mut kept = Datasets::default();
    for (dataset, entry) in held.iter() {
        if !dataset.hosted_only() {
            kept.insert(dataset, entry.clone());
        }
    }
    for (id, tables) in schema::HOSTED_ONLY_TABLES {
        if Dataset::parse(id).is_none_or(|dataset| !dataset.hosted_only()) {
            bail!("{id} is listed in HOSTED_ONLY_TABLES but is not a hosted-only dataset");
        }
        for table in *tables {
            connection.execute_batch(&format!("DROP TABLE IF EXISTS \"{table}\";"))?;
        }
    }
    connection.execute(
        "UPDATE meta SET value = ?1 WHERE key = 'datasets'",
        [kept.to_json()],
    )?;
    connection.execute_batch("VACUUM;")?;
    Ok(kept)
}
