use std::io::Write;
use std::path::Path;

use flate2::Compression;
use flate2::write::GzEncoder;
use serde_json::{Value, json};
use wenmar_open_cli::env::Env;

use crate::common::{self, Fixture, KONA};
use crate::server::{self, Handler, Reply, Seen, Server};

fn gzip(bytes: &[u8]) -> Vec<u8> {
    let mut encoder = GzEncoder::new(Vec::new(), Compression::default());
    encoder.write_all(bytes).unwrap();
    encoder.finish().unwrap()
}

/// A data file of the given versions, gzipped as a release carries it.
fn packed(schema_version: &str, data_version: &str) -> Vec<u8> {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("built.sqlite3");
    common::write_data_file(&path, schema_version, data_version);
    gzip(&std::fs::read(path).unwrap())
}

fn release(host: &str, version: &str) -> Value {
    let name = format!("wenmar-open-{version}.sqlite3.gz");
    json!({
        "tag_name": format!("data-{version}"),
        "draft": false,
        "prerelease": false,
        "assets": [
            { "name": format!("{name}.sha256"), "browser_download_url": format!("http://{host}/assets/{name}.sha256") },
            { "name": name, "browser_download_url": format!("http://{host}/assets/{name}") }
        ]
    })
}

/// A release server with data releases 2026.08 and 2026.09 and one release
/// of the tool itself. `asset` answers the download of 2026.09.
fn releases(asset: impl Fn() -> Reply + Send + 'static) -> Server {
    let older = packed("3", "2026.08");
    server::serve(move |request: &Seen| {
        let host = request.header("host").unwrap_or_default().to_owned();
        match request.target.as_str() {
            "/releases?per_page=100" => Reply::json(
                200,
                &json!([
                    { "tag_name": "v0.2.0", "draft": false, "prerelease": false, "assets": [] },
                    release(&host, "2026.08"),
                    release(&host, "2026.09")
                ]),
            ),
            "/releases/tags/data-2026.08" => Reply::json(200, &release(&host, "2026.08")),
            "/assets/wenmar-open-2026.08.sqlite3.gz" => Reply::bytes(older.clone()),
            "/assets/wenmar-open-2026.09.sqlite3.gz" => asset(),
            _ => Reply::json(404, &json!({ "message": "Not Found" })),
        }
    })
}

fn good() -> Server {
    let file = packed("3", "2026.09");
    releases(move || Reply::bytes(file.clone()))
}

fn env(fixture: &Fixture, server: &Server) -> Env {
    Env {
        releases: Some(format!("{}/releases", server.url())),
        ..common::env(fixture)
    }
}

#[test]
fn pull_downloads_the_newest_release_and_puts_it_in_place() {
    let server = good();
    let fixture = common::empty_dir();
    let env = env(&fixture, &server);

    let run = common::run(&env, &["data", "pull"]);
    assert_eq!(run.code, 0, "{}", run.stderr);
    let answer = run.json();
    assert_eq!(answer["updated"], true);
    assert_eq!(answer["data_version"], "2026.09");
    assert_eq!(answer["path"], fixture.file().display().to_string());
    assert_eq!(
        answer["bytes"],
        std::fs::metadata(fixture.file()).unwrap().len()
    );
    assert!(answer.get("previous").is_none());
    // Only the data file is left: no part-downloaded file, no archive.
    assert_eq!(fixture.files(), ["wenmar-open.sqlite3"]);
    assert_eq!(
        server.targets(),
        [
            "/releases?per_page=100",
            "/assets/wenmar-open-2026.09.sqlite3.gz"
        ]
    );
    // And it answers.
    let run = common::run(&env, &["vin", "decode", KONA, "--offline"]);
    assert_eq!(run.json()["model"], "Kona");
}

