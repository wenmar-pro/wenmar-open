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

/// NHTSA defect investigations: the `investigations` dataset.
///
/// `investigation` holds one row for each action number. `is_open` is 1
/// when it has a day opened on or after
/// [`crate::investigations::OPEN_FROM`] and no day closed, and 0 otherwise.
/// `investigation_vehicle` holds one row for each year, make and model an
/// investigation lists, and one for each catalog model when a name matched
/// several; it has the columns the coverage report reads. `"match"` is
/// `exact`, `base` or `curated`, and NULL exactly when `model_id` is NULL.
/// `investigation_campaign` holds the recall campaigns an investigation
/// led to, written as `recall.campaign` writes them.
pub const INVESTIGATIONS: &str = "
CREATE TABLE IF NOT EXISTS investigation (
    id           INTEGER PRIMARY KEY,
    action       TEXT NOT NULL UNIQUE,
    kind         TEXT NOT NULL,
    opened       TEXT,
    closed       TEXT,
    is_open      INTEGER NOT NULL,
    manufacturer TEXT,
    component    TEXT,
    subject      TEXT,
    summary      TEXT
);

CREATE TABLE IF NOT EXISTS investigation_vehicle (
    investigation_id INTEGER NOT NULL,
    year             INTEGER,
    make_id          INTEGER,
    model_id         INTEGER,
    make_text        TEXT NOT NULL,
    model_text       TEXT NOT NULL,
    \"match\"          TEXT
);
CREATE INDEX IF NOT EXISTS investigation_vehicle_ymm ON investigation_vehicle (make_id, model_id, year);
CREATE INDEX IF NOT EXISTS investigation_vehicle_investigation ON investigation_vehicle (investigation_id);

CREATE TABLE IF NOT EXISTS investigation_campaign (
    investigation_id INTEGER NOT NULL,
    campaign         TEXT NOT NULL,
    PRIMARY KEY (campaign, investigation_id)
) WITHOUT ROWID;
CREATE INDEX IF NOT EXISTS investigation_campaign_investigation ON investigation_campaign (investigation_id);
";

/// The newest report of remedy progress a manufacturer filed with NHTSA
/// for each recall campaign: the `recall_reports` dataset.
///
/// One row for each NHTSA vehicle campaign that has a report. `campaign`
/// is the campaign number as `recall.campaign` holds it for `source`
/// `nhtsa`; it is text and not `recall.id`, so the dataset can be built
/// and read without the recalls. `kind` is `quarterly` or `annual`. A
/// period is a year and a quarter, such as `2026-2`: a quarterly report
/// has the same one in `period_from` and `period_to`, an annual report
/// covers four. The four counts are totals for the whole campaign as the
/// manufacturer reported them at the end of the period.
pub const RECALL_REPORTS: &str = "
CREATE TABLE IF NOT EXISTS recall_report (
    campaign      TEXT PRIMARY KEY,
    kind          TEXT NOT NULL,
    report_number INTEGER NOT NULL,
    period_from   TEXT NOT NULL,
    period_to     TEXT NOT NULL,
    submitted     TEXT,
    involved      INTEGER NOT NULL,
    remedied      INTEGER NOT NULL,
    unreachable   INTEGER NOT NULL,
    removed       INTEGER NOT NULL
);
";

