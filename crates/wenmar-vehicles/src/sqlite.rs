//! A [`Source`] backed by a Wenmar Open data file, through `rusqlite`.

use std::path::Path;

use rusqlite::types::ValueRef;
use rusqlite::{Connection, OpenFlags, OptionalExtension, params_from_iter};

use crate::schema::SCHEMA_VERSION;
use crate::source::{Source, SourceError, Value};

/// The catalog tables of a data file.
#[derive(Debug)]
pub struct SqliteSource {
    connection: Connection,
}

impl SqliteSource {
    /// Opens a data file read-only.
    pub fn open(path: impl AsRef<Path>) -> Result<Self, SourceError> {
        let flags = OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_NO_MUTEX;
        Self::from_connection(Connection::open_with_flags(path, flags)?)
    }

    /// Wraps an open connection after checking it holds a data file this
    /// version of the crate understands.
    pub fn from_connection(connection: Connection) -> Result<Self, SourceError> {
        let version: Option<String> = connection
            .query_row(
                "SELECT value FROM meta WHERE key = 'schema_version'",
                [],
                |row| row.get(0),
            )
            .optional()
            .map_err(|_| "not a Wenmar Open data file: it has no meta table")?;
        match version.as_deref() {
            Some(SCHEMA_VERSION) => Ok(Self { connection }),
            Some(other) => Err(format!(
                "data file has schema version {other}, this build reads version {SCHEMA_VERSION}; rebuild the data file with open-data build"
            )
            .into()),
            None => Err("not a Wenmar Open data file: meta has no schema_version".into()),
        }
    }
}

impl Source for SqliteSource {
    fn query(&self, sql: &str, params: &[Value]) -> Result<Vec<Vec<Value>>, SourceError> {
        let mut statement = self.connection.prepare_cached(sql)?;
        let columns = statement.column_count();
        let bound = params.iter().map(|value| match value {
            Value::Null => rusqlite::types::Value::Null,
            Value::Integer(number) => rusqlite::types::Value::Integer(*number),
            Value::Real(number) => rusqlite::types::Value::Real(*number),
            Value::Text(text) => rusqlite::types::Value::Text(text.clone()),
        });
        let mut rows = statement.query(params_from_iter(bound))?;
        let mut found = Vec::new();
        while let Some(row) = rows.next()? {
            let mut values = Vec::with_capacity(columns);
            for index in 0..columns {
                values.push(match row.get_ref(index)? {
                    ValueRef::Null => Value::Null,
                    ValueRef::Integer(number) => Value::Integer(number),
                    ValueRef::Real(number) => Value::Real(number),
                    ValueRef::Text(text) => Value::Text(String::from_utf8_lossy(text).into_owned()),
                    ValueRef::Blob(_) => return Err("the catalog holds no blobs".into()),
                });
            }
            found.push(values);
        }
        Ok(found)
    }
}
