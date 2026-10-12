//! The recall commands over the shop file, and `data pull --shop`.

use std::io::Write;

use flate2::Compression;
use flate2::write::GzEncoder;
use serde_json::{Value, json};
use wenmar_open_cli::env::Env;

use crate::common::{self, Fixture, KONA};
use crate::server::{self, Reply, Seen, Server};

const SITE: &str = "https://open.wenmarpro.com";

fn offline(fixture: &Fixture, args: &[&str]) -> common::Run {
    let mut args = args.to_vec();
    args.push("--offline");
    common::run(&common::env(fixture), &args)
}

#[test]
fn the_recall_commands_answer_from_the_shop_file_as_the_api_words_it() {
    let fixture = common::shop_dir();

    let run = offline(&fixture, &["recalls", "vehicle", "2023_hyundai_kona"]);
    assert_eq!(run.code, 0, "{}", run.stderr);
    let answer = run.json();
    assert_eq!(
        answer["vehicle"],
        json!({ "id": "2023_hyundai_kona", "year": 2023, "make": "Hyundai", "model": "Kona" })
    );
    assert_eq!(answer["may_apply"], true);
    assert_eq!(
        answer["as_of"],
        json!({ "us": "2026-10-11", "ca": "2026-10-06" })
    );
    let campaigns: Vec<&str> = answer["recalls"]
        .as_array()
        .unwrap()
        .iter()
        .map(|recall| recall["campaign"].as_str().unwrap())
        .collect();
    assert_eq!(campaigns, ["24V456000", "2023101", "23V123000"]);
    assert_eq!(
        answer["recalls"][2],
        json!({
            "source": "nhtsa", "country": "us", "campaign": "23V123000", "date": "2023-03-02",
            "manufacturer": "Hyundai Motor America", "component": "ELECTRICAL SYSTEM:WIRING",
            "summary": "A wiring harness may overheat.", "mfr_campaign": "240", "units": 5000,
            "do_not_drive": false, "park_outside": true, "notification": null,
            "match": "exact", "models": ["KONA"], "built_from": "2022-09-01", "built_to": "2023-01-31",
            "url": format!("{SITE}/recalls/nhtsa/23V123000")
        })
    );
    assert_eq!(answer["recalls"][0]["match"], "base");
    assert_eq!(
        answer["lookup"],
        json!({
            "nhtsa": "https://www.nhtsa.gov/recalls",
            "tc": "https://wwwapps.tc.gc.ca/Saf-Sec-Sur/7/VRDB-BDRV/search-recherche/menu.aspx?lang=eng",
            "maker": null
        })
    );
    assert_eq!(
        answer["attribution"],
        json!(["Contains information licensed under the Open Government Licence - Canada."])
    );

    // A submodel in the id is set aside, and a country narrows the list.
    let run = offline(
        &fixture,
        &[
            "recalls",
            "vehicle",
            "2023_hyundai_kona_se",
            "--country",
            "US",
        ],
    );
    let answer = run.json();
    assert_eq!(answer["vehicle"]["id"], "2023_hyundai_kona");
    assert_eq!(answer["recalls"].as_array().unwrap().len(), 2);
    assert_eq!(answer["attribution"], json!([]));

    // The VIN names the model year. Nothing says the recall applies to it.
    let run = offline(&fixture, &["recalls", "vin", "km8k2-cab4pu001140"]);
    assert_eq!(run.code, 0, "{}", run.stderr);
    let answer = run.json();
    assert_eq!(
        answer["decode"],
        json!({ "vin": KONA, "valid": true, "year": 2023, "make": "Hyundai", "model": "Kona", "trim": "SE" })
    );
    assert_eq!(answer["vehicle"]["id"], "2023_hyundai_kona");
    assert_eq!(answer["may_apply"], true);
    assert_eq!(answer["recalls"].as_array().unwrap().len(), 3);
    assert_eq!(answer["warnings"], json!([]));

    let run = offline(&fixture, &["recalls", "campaign", "nhtsa", "19v888000"]);
    assert_eq!(run.code, 0, "{}", run.stderr);
    let answer = run.json();
    assert_eq!(answer["campaign"], "19V888000");
    assert_eq!(answer["remedy"], "Dealers will tighten the nut.");
    assert_eq!(answer["french"], Value::Null);
    assert_eq!(answer["as_of"], "2026-10-11");
    assert_eq!(
        answer["vehicles"],
        json!([
            { "year": 2019, "make": "CHEVROLET", "model": "SILVERADO 1500", "vehicle_id": "2019_chevrolet_silverado", "match": "base", "built_from": "2018-07-11", "built_to": "2019-11-22" },
            { "year": 2019, "make": "GMC", "model": "SIERRA 1500", "vehicle_id": null, "match": null, "built_from": null, "built_to": null }
        ])
    );
    let run = offline(&fixture, &["recalls", "campaign", "tc", "2023101"]);
    assert_eq!(
        run.json()["french"]["summary"],
        "Un faisceau peut surchauffer."
    );

    let run = offline(&fixture, &["recalls", "search", "--limit", "3"]);
    let answer = run.json();
    assert_eq!(answer["recalls"].as_array().unwrap().len(), 3);
    assert_eq!(answer["recalls"][0]["campaign"], "24V456000");
    assert_eq!(answer["recalls"][0]["match"], Value::Null);
    let cursor = answer["next_cursor"].as_str().unwrap().to_owned();
    let run = offline(&fixture, &["recalls", "search", "--cursor", &cursor]);
    let answer = run.json();
    assert_eq!(answer["recalls"][0]["campaign"], "19V888000");
    assert_eq!(answer["next_cursor"], Value::Null);
    let run = offline(
        &fixture,
        &[
            "recalls", "search", "wiring", "harness", "--make", "hyundai", "--year", "2023",
        ],
    );
    let found: Vec<String> = run.json()["recalls"]
        .as_array()
        .unwrap()
        .iter()
        .map(|recall| recall["campaign"].as_str().unwrap().to_owned())
        .collect();
    assert_eq!(found, ["2023101", "23V123000"]);
    // A make the catalog does not have lists nothing. It is not an error.
    let run = offline(&fixture, &["recalls", "search", "--make", "delorean"]);
    assert_eq!(run.code, 0);
    assert_eq!(run.json()["recalls"], json!([]));
}

