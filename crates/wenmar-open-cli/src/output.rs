//! What is written to standard output and standard error.

use std::io::Write;
use std::path::PathBuf;

use serde_json::Value;

use crate::env::Env;
use crate::error::CliError;
use crate::{jq, render};

/// How answers are written.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Mode {
    /// Text for a person.
    Text,
    /// JSON. Indented at a terminal, one line otherwise.
    Json { pretty: bool },
    /// The JSON, filtered with a jq expression.
    Jq {
        expression: String,
        /// Whether a terminal shows the results. Text results are written
        /// without quotes, so there they are cleaned as other text is.
        terminal: bool,
        /// The program that runs the expression in a process of its own.
        worker: Option<PathBuf>,
    },
}

impl Mode {
    /// `--jq` wins, then `--json`. Without either: text at a terminal and
    /// JSON anywhere else.
    pub fn choose(json: bool, jq: Option<&str>, env: &Env) -> Mode {
        match jq {
            Some(expression) => Mode::Jq {
                expression: expression.to_owned(),
                terminal: env.stdout_terminal,
                worker: env.program.clone(),
            },
            None if json || !env.stdout_terminal => Mode::Json {
                pretty: env.stdout_terminal,
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
        Mode::Jq {
            expression,
            terminal,
            worker,
        } => {
            let lines = jq::written(expression, value, worker.as_deref())?;
            if *terminal {
                write(out, &render::clean_lines(&lines))
            } else {
                write(out, &lines)
            }
        }
    }
}

/// Writes an error to standard error: the API's error object as JSON, or a
/// line or two of text for a person.
pub fn error(err: &mut dyn Write, mode: &Mode, error: &CliError) {
    let text = match mode {
        Mode::Text => {
            let mut text = format!("error: {}\n", render::clean(&error.message));
            if let Some(hint) = error.hint() {
                text.push_str(&render::clean(hint));
                text.push('\n');
            }
            text
        }
        Mode::Json { pretty } => json_text(&error.body(), *pretty) + "\n",
        Mode::Jq { .. } => json_text(&error.body(), false) + "\n",
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

    /// `--jq` with this expression, run in this process.
    fn jq(expression: &str, terminal: bool) -> Mode {
        Mode::Jq {
            expression: expression.to_owned(),
            terminal,
            worker: None,
        }
    }

    #[test]
    fn json_when_piped_text_at_a_terminal_and_flags_override() {
        let piped = Env::default();
        let terminal = Env {
            stdout_terminal: true,
            ..Env::default()
        };
        assert_eq!(Mode::choose(false, None, &terminal), Mode::Text);
        assert_eq!(
            Mode::choose(false, None, &piped),
            Mode::Json { pretty: false }
        );
        assert_eq!(
            Mode::choose(true, None, &terminal),
            Mode::Json { pretty: true }
        );
        assert_eq!(Mode::choose(true, Some(".id"), &terminal), jq(".id", true));
        let installed = Env {
            program: Some(PathBuf::from("/bin/wenmar-open")),
            ..Env::default()
        };
        assert_eq!(
            Mode::choose(false, Some(".id"), &installed),
            Mode::Jq {
                expression: ".id".to_owned(),
                terminal: false,
                worker: Some(PathBuf::from("/bin/wenmar-open")),
            }
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
        for mode in [Mode::Text, Mode::Json { pretty: false }, jq(".", false)] {
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
        for mode in [Mode::Json { pretty: false }, jq(".", false)] {
            let mut json = Vec::new();
            error(&mut json, &mode, &failure);
            let body: Value = serde_json::from_slice(&json).unwrap();
            assert_eq!(body, failure.body());
        }
    }

    #[test]
    fn jq_with_no_results_writes_nothing() {
        let mut out = Vec::new();
        answer(&mut out, &jq("empty", false), &json!(1), |_| String::new()).unwrap();
        assert!(out.is_empty());
    }

    #[test]
    fn jq_text_is_cleaned_for_a_terminal_and_exact_otherwise() {
        let value = json!(["Kona\u{1b}]0;owned\u{7}", "two\nlines", { "a": "\u{1b}" }]);
        let written = |terminal: bool| {
            let mut out = Vec::new();
            answer(&mut out, &jq(".[]", terminal), &value, |_| String::new()).unwrap();
            String::from_utf8(out).unwrap()
        };
        assert_eq!(
            written(true),
            "Kona\u{fffd}]0;owned\u{fffd}\ntwo\nlines\n{\"a\":\"\\u001b\"}\n"
        );
        assert_eq!(
            written(false),
            "Kona\u{1b}]0;owned\u{7}\ntwo\nlines\n{\"a\":\"\\u001b\"}\n"
        );
    }
}
