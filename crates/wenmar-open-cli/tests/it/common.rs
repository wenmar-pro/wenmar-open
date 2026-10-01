//! A small data file, built with `rusqlite` the way `open-data` builds the
//! real one, and a way to run the tool in this process.

// These helpers are shared by every test module, and not every module uses
// every one of them.
#![allow(dead_code)]

use std::ffi::OsString;
use std::path::{Path, PathBuf};

use rusqlite::Connection;
use serde_json::Value;
use tempfile::TempDir;
use wenmar_open_cli::env::{DATA_FILE, Env};

/// A Hyundai Kona, model year 2023. Its check digit is right.
pub const KONA: &str = "KM8K2CAB4PU001140";
/// The same VIN with a wrong check digit.
pub const KONA_BAD_CHECK: &str = "KM8K2CAB0PU001140";
/// A VIN whose model, in the data, carries terminal control characters.
/// Data is never trusted to be plain text.
pub const HOSTILE: &str = "KM8K3CAB4PU001140";
/// No manufacturer is registered for `ZZZ`.
pub const UNKNOWN: &str = "ZZZK2CAB4PU001140";

const DECODER_ROWS: &str = "
INSERT INTO meta VALUES
  ('data_version', '2026.09'),
  ('vpic_release', 'vPICList_lite_2026_09'),
  ('built_at', '2026-10-01 04:25:57');
INSERT INTO wmi VALUES
  ('KM8', 'Hyundai Motor Co', 'Hyundai', 'South Korea', 'Multipurpose Passenger Vehicle (MPV)', 1, 7);
INSERT INTO wmi_make VALUES ('KM8', 498);
INSERT INTO wmi_schema VALUES ('KM8', 1, 2022, NULL), ('KM8', 2, 1990, 1995);
INSERT INTO pattern VALUES
  (10, 1, 'K2***', 28, 'Kona', '2022-05-01 00:00:00', 'Hyundai', '900'),
  (11, 1, 'K[2-3]CA', 38, 'SE', '2022-05-01 00:00:00', NULL, 'SE'),
  (13, 1, '*****|*U', 31, 'Ulsan', '2022-05-01 00:00:00', NULL, 'Ulsan'),
  (15, 2, 'K2***', 28, 'Old Model', '1995-01-01 00:00:00', 'Hyundai', '901'),
  (16, 1, 'K2***', 18, 'G4NH', '2022-05-01 00:00:00', NULL, 'G4NH'),
  (17, 1, 'K2***', 13, '2.0', '2022-05-01 00:00:00', NULL, '2.0'),
  (18, 1, 'K2***', 5, 'Sport Utility Vehicle (SUV)/Multi-Purpose Vehicle (MPV)', '2022-05-01 00:00:00', NULL, '7'),
  (19, 1, 'K3***', 28, 'Kona' || char(27) || '[31m' || char(7) || ' Red', '2022-05-01 00:00:00', 'Hyundai', '902');
INSERT INTO spec_schema VALUES (50, 498, 7);
INSERT INTO spec_schema_model VALUES (50, 900);
INSERT INTO spec_schema_year VALUES (50, 2023);
INSERT INTO spec_row VALUES
  (1, 10, 50, 1, 38, 'SE', 'SE', '2023-01-01 00:00:00'),
  (2, 10, 50, 0, 86, '1', 'Standard', '2023-01-01 00:00:00'),
  (6, 10, 50, 0, 168, '1', 'Direct', '2023-01-01 00:00:00'),
  (7, 10, 50, 0, 37, '2', 'Automatic', '2023-01-01 00:00:00');
INSERT INTO engine_model_row VALUES
  (1, 'g4nh', 9, '4', '4', '2020-01-01 00:00:00'),
  (2, 'g4nh', 24, '4', 'Gasoline', '2020-01-01 00:00:00');
";

/// Type bits: passenger car 4, truck 8, trailer 64, MPV 128.
const CATALOG_ROWS: &str = "
INSERT INTO catalog_type VALUES
  (2, 'Passenger Car'), (3, 'Truck'), (6, 'Trailer'),
  (7, 'Multipurpose Passenger Vehicle (MPV)');
INSERT INTO catalog_make VALUES
  (460, 'ford', 'Ford', 'ford', 2, 8, 1),
  (467, 'chevrolet', 'Chevrolet', 'chevrolet', 3, 8, 1),
  (474, 'honda', 'Honda', 'honda', 4, 132, 1),
  (498, 'hyundai', 'Hyundai', 'hyundai', 12, 132, 1),
  (5000, 'ranger-trailers', 'Ranger Trailers', 'rangertrailers', NULL, 64, 0);
INSERT INTO catalog_alias VALUES ('chevy', 467);
INSERT INTO catalog_model VALUES
  (900, 498, 'kona', 'Kona', 'kona', 2022, 2023, 128, 1),
  (1801, 460, 'f-150', 'F-150', 'f150', 2019, 2019, 8, 1),
  (1850, 467, 'silverado', 'Silverado', 'silverado', 2019, 2019, 8, 1),
  (1863, 474, 'civic', 'Civic', 'civic', 2018, 2020, 4, 1),
  (1865, 474, 'cr-v', 'CR-V', 'crv', 2019, 2019, 128, 1),
  (9100, 5000, 'tilt-deck', 'Tilt Deck', 'tiltdeck', 2019, 2019, 64, 0);