/// NHTSA's crash test ratings, the `ratings` dataset.
///
/// `rating` holds one row for each vehicle NHTSA rated or listed: a make,
/// a model, a model year, a body style and a drive, in NHTSA's words. One
/// model year often has several. A star column holds 1 to 5, or NULL where
/// NHTSA gave no rating. A feature column holds `standard`, `optional`,
/// `both` or `no`, and an evaluation column `meets`, `does_not_meet`,
/// `standard_only`, `upgrade_only` or `pending`; NULL where the source
/// does not say. `rollover_risk_permille` is in thousandths: 191 is 19.1%.
/// `notes` holds NHTSA's safety concerns and footnotes, one to a line.
///
/// `rating_vehicle` ties a rating to the catalog: one row for each catalog
/// model its name matched, or one with a NULL model when it matched none.
/// `\"match\"` is `exact`, `base` or `curated`, and NULL exactly when
/// `model_id` is NULL.
pub const RATINGS: &str = "
CREATE TABLE IF NOT EXISTS rating (
    id                     INTEGER PRIMARY KEY,
    year                   INTEGER NOT NULL,
    make_text              TEXT NOT NULL,
    model_text             TEXT NOT NULL,
    body_style             TEXT,
    vehicle_type           TEXT,
    drive                  TEXT,
    release                INTEGER NOT NULL,
    overall                INTEGER,
    frontal                INTEGER,
    frontal_driver         INTEGER,
    frontal_passenger      INTEGER,
    side                   INTEGER,
    side_driver            INTEGER,
    side_passenger         INTEGER,
    side_barrier           INTEGER,
    side_front_seat        INTEGER,
    side_rear_seat         INTEGER,
    side_pole              INTEGER,
    rollover               INTEGER,
    rollover_risk_permille INTEGER,
    rollover_tip           INTEGER,
    curb_weight_lb         INTEGER,
    esc                    TEXT,
    fcw                    TEXT,
    fcw_evaluation         TEXT,
    ldw                    TEXT,
    ldw_evaluation         TEXT,
    cib                    TEXT,
    cib_evaluation         TEXT,
    dbs                    TEXT,
    dbs_evaluation         TEXT,
    backup_camera          TEXT,
    blind_spot_detection   TEXT,
    head_side_airbag       TEXT,
    torso_side_airbag      TEXT,
    notes                  TEXT
);

CREATE TABLE IF NOT EXISTS rating_vehicle (
    rating_id  INTEGER NOT NULL,
    year       INTEGER,
    make_id    INTEGER,
    model_id   INTEGER,
    make_text  TEXT NOT NULL,
    model_text TEXT NOT NULL,
    \"match\"    TEXT
);
CREATE INDEX IF NOT EXISTS rating_vehicle_ymm ON rating_vehicle (make_id, model_id, year);
CREATE INDEX IF NOT EXISTS rating_vehicle_rating ON rating_vehicle (rating_id);
";

/// NHTSA's index of the letters manufacturers file under 49 CFR Part 565
/// to explain how their VINs are built: the `vin_guides` dataset.
///
/// `vin_guide` holds one row for each filing, in order of letter date, and
/// never the letter itself: `url` is the address of NHTSA's copy.
/// `manufacturer_id` is vPIC's number for the manufacturer that filed.
/// `vin_guide_wmi` holds the manufacturer codes of the core data file
/// whose manufacturer name is the filer's, and `vin_guide_make` the catalog
/// makes built under those codes, so a reader is given a code or a make id
/// and matches no names.
pub const VIN_GUIDES: &str = "
CREATE TABLE vin_guide (
    id              INTEGER PRIMARY KEY,
    name            TEXT NOT NULL UNIQUE,
    manufacturer_id INTEGER NOT NULL,
    manufacturer    TEXT NOT NULL,
    kind            TEXT NOT NULL,
    letter_date     TEXT NOT NULL,
    year_from       INTEGER,
    year_to         INTEGER,
    url             TEXT NOT NULL
);
CREATE INDEX vin_guide_manufacturer ON vin_guide (manufacturer_id, letter_date);

CREATE TABLE vin_guide_wmi (
    wmi             TEXT NOT NULL,
    manufacturer_id INTEGER NOT NULL,
    PRIMARY KEY (wmi, manufacturer_id)
) WITHOUT ROWID;

CREATE TABLE vin_guide_make (
    make_id         INTEGER NOT NULL,
    manufacturer_id INTEGER NOT NULL,
    PRIMARY KEY (make_id, manufacturer_id)
) WITHOUT ROWID;
";