#[test]
fn pulling_again_downloads_nothing_unless_forced() {
    let server = good();
    let fixture = common::empty_dir();
    let env = env(&fixture, &server);
    common::run(&env, &["data", "pull"]);

    let run = common::run(&env, &["data", "pull"]);
    assert_eq!(run.code, 0);
    assert_eq!(run.json()["updated"], false);
    assert_eq!(run.json()["data_version"], "2026.09");
    assert_eq!(server.seen().len(), 3, "{:?}", server.targets());

    let run = common::run(&env, &["data", "pull", "--force"]);
    assert_eq!(run.json()["updated"], true);
    assert_eq!(server.seen().len(), 5);
}

#[test]
fn a_version_can_be_asked_for_and_replaces_the_one_in_place() {
    let server = good();
    let fixture = common::empty_dir();
    let env = env(&fixture, &server);
    common::run(&env, &["data", "pull"]);

    let run = common::run(&env, &["data", "pull", "2026.08"]);
    assert_eq!(run.code, 0, "{}", run.stderr);
    assert_eq!(run.json()["data_version"], "2026.08");
    assert_eq!(run.json()["previous"], "2026.09");
    let run = common::run(&env, &["data", "status"]);
    assert_eq!(run.json()["data_version"], "2026.08");

    let run = common::run(&env, &["data", "pull", "2025.01"]);
    assert_eq!(run.code, 3);
    assert_eq!(
        run.error()["error"]["message"],
        "There is no data release 2025.01."
    );
    // Text that is not a version never reaches an address.
    let before = server.seen().len();
    for version in ["../../x", "latest", "2026.9", "2026.09?x=1", ""] {
        let run = common::run(&env, &["data", "pull", version]);
        assert_eq!(run.code, 4, "{version}");
        assert_eq!(run.error()["error"]["code"], "validation_failed");
    }
    assert_eq!(server.seen().len(), before);
}

/// Pulls with `asset` as the download, over a data file that is already
/// in place, and checks that the failure changed nothing.
fn failed_pull(asset: impl Fn() -> Reply + Send + 'static, code: &str, exit: u8) -> Value {
    let server = releases(asset);
    let fixture = common::empty_dir();
    common::write_data_file(&fixture.file(), "3", "2026.07");
    let before = std::fs::read(fixture.file()).unwrap();

    let run = common::run(&env(&fixture, &server), &["data", "pull"]);
    assert_eq!(run.stdout, "");
    let error = run.error();
    assert_eq!(error["error"]["code"], code, "{error}");
    assert_eq!(run.code, exit);
    // The old data file is as it was, and nothing was left beside it.
    assert_eq!(fixture.files(), ["wenmar-open.sqlite3"]);
    assert_eq!(std::fs::read(fixture.file()).unwrap(), before);
    error
}

#[test]
fn an_interrupted_download_changes_nothing() {
    let file = packed("3", "2026.09");
    let half = file.len() / 2;
    // The connection drops half way through the body.
    let error = failed_pull(
        move || Reply {
            claimed_length: Some(file.len()),
            ..Reply::bytes(file[..half].to_vec())
        },
        "download_failed",
        1,
    );
    assert_eq!(
        error["error"]["details"]["hint"],
        "Nothing was changed. Run `wenmar-open data pull` again."
    );
}

/// A server that reads a request, sends `sent`, and then holds the
/// connection open without another byte for as long as the tests run.
/// Returns its host and port.
fn stalling(sent: Vec<u8>) -> String {
    use std::io::{BufRead, BufReader};

    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let host = listener.local_addr().unwrap().to_string();
    std::thread::spawn(move || {
        let mut held = Vec::new();
        for stream in listener.incoming() {
            let Ok(mut stream) = stream else { continue };
            let mut reader = BufReader::new(stream.try_clone().unwrap());
            loop {
                let mut line = String::new();
                if reader.read_line(&mut line).unwrap_or(0) == 0 || line == "\r\n" {
                    break;
                }
            }
            let _ = stream.write_all(&sent);
            let _ = stream.flush();
            held.push(stream);
        }
    });
    host
}

