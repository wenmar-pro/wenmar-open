//! The `wenmar-open` command-line tool.
//!
//! [`run`] is the whole program. It takes its arguments, its surroundings
//! and its three streams as parameters, so tests drive it without a
//! process, a terminal or the network.

pub mod backend;
pub mod cli;
pub mod data;
pub mod doctor;
pub mod env;
pub mod error;
pub mod jq;
pub mod local;
pub mod mcp;
pub mod output;
pub mod pull;
pub mod remote;
pub mod render;
pub mod request;
pub mod setup;
pub mod tui;

use std::ffi::OsString;
use std::io::{BufRead, Write};

use clap::Parser;
use clap::error::ErrorKind;

use crate::backend::Backend;
use crate::cli::{Cli, Command, DataCommand};
use crate::env::Env;
use crate::error::{CliError, USAGE};
use crate::output::Mode;

/// Runs one invocation and returns the process exit code.
pub fn run(
    args: Vec<OsString>,
    env: &Env,
    stdin: &mut dyn BufRead,
    stdout: &mut dyn Write,
    stderr: &mut dyn Write,
) -> u8 {
    let mut mode = Mode::before_parsing(&args, env.stdout_terminal);
    let outcome = match Cli::try_parse_from(args) {
        Ok(cli) => {
            mode = Mode::choose(
                cli.global.json,
                cli.global.jq.as_deref(),
                env.stdout_terminal,
            );
            execute(cli, env, &mode, stdin, stdout, stderr)
        }
        Err(problem) => match problem.kind() {
            ErrorKind::DisplayHelp | ErrorKind::DisplayVersion => stdout
                .write_all(problem.render().to_string().as_bytes())
                .map_err(|error| CliError::from_write(&error)),
            _ => Err(usage(&problem)),
        },
    };
    match outcome {
        Ok(()) => 0,
        Err(failure) => {
            let code = failure.exit_code();
            if code != 0 {
                output::error(stderr, &mode, &failure);
            }
            code
        }
    }
}

/// A command line that could not be read, as an error. The message is
/// clap's own, without its styling.
fn usage(problem: &clap::Error) -> CliError {
    let text = problem.render().to_string();
    let message = text
        .lines()
        .next()
        .unwrap_or_default()
        .trim_start_matches("error: ")
        .to_owned();
    CliError::new(USAGE, message).with_hint("Run `wenmar-open --help` for the commands.")
}

/// Opens the source of answers. What it has to say about its choice goes
/// to standard error, for a person only: JSON on standard error is always
/// an error.
fn open(
    env: &Env,
    global: &cli::Global,
    mode: &Mode,
    stderr: &mut dyn Write,
) -> Result<Backend, CliError> {
    let mut notes = Vec::new();
    let backend = Backend::open(env, global, &mut notes)?;
    if *mode == Mode::Text {
        for note in notes {
            let _ = writeln!(stderr, "note: {}", render::clean(&note));
        }
    }
    Ok(backend)
}

fn to_json<T: serde::Serialize>(value: &T) -> Result<serde_json::Value, CliError> {
    serde_json::to_value(value).map_err(|error| {
        CliError::new(
            error::IO,
            format!("the answer could not be written: {error}"),
        )
    })
}

fn execute(
    cli: Cli,
    env: &Env,
    mode: &Mode,
    stdin: &mut dyn BufRead,
    stdout: &mut dyn Write,
    stderr: &mut dyn Write,
) -> Result<(), CliError> {
    let request = match cli.command {
        Some(Command::Mcp) => {
            // Standard output carries protocol messages only, so nothing
            // is said about the choice of source.
            let backend = Backend::open(env, &cli.global, &mut Vec::new())?;
            return mcp::serve(&backend, stdin, stdout);
        }
        Some(Command::Setup {
            agent,
            dir,
            force,
            yes,
        }) => {
            let done = setup::run(agent, env, dir.as_deref(), force, yes)?;
            return output::answer(stdout, mode, &to_json(&done)?, render::setup);
        }
        Some(Command::Doctor) => {
            let report = doctor::report(env, &cli.global);
            output::answer(stdout, mode, &report, render::doctor)?;
            return if report["ok"] == true {
                Ok(())
            } else {
                Err(doctor::unhealthy())
            };
        }
        Some(Command::Vin { command }) => command.request(),
        Some(Command::Vehicles { command }) => command.request(),
        Some(Command::Data { command }) => {
            let directory = env.data_directory(cli.global.data_dir.as_deref())?;
            return match command {
                DataCommand::Status => {
                    let status = data::inspect(&directory.join(env::DATA_FILE));
                    output::answer(stdout, mode, &to_json(&status)?, render::status)
                }
                DataCommand::Pull {
                    data_version,
                    force,
                    releases,
                } => {
                    let pulled = pull::pull(
                        &directory,
                        &env.releases_url(releases.as_deref()),
                        data_version.as_deref(),
                        force,
                    )?;
                    output::answer(stdout, mode, &to_json(&pulled)?, render::pulled)
                }
            };
        }
        None if env.stdin_terminal && env.stdout_terminal => {
            let backend = open(env, &cli.global, mode, stderr)?;
            return tui::run(backend, env.no_color);
        }
        None => {
            // With nobody at a terminal there is nothing to open: say what
            // the tool is and stop. This never waits for input.
            let overview = doctor::overview(env, &cli.global);
            let json = match mode {
                Mode::Text => &Mode::Json {
                    pretty: env.stdout_terminal,
                },
                asked => asked,
            };
            return output::answer(stdout, json, &overview, |_| String::new());
        }
    };
    let backend = open(env, &cli.global, mode, stderr)?;
    let value = backend.run(request.clone())?;
    output::answer(stdout, mode, &value, |value| render::text(&request, value))
}
