//! The catalog tables of a Wenmar Open data file.

/// Version of the data file's table layout. It is the same number
/// `wenmar-vin` checks, kept here so readers that do not link `wenmar-vin`'s
/// SQLite support can check it too.
pub const SCHEMA_VERSION: &str = "3";

/// vPIC vehicle type ids shown by default: passenger car, truck, and
/// multipurpose passenger vehicle.
pub const LIGHT_TYPES: [u8; 3] = [2, 3, 7];

/// The catalog's tables. The data build creates them after `wenmar-vin`'s.
pub const SCHEMA: &str = "
CREATE TABLE catalog_type (
    id   INTEGER PRIMARY KEY,
    name TEXT NOT NULL
);

CREATE TABLE catalog_make (
    id    INTEGER PRIMARY KEY,
    slug  TEXT NOT NULL,
    name  TEXT NOT NULL,
    norm  TEXT NOT NULL,
    rank  INTEGER,
    types INTEGER NOT NULL,
    light INTEGER NOT NULL
);
CREATE UNIQUE INDEX catalog_make_slug ON catalog_make (slug);

CREATE TABLE catalog_alias (
    norm    TEXT NOT NULL,
    make_id INTEGER NOT NULL
);

CREATE TABLE catalog_model (
    id        INTEGER PRIMARY KEY,
    make_id   INTEGER NOT NULL,
    slug      TEXT NOT NULL,
    name      TEXT NOT NULL,
    norm      TEXT NOT NULL,
    year_from INTEGER NOT NULL,
    year_to   INTEGER NOT NULL,
    types     INTEGER NOT NULL,
    light     INTEGER NOT NULL
);
CREATE UNIQUE INDEX catalog_model_slug ON catalog_model (make_id, slug);
CREATE INDEX catalog_model_norm ON catalog_model (norm);

CREATE TABLE catalog_vehicle (
    id        INTEGER PRIMARY KEY,
    year      INTEGER NOT NULL,
    make_id   INTEGER NOT NULL,
    model_id  INTEGER NOT NULL,
    types     INTEGER NOT NULL,
    light     INTEGER NOT NULL,
    detail_id INTEGER
);
CREATE UNIQUE INDEX catalog_vehicle_key ON catalog_vehicle (year, make_id, model_id);
CREATE INDEX catalog_vehicle_model ON catalog_vehicle (model_id, year);

CREATE TABLE catalog_detail (
    id           INTEGER PRIMARY KEY,
    body         TEXT,
    drive        TEXT,
    transmission TEXT
);

CREATE TABLE catalog_submodel (
    id           INTEGER PRIMARY KEY,
    detail_id    INTEGER NOT NULL,
    name         TEXT NOT NULL,
    norm         TEXT NOT NULL,
    kind         TEXT NOT NULL,
    listed       INTEGER NOT NULL,
    body         TEXT,
    drive        TEXT,
    transmission TEXT
);
CREATE INDEX catalog_submodel_detail ON catalog_submodel (detail_id);

CREATE TABLE catalog_engine (
    id        INTEGER PRIMARY KEY,
    detail_id INTEGER NOT NULL,
    label     TEXT NOT NULL,
    vin8      TEXT,
    source    TEXT NOT NULL
);
CREATE INDEX catalog_engine_detail ON catalog_engine (detail_id);

CREATE TABLE catalog_submodel_engine (
    submodel_id INTEGER NOT NULL,
    engine_id   INTEGER NOT NULL
);
CREATE INDEX catalog_submodel_engine_submodel ON catalog_submodel_engine (submodel_id);
";
