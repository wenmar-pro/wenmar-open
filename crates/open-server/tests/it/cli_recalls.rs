//! The command-line tool against this server: the same two files and the
//! same questions about recalls, asked three ways. The tool answers from
//! the files with `wenmar_shopdata::answer`; the tool asks this server
//! over HTTP; and the API is asked directly. All three must give the same
//! JSON.

use std::ffi::OsString;
use std::net::SocketAddr;
use std::path::Path;

use axum::http::StatusCode;
use open_server::serve::{Limits, serve};
use serde_json::Value;
use tokio::net::TcpListener;
use tokio::sync::oneshot;
use wenmar_open_cli::env::{DATA_FILE, Env, SHOP_FILE};
use wenmar_shopdata::answer::SITE;

use crate::common;

/// Runs the tool in this process and reads what it printed: the answer on
/// standard output, or the error on standard error.
fn tool(directory: &Path, api: &str, args: &[&str]) -> (u8, Value) {
    let env = Env {
        data_dir: Some(directory.to_path_buf()),
        api: Some(api.to_owned()),
        releases: Some("http://127.0.0.1:1".to_owned()),
        ..Env::default()
    };
    let mut args: Vec<OsString> = args.iter().map(OsString::from).collect();
    args.insert(0, OsString::from("wenmar-open"));
    let mut stdin: &[u8] = b"";
    let (mut stdout, mut stderr) = (Vec::new(), Vec::new());
    let code = wenmar_open_cli::run(args, &env, &mut stdin, &mut stdout, &mut stderr);
    let printed = if code == 0 { stdout } else { stderr };
    let value = serde_json::from_slice(&printed)
        .unwrap_or_else(|error| panic!("{error}: {}", String::from_utf8_lossy(&printed)));
    (code, value)
}

/// The one thing the files cannot say: the maker's own lookup is compiled
/// into the server. It is set aside before two answers are compared.
fn without_maker(mut answer: Value) -> Value {
    if let Some(lookup) = answer.get_mut("lookup").and_then(Value::as_object_mut) {
        lookup.insert("maker".to_owned(), Value::Null);
    }
    answer
}

