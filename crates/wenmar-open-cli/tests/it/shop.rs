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

/// A release server whose one shop release has both files: the offline one
/// under the name every release has had, and the full one, which holds the
/// bulletins too. The full file answers with something that is not a shop
/// file, so a download of it would fail.
fn two_files() -> Server {
    let offline = packed("2026.10.12", "2026.09");
    server::serve(move |request: &Seen| {
        let host = request.header("host").unwrap_or_default().to_owned();
        let full = "wenmar-open-shop-full-2026.10.12.sqlite3.gz";
        let mut both = release(&host, "2026.10.12");
        // GitHub lists assets by name, so the full file comes first.
        let mut assets = vec![
            json!({ "name": full, "browser_download_url": format!("http://{host}/assets/{full}") }),
            json!({ "name": format!("{full}.sha256"), "browser_download_url": format!("http://{host}/assets/{full}.sha256") }),
        ];
        assets.extend(both["assets"].as_array().unwrap().clone());
        both["assets"] = json!(assets);
        let bulletins = json!({ "as_of": "2026-10-11", "rows": 2, "licence": "public-domain", "attribution": "" });
        match request.target.as_str() {
            "/releases?per_page=100" => Reply::json(200, &json!([both])),
            "/releases/tags/shop-2026.10.12" => Reply::json(200, &both),
            // The manifest as scripts/shop-manifest.mjs writes it for two
            // files: the fields it always had, and `full`.
            "/assets/wenmar-open-shop-2026.10.12.json" => Reply::json(
                200,
                &json!({
                    "data_version": "2026.10.12",
                    "catalog_data_version": "2026.09",
                    "schema_version": "1",
                    "built_at": "2026-10-12 06:00:00",
                    "bytes": 1,
                    "sha256": "00",
                    "datasets": serde_json::from_str::<Value>(common::SHOP_DATASETS).unwrap(),
                    "full": { "bytes": 2, "sha256": "11", "datasets": { "bulletins": bulletins } }
                }),
            ),
            "/assets/wenmar-open-shop-2026.10.12.sqlite3.gz" => Reply::bytes(offline.clone()),
            "/assets/wenmar-open-shop-full-2026.10.12.sqlite3.gz" => {
                Reply::bytes(gzip(b"the full file, which is not for here"))
            }
            _ => Reply::json(404, &json!({ "message": "Not Found" })),
        }
    })
}

