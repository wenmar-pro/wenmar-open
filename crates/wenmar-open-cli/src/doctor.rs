//! `doctor`: what this tool can answer from, and what is wrong if it
//! cannot. Also the short report a bare `wenmar-open` prints when nobody
//! is at a terminal.

use clap::CommandFactory;
use serde_json::{Value, json};

use crate::cli::{Cli, Global};
use crate::data;
use crate::env::Env;
use crate::error::CliError;
use crate::remote::Remote;
use crate::setup::{Agent, MARKER, SKILL};

/// The data file's status, or why there is nowhere to keep one.
fn data_report(env: &Env, global: &Global) -> Value {
    match env.data_file(global.data_dir.as_deref()) {
        Ok(path) => serde_json::to_value(data::inspect(&path)).unwrap_or(Value::Null),
        Err(error) => json!({ "installed": false, "usable": false, "problem": error.message }),
    }
}

fn skills_report(env: &Env) -> Value {
    let mut report = serde_json::Map::new();
    for agent in Agent::ALL {
        let entry = match agent.skills_directory(env, None) {
            Err(_) => json!({ "installed": false }),
            Ok(directory) => {
                let path = Agent::skill_path(&directory);
                match std::fs::read_to_string(&path) {
                    Ok(text) => json!({
                        "installed": true,
                        "current": text == SKILL,
                        "managed": text.contains(MARKER),
                        "path": path.display().to_string()
                    }),
                    Err(_) => json!({ "installed": false, "path": path.display().to_string() }),
                }
            }
        };
        report.insert(agent.name().to_owned(), entry);
    }
    Value::Object(report)
}

/// Checks the data file and the API. The report's `ok` is whether at
/// least one of them can answer the way the options ask.
pub fn report(env: &Env, global: &Global) -> Value {
    let data = data_report(env, global);
    let url = env.api_base(global.api.as_deref());
    let api = if global.offline {
        json!({ "url": url, "checked": false })
    } else {
        match Remote::new(&url).and_then(|remote| remote.meta()) {
            Ok(meta) => json!({
                "url": url,
                "checked": true,
                "reachable": true,
                "data_version": meta["data_version"]
            }),
            Err(error) => json!({
                "url": url,
                "checked": true,
                "reachable": false,
                "problem": error.message
            }),
        }
    };
    let local = data["usable"] == true && !global.online;
    let remote = api["reachable"] == true;
    let answers_from = if local {
        "data file"
    } else if remote {
        "api"
    } else {
        "nothing"
    };
    json!({
        "version": env!("CARGO_PKG_VERSION"),
        "ok": local || remote,
        "answers_from": answers_from,
        "data": data,
        "api": api,
        "skills": skills_report(env)
    })
}

/// The error `doctor` ends with when nothing can answer.
pub fn unhealthy() -> CliError {
    CliError::new("unhealthy", "Neither the data file nor the API can answer.")
        .with_hint("Check the connection, or run `wenmar-open data pull` when there is one.")
}

/// Every command that has no commands under it, with what it does. A
/// hidden command is the tool's own and is left out, as the help leaves it.
fn commands(command: &clap::Command, prefix: &str, found: &mut Vec<Value>) {
    let name = format!("{prefix}{}", command.get_name());
    let mut under = command
        .get_subcommands()
        .filter(|sub| sub.get_name() != "help" && !sub.is_hide_set())
        .peekable();
    if under.peek().is_none() {
        let about = command
            .get_about()
            .map(ToString::to_string)
            .unwrap_or_default();
        found.push(json!({ "command": name, "about": about }));
        return;
    }
    for sub in under {
        commands(sub, &format!("{name} "), found);
    }
}

/// What a bare `wenmar-open` prints when it is not at a terminal: what the
/// tool is, where answers will come from, and its commands. It asks
/// nothing of the network.
pub fn overview(env: &Env, global: &Global) -> Value {
    let data = data_report(env, global);
    let local = data["usable"] == true && !global.online;
    let mut found = Vec::new();
    commands(&Cli::command(), "", &mut found);
    json!({
        "name": "wenmar-open",
        "version": env!("CARGO_PKG_VERSION"),
        "about": "Free vehicle data for auto repair shops: VIN decoding and a year, make, model, submodel and engine catalog. No key, no account.",
        "answers_from": if local { "data file" } else { "api" },
        "data": data,
        "api": env.api_base(global.api.as_deref()),
        "commands": found,
        "help": "wenmar-open --help"
    })
}