#[test]
fn a_download_that_stops_arriving_ends_and_changes_nothing() {
    use std::time::Duration;

    use wenmar_open_cli::pull::{self, Patience};

    let file = packed("3", "2026.09");
    // A quarter of the body arrives and then nothing more does.
    let mut part_way = format!(
        "HTTP/1.1 200 OK\r\nContent-Type: application/gzip\r\nContent-Length: {}\r\n\r\n",
        file.len()
    )
    .into_bytes();
    part_way.extend_from_slice(&file[..file.len() / 4]);
    // Or the server takes the request and never answers it.
    let silent = Vec::new();

    for sent in [part_way, silent] {
        let host = stalling(sent);
        let server = server::serve(move |request: &Seen| match request.target.as_str() {
            "/releases?per_page=100" => Reply::json(200, &json!([release(&host, "2026.09")])),
            _ => Reply::json(404, &json!({ "message": "Not Found" })),
        });
        let fixture = common::empty_dir();
        common::write_data_file(&fixture.file(), "3", "2026.07");
        let before = std::fs::read(fixture.file()).unwrap();

        // The pull runs on a thread of its own, so that one which waits
        // for ever fails this test and does not hang it.
        let directory = fixture.directory().to_path_buf();
        let releases = format!("{}/releases", server.url());
        let patience = Patience {
            response: Duration::from_millis(500),
            body: Duration::from_millis(500),
        };
        let (done, outcome) = std::sync::mpsc::channel();
        std::thread::spawn(move || {
            let _ = done.send(pull::pull_with(
                &directory, &releases, None, false, patience,
            ));
        });
        let error = outcome
            .recv_timeout(Duration::from_secs(20))
            .expect("the download was still waiting after 20 seconds")
            .unwrap_err();
        assert_eq!(error.code, "download_failed", "{error:?}");
        assert_eq!(error.exit_code(), 1);
        assert_eq!(
            error.hint(),
            Some("Nothing was changed. Run `wenmar-open data pull` again.")
        );
        assert_eq!(fixture.files(), ["wenmar-open.sqlite3"]);
        assert_eq!(std::fs::read(fixture.file()).unwrap(), before);
    }
}

#[test]
fn a_corrupted_download_changes_nothing() {
    let file = packed("3", "2026.09");
    // Cut short, with a length that agrees.
    let cut = file[..file.len() / 2].to_vec();
    failed_pull(move || Reply::bytes(cut.clone()), "download_failed", 1);
    // A byte changed in the middle: the archive's own checksum catches it.
    let mut flipped = file.clone();
    let middle = flipped.len() / 2;
    flipped[middle] ^= 0xff;
    failed_pull(move || Reply::bytes(flipped.clone()), "download_failed", 1);
    // Not an archive at all: a proxy's error page with status 200.
    failed_pull(
        || Reply::html(200, "<html>Service unavailable</html>"),
        "download_failed",
        1,
    );
    failed_pull(|| Reply::bytes(Vec::new()), "download_failed", 1);
    failed_pull(|| Reply::html(503, "busy"), "download_failed", 1);
    // An archive of something that is not a database.
    let other = gzip(b"this is not a database");
    failed_pull(move || Reply::bytes(other.clone()), "data_invalid", 11);
}

#[test]
fn a_release_of_another_schema_version_is_refused_and_the_old_file_kept() {
    let newer = packed("4", "2026.09");
    let error = failed_pull(move || Reply::bytes(newer.clone()), "data_invalid", 11);
    assert_eq!(
        error["error"]["message"],
        "The downloaded file cannot be used: it has schema version 4 and this build reads version 3."
    );
    assert_eq!(
        error["error"]["details"]["hint"],
        "This data release is for another version of wenmar-open. Update wenmar-open, or name an older release, as in `wenmar-open data pull 2026.08`."
    );
    // A file that says it is another data version than its release.
    let mislabelled = packed("3", "2026.01");
    let error = failed_pull(
        move || Reply::bytes(mislabelled.clone()),
        "data_invalid",
        11,
    );
    assert_eq!(
        error["error"]["message"],
        "The downloaded file cannot be used: it says it is data version 2026.01, and the release is 2026.09."
    );
}

