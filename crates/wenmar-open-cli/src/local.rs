//! Answers from the local data file, through the two libraries.

use std::fmt::Display;
use std::path::Path;

use serde::Serialize;
use serde_json::Value;
use wenmar_vehicles::sqlite::SqliteSource;
use wenmar_vehicles::{Catalog, Scope};
use wenmar_vin::sqlite::SqliteData;
use wenmar_vin::{DecodeError, DecodeOptions, Decoder};

use crate::data::{self, Status};
use crate::error::{CliError, DATA_INVALID, NO_DATA, NOT_FOUND};
use crate::request::{Action, Lookup, MOST, Request, SEARCH_DEFAULT, current_year};

/// What to do about a data file that cannot be used.
pub const PULL_HINT: &str = "Run `wenmar-open data pull` to download the current data file.";

/// An open data file.
pub struct Local {
    decoder: Decoder<SqliteData>,
    catalog: Catalog<SqliteSource>,
    status: Status,
}

fn unreadable(error: impl Display) -> CliError {
    CliError::new(
        DATA_INVALID,
        format!("The data file could not be read: {error}"),
    )
    .with_hint(PULL_HINT)
}

fn json<T: Serialize>(value: T) -> Result<Value, CliError> {
    serde_json::to_value(value).map_err(unreadable)
}

impl Local {
    /// Opens the data file at `path`, read-only.
    pub fn open(path: &Path) -> Result<Local, CliError> {
        let status = data::inspect(path);
        if !status.installed {
            return Err(CliError::new(
                NO_DATA,
                format!("There is no data file at {}.", status.path),
            )
            .with_hint(PULL_HINT));
        }
        if let Some(problem) = &status.problem {
            return Err(CliError::new(
                DATA_INVALID,
                format!(
                    "The data file at {} cannot be used: {problem}.",
                    status.path
                ),
            )
            .with_hint(PULL_HINT));
        }
        let decoder = Decoder::new(SqliteData::open(path).map_err(unreadable)?);
        let source = SqliteSource::open(path).map_err(unreadable)?;
        let catalog = Catalog::new(source).map_err(unreadable)?;
        Ok(Local {
            decoder,
            catalog,
            status,
        })
    }

    pub fn status(&self) -> &Status {
        &self.status
    }

    /// Answers a request that has been through [`Request::checked`].
    pub fn run(&self, request: &Request) -> Result<Value, CliError> {
        match request {
            Request::VinDecode { vin, year } => self.decode(vin, *year),
            Request::VinBatch { vins, year } => {
                let current_year = current_year();
                let items = vins
                    .iter()
                    .map(|vin| {
                        let one = Request::VinDecode {
                            vin: vin.clone(),
                            year: *year,
                        };
                        let answer = match one.checked(current_year) {
                            Ok(Request::VinDecode { vin, year }) => self.decode(&vin, year),
                            Ok(_) => Err(unreadable("a VIN became another request")),
                            Err(error) => Err(error),
                        };
                        answer.unwrap_or_else(|error| error.body())
                    })
                    .collect();
                Ok(Value::Array(items))
            }
            Request::Vehicles { action, lookup } => self.vehicles(*action, lookup),
        }
    }

    fn decode(&self, vin: &str, year: Option<u16>) -> Result<Value, CliError> {
        let options = DecodeOptions {
            model_year: year,
            current_year: None,
        };
        let decoded = match self.decoder.decode(vin, options) {
            Ok(decoded) => decoded,
            Err(DecodeError::UnknownManufacturer { wmi }) => {
                return Err(CliError::new(
                    NOT_FOUND,
                    format!("No manufacturer is registered for {wmi}."),
                ));
            }
            Err(DecodeError::InvalidVin { .. }) => {
                // `checked` has already refused it with the full details.
                return Err(crate::request::well_formed(vin)
                    .err()
                    .unwrap_or_else(|| unreadable("the VIN was refused")));
            }
            Err(other) => return Err(unreadable(describe(&other))),
        };
        let selection = self.catalog.selection(&decoded).map_err(unreadable)?;
        let mut value = json(&decoded)?;
        if let (Some(selection), Some(object)) = (selection, value.as_object_mut()) {
            object.insert("catalog".to_owned(), json(selection)?);
        }
        Ok(value)
    }

    fn vehicles(&self, action: Action, lookup: &Lookup) -> Result<Value, CliError> {
        let catalog = &self.catalog;
        let scope = lookup
            .scope
            .as_deref()
            .and_then(Scope::parse)
            .unwrap_or_default();
        let text = |field: &Option<String>| field.clone().unwrap_or_default();
        let (term, make, model) = (text(&lookup.term), text(&lookup.make), text(&lookup.model));
        let year = lookup.year.unwrap_or_default();
        match action {
            Action::Years => {
                let mut years = catalog.years(scope, &term).map_err(unreadable)?;
                years.sort_unstable_by(|left, right| right.cmp(left));
                json(years)
            }
            Action::Makes => json(
                catalog
                    .makes(lookup.year, scope, &term, lookup.limit.unwrap_or(MOST))
                    .map_err(unreadable)?,
            ),
            Action::Models => json(
                catalog
                    .models(
                        &make,
                        lookup.year,
                        scope,
                        &term,
                        lookup.limit.unwrap_or(MOST),
                    )
                    .map_err(unreadable)?,
            ),
            Action::Submodels => json(
                catalog
                    .submodels(&make, &model, year, &term)
                    .map_err(unreadable)?,
            ),
            Action::Engines => json(
                catalog
                    .engines(&make, &model, year, lookup.submodel.as_deref(), &term)
                    .map_err(unreadable)?,
            ),
            Action::Search => json(
                catalog
                    .search(
                        &text(&lookup.query),
                        scope,
                        lookup.limit.unwrap_or(SEARCH_DEFAULT),
                    )
                    .map_err(unreadable)?,
            ),
            Action::Entry => match catalog.entry(&text(&lookup.id)).map_err(unreadable)? {
                Some(entry) => json(entry),
                None => Err(CliError::new(NOT_FOUND, "No vehicle has that id.")),
            },
        }
    }
}

/// An error with the errors that caused it.
fn describe(error: &dyn std::error::Error) -> String {
    let mut text = error.to_string();
    let mut source = error.source();
    while let Some(cause) = source {
        text.push_str(": ");
        text.push_str(&cause.to_string());
        source = cause.source();
    }
    text
}
