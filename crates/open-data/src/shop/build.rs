//! Creates a shop file: one shape step per dataset given, then `meta`.

use std::path::Path;

use anyhow::{Context, Result, bail};
use rusqlite::{Connection, OpenFlags, OptionalExtension, Transaction};
use wenmar_shopdata::{Dataset, DatasetEntry, Datasets, licence, schema};

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
pub struct ShopInputs {}

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
    let ShopInputs {} = inputs;
    let _ = context;
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

    let transaction = connection.transaction()?;
    let mut context = ShapeContext {
        transaction: &transaction,
        catalog,
        curated,
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
