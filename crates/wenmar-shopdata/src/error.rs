//! Why a shop file could not be read.

use wenmar_vehicles::SourceError;

use crate::dataset::Dataset;

#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum ShopError {
    /// The database failed.
    #[error("the shop data could not be read: {0}")]
    Source(#[source] SourceError),
    /// The file has no usable `meta` table.
    #[error("not a Wenmar Open shop file: {0}")]
    NotAShopFile(String),
    /// The file was built for another table layout.
    #[error(
        "shop file has schema version {found}, this build reads version {expected}; rebuild it with open-data build-shop"
    )]
    Schema {
        found: String,
        expected: &'static str,
    },
    /// The file's catalog ids belong to another core data file.
    #[error(
        "shop file was matched against data version {shop}, but the data file is version {core}; rebuild it with open-data build-shop --catalog <the data file>"
    )]
    Pairing { shop: String, core: String },
    /// The file does not hold this dataset. The API, MCP and CLI all show
    /// this message.
    #[error("The {0} dataset is not available.")]
    DatasetUnavailable(Dataset),
    /// A row did not have the columns this crate expects.
    #[error("the shop file has an unexpected row in {0}")]
    Shape(&'static str),
}