#[tokio::test(flavor = "multi_thread")]
async fn the_tool_answers_recall_questions_as_the_api_does_from_files_and_over_http() {
    // The API, saying its pages are where the tool says they are.
    let app = common::recalls_app_at(SITE).await;
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address: SocketAddr = listener.local_addr().unwrap();
    let (stop, stopped) = oneshot::channel::<()>();
    let serving = tokio::spawn(serve(
        listener,
        app.router.clone(),
        async move {
            let _ = stopped.await;
        },
        Limits::default(),
    ));
    let api = format!("http://{address}");

    // The same two files, under the names the tool looks for.
    let fixture = common::data_file();
    common::add_shop_file(
        &fixture,
        wenmar_shopdata::schema::SCHEMA_VERSION,
        "2026.09",
        common::RECALL_DATASETS,
        &common::recall_rows(),
    );
    let directory = tempfile::tempdir().unwrap();
    std::fs::copy(fixture.path(), directory.path().join(DATA_FILE)).unwrap();
    std::fs::copy(fixture.shop_path(), directory.path().join(SHOP_FILE)).unwrap();

    let questions: Vec<(&str, Vec<&str>)> = vec![
        (
            "/v1/vehicles/2023_hyundai_kona/recalls",
            vec!["recalls", "vehicle", "2023_hyundai_kona"],
        ),
        (
            "/v1/vehicles/2023_hyundai_kona_se/recalls?country=ca",
            vec![
                "recalls",
                "vehicle",
                "2023_hyundai_kona_se",
                "--country",
                "ca",
            ],
        ),
        (
            "/v1/vehicles/2019_chevrolet_silverado/recalls?country=us",
            vec![
                "recalls",
                "vehicle",
                "2019_chevrolet_silverado",
                "--country",
                "US",
            ],
        ),
        (
            "/v1/vehicles/2019_honda_civic/recalls",
            vec!["recalls", "vehicle", "2019_honda_civic"],
        ),
        (
            "/v1/vehicles/2019_honda_nothing/recalls",
            vec!["recalls", "vehicle", "2019_honda_nothing"],
        ),
        (
            "/v1/vehicles/2023_hyundai_kona/recalls?country=mx",
            vec!["recalls", "vehicle", "2023_hyundai_kona", "--country", "mx"],
        ),
        (
            "/v1/vin/KM8K2CAB4PU001140/recalls",
            vec!["recalls", "vin", common::KONA],
        ),
        (
            "/v1/vin/KM8K2CAB4PU001140/recalls?year=2023&country=us",
            vec![
                "recalls",
                "vin",
                "km8k2-cab4pu001140",
                "--year",
                "2023",
                "--country",
                "us",
            ],
        ),
        (
            "/v1/vin/KM8K2CAB0PU001140/recalls",
            vec!["recalls", "vin", common::KONA_BAD_CHECK],
        ),
        (
            "/v1/vin/1M8PDMPA9SP000001/recalls",
            vec!["recalls", "vin", common::COACH],
        ),
        (
            "/v1/vin/1A9100AA851881001/recalls",
            vec!["recalls", "vin", common::TRAILER],
        ),
        (
            "/v1/vin/ZZZK2CAB4PU001140/recalls",
            vec!["recalls", "vin", common::UNKNOWN],
        ),
        (
            "/v1/vin/KM8K2CAB4PUO01140/recalls",
            vec!["recalls", "vin", "KM8K2CAB4PUO01140"],
        ),
        (
            "/v1/recalls/nhtsa/19V888000",
            vec!["recalls", "campaign", "nhtsa", "19V888000"],
        ),
        (
            "/v1/recalls/nhtsa/23v123000",
            vec!["recalls", "campaign", "nhtsa", "23v123000"],
        ),
        (
            "/v1/recalls/tc/2023101",
            vec!["recalls", "campaign", "tc", "2023101"],
        ),
        (
            "/v1/recalls/nhtsa/68V023000",
            vec!["recalls", "campaign", "nhtsa", "68V023000"],
        ),
        (
            "/v1/recalls/nhtsa/19V600000",
            vec!["recalls", "campaign", "nhtsa", "19V600000"],
        ),
        (
            "/v1/recalls/nhtsa/00V000000",
            vec!["recalls", "campaign", "nhtsa", "00V000000"],
        ),
        (
            "/v1/recalls/epa/19V888000",
            vec!["recalls", "campaign", "epa", "19V888000"],
        ),
        ("/v1/recalls", vec!["recalls", "search"]),
        (
            "/v1/recalls?limit=2",
            vec!["recalls", "search", "--limit", "2"],
        ),
        (
            "/v1/recalls?limit=4&cursor=2022-11-20_2",
            vec![
                "recalls",
                "search",
                "--limit",
                "4",
                "--cursor",
                "2022-11-20_2",
            ],
        ),
        (
            "/v1/recalls?make=hyundai&year=2023",
            vec!["recalls", "search", "--make", "hyundai", "--year", "2023"],
        ),
        (
            "/v1/recalls?make=chevy&model=silverado&country=ca",
            vec![
                "recalls",
                "search",
                "--make",
                "chevy",
                "--model",
                "silverado",
                "--country",
                "ca",
            ],
        ),
        (
            "/v1/recalls?q=wiring%20harness&component=electrical",
            vec![
                "recalls",
                "search",
                "wiring",
                "harness",
                "--component",
                "electrical",
            ],
        ),
        (
            "/v1/recalls?make=delorean",
            vec!["recalls", "search", "--make", "delorean"],
        ),
        (
            "/v1/recalls?model=kona",
            vec!["recalls", "search", "--model", "kona"],
        ),
        (
            "/v1/recalls?cursor=nonsense",
            vec!["recalls", "search", "--cursor", "nonsense"],
        ),
    ];
    for (path, args) in questions {
        let (status, served) = app.json(path).await;
        let ask = |source: &'static str| {
            let directory = directory.path().to_path_buf();
            let api = api.clone();
            let mut args: Vec<String> = args.iter().map(|arg| (*arg).to_owned()).collect();
            args.push(source.to_owned());
            tokio::task::spawn_blocking(move || {
                let args: Vec<&str> = args.iter().map(String::as_str).collect();
                tool(&directory, &api, &args)
            })
        };
        let (local_code, local) = ask("--offline").await.unwrap();
        let (remote_code, remote) = ask("--online").await.unwrap();
        if status == StatusCode::OK {
            assert_eq!((local_code, remote_code), (0, 0), "{path}");
            assert_eq!(served["may_apply"], true, "{path}");
            // Over HTTP the tool prints what the API sent, untouched.
            assert_eq!(remote, served, "{path}");
            assert_eq!(local, without_maker(served), "{path}");
        } else {
            // The same code and the same sentence, and the same exit code
            // both ways. The tool may add a hint to the details.
            assert_eq!(local_code, remote_code, "{path}");
            assert_ne!(local_code, 0, "{path}");
            for told in [&local, &remote] {
                assert_eq!(told["error"]["code"], served["error"]["code"], "{path}");
                assert_eq!(
                    told["error"]["message"], served["error"]["message"],
                    "{path}"
                );
            }
        }
    }

    // The words and the addresses the files carry are the server's own.
    assert_eq!(
        wenmar_shopdata::answer::NHTSA_LOOKUP,
        open_server::api::recalls::NHTSA_LOOKUP
    );
    assert_eq!(
        wenmar_shopdata::answer::TC_LOOKUP,
        open_server::api::recalls::TC_LOOKUP
    );
    assert_eq!(
        wenmar_shopdata::answer::NO_CATALOG_VEHICLE,
        open_server::api::recalls::NO_CATALOG_VEHICLE
    );

    let _ = stop.send(());
    let _ = serving.await;
}

