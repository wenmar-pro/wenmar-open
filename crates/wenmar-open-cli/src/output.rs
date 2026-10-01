//! What is written to standard output and standard error.

use std::io::Write;

use serde_json::Value;

use crate::error::CliError;

/// How answers are written.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Mode {
    /// JSON. Indented at a terminal, one line otherwise.
    Json { pretty: bool },
}

impl Mode {
    pub fn choose(stdout_terminal: bool) -> Mode {
        Mode::Json {
            pretty: stdout_terminal,
        }
    }
}

fn line(out: &mut dyn Write, text: &str) -> Result<(), CliError> {
    out.write_all(text.as_bytes())
        .and_then(|()| out.write_all(b"\n"))
        .and_then(|()| out.flush())
        .map_err(|error| CliError::from_write(&error))
}

fn json_text(value: &Value, pretty: bool) -> String {
    let text = if pretty {
        serde_json::to_string_pretty(value)
    } else {
        serde_json::to_string(value)
    };
    // A `Value` always serializes.
    text.unwrap_or_else(|_| "null".to_owned())
}

/// Writes an answer to standard output.
pub fn answer(out: &mut dyn Write, mode: &Mode, value: &Value) -> Result<(), CliError> {
    match mode {
        Mode::Json { pretty } => line(out, &json_text(value, *pretty)),
    }
}

/// Writes an error to standard error: the API's error object as JSON.
pub fn error(err: &mut dyn Write, mode: &Mode, error: &CliError) {
    let text = match mode {
        Mode::Json { pretty } => json_text(&error.body(), *pretty),
    };
    // There is nowhere left to report a failure to write this.
    let _ = line(err, &text);
}
