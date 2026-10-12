//! Answers from the local data file, through the two libraries.

use std::fmt::Display;
use std::path::Path;

use serde::Serialize;
use serde_json::Value;
use wenmar_shopdata::answer::{self, AnswerError, Answers, Overviews, Recent, SITE, Search};
use wenmar_shopdata::sqlite::ShopSource;
use wenmar_shopdata::{Dataset, ShopData, ShopError};
use wenmar_vehicles::sqlite::SqliteSource;
use wenmar_vehicles::{Catalog, Scope};
use wenmar_vin::sqlite::SqliteData;
use wenmar_vin::{DecodeError, DecodeOptions, Decoder};

use crate::data::{self, ShopStatus, Status};
use crate::env::SHOP_FILE;
use crate::error::{CliError, DATA_INVALID, DATASET_UNAVAILABLE, NO_DATA, NOT_FOUND};
use crate::request::{
    Action, Lookup, MOST, RecallAction, RecallLookup, Request, SEARCH_DEFAULT, Topic, current_year,
};

use crate::request::{CodeAction, CodeLookup};

/// What to do about a data file that cannot be used.
pub const PULL_HINT: &str = "Run `wenmar-open data pull` to download the current data file.";

/// What to do when the recalls are not here.
pub const SHOP_PULL_HINT: &str = "Run: wenmar-open data pull --shop";

/// What to do about a dataset only the hosted service carries.
pub const HOSTED_ONLY_HINT: &str =
    "Bulletins are served by the hosted API only. Run the command again with --online.";

/// The error for a dataset the local files do not hold: the API's own
/// `dataset_unavailable`, with what to run to get the data. A dataset only
/// the hosted service carries is told to be asked for online.
pub fn dataset_unavailable(dataset: Dataset) -> CliError {
    let hint = if dataset.hosted_only() {
        HOSTED_ONLY_HINT
    } else {
        SHOP_PULL_HINT
    };
    CliError::new(
        DATASET_UNAVAILABLE,
        ShopError::DatasetUnavailable(dataset).to_string(),
    )
    .with_details(serde_json::json!({ "dataset": dataset.id() }))
    .with_hint(hint)
}

/// The recall dataset a question needs first: the country asked for, or
/// the United States' when none is, as the API names it.
pub fn recall_dataset(lookup: &RecallLookup) -> Dataset {
    match (lookup.country.as_deref(), lookup.source.as_deref()) {
        (Some("ca"), _) | (_, Some("tc")) => Dataset::RecallsCa,
        _ => Dataset::RecallsUs,
    }
}

/// An open data file.
pub struct Local {
    decoder: Decoder<SqliteData>,
    catalog: Catalog<SqliteSource>,
    /// A second connection to the data file, for the lookups the catalog
    /// has no method for.
    core: SqliteSource,
    /// The shop file beside the data file, when it is there and goes with
    /// this data file.
    shop: Option<ShopData<ShopSource>>,
    shop_status: ShopStatus,
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

/// What to do about a shop file that cannot be used.
fn shop_unusable(status: &ShopStatus) -> CliError {
    CliError::new(
        DATA_INVALID,
        format!(
            "The shop data file at {} cannot be used: {}.",
            status.path,
            status.problem.as_deref().unwrap_or("it could not be read")
        ),
    )
    .with_hint(SHOP_PULL_HINT)
}

/// An error of the shared answers, as this tool reports it.
fn answer_error(error: AnswerError) -> CliError {
    match error {
        AnswerError::Validation { field, message } => CliError::validation(field, message),
        AnswerError::NotFound(message) => CliError::new(NOT_FOUND, message),
        AnswerError::Shop(ShopError::DatasetUnavailable(dataset)) => dataset_unavailable(dataset),
        other => unreadable(other),
    }
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
        let core = SqliteSource::open(path).map_err(unreadable)?;
        // The shop file is beside the data file. One that is missing or
        // cannot be used does not stop the data file from answering: only
        // a question about recalls says so.
        let shop_path = path.with_file_name(SHOP_FILE);
        let shop_status = data::inspect_shop(&shop_path, status.data_version.as_deref());
        let shop = if shop_status.usable {
            data::open_shop(&shop_path).ok()
        } else {
            None
        };
        Ok(Local {
            decoder,
            catalog,
            core,
            shop,
            shop_status,
            status,
        })
    }

