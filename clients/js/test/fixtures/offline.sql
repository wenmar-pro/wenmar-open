CREATE TABLE meta (
    key   TEXT PRIMARY KEY,
    value TEXT NOT NULL
);

CREATE TABLE wmi (
    code            TEXT PRIMARY KEY,
    manufacturer    TEXT NOT NULL,
    make            TEXT,
    country         TEXT,
    vehicle_type    TEXT,
    light_vehicle   INTEGER NOT NULL,
    vehicle_type_id INTEGER
);

CREATE TABLE wmi_make (
    wmi     TEXT NOT NULL,
    make_id INTEGER NOT NULL
);
CREATE INDEX wmi_make_wmi ON wmi_make (wmi);

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
    make       TEXT,
    attribute  TEXT NOT NULL
);
CREATE INDEX pattern_schema ON pattern (schema_id);

CREATE TABLE spec_schema (
    id              INTEGER PRIMARY KEY,
    make_id         INTEGER NOT NULL,
    vehicle_type_id INTEGER
);

CREATE TABLE spec_schema_model (
    schema_id INTEGER NOT NULL,
    model_id  INTEGER NOT NULL
);
CREATE INDEX spec_schema_model_model ON spec_schema_model (model_id);

CREATE TABLE spec_schema_year (
    schema_id INTEGER NOT NULL,
    year      INTEGER NOT NULL
);
CREATE INDEX spec_schema_year_schema ON spec_schema_year (schema_id);

CREATE TABLE spec_row (
    id              INTEGER PRIMARY KEY,
    spec_pattern_id INTEGER NOT NULL,
    schema_id       INTEGER NOT NULL,
    is_key          INTEGER NOT NULL,
    element_id      INTEGER NOT NULL,
    attribute       TEXT NOT NULL,
    value           TEXT NOT NULL,
    changed_on      TEXT NOT NULL
);
CREATE INDEX spec_row_schema ON spec_row (schema_id);

CREATE TABLE engine_model_row (
    id           INTEGER PRIMARY KEY,
    engine_model TEXT NOT NULL,
    element_id   INTEGER NOT NULL,
    attribute    TEXT NOT NULL,
    value        TEXT NOT NULL,
    changed_on   TEXT NOT NULL
);
CREATE INDEX engine_model_row_name ON engine_model_row (engine_model);
INSERT INTO meta VALUES ('schema_version', '3');

INSERT INTO meta VALUES
  ('data_version', '2026.09'),
  ('vpic_release', 'vPICList_lite_2026_09'),
  ('built_at', '2026-10-01 04:25:57');
INSERT INTO wmi VALUES
  ('KM8', 'Hyundai Motor Co', 'Hyundai', 'South Korea', 'Multipurpose Passenger Vehicle (MPV)', 1, 7),
  ('1M8', 'Motor Coach Industries', NULL, 'United States (USA)', 'Bus', 0, 5),
  ('1A9', 'Many Small Makers', NULL, 'United States (USA)', 'Trailer', 0, 6),
  ('1A9881', 'Ranger Trailer Works', 'Ranger Trailers', 'United States (USA)', 'Trailer', 0, 6);
INSERT INTO wmi_make VALUES ('KM8', 498), ('1A9881', 5000);
INSERT INTO wmi_schema VALUES
  ('KM8', 1, 2022, NULL), ('KM8', 2, 1990, 1995),
  ('1M8', 3, 1990, 1999), ('1M8', 4, 2020, NULL),
  ('1A9881', 5, 2000, NULL);
