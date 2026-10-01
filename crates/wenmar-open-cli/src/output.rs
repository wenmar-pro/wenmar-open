//! What is written to standard output and standard error.

use std::io::Write;

use serde_json::Value;

use crate::error::CliError;
use crate::jq;

/// How answers are written.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Mode {
    /// Text for a person.
    Text,
    /// JSON. Indented at a terminal, one line otherwise.
    Json { pretty: bool },
    /// The JSON, filtered with a jq expression.
    Jq(String),
}

impl Mode {
    /// `--jq` wins, then `--json`. Without either: text at a terminal and
    /// JSON anywhere else.
    pub fn choose(json: bool, jq: Option<&str>, stdout_terminal: bool) -> Mode {
        match jq {
            Some(expression) => Mode::Jq(expression.to_owned()),
            None if json || !stdout_terminal => Mode::Json {
                pretty: stdout_terminal,
            },
            None => Mode::Text,
        }
    }

    /// The mode before the command line has been read, for an error in
    /// the command line itself: JSON unless a person is watching.
    pub fn before_parsing(args: &[std::ffi::OsString], stdout_terminal: bool) -> Mode {
        let asked = args.iter().any(|arg| {
            arg == "--json" || arg == "--jq" || arg.to_string_lossy().starts_with("--jq=")
        });
        if asked || !stdout_terminal {
            Mode::Json {
                pretty: stdout_terminal,
            }
        } else {
            Mode::Text
        }
    }
}

fn write(out: &mut dyn Write, text: &str) -> Result<(), CliError> {
    out.write_all(text.as_bytes())
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

/// Writes an answer to standard output. `text` makes the text for a
/// person, and is only called when that is what is wanted.
pub fn answer(
    out: &mut dyn Write,
    mode: &Mode,
    value: &Value,
    text: impl FnOnce(&Value) -> String,
) -> Result<(), CliError> {
    match mode {
        Mode::Text => write(out, &text(value)),
        Mode::Json { pretty } => write(out, &(json_text(value, *pretty) + "\n")),
        Mode::Jq(expression) => {
            let mut lines = jq::filter(expression, value)?.join("\n");
            if !lines.is_empty() {
                lines.push('\n');
            }
            write(out, &lines)
        }
    }
}

/// Writes an error to standard error: the API's error object as JSON, or a
/// line or two of text for a person.
pub fn error(err: &mut dyn Write, mode: &Mode, error: &CliError) {
    let text = match mode {
        Mode::Text => {
            let mut text = format!("error: {}\n", crate::render::clean(&error.message));
            if let Some(hint) = error.hint() {
                text.push_str(&crate::render::clean(hint));
                text.push('\n');
            }
            text
        }
        Mode::Json { pretty } => json_text(&error.body(), *pretty) + "\n",
        Mode::Jq(_) => json_text(&error.body(), false) + "\n",
    };
    // There is nowhere left to report a failure to write this.
    let _ = write(err, &text);
}

#[cfg(test)]
mod tests {
    use std::ffi::OsString;
    use std::io;

    use serde_json::json;

    use super::*;

    #[test]
    fn json_when_piped_text_at_a_terminal_and_flags_override() {
        assert_eq!(Mode::choose(false, None, true), Mode::Text);
        assert_eq!(
            Mode::choose(false, None, false),
            Mode::Json { pretty: false }
        );
        assert_eq!(Mode::choose(true, None, true), Mode::Json { pretty: true });
        assert_eq!(
            Mode::choose(true, Some(".id"), true),
            Mode::Jq(".id".to_owned())
        );
    }

    #[test]
    fn a_bad_command_line_is_reported_in_the_form_asked_for() {
        let args = |list: &[&str]| -> Vec<OsString> { list.iter().map(OsString::from).collect() };
        assert_eq!(
            Mode::before_parsing(&args(&["x", "nope"]), true),
            Mode::Text
        );
        assert_eq!(
            Mode::before_parsing(&args(&["x", "nope"]), false),
            Mode::Json { pretty: false }
        );
        for flag in ["--json", "--jq", "--jq=.id"] {
            assert_eq!(
                Mode::before_parsing(&args(&["x", flag]), true),
                Mode::Json { pretty: true },
                "{flag}"
            );
        }
    }

    /// A reader that has gone away.
    struct Closed;

    impl Write for Closed {
        fn write(&mut self, _: &[u8]) -> io::Result<usize> {
            Err(io::ErrorKind::BrokenPipe.into())
        }

        fn flush(&mut self) -> io::Result<()> {
            Err(io::ErrorKind::BrokenPipe.into())
        }
    }

    #[test]
    fn a_closed_pipe_is_its_own_outcome_in_every_mode() {
        for mode in [
            Mode::Text,
            Mode::Json { pretty: false },
            Mode::Jq(".".to_owned()),
        ] {
            let error =
                answer(&mut Closed, &mode, &json!([1]), |_| "text\n".to_owned()).unwrap_err();
            assert_eq!(error.code, "pipe_closed", "{mode:?}");
            assert_eq!(error.exit_code(), 0);
        }
        // Writing an error to a closed pipe does nothing, quietly.
        error(&mut Closed, &Mode::Text, &CliError::new("io", "m"));
    }

    #[test]
    fn an_error_is_text_for_a_person_and_json_otherwise() {
        let failure = CliError::new("no_data", "There is no data file.").with_hint("Pull it.");
        let mut text = Vec::new();
        error(&mut text, &Mode::Text, &failure);
        assert_eq!(
            String::from_utf8(text).unwrap(),
            "error: There is no data file.\nPull it.\n"
        );
        for mode in [Mode::Json { pretty: false }, Mode::Jq(".".to_owned())] {
            let mut json = Vec::new();
            error(&mut json, &mode, &failure);
            let body: Value = serde_json::from_slice(&json).unwrap();
            assert_eq!(body, failure.body());
        }
    }

    #[test]
    fn jq_with_no_results_writes_nothing() {
        let mut out = Vec::new();
        answer(&mut out, &Mode::Jq("empty".to_owned()), &json!(1), |_| {
            String::new()
        })
        .unwrap();
        assert!(out.is_empty());
    }
}
