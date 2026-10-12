CREATE TABLE meta (
    key   TEXT PRIMARY KEY,
    value TEXT NOT NULL
);
INSERT INTO meta VALUES
  ('schema_version', '1'),
  ('data_version', '2026.10.12'),
  ('catalog_data_version', '2026.09'),
  ('built_at', '2026-10-12 06:00:00'),
  ('datasets', '{"recalls_us":{"as_of":"2026-10-11","rows":3,"licence":"public-domain","attribution":""},"recalls_ca":{"as_of":"2026-10-06","rows":1,"licence":"ogl-canada","attribution":"Contains information licensed under the Open Government Licence - Canada."},"ratings":{"as_of":"2026-08-07","rows":3,"licence":"public-domain","attribution":""},"fuel_us":{"as_of":"2026-09-29","rows":3,"licence":"public-domain","attribution":""},"fuel_ca":{"as_of":"2026-10-06","rows":1,"licence":"ogl-canada","attribution":"Contains information licensed under the Open Government Licence - Canada."},"specs":{"as_of":"2026-09-29","rows":3,"licence":"public-domain","attribution":""},"oil_grade":{"as_of":"2026-09-29","rows":2,"licence":"public-domain","attribution":""},"fluids":{"as_of":"2026-10-09","rows":3,"licence":"mit","attribution":""},"tires":{"as_of":"2026-10-09","rows":1,"licence":"mit","attribution":""}}');
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
    "match"    TEXT,
    built_from TEXT,
    built_to   TEXT
);
CREATE INDEX IF NOT EXISTS recall_vehicle_ymm ON recall_vehicle (make_id, model_id, year);
CREATE INDEX IF NOT EXISTS recall_vehicle_recall ON recall_vehicle (recall_id);
CREATE INDEX IF NOT EXISTS recall_vehicle_year ON recall_vehicle (year, model_id);
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
    "match"    TEXT
);
CREATE INDEX IF NOT EXISTS rating_vehicle_ymm ON rating_vehicle (make_id, model_id, year);
CREATE INDEX IF NOT EXISTS rating_vehicle_rating ON rating_vehicle (rating_id);
CREATE TABLE IF NOT EXISTS fuel_rating (
    id               INTEGER PRIMARY KEY,
    source           TEXT NOT NULL,    -- 'epa' | 'nrcan'
    source_id        INTEGER,          -- EPA's vehicle id; NULL for nrcan
    year             INTEGER NOT NULL,
    make_id          INTEGER,          -- catalog_make.id; NULL when the make is not in the catalog
    model_id         INTEGER,          -- catalog_model.id; NULL when the model did not match
    make_text        TEXT NOT NULL,
    model_text       TEXT NOT NULL,    -- as the source wrote it: 'F150 Pickup 2WD'
    "match"          TEXT,             -- 'exact' | 'base' | 'curated'; NULL exactly when model_id is NULL
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

INSERT INTO recall VALUES
  (1, 'nhtsa', '19V888000', 'General Motors LLC', '2019-12-12', '2020-03-09', 'ELECTRICAL SYSTEM:12V/24V/48V BATTERY:CABLES', NULL, NULL, NULL, 'The battery cable nut may be loose.', 'A fire may start.', 'Dealers will tighten the nut.', NULL, NULL, 'N192273510', NULL, 350371, 0, 0),
  (2, 'nhtsa', '23V123000', 'Hyundai Motor America', '2023-03-02', '2023-04-20', 'ELECTRICAL SYSTEM:WIRING', NULL, NULL, NULL, 'A wiring harness may overheat.', 'A fire may start while the vehicle is parked.', 'Dealers will replace the harness.', NULL, NULL, '240', '91850-J9000', 5000, 0, 1),
  (3, 'nhtsa', '24V456000', 'Hyundai Motor America', '2024-05-10', NULL, 'SERVICE BRAKES, HYDRAULIC', NULL, NULL, NULL, 'The brake control unit may fail.', 'Braking distance may increase.', 'Dealers will replace the unit.', NULL, NULL, '257', NULL, 800, 1, 0),
  (4, 'tc', '2023101', NULL, '2023-03-10', NULL, 'Electrical', 'Circuit electrique', 'Safety Mfr', 'Securite - fabricant', 'Issue: a wiring harness may overheat.', NULL, NULL, NULL, 'Un faisceau peut surchauffer.', '240', NULL, 410, NULL, NULL);
INSERT INTO recall_vehicle VALUES
  (1, 2019, 467, 1850, 'CHEVROLET', 'SILVERADO 1500', 'base', '2018-07-11', '2019-11-22'),
  (1, 2019, NULL, NULL, 'GMC', 'SIERRA 1500', NULL, NULL, NULL),
  (2, 2023, 498, 900, 'HYUNDAI', 'KONA', 'exact', '2022-09-01', '2023-01-31'),
  (3, 2023, 498, 900, 'HYUNDAI', 'KONA ELECTRIC', 'base', NULL, NULL),
  (4, 2023, 498, 900, 'HYUNDAI', 'KONA', 'exact', NULL, NULL);
INSERT INTO rating VALUES
  (1, 2023, 'HYUNDAI', 'KONA', 'SUV', 'MPV', 'AWD', 1, 5, 4, 4, 4, 5, 5, 5, 5, 5, 5, 5, 4, 148, 0, 3225, NULL, 'standard', 'meets', 'standard', 'meets', 'standard', 'does_not_meet', 'standard', 'pending', NULL, 'optional', 'standard', 'standard', NULL),
  (2, 2023, 'HYUNDAI', 'KONA', 'SUV', 'MPV', 'FWD', 1, 4, 4, 4, 4, 5, 5, 5, 5, 5, 5, 5, 4, 155, 0, 3005, NULL, 'standard', 'meets', 'standard', 'meets', 'both', 'upgrade_only', 'both', 'standard_only', NULL, 'optional', 'standard', 'standard', 'These ratings do not apply to the N Line.' || char(10) || 'The rear door opened in the side test.'),
  (3, 2023, 'HYUNDAI', 'KONA ELECTRIC', 'SUV', 'MPV', 'FWD', 1, NULL, NULL, NULL, NULL, NULL, NULL, NULL, NULL, NULL, NULL, NULL, NULL, NULL, NULL, NULL, NULL, 'standard', 'pending', 'standard', 'pending', NULL, NULL, NULL, NULL, NULL, NULL, 'standard', 'standard', NULL);
INSERT INTO rating_vehicle VALUES
  (1, 2023, 498, 900, 'HYUNDAI', 'KONA', 'exact'),
  (2, 2023, 498, 900, 'HYUNDAI', 'KONA', 'exact'),
  (3, 2023, 498, 900, 'HYUNDAI', 'KONA ELECTRIC', 'base');

INSERT INTO fuel_rating VALUES
  (1, 'epa', 46001, 2023, 498, 900, 'Hyundai', 'Kona FWD', 'exact', 8, NULL, 'engine', 'FWD', NULL, NULL, 2.0, 4, 'Automatic (variable gear ratios)', 'Front-Wheel Drive', 'Regular Gasoline', NULL, 7.8, 6.7, 7.4, 30, 35, 32, NULL, NULL, 172),
  (2, 'epa', 46003, 2023, 498, 900, 'Hyundai', 'Kona N Line AWD', 'base', 9, NULL, 'engine', 'AWD', NULL, NULL, 1.6, 4, 'Automatic (AM7)', 'All-Wheel Drive', 'Regular Gasoline', NULL, 8.7, 7.4, 8.1, 27, 32, 29, NULL, NULL, 190),
  (3, 'epa', 46004, 2023, 498, 900, 'Hyundai', 'Kona Electric', 'base', NULL, NULL, 'model', NULL, NULL, NULL, NULL, NULL, 'Automatic (A1)', 'Front-Wheel Drive', 'Electricity', NULL, NULL, NULL, NULL, NULL, NULL, NULL, 17.4, 415, 0),
  (4, 'nrcan', NULL, 2023, 498, 900, 'Hyundai', 'Kona', 'exact', 8, NULL, 'engine', NULL, NULL, NULL, 2.0, 4, 'AV', NULL, 'Regular Gasoline', NULL, 7.9, 6.6, 7.3, 30, 36, 32, NULL, NULL, 171);
INSERT INTO vehicle_spec VALUES
  (46001, 'Small Sport Utility Vehicle 2WD', 0, 0, 1, NULL, NULL, NULL, NULL, 94, NULL, NULL, 19, NULL, 1, 'Continuously Variable', 'Gasoline (Regular Unleaded Recommended)', 0, NULL, 1, 0, 2, 2, NULL, NULL, NULL),
  (46003, 'Small Sport Utility Vehicle 4WD', 1, 0, 1, NULL, NULL, NULL, NULL, 94, NULL, NULL, 19, NULL, 8, 'Automatic', 'Gasoline (Regular Unleaded Recommended)', 1, 'Half Engine Mode', 1, 0, 2, 2, NULL, NULL, NULL),
  (46004, 'Small Sport Utility Vehicle 2WD', 0, 0, NULL, 'EV', '150 kW AC 3-Phase', NULL, NULL, 94, NULL, NULL, 19, NULL, NULL, NULL, NULL, NULL, NULL, NULL, NULL, NULL, NULL, 'Lithium Ion', 400, '150');
INSERT INTO emission_standard VALUES
  (46001, 3, 'T3B30', 'Federal Tier 3 Bin 30', 7, 'PHYXV02.0AAA');
INSERT INTO oil_grade VALUES
  (46001, '0W-99', '0W-99'),
  (46003, '5W-99, 0W-98', '5W-99/0W-98');
INSERT INTO fluid VALUES
  (1, 498, 900, 2023, 'engine_oil', 8, NULL, '0W-99', 'TEST-SPEC-K', 77.7, 82.1, 'l', 1, NULL, NULL, NULL, NULL, 'Test manual K, page 1', '2026-10-09', 'test-reviewer'),
  (2, 498, 900, 2023, 'engine_oil', 9, NULL, NULL, 'TEST-SPEC-T', NULL, NULL, NULL, NULL, NULL, NULL, NULL, NULL, 'Test manual K, page 2', '2026-10-09', 'test-reviewer'),
  (3, 498, 900, 2023, 'brake', NULL, NULL, NULL, 'TEST BRAKE 9', NULL, NULL, NULL, NULL, NULL, NULL, NULL, NULL, 'Test manual K, page 3', '2026-10-09', 'test-reviewer');
INSERT INTO tire_fitment VALUES
  (1, 498, 900, 2023, '996/96R96', NULL, '["SE"]', 990, 980, 144, 142, 'kpa', NULL, NULL, NULL, NULL, NULL, 'Test door placard K', '2026-10-09', 'test-reviewer');
