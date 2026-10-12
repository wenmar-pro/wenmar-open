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

/// Fuel ratings, the specifications that come with them, the certified
/// engine oil grade and the emission standard. The datasets `fuel_us`,
/// `fuel_ca`, `specs` and `oil_grade` share these tables. Every statement
/// is `IF NOT EXISTS`, so either fuel shape step may run it.
pub const FUEL: &str = "
CREATE TABLE IF NOT EXISTS fuel_rating (
    id               INTEGER PRIMARY KEY,
    source           TEXT NOT NULL,    -- 'epa' | 'nrcan'
    source_id        INTEGER,          -- EPA's vehicle id; NULL for nrcan
    year             INTEGER NOT NULL,
    make_id          INTEGER,          -- catalog_make.id; NULL when the make is not in the catalog
    model_id         INTEGER,          -- catalog_model.id; NULL when the model did not match
    make_text        TEXT NOT NULL,
    model_text       TEXT NOT NULL,    -- as the source wrote it: 'F150 Pickup 2WD'
    \"match\"          TEXT,             -- 'exact' | 'base' | 'curated'; NULL exactly when model_id is NULL
    engine_id        INTEGER,          -- catalog_engine.id; NULL unless one engine was settled
    submodel_id      INTEGER,          -- catalog_submodel.id; NULL unless the name spelled one
    confidence       TEXT,             -- 'model' | 'engine' | 'submodel'; NULL exactly when model_id is NULL
    drive_text       TEXT,             -- the drive word in the name: '2WD' | '4WD' | 'AWD' | 'FWD' | 'RWD'
    body_text        TEXT,             -- the body words in the name: 'Pickup'
    note_text        TEXT,             -- the text in parentheses in the name
    displacement_l   REAL,
    cylinders        INTEGER,
    transmission     TEXT,             -- EPA 'Automatic (S10)' | NRCan 'AS10'
    drive            TEXT,             -- EPA 'Rear-Wheel Drive'; NULL for nrcan
    fuel             TEXT NOT NULL,    -- 'Regular Gasoline' | 'Midgrade Gasoline' | 'Premium Gasoline' | 'Diesel' | 'E85' | 'Electricity' | 'Natural Gas' | 'Hydrogen'
    fuel2            TEXT,             -- EPA's second fuel: 'E85' | 'Electricity' | 'Natural Gas' | 'Propane'
    city_l_100km     REAL,             -- NULL when the fuel is not a liquid
    highway_l_100km  REAL,
    combined_l_100km REAL,
    city_mpg         INTEGER,          -- miles per US gallon
    highway_mpg      INTEGER,
    combined_mpg     INTEGER,
    kwh_100km        REAL,             -- combined electricity use
    range_km         INTEGER,          -- on a full battery
    co2_g_km         INTEGER           -- tailpipe, combined
);
CREATE INDEX IF NOT EXISTS fuel_rating_ymm ON fuel_rating (make_id, model_id, year);
CREATE INDEX IF NOT EXISTS fuel_rating_source ON fuel_rating (source, source_id);