#[test]
fn a_recall_question_that_is_wrong_fails_with_the_api_codes() {
    let fixture = common::shop_dir();
    let cases: [(&[&str], &str, u8); 7] = [
        (
            &["recalls", "vehicle", "2023_hyundai_nothing"],
            "not_found",
            3,
        ),
        (
            &["recalls", "vehicle", "2023_hyundai_kona", "--country", "mx"],
            "validation_failed",
            4,
        ),
        (&["recalls", "vin", "KM8K2CAB4PUO01140"], "invalid_vin", 4),
        (&["recalls", "vin", common::UNKNOWN], "not_found", 3),
        (
            &["recalls", "campaign", "nhtsa", "00V000000"],
            "not_found",
            3,
        ),
        (
            &["recalls", "search", "--model", "kona"],
            "validation_failed",
            4,
        ),
        (
            &["recalls", "search", "--cursor", "nonsense"],
            "validation_failed",
            4,
        ),
    ];
    for (args, code, exit) in cases {
        let run = offline(&fixture, args);
        assert_eq!(run.error()["error"]["code"], code, "{args:?}");
        assert_eq!(run.code, exit, "{args:?}");
        assert_eq!(run.stdout, "", "{args:?}");
    }
    let run = offline(&fixture, &["recalls", "campaign", "nhtsa", "00V000000"]);
    assert_eq!(
        run.error()["error"]["message"],
        "No recall campaign has that number."
    );
}