/// The blocks of an overview that the hosted API fills where its server
/// holds their data, and that the files do not give yet: from files they
/// are `null` whatever the shop file holds. `specs`, `fuel` and `fluids`
/// wait for the plan that brings the fuel data to the files, and `safety`
/// for plan D3.
const HOSTED_ONLY: [&str; 4] = ["specs", "fuel", "fluids", "safety"];

/// An answer of the API as the files give it. What is set aside before
/// two answers are compared, and why:
///
/// - `lookup.maker`, `recalls.lookup.maker` and `links.maker`: the makers'
///   own lookup and links are compiled into the server from
///   `data/catalog/makes.yaml` and are in neither file.
/// - the blocks of [`HOSTED_ONLY`].
///
/// Nothing else is: every other field, the other `null` blocks among them,
/// must be the same. The shop file of these tests holds recalls only, so
/// `attribution` is compared whole. A test over a shop file that holds the
/// data of a [`HOSTED_ONLY`] block must set that block's credit lines
/// aside too.
fn as_the_files_give_it(mut answer: Value) -> Value {
    for place in ["/lookup/maker", "/recalls/lookup/maker", "/links/maker"] {
        if let Some(maker) = answer.pointer_mut(place) {
            *maker = Value::Null;
        }
    }
    for block in HOSTED_ONLY {
        if let Some(held) = answer.get_mut(block) {
            *held = Value::Null;
        }
    }
    answer
}

