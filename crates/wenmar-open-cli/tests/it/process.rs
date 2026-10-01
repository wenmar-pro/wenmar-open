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