#[test]
fn without_the_shop_file_the_recall_commands_say_what_to_run() {
    // The data file is here; the shop file is not.
    let fixture = common::data_dir();
    for args in [
        &["recalls", "vehicle", "2023_hyundai_kona"][..],
        &["recalls", "vin", KONA],
        &["recalls", "campaign", "nhtsa", "19V888000"],
        &["recalls", "search"],
    ] {
        let run = common::run(&common::env(&fixture), args);
        assert_eq!(run.code, 11, "{args:?}");
        assert_eq!(
            run.error(),
            json!({
                "error": {
                    "code": "dataset_unavailable",
                    "message": "The recalls_us dataset is not available.",
                    "details": { "dataset": "recalls_us", "hint": "Run: wenmar-open data pull --shop" }
                }
            }),
            "{args:?}"
        );
    }
    // A person at a terminal is told the same in a line of text.
    let terminal = Env {
        stdout_terminal: true,
        ..common::env(&fixture)
    };
    let run = common::run(&terminal, &["recalls", "search", "--country", "ca"]);
    assert!(
        run.stderr
            .contains("The recalls_ca dataset is not available."),
        "{}",
        run.stderr
    );
    assert!(
        run.stderr.contains("Run: wenmar-open data pull --shop"),
        "{}",
        run.stderr
    );
    // The data file still answers everything else.
    let run = common::run(&common::env(&fixture), &["vin", "decode", KONA]);
    assert_eq!(run.json()["model"], "Kona");
}

#[test]
fn a_shop_file_that_goes_with_another_data_file_is_refused() {
    let fixture = common::data_dir();
    common::write_shop_file(&fixture.shop_file(), "2026.09.14", "2026.08");
    let run = offline(&fixture, &["recalls", "vehicle", "2023_hyundai_kona"]);
    assert_eq!(run.code, 11);
    let error = run.error();
    assert_eq!(error["error"]["code"], "data_invalid");
    let message = error["error"]["message"].as_str().unwrap();
    assert!(
        message.contains("was matched against data version 2026.08")
            && message.contains("the data file is version 2026.09"),
        "{message}"
    );
    assert_eq!(
        error["error"]["details"]["hint"],
        "Run: wenmar-open data pull --shop"
    );
    // A file that is not a shop file at all is refused the same way, and
    // neither stops the data file from answering.
    std::fs::write(fixture.shop_file(), b"not a database").unwrap();
    let run = offline(&fixture, &["recalls", "search"]);
    assert_eq!(run.error()["error"]["code"], "data_invalid");
    let run = offline(&fixture, &["vin", "decode", KONA]);
    assert_eq!(run.json()["model"], "Kona");

    let run = common::run(&common::env(&fixture), &["data", "status", "--shop"]);
    assert_eq!(run.code, 0);
    assert_eq!(run.json()["installed"], true);
    assert_eq!(run.json()["usable"], false);
}

#[test]
fn at_a_terminal_recalls_are_text_that_says_may_apply() {
    let fixture = common::shop_dir();
    let terminal = Env {
        stdout_terminal: true,
        ..common::env(&fixture)
    };
    let run = common::run(&terminal, &["recalls", "vin", KONA, "--offline"]);
    assert_eq!(run.code, 0, "{}", run.stderr);
    assert_eq!(
        run.stdout,
        "\
VIN KM8K2CAB4PU001140: 2023 Hyundai Kona: 3 recall campaigns may apply

2024-05-10  nhtsa 24V456000  DO NOT DRIVE (NHTSA advisory)
  SERVICE BRAKES, HYDRAULIC
  The brake control unit may fail.

2023-03-10  tc 2023101
  Electrical
  Issue: A wiring harness may overheat.

2023-03-02  nhtsa 23V123000  PARK OUTSIDE (NHTSA advisory)
  ELECTRICAL SYSTEM:WIRING
  A wiring harness may overheat.

As of: United States 2026-10-11, Canada 2026-10-06
Check the VIN:
  United States: https://www.nhtsa.gov/recalls
  Canada: https://wwwapps.tc.gc.ca/Saf-Sec-Sur/7/VRDB-BDRV/search-recherche/menu.aspx?lang=eng
Contains information licensed under the Open Government Licence - Canada.
A recall listed here may apply to a vehicle of that year, make and model. Only the manufacturer can say whether it applies to one vehicle: check the VIN at the addresses above.
"
    );
    // Every kind of recall answer says "may apply" and gives both lookups,
    // and none says a recall applies to this vehicle.
    for args in [
        &[
            "recalls",
            "vehicle",
            "2019_chevrolet_silverado",
            "--offline",
        ][..],
        &["recalls", "vehicle", "2019_honda_civic", "--offline"],
        &["recalls", "campaign", "nhtsa", "19V888000", "--offline"],
        &["recalls", "campaign", "tc", "2023101", "--offline"],
        &["recalls", "search", "--offline"],
    ] {
        let run = common::run(&terminal, args);
        assert_eq!(run.code, 0, "{args:?} {}", run.stderr);
        assert!(run.stdout.contains("may apply"), "{args:?}");
        assert!(run.stdout.contains("https://www.nhtsa.gov/recalls"));
        assert!(run.stdout.contains("https://wwwapps.tc.gc.ca/"));
        for claim in [
            "is affected",
            "open recall",
            "your vehicle",
            "applies to this",
        ] {
            assert!(!run.stdout.contains(claim), "{args:?} says {claim}");
        }
    }
    let run = common::run(
        &terminal,
        &["recalls", "vehicle", "2019_honda_civic", "--offline"],
    );
    assert!(
        run.stdout
            .starts_with("2019 Honda Civic: no recall campaigns were found that may apply\n"),
        "{}",
        run.stdout
    );
    let run = common::run(
        &terminal,
        &["recalls", "campaign", "nhtsa", "19V888000", "--offline"],
    );
    assert!(
        run.stdout.starts_with("NHTSA recall 19V888000\n"),
        "{}",
        run.stdout
    );
    assert!(
        run.stdout
            .contains("Remedy\nDealers will tighten the nut.\n")
    );
    assert!(
        run.stdout
            .contains("2019  CHEVROLET  SILVERADO 1500  2019_chevrolet_silverado")
    );
}

