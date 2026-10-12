//! The tables of a Wenmar Open shop file.

/// Version of the shop file's table layout. Bump it only when an existing
/// table changes shape. A new table needs a `datasets` entry, not a bump.
pub const SCHEMA_VERSION: &str = "1";

/// The `meta` table, the same shape as the core data file's.
pub const META: &str = "
CREATE TABLE meta (
    key   TEXT PRIMARY KEY,
    value TEXT NOT NULL
);
";
