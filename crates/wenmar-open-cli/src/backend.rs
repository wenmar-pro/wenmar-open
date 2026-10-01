//! Where answers come from.

use serde_json::Value;

use crate::cli::Global;
use crate::env::Env;
use crate::error::CliError;
use crate::local::Local;
use crate::request::{Request, current_year};

/// A source of answers.
pub enum Backend {
    /// The local data file.
    Local(Box<Local>),
}

impl Backend {
    /// Opens the source the options and the surroundings call for.
    pub fn open(env: &Env, global: &Global) -> Result<Backend, CliError> {
        let path = env.data_file(global.data_dir.as_deref())?;
        Ok(Backend::Local(Box::new(Local::open(&path)?)))
    }

    /// Checks a request and answers it. The answer is the JSON the hosted
    /// API gives for the same question.
    pub fn run(&self, request: Request) -> Result<Value, CliError> {
        let request = request.checked(current_year())?;
        match self {
            Backend::Local(local) => local.run(&request),
        }
    }
}
