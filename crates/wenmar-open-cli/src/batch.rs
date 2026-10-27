//! `wenmar-open vin batch`: many VINs at once, from the arguments, a file
//! or standard input, as JSON or as CSV.

use std::io::{BufRead, Write};
use std::path::PathBuf;

use serde_json::{Value, json};
use wenmar_shopdata::batch as shape;

use crate::cli::Global;
use crate::env::Env;
use crate::error::{self, CliError};
use crate::output::{self, Mode};
use crate::render;
use crate::request::{MOST_VINS, Request};

/// What `vin batch` was asked.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Asked {
    pub vins: Vec<String>,
    pub input: Option<PathBuf>,
    pub year: Option<u16>,
    pub csv: bool,
    pub fields: Option<String>,
}

fn read(path: &PathBuf, stdin: &mut dyn BufRead) -> Result<String, CliError> {
    let mut text = String::new();
    let read = if path.as_os_str() == "-" {
        stdin.read_to_string(&mut text).map(|_| ())
    } else {
        std::fs::read_to_string(path).map(|found| text = found)
    };
    read.map_err(|problem| {
        CliError::new(
            error::IO,
            format!("{} could not be read: {problem}", path.display()),
        )
    })?;
    Ok(text)
}

/// The VINs asked for, in order: those typed, then those of the file or of
/// standard input. Standard input is read when nothing else names a VIN
/// and it is not a terminal.
fn gather(asked: &Asked, env: &Env, stdin: &mut dyn BufRead) -> Result<Vec<String>, CliError> {
    let mut vins = asked.vins.clone();
    match &asked.input {
        Some(path) => vins.extend(shape::find_vins(&read(path, stdin)?)),
        None if vins.is_empty() && !env.stdin_terminal => {
            vins.extend(shape::find_vins(&read(&PathBuf::from("-"), stdin)?));
        }
        None => {}
    }
    if vins.is_empty() {
        return Err(CliError::new(
            error::VALIDATION_FAILED,
            "no VIN was given: name VINs, give --input FILE, or pipe a list in",
        ));
    }
    Ok(vins)
}

pub fn run(
    asked: Asked,
    env: &Env,
    global: &Global,
    mode: &Mode,
    stdin: &mut dyn BufRead,
    stdout: &mut dyn Write,
    stderr: &mut dyn Write,
) -> Result<(), CliError> {
    let fields = shape::parse_fields(asked.fields.as_deref()).map_err(|problem| {
        CliError::new(error::VALIDATION_FAILED, problem.message).with_details(problem.details)
    })?;
    let vins = gather(&asked, env, stdin)?;
    let backend = crate::open(env, global, mode, stderr)?;
    // The API takes a batch of at most MOST_VINS, so a longer list is asked
    // for in parts and put back together in order.
    let mut items: Vec<Value> = Vec::with_capacity(vins.len());
    for part in vins.chunks(MOST_VINS) {
        let answer = backend.run(Request::VinBatch {
            vins: part.to_vec(),
            year: asked.year,
        })?;
        match answer {
            Value::Array(found) => items.extend(found),
            other => {
                return Err(CliError::new(
                    error::INTERNAL_ERROR,
                    format!("a batch was answered with {other}, not a list"),
                ));
            }
        }
    }
    if asked.csv {
        let text = shape::csv(&items, &vins, fields.as_deref());
        return stdout
            .write_all(text.as_bytes())
            .and_then(|()| stdout.flush())
            .map_err(|problem| CliError::from_write(&problem));
    }
    if let Some(fields) = &fields {
        items = items
            .into_iter()
            .map(|item| shape::project(item, fields))
            .collect();
    }
    let request = Request::VinBatch {
        vins,
        year: asked.year,
    };
    output::answer(stdout, mode, &json!(items), |value| {
        render::text(&request, value)
    })
}
