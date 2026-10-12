//! The command-line tool's bulletin commands against this server. Asked
//! online, the tool prints what the API gives. The bulletins are not in
//! the files, so asked offline the tool says to ask online.

use std::ffi::OsString;
use std::net::SocketAddr;
use std::path::Path;

use axum::http::StatusCode;
use open_server::serve::{Limits, serve};
use serde_json::Value;
use tokio::net::TcpListener;
use tokio::sync::oneshot;
use wenmar_open_cli::env::{DATA_FILE, Env};

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

#[tokio::test(flavor = "multi_thread")]
async fn the_tool_answers_bulletin_questions_online_as_the_api_does() {
    let app = common::bulletins_app().await;
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
    let directory = tempfile::tempdir().unwrap();

    let (code, answer) = tool(
        directory.path(),
        &api,
        &["bulletins", "vehicle", "2023_hyundai_kona", "--online"],
    );
    assert_eq!(code, 0, "{answer}");
    let (status, served) = app.json("/v1/vehicles/2023_hyundai_kona/bulletins").await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(answer, served);
    assert_eq!(answer["counts"]["count"], 6);

    // With the data file beside it and no flag, the question still goes to
    // the API; with --offline the files cannot answer it.
    let fixture = common::data_file();
    std::fs::copy(fixture.path(), directory.path().join(DATA_FILE)).unwrap();
    let (code, answer) = tool(
        directory.path(),
        &api,
        &["bulletins", "get", "11000006", "--offline"],
    );
    assert_ne!(code, 0);
    assert_eq!(answer["error"]["code"], "dataset_unavailable");
    assert!(answer.to_string().contains("--online"), "{answer}");
    let (code, answer) = tool(directory.path(), &api, &["bulletins", "get", "11000006"]);
    assert_eq!(code, 0, "{answer}");
    let (_, served) = app.json("/v1/bulletins/11000006").await;
    assert_eq!(answer, served);

    let _ = stop.send(());
    let _ = serving.await;
}
