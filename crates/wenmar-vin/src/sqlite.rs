//! A [`VinData`] source backed by a Wenmar Open SQLite data file.

use std::path::Path;

use rusqlite::{Connection, OpenFlags, OptionalExtension, params_from_iter};

use crate::data::{DataError, Element, Manufacturer, Pattern, SchemaRef, VinData};

/// Version of the table layout below. A data file records the version it was
/// built with, and a mismatch is refused.
pub const SCHEMA_VERSION: &str = "1";

/// The data file's tables. The data build creates the file with this.
pub const SCHEMA: &str = "
CREATE TABLE meta (
    key   TEXT PRIMARY KEY,
    value TEXT NOT NULL
);

CREATE TABLE wmi (
    code          TEXT PRIMARY KEY,
    manufacturer  TEXT NOT NULL,
    make          TEXT,
    country       TEXT,
    vehicle_type  TEXT,
    light_vehicle INTEGER NOT NULL
);

CREATE TABLE wmi_schema (
    wmi       TEXT NOT NULL,
    schema_id INTEGER NOT NULL,
    year_from INTEGER NOT NULL,
    year_to   INTEGER
);
CREATE INDEX wmi_schema_wmi ON wmi_schema (wmi);

CREATE TABLE pattern (
    id         INTEGER PRIMARY KEY,
    schema_id  INTEGER NOT NULL,
    keys       TEXT NOT NULL,
    element_id INTEGER NOT NULL,
    value      TEXT NOT NULL,
    changed_on TEXT NOT NULL,
    make       TEXT
);
CREATE INDEX pattern_schema ON pattern (schema_id);
";

/// vPIC's id for the Model element, whose rows also carry the make.
const MODEL_ELEMENT_ID: i64 = 28;

/// Decoding data read from a Wenmar Open data file.
#[derive(Debug)]
pub struct SqliteData {
    connection: Connection,
}

impl SqliteData {
    /// Opens a data file read-only.
    pub fn open(path: impl AsRef<Path>) -> Result<Self, DataError> {
        let flags = OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_NO_MUTEX;
        Self::from_connection(Connection::open_with_flags(path, flags)?)
    }

    /// Wraps an open connection after checking it holds a data file this
    /// version of the crate understands.
    pub fn from_connection(connection: Connection) -> Result<Self, DataError> {
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
                "data file has schema version {other}, this build reads version {SCHEMA_VERSION}"
            )
            .into()),
            None => Err("not a Wenmar Open data file: meta has no schema_version".into()),
        }
    }

    /// A value from the `meta` table, such as `data_version`.
    pub fn meta(&self, key: &str) -> Result<Option<String>, DataError> {
        Ok(self
            .connection
            .query_row("SELECT value FROM meta WHERE key = ?1", [key], |row| {
                row.get(0)
            })
            .optional()?)
    }
}

impl VinData for SqliteData {
    fn manufacturer(&self, wmi: &str) -> Result<Option<Manufacturer>, DataError> {
        Ok(self
            .connection
            .query_row(
                "SELECT code, manufacturer, make, country, vehicle_type, light_vehicle
                 FROM wmi WHERE code = ?1",
                [wmi],
                |row| {
                    Ok(Manufacturer {
                        wmi: row.get(0)?,
                        name: row.get(1)?,
                        make: row.get(2)?,
                        country: row.get(3)?,
                        vehicle_type: row.get(4)?,
                        light_vehicle: row.get::<_, i64>(5)? != 0,
                    })
                },
            )
            .optional()?)
    }

    fn schemas(&self, wmi: &str, year: u16) -> Result<Vec<SchemaRef>, DataError> {
        let mut statement = self.connection.prepare_cached(
            "SELECT schema_id, year_from FROM wmi_schema
             WHERE wmi = ?1 AND year_from <= ?2 AND (year_to IS NULL OR year_to >= ?2)",
        )?;
        let rows = statement.query_map((wmi, year), |row| {
            Ok(SchemaRef {
                id: row.get(0)?,
                year_from: row.get(1)?,
            })
        })?;
        Ok(rows.collect::<Result<_, _>>()?)
    }

    fn patterns(&self, schema_ids: &[i64], match_key: &str) -> Result<Vec<Pattern>, DataError> {
        if schema_ids.is_empty() {
            return Ok(Vec::new());
        }
        // GLOB uses `?` for one character and understands `[...]` sets, so it
        // narrows the rows to those that can match. The decoder checks again.
        let placeholders = vec!["?"; schema_ids.len()].join(", ");
        let sql = format!(
            "SELECT id, schema_id, keys, element_id, value, changed_on, make
             FROM pattern
             WHERE schema_id IN ({placeholders})
               AND ?{} GLOB (REPLACE(keys, '*', '?') || '*')",
            schema_ids.len() + 1
        );
        let mut statement = self.connection.prepare(&sql)?;
        let parameters = schema_ids
            .iter()
            .map(|id| rusqlite::types::Value::Integer(*id))
            .chain([rusqlite::types::Value::Text(match_key.to_owned())]);
        let mut rows = statement.query(params_from_iter(parameters))?;

        let mut patterns = Vec::new();
        while let Some(row) = rows.next()? {
            let element_id: i64 = row.get(3)?;
            let Some(element) = Element::from_vpic_id(element_id) else {
                continue;
            };
            let pattern = Pattern {
                id: row.get(0)?,
                schema_id: row.get(1)?,
                keys: row.get(2)?,
                element,
                value: row.get(4)?,
                changed_on: row.get(5)?,
            };
            if element_id == MODEL_ELEMENT_ID
                && let Some(make) = row.get::<_, Option<String>>(6)?
            {
                patterns.push(Pattern {
                    element: Element::Make,
                    value: make,
                    ..pattern.clone()
                });
            }
            patterns.push(pattern);
        }
        Ok(patterns)
    }
}
