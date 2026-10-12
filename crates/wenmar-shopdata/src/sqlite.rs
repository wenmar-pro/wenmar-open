//! A [`Source`] backed by a shop file, through `rusqlite`.

use std::path::Path;

use rusqlite::types::ValueRef;
use rusqlite::{Connection, OpenFlags, params_from_iter};
use wenmar_vehicles::{Source, SourceError, Value};

/// The tables of a shop file. It checks nothing: [`crate::ShopData::new`]
/// checks the schema version, for every engine.
#[derive(Debug)]
pub struct ShopSource {
    connection: Connection,
}

impl ShopSource {
    /// Opens a shop file read-only.
    pub fn open(path: impl AsRef<Path>) -> Result<Self, SourceError> {
        let flags = OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_NO_MUTEX;
        Ok(Self::from_connection(Connection::open_with_flags(
            path, flags,
        )?))
    }

    pub fn from_connection(connection: Connection) -> Self {
        Self { connection }
    }
}

impl Source for ShopSource {
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
                    ValueRef::Blob(_) => return Err("the shop file holds no blobs".into()),
                });
            }
            found.push(values);
        }
        Ok(found)
    }
}
