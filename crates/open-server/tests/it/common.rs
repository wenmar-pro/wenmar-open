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
    /// A directory with no data file in it yet.
    ///
    /// Nothing of the server can run before a test has one of these, so
    /// this is where the test binary's log subscriber is set: see `capture`.
    fn empty() -> Fixture {
        crate::capture::install();
        Fixture {
            directory: tempfile::tempdir().unwrap(),
        }
    }

    pub fn path(&self) -> PathBuf {
        self.directory.path().join("data.sqlite3")
    }

    /// Where [`add_shop_file`] puts the shop file.
    pub fn shop_path(&self) -> PathBuf {
        self.directory.path().join("shop.sqlite3")
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
    let fixture = Fixture::empty();
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

/// How many makes, models and model years [`large_data_file`] adds.
pub const LARGE_MAKES: usize = 40;
pub const LARGE_MODELS: usize = LARGE_MAKES * 50;

/// A data file with a catalog large enough that reading every model year
/// takes far longer than looking a few of them up: the small catalog, and
/// beside it forty makes of fifty models each, in up to thirty model years.
///
/// Every make has the same fifty model names (`Atlas 0` to `Halo 4`), so a
/// name is shared by forty models and only the order settles which come
/// first. Every fourth make builds trailers. One model in ten has trims.
pub fn large_data_file() -> Fixture {
    use wenmar_vehicles::text::{normalize, slug};

    const STEMS: [&str; 10] = [
        "Atlas", "Arrow", "Bolt", "Civet", "Comet", "Delta", "Echo", "Falcon", "Gale", "Halo",
    ];
    let fixture = data_file();
    let mut connection = Connection::open(fixture.path()).unwrap();
    let transaction = connection.transaction().unwrap();
    let mut vehicle = 1_000;
    for make in 0..i64::try_from(LARGE_MAKES).unwrap() {
        let make_id = 7_000 + make;
        let name = format!("Maker {make:02}");
        let (types, light) = if make % 4 == 3 { (64, 0) } else { (8, 1) };
        transaction
            .execute(
                "INSERT INTO catalog_make VALUES (?1, ?2, ?3, ?4, NULL, ?5, ?6)",
                rusqlite::params![make_id, slug(&name), name, normalize(&name), types, light],
            )
            .unwrap();
        for model in 0..50_i64 {
            let model_id = 20_000 + make * 100 + model;
            let stem = STEMS[usize::try_from(model % 10).unwrap()];
            let name = format!("{stem} {}", model / 10);
            let (first, last) = (1990 + model % 7, 2019 - make % 5);
            transaction
                .execute(
                    "INSERT INTO catalog_model VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
                    rusqlite::params![
                        model_id,
                        make_id,
                        slug(&name),
                        name,
                        normalize(&name),
                        first,
                        last,
                        types,
                        light
                    ],
                )
                .unwrap();
            let detail = (model % 10 == 0).then_some(model_id);
            if let Some(detail) = detail {
                transaction
                    .execute(
                        "INSERT INTO catalog_detail VALUES (?1, NULL, NULL, NULL)",
                        [detail],
                    )
                    .unwrap();
                for (index, trim) in ["Base", "Sport", "Sport Plus", "1500"].iter().enumerate() {
                    transaction
                        .execute(
                            "INSERT INTO catalog_submodel VALUES
                               (?1, ?2, ?3, ?4, 'trim', 1, NULL, NULL, NULL)",
                            rusqlite::params![
                                detail * 10 + i64::try_from(index).unwrap(),
                                detail,
                                trim,
                                normalize(trim)
                            ],
                        )
                        .unwrap();
                }
            }
            for year in first..=last {
                vehicle += 1;
                transaction
                    .execute(
                        "INSERT INTO catalog_vehicle VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
                        rusqlite::params![vehicle, year, make_id, model_id, types, light, detail],
                    )
                    .unwrap();
            }
        }
    }
    transaction.commit().unwrap();
    connection.close().unwrap();
    fixture
}

/// A data file built before the catalog existed.
pub fn old_data_file() -> Fixture {
    build("2", false)
}

/// A SQLite file that is not a data file at all.
pub fn other_sqlite_file() -> Fixture {
    let fixture = Fixture::empty();
    let connection = Connection::open(fixture.path()).unwrap();
    connection
        .execute_batch("CREATE TABLE notes (body TEXT);")
        .unwrap();
    connection.close().unwrap();
    fixture
}

/// The `datasets` value of the fixture shop file: US and Canadian recalls.
pub const SHOP_DATASETS: &str = r#"{"recalls_us":{"as_of":"2026-10-11","rows":2,"licence":"public-domain","attribution":""},"recalls_ca":{"as_of":"2026-10-10","rows":1,"licence":"ogl-canada","attribution":"Contains information licensed under the Open Government Licence - Canada."}}"#;

/// Writes a shop file beside the fixture's data file. `rows` is SQL run
/// after `meta` is filled: a dataset's tables and their rows.
pub fn add_shop_file(
    fixture: &Fixture,
    schema_version: &str,
    catalog_data_version: &str,
    datasets: &str,
    rows: &str,
) {
    let connection = Connection::open(fixture.shop_path()).unwrap();
    connection
        .execute_batch(wenmar_shopdata::schema::META)
        .unwrap();
    for (key, value) in [
        ("schema_version", schema_version),
        ("data_version", "2026.10.12"),
        ("catalog_data_version", catalog_data_version),
        ("built_at", "2026-10-12 06:00:00"),
        ("datasets", datasets),
    ] {
        connection
            .execute(
                "INSERT INTO meta (key, value) VALUES (?1, ?2)",
                [key, value],
            )
            .unwrap();
    }
    connection.execute_batch(rows).unwrap();
    connection.close().unwrap();
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

// ----- the application under test -----

use axum::Router;
use axum::body::Body;
use axum::http::{Request, Response, StatusCode};
use http_body_util::BodyExt;
use open_server::config::Config;
use open_server::state::AppState;
use serde_json::Value;
use tower::ServiceExt;

/// The application over a fixture data file. Keep the fixture alive for as
/// long as the application is used.
pub struct TestApp {
    pub router: Router,
    /// What the handlers share, for tests that keep the data file busy.
    pub state: AppState,
    _fixture: Fixture,
}

pub fn config(fixture: &Fixture) -> Config {
    Config {
        data: fixture.path(),
        shop_data: None,
        port: 0,
        connections: 2,
        trusted_proxies: 0,
        requests_per_minute: 600,
        base_url: "https://open.example".to_owned(),
        feedback_email: None,
        github_repo: "wenmar-pro/wenmar-open".to_owned(),
        github_token: None,
        github_api: "https://api.github.com".to_owned(),
    }
}

/// [`config`] with the fixture's shop file named.
pub fn shop_config(fixture: &Fixture) -> Config {
    Config {
        shop_data: Some(fixture.shop_path()),
        ..config(fixture)
    }
}

async fn app_over(fixture: Fixture, change: impl FnOnce(&mut Config)) -> TestApp {
    let mut config = config(&fixture);
    change(&mut config);
    let state = AppState::open(config).await.unwrap();
    TestApp {
        router: open_server::app(state.clone()),
        state,
        _fixture: fixture,
    }
}

pub async fn app_with(change: impl FnOnce(&mut Config)) -> TestApp {
    app_over(data_file(), change).await
}

/// The application over the small data file with `rows` added to it, for a
/// test that needs something the small data file does not hold.
pub async fn app_with_rows(rows: &str) -> TestApp {
    let fixture = data_file();
    let connection = Connection::open(fixture.path()).unwrap();
    connection.execute_batch(rows).unwrap();
    connection.close().unwrap();
    app_over(fixture, |_| {}).await
}

pub async fn app() -> TestApp {
    app_with(|_| {}).await
}

/// The application over the small data file and a shop file built against
/// it, holding `datasets` and whatever `rows` creates.
pub async fn app_with_shop(datasets: &str, rows: &str) -> TestApp {
    let fixture = data_file();
    add_shop_file(
        &fixture,
        wenmar_shopdata::schema::SCHEMA_VERSION,
        "2026.09",
        datasets,
        rows,
    );
    let shop = fixture.shop_path();
    app_over(fixture, |config| config.shop_data = Some(shop)).await
}

impl TestApp {
    pub async fn send(&self, request: Request<Body>) -> Response<Body> {
        self.router.clone().oneshot(request).await.unwrap()
    }

    pub async fn get(&self, path: &str) -> Response<Body> {
        self.send(Request::get(path).body(Body::empty()).unwrap())
            .await
    }

    /// `GET`s a path and reads the body as JSON.
    pub async fn json(&self, path: &str) -> (StatusCode, Value) {
        let response = self.get(path).await;
        let status = response.status();
        (status, body_json(response).await)
    }

    pub async fn post_json(&self, path: &str, body: &str) -> Response<Body> {
        self.send(
            Request::post(path)
                .header("content-type", "application/json")
                .body(Body::from(body.to_owned()))
                .unwrap(),
        )
        .await
    }
}

pub async fn body_text(response: Response<Body>) -> String {
    let bytes = response.into_body().collect().await.unwrap().to_bytes();
    String::from_utf8(bytes.to_vec()).unwrap()
}

pub async fn body_json(response: Response<Body>) -> Value {
    let text = body_text(response).await;
    serde_json::from_str(&text).unwrap_or_else(|_| panic!("not JSON: {text}"))
}

pub fn header<'r>(response: &'r Response<Body>, name: &str) -> &'r str {
    response
        .headers()
        .get(name)
        .unwrap_or_else(|| panic!("no {name} header"))
        .to_str()
        .unwrap()
}

