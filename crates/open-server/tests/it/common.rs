//! A small data file, built with `rusqlite` the way `open-data` builds the
//! real one, so the server is tested reading a plain SQLite file.

// These helpers are shared by every test module, and not every module uses
// every one of them.
#![allow(dead_code)]

use std::path::{Path, PathBuf};

use rusqlite::Connection;
use tempfile::TempDir;

/// A Hyundai Kona, model year 2023. Its check digit is right.
pub const KONA: &str = "KM8K2CAB4PU001140";
/// The same VIN with a wrong check digit.
pub const KONA_BAD_CHECK: &str = "KM8K2CAB0PU001140";
/// A bus. Position 7 does not settle the model year of a heavy vehicle, so
/// both 1995 and 2025 are tried.
pub const COACH: &str = "1M8PDMPA9SP000001";
/// A trailer from a low-volume maker: the code is `1A9` plus `881`.
pub const TRAILER: &str = "1A9100AA851881001";
/// A VIN whose model and trim, in the data, are HTML. Data is never trusted
/// to be plain text.
pub const HOSTILE: &str = "KM8K3CAB4PU001140";
/// No manufacturer is registered for `ZZZ`.
pub const UNKNOWN: &str = "ZZZK2CAB4PU001140";

/// A data file in a temporary directory, removed when dropped.
pub struct Fixture {
    directory: TempDir,
}

impl Fixture {
    pub fn path(&self) -> PathBuf {
        self.directory.path().join("data.sqlite3")
    }

    pub fn directory(&self) -> &Path {
        self.directory.path()
    }
}

const DECODER_ROWS: &str = "
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
  (19, 1, 'K3***', 28, '<script>alert(1)</script>', '2022-05-01 00:00:00', 'Hyundai', '902'),
  (24, 1, 'K3***', 38, '\"><img src=x onerror=alert(1)>', '2022-05-01 00:00:00', NULL, 'x'),
  (20, 3, 'PD***', 28, 'D-Series', '1996-01-01 00:00:00', 'MCI', '950'),
  (21, 4, 'PD***', 28, 'D-Series', '2021-01-01 00:00:00', 'MCI', '950'),
  (22, 4, 'PDM**', 5, 'Bus', '2021-01-01 00:00:00', NULL, '16'),
  (23, 4, 'PDMP*', 15, '6x4', '2021-01-01 00:00:00', NULL, '6'),
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
";

/// Type bits: passenger car 4, truck 8, trailer 64, MPV 128.
const CATALOG_ROWS: &str = "
INSERT INTO catalog_type VALUES
  (2, 'Passenger Car'), (3, 'Truck'), (5, 'Bus'), (6, 'Trailer'),
  (7, 'Multipurpose Passenger Vehicle (MPV)');
INSERT INTO catalog_make VALUES
  (460, 'ford', 'Ford', 'ford', 2, 8, 1),
  (467, 'chevrolet', 'Chevrolet', 'chevrolet', 3, 8, 1),
  (474, 'honda', 'Honda', 'honda', 4, 132, 1),
  (498, 'hyundai', 'Hyundai', 'hyundai', 12, 132, 1),
  (5000, 'ranger-trailers', 'Ranger Trailers', 'rangertrailers', NULL, 64, 0),
  (6000, 'b-b-script', 'B&B <script>alert(1)</script>', 'bbscriptalert1script', NULL, 4, 1);
INSERT INTO catalog_alias VALUES ('chevy', 467);
INSERT INTO catalog_model VALUES
  (900, 498, 'kona', 'Kona', 'kona', 2022, 2023, 128, 1),
  (1801, 460, 'f-150', 'F-150', 'f150', 2019, 2019, 8, 1),
  (1850, 467, 'silverado', 'Silverado', 'silverado', 2019, 2019, 8, 1),
  (1863, 474, 'civic', 'Civic', 'civic', 2018, 2020, 4, 1),
  (1865, 474, 'cr-v', 'CR-V', 'crv', 2019, 2019, 128, 1),
  (9100, 5000, 'tilt-deck', 'Tilt Deck', 'tiltdeck', 2019, 2019, 64, 0),
  (9500, 6000, 'model-img', 'Model \"<img src=x onerror=alert(1)>', 'modelimgsrcxonerroralert1', 2019, 2019, 4, 1);
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
  (10, 2019, 6000, 9500, 4, 1, 6);
INSERT INTO catalog_detail VALUES
  (1, NULL, 'FWD', NULL),
  (2, 'Pickup', NULL, NULL),
  (3, NULL, NULL, NULL),
  (4, 'Sedan', 'FWD', 'CVT'),
  (5, 'SUV', NULL, NULL),
  (6, NULL, NULL, NULL);
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
  (12, 5, 'Limited', 'limited', 'trim', 1, NULL, NULL, NULL),
  (13, 6, '<b>Bold</b>', 'bbold', 'trim', 1, NULL, NULL, NULL);
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
";

fn build(schema_version: &str, with_catalog: bool) -> Fixture {
    let fixture = Fixture {
        directory: tempfile::tempdir().unwrap(),
    };
    let connection = Connection::open(fixture.path()).unwrap();
    connection
        .execute_batch(wenmar_vin::sqlite::SCHEMA)
        .unwrap();
    connection
        .execute_batch(&format!(
            "INSERT INTO meta VALUES ('schema_version', '{schema_version}');"
        ))
        .unwrap();
    connection.execute_batch(DECODER_ROWS).unwrap();
    if with_catalog {
        connection
            .execute_batch(wenmar_vehicles::schema::SCHEMA)
            .unwrap();
        connection.execute_batch(CATALOG_ROWS).unwrap();
    }
    connection.close().unwrap();
    fixture
}

/// A data file of the current schema version.
pub fn data_file() -> Fixture {
    build(wenmar_vehicles::schema::SCHEMA_VERSION, true)
}

/// A data file built before the catalog existed.
pub fn old_data_file() -> Fixture {
    build("2", false)
}

/// A SQLite file that is not a data file at all.
pub fn other_sqlite_file() -> Fixture {
    let fixture = Fixture {
        directory: tempfile::tempdir().unwrap(),
    };
    let connection = Connection::open(fixture.path()).unwrap();
    connection
        .execute_batch("CREATE TABLE notes (body TEXT);")
        .unwrap();
    connection.close().unwrap();
    fixture
}

/// The names of the files beside the data file.
pub fn files(fixture: &Fixture) -> Vec<String> {
    let mut names: Vec<String> = std::fs::read_dir(fixture.directory())
        .unwrap()
        .map(|entry| entry.unwrap().file_name().to_string_lossy().into_owned())
        .collect();
    names.sort();
    names
}