CREATE TABLE IF NOT EXISTS vehicle_spec (
    epa_id                     INTEGER PRIMARY KEY,  -- fuel_rating.source_id where source = 'epa'
    size_class                 TEXT NOT NULL,        -- EPA's class: 'Standard Pickup Trucks 2WD'
    turbo                      INTEGER NOT NULL,     -- 0 or 1
    supercharger               INTEGER NOT NULL,
    start_stop                 INTEGER,              -- 0, 1, or NULL where EPA does not say
    electrification            TEXT,                 -- EPA atvType: 'Hybrid' | 'Plug-in Hybrid' | 'EV' | 'FFV' | 'Diesel' | ...
    ev_motor                   TEXT,                 -- '259V Li-Ion', '211 kW AC 3-Phase'
    charge_240v_h              REAL,
    passenger_ft3_2door        INTEGER,
    passenger_ft3_4door        INTEGER,
    passenger_ft3_hatch        INTEGER,
    luggage_ft3_2door          INTEGER,
    luggage_ft3_4door          INTEGER,
    luggage_ft3_hatch          INTEGER,
    -- The rest are from the yearly guide file and NULL without it.
    gears                      INTEGER,
    transmission_type          TEXT,                 -- 'Automatic' | 'Semi-Automatic' | 'Manual' | 'Continuously Variable' | ...
    fuel_usage                 TEXT,                 -- 'Gasoline (Premium Unleaded Required)'
    cylinder_deactivation      INTEGER,
    cylinder_deactivation_name TEXT,
    variable_valve_timing      INTEGER,
    variable_valve_lift        INTEGER,
    intake_valves              INTEGER,              -- per cylinder
    exhaust_valves             INTEGER,
    battery_type               TEXT,                 -- 'Lithium Ion' | 'NiMH'
    battery_voltage            INTEGER,
    motor_kw                   TEXT                  -- as written: '135', '8 and 30'
);

CREATE TABLE IF NOT EXISTS oil_grade (
    epa_id     INTEGER NOT NULL,   -- fuel_rating.source_id where source = 'epa'
    grades     TEXT NOT NULL,      -- '5W-30', or several joined by ', '
    grade_text TEXT NOT NULL       -- as the maker wrote it for EPA: '5W30 GF-5'
);
CREATE INDEX IF NOT EXISTS oil_grade_epa ON oil_grade (epa_id);

CREATE TABLE IF NOT EXISTS emission_standard (
    epa_id        INTEGER NOT NULL,  -- fuel_rating.source_id where source = 'epa'
    sales_area    INTEGER NOT NULL,  -- EPA's code: 1, 3, 5, 7, 8 or 9
    standard      TEXT NOT NULL,     -- 'T3B70'
    standard_text TEXT,              -- 'Federal Tier 3 Bin 70'
    smog_score    INTEGER,           -- 1 (worst) to 10 (best)
    engine_family TEXT NOT NULL      -- 'JFMXT02.73JK'
);
CREATE INDEX IF NOT EXISTS emission_standard_epa ON emission_standard (epa_id);
";

/// Manufacturer communications and the vehicles each lists: the
/// `bulletins` dataset.
///
/// `bulletin.id` is NHTSA's own id number. `type` is one of
/// `wenmar_shopdata::bulletins::BulletinType`'s ids. `added` is the day
/// NHTSA published the communication and is never empty; `date` is the
/// manufacturer's own date. `component` holds every NHTSA component the
/// communication names, separated by `; `.
///
/// A communication lists four million vehicles between them, so the
/// vehicle rows are compacted. `bulletin_model` holds each distinct make
/// and model text once for each catalog model it matched (`model_id` and
/// `"match"` are NULL together for a name that matched nothing).
/// `bulletin_vehicle` holds one row for each communication, name and run
/// of consecutive model years, `year_from` to `year_to` inclusive. A
/// communication that gave no model year has `year_from = year_to = 0`.
/// `bulletin_year` holds every year from 1900 to 2100 once: joined to a
/// range, it gives one row for each model year.
pub const BULLETINS: &str = "
CREATE TABLE bulletin (
    id           INTEGER PRIMARY KEY,
    type         TEXT NOT NULL,
    added        TEXT NOT NULL,
    date         TEXT,
    document_id  TEXT,
    mfr_campaign TEXT,
    replaces     TEXT,
    component    TEXT,
    mfr_system   TEXT,
    summary      TEXT
);
CREATE INDEX bulletin_added ON bulletin (added);

CREATE TABLE bulletin_model (
    id         INTEGER PRIMARY KEY,
    make_text  TEXT NOT NULL,
    model_text TEXT NOT NULL,
    make_id    INTEGER,
    model_id   INTEGER,
    \"match\"    TEXT
);
CREATE INDEX bulletin_model_model ON bulletin_model (model_id);