#[test]
fn a_release_server_that_is_down_limited_or_wrong_is_said_plainly() {
    let fixture = common::empty_dir();
    // Down.
    let run = common::run(&common::env(&fixture), &["data", "pull"]);
    assert_eq!(run.code, 10);
    assert_eq!(run.error()["error"]["code"], "network");

    let cases: Vec<(Handler, &str, u8)> = vec![
        (
            Box::new(|_| Reply::json(403, &json!({ "message": "API rate limit exceeded" }))),
            "rate_limited",
            5,
        ),
        (Box::new(|_| Reply::json(200, &json!([]))), "not_found", 3),
        (
            Box::new(|_| Reply::html(200, "<html>Sign in</html>")),
            "bad_response",
            6,
        ),
        (
            Box::new(|_| Reply::json(200, &json!({ "message": "Not Found" }))),
            "bad_response",
            6,
        ),
    ];
    for (handler, code, exit) in cases {
        let server = server::serve(handler);
        let run = common::run(&env(&fixture, &server), &["data", "pull"]);
        assert_eq!(run.error()["error"]["code"], code);
        assert_eq!(run.code, exit, "{code}");
    }
    let run = common::run(
        &common::env(&fixture),
        &["data", "pull", "--releases", "file:///etc/passwd"],
    );
    assert_eq!(run.code, 2);
    // Nothing was created by any of it.
    assert!(fixture.files().is_empty());
}

#[test]
fn nothing_is_written_outside_the_data_directory() {
    let server = good();
    let root = tempfile::tempdir().unwrap();
    let directory = root.path().join("a").join("data");
    let env = Env {
        data_dir: Some(directory.clone()),
        releases: Some(format!("{}/releases", server.url())),
        ..Env::default()
    };
    let run = common::run(&env, &["data", "pull"]);
    assert_eq!(run.code, 0, "{}", run.stderr);
    // The directory was made, with its parents, and holds one file.
    let list = |path: &Path| -> Vec<String> {
        let mut names: Vec<String> = std::fs::read_dir(path)
            .unwrap()
            .map(|entry| entry.unwrap().file_name().to_string_lossy().into_owned())
            .collect();
        names.sort();
        names
    };
    assert_eq!(list(root.path()), ["a"]);
    assert_eq!(list(&root.path().join("a")), ["data"]);
    assert_eq!(list(&directory), ["wenmar-open.sqlite3"]);
    // --data-dir does the same as the variable.
    let other = root.path().join("b");
    let run = common::run(
        &env,
        &["data", "pull", "--data-dir", other.to_str().unwrap()],
    );
    assert_eq!(run.code, 0, "{}", run.stderr);
    assert_eq!(list(&other), ["wenmar-open.sqlite3"]);
}

#[cfg(unix)]
#[test]
fn a_data_directory_that_cannot_be_written_is_an_error_that_says_what_to_set() {
    use std::os::unix::fs::PermissionsExt;

    let server = good();
    let fixture = common::empty_dir();
    std::fs::set_permissions(fixture.directory(), std::fs::Permissions::from_mode(0o555)).unwrap();
    // A process that may write anywhere, as root may, cannot test this.
    let writable = std::fs::write(fixture.directory().join("probe"), "").is_ok();
    if !writable {
        for directory in [
            fixture.directory().to_path_buf(),
            fixture.directory().join("below"),
        ] {
            let env = Env {
                data_dir: Some(directory),
                ..env(&fixture, &server)
            };
            let run = common::run(&env, &["data", "pull"]);
            assert_eq!(run.code, 1, "{}", run.stderr);
            let error = run.error();
            assert_eq!(error["error"]["code"], "io");
            assert_eq!(
                error["error"]["details"]["hint"],
                "Set WENMAR_OPEN_DATA_DIR to a directory you can write to, or pass --data-dir."
            );
        }
    }
    std::fs::set_permissions(fixture.directory(), std::fs::Permissions::from_mode(0o755)).unwrap();
}