#[tokio::test(flavor = "multi_thread")]
async fn the_tool_answers_overview_questions_as_the_api_does_from_files_and_over_http() {
    // Once with the shop file and once with the data file alone: an
    // overview answers either way, and both must agree with the API.
    for with_shop in [true, false] {
        // The API, saying its pages are where the tool says they are.
        let app = if with_shop {
            common::recalls_app_at(SITE).await
        } else {
            common::app_with(|config| config.base_url = SITE.to_owned()).await
        };
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address: SocketAddr = listener.local_addr().unwrap();
        let (stop, stopped) = oneshot::channel::<()>();
        let serving = tokio::spawn(serve(
            listener,
            app.router.clone(),
            async move {
                let _ = stopped.await;
            },
            Limits::default(),
        ));
        let api = format!("http://{address}");

        // The same files, under the names the tool looks for.
        let fixture = common::data_file();
        let directory = tempfile::tempdir().unwrap();
        std::fs::copy(fixture.path(), directory.path().join(DATA_FILE)).unwrap();
        if with_shop {
            common::add_shop_file(
                &fixture,
                wenmar_shopdata::schema::SCHEMA_VERSION,
                "2026.09",
                common::RECALL_DATASETS,
                &common::recall_rows(),
            );
            std::fs::copy(fixture.shop_path(), directory.path().join(SHOP_FILE)).unwrap();
        }

        // The API's address, then the tool's words, separated by one space.
        let questions = [
            (
                "/v1/vehicles/2023_hyundai_kona/overview",
                "vehicles overview 2023_hyundai_kona",
            ),
            (
                "/v1/vehicles/2023_hyundai_kona_se/overview",
                "vehicles overview 2023_hyundai_kona_se",
            ),
            (
                "/v1/vehicles/2019_chevrolet_silverado/overview",
                "vehicles overview 2019_chevrolet_silverado",
            ),
            (
                "/v1/vehicles/2019_honda_civic/overview",
                "vehicles overview 2019_honda_civic",
            ),
            (
                "/v1/vehicles/2019_honda_nothing/overview",
                "vehicles overview 2019_honda_nothing",
            ),
            (
                "/v1/vin/KM8K2CAB4PU001140/overview",
                "vin overview KM8K2CAB4PU001140",
            ),
            (
                "/v1/vin/KM8K2CAB4PU001140/overview?year=2023",
                "vin overview km8k2-cab4pu001140 --year 2023",
            ),
            (
                "/v1/vin/KM8K2CAB0PU001140/overview",
                "vin overview KM8K2CAB0PU001140",
            ),
            (
                "/v1/vin/1M8PDMPA9SP000001/overview",
                "vin overview 1M8PDMPA9SP000001",
            ),
            (
                "/v1/vin/1A9100AA851881001/overview",
                "vin overview 1A9100AA851881001",
            ),
            (
                "/v1/vin/ZZZK2CAB4PU001140/overview",
                "vin overview ZZZK2CAB4PU001140",
            ),
            (
                "/v1/vin/KM8K2CAB4PUO01140/overview",
                "vin overview KM8K2CAB4PUO01140",
            ),
            (
                "/v1/models/hyundai/kona",
                "vehicles model-years --make hyundai --model kona",
            ),
            (
                "/v1/models/chevy/Silverado",
                "vehicles model-years --make chevy --model Silverado",
            ),
            (
                "/v1/models/ford/f150",
                "vehicles model-years --make ford --model f150",
            ),
            (
                "/v1/models/honda/civic",
                "vehicles model-years --make honda --model civic",
            ),
            (
                "/v1/models/honda/nothing",
                "vehicles model-years --make honda --model nothing",
            ),
            (
                "/v1/models/nobody/civic",
                "vehicles model-years --make nobody --model civic",
            ),
            (
                "/v1/recalls/recent?since=2019-01-01",
                "recalls recent --since 2019-01-01",
            ),
            (
                "/v1/recalls/recent?since=2019-01-01&country=ca",
                "recalls recent --since 2019-01-01 --country CA",
            ),
            (
                "/v1/recalls/recent?since=2019-01-01&make=chevy",
                "recalls recent --since 2019-01-01 --make chevy",
            ),
            (
                "/v1/recalls/recent?since=2019-01-01&make=nobody",
                "recalls recent --since 2019-01-01 --make nobody",
            ),
            (
                "/v1/recalls/recent?since=1960-01-01&limit=2",
                "recalls recent --since 1960-01-01 --limit 2",
            ),
            (
                "/v1/recalls/recent?since=1960-01-01&limit=2&cursor=2023-03-10_9",
                "recalls recent --since 1960-01-01 --limit 2 --cursor 2023-03-10_9",
            ),
            (
                "/v1/recalls/recent?since=2024-05-11",
                "recalls recent --since 2024-05-11",
            ),
            (
                "/v1/recalls/recent?since=yesterday",
                "recalls recent --since yesterday",
            ),
            (
                "/v1/recalls/recent?since=2019-01-01&country=mx",
                "recalls recent --since 2019-01-01 --country mx",
            ),
            (
                "/v1/recalls/recent?since=2019-01-01&cursor=nope",
                "recalls recent --since 2019-01-01 --cursor nope",
            ),
        ];
        let mut answered = 0;
        for (path, args) in questions {
            let (status, served) = app.json(path).await;
            let ask = |source: &'static str| {
                let directory = directory.path().to_path_buf();
                let api = api.clone();
                let mut args: Vec<String> = args.split(' ').map(str::to_owned).collect();
                args.push(source.to_owned());
                tokio::task::spawn_blocking(move || {
                    let args: Vec<&str> = args.iter().map(String::as_str).collect();
                    tool(&directory, &api, &args)
                })
            };
            let (local_code, local) = ask("--offline").await.unwrap();
            let (remote_code, remote) = ask("--online").await.unwrap();
            if status == StatusCode::OK {
                answered += 1;
                assert_eq!((local_code, remote_code), (0, 0), "{path} {with_shop}");
                // Over HTTP the tool prints what the API sent, untouched.
                assert_eq!(remote, served, "{path} {with_shop}");
                assert_eq!(
                    local,
                    as_the_files_give_it(served.clone()),
                    "{path} {with_shop}"
                );
                // An overview has a recall block only with a shop file, and
                // every block the files do not hold is null on both sides.
                if path.ends_with("/overview") || path.contains("/overview?") {
                    for block in HOSTED_ONLY {
                        assert_eq!(local[block], Value::Null, "{path} {block}");
                    }
                    assert_eq!(
                        served["recalls"].is_object(),
                        with_shop && served["vehicle"].is_object(),
                        "{path}"
                    );
                }
            } else {
                // The same code and the same sentence, and the same exit
                // code both ways. The tool may add a hint to the details.
                assert_eq!(local_code, remote_code, "{path} {with_shop}");
                assert_ne!(local_code, 0, "{path}");
                for told in [&local, &remote] {
                    assert_eq!(told["error"]["code"], served["error"]["code"], "{path}");
                    assert_eq!(
                        told["error"]["message"], served["error"]["message"],
                        "{path}"
                    );
                }
            }
        }
        // With the shop file 20 of the 28 are answers. Without it the 7 of
        // them that are recent lists are refused, which leaves 13.
        assert_eq!(answered, if with_shop { 20 } else { 13 }, "{with_shop}");

        let _ = stop.send(());
        let _ = serving.await;
    }
    assert_eq!(
        wenmar_shopdata::answer::NEWEST,
        open_server::api::overview::NEWEST
    );
    assert_eq!(
        wenmar_shopdata::answer::MOST_VEHICLES_EACH,
        open_server::api::overview::MOST_VEHICLES_EACH
    );
}
