//! The `wenmar-open` command-line tool.
//!
//! [`run`] is the whole program. It takes its arguments, its surroundings
//! and its three streams as parameters, so tests drive it without a
//! process, a terminal or the network.

pub mod backend;
pub mod cli;
pub mod data;
pub mod env;
pub mod error;
pub mod jq;
pub mod local;
pub mod output;
pub mod render;
pub mod request;

use std::ffi::OsString;
use std::io::{BufRead, Write};

use clap::Parser;
use clap::error::ErrorKind;

use crate::backend::Backend;
use crate::cli::{Cli, Command};
use crate::env::Env;
use crate::error::{CliError, USAGE};
use crate::output::Mode;

/// Runs one invocation and returns the process exit code.
pub fn run(
    args: Vec<OsString>,
    env: &Env,
    _stdin: &mut dyn BufRead,
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
            execute(cli, env, &mode, stdout)
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

fn execute(cli: Cli, env: &Env, mode: &Mode, stdout: &mut dyn Write) -> Result<(), CliError> {
    let request = match cli.command {
        Some(Command::Vin { command }) => command.request(),
        Some(Command::Vehicles { command }) => command.request(),
        None => {
            return Err(CliError::new(USAGE, "a command is required")
                .with_hint("Run `wenmar-open --help` for the commands."));
        }
    };
    let backend = Backend::open(env, &cli.global)?;
    let value = backend.run(request.clone())?;
    output::answer(stdout, mode, &value, |value| render::text(&request, value))
}