/// Waits until `condition` holds, for something the server does in its own
/// time. A fixed pause is not enough: on a busy machine the server can be
/// later than any pause a test would want to make every time.
pub async fn until(what: &str, condition: impl Fn() -> bool) {
    for _ in 0..10_000 {
        if condition() {
            return;
        }
        tokio::time::sleep(std::time::Duration::from_millis(2)).await;
    }
    panic!("{what}: not within 20 seconds");
}

/// Database work that does not finish until it is told to, for tests of
/// what the server does while the data file is busy.
#[derive(Clone, Default)]
pub struct Hold {
    released: std::sync::Arc<std::sync::atomic::AtomicBool>,
    started: std::sync::Arc<std::sync::atomic::AtomicUsize>,
}

impl Hold {
    /// Blocks the calling thread until `release`. Call it inside `Db::run`.
    pub fn wait(&self) {
        use std::sync::atomic::Ordering;
        self.started.fetch_add(1, Ordering::SeqCst);
        // A test that fails before `release` must still let its threads go.
        let give_up = std::time::Instant::now() + std::time::Duration::from_secs(20);
        while !self.released.load(Ordering::SeqCst) && std::time::Instant::now() < give_up {
            std::thread::sleep(std::time::Duration::from_millis(2));
        }
    }

    /// How many calls of `wait` have begun.
    pub fn started(&self) -> usize {
        self.started.load(std::sync::atomic::Ordering::SeqCst)
    }

    /// Waits until `count` calls of `wait` have begun.
    pub async fn until_started(&self, count: usize) {
        for _ in 0..2_000 {
            if self.started() >= count {
                return;
            }
            tokio::time::sleep(std::time::Duration::from_millis(2)).await;
        }
        panic!("only {} of {count} began", self.started());
    }

    pub fn release(&self) {
        self.released
            .store(true, std::sync::atomic::Ordering::SeqCst);
    }
}

// ----- recalls -----

/// The credit line the Canadian recall fixture carries.
pub const OGL_CANADA: &str =
    "Contains information licensed under the Open Government Licence – Canada.";

/// The `datasets` value of the recall fixture: both countries, then each
/// country alone.
pub const RECALL_DATASETS: &str = r#"{"recalls_us":{"as_of":"2026-10-11","rows":7,"licence":"public-domain","attribution":""},"recalls_ca":{"as_of":"2026-10-06","rows":2,"licence":"ogl-canada","attribution":"Contains information licensed under the Open Government Licence – Canada."}}"#;
pub const RECALL_DATASETS_US: &str =
    r#"{"recalls_us":{"as_of":"2026-10-11","rows":7,"licence":"public-domain","attribution":""}}"#;
pub const RECALL_DATASETS_CA: &str = r#"{"recalls_ca":{"as_of":"2026-10-06","rows":2,"licence":"ogl-canada","attribution":"Contains information licensed under the Open Government Licence – Canada."}}"#;

/// Nine campaigns against the small data file's catalog. The columns are
/// those of `wenmar_shopdata::schema::RECALLS`, in order. The Kona of
/// model year 2023, which [`KONA`] decodes to, has four: one with the "do
/// not drive" advisory, one with "park outside" and one Canadian. A
/// trailer has one, a campaign has HTML where its names should be, and one
/// old campaign lists a vehicle with no year that matched nothing.
pub const RECALL_INSERTS: &str = r#"
INSERT INTO recall VALUES
  (1, 'nhtsa', '19V888000', 'General Motors LLC', '2019-12-12', '2020-03-09', 'ELECTRICAL SYSTEM:12V/24V/48V BATTERY:CABLES', NULL, NULL, NULL, 'The battery cable nut may be loose.', 'A fire may start.', 'Dealers will tighten the nut.', NULL, NULL, 'N192273510', NULL, 350371, 0, 0),
  (2, 'nhtsa', '22V789000', 'Hyundai Motor America', '2022-11-20', '2023-01-15', 'SEAT BELTS:FRONT:BUCKLE ASSEMBLY', NULL, NULL, NULL, 'The seat belt buckle may not latch.', 'An occupant may not be restrained.', 'Dealers will replace the buckle.', NULL, NULL, '232', NULL, 1200, 0, 0),
  (3, 'nhtsa', '23V123000', 'Hyundai Motor America', '2023-03-02', '2023-04-20', 'ELECTRICAL SYSTEM:WIRING; EXTERIOR LIGHTING:HEADLIGHTS', NULL, NULL, NULL, 'A wiring harness may overheat.', 'A fire may start while the vehicle is parked.', 'Dealers will replace the harness.', 'Owners may call Hyundai.', NULL, '240', '91850-J9000, 91850-J9010', 5000, 0, 1),
  (4, 'nhtsa', '24V456000', 'Hyundai Motor America', '2024-05-10', '2024-06-30', 'SERVICE BRAKES, HYDRAULIC:ANTILOCK/TRACTION CONTROL/ELECTRONIC LIMITED SLIP', NULL, NULL, NULL, 'The brake control unit may fail.', 'Braking distance may increase.', 'Dealers will replace the unit.', NULL, NULL, '257', NULL, 800, 1, 0),
  (5, 'nhtsa', '19V500000', 'Ranger Trailer Works', '2019-07-01', NULL, 'TRAILER HITCHES', NULL, NULL, NULL, 'The hitch may crack.', 'The trailer may separate.', 'Dealers will replace the hitch.', NULL, NULL, NULL, NULL, 40, 0, 0),
  (6, 'nhtsa', '19V600000', 'B&B <script>alert(1)</script>', '2019-08-01', NULL, '<b>Bold</b> SYSTEM:PART', NULL, NULL, NULL, 'A part named <script>alert(1)</script> may fail.', 'It may fail <img src=x onerror=alert(1)>.', 'Dealers will replace it.', NULL, NULL, '"><img src=x onerror=alert(1)>', NULL, 7, 0, 0),
  (7, 'nhtsa', '68V023000', 'CHRYSLER CORPORATION', '1968-03-19', '1968-03-14', 'SERVICE BRAKES, HYDRAULIC:FOUNDATION COMPONENTS:HOSES, LINES/PIPING, AND FITTINGS', NULL, NULL, NULL, NULL, NULL, NULL, 'POSSIBILITY THE FRONT WHEEL FLEXIBLE BRAKE HOSE WAS IMPROPERLY INSTALLED.', NULL, NULL, NULL, 2800, 0, 0),
  (8, 'tc', '2019631', NULL, '2019-12-12', NULL, 'Electrical', 'Circuit électrique', 'Safety Mfr', 'Sécurité - fabricant', 'Issue:' || char(10) || 'The nut may be loose.', NULL, NULL, NULL, 'Problème :' || char(10) || 'Un écrou peut être desserré.', 'N192273510', NULL, 24289, NULL, NULL),
  (9, 'tc', '2023101', NULL, '2023-03-10', NULL, 'Electrical', 'Circuit électrique', 'Safety Mfr', 'Sécurité - fabricant', 'Issue:' || char(10) || 'A wiring harness may overheat.', NULL, NULL, NULL, 'Problème :' || char(10) || 'Un faisceau peut surchauffer.', '240', NULL, 410, NULL, NULL);
