//! Where answers come from: the local data file, or the hosted API.

use serde_json::Value;

use crate::cli::Global;
use crate::data;
use crate::env::Env;
use crate::error::CliError;
use crate::local::Local;
use crate::remote::Remote;
use crate::request::{Request, current_year};

/// A source of answers.
pub enum Backend {
    /// The local data file.
    Local(Box<Local>),
    /// The hosted API.
    Remote(Remote),
}

impl Backend {
    /// Opens the source the options and the surroundings call for.
    ///
    /// `--offline` is the data file or an error. `--online` is the API.
    /// With neither, it is the data file when there is one this build can
    /// read, and the API otherwise. A data file that is there and cannot
    /// be used is said so in `notes`.
    pub fn open(env: &Env, global: &Global, notes: &mut Vec<String>) -> Result<Backend, CliError> {
        let api = env.api_base(global.api.as_deref());
        if global.online {
            return Ok(Backend::Remote(Remote::new(&api)?));
        }
        let path = env.data_file(global.data_dir.as_deref());
        if global.offline {
            return Ok(Backend::Local(Box::new(Local::open(&path?)?)));
        }
        // With no home directory there is nowhere a data file could be.
        let Ok(path) = path else {
            return Ok(Backend::Remote(Remote::new(&api)?));
        };
        let status = data::inspect(&path);
        if status.usable {
            match Local::open(&path) {
                Ok(local) => return Ok(Backend::Local(Box::new(local))),
                Err(error) => notes.push(format!("{} Using the API at {api}.", error.message)),
            }
        } else if let Some(problem) = &status.problem {
            notes.push(format!(
                "The data file at {} cannot be used: {problem}. Using the API at {api}. Run `wenmar-open data pull` to replace it.",
                status.path
            ));
        }
        Ok(Backend::Remote(Remote::new(&api)?))
    }

    /// Checks a request and answers it. The answer is the JSON the hosted
    /// API gives for the same question, whichever source it came from.
    pub fn run(&self, request: Request) -> Result<Value, CliError> {
        let request = request.checked(current_year())?;
        match self {
            Backend::Local(local) => local.run(&request),
            Backend::Remote(remote) => remote.run(&request),
        }
    }

    /// One line saying where answers come from.
    pub fn describe(&self) -> String {
        match self {
            Backend::Local(local) => format!(
                "offline, data {}",
                local.status().data_version.as_deref().unwrap_or("unknown")
            ),
            Backend::Remote(remote) => format!("online, {}", remote.base()),
        }
    }
}