INSERT INTO catalog_vehicle VALUES
  (1, 2018, 474, 1863, 4, 1, 1),
  (2, 2019, 460, 1801, 8, 1, 2),
  (3, 2019, 467, 1850, 8, 1, 3),
  (4, 2019, 474, 1863, 4, 1, 1),
  (5, 2019, 474, 1865, 128, 1, NULL),
  (6, 2019, 5000, 9100, 64, 0, NULL),
  (7, 2020, 474, 1863, 4, 1, 4),
  (8, 2022, 498, 900, 128, 1, 5),
  (9, 2023, 498, 900, 128, 1, 5);
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
";

/// A data directory in a temporary directory, removed when dropped.
pub struct Fixture {
    directory: TempDir,
}

impl Fixture {
    /// The data directory.
    pub fn directory(&self) -> &Path {
        self.directory.path()
    }

    /// The data file inside it.
    pub fn file(&self) -> PathBuf {
        self.directory.path().join(DATA_FILE)
    }

    /// The names of the files in the data directory.
    pub fn files(&self) -> Vec<String> {
        let mut names: Vec<String> = std::fs::read_dir(self.directory())
            .unwrap()
            .map(|entry| entry.unwrap().file_name().to_string_lossy().into_owned())
            .collect();
        names.sort();
        names
    }
}

/// Writes a data file of the given schema version at `path`.
pub fn write_data_file(path: &Path, schema_version: &str, data_version: &str) {
    let connection = Connection::open(path).unwrap();
    connection
        .execute_batch(wenmar_vin::sqlite::SCHEMA)
        .unwrap();
    connection
        .execute_batch(&format!(
            "INSERT INTO meta VALUES ('schema_version', '{schema_version}');"
        ))
        .unwrap();
    connection.execute_batch(DECODER_ROWS).unwrap();
    connection
        .execute_batch(&format!(
            "UPDATE meta SET value = '{data_version}' WHERE key = 'data_version';"
        ))
        .unwrap();
    connection
        .execute_batch(wenmar_vehicles::schema::SCHEMA)
        .unwrap();
    connection.execute_batch(CATALOG_ROWS).unwrap();
    connection.close().unwrap();
}

/// An empty data directory.
pub fn empty_dir() -> Fixture {
    Fixture {
        directory: tempfile::tempdir().unwrap(),
    }
}

/// A data directory holding a data file of the current schema version.
pub fn data_dir() -> Fixture {
    let fixture = empty_dir();
    write_data_file(
        &fixture.file(),
        wenmar_vehicles::schema::SCHEMA_VERSION,
        "2026.09",
    );
    fixture
}

/// A data directory holding a data file of an older schema version.
pub fn old_data_dir() -> Fixture {
    let fixture = empty_dir();
    write_data_file(&fixture.file(), "2", "2026.03");
    fixture
}

/// An address on this computer that refuses connections. No test may
/// reach the real API.
pub const NO_API: &str = "http://127.0.0.1:1";

/// Surroundings with no terminal, no home directory, this data directory,
/// and no API to fall back on.
pub fn env(fixture: &Fixture) -> Env {
    Env {
        data_dir: Some(fixture.directory().to_path_buf()),
        api: Some(NO_API.to_owned()),
        releases: Some(NO_API.to_owned()),
        ..Env::default()
    }
}

/// What one invocation did.
pub struct Run {
    pub code: u8,
    pub stdout: String,
    pub stderr: String,
}

impl Run {
    /// Standard output as JSON.
    pub fn json(&self) -> Value {
        serde_json::from_str(&self.stdout)
            .unwrap_or_else(|error| panic!("{error}: {:?} / {:?}", self.stdout, self.stderr))
    }

    /// Standard error as JSON: `{ "error": { ... } }`.
    pub fn error(&self) -> Value {
        serde_json::from_str(&self.stderr)
            .unwrap_or_else(|error| panic!("{error}: {:?} / {:?}", self.stderr, self.stdout))
    }
}

/// Runs the tool in this process with the given standard input.
pub fn run_with_input(env: &Env, args: &[&str], input: &[u8]) -> Run {
    let mut args: Vec<OsString> = args.iter().map(OsString::from).collect();
    args.insert(0, OsString::from("wenmar-open"));
    let mut stdin = input;
    let (mut stdout, mut stderr) = (Vec::new(), Vec::new());
    let code = wenmar_open_cli::run(args, env, &mut stdin, &mut stdout, &mut stderr);
    Run {
        code,
        stdout: String::from_utf8(stdout).unwrap(),
        stderr: String::from_utf8(stderr).unwrap(),
    }
}

/// Runs the tool in this process.
pub fn run(env: &Env, args: &[&str]) -> Run {
    run_with_input(env, args, b"")
}