INSERT INTO pattern VALUES
  (10, 1, 'K2***', 28, 'Kona', '2022-05-01 00:00:00', 'Hyundai', '900'),
  (11, 1, 'K[2-3]CA', 38, 'SE', '2022-05-01 00:00:00', NULL, 'SE'),
  (12, 1, 'K9***', 38, 'Does Not Match', '2022-05-01 00:00:00', NULL, 'x'),
  (13, 1, '*****|*U', 31, 'Ulsan', '2022-05-01 00:00:00', NULL, 'Ulsan'),
  (14, 1, 'K2***', 96, 'Internal Element', '2022-05-01 00:00:00', NULL, 'x'),
  (15, 2, 'K2***', 28, 'Old Model', '1995-01-01 00:00:00', 'Hyundai', '901'),
  (16, 1, 'K2***', 18, 'G4NH', '2022-05-01 00:00:00', NULL, 'G4NH'),
  (17, 1, 'K2***', 13, '2.0', '2022-05-01 00:00:00', NULL, '2.0'),
  (18, 1, 'K2***', 5, 'Sport Utility Vehicle (SUV)/Multi-Purpose Vehicle (MPV)', '2022-05-01 00:00:00', NULL, '7'),
  (20, 3, 'PD***', 28, 'D-Series', '1996-01-01 00:00:00', 'MCI', '950'),
  (21, 4, 'PD***', 28, 'D-Series', '2021-01-01 00:00:00', 'MCI', '950'),
  (22, 4, 'PDM**', 5, 'Bus', '2021-01-01 00:00:00', NULL, '16'),
  (23, 4, 'PDMP*', 15, '6x4', '2021-01-01 00:00:00', NULL, '6'),
  (25, 3, 'PE***', 28, 'E-Series', '1996-01-01 00:00:00', 'MCI', '951'),
  (26, 3, 'PE***', 5, 'Bus', '1996-01-01 00:00:00', NULL, '16'),
  (27, 3, 'PE***', 9, '6', '1996-01-01 00:00:00', NULL, '6'),
  (28, 4, 'PE***', 28, 'E-Series', '2021-01-01 00:00:00', 'MCI', '951'),
  (30, 5, '100**', 28, 'Tilt Deck', '2001-01-01 00:00:00', 'Ranger Trailers', '9100');
INSERT INTO spec_schema VALUES (50, 498, 7), (51, 498, 7), (52, 999, 7), (53, 498, 5);
INSERT INTO spec_schema_model VALUES (50, 900), (51, 900), (52, 900), (53, 900);
INSERT INTO spec_schema_year VALUES (50, 2023), (51, 2021);
INSERT INTO spec_row VALUES
  (1, 10, 50, 1, 38, 'SE', 'SE', '2023-01-01 00:00:00'),
  (2, 10, 50, 0, 86, '1', 'Standard', '2023-01-01 00:00:00'),
  (3, 11, 51, 0, 86, '2', 'Other Year', '2023-01-01 00:00:00'),
  (4, 12, 52, 0, 86, '2', 'Other Make', '2023-01-01 00:00:00'),
  (5, 13, 53, 0, 86, '2', 'Other Vehicle Type', '2023-01-01 00:00:00'),
  (6, 10, 50, 0, 168, '1', 'Direct', '2023-01-01 00:00:00'),
  (7, 10, 50, 0, 37, '2', 'Automatic', '2023-01-01 00:00:00');
INSERT INTO engine_model_row VALUES
  (1, 'g4nh', 9, '4', '4', '2020-01-01 00:00:00'),
  (2, 'g4nh', 24, '4', 'Gasoline', '2020-01-01 00:00:00'),
  (3, 'other', 9, '8', '8', '2020-01-01 00:00:00');

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

-- Trim and series spellings the build replaced with another name. `raw` is
-- the spelling replaced, in lowercase with single spaces. A decode still
-- carries the old spelling, so the reader needs these to find the submodel.
CREATE TABLE catalog_rename (
    raw  TEXT NOT NULL,
    name TEXT NOT NULL
);

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

INSERT INTO catalog_type VALUES
  (2, 'Passenger Car'), (3, 'Truck'), (5, 'Bus'), (6, 'Trailer'),
  (7, 'Multipurpose Passenger Vehicle (MPV)');
