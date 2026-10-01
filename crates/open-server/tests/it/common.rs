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
        port: 0,
        connections: 2,
        trusted_proxies: 0,
        requests_per_minute: 600,
        base_url: "https://open.example".to_owned(),
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
