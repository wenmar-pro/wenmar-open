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

/// Recall campaigns and the vehicles each lists. The two recall datasets,
/// `recalls_us` and `recalls_ca`, share these tables, and `recall.source`
/// tells them apart: `nhtsa` or `tc`. Either dataset's shape step may
/// create them, so every statement is `IF NOT EXISTS`.
///
/// `recall_vehicle` holds one row for each campaign, year, make and model
/// the source lists, and one for each catalog model when a name matched
/// several. `"match"` is `exact`, `base` or `curated`, and NULL exactly
/// when `model_id` is NULL. `year` is NULL where the source gave none.
/// `recall.report_date` is `''` where the source gave none, so that a list
/// can be read newest first straight from its index.
pub const RECALLS: &str = "
CREATE TABLE IF NOT EXISTS recall (
    id              INTEGER PRIMARY KEY,
    source          TEXT NOT NULL,
    campaign        TEXT NOT NULL,
    manufacturer    TEXT,
    report_date     TEXT NOT NULL,
    owner_notified  TEXT,
    component       TEXT,
    component_fr    TEXT,
    notification    TEXT,
    notification_fr TEXT,
    summary_en      TEXT,
    consequence_en  TEXT,
    remedy_en       TEXT,
    notes_en        TEXT,
    summary_fr      TEXT,
    mfr_campaign    TEXT,
    part_number     TEXT,
    units           INTEGER,
    do_not_drive    INTEGER,
    park_outside    INTEGER,
    UNIQUE (source, campaign)
);
CREATE INDEX IF NOT EXISTS recall_report_date ON recall (report_date);

CREATE TABLE IF NOT EXISTS recall_vehicle (
    recall_id  INTEGER NOT NULL,
    year       INTEGER,
    make_id    INTEGER,
    model_id   INTEGER,
    make_text  TEXT NOT NULL,
    model_text TEXT NOT NULL,
    \"match\"    TEXT,
    built_from TEXT,
    built_to   TEXT
);
CREATE INDEX IF NOT EXISTS recall_vehicle_ymm ON recall_vehicle (make_id, model_id, year);
CREATE INDEX IF NOT EXISTS recall_vehicle_recall ON recall_vehicle (recall_id);
CREATE INDEX IF NOT EXISTS recall_vehicle_year ON recall_vehicle (year, model_id);
";