// ----- data pull --shop -----

fn gzip(bytes: &[u8]) -> Vec<u8> {
    let mut encoder = GzEncoder::new(Vec::new(), Compression::default());
    encoder.write_all(bytes).unwrap();
    encoder.finish().unwrap()
}

/// A shop file of the given versions, gzipped as a release carries it.
fn packed(data_version: &str, catalog_data_version: &str) -> Vec<u8> {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("built.sqlite3");
    common::write_shop_file(&path, data_version, catalog_data_version);
    gzip(&std::fs::read(path).unwrap())
}

fn release(host: &str, version: &str) -> Value {
    let name = format!("wenmar-open-shop-{version}");
    json!({
        "tag_name": format!("shop-{version}"),
        "draft": false,
        "prerelease": false,
        "assets": [
            { "name": format!("{name}.sqlite3.gz.sha256"), "browser_download_url": format!("http://{host}/assets/{name}.sqlite3.gz.sha256") },
            { "name": format!("{name}.json"), "browser_download_url": format!("http://{host}/assets/{name}.json") },
            { "name": format!("{name}.sqlite3.gz"), "browser_download_url": format!("http://{host}/assets/{name}.sqlite3.gz") }
        ]
    })
}

/// A release server with one data release and three shop releases: two
/// matched against data 2026.09 and the newest against data 2026.10.
/// `asset` answers the download of shop data 2026.10.12.
fn releases(asset: impl Fn() -> Reply + Send + 'static) -> Server {
    let older = packed("2026.10.05", "2026.09");
    let newer = packed("2026.10.19", "2026.10");
    server::serve(move |request: &Seen| {
        let host = request.header("host").unwrap_or_default().to_owned();
        let manifest = |version: &str, catalog: &str| {
            Reply::json(
                200,
                &json!({ "data_version": version, "catalog_data_version": catalog, "schema_version": "1" }),
            )
        };
        match request.target.as_str() {
            "/releases?per_page=100" => Reply::json(
                200,
                &json!([
                    { "tag_name": "data-2026.09", "draft": false, "prerelease": false, "assets": [] },
                    release(&host, "2026.10.05"),
                    release(&host, "2026.10.19"),
                    release(&host, "2026.10.12")
                ]),
            ),
            "/releases/tags/shop-2026.10.05" => Reply::json(200, &release(&host, "2026.10.05")),
            "/releases/tags/shop-2026.10.19" => Reply::json(200, &release(&host, "2026.10.19")),
            "/assets/wenmar-open-shop-2026.10.05.json" => manifest("2026.10.05", "2026.09"),
            "/assets/wenmar-open-shop-2026.10.12.json" => manifest("2026.10.12", "2026.09"),
            "/assets/wenmar-open-shop-2026.10.19.json" => manifest("2026.10.19", "2026.10"),
            "/assets/wenmar-open-shop-2026.10.05.sqlite3.gz" => Reply::bytes(older.clone()),
            "/assets/wenmar-open-shop-2026.10.19.sqlite3.gz" => Reply::bytes(newer.clone()),
            "/assets/wenmar-open-shop-2026.10.12.sqlite3.gz" => asset(),
            _ => Reply::json(404, &json!({ "message": "Not Found" })),
        }
    })
}

