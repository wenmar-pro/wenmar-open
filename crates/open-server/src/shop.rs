//! The shop file: the datasets beside the catalog, read through `rusqlite`.
//!
//! It is a file of its own with a pool of its own. The data file's pool is
//! `wenmar-open-db`'s, and every connection in it is checked to be a data
//! file; a shop file is not one. Shop readers are given catalog ids as
//! numbers, so nothing here joins across the two files.

use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, PoisonError};

use tokio::sync::Semaphore;
use wenmar_shopdata::sqlite::ShopSource;
use wenmar_shopdata::{Datasets, ShopData, ShopError, ShopMeta};

use crate::db::DbError;

/// What a handler reads a dataset through.
pub type ShopReader = ShopData<ShopSource>;

/// Why the shop file could not be opened at startup.
#[derive(Debug, thiserror::Error)]
pub enum ShopOpenError {
    #[error("the shop file {0} does not exist")]
    Missing(String),
    #[error("the shop file {path} cannot be used: {reason}")]
    Unusable { path: String, reason: ShopError },
    #[error("a database task stopped before it finished")]
    Stopped,
}

/// Opens one read-only reader. Blocking.
fn reader(path: &Path) -> Result<ShopReader, ShopError> {
    ShopData::new(ShopSource::open(path).map_err(ShopError::Source)?)
}

/// The open shop file.
pub struct ShopDb {
    /// Opened afresh when a reader is missing, which only happens after a
    /// task panicked.
    path: PathBuf,
    idle: Arc<Mutex<Vec<ShopReader>>>,
    permits: Arc<Semaphore>,
    size: usize,
    meta: ShopMeta,
    datasets: Datasets,
}

impl ShopDb {
    /// Opens a shop file read-only with `connections` readers.
    ///
    /// Refuses a file that is missing, is not a shop file, has another
    /// schema version, or was matched against a data file other than the
    /// one whose `data_version` is `core_data_version`.
    pub async fn open(
        path: &Path,
        connections: usize,
        core_data_version: &str,
    ) -> Result<ShopDb, ShopOpenError> {
        // rusqlite's own message for a missing file names no file.
        if !path.is_file() {
            return Err(ShopOpenError::Missing(path.display().to_string()));
        }
        let path = path.to_path_buf();
        let size = connections.max(1);
        let readers = {
            let file = path.clone();
            let core = core_data_version.to_owned();
            tokio::task::spawn_blocking(move || {
                let mut readers = Vec::with_capacity(size);
                for _ in 0..size {
                    let reader = reader(&file)?;
                    reader.check_catalog(&core)?;
                    readers.push(reader);
                }
                Ok::<_, ShopError>(readers)
            })
            .await
            .map_err(|_| ShopOpenError::Stopped)?
            .map_err(|reason| ShopOpenError::Unusable {
                path: path.display().to_string(),
                reason,
            })?
        };
        let Some(first) = readers.first() else {
            return Err(ShopOpenError::Stopped);
        };
        Ok(ShopDb {
            meta: first.meta().clone(),
            datasets: first.datasets().clone(),
            path,
            idle: Arc::new(Mutex::new(readers)),
            permits: Arc::new(Semaphore::new(size)),
            size,
        })
    }

    pub fn meta(&self) -> &ShopMeta {
        &self.meta
    }

    /// The datasets the file holds.
    pub fn datasets(&self) -> &Datasets {
        &self.datasets
    }

    /// Runs `work` on a blocking thread with one of the readers.
    ///
    /// Waits, without holding a thread, while every reader is busy. The
    /// reader goes back to the pool when `work` returns, even if the caller
    /// has stopped waiting.
    pub async fn run<T, F>(&self, work: F) -> Result<T, DbError>
    where
        F: FnOnce(&ShopReader) -> T + Send + 'static,
        T: Send + 'static,
    {
        let permit = Arc::clone(&self.permits)
            .acquire_owned()
            .await
            .map_err(|_| DbError::Stopped)?;
        let idle = Arc::clone(&self.idle);
        let path = self.path.clone();
        let size = self.size;
        tokio::task::spawn_blocking(move || {
            let _permit = permit;
            let taken = idle.lock().unwrap_or_else(PoisonError::into_inner).pop();
            // A reader is missing only if an earlier task panicked.
            let shop = match taken {
                Some(shop) => shop,
                None => reader(&path).map_err(|error| DbError::Read(error.into()))?,
            };
            let output = work(&shop);
            let mut idle = idle.lock().unwrap_or_else(PoisonError::into_inner);
            if idle.len() < size {
                idle.push(shop);
            }
            Ok(output)
        })
        .await
        .map_err(|_| DbError::Stopped)?
    }
}
