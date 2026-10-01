//! The real binary, for what only a process can show.

use std::process::{Command, Stdio};

use crate::common::{self, KONA};

fn binary() -> Command {
    let mut command = Command::new(env!("CARGO_BIN_EXE_wenmar-open"));
    // Nothing of the developer's own surroundings reaches the tool.
    command
        .env("WENMAR_OPEN_API", common::NO_API)
        .env("WENMAR_OPEN_RELEASES", common::NO_API)
        .env_remove("WENMAR_OPEN_DATA_DIR")
        .stdin(Stdio::null());
    command
}

#[test]
fn the_binary_reads_its_data_directory_from_the_environment() {
    let fixture = common::data_dir();
    let output = binary()
        .env("WENMAR_OPEN_DATA_DIR", fixture.directory())
        .args(["vin", "decode", KONA])
        .output()
        .unwrap();
    assert!(output.status.success());
    let body: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(body["model"], "Kona");
    assert!(output.stderr.is_empty());
}

#[test]
fn a_closed_pipe_ends_the_process_quietly() {
    let fixture = common::data_dir();
    // The reading end is closed before the tool starts, as when the next
    // command in a pipeline has already exited.
    let (reader, writer) = std::io::pipe().unwrap();
    drop(reader);
    let output = binary()
        .env("WENMAR_OPEN_DATA_DIR", fixture.directory())
        .args(["vehicles", "makes"])
        .stdout(writer)
        .stderr(Stdio::piped())
        .output()
        .unwrap();
    assert_eq!(output.status.code(), Some(0));
    assert_eq!(String::from_utf8_lossy(&output.stderr), "");
}

#[cfg(unix)]
#[test]
fn an_argument_that_is_not_utf8_is_a_usage_error() {
    use std::ffi::OsString;
    use std::os::unix::ffi::OsStringExt;

    let fixture = common::data_dir();
    let output = binary()
        .env("WENMAR_OPEN_DATA_DIR", fixture.directory())
        .arg("vin")
        .arg("decode")
        .arg(OsString::from_vec(vec![0x4b, 0xff, 0xfe, 0x38]))
        .output()
        .unwrap();
    assert_eq!(output.status.code(), Some(2));
    assert!(output.stdout.is_empty());
    let error: serde_json::Value = serde_json::from_slice(&output.stderr).unwrap();
    assert_eq!(error["error"]["code"], "usage");
}

#[test]
fn an_exit_code_says_what_went_wrong() {
    let fixture = common::empty_dir();
    let output = binary()
        .env("WENMAR_OPEN_DATA_DIR", fixture.directory())
        .args(["vin", "decode", KONA, "--offline"])
        .output()
        .unwrap();
    assert_eq!(output.status.code(), Some(11));
    // With nothing to answer from, the API is tried, and it is not there.
    let output = binary()
        .env("WENMAR_OPEN_DATA_DIR", fixture.directory())
        .args(["vin", "decode", KONA])
        .output()
        .unwrap();
    assert_eq!(output.status.code(), Some(10));
}

#[test]
fn an_mcp_client_that_goes_away_ends_the_server_quietly() {
    use std::io::Write;

    let fixture = common::data_dir();
    let (reader, writer) = std::io::pipe().unwrap();
    drop(reader);
    let mut child = binary()
        .env("WENMAR_OPEN_DATA_DIR", fixture.directory())
        .arg("mcp")
        .stdin(Stdio::piped())
        .stdout(writer)
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    child
        .stdin
        .take()
        .unwrap()
        .write_all(b"{\"jsonrpc\":\"2.0\",\"id\":1,\"method\":\"ping\"}\n")
        .unwrap();
    let output = child.wait_with_output().unwrap();
    assert_eq!(output.status.code(), Some(0));
    assert_eq!(String::from_utf8_lossy(&output.stderr), "");
}

/// `--jq`, run by the real binary, which hands the expression to a process
/// of its own.
fn filtered(fixture: &common::Fixture, expression: &str) -> std::process::Output {
    binary()
        .env("WENMAR_OPEN_DATA_DIR", fixture.directory())
        .args(["vehicles", "years", "--jq", expression])
        .output()
        .unwrap()
}

#[test]
fn a_jq_expression_that_exhausts_the_stack_is_an_error_and_not_a_crash() {
    let fixture = common::data_dir();
    // Short and shallow, and it calls itself without end.
    let output = filtered(&fixture, "def f: 1 + f; f");
    assert_eq!(output.status.code(), Some(1), "{output:?}");
    assert!(output.stdout.is_empty());
    let error: serde_json::Value = serde_json::from_slice(&output.stderr)
        .unwrap_or_else(|_| panic!("{}", String::from_utf8_lossy(&output.stderr)));
    assert_eq!(error["error"]["code"], "jq_error");
    assert_eq!(
        error["error"]["message"],
        "the --jq expression stopped before it finished"
    );
}

#[test]
fn jq_in_a_process_of_its_own_answers_and_fails_as_it_does_in_this_one() {
    let fixture = common::data_dir();
    let output = filtered(&fixture, ".[0], length");
    assert_eq!(output.status.code(), Some(0));
    assert_eq!(String::from_utf8_lossy(&output.stdout), "2023\n5\n");
    assert!(output.stderr.is_empty());
    let output = filtered(&fixture, "empty");
    assert_eq!(output.status.code(), Some(0));
    assert!(output.stdout.is_empty());
    // An expression that starts with a dash is still an expression.
    let output = binary()
        .env("WENMAR_OPEN_DATA_DIR", fixture.directory())
        .args(["vehicles", "years", "--jq=-(.[0])"])
        .output()
        .unwrap();
    assert_eq!(String::from_utf8_lossy(&output.stdout), "-2023\n");

    let error = |output: &std::process::Output| -> serde_json::Value {
        assert!(output.stdout.is_empty());
        serde_json::from_slice(&output.stderr).unwrap()
    };
    let output = filtered(&fixture, ".[");
    assert_eq!(output.status.code(), Some(2));
    assert_eq!(
        error(&output),
        serde_json::json!({ "error": {
            "code": "usage",
            "message": "the --jq expression could not be read",
            "details": { "hint": "It is a jq expression, such as `.make` or `.[].id`." }
        } })
    );
    let output = filtered(&fixture, ".[0].x");
    assert_eq!(output.status.code(), Some(1));
    assert_eq!(
        error(&output)["error"]["message"],
        "the --jq expression failed: cannot index 2023 with \"x\""
    );
}
