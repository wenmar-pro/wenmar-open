CREATE TABLE meta (
    key   TEXT PRIMARY KEY,
    value TEXT NOT NULL
);
INSERT INTO meta VALUES
  ('schema_version', '1'),
  ('data_version', '2026.10.12'),
  ('catalog_data_version', '2026.09'),
  ('built_at', '2026-10-12 06:00:00'),
  ('datasets', '{"recalls_us":{"as_of":"2026-10-11","rows":3,"licence":"public-domain","attribution":""},"recalls_ca":{"as_of":"2026-10-06","rows":1,"licence":"ogl-canada","attribution":"Contains information licensed under the Open Government Licence - Canada."}}');
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