CREATE TABLE bulletin_vehicle (
    bulletin_id       INTEGER NOT NULL,
    bulletin_model_id INTEGER NOT NULL,
    year_from         INTEGER NOT NULL,
    year_to           INTEGER NOT NULL,
    PRIMARY KEY (bulletin_id, bulletin_model_id, year_from)
) WITHOUT ROWID;
CREATE INDEX bulletin_vehicle_model ON bulletin_vehicle (bulletin_model_id, year_from, year_to);

CREATE TABLE bulletin_year (
    year INTEGER PRIMARY KEY
);
";

/// What `year_from` and `year_to` hold when a communication gave no model
/// year.
pub const BULLETIN_NO_YEAR: i64 = 0;

/// The first and last year in `bulletin_year`.
pub const BULLETIN_YEARS: (i64, i64) = (1900, 2100);

/// `bulletin_vehicle` with one row for each model year, in the columns
/// `open-data coverage` reads. A row with no model year has `year` NULL.
pub const BULLETIN_VEHICLE_YEARS: &str = "(SELECT y.year AS year, m.make_id AS make_id, m.model_id AS model_id, m.make_text AS make_text, m.model_text AS model_text, m.\"match\" AS \"match\"
 FROM bulletin_vehicle v
 JOIN bulletin_model m ON m.id = v.bulletin_model_id
 JOIN bulletin_year y ON y.year BETWEEN v.year_from AND v.year_to
 UNION ALL
 SELECT NULL, m.make_id, m.model_id, m.make_text, m.model_text, m.\"match\"
 FROM bulletin_vehicle v
 JOIN bulletin_model m ON m.id = v.bulletin_model_id
 WHERE v.year_from = 0)";

/// Every table of a dataset that only the hosted service carries, by
/// dataset id. `open-data build-shop --offline-out` drops these from the
/// copy it makes for offline use.
pub const HOSTED_ONLY_TABLES: &[(&str, &[&str])] = &[(
    "bulletins",
    &[
        "bulletin_vehicle",
        "bulletin_model",
        "bulletin_year",
        "bulletin",
        "bulletin_component",
        "bulletin_code",
        "bulletin_topic",
    ],
)];

/// The components of `data/components.yaml`, and which of them each recall
/// and each bulletin is about. `component.parent` is the id of the broad
/// system a part belongs to, and NULL for a system. A shape step that
/// follows the sources creates these; `recall_component` exists when the
/// file holds a recall dataset and `bulletin_component` when it holds
/// bulletins.
pub const COMPONENTS: &str = "
CREATE TABLE component (
    id     TEXT PRIMARY KEY,
    name   TEXT NOT NULL,
    parent TEXT
) WITHOUT ROWID;
";

pub const RECALL_COMPONENTS: &str = "
CREATE TABLE recall_component (
    recall_id    INTEGER NOT NULL,
    component_id TEXT NOT NULL,
    PRIMARY KEY (recall_id, component_id)
) WITHOUT ROWID;
CREATE INDEX recall_component_component ON recall_component (component_id);
";

pub const BULLETIN_COMPONENTS: &str = "
CREATE TABLE bulletin_component (
    bulletin_id  INTEGER NOT NULL,
    component_id TEXT NOT NULL,
    PRIMARY KEY (bulletin_id, component_id)
) WITHOUT ROWID;
CREATE INDEX bulletin_component_component ON bulletin_component (component_id);
";

/// What a bulletin's summary names: trouble codes such as `P0420`, and
/// topics such as `maintenance`.
pub const BULLETIN_TEXT: &str = "
CREATE TABLE bulletin_code (
    bulletin_id INTEGER NOT NULL,
    code        TEXT NOT NULL,
    PRIMARY KEY (bulletin_id, code)
) WITHOUT ROWID;
CREATE INDEX bulletin_code_code ON bulletin_code (code);