INSERT INTO recall_vehicle VALUES
  (1, 2019, 467, 1850, 'CHEVROLET', 'SILVERADO 1500', 'base', '2018-07-11', '2019-11-22'),
  (1, 2019, NULL, NULL, 'GMC', 'SIERRA 1500', NULL, '2018-07-11', '2019-11-22'),
  (2, 2022, 498, 900, 'HYUNDAI', 'KONA', 'exact', NULL, NULL),
  (2, 2023, 498, 900, 'HYUNDAI', 'KONA', 'exact', NULL, NULL),
  (3, 2023, 498, 900, 'HYUNDAI', 'KONA', 'exact', '2022-09-01', '2023-01-31'),
  (4, 2023, 498, 900, 'HYUNDAI', 'KONA ELECTRIC', 'base', NULL, NULL),
  (5, 2019, 5000, 9100, 'RANGER TRAILERS', 'TILT DECK', 'exact', NULL, NULL),
  (6, 2019, 6000, 9500, 'B&B <script>alert(1)</script>', 'Model "<img src=x onerror=alert(1)>', 'exact', NULL, NULL),
  (7, NULL, NULL, NULL, 'JEEP', 'WAGONEER', NULL, NULL, NULL),
  (8, 2019, 467, 1850, 'CHEVROLET', 'SILVERADO', 'exact', NULL, NULL),
  (9, 2023, 498, 900, 'HYUNDAI', 'KONA', 'exact', NULL, NULL);
"#;

/// The recall tables and [`RECALL_INSERTS`], as `rows` for
/// [`app_with_shop`].
pub fn recall_rows() -> String {
    format!("{}{RECALL_INSERTS}", wenmar_shopdata::schema::RECALLS)
}

/// The application over the small data file and a shop file holding the
/// recalls of both countries.
pub async fn recalls_app() -> TestApp {
    app_with_shop(RECALL_DATASETS, &recall_rows()).await
}

/// [`recalls_app`], saying its pages are at `base_url`.
pub async fn recalls_app_at(base_url: &str) -> TestApp {
    let fixture = data_file();
    add_shop_file(
        &fixture,
        wenmar_shopdata::schema::SCHEMA_VERSION,
        "2026.09",
        RECALL_DATASETS,
        &recall_rows(),
    );
    let shop = fixture.shop_path();
    let base_url = base_url.to_owned();
    app_over(fixture, |config| {
        config.shop_data = Some(shop);
        config.base_url = base_url;
    })
    .await
}

// ----- recall remedy progress -----

/// The `datasets` value of the recall fixture with the manufacturers'
/// reports of remedy progress beside the recalls of both countries.
pub const REPORT_DATASETS: &str = r#"{"recalls_us":{"as_of":"2026-10-11","rows":7,"licence":"public-domain","attribution":""},"recalls_ca":{"as_of":"2026-10-06","rows":2,"licence":"ogl-canada","attribution":"Contains information licensed under the Open Government Licence – Canada."},"recall_reports":{"as_of":"2026-10-12","rows":4,"licence":"public-domain","attribution":""}}"#;

/// Four reports. The columns are those of
/// `wenmar_shopdata::schema::RECALL_REPORTS`, in order. `19V888000` has
/// the figures of its real last report. The trailer's report gives no
/// vehicles involved and no day filed. The last row has the number of the
/// Canadian campaign of [`RECALL_INSERTS`]: reports are NHTSA's, so it
/// must never be shown.
pub const REPORT_INSERTS: &str = "
INSERT INTO recall_report VALUES
  ('19V500000', 'quarterly', 1, '2019-3', '2019-3', NULL, 0, 22, 0, 5),
  ('19V888000', 'quarterly', 6, '2021-2', '2021-2', '2021-07-19', 350371, 333125, 551, 46),
  ('2023101', 'quarterly', 1, '2023-2', '2023-2', '2023-07-01', 410, 400, 0, 0),
  ('24V456000', 'annual', 2, '2025-2', '2026-1', '2026-04-13', 800, 536, 12, 3);
";

/// [`recall_rows`], then the report table and [`REPORT_INSERTS`].
pub fn report_rows() -> String {
    format!(
        "{}{}{REPORT_INSERTS}",
        recall_rows(),
        wenmar_shopdata::schema::RECALL_REPORTS
    )
}

/// The application over the small data file and a shop file holding the
/// recalls of both countries and the reports of remedy progress.
pub async fn reports_app() -> TestApp {
    app_with_shop(REPORT_DATASETS, &report_rows()).await
}

/// [`reports_app`], saying its pages are at `base_url`, and the directory
/// holding its two files under the names the command-line tool looks for.
pub async fn reports_app_at(base_url: &str) -> (TestApp, TempDir) {
    let fixture = data_file();
    add_shop_file(
        &fixture,
        wenmar_shopdata::schema::SCHEMA_VERSION,
        "2026.09",
        REPORT_DATASETS,
        &report_rows(),
    );
    let files = tempfile::tempdir().unwrap();
    std::fs::copy(
        fixture.path(),
        files.path().join(wenmar_open_cli::env::DATA_FILE),
    )
    .unwrap();
    std::fs::copy(
        fixture.shop_path(),
        files.path().join(wenmar_open_cli::env::SHOP_FILE),
    )
    .unwrap();
    let shop = fixture.shop_path();
    let base_url = base_url.to_owned();
    let app = app_over(fixture, |config| {
        config.shop_data = Some(shop);
        config.base_url = base_url;
    })
    .await;
    (app, files)
}

// ----- crash test ratings -----

/// The `datasets` value of the ratings fixture: the ratings alone, then
/// with the recalls of both countries.
pub const RATING_DATASETS: &str =
    r#"{"ratings":{"as_of":"2026-08-07","rows":7,"licence":"public-domain","attribution":""}}"#;
pub const RECALL_AND_RATING_DATASETS: &str = r#"{"recalls_us":{"as_of":"2026-10-11","rows":7,"licence":"public-domain","attribution":""},"recalls_ca":{"as_of":"2026-10-06","rows":2,"licence":"ogl-canada","attribution":"Contains information licensed under the Open Government Licence – Canada."},"ratings":{"as_of":"2026-08-07","rows":7,"licence":"public-domain","attribution":""}}"#;