fn good() -> Server {
    let file = packed("2026.10.12", "2026.09");
    releases(move || Reply::bytes(file.clone()))
}

fn env(fixture: &Fixture, server: &Server) -> Env {
    Env {
        releases: Some(format!("{}/releases", server.url())),
        ..common::env(fixture)
    }
}

#[test]
fn pull_shop_downloads_the_newest_shop_file_that_goes_with_the_data_file() {
    let server = good();
    let fixture = common::data_dir();
    let env = env(&fixture, &server);

    let run = common::run(&env, &["data", "pull", "--shop"]);
    assert_eq!(run.code, 0, "{}", run.stderr);
    let answer = run.json();
    assert_eq!(answer["updated"], true);
    // 2026.10.19 is newer, and goes with a data file that is not here.
    assert_eq!(answer["data_version"], "2026.10.12");
    assert_eq!(answer["path"], fixture.shop_file().display().to_string());
    assert_eq!(
        fixture.files(),
        ["wenmar-open-shop.sqlite3", "wenmar-open.sqlite3"]
    );
    assert_eq!(
        server.targets(),
        [
            "/releases?per_page=100",
            "/assets/wenmar-open-shop-2026.10.19.json",
            "/assets/wenmar-open-shop-2026.10.12.json",
            "/assets/wenmar-open-shop-2026.10.12.sqlite3.gz"
        ]
    );
    // And it answers.
    let run = common::run(&env, &["recalls", "vin", KONA, "--offline"]);
    assert_eq!(run.json()["recalls"].as_array().unwrap().len(), 3);

    let run = common::run(&env, &["data", "status", "--shop"]);
    assert_eq!(
        run.json(),
        json!({
            "installed": true,
            "usable": true,
            "path": fixture.shop_file().display().to_string(),
            "bytes": std::fs::metadata(fixture.shop_file()).unwrap().len(),
            "data_version": "2026.10.12",
            "catalog_data_version": "2026.09",
            "reads_schema_version": "1",
            "built_at": "2026-10-12 06:00:00",
            "datasets": serde_json::from_str::<Value>(common::SHOP_DATASETS).unwrap()
        })
    );

    // Pulling again downloads nothing, unless it is forced.
    let before = server.seen().len();
    let run = common::run(&env, &["data", "pull", "--shop"]);
    assert_eq!(run.json()["updated"], false);
    assert_eq!(server.seen().len(), before + 3);
    let run = common::run(&env, &["data", "pull", "--shop", "--force"]);
    assert_eq!(run.json()["updated"], true);

    // An older one can be asked for by its day.
    let run = common::run(&env, &["data", "pull", "--shop", "2026.10.05"]);
    assert_eq!(run.code, 0, "{}", run.stderr);
    assert_eq!(run.json()["data_version"], "2026.10.05");
    assert_eq!(run.json()["previous"], "2026.10.12");
}

#[test]
fn pull_shop_needs_the_data_file_it_goes_with() {
    let server = good();
    // No data file at all.
    let empty = common::empty_dir();
    let run = common::run(&env(&empty, &server), &["data", "pull", "--shop"]);
    assert_eq!(run.code, 11);
    assert_eq!(run.error()["error"]["code"], "no_data");
    assert_eq!(
        run.error()["error"]["details"]["hint"],
        "Run `wenmar-open data pull`, then `wenmar-open data pull --shop`."
    );
    assert!(server.seen().is_empty());
    assert!(empty.files().is_empty());

    // A data file no shop release goes with.
    let other = common::empty_dir();
    common::write_data_file(&other.file(), "3", "2026.07");
    let run = common::run(&env(&other, &server), &["data", "pull", "--shop"]);
    assert_eq!(run.code, 3);
    assert_eq!(
        run.error()["error"]["message"],
        "No shop data release goes with data version 2026.07."
    );
    assert_eq!(other.files(), ["wenmar-open.sqlite3"]);

    // A shop release asked for by its day that goes with another data file.
    let fixture = common::data_dir();
    let run = common::run(
        &env(&fixture, &server),
        &["data", "pull", "--shop", "2026.10.19"],
    );
    assert_eq!(run.error()["error"]["code"], "data_invalid");
    assert_eq!(
        run.error()["error"]["message"],
        "Shop data 2026.10.19 goes with data version 2026.10, and the data file here is 2026.09."
    );
    assert_eq!(fixture.files(), ["wenmar-open.sqlite3"]);
    // Text that is not a shop version never reaches an address.
    let before = server.seen().len();
    for version in ["2026.10", "../../x", "latest", "2026.10.12?x=1"] {
        let run = common::run(
            &env(&fixture, &server),
            &["data", "pull", "--shop", version],
        );
        assert_eq!(run.code, 4, "{version}");
        assert_eq!(run.error()["error"]["code"], "validation_failed");
    }
    assert_eq!(server.seen().len(), before);
}

