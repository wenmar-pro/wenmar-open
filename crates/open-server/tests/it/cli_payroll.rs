//! `wenmar-open payroll deductions`, asked three ways: the API directly,
//! the tool on this computer with no data file, and the tool over HTTP.
//! All three must give the same JSON. The figures are PDOC's, captured
//! 2026-10-14 for Ontario, pay date 2026-07-15.

use std::ffi::OsString;
use std::net::SocketAddr;
use std::path::Path;

use axum::http::StatusCode;
use open_server::serve::{Limits, serve};
use serde_json::Value;
use tokio::net::TcpListener;
use tokio::sync::oneshot;
use wenmar_open_cli::env::Env;

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

/// A query string as the command line of the same pay: `frequency=weekly`
/// is `--frequency weekly`, `pay_date` is `--pay-date`, and so on.
fn args_for(query: &str) -> Vec<String> {
    let mut args = vec!["payroll".to_owned(), "deductions".to_owned()];
    for pair in query.split('&') {
        let (name, value) = pair.split_once('=').unwrap();
        let flag = match name {
            "federal_claim" => "federal-claim",
            "provincial_claim" => "provincial-claim",
            "pay_date" => "pay-date",
            "ytd_cpp" => "ytd-cpp",
            "ytd_cpp2" => "ytd-cpp2",
            "ytd_ei" => "ytd-ei",
            "ytd_pensionable" => "ytd-pensionable",
            other => other,
        };
        args.push(format!("--{flag}"));
        args.push(value.to_owned());
    }
    args
}

#[tokio::test(flavor = "multi_thread")]
async fn the_api_the_tool_here_and_the_tool_online_give_one_answer() {
    let app = common::app().await;
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

    // Payroll needs no data file: an empty directory is the whole world.
    let directory = tempfile::tempdir().unwrap();

    // The three PDOC pays of the API's tests, and the one the tables refuse.
    let queries = [
        "frequency=weekly&gross=800.00&pay_date=2026-07-15",
        "frequency=biweekly&gross=2400.00&pay_date=2026-07-15",
        "frequency=weekly&gross=800&federal_claim=0&provincial_claim=0&pay_date=2026-07-15",
        "frequency=weekly&gross=800&pay_date=2027-01-15",
    ];
    for query in queries {
        let (status, served) = app.json(&format!("/v1/payroll/deductions?{query}")).await;
        let ask = |source: &'static str| {
            let directory = directory.path().to_path_buf();
            let api = api.clone();
            let args = args_for(query);
            let mut args = args;
            args.push(source.to_owned());
            tokio::task::spawn_blocking(move || {
                let args: Vec<&str> = args.iter().map(String::as_str).collect();
                tool(&directory, &api, &args)
            })
        };
        let (local_code, local) = ask("--offline").await.unwrap();
        let (remote_code, remote) = ask("--online").await.unwrap();
        if status == StatusCode::OK {
            assert_eq!((local_code, remote_code), (0, 0), "{query}");
            // Nothing is set aside: the answer has no address of this site
            // and no link of a maker's, so all three give every field.
            assert_eq!(remote, served, "{query}");
            assert_eq!(local, served, "{query}");
        } else {
            // The same code and the same sentence, and the same exit code
            // both ways. The tool may add a hint to the details.
            assert_eq!(local_code, remote_code, "{query}");
            assert_ne!(local_code, 0, "{query}");
            for told in [&local, &remote] {
                assert_eq!(told["error"]["code"], served["error"]["code"], "{query}");
                assert_eq!(
                    told["error"]["message"], served["error"]["message"],
                    "{query}"
                );
            }
        }
    }

    let _ = stop.send(());
    let _ = serving.await;
}