/// Seven ratings against the small data file's catalog. The columns are
/// those of `wenmar_shopdata::schema::RATINGS`, in order. The figures are
/// made up. The Kona of model year 2023 has three: two rated, by drive,
/// and one NHTSA lists and has not tested, reached through `KONA
/// ELECTRIC`. The 2022 Kona has a later release, the F-150 a rating from
/// before the evaluations, one rating has HTML where its words should be,
/// and one matched nothing. The 2019 Civic has none.
pub const RATING_INSERTS: &str = r#"
INSERT INTO rating VALUES
  (1, 2023, 'HYUNDAI', 'KONA', 'SUV', 'MPV', 'AWD', 1, 5, 4, 4, 4, 5, 5, 5, 5, 5, 5, 5, 4, 148, 0, 3225, NULL, 'standard', 'meets', 'standard', 'meets', 'standard', 'does_not_meet', 'standard', 'pending', NULL, 'optional', 'standard', 'standard', NULL),
  (2, 2023, 'HYUNDAI', 'KONA', 'SUV', 'MPV', 'FWD', 1, 4, 4, 4, 4, 5, 5, 5, 5, 5, 5, 5, 4, 155, 0, 3005, NULL, 'standard', 'meets', 'standard', 'meets', 'both', 'upgrade_only', 'both', 'standard_only', NULL, 'optional', 'standard', 'standard', 'These ratings do not apply to the N Line.' || char(10) || 'The rear door opened in the side test.'),
  (3, 2023, 'HYUNDAI', 'KONA ELECTRIC', 'SUV', 'MPV', 'FWD', 1, NULL, NULL, NULL, NULL, NULL, NULL, NULL, NULL, NULL, NULL, NULL, NULL, NULL, NULL, NULL, NULL, 'standard', 'pending', 'standard', 'pending', NULL, NULL, NULL, NULL, NULL, NULL, 'standard', 'standard', NULL),
  (4, 2022, 'HYUNDAI', 'KONA', 'SUV', 'MPV', 'FWD', 2, 5, 5, 5, 5, 5, 5, 5, 5, 5, 5, 5, 4, 150, 0, 2998, NULL, 'optional', NULL, 'optional', NULL, 'no', NULL, 'no', NULL, NULL, NULL, 'standard', 'standard', NULL),
  (5, 2019, 'FORD', 'F-150 SUPER CREW', 'PU/CC', 'TRUCK', '4x4', 1, 5, 5, 5, 5, 5, 5, 5, 5, 5, 5, 5, 4, 191, 0, 5060, NULL, 'optional', NULL, 'optional', NULL, 'optional', NULL, 'optional', NULL, NULL, 'optional', 'standard', 'standard', NULL),
  (6, 2019, 'B&B <script>alert(1)</script>', 'Model "<img src=x onerror=alert(1)>', '<b>Bold</b>', 'PC', NULL, 1, NULL, NULL, 4, 4, NULL, 4, NULL, NULL, NULL, NULL, NULL, 3, 200, 1, NULL, 'standard', NULL, NULL, NULL, NULL, NULL, NULL, NULL, NULL, NULL, NULL, NULL, NULL, 'A note with <script>alert(1)</script> in it.'),
  (7, 1999, 'TOYOTA', 'COROLLA', '4-DR W/SAB', 'LIGHT PASSENGER CAR', NULL, 1, NULL, NULL, 4, 4, NULL, 4, 3, NULL, NULL, NULL, NULL, NULL, NULL, NULL, NULL, 'no', 'no', NULL, 'no', NULL, 'no', NULL, 'no', NULL, NULL, NULL, NULL, 'optional', NULL);
INSERT INTO rating_vehicle VALUES
  (1, 2023, 498, 900, 'HYUNDAI', 'KONA', 'exact'),
  (2, 2023, 498, 900, 'HYUNDAI', 'KONA', 'exact'),
  (3, 2023, 498, 900, 'HYUNDAI', 'KONA ELECTRIC', 'base'),
  (4, 2022, 498, 900, 'HYUNDAI', 'KONA', 'exact'),
  (5, 2019, 460, 1801, 'FORD', 'F-150 SUPER CREW', 'base'),
  (6, 2019, 6000, 9500, 'B&B <script>alert(1)</script>', 'Model "<img src=x onerror=alert(1)>', 'exact'),
  (7, 1999, NULL, NULL, 'TOYOTA', 'COROLLA', NULL);
"#;

/// The rating tables and [`RATING_INSERTS`], as `rows` for
/// [`app_with_shop`].
pub fn rating_rows() -> String {
    format!("{}{RATING_INSERTS}", wenmar_shopdata::schema::RATINGS)
}

/// The application over the small data file and a shop file holding the
/// crash test ratings and nothing else.
pub async fn ratings_app() -> TestApp {
    app_with_shop(RATING_DATASETS, &rating_rows()).await
}

/// The same, with the recalls of both countries beside the ratings.
pub async fn ratings_and_recalls_app() -> TestApp {
    app_with_shop(
        RECALL_AND_RATING_DATASETS,
        &format!("{}{}", recall_rows(), rating_rows()),
    )
    .await
}

// ----- VIN guides -----

/// The `datasets` value of the VIN guide fixture.
pub const VIN_GUIDE_DATASETS: &str =
    r#"{"vin_guides":{"as_of":"2026-10-12","rows":16,"licence":"public-domain","attribution":""}}"#;