/// What answers the download of a shop file.
type Asset = Box<dyn Fn() -> Reply + Send>;

#[test]
fn a_shop_download_that_is_wrong_changes_nothing() {
    // The release says 2026.10.12 and the file inside says otherwise; then
    // one that is not a shop file; then one cut short.
    let mislabelled = packed("2026.10.11", "2026.09");
    let wrong_pair = packed("2026.10.12", "2026.08");
    let whole = packed("2026.10.12", "2026.09");
    let half = whole[..whole.len() / 2].to_vec();
    let length = whole.len();
    let cases: Vec<(Asset, &str)> = vec![
        (
            Box::new(move || Reply::bytes(mislabelled.clone())),
            "data_invalid",
        ),
        (
            Box::new(move || Reply::bytes(wrong_pair.clone())),
            "data_invalid",
        ),
        (
            Box::new(|| Reply::bytes(gzip(b"this is not a database"))),
            "data_invalid",
        ),
        (
            Box::new(move || Reply {
                claimed_length: Some(length),
                ..Reply::bytes(half.clone())
            }),
            "download_failed",
        ),
    ];
    for (asset, code) in cases {
        let server = releases(asset);
        let fixture = common::shop_dir();
        let before = std::fs::read(fixture.shop_file()).unwrap();
        let run = common::run(
            &env(&fixture, &server),
            &["data", "pull", "--shop", "--force"],
        );
        assert_eq!(run.error()["error"]["code"], code, "{}", run.stderr);
        assert_eq!(run.stdout, "");
        // The old shop file is as it was, and nothing was left beside it.
        assert_eq!(
            fixture.files(),
            ["wenmar-open-shop.sqlite3", "wenmar-open.sqlite3"]
        );
        assert_eq!(std::fs::read(fixture.shop_file()).unwrap(), before);
    }
}

#[test]
fn shop_status_says_what_is_there() {
    let empty = common::data_dir();
    let run = common::run(&common::env(&empty), &["data", "status", "--shop"]);
    assert_eq!(run.code, 0);
    assert_eq!(
        run.json(),
        json!({
            "installed": false,
            "usable": false,
            "path": empty.shop_file().display().to_string(),
            "reads_schema_version": "1"
        })
    );
    let terminal = Env {
        stdout_terminal: true,
        ..common::env(&empty)
    };
    let run = common::run(&terminal, &["data", "status", "--shop"]);
    assert_eq!(
        run.stdout,
        format!(
            "There is no shop data file at {}.\nIt holds the recalls. Run `wenmar-open data pull --shop` to download it, or pass --online to ask the hosted API.\n",
            empty.shop_file().display()
        )
    );
    // The data file's own status is as it was: it does not mention the
    // shop file.
    let run = common::run(&common::env(&empty), &["data", "status"]);
    assert!(run.json().get("datasets").is_none());

    let fixture = common::shop_dir();
    let terminal = Env {
        stdout_terminal: true,
        ..common::env(&fixture)
    };
    let run = common::run(&terminal, &["data", "status", "--shop"]);
    assert!(
        run.stdout
            .contains("Version         2026.10.12 (for data 2026.09)"),
        "{}",
        run.stdout
    );
    assert!(
        run.stdout
            .contains("recalls_us      as of 2026-10-11, 3 rows")
    );
}
