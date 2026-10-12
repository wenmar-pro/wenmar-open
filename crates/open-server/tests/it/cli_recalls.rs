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
