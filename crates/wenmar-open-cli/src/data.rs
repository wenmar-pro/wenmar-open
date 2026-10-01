//! The local data file: what is there, and whether this build can read it.

use std::path::Path;

use rusqlite::{Connection, OpenFlags};
use serde::Serialize;
use wenmar_vehicles::schema::SCHEMA_VERSION;

/// What is at the data file's path.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Status {
    /// Whether anything is at the path.
    pub installed: bool,
    /// Whether this build can answer from it.
    pub usable: bool,
    pub path: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub bytes: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data_version: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub schema_version: Option<String>,
    /// The schema version this build reads.
    pub reads_schema_version: &'static str,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub vpic_release: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub built_at: Option<String>,
    /// Why the file cannot be used, when it cannot.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub problem: Option<String>,
}

impl Status {
    fn empty(path: &Path) -> Status {
        Status {
            installed: false,
            usable: false,
            path: path.display().to_string(),
            bytes: None,
            data_version: None,
            schema_version: None,
            reads_schema_version: SCHEMA_VERSION,
            vpic_release: None,
            built_at: None,
            problem: None,
        }
    }

    fn unusable(mut self, problem: impl Into<String>) -> Status {
        self.usable = false;
        self.problem = Some(problem.into());
        self
    }
}

/// Looks at the data file without changing it. A missing file is not a
/// problem: it is reported as not installed.
pub fn inspect(path: &Path) -> Status {
    let mut status = Status::empty(path);
    let metadata = match std::fs::metadata(path) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return status,
        Err(error) => {
            status.installed = true;
            return status.unusable(format!("it could not be read: {error}"));
        }
    };
    status.installed = true;
    if !metadata.is_file() {
        return status.unusable("it is not a file");
    }
    status.bytes = Some(metadata.len());

    let flags = OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_NO_MUTEX;
    let connection = match Connection::open_with_flags(path, flags) {
        Ok(connection) => connection,
        Err(error) => return status.unusable(format!("it could not be opened: {error}")),
    };
    let rows: Result<Vec<(String, String)>, rusqlite::Error> = connection
        .prepare("SELECT key, value FROM meta")
        .and_then(|mut statement| {
            statement
                .query_map([], |row| Ok((row.get(0)?, row.get(1)?)))?
                .collect()
        });
    let rows = match rows {
        Ok(rows) => rows,
        Err(_) => return status.unusable("it is not a Wenmar Open data file"),
    };
    for (key, value) in rows {
        match key.as_str() {
            "schema_version" => status.schema_version = Some(value),
            "data_version" => status.data_version = Some(value),
            "vpic_release" => status.vpic_release = Some(value),
            "built_at" => status.built_at = Some(value),
            _ => {}
        }
    }
    match status.schema_version.clone() {
        Some(version) if version == SCHEMA_VERSION => {
            status.usable = true;
            status
        }
        Some(version) => status.unusable(format!(
            "it has schema version {version} and this build reads version {SCHEMA_VERSION}"
        )),
        None => status.unusable("it is not a Wenmar Open data file"),
    }
}
