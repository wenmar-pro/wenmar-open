//! The data file, read through the `turso` crate.
//!
//! The file is opened read-only and never written. A fixed number of
//! connections is shared by every request. All database work runs on tokio's
//! blocking thread pool, through [`Db::run`], because the catalog library's
//! [`Source`] is a blocking call.
//!
//! Work that costs the engine more than a few lookups, such as a free-text
//! search or a list of years that has to be read from every model year,
//! goes through [`Db::run_slow`], which may use only some of the
//! connections. The rest are always there for a VIN decode.

use std::path::Path;
use std::sync::{Arc, Mutex, PoisonError};

use serde::Serialize;
use tokio::runtime::Handle;
use tokio::sync::{OwnedSemaphorePermit, Semaphore};
use utoipa::ToSchema;
use wenmar_vehicles::schema::SCHEMA_VERSION;
use wenmar_vehicles::{Catalog, Source, SourceError, Value};

/// What the data file says about itself.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, ToSchema)]
pub struct Meta {
    /// The data release, such as `2026.09`.
    #[schema(example = "2026.09")]
    pub data_version: String,
    /// The NHTSA vPIC release the data was built from.
    #[schema(example = "vPICList_lite_2026_09")]
    pub vpic_release: String,
    /// When the data file was built, in UTC.
    #[schema(example = "2026-10-01 04:25:57")]
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

/// One read-only connection to the data file.
///
/// `query` waits for the answer, so a `TursoSource` must only be used on a
/// blocking thread. Nothing outside this module can make one: handlers reach
/// it through [`Db::run`].
#[derive(Clone)]
pub struct TursoSource {
    connection: turso::Connection,
    runtime: Handle,
}

fn to_turso(value: &Value) -> turso::Value {
    match value {
        Value::Null => turso::Value::Null,
        Value::Integer(number) => turso::Value::Integer(*number),
        Value::Real(number) => turso::Value::Real(*number),
        Value::Text(text) => turso::Value::Text(text.clone()),
    }
}

fn from_turso(value: turso::Value) -> Value {
    match value {
        turso::Value::Null => Value::Null,
        turso::Value::Integer(number) => Value::Integer(number),
        turso::Value::Real(number) => Value::Real(number),
        turso::Value::Text(text) => Value::Text(text),
        // The data file has no blob columns.
        turso::Value::Blob(_) => Value::Null,
    }
}

async fn all_rows(
    connection: &turso::Connection,
    sql: &str,
    params: Vec<turso::Value>,
) -> Result<Vec<Vec<Value>>, turso::Error> {
    let mut rows = connection
        .query(sql, turso::params_from_iter(params))
        .await?;
    let columns = rows.column_count();
    let mut found = Vec::new();
    while let Some(row) = rows.next().await? {
        let mut values = Vec::with_capacity(columns);
        for index in 0..columns {
            values.push(from_turso(row.get_value(index)?));
        }
        found.push(values);
    }
    Ok(found)
}

impl Source for TursoSource {
    fn query(&self, sql: &str, params: &[Value]) -> Result<Vec<Vec<Value>>, SourceError> {
        let params = params.iter().map(to_turso).collect();
        Ok(self
            .runtime
            .block_on(all_rows(&self.connection, sql, params))?)
    }
}

/// A connection with the catalog that reads through it.
pub struct Worker {
    /// For queries the catalog library does not make, such as a VIN's rows.
    pub source: TursoSource,
    pub catalog: Catalog<TursoSource>,
}

impl Worker {
    /// Blocks. Reads the makes, aliases, vehicle types and year range.
    fn new(database: &turso::Database, runtime: Handle) -> Result<Worker, DbError> {
        let connection = database
            .connect()
            .map_err(|error| DbError::Read(error.into()))?;
        let source = TursoSource {
            connection,
            runtime,
        };
        let catalog = Catalog::new(source.clone()).map_err(|error| DbError::Read(error.into()))?;
        Ok(Worker { source, catalog })
    }
}

/// The open data file.
pub struct Db {
    database: turso::Database,
    runtime: Handle,
    idle: Arc<Mutex<Vec<Worker>>>,
    permits: Arc<Semaphore>,
    size: usize,
    /// Places for slow work: fewer than there are connections.
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

impl Db {
    /// Opens a data file read-only with `connections` connections.
    ///
    /// Refuses a file that is missing, has no `meta` table, or was built for
    /// another schema version.
    pub async fn open(path: &Path, connections: usize) -> Result<Db, DbError> {
        let shown = path.display().to_string();
        // turso would create an empty database for a path that does not
        // exist; a missing data file must stop the server instead.
        if !path.is_file() {
            return Err(DbError::Missing(shown));
        }
        let Some(text) = path.to_str() else {
            return Err(DbError::Missing(shown));
        };
        let database = turso::Builder::new_local(text)
            .read_only(true)
            .build()
            .await
            .map_err(|error| DbError::Read(error.into()))?;
        let connection = database
            .connect()
            .map_err(|error| DbError::Read(error.into()))?;
        let rows = all_rows(&connection, "SELECT key, value FROM meta", Vec::new())
            .await
            .map_err(|_| {
                DbError::NotADataFile(
                    "not a Wenmar Open data file: it has no meta table".to_owned(),
                )
            })?;
        match meta_value(&rows, "schema_version").as_deref() {
            Some(SCHEMA_VERSION) => {}
            Some(other) => {
                return Err(DbError::NotADataFile(format!(
                    "data file has schema version {other}, this build reads version {SCHEMA_VERSION}; rebuild the data file with open-data build"
                )));
            }
            None => {
                return Err(DbError::NotADataFile(
                    "not a Wenmar Open data file: meta has no schema_version".to_owned(),
                ));
            }
        }
        let Some(data_version) = meta_value(&rows, "data_version") else {
            return Err(DbError::NotADataFile(
                "not a Wenmar Open data file: meta has no data_version".to_owned(),
            ));
        };
        let meta = Meta {
            data_version,
            vpic_release: meta_value(&rows, "vpic_release").unwrap_or_default(),
            built_at: meta_value(&rows, "built_at").unwrap_or_default(),
        };

        let size = connections.max(1);
        let runtime = Handle::current();
        let workers = {
            let database = database.clone();
            let runtime = runtime.clone();
            tokio::task::spawn_blocking(move || {
                (0..size)
                    .map(|_| Worker::new(&database, runtime.clone()))
                    .collect::<Result<Vec<Worker>, DbError>>()
            })
            .await
            .map_err(|_| DbError::Stopped)??
        };
        Ok(Db {
            database,
            runtime,
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
        let database = self.database.clone();
        let runtime = self.runtime.clone();
        let size = self.size;
        tokio::task::spawn_blocking(move || {
            let _permit = permit;
            let _place = place;
            let taken = idle.lock().unwrap_or_else(PoisonError::into_inner).pop();
            // A worker is missing only if an earlier task panicked.
            let worker = match taken {
                Some(worker) => worker,
                None => Worker::new(&database, runtime)?,
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