INSERT INTO catalog_make VALUES
  (460, 'ford', 'Ford', 'ford', 2, 8, 1),
  (467, 'chevrolet', 'Chevrolet', 'chevrolet', 3, 8, 1),
  (474, 'honda', 'Honda', 'honda', 4, 132, 1),
  (498, 'hyundai', 'Hyundai', 'hyundai', 12, 132, 1),
  (5000, 'ranger-trailers', 'Ranger Trailers', 'rangertrailers', NULL, 64, 0);
INSERT INTO catalog_alias VALUES ('chevy', 467);
INSERT INTO catalog_rename VALUES ('se plus', 'SE');
INSERT INTO catalog_model VALUES
  (900, 498, 'kona', 'Kona', 'kona', 2022, 2023, 128, 1),
  (1801, 460, 'f-150', 'F-150', 'f150', 2019, 2019, 8, 1),
  (1850, 467, 'silverado', 'Silverado', 'silverado', 2019, 2019, 8, 1),
  (1863, 474, 'civic', 'Civic', 'civic', 2018, 2020, 4, 1),
  (1865, 474, 'cr-v', 'CR-V', 'crv', 2019, 2019, 128, 1),
  (9100, 5000, 'tilt-deck', 'Tilt Deck', 'tiltdeck', 2019, 2020, 64, 0);
INSERT INTO catalog_vehicle VALUES
  (1, 2018, 474, 1863, 4, 1, 1),
  (2, 2019, 460, 1801, 8, 1, 2),
  (3, 2019, 467, 1850, 8, 1, 3),
  (4, 2019, 474, 1863, 4, 1, 1),
  (5, 2019, 474, 1865, 128, 1, NULL),
  (6, 2019, 5000, 9100, 64, 0, NULL),
  (7, 2020, 474, 1863, 4, 1, 4),
  (8, 2022, 498, 900, 128, 1, 5),
  (9, 2023, 498, 900, 128, 1, 5),
  (11, 2020, 5000, 9100, 64, 0, NULL);
INSERT INTO catalog_detail VALUES
  (1, NULL, 'FWD', NULL),
  (2, 'Pickup', NULL, NULL),
  (3, NULL, NULL, NULL),
  (4, 'Sedan', 'FWD', 'CVT'),
  (5, 'SUV', NULL, NULL);
INSERT INTO catalog_submodel VALUES
  (1, 1, 'LX', 'lx', 'trim', 1, NULL, NULL, NULL),
  (2, 1, 'Si', 'si', 'trim', 1, 'Sedan', NULL, 'Manual'),
  (3, 1, 'Touring', 'touring', 'trim', 1, NULL, NULL, 'CVT'),
  (4, 2, 'Raptor', 'raptor', 'trim', 1, NULL, '4WD', NULL),
  (5, 2, 'XLT', 'xlt', 'preset', 1, NULL, NULL, NULL),
  (7, 3, 'LT', 'lt', 'trim', 1, NULL, NULL, NULL),
  (8, 3, '1500', '1500', 'series', 0, NULL, NULL, NULL),
  (10, 4, 'LX', 'lx', 'trim', 1, NULL, NULL, NULL),
  (11, 5, 'SE', 'se', 'trim', 1, NULL, NULL, NULL),
  (12, 5, 'Limited', 'limited', 'trim', 1, NULL, NULL, NULL);
INSERT INTO catalog_engine VALUES
  (1, 1, '1.5L Turbo', NULL, 'vpic'),
  (2, 1, '2.0L', NULL, 'vpic'),
  (3, 2, '3.5L Turbo V6', '4G', 'vpic'),
  (4, 2, '5.0L V8', '5', 'vpic'),
  (5, 3, '5.3L V8', 'CR', 'vpic'),
  (7, 4, '2.0L', NULL, 'preset'),
  (8, 5, '2.0L', 'A', 'vpic'),
  (9, 5, '1.6L Turbo', NULL, 'vpic');
INSERT INTO catalog_submodel_engine VALUES (2, 1);