#[test]
fn pull_shop_takes_the_offline_file_and_never_the_full_one() {
    let server = two_files();
    let fixture = common::data_dir();
    let env = env(&fixture, &server);

    let run = common::run(&env, &["data", "pull", "--shop"]);
    assert_eq!(run.code, 0, "{}", run.stderr);
    assert_eq!(run.json()["updated"], true);
    assert_eq!(run.json()["data_version"], "2026.10.12");
    assert_eq!(
        server.targets(),
        [
            "/releases?per_page=100",
            "/assets/wenmar-open-shop-2026.10.12.json",
            "/assets/wenmar-open-shop-2026.10.12.sqlite3.gz"
        ]
    );
    // Asked for by its day, and forced: the same file again.
    let run = common::run(&env, &["data", "pull", "--shop", "2026.10.12", "--force"]);
    assert_eq!(run.code, 0, "{}", run.stderr);
    assert_eq!(run.json()["updated"], true);
    for target in server.targets() {
        assert!(!target.contains("full"), "{target}");
    }
    // What is in place holds no bulletins, and answers.
    let run = common::run(&env, &["data", "status", "--shop"]);
    let datasets = run.json()["datasets"].clone();
    assert!(datasets.get("recalls_us").is_some());
    assert!(datasets.get("bulletins").is_none());
    let run = common::run(&env, &["recalls", "vin", KONA, "--offline"]);
    assert_eq!(run.json()["recalls"].as_array().unwrap().len(), 3);
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

// ----- the overview commands -----

#[test]
fn the_overview_commands_answer_from_the_files_as_the_api_words_it() {
    let fixture = common::shop_dir();

    let run = offline(&fixture, &["vehicles", "overview", "2023_hyundai_kona_se"]);
    assert_eq!(run.code, 0, "{}", run.stderr);
    let answer = run.json();
    assert_eq!(
        answer["vehicle"],
        json!({ "id": "2023_hyundai_kona", "year": 2023, "make": "Hyundai", "model": "Kona" })
    );
    assert_eq!(answer["decode"], Value::Null);
    let recalls = &answer["recalls"];
    assert_eq!(recalls["may_apply"], true);
    assert_eq!(recalls["count"], json!({ "us": 2, "ca": 1 }));
    assert_eq!(
        recalls["as_of"],
        json!({ "us": "2026-10-11", "ca": "2026-10-06" })
    );
    assert_eq!(recalls["do_not_drive"], true);
    assert_eq!(recalls["park_outside"], true);
    let newest: Vec<&str> = recalls["newest"]
        .as_array()
        .unwrap()
        .iter()
        .map(|recall| recall["campaign"].as_str().unwrap())
        .collect();
    assert_eq!(newest, ["24V456000", "2023101", "23V123000"]);
    assert_eq!(
        recalls["url"],
        format!("{SITE}/makes/hyundai/kona/2023/recalls")
    );
    assert_eq!(recalls["lookup"]["nhtsa"], "https://www.nhtsa.gov/recalls");
    assert_eq!(recalls["lookup"]["maker"], Value::Null);
    assert_eq!(
        answer["links"],
        json!({ "page": format!("{SITE}/makes/hyundai/kona/2023"), "maker": null })
    );
    // A block the files do not hold is null, and it is there.
    for block in [
        "specs",
        "fuel",
        "fluids",
        "tires",
        "safety",
        "labor",
        "bulletins",
    ] {
        assert_eq!(answer[block], Value::Null, "{block}");
        assert!(answer.as_object().unwrap().contains_key(block), "{block}");
    }
    assert_eq!(
        answer["attribution"],
        json!(["Contains information licensed under the Open Government Licence - Canada."])
    );
    assert_eq!(answer["warnings"], json!([]));

    // By VIN: the same, with what the VIN decoded to.
    let run = offline(&fixture, &["vin", "overview", KONA]);
    assert_eq!(run.code, 0, "{}", run.stderr);
    let by_vin = run.json();
    assert_eq!(by_vin["decode"]["vin"], KONA);
    assert_eq!(by_vin["decode"]["valid"], true);
    assert_eq!(by_vin["decode"]["year"], 2023);
    assert_eq!(by_vin["decode"]["entry_id"], "2023_hyundai_kona_se_2-0l");
    assert_eq!(by_vin["decode"]["engine"], "2.0L");
    assert_eq!(by_vin["decode"]["engine_settled"], true);
    for key in ["vehicle", "recalls", "links", "attribution"] {
        assert_eq!(by_vin[key], answer[key], "{key}");
    }

    // The model years of a model, each with its count.
    let run = offline(
        &fixture,
        &[
            "vehicles",
            "model-years",
            "--make",
            "Hyundai",
            "--model",
            "KONA",
        ],
    );
    assert_eq!(run.code, 0, "{}", run.stderr);
    let years = run.json();
    assert_eq!(years["make"], json!({ "id": "hyundai", "name": "Hyundai" }));
    assert_eq!(years["model"], json!({ "id": "kona", "name": "Kona" }));
    assert_eq!(years["may_apply"], true);
    assert_eq!(
        years["years"],
        json!([
            {
                "id": "2023_hyundai_kona", "year": 2023,
                "recalls": { "us": 2, "ca": 1 }, "bulletins": null,
                "url": format!("{SITE}/makes/hyundai/kona/2023")
            },
            {
                "id": "2022_hyundai_kona", "year": 2022,
                "recalls": { "us": 0, "ca": 0 }, "bulletins": null,
                "url": format!("{SITE}/makes/hyundai/kona/2022")
            }
        ])
    );
    assert_eq!(years["lookup"]["maker"], Value::Null);

    // The campaigns reported since a day, with the vehicles they list.
    let run = offline(&fixture, &["recalls", "recent", "--since", "2023-03-05"]);
    assert_eq!(run.code, 0, "{}", run.stderr);
    let recent = run.json();
    assert_eq!(recent["may_apply"], true);
    assert_eq!(recent["since"], "2023-03-05");
    let listed: Vec<&str> = recent["recalls"]
        .as_array()
        .unwrap()
        .iter()
        .map(|entry| entry["recall"]["campaign"].as_str().unwrap())
        .collect();
    assert_eq!(listed, ["24V456000", "2023101"]);
    assert_eq!(recent["recalls"][0]["vehicles_listed"], 1);
    assert_eq!(
        recent["recalls"][0]["vehicles"],
        json!([{
            "year": 2023, "make": "HYUNDAI", "model": "KONA ELECTRIC",
            "vehicle_id": "2023_hyundai_kona", "match": "base",
            "built_from": null, "built_to": null
        }])
    );
    assert_eq!(recent["next_cursor"], Value::Null);

    // What is wrong fails with the API's codes and sentences.
    let run = offline(&fixture, &["vehicles", "overview", "2019_honda_nothing"]);
    assert_eq!(run.code, 3);
    assert_eq!(run.error()["error"]["message"], "No vehicle has that id.");
    let run = offline(
        &fixture,
        &[
            "vehicles",
            "model-years",
            "--make",
            "honda",
            "--model",
            "nothing",
        ],
    );
    assert_eq!(run.code, 3);
    assert_eq!(run.error()["error"]["message"], "No model has that id.");
    let run = offline(&fixture, &["recalls", "recent", "--since", "yesterday"]);
    assert_eq!(run.code, 4);
    assert_eq!(run.error()["error"]["details"]["field"], "since");
    let run = offline(&fixture, &["vin", "overview", "KM8"]);
    assert_eq!(run.error()["error"]["code"], "invalid_vin");
}

#[test]
fn without_the_shop_file_an_overview_still_answers_and_its_recalls_are_null() {
    // The data file is here; the shop file is not.
    let fixture = common::data_dir();
    let run = offline(&fixture, &["vehicles", "overview", "2023_hyundai_kona"]);
    assert_eq!(run.code, 0, "{}", run.stderr);
    let answer = run.json();
    assert_eq!(answer["vehicle"]["id"], "2023_hyundai_kona");
    assert_eq!(answer["recalls"], Value::Null);
    assert_eq!(answer["safety"], Value::Null);
    assert_eq!(answer["attribution"], json!([]));
    assert_eq!(
        answer["links"]["page"],
        format!("{SITE}/makes/hyundai/kona/2023")
    );
    assert_eq!(answer.as_object().unwrap().len(), 14);
    assert_eq!(answer["complaints"], Value::Null);

    let run = offline(&fixture, &["vin", "overview", KONA]);
    assert_eq!(run.code, 0, "{}", run.stderr);
    assert_eq!(run.json()["recalls"], Value::Null);
    assert_eq!(run.json()["decode"]["vin"], KONA);

    let run = offline(
        &fixture,
        &[
            "vehicles",
            "model-years",
            "--make",
            "hyundai",
            "--model",
            "kona",
        ],
    );
    assert_eq!(run.code, 0, "{}", run.stderr);
    let years = run.json();
    assert_eq!(years["as_of"], json!({ "us": null, "ca": null }));
    assert_eq!(years["attribution"], json!([]));
    assert_eq!(years["years"].as_array().unwrap().len(), 2);
    for year in years["years"].as_array().unwrap() {
        assert_eq!(year["recalls"], Value::Null);
    }

    // The recent list is recalls and nothing else: it says what to run.
    let run = offline(&fixture, &["recalls", "recent", "--since", "2023-03-05"]);
    assert_eq!(run.code, 11);
    assert_eq!(
        run.error(),
        json!({
            "error": {
                "code": "dataset_unavailable",
                "message": "The recalls_us dataset is not available.",
                "details": { "dataset": "recalls_us", "hint": "Run: wenmar-open data pull --shop" }
            }
        })
    );

    // A shop file that goes with another data file is not read: the
    // overview answers as with none. `data status --shop` says why.
    common::write_shop_file(&fixture.shop_file(), "2026.10.12", "2026.08");
    let run = offline(&fixture, &["vehicles", "overview", "2023_hyundai_kona"]);
    assert_eq!(run.code, 0, "{}", run.stderr);
    assert_eq!(run.json()["recalls"], Value::Null);
}

#[test]
fn at_a_terminal_an_overview_is_text_that_says_may_apply() {
    let fixture = common::shop_dir();
    let terminal = Env {
        stdout_terminal: true,
        ..common::env(&fixture)
    };
    let run = common::run(&terminal, &["vin", "overview", KONA, "--offline"]);
    assert_eq!(run.code, 0, "{}", run.stderr);
    assert_eq!(
        run.stdout,
        "\
2023 Hyundai Kona
VIN KM8K2CAB4PU001140
Page: https://open.wenmarpro.com/makes/hyundai/kona/2023

Recalls: 3 recall campaigns may apply
  At least one says DO NOT DRIVE (NHTSA advisory)
  At least one says PARK OUTSIDE (NHTSA advisory)

2024-05-10  nhtsa 24V456000  DO NOT DRIVE (NHTSA advisory)
  SERVICE BRAKES, HYDRAULIC
  The brake control unit may fail.

2023-03-10  tc 2023101
  Electrical
  Issue: A wiring harness may overheat.

2023-03-02  nhtsa 23V123000  PARK OUTSIDE (NHTSA advisory)
  ELECTRICAL SYSTEM:WIRING
  A wiring harness may overheat.

All of them: https://open.wenmarpro.com/makes/hyundai/kona/2023/recalls
As of: United States 2026-10-11, Canada 2026-10-06
Check the VIN:
  United States: https://www.nhtsa.gov/recalls
  Canada: https://wwwapps.tc.gc.ca/Saf-Sec-Sur/7/VRDB-BDRV/search-recherche/menu.aspx?lang=eng
Contains information licensed under the Open Government Licence - Canada.
A recall listed here may apply to a vehicle of that year, make and model. Only the manufacturer can say whether it applies to one vehicle: check the VIN at the addresses above.
"
    );
    for args in [
        &["vehicles", "overview", "2019_honda_civic", "--offline"][..],
        &[
            "vehicles",
            "model-years",
            "--make",
            "hyundai",
            "--model",
            "kona",
            "--offline",
        ],
        &["recalls", "recent", "--since", "2019-01-01", "--offline"],
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
        &[
            "vehicles",
            "model-years",
            "--make",
            "hyundai",
            "--model",
            "kona",
            "--offline",
        ],
    );
    assert!(
        run.stdout.starts_with(
            "Hyundai Kona: 2 model years\n\nID                 YEAR  RECALLS US  RECALLS CA\n2023_hyundai_kona  2023  2           1\n2022_hyundai_kona  2022  0           0\n"
        ),
        "{}",
        run.stdout
    );
    let run = common::run(
        &terminal,
        &["recalls", "recent", "--since", "2019-01-01", "--offline"],
    );
    assert!(
        run.stdout
            .starts_with("4 recall campaigns reported since 2019-01-01, newest first\n"),
        "{}",
        run.stdout
    );
    assert!(
        run.stdout
            .contains("  Lists: 2019 CHEVROLET SILVERADO 1500; 2019 GMC SIERRA 1500\n"),
        "{}",
        run.stdout
    );
    // With no shop file the text says so, and claims nothing.
    let bare = common::data_dir();
    let terminal = Env {
        stdout_terminal: true,
        ..common::env(&bare)
    };
    let run = common::run(
        &terminal,
        &["vehicles", "overview", "2023_hyundai_kona", "--offline"],
    );
    assert_eq!(
        run.stdout,
        "2023 Hyundai Kona\nPage: https://open.wenmarpro.com/makes/hyundai/kona/2023\n\nRecalls: no recall data is held where this was asked.\n"
    );
}

// ----- crash test ratings -----

#[test]
fn the_safety_command_answers_from_the_shop_file_as_the_api_words_it() {
    let fixture = common::ratings_dir();
    let run = offline(&fixture, &["vehicles", "safety", "2023_hyundai_kona_se"]);
    assert_eq!(run.code, 0, "{}", run.stderr);
    let answer = run.json();
    assert_eq!(
        answer["vehicle"],
        json!({ "id": "2023_hyundai_kona", "year": 2023, "make": "Hyundai", "model": "Kona" })
    );
    assert_eq!(answer["as_of"], "2026-08-07");
    assert_eq!(answer["source"], "NHTSA New Car Assessment Program");
    assert_eq!(answer["source_url"], "https://www.nhtsa.gov/ratings");
    assert_eq!(answer["attribution"], json!([]));
    assert_eq!(answer.as_object().unwrap().len(), 6);
    // Every rating, by NHTSA's model name, body style and drive. None is
    // picked.
    let described: Vec<&str> = answer["ratings"]
        .as_array()
        .unwrap()
        .iter()
        .map(|rating| rating["description"].as_str().unwrap())
        .collect();
    assert_eq!(
        described,
        [
            "KONA, SUV, AWD",
            "KONA, SUV, FWD",
            "KONA ELECTRIC, SUV, FWD"
        ]
    );
    assert_eq!(
        answer["ratings"][0],
        json!({
            "description": "KONA, SUV, AWD",
            "model": "KONA",
            "body_style": "SUV",
            "vehicle_type": "MPV",
            "drive": "AWD",
            "release": 1,
            "match": "exact",
            "stars": {
                "overall": 5,
                "frontal": 4,
                "frontal_driver": 4,
                "frontal_passenger": 4,
                "side": 5,
                "side_driver": 5,
                "side_passenger": 5,
                "side_barrier": 5,
                "side_front_seat": 5,
                "side_rear_seat": 5,
                "side_pole": 5,
                "rollover": 4
            },
            "rollover_risk_percent": 14.8,
            "rollover_tip": false,
            "curb_weight_kg": 1463,
            "curb_weight_lb": 3225,
            "features": {
                "esc": null,
                "forward_collision_warning": "standard",
                "forward_collision_warning_evaluation": "meets",
                "lane_departure_warning": "standard",
                "lane_departure_warning_evaluation": "meets",
                "crash_imminent_braking": "standard",
                "crash_imminent_braking_evaluation": "does_not_meet",
                "dynamic_brake_support": "standard",
                "dynamic_brake_support_evaluation": "pending",
                "backup_camera": null,
                "blind_spot_detection": "optional",
                "head_side_airbag": "standard",
                "torso_side_airbag": "standard"
            },
            "notes": []
        })
    );
    assert_eq!(
        answer["ratings"][1]["notes"],
        json!([
            "These ratings do not apply to the N Line.",
            "The rear door opened in the side test."
        ])
    );
    // A vehicle NHTSA lists and has not tested: every star is null.
    let electric = &answer["ratings"][2];
    assert_eq!(electric["match"], "base");
    for (name, stars) in electric["stars"].as_object().unwrap() {
        assert!(stars.is_null(), "{name}");
    }
    // A model year NHTSA lists nothing for: an empty list, not an error.
    let run = offline(&fixture, &["vehicles", "safety", "2019_honda_civic"]);
    assert_eq!(run.code, 0, "{}", run.stderr);
    assert_eq!(run.json()["ratings"], json!([]));
    assert_eq!(run.json()["as_of"], "2026-08-07");
    let run = offline(&fixture, &["vehicles", "safety", "2019_honda_nothing"]);
    assert_eq!(run.code, 3);
    assert_eq!(run.error()["error"]["message"], "No vehicle has that id.");
}

#[test]
fn without_the_ratings_the_safety_command_says_what_to_run() {
    // With no shop file, and with one that holds the recalls alone.
    for fixture in [common::data_dir(), common::shop_dir()] {
        let run = offline(&fixture, &["vehicles", "safety", "2023_hyundai_kona"]);
        assert_eq!(run.code, 11);
        assert_eq!(
            run.error(),
            json!({
                "error": {
                    "code": "dataset_unavailable",
                    "message": "The ratings dataset is not available.",
                    "details": { "dataset": "ratings", "hint": "Run: wenmar-open data pull --shop" }
                }
            })
        );
        // The data file still answers everything else.
        let run = offline(&fixture, &["vehicles", "entry", "2023_hyundai_kona"]);
        assert_eq!(run.code, 0, "{}", run.stderr);
    }
}

#[test]
fn at_a_terminal_the_ratings_are_text_in_nhtsas_words() {
    let fixture = common::ratings_dir();
    let terminal = Env {
        stdout_terminal: true,
        ..common::env(&fixture)
    };
    let run = common::run(
        &terminal,
        &["vehicles", "safety", "2023_hyundai_kona", "--offline"],
    );
    assert_eq!(run.code, 0, "{}", run.stderr);
    assert_eq!(
        run.stdout,
        "\
2023 Hyundai Kona: 3 crash test ratings from NHTSA

KONA, SUV, AWD
  Overall 5/5, Frontal 4/5, Side 5/5, Rollover 4/5
  Rollover risk 14.8%

KONA, SUV, FWD
  Overall 4/5, Frontal 4/5, Side 5/5, Rollover 4/5
  Rollover risk 15.5%
  Note: These ratings do not apply to the N Line.
  Note: The rear door opened in the side test.

KONA ELECTRIC, SUV, FWD
  No stars: NHTSA lists this vehicle and has not tested it.

As of: 2026-08-07
Source: NHTSA New Car Assessment Program, https://www.nhtsa.gov/ratings
NHTSA rates by body style and drive: each rating is for the vehicle named above it.
"
    );
    let run = common::run(
        &terminal,
        &["vehicles", "safety", "2019_honda_civic", "--offline"],
    );
    assert!(
        run.stdout
            .starts_with("2019 Honda Civic: NHTSA lists no crash test rating\n"),
        "{}",
        run.stdout
    );
}

#[test]
fn an_overview_has_the_crash_test_ratings_when_the_shop_file_holds_them() {
    let fixture = common::ratings_dir();
    let overview = offline(&fixture, &["vehicles", "overview", "2023_hyundai_kona"]).json();
    let safety = offline(&fixture, &["vehicles", "safety", "2023_hyundai_kona"]).json();
    assert_eq!(overview["safety"], safety);
    assert_eq!(overview["safety"]["ratings"].as_array().unwrap().len(), 3);
    let by_vin = offline(&fixture, &["vin", "overview", KONA]).json();
    assert_eq!(by_vin["safety"], safety);
    // The ratings ask for no credit line: the recalls' own is all there is.
    assert_eq!(
        overview["attribution"],
        json!(["Contains information licensed under the Open Government Licence - Canada."])
    );
    // A model year NHTSA lists nothing for: the block is there, with an
    // empty list.
    let civic = offline(&fixture, &["vehicles", "overview", "2019_honda_civic"]).json();
    assert_eq!(civic["safety"]["ratings"], json!([]));
    // A shop file with the recalls alone: null, and still there.
    let recalls_only = common::shop_dir();
    let overview = offline(
        &recalls_only,
        &["vehicles", "overview", "2023_hyundai_kona"],
    )
    .json();
    assert_eq!(overview["safety"], Value::Null);
    assert!(overview.as_object().unwrap().contains_key("safety"));
    // At a terminal the overview says how many there are and where they are.
    let terminal = Env {
        stdout_terminal: true,
        ..common::env(&fixture)
    };
    let run = common::run(
        &terminal,
        &["vehicles", "overview", "2023_hyundai_kona", "--offline"],
    );
    assert!(
        run.stdout
            .contains("\nCrash test ratings: 3 from NHTSA (wenmar-open vehicles safety)\n"),
        "{}",
        run.stdout
    );
}