CREATE TABLE bulletin_topic (
    bulletin_id INTEGER NOT NULL,
    topic       TEXT NOT NULL,
    PRIMARY KEY (bulletin_id, topic)
) WITHOUT ROWID;
CREATE INDEX bulletin_topic_topic ON bulletin_topic (topic);
";

/// Models that share campaigns and bulletins with a model year.
///
/// Derived at build time: a row is written when at least
/// [`SIBLING_FEWEST_SHARED`] recall campaigns and bulletins list both
/// models for that model year, and those are at least one in
/// [`SIBLING_SHARE`] of the documents that list `model_id`. Only documents
/// that list at most [`SIBLING_MOST_MODELS`] catalog models are counted on
/// either side. It says the two have documents in common, not that a part
/// fits both.
pub const SIBLINGS: &str = "
CREATE TABLE vehicle_sibling (
    model_id         INTEGER NOT NULL,
    year             INTEGER NOT NULL,
    sibling_model_id INTEGER NOT NULL,
    shared           INTEGER NOT NULL,
    total            INTEGER NOT NULL,
    PRIMARY KEY (model_id, year, sibling_model_id)
) WITHOUT ROWID;
";

/// A document listing more catalog models than this claims nothing.
pub const SIBLING_MOST_MODELS: i64 = 4;
/// A pair needs at least this many shared documents.
pub const SIBLING_FEWEST_SHARED: i64 = 5;
/// A pair needs shared documents at least a fifth of the model's total.
pub const SIBLING_SHARE: i64 = 5;

/// Owner complaints to NHTSA, as counts only: the `complaints` dataset.
///
/// One row for each catalog model year and component of
/// `data/components.yaml` that has a complaint, and one row with
/// `component_id = ''` that counts every complaint of the model year.
/// `crashes`, `fires` and `injuries` are how many of the complaints
/// report one. `median_miles` is the middle mileage at failure among the
/// `with_miles` complaints that gave one.
///
/// Nothing an owner wrote, no place, no VIN and no name is in the file:
/// these ten columns are everything kept.
pub const COMPLAINTS: &str = "
CREATE TABLE complaint_count (
    make_id      INTEGER NOT NULL,
    model_id     INTEGER NOT NULL,
    year         INTEGER NOT NULL,
    component_id TEXT NOT NULL,
    complaints   INTEGER NOT NULL,
    crashes      INTEGER NOT NULL,
    fires        INTEGER NOT NULL,
    injuries     INTEGER NOT NULL,
    with_miles   INTEGER NOT NULL,
    median_miles INTEGER,
    PRIMARY KEY (model_id, year, component_id)
) WITHOUT ROWID;
";

/// The columns of `complaint_count`, in order. A test holds the table to
/// exactly these.
pub const COMPLAINT_COLUMNS: [&str; 10] = [
    "make_id",
    "model_id",
    "year",
    "component_id",
    "complaints",
    "crashes",
    "fires",
    "injuries",
    "with_miles",
    "median_miles",
];

