//! The data file, read through the `rusqlite` crate.
//!
//! The file is opened read-only and never written. A fixed number of
//! connections is shared by every request. All database work runs on tokio's
//! blocking thread pool, through [`Db::run`], because the catalog library's
//! [`Source`](wenmar_vehicles::Source) is a blocking call.
//!
//! A `rusqlite::Connection` is `Send` but not `Sync`: it may move between
//! threads but may not be shared between tasks. The pool is how a fixed
//! number of connections serves any number of callers, and it is why this
//! crate exists rather than a bare `Connection`.
//!
//! Work that costs the engine more than a few lookups, such as a free-text
//! search or a list of years that has to be read from every model year,
//! goes through [`Db::run_slow`], which may use only some of the
//! connections. The rest are always there for a VIN decode.

use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, PoisonError};

use rusqlite::{Connection, OpenFlags};
use serde::Serialize;
use tokio::sync::{OwnedSemaphorePermit, Semaphore};
use wenmar_vehicles::{Catalog, Source, SourceError, Value};

/// The catalog's `Source`, re-exported so the type of [`Worker::source`]
/// and `Catalog<SqliteSource>` can be named from this crate.
pub use wenmar_vehicles::sqlite::SqliteSource;

/// What the data file says about itself.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Meta {
    /// The data release, such as `2026.09`.
    pub data_version: String,
    /// The NHTSA vPIC release the data was built from.
    pub vpic_release: String,
    /// When the data file was built, in UTC.
    pub built_at: String,
}

/// Why the data file could not be used.
#[derive(Debug, thiserror::Error)]
pub enum DbError {
    #[error("the data file {0} does not exist")]
    Missing(String),
    #[error("{0}")]
    NotADataFile(String),
    #[error("the data file could not be read: {0}")]
    Read(#[source] SourceError),
    #[error("a database task stopped before it finished")]
    Stopped,
}

/// Opens a connection to the data file that can only read it. Blocking.
///
/// `NO_MUTEX` because SQLite's own mutexes protect a connection shared by
/// nothing, and this crate's pool gives each query a connection of its own.
fn read_only(path: &Path) -> Result<Connection, rusqlite::Error> {
    Connection::open_with_flags(
        path,
        OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_NO_MUTEX,
    )
}

/// A connection with the catalog that reads through it.
pub struct Worker {
    /// For queries the catalog library does not make, such as a VIN's rows.
    pub source: SqliteSource,
    pub catalog: Catalog<SqliteSource>,
}

impl Worker {
    /// Blocks. Opens the data file read-only, asks SQLite to map it, and
    /// reads the makes, aliases, vehicle types and year range.
    ///
    /// A worker is two read-only connections: the catalog holds one of its
    /// own. `rusqlite::Connection` may not be shared between threads, so
    /// there is no way to hand the same one to both.
    fn new(path: &Path) -> Result<Worker, DbError> {
        let source = open_source(path)?;
        let catalog =
            Catalog::new(open_source(path)?).map_err(|error| DbError::Read(error.into()))?;
        Ok(Worker { source, catalog })
    }
}

/// Opens one read-only connection to the data file and asks SQLite to map
/// it. Blocking.
///
/// The data file is read-only and read repeatedly, so map it rather than
/// copying pages through the buffer cache. The value is the file's length;
/// SQLite maps on demand and the mapping is shared by every connection in
/// the pool. A filesystem that cannot map it ignores the pragma.
fn open_source(path: &Path) -> Result<SqliteSource, DbError> {
    let connection = read_only(path).map_err(|error| DbError::Read(error.into()))?;
    let length = std::fs::metadata(path)
        .map_err(|error| DbError::Read(error.into()))?
        .len();
    connection
        .pragma_update(None, "mmap_size", i64::try_from(length).unwrap_or(i64::MAX))
        .map_err(|error| DbError::Read(error.into()))?;
    SqliteSource::from_connection(connection).map_err(DbError::Read)
}

/// The open data file.
pub struct Db {
    /// Opened afresh when a worker is missing, which only happens after a
    /// task panicked. Keeping the path is enough: every worker opens its
    /// own read-only connections.
    path: PathBuf,
    idle: Arc<Mutex<Vec<Worker>>>,
    permits: Arc<Semaphore>,
    size: usize,
    /// Places for slow work: fewer than there are slots.
    slow: Arc<Semaphore>,
    slow_size: usize,
    meta: Meta,
}

/// How many of `size` connections slow work may use at once: half, so the
/// other half is never waited for. A single connection is shared, and then
/// quick work waits for at most one piece of slow work.
fn slow_share(size: usize) -> usize {
    (size / 2).max(1)
}

impl std::fmt::Debug for Db {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("Db")
            .field("meta", &self.meta)
            .field("size", &self.size)
            .field("slow_size", &self.slow_size)
            .finish()
    }
}