    pub fn status(&self) -> &Status {
        &self.status
    }

    /// The shop file's status: `data status --shop` prints it.
    pub fn shop_status(&self) -> &ShopStatus {
        &self.shop_status
    }

    /// What an overview is built from. The shop file is used when it is
    /// there and goes with this data file. Without it, and with one that
    /// cannot be used, the answer's recalls are `null`: that is not an
    /// error, and `data status --shop` says what is wrong with the file.
    fn overviews(&self) -> Overviews<'_, ShopSource, SqliteSource> {
        Overviews {
            shop: self.shop.as_ref(),
            catalog: &self.catalog,
            core: &self.core,
            base: SITE,
        }
    }

    /// NHTSA's crash test ratings for a model year, from the shop file
    /// beside the data file, worded as the hosted API words them.
    fn safety(&self, id: &str) -> Result<Value, CliError> {
        let Some(shop) = &self.shop else {
            return Err(if self.shop_status.installed {
                shop_unusable(&self.shop_status)
            } else {
                dataset_unavailable(Dataset::Ratings)
            });
        };
        let answers = Answers {
            shop,
            catalog: &self.catalog,
            core: &self.core,
            base: SITE,
        };
        json(answers.safety(id).map_err(answer_error)?)
    }

    /// What owners reported to NHTSA about a model year, from the shop file
    /// beside the data file, worded as the hosted API words them.
    fn complaints(&self, id: &str) -> Result<Value, CliError> {
        let Some(shop) = &self.shop else {
            return Err(if self.shop_status.installed {
                shop_unusable(&self.shop_status)
            } else {
                dataset_unavailable(Dataset::Complaints)
            });
        };
        let answers = Answers {
            shop,
            catalog: &self.catalog,
            core: &self.core,
            base: SITE,
        };
        json(answers.complaints(id).map_err(answer_error)?)
    }

    /// Trouble codes, from the shop file beside the data file, worded as
    /// the hosted API words them. The bulletin parts of a vehicle's answer
    /// are always `null` here: bulletins are on the hosted service only.
    fn codes(&self, action: CodeAction, lookup: &CodeLookup) -> Result<Value, CliError> {
        let Some(shop) = &self.shop else {
            return Err(if self.shop_status.installed {
                shop_unusable(&self.shop_status)
            } else {
                dataset_unavailable(Dataset::Codes)
            });
        };
        let answers = Answers {
            shop,
            catalog: &self.catalog,
            core: &self.core,
            base: SITE,
        };
        let text = |field: &Option<String>| field.clone().unwrap_or_default();
        match action {
            CodeAction::Get => json(answers.code(&text(&lookup.code)).map_err(answer_error)?),
            CodeAction::Search => json(
                answers
                    .codes(&answer::CodeSearch {
                        q: lookup.query.clone(),
                        system: lookup.system.clone(),
                        limit: lookup.limit,
                        cursor: lookup.cursor.clone(),
                    })
                    .map_err(answer_error)?,
            ),
            CodeAction::Vehicle => json(
                answers
                    .vehicle_code(&text(&lookup.id), &text(&lookup.code))
                    .map_err(answer_error)?,
            ),
        }
    }

    /// The recalls that may apply, from the shop file beside the data
    /// file, worded as the hosted API words them.
    fn recalls(&self, action: RecallAction, lookup: &RecallLookup) -> Result<Value, CliError> {
        let Some(shop) = &self.shop else {
            return Err(if self.shop_status.installed {
                shop_unusable(&self.shop_status)
            } else {
                dataset_unavailable(recall_dataset(lookup))
            });
        };
        let answers = Answers {
            shop,
            catalog: &self.catalog,
            core: &self.core,
            base: SITE,
        };
        let text = |field: &Option<String>| field.clone().unwrap_or_default();
        let country = lookup.country.as_deref();
        match action {
            RecallAction::Vehicle => json(
                answers
                    .vehicle(&text(&lookup.id), country)
                    .map_err(answer_error)?,
            ),
            RecallAction::Vin => {
                // The API looks for the recalls before it decodes the VIN,
                // so the same error comes first here.
                let source = answer::country(country).map_err(answer_error)?;
                shop.require_recalls(source)
                    .map_err(|error| answer_error(error.into()))?;
                let decode = self.decode(&text(&lookup.vin), lookup.year)?;
                json(answers.vin(&decode, country).map_err(answer_error)?)
            }
            RecallAction::Campaign => json(
                answers
                    .campaign(&text(&lookup.source), &text(&lookup.campaign))
                    .map_err(answer_error)?,
            ),
            RecallAction::Search => json(
                answers
                    .search(&Search {
                        make: lookup.make.clone(),
                        model: lookup.model.clone(),
                        year: lookup.year,
                        component: lookup.component.clone(),
                        q: lookup.query.clone(),
                        country: lookup.country.clone(),
                        limit: lookup.limit,
                        cursor: lookup.cursor.clone(),
                    })
                    .map_err(answer_error)?,
            ),
            RecallAction::Recent => json(
                answers
                    .recent(&Recent {
                        since: lookup.since.clone(),
                        country: lookup.country.clone(),
                        make: lookup.make.clone(),
                        limit: lookup.limit,
                        cursor: lookup.cursor.clone(),
                    })
                    .map_err(answer_error)?,
            ),
        }
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
            Request::VinOverview { vin, year } => {
                // The decode comes first, as in the API: a VIN that cannot
                // be decoded fails as a decode does.
                let decode = self.decode(vin, *year)?;
                json(self.overviews().vin(&decode).map_err(answer_error)?)
            }
            Request::VehicleFacts { topic, id, country } => {
                let answers = self.facts(*topic, country.as_deref())?;
                let country = country.as_deref();
                match topic {
                    Topic::Fuel => answers.vehicle_fuel(id, country),
                    Topic::Specs => answers.vehicle_specs(id),
                    Topic::Fluids => answers.vehicle_fluids(id),
                    Topic::Tires => answers.vehicle_tires(id),
                }
                .map_err(answer_error)
            }
            Request::VinFacts {
                topic,
                vin,
                year,
                country,
            } => {
                // The data is looked for first, then the VIN is decoded, as
                // in the API.
                let answers = self.facts(*topic, country.as_deref())?;
                let country = country.as_deref();
                answers
                    .require(topic.name(), country)
                    .map_err(answer_error)?;
                let decode = self.decode(vin, *year)?;
                match topic {
                    Topic::Fuel => answers.vin_fuel(&decode, country),
                    Topic::Specs => answers.vin_specs(&decode),
                    Topic::Fluids => answers.vin_fluids(&decode),
                    Topic::Tires => answers.vin_tires(&decode),
                }
                .map_err(answer_error)
            }
            Request::Vehicles { action, lookup } => self.vehicles(*action, lookup),
            Request::Recalls { action, lookup } => self.recalls(*action, lookup),
            Request::Codes { action, lookup } => self.codes(*action, lookup),
            // The files do not hold bulletins: the hosted API answers them.
            Request::Bulletins { .. } => Err(dataset_unavailable(Dataset::Bulletins)),
        }
    }

    /// What fuel, specifications, fluids and tire fitments are answered
    /// from: the shop file beside the data file. Without one, the error
    /// names the dataset the hosted API would name.
    fn facts(
        &self,
        topic: Topic,
        country: Option<&str>,
    ) -> Result<Answers<'_, ShopSource, SqliteSource>, CliError> {
        let Some(shop) = &self.shop else {
            return Err(if self.shop_status.installed {
                shop_unusable(&self.shop_status)
            } else {
                dataset_unavailable(answer::facts::dataset_for(topic.name(), country))
            });
        };
        Ok(Answers {
            shop,
            catalog: &self.catalog,
            core: &self.core,
            base: SITE,
        })
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
            Action::Safety => self.safety(&text(&lookup.id)),
            Action::Complaints => self.complaints(&text(&lookup.id)),
            Action::Overview => json(
                self.overviews()
                    .vehicle(&text(&lookup.id))
                    .map_err(answer_error)?,
            ),
            Action::ModelYears => json(
                self.overviews()
                    .model_years(&make, &model)
                    .map_err(answer_error)?,
            ),
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