/// Fluids read by hand from the maker's own documents: the `fluids`
/// dataset, built from `data/fluids/*.yaml`.
///
/// One row for each model year a record covers, so a reader asks for one
/// year and joins nothing. `engine_id` is the engine of that model year
/// the record names, and NULL when the record is for every engine. A
/// capacity or a charge is stored in both units: the one in
/// `capacity_unit` or `charge_unit` is the figure the source printed, and
/// the other was worked out from it at build time.
pub const FLUIDS: &str = "
CREATE TABLE IF NOT EXISTS fluid (
    id            INTEGER PRIMARY KEY,
    make_id       INTEGER NOT NULL,   -- catalog_make.id
    model_id      INTEGER NOT NULL,   -- catalog_model.id
    year          INTEGER NOT NULL,
    kind          TEXT NOT NULL,      -- 'engine_oil' | 'transmission' | 'coolant' | 'refrigerant' | 'brake'
    engine_id     INTEGER,            -- catalog_engine.id; NULL when the record names no engine
    applies_to    TEXT,               -- which transmission or system, in our words; NULL for all
    grade         TEXT,               -- engine oil viscosity, as the source gives it
    specification TEXT,               -- the maker's specification or the type of fluid
    capacity_l    REAL,
    capacity_qt   REAL,               -- US liquid quarts
    capacity_unit TEXT,               -- the unit the source printed: 'l' | 'qt'
    with_filter   INTEGER,            -- 0 or 1; NULL when there is no capacity
    charge_g      INTEGER,            -- refrigerant charge
    charge_oz     REAL,
    charge_unit   TEXT,               -- 'g' | 'kg' | 'oz' | 'lb'
    market        TEXT,               -- 'us' | 'ca'; NULL when the source is for both
    source        TEXT NOT NULL,      -- the document read
    checked       TEXT NOT NULL,      -- the day it was read, 2026-10-12
    reviewer      TEXT NOT NULL       -- the GitHub name of who compared it with the source
);
CREATE INDEX IF NOT EXISTS fluid_ymm ON fluid (make_id, model_id, year);
";

/// Original-equipment tire fitments read by hand from the door placard
/// and the owner's manual: the `tires` dataset, built from
/// `data/tires/*.yaml`.
///
/// One row for each fitment and each model year its record covers. A
/// model year has as many rows as the maker fitted sizes. `trims` is a
/// JSON list of the trims the source ties the size to, as the source
/// names them, and `[]` when it names none. Pressures and torque are
/// whole numbers in both units; `pressure_unit` and `lug_torque_unit`
/// say which the source printed.
pub const TIRES: &str = "
CREATE TABLE IF NOT EXISTS tire_fitment (
    id                 INTEGER PRIMARY KEY,
    make_id            INTEGER NOT NULL,
    model_id           INTEGER NOT NULL,
    year               INTEGER NOT NULL,
    size               TEXT NOT NULL,   -- the front size, and the rear when size_rear is NULL
    size_rear          TEXT,            -- only when the rear differs
    trims              TEXT NOT NULL,   -- JSON list of text
    pressure_front_kpa INTEGER,         -- cold
    pressure_rear_kpa  INTEGER,
    pressure_front_psi INTEGER,
    pressure_rear_psi  INTEGER,
    pressure_unit      TEXT,            -- 'kpa' | 'bar' | 'psi'
    lug_torque_nm      INTEGER,
    lug_torque_lbft    INTEGER,
    lug_torque_unit    TEXT,            -- 'nm' | 'lbft'
    bolt_pattern       TEXT,            -- studs x pitch circle in millimetres, as the source gives it
    market             TEXT,
    source             TEXT NOT NULL,
    checked            TEXT NOT NULL,
    reviewer           TEXT NOT NULL
);
CREATE INDEX IF NOT EXISTS tire_fitment_ymm ON tire_fitment (make_id, model_id, year);
";

/// Links to a model year's owner's manual and maintenance schedule on the
/// maker's own site, found by a person: the `manuals` dataset, built from
/// the `manuals` lists of `data/fluids/*.yaml`.
///
/// One row for each link and each model year its record covers. Only the
/// address is held, never the document.
pub const MANUALS: &str = "
CREATE TABLE IF NOT EXISTS manual_link (
    id       INTEGER PRIMARY KEY,
    make_id  INTEGER NOT NULL,   -- catalog_make.id
    model_id INTEGER NOT NULL,   -- catalog_model.id
    year     INTEGER NOT NULL,
    kind     TEXT NOT NULL,      -- 'owner_manual' | 'maintenance_schedule'
    url      TEXT NOT NULL,      -- on the maker's own site
    market   TEXT,               -- 'us' | 'ca'; NULL when the page is for both
    checked  TEXT NOT NULL,      -- the day a person opened it, 2026-10-12
    reviewer TEXT NOT NULL
);
CREATE INDEX IF NOT EXISTS manual_link_ymm ON manual_link (make_id, model_id, year);
";