/// Sixteen filings against the small data file. The columns are those of
/// `wenmar_shopdata::schema::VIN_GUIDES`, in order. Manufacturer 1 is
/// Hyundai Motor Co, the manufacturer of code `KM8`, with twelve filings,
/// and manufacturer 4 also builds Hyundais, with one. Manufacturer 2 is
/// the trailer maker of code `1A9881`. Manufacturer 3 has a name that is
/// HTML and builds the make whose name is HTML. Filing 15 has an address
/// that is not NHTSA's copy of a filing, and must never be linked. The
/// document ids in the addresses are made up.
pub const VIN_GUIDE_INSERTS: &str = r#"
INSERT INTO vin_guide VALUES
  (1, '01-22-N11B-2001', 1, 'HYUNDAI MOTOR CO', '565', '1990-02-01', NULL, NULL, 'https://vpic.nhtsa.dot.gov/mid/home/displayfile/00000000-0000-4000-8000-000000000001'),
  (2, 'ORG20002', 1, 'HYUNDAI MOTOR CO', '565', '2022-01-10', 2023, 2023, 'https://vpic.nhtsa.dot.gov/mid/home/displayfile/00000000-0000-4000-8000-000000000002'),
  (3, 'ORG20003', 1, 'HYUNDAI MOTOR CO', '565', '2022-02-10', 2023, 2023, 'https://vpic.nhtsa.dot.gov/mid/home/displayfile/00000000-0000-4000-8000-000000000003'),
  (4, 'ORG20004', 1, 'HYUNDAI MOTOR CO', '565', '2022-03-10', 2023, 2023, 'https://vpic.nhtsa.dot.gov/mid/home/displayfile/00000000-0000-4000-8000-000000000004'),
  (5, 'ORG20005', 1, 'HYUNDAI MOTOR CO', '565', '2022-04-10', 2023, 2023, 'https://vpic.nhtsa.dot.gov/mid/home/displayfile/00000000-0000-4000-8000-000000000005'),
  (6, 'ORG20006', 1, 'HYUNDAI MOTOR CO', '565', '2022-05-10', 2023, 2023, 'https://vpic.nhtsa.dot.gov/mid/home/displayfile/00000000-0000-4000-8000-000000000006'),
  (7, 'ORG20007', 1, 'HYUNDAI MOTOR CO', '565', '2023-01-10', 2024, 2024, 'https://vpic.nhtsa.dot.gov/mid/home/displayfile/00000000-0000-4000-8000-000000000007'),
  (8, 'ORG20008', 1, 'HYUNDAI MOTOR CO', '565', '2023-02-10', 2024, 2024, 'https://vpic.nhtsa.dot.gov/mid/home/displayfile/00000000-0000-4000-8000-000000000008'),
  (9, 'ORG20009', 1, 'HYUNDAI MOTOR CO', '565', '2024-01-10', 2025, 2025, 'https://vpic.nhtsa.dot.gov/mid/home/displayfile/00000000-0000-4000-8000-000000000009'),
  (10, 'ORG20010', 1, 'HYUNDAI MOTOR CO', '565', '2025-01-10', 2026, 2026, 'https://vpic.nhtsa.dot.gov/mid/home/displayfile/00000000-0000-4000-8000-000000000010'),
  (11, 'ORG20011', 1, 'HYUNDAI MOTOR CO', '565', '2026-01-10', 2026, 2028, 'https://vpic.nhtsa.dot.gov/mid/home/displayfile/00000000-0000-4000-8000-000000000011'),
  (12, 'ORG20012', 1, 'HYUNDAI MOTOR CO', '565', '2026-06-01', 2027, NULL, 'https://vpic.nhtsa.dot.gov/mid/home/displayfile/00000000-0000-4000-8000-000000000012'),
  (13, 'ORG20013', 4, 'HYUNDAI MOTOR MANUFACTURING ALABAMA LLC (HMMA)', '565', '2026-03-03', 2027, 2027, 'https://vpic.nhtsa.dot.gov/mid/home/displayfile/00000000-0000-4000-8000-000000000013'),
  (14, 'ORG20014', 2, 'RANGER TRAILER WORKS', '565', '2019-05-05', NULL, 2020, 'https://vpic.nhtsa.dot.gov/mid/home/displayfile/00000000-0000-4000-8000-000000000014'),
  (15, 'ORG20015', 2, 'RANGER TRAILER WORKS', '565', '2020-05-05', NULL, NULL, 'https://example.com/"><script>alert(1)</script>'),
  (16, '<b>Bold</b>', 3, 'B&B <script>alert(1)</script>', '565', '2019-08-01', NULL, NULL, 'https://vpic.nhtsa.dot.gov/mid/home/displayfile/00000000-0000-4000-8000-000000000016');
INSERT INTO vin_guide_wmi VALUES ('KM8', 1), ('1A9881', 2);
INSERT INTO vin_guide_make VALUES (498, 1), (498, 4), (5000, 2), (6000, 3);
"#;

/// The VIN guide tables and [`VIN_GUIDE_INSERTS`], as `rows` for
/// [`app_with_shop`].
pub fn vin_guide_rows() -> String {
    format!("{}{VIN_GUIDE_INSERTS}", wenmar_shopdata::schema::VIN_GUIDES)
}

/// The application over the small data file and a shop file holding the
/// VIN guide list.
pub async fn vin_guides_app() -> TestApp {
    app_with_shop(VIN_GUIDE_DATASETS, &vin_guide_rows()).await
}

/// [`ratings_and_recalls_app`], saying its pages are at `base_url`.
pub async fn ratings_and_recalls_app_at(base_url: &str) -> TestApp {
    let fixture = data_file();
    add_shop_file(
        &fixture,
        wenmar_shopdata::schema::SCHEMA_VERSION,
        "2026.09",
        RECALL_AND_RATING_DATASETS,
        &format!("{}{}", recall_rows(), rating_rows()),
    );
    let shop = fixture.shop_path();
    let base_url = base_url.to_owned();
    app_over(fixture, |config| {
        config.shop_data = Some(shop);
        config.base_url = base_url;
    })
    .await
}

// ----- bulletins -----

/// The `datasets` value of the bulletin fixture: both countries' recalls
/// and the bulletins.
pub const BULLETIN_DATASETS: &str = r#"{"recalls_us":{"as_of":"2026-10-11","rows":7,"licence":"public-domain","attribution":""},"recalls_ca":{"as_of":"2026-10-06","rows":2,"licence":"ogl-canada","attribution":"Contains information licensed under the Open Government Licence – Canada."},"bulletins":{"as_of":"2026-10-11","rows":10,"licence":"public-domain","attribution":""}}"#;

/// Ten bulletins against the small data file's catalog, shaped as
/// `open-data build-shop` stores them; the text is made up. The Kona of
/// model year 2023, which [`KONA`] decodes to, has six: a warranty
/// extension, an over-the-air update, one about maintenance, one that
/// names two trouble codes, and one with HTML where its text should be.
/// 11000002 reaches the Kona through two names. 10100001 is too old for
/// a document address; 11000005 has no manufacturer's date.
pub const BULLETIN_INSERTS: &str = r#"
INSERT INTO bulletin VALUES
  (11000001, 'service_bulletin', '2025-03-10', '2025-03-01', 'TSB 25-01-012', NULL, NULL, 'ENGINE AND ENGINE COOLING:ENGINE; ELECTRICAL SYSTEM:SOFTWARE', NULL, 'Some 2022-2023 Kona vehicles may set DTC P0420 and P0300 with a rough idle. Update the ECM software.'),
  (11000002, 'warranty_extension', '2025-06-02', '2025-05-20', 'WX-25-003', 'WX003', NULL, 'POWER TRAIN:AUTOMATIC TRANSMISSION', NULL, 'Warranty extension: the transmission control module is covered for 10 years.'),
  (11000003, 'over_the_air', '2026-01-15', '2026-01-10', 'OTA 26.1', '26.1.4', NULL, 'ELECTRICAL SYSTEM:SOFTWARE', 'Infotainment: Head Unit', 'Infotainment update: improved navigation and voice control.'),
  (11000004, 'service_bulletin', '2024-02-01', '2024-01-20', 'TSB 24-BR-001', NULL, NULL, 'SERVICE BRAKES, HYDRAULIC:FOUNDATION COMPONENTS:DISC:PADS', NULL, 'Brake squeal noise at low speed. Replace the front brake pads.'),
  (11000005, 'service_bulletin', '2024-02-01', NULL, NULL, NULL, NULL, 'UNKNOWN OR OTHER', NULL, 'Maintenance schedule update: the oil change service interval is now 8,000 miles.'),
  (11000006, 'emissions', '2023-07-07', '2023-07-01', 'EM-23', NULL, NULL, 'ENGINE AND ENGINE COOLING:EXHAUST SYSTEM:EMISSION CONTROL:CATALYTIC CONVERTOR', NULL, 'Emissions recall: catalytic converter. DTC P0420 may set.'),
  (11000007, 'service_campaign', '2026-04-23', '2026-04-23', 'N262548050', 'N262548050', NULL, 'ENGINE', NULL, 'Certain vehicles may have a condition that could cause the catalytic converter to fail.'),
  (10100001, 'service_bulletin', '2017-03-01', '2017-02-20', 'sb2652', NULL, '95-011', 'EQUIPMENT', NULL, 'DT12 PTO Upfit'),
  (11000009, 'other', '2022-09-22', '2022-09-16', 'A2090B', NULL, NULL, 'VISIBILITY', NULL, 'Dealer message: washer pumps were 100% tested.'),
  (11000010, 'service_bulletin', '2025-03-10', '2025-03-02', '"><img src=x onerror=alert(1)>', NULL, NULL, '<b>Bold</b> SYSTEM:PART', NULL, 'A part named <script>alert(1)</script> may fail.');
