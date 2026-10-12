//! The reader of one shop file.

use serde::Serialize;
use wenmar_vehicles::{Source, Value};

use crate::dataset::{Dataset, Datasets};
use crate::error::ShopError;
use crate::schema::SCHEMA_VERSION;

/// What a shop file says about itself.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ShopMeta {
    pub schema_version: String,
    /// The build date, such as `2026.10.12`.
    pub data_version: String,
    /// The `data_version` of the core data file the build matched against.
    pub catalog_data_version: String,
    /// When the file was built, in UTC. Empty when the file does not say.
    pub built_at: String,
}

/// Whether a shop file's catalog ids belong to the core data file in use.
pub fn check_pairing(catalog_data_version: &str, core_data_version: &str) -> Result<(), ShopError> {
    if catalog_data_version == core_data_version {
        Ok(())
    } else {
        Err(ShopError::Pairing {
            shop: catalog_data_version.to_owned(),
            core: core_data_version.to_owned(),
        })
    }
}

/// The datasets of one shop file.
#[derive(Debug)]
pub struct ShopData<S> {
    source: S,
    meta: ShopMeta,
    datasets: Datasets,
}

impl<S: Source> ShopData<S> {
    /// Reads `meta` once. Refuses a file with no `meta` table, a missing
    /// key, or another schema version.
    pub fn new(source: S) -> Result<Self, ShopError> {
        let rows = source
            .query("SELECT key, value FROM meta", &[])
            .map_err(|_| ShopError::NotAShopFile("it has no meta table".to_owned()))?;
        let value = |key: &str| {
            rows.iter().find_map(|row| match (row.first(), row.get(1)) {
                (Some(Value::Text(found)), Some(Value::Text(value))) if found == key => {
                    Some(value.clone())
                }
                _ => None,
            })
        };
        let required = |key: &str| {
            value(key).ok_or_else(|| ShopError::NotAShopFile(format!("meta has no {key}")))
        };
        let schema_version = required("schema_version")?;
        if schema_version != SCHEMA_VERSION {
            return Err(ShopError::Schema {
                found: schema_version,
                expected: SCHEMA_VERSION,
            });
        }
        let meta = ShopMeta {
            schema_version,
            data_version: required("data_version")?,
            catalog_data_version: required("catalog_data_version")?,
            built_at: value("built_at").unwrap_or_default(),
        };
        let datasets = Datasets::parse(&required("datasets")?)?;
        Ok(ShopData {
            source,
            meta,
            datasets,
        })
    }

    pub fn meta(&self) -> &ShopMeta {
        &self.meta
    }

    pub fn data_version(&self) -> &str {
        &self.meta.data_version
    }

    pub fn catalog_data_version(&self) -> &str {
        &self.meta.catalog_data_version
    }

    pub fn built_at(&self) -> &str {
        &self.meta.built_at
    }

    pub fn datasets(&self) -> &Datasets {
        &self.datasets
    }

    /// Every reader method calls this first, so a dataset the file does not
    /// hold is one clear error and never a SQL failure.
    pub fn require(&self, dataset: Dataset) -> Result<(), ShopError> {
        if self.datasets.contains(dataset) {
            Ok(())
        } else {
            Err(ShopError::DatasetUnavailable(dataset))
        }
    }

    /// Refuses a core data file other than the one this file was matched
    /// against. `core_data_version` is that file's `data_version`.
    pub fn check_catalog(&self, core_data_version: &str) -> Result<(), ShopError> {
        check_pairing(&self.meta.catalog_data_version, core_data_version)
    }

    pub fn source(&self) -> &S {
        &self.source
    }

    /// Runs one `SELECT` on the shop file.
    pub fn query(&self, sql: &str, params: &[Value]) -> Result<Vec<Vec<Value>>, ShopError> {
        self.source.query(sql, params).map_err(ShopError::Source)
    }
}