fn meta_value(rows: &[Vec<Value>], key: &str) -> Option<String> {
    rows.iter().find_map(|row| match (row.first(), row.get(1)) {
        (Some(Value::Text(found)), Some(Value::Text(value))) if found == key => Some(value.clone()),
        _ => None,
    })
}

/// Reads the `meta` table: what the data file says about itself. Blocks.
///
/// The schema-version check is `SqliteSource::from_connection`'s — every
/// connection passes through it — so the check and its message are written
/// once, in `wenmar-vehicles`. The one thing it does not look for is
/// `data_version`, which only this server cares about.
fn read_meta(path: &Path) -> Result<Meta, DbError> {
    let connection = read_only(path).map_err(|error| DbError::Read(error.into()))?;
    let source = SqliteSource::from_connection(connection)
        .map_err(|error| DbError::NotADataFile(error.to_string()))?;
    let rows = source
        .query("SELECT key, value FROM meta", &[])
        .map_err(|_| {
            DbError::NotADataFile(
                "not a Wenmar Open data file: its meta table could not be read".to_owned(),
            )
        })?;
    let Some(data_version) = meta_value(&rows, "data_version") else {
        return Err(DbError::NotADataFile(
            "not a Wenmar Open data file: meta has no data_version".to_owned(),
        ));
    };
    Ok(Meta {
        data_version,
        vpic_release: meta_value(&rows, "vpic_release").unwrap_or_default(),
        built_at: meta_value(&rows, "built_at").unwrap_or_default(),
    })
}

impl Db {
    /// Opens a data file read-only with `connections` pool slots.
    ///
    /// The argument counts slots, not connections: each slot holds two
    /// read-only connections, one for the catalog and one for
    /// [`Worker::source`], and one more is opened while the file's `meta`
    /// table is read. All of them map the same file pages, one shared OS
    /// mapping; where the platform cannot map the file, each connection
    /// falls back to its own SQLite page cache (about 2 MiB by default).
    ///
    /// Refuses a file that is missing, has no `meta` table, or was built for
    /// another schema version.
    pub async fn open(path: &Path, connections: usize) -> Result<Db, DbError> {
        // rusqlite would create an empty database for a path that does not
        // exist; a missing data file must stop the server instead.
        if !path.is_file() {
            return Err(DbError::Missing(path.display().to_string()));
        }
        let path = path.to_path_buf();
        let size = connections.max(1);
        let (meta, workers) = {
            let path = path.clone();
            tokio::task::spawn_blocking(move || {
                let meta = read_meta(&path)?;
                let workers = (0..size)
                    .map(|_| Worker::new(&path))
                    .collect::<Result<Vec<Worker>, DbError>>()?;
                Ok::<_, DbError>((meta, workers))
            })
            .await
            .map_err(|_| DbError::Stopped)??
        };
        Ok(Db {
            path,
            idle: Arc::new(Mutex::new(workers)),
            permits: Arc::new(Semaphore::new(size)),
            size,
            slow: Arc::new(Semaphore::new(slow_share(size))),
            slow_size: slow_share(size),
            meta,
        })
    }

    pub fn meta(&self) -> &Meta {
        &self.meta
    }

    /// How many connections slow work may use at once.
    pub fn slow_connections(&self) -> usize {
        self.slow_size
    }

    /// Runs `work` on a blocking thread with one of the connections.
    ///
    /// Waits, without holding a thread, while every connection is busy. The
    /// connection goes back to the pool when `work` returns, even if the
    /// caller has stopped waiting.
    pub async fn run<T, F>(&self, work: F) -> Result<T, DbError>
    where
        F: FnOnce(&Worker) -> T + Send + 'static,
        T: Send + 'static,
    {
        self.start(None, work).await
    }

    /// The same as [`Db::run`], for work that can take the engine a long
    /// time. Only [`Db::slow_connections`] pieces of it run at once, so it
    /// can never keep every connection busy.
    ///
    /// The place is given back when `work` returns, not when the caller
    /// stops waiting: a request that timed out still has its query running.
    pub async fn run_slow<T, F>(&self, work: F) -> Result<T, DbError>
    where
        F: FnOnce(&Worker) -> T + Send + 'static,
        T: Send + 'static,
    {
        let place = Arc::clone(&self.slow)
            .acquire_owned()
            .await
            .map_err(|_| DbError::Stopped)?;
        self.start(Some(place), work).await
    }

    async fn start<T, F>(&self, place: Option<OwnedSemaphorePermit>, work: F) -> Result<T, DbError>
    where
        F: FnOnce(&Worker) -> T + Send + 'static,
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
            let _place = place;
            let taken = idle.lock().unwrap_or_else(PoisonError::into_inner).pop();
            // A worker is missing only if an earlier task panicked.
            let worker = match taken {
                Some(worker) => worker,
                None => Worker::new(&path)?,
            };
            let output = work(&worker);
            let mut idle = idle.lock().unwrap_or_else(PoisonError::into_inner);
            if idle.len() < size {
                idle.push(worker);
            }
            Ok(output)
        })
        .await
        .map_err(|_| DbError::Stopped)?
    }
}