INSERT INTO bulletin_model VALUES
  (1, 'HYUNDAI', 'KONA', 498, 900, 'exact'),
  (2, 'HYUNDAI', 'KONA ELECTRIC', 498, 900, 'base'),
  (3, 'HYUNDAI', 'KONA', 498, NULL, NULL),
  (4, 'CHEVROLET', 'SILVERADO 1500', 467, 1850, 'base'),
  (5, 'GMC', 'SIERRA 1500', NULL, NULL, NULL),
  (6, 'FREIGHTLINER', 'CASCADIA', NULL, NULL, NULL),
  (7, 'HYUNDAI', 'HYUNDAI', 498, NULL, NULL);
INSERT INTO bulletin_vehicle VALUES
  (11000001, 1, 2022, 2023),
  (11000002, 1, 2023, 2023),
  (11000002, 2, 2023, 2023),
  (11000003, 1, 2023, 2023),
  (11000003, 3, 2024, 2025),
  (11000004, 1, 2023, 2023),
  (11000005, 1, 2023, 2023),
  (11000006, 1, 2022, 2022),
  (11000006, 4, 2019, 2019),
  (11000007, 4, 2019, 2020),
  (11000007, 5, 2019, 2020),
  (10100001, 6, 2020, 2020),
  (11000009, 7, 0, 0),
  (11000010, 1, 2023, 2023);
INSERT INTO component VALUES
  ('brakes', 'Brakes', NULL),
  ('brake-pad', 'Brake pads', 'brakes'),
  ('electrical', 'Electrical system', NULL),
  ('software', 'Software', 'electrical'),
  ('engine', 'Engine', NULL),
  ('exhaust-emissions', 'Exhaust and emissions', NULL),
  ('catalytic-converter', 'Catalytic converter', 'exhaust-emissions'),
  ('powertrain', 'Transmission and driveline', NULL),
  ('automatic-transmission', 'Automatic transmission', 'powertrain'),
  ('visibility', 'Glass, wipers and mirrors', NULL),
  ('equipment', 'Equipment and accessories', NULL);
INSERT INTO bulletin_component VALUES
  (11000001, 'engine'), (11000001, 'electrical'), (11000001, 'software'),
  (11000002, 'powertrain'), (11000002, 'automatic-transmission'),
  (11000003, 'electrical'), (11000003, 'software'),
  (11000004, 'brakes'), (11000004, 'brake-pad'),
  (11000006, 'engine'), (11000006, 'exhaust-emissions'), (11000006, 'catalytic-converter'),
  (11000007, 'engine'),
  (10100001, 'equipment'),
  (11000009, 'visibility');
INSERT INTO bulletin_code VALUES
  (11000001, 'P0300'), (11000001, 'P0420'), (11000006, 'P0420');
INSERT INTO bulletin_topic VALUES
  (11000005, 'maintenance');
INSERT INTO bulletin_year (year)
  WITH RECURSIVE years(year) AS (SELECT 1900 UNION ALL SELECT year + 1 FROM years WHERE year < 2100)
  SELECT year FROM years;
"#;

/// The recall tables and rows of [`recall_rows`], then the bulletin
/// tables and [`BULLETIN_INSERTS`], as `rows` for [`app_with_shop`].
pub fn bulletin_rows() -> String {
    use wenmar_shopdata::schema;
    format!(
        "{}{}{}{}{}{BULLETIN_INSERTS}",
        recall_rows(),
        schema::BULLETINS,
        schema::COMPONENTS,
        schema::BULLETIN_COMPONENTS,
        schema::BULLETIN_TEXT
    )
}

/// The application over the small data file and a shop file holding
/// recalls and bulletins.
pub async fn bulletins_app() -> TestApp {
    app_with_shop(BULLETIN_DATASETS, &bulletin_rows()).await
}

/// [`bulletin_rows`] with the components of four recall campaigns and one
/// sibling row: the 2023 Kona (model 900) shares 7 of its 20 documents
/// with the Silverado (model 1850).
pub fn cross_link_rows() -> String {
    use wenmar_shopdata::schema;
    format!(
        "{}{}{}
INSERT INTO recall_component VALUES
  (1, 'electrical'), (3, 'electrical'), (4, 'brakes'), (2, 'equipment');
INSERT INTO vehicle_sibling VALUES (900, 2023, 1850, 7, 20);
",
        bulletin_rows(),
        schema::RECALL_COMPONENTS,
        schema::SIBLINGS
    )
}

/// The application over recalls, bulletins, their components and the
/// siblings.
pub async fn cross_links_app() -> TestApp {
    app_with_shop(BULLETIN_DATASETS, &cross_link_rows()).await
}

// ----- helpers for the pages -----

/// `GET`s a page and reads its body as text.
pub async fn page(app: &TestApp, path: &str) -> (StatusCode, String) {
    let response = app.get(path).await;
    let status = response.status();
    (status, body_text(response).await)
}

/// `GET`s an address that redirects, and says where to.
pub async fn redirect(app: &TestApp, path: &str) -> (StatusCode, String) {
    let response = app.get(path).await;
    let to = response
        .headers()
        .get("location")
        .map(|value| value.to_str().unwrap().to_owned())
        .unwrap_or_default();
    (response.status(), to)
}

/// Nothing that came from the data file or from the address may arrive as
/// markup. The fixture holds a make, a model, a trim and a VIN pattern
/// whose names are HTML.
pub fn assert_no_injection(html: &str) {
    for needle in [
        "<script>alert",
        "<img src=x",
        "onerror=alert(1)>",
        "<b>Bold",
    ] {
        assert!(!html.contains(needle), "found {needle:?} in:\n{html}");
    }
}