#[test]
fn a_data_directory_that_is_a_file_is_an_error() {
    let server = good();
    let fixture = common::empty_dir();
    let file = fixture.directory().join("not-a-directory");
    std::fs::write(&file, "x").unwrap();
    let env = Env {
        data_dir: Some(file.clone()),
        ..env(&fixture, &server)
    };
    let run = common::run(&env, &["data", "pull"]);
    assert_eq!(run.code, 1);
    assert_eq!(run.error()["error"]["code"], "io");
    assert_eq!(std::fs::read_to_string(file).unwrap(), "x");
}

#[test]
fn with_no_home_directory_pull_and_status_say_what_to_set() {
    for args in [vec!["data", "pull"], vec!["data", "status"]] {
        let run = common::run(&Env::default(), &args);
        assert_eq!(run.code, 11, "{args:?}");
        assert_eq!(
            run.error()["error"]["details"]["hint"],
            "Set WENMAR_OPEN_DATA_DIR to a directory, or pass --data-dir."
        );
    }
}

#[test]
fn a_dead_downloads_leftovers_are_removed_and_a_live_ones_are_not() {
    let server = good();
    let fixture = common::empty_dir();
    let dead = fixture.directory().join("wenmar-open.sqlite3.111.part");
    let live = fixture.directory().join("wenmar-open.sqlite3.222.part");
    let unrelated = fixture.directory().join("notes.part");
    for path in [&dead, &live, &unrelated] {
        std::fs::write(path, "partial").unwrap();
    }
    let two_hours_ago = std::time::SystemTime::now() - std::time::Duration::from_secs(2 * 60 * 60);
    for path in [&dead, &unrelated] {
        std::fs::File::options()
            .write(true)
            .open(path)
            .unwrap()
            .set_modified(two_hours_ago)
            .unwrap();
    }
    let run = common::run(&env(&fixture, &server), &["data", "pull"]);
    assert_eq!(run.code, 0, "{}", run.stderr);
    assert_eq!(
        fixture.files(),
        [
            "notes.part",
            "wenmar-open.sqlite3",
            "wenmar-open.sqlite3.222.part"
        ]
    );
}

#[test]
fn status_says_what_is_there() {
    let fixture = common::data_dir();
    let run = common::run(&common::env(&fixture), &["data", "status"]);
    assert_eq!(run.code, 0);
    assert_eq!(
        run.json(),
        json!({
            "installed": true,
            "usable": true,
            "path": fixture.file().display().to_string(),
            "bytes": std::fs::metadata(fixture.file()).unwrap().len(),
            "data_version": "2026.09",
            "schema_version": "3",
            "reads_schema_version": "3",
            "vpic_release": "vPICList_lite_2026_09",
            "built_at": "2026-10-01 04:25:57"
        })
    );

    let empty = common::empty_dir();
    let run = common::run(&common::env(&empty), &["data", "status"]);
    assert_eq!(run.code, 0);
    assert_eq!(
        run.json(),
        json!({
            "installed": false,
            "usable": false,
            "path": empty.file().display().to_string(),
            "reads_schema_version": "3"
        })
    );
    assert!(empty.files().is_empty());

    let old = common::old_data_dir();
    let run = common::run(&common::env(&old), &["data", "status"]);
    assert_eq!(run.code, 0);
    let status = run.json();
    assert_eq!(status["installed"], true);
    assert_eq!(status["usable"], false);
    assert_eq!(status["schema_version"], "2");
    assert_eq!(
        status["problem"],
        "it has schema version 2 and this build reads version 3"
    );

    let terminal = Env {
        stdout_terminal: true,
        ..common::env(&old)
    };
    let run = common::run(&terminal, &["data", "status"]);
    assert_eq!(
        run.stdout,
        format!(
            "The data file at {} cannot be used: it has schema version 2 and this build reads version 3.\nRun `wenmar-open data pull` to replace it.\n",
            old.file().display()
        )
    );
}