/// What every page must have for a screen reader and a keyboard.
pub fn assert_basics(html: &str, path: &str) {
    assert!(
        html.starts_with("<!doctype html>\n<html lang=\"en\">"),
        "{path}"
    );
    assert!(
        html.contains(r#"<meta name="viewport" content="width=device-width, initial-scale=1">"#),
        "{path}"
    );
    assert!(
        html.contains(r##"<a class="skip" href="#main">Skip to content</a>"##),
        "{path}"
    );
    assert!(html.contains(r#"<main id="main">"#), "{path}");
    assert!(html.contains(r#"<nav aria-label="Site">"#), "{path}");
    assert_eq!(html.matches("<h1").count(), 1, "{path}: one main heading");
    assert!(
        html.contains("<title>") && !html.contains("<title></title>"),
        "{path}"
    );
    // Every field has a label that names it.
    for field in html
        .split("<input ")
        .skip(1)
        .chain(html.split("<select ").skip(1))
    {
        let id = field
            .split("id=\"")
            .nth(1)
            .unwrap()
            .split('"')
            .next()
            .unwrap();
        assert!(
            html.contains(&format!("<label for=\"{id}\">")),
            "{path}: no label for {id}"
        );
    }
    // A table says what its rows are, with a class or without one.
    assert_eq!(
        html.matches("<table").count(),
        html.matches("<caption>").count(),
        "{path}: every table has a caption"
    );
}

/// `GET`s the Markdown version of a page, checks it is marked as a copy of
/// the HTML page and that the HTML page points to it, and returns its text.
pub async fn markdown(app: &TestApp, path: &str) -> String {
    let response = app.get(path).await;
    assert_eq!(response.status(), StatusCode::OK, "{path}");
    assert_eq!(
        header(&response, "content-type"),
        "text/markdown; charset=utf-8",
        "{path}"
    );
    // The HTML page is the one to index.
    assert_eq!(header(&response, "x-robots-tag"), "noindex", "{path}");
    let html_path = path.split(".md").next().unwrap().to_owned();
    assert_eq!(
        header(&response, "link"),
        format!("<https://open.example{html_path}>; rel=\"canonical\""),
        "{path}"
    );
    let text = body_text(response).await;
    assert!(!text.contains("<h1>"), "{path}");

    let (status, html) = page(app, &html_path).await;
    assert_eq!(status, StatusCode::OK, "{html_path}");
    assert!(
        html.contains(&format!(
            r#"<link rel="alternate" type="text/markdown" href="{html_path}.md">"#
        )),
        "{html_path}"
    );
    text
}

/// What the head of every page has for someone who shares the page, and
/// the wordmark every page starts with.
pub fn assert_head(html: &str, path: &str) {
    assert!(
        html.contains(
            r#"<a class="name" href="/" aria-label="Wenmar Open, home">Wenmar<span>Open</span></a>"#
        ),
        "{path}: the wordmark"
    );
    let title = html
        .split("<title>")
        .nth(1)
        .and_then(|rest| rest.split("</title>").next())
        .unwrap_or_else(|| panic!("{path}: no title"));
    assert!(!title.is_empty(), "{path}");
    // The title a search engine and a chat application are given is one.
    assert!(
        html.contains(&format!(r#"<meta property="og:title" content="{title}">"#)),
        "{path}: og:title"
    );
    for tag in [
        r#"<meta property="og:site_name" content="Wenmar Open">"#,
        r#"<meta property="og:type" content=""#,
        r#"<meta property="og:description" content=""#,
        r#"<meta property="og:image" content="https://open.example/assets/og.png">"#,
        r#"<meta property="og:image:width" content="1200">"#,
        r#"<meta property="og:image:height" content="630">"#,
        r#"<meta property="og:image:alt" content="Wenmar Open: free VIN decoder and vehicle data">"#,
        r#"<meta name="twitter:card" content="summary_large_image">"#,
        r#"<link rel="preload" href="/assets/fonts/dm-sans-latin-wght.woff2" as="font" type="font/woff2" crossorigin>"#,
    ] {
        assert!(html.contains(tag), "{path}: {tag}");
    }
    // A page says where it lives to a chat application exactly when it says
    // so to a search engine.
    let canonical = html
        .split(r#"<link rel="canonical" href=""#)
        .nth(1)
        .and_then(|rest| rest.split('"').next());
    match canonical {
        Some(address) => assert!(
            html.contains(&format!(r#"<meta property="og:url" content="{address}">"#)),
            "{path}: og:url"
        ),
        None => assert!(
            !html.contains("og:url"),
            "{path}: og:url without a canonical"
        ),
    }
    assert!(
        html.matches(r#"class="primary""#).count() <= 1,
        "{path}: more than one red action"
    );
}

/// Anything that can be tapped is at least 44 pixels tall. A link inside a
/// sentence is exempt. A link that stands alone is not, so it sits in
/// something the stylesheet gives the height to: a paragraph with class
/// `more`, or a list with class `plain`.
pub fn assert_targets(html: &str, path: &str) {
    assert!(
        !html.contains("<p><a "),
        "{path}: a paragraph that is one link needs class \"more\""
    );
    assert!(
        !html.contains("<ul>"),
        "{path}: a list of links needs class \"plain\""
    );
}

/// The structured data in a page's head, parsed, or `None` when the page
/// has none. Fails if the block could have ended its script element early
/// or is not JSON.
pub fn json_ld(html: &str) -> Option<Value> {
    let block = html
        .split(r#"<script type="application/ld+json">"#)
        .nth(1)?
        .split("</script>")
        .next()?;
    assert!(
        !block.contains('<') && !block.contains('>') && !block.contains('&'),
        "structured data holds markup characters: {block}"
    );
    Some(serde_json::from_str(block).unwrap_or_else(|error| panic!("{error}: {block}")))
}

// ----- investigations -----

/// The `datasets` value of the investigation fixture: both countries'
/// recalls and the investigations, then the investigations alone.
pub const INVESTIGATION_DATASETS: &str = r#"{"recalls_us":{"as_of":"2026-10-11","rows":7,"licence":"public-domain","attribution":""},"recalls_ca":{"as_of":"2026-10-06","rows":2,"licence":"ogl-canada","attribution":"Contains information licensed under the Open Government Licence – Canada."},"investigations":{"as_of":"2026-10-12","rows":9,"licence":"public-domain","attribution":""}}"#;
pub const INVESTIGATION_DATASETS_ALONE: &str = r#"{"investigations":{"as_of":"2026-10-12","rows":9,"licence":"public-domain","attribution":""}}"#;

/// Nine investigations against the small data file's catalog. The columns
/// are those of `wenmar_shopdata::schema::INVESTIGATIONS`, in order; the
/// numbers and texts are made up. The Kona of model year 2023 is listed by
/// four: two open ones about the vehicle, an open audit query, and a
/// closed one. Campaign `19V888000` of [`RECALL_INSERTS`] followed two,
/// `24V456000` and `23V123000` one each. One has HTML where its texts
/// should be.
pub const INVESTIGATION_INSERTS: &str = r#"
INSERT INTO investigation VALUES
  (1, 'PE25010', 'PE', '2025-09-15', NULL, 1, 'Hyundai Motor America', 'ENGINE; FUEL SYSTEM, GASOLINE', 'Loss of motive power', 'The engine may stall while driving.'),
  (2, 'EA24003', 'EA', '2024-08-01', NULL, 1, 'Hyundai Motor America', 'SERVICE BRAKES, HYDRAULIC', 'Brake control unit failure', 'The unit may fail after the recall repair.'),
  (3, 'AQ22002', 'AQ', '2022-06-03', NULL, 1, 'Hyundai Motor America', NULL, 'Dealer sale of vehicles with open recalls', 'A dealer sold them.'),
  (4, 'PE22001', 'PE', '2022-06-01', '2022-12-01', 0, 'Hyundai Motor America', 'ELECTRICAL SYSTEM:WIRING', 'Wiring harness overheating', 'A harness may overheat.'),
  (5, 'RQ98022', 'RQ', '1998-11-13', NULL, 0, 'General Motors, LLC', 'SUSPENSION', 'REAR SUSPENSION SPRING BREAKAGE', NULL),
  (6, 'PE19004', 'PE', '2019-02-11', '2019-06-20', 0, 'General Motors, LLC', 'ELECTRICAL SYSTEM:12V/24V/48V BATTERY:CABLES', 'Battery cable connection', 'The cable nut may be loose.'),
  (7, 'EA19009', 'EA', '2019-06-20', '2019-12-20', 0, 'General Motors, LLC', 'ELECTRICAL SYSTEM:12V/24V/48V BATTERY:CABLES', 'Battery cable connection', NULL),
  (8, 'DP19001', 'DP', '2019-03-01', NULL, 1, 'B&B <script>alert(1)</script>', '<b>Bold</b> SYSTEM', 'A part named <script>alert(1)</script>', 'It may fail <img src=x onerror=alert(1)>.'),
  (9, 'ID94001', 'ID', NULL, NULL, 0, NULL, NULL, NULL, NULL);
INSERT INTO investigation_vehicle VALUES
  (1, 2023, 498, 900, 'HYUNDAI', 'KONA', 'exact'),
  (1, 2023, 498, 900, 'HYUNDAI', 'KONA ELECTRIC', 'base'),
  (2, 2023, 498, 900, 'HYUNDAI', 'KONA', 'exact'),
  (3, 2023, 498, 900, 'HYUNDAI', 'KONA', 'exact'),
  (4, 2022, 498, 900, 'HYUNDAI', 'KONA', 'exact'),
  (4, 2023, 498, 900, 'HYUNDAI', 'KONA', 'exact'),
  (5, 2019, 467, 1850, 'CHEVROLET', 'SILVERADO', 'exact'),
  (6, 2019, 467, 1850, 'CHEVROLET', 'SILVERADO 1500', 'base'),
  (6, 2019, NULL, NULL, 'GMC', 'SIERRA 1500', NULL),
  (7, 2019, 467, 1850, 'CHEVROLET', 'SILVERADO 1500', 'base'),
  (8, 2019, 6000, 9500, 'B&B <script>alert(1)</script>', 'Model "<img src=x onerror=alert(1)>', 'exact');
INSERT INTO investigation_campaign VALUES
  (2, '24V456000'),
  (4, '23V123000'),
  (6, '19V888000'),
  (7, '19V888000'),
  (8, '19V600000'),
  (9, '94E013000');
"#;

/// The recall tables and rows, then the investigation tables and
/// [`INVESTIGATION_INSERTS`], as `rows` for [`app_with_shop`].
pub fn investigation_rows() -> String {
    format!(
        "{}{}{INVESTIGATION_INSERTS}",
        recall_rows(),
        wenmar_shopdata::schema::INVESTIGATIONS
    )
}

/// The application over the small data file and a shop file holding the
/// recalls of both countries and the investigations.
pub async fn investigations_app() -> TestApp {
    app_with_shop(INVESTIGATION_DATASETS, &investigation_rows()).await
}

/// A datasets JSON with the four fuel datasets.
pub const FUEL_DATASETS: &str = r#"{"fuel_us":{"as_of":"2026-09-29","rows":5,"licence":"public-domain","attribution":""},"fuel_ca":{"as_of":"2026-10-06","rows":1,"licence":"ogl-canada","attribution":"Contains information licensed under the Open Government Licence – Canada."},"specs":{"as_of":"2026-09-29","rows":5,"licence":"public-domain","attribution":""},"oil_grade":{"as_of":"2026-09-29","rows":3,"licence":"public-domain","attribution":""}}"#;

/// EPA's ratings alone: no Canadian ratings, no specifications, no oil.
pub const FUEL_DATASETS_US: &str =
    r#"{"fuel_us":{"as_of":"2026-09-29","rows":5,"licence":"public-domain","attribution":""}}"#;

/// Ratings against the small data file's catalog. The columns are those
/// of `wenmar_shopdata::schema::FUEL`, in order. Every figure is made up
/// for the test; none is a real rating. The Kona of model year 2023 (make
/// 498, model 900), which [`KONA`] decodes to with the 2.0L engine
/// (catalog engine 8), has two ratings on that engine, one on the 1.6L
/// turbo (engine 9), one electric rating matched to the model year only,
/// and one Canadian rating. The 2019 Civic has one, matched to no engine.
pub const FUEL_INSERTS: &str = r#"
INSERT INTO fuel_rating VALUES
  (1, 'epa', 46001, 2023, 498, 900, 'Hyundai', 'Kona FWD', 'exact', 8, NULL, 'engine', 'FWD', NULL, NULL, 2.0, 4, 'Automatic (variable gear ratios)', 'Front-Wheel Drive', 'Regular Gasoline', NULL, 7.8, 6.7, 7.4, 30, 35, 32, NULL, NULL, 172),
  (2, 'epa', 46002, 2023, 498, 900, 'Hyundai', 'Kona AWD', 'exact', 8, NULL, 'engine', 'AWD', NULL, NULL, 2.0, 4, 'Automatic (variable gear ratios)', 'All-Wheel Drive', 'Regular Gasoline', NULL, 8.4, 7.1, 7.8, 28, 33, 30, NULL, NULL, 184),
  (3, 'epa', 46003, 2023, 498, 900, 'Hyundai', 'Kona N Line AWD', 'base', 9, NULL, 'engine', 'AWD', NULL, NULL, 1.6, 4, 'Automatic (AM7)', 'All-Wheel Drive', 'Regular Gasoline', NULL, 8.7, 7.4, 8.1, 27, 32, 29, NULL, NULL, 190),
  (4, 'epa', 46004, 2023, 498, 900, 'Hyundai', 'Kona Electric', 'base', NULL, NULL, 'model', NULL, NULL, NULL, NULL, NULL, 'Automatic (A1)', 'Front-Wheel Drive', 'Electricity', NULL, NULL, NULL, NULL, NULL, NULL, NULL, 17.4, 415, 0),
  (5, 'nrcan', NULL, 2023, 498, 900, 'Hyundai', 'Kona', 'exact', 8, NULL, 'engine', NULL, NULL, NULL, 2.0, 4, 'AV', NULL, 'Regular Gasoline', NULL, 7.9, 6.6, 7.3, 30, 36, 32, NULL, NULL, 171),
  (6, 'epa', 46010, 2019, 474, 1863, 'Honda', 'Civic 4Dr', 'base', NULL, NULL, 'model', NULL, NULL, NULL, 1.8, 4, 'Manual 6-spd', 'Front-Wheel Drive', 'Regular Gasoline', NULL, 9.4, 6.7, 8.1, 25, 35, 29, NULL, NULL, 189);
INSERT INTO vehicle_spec VALUES
  (46001, 'Small Sport Utility Vehicle 2WD', 0, 0, 1, NULL, NULL, NULL, NULL, 94, NULL, NULL, 19, NULL, 1, 'Continuously Variable', 'Gasoline (Regular Unleaded Recommended)', 0, NULL, 1, 0, 2, 2, NULL, NULL, NULL),
  (46002, 'Small Sport Utility Vehicle 4WD', 0, 0, 1, NULL, NULL, NULL, NULL, 94, NULL, NULL, 19, NULL, 1, 'Continuously Variable', 'Gasoline (Regular Unleaded Recommended)', 0, NULL, 1, 0, 2, 2, NULL, NULL, NULL),
  (46003, 'Small Sport Utility Vehicle 4WD', 1, 0, 1, NULL, NULL, NULL, NULL, 94, NULL, NULL, 19, NULL, 8, 'Automatic', 'Gasoline (Regular Unleaded Recommended)', 1, 'Half Engine Mode', 1, 0, 2, 2, NULL, NULL, NULL),
  (46004, 'Small Sport Utility Vehicle 2WD', 0, 0, NULL, 'EV', '150 kW AC 3-Phase', NULL, NULL, 94, NULL, NULL, 19, NULL, NULL, NULL, NULL, NULL, NULL, NULL, NULL, NULL, NULL, 'Lithium Ion', 400, '150');
INSERT INTO emission_standard VALUES
  (46001, 3, 'T3B30', 'Federal Tier 3 Bin 30', 7, 'PHYXV02.0AAA'),
  (46001, 7, 'L3SULEV30', 'California LEV-III SULEV30', 7, 'PHYXV02.0AAA');
INSERT INTO oil_grade VALUES
  (46001, '0W-20', '0W-20'),
  (46002, '0W-20', 'SAE 0W-20'),
  (46003, '5W-30, 0W-30', '5W-30/0W-30'),
  (46010, '0W-20', '0W20');
"#;

/// The fuel tables with [`FUEL_INSERTS`], as `rows` for [`app_with_shop`].
pub fn fuel_rows() -> String {
    format!("{}{FUEL_INSERTS}", wenmar_shopdata::schema::FUEL)
}

/// The application over the small data file and a shop file holding fuel
/// ratings of both countries, specifications and oil grades.
pub async fn fuel_app() -> TestApp {
    app_with_shop(FUEL_DATASETS, &fuel_rows()).await
}
