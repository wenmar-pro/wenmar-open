//! One lookup, however it was asked for: on the command line, as an MCP
//! tool call, or in the terminal interface.
//!
//! [`Request::checked`] applies the hosted API's own rules (which
//! parameters are required, the limits, the bounds of a model year) before
//! anything is read or sent, so the local data file and the API refuse the
//! same things with the same codes, and nothing malformed reaches a URL.

use std::time::{SystemTime, UNIX_EPOCH};

use serde_json::{Value, json};
use wenmar_vehicles::Scope;
use wenmar_vin::{Vin, VinError, suggest};

use crate::error::{CliError, INVALID_VIN, NOT_FOUND, VALIDATION_FAILED};

/// Most VINs in one batch.
pub const MOST_VINS: usize = 50;
/// Longest text read as a VIN. A VIN has 17 characters; spaces and dashes
/// are allowed, so there is some room. Anything longer is refused unread.
pub const LONGEST_INPUT: usize = 64;
/// Most items in a list, whatever `limit` says.
pub const MOST: usize = 500;
/// Search results when `limit` is not given, and the most it may be.
pub const SEARCH_DEFAULT: usize = 10;
pub const SEARCH_MOST: usize = 50;
/// Longest text read for a name, a term, an id or a search. The rest is
/// ignored.
pub const LONGEST_TEXT: usize = 200;
/// The first model year a 17-character VIN can have.
const FIRST_VIN_YEAR: u16 = 1980;
/// Recall campaigns in a list when `limit` is not given, and the most it
/// may be.
pub const RECALLS_DEFAULT: usize = 25;
pub const RECALLS_MOST: usize = 100;

/// Bulletins in a list when `limit` is not given, and the most one list
/// holds.
pub const BULLETINS_DEFAULT: usize = 25;
pub const BULLETINS_MOST: usize = 100;
/// Longest recall campaign number read.
pub const LONGEST_CAMPAIGN: usize = 20;
/// What the API says of a campaign it does not have.
pub const NO_CAMPAIGN: &str = "No recall campaign has that number.";

/// A step of the vehicle catalog.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Action {
    Years,
    Makes,
    Models,
    Submodels,
    Engines,
    Search,
    Entry,
    /// Everything held for one model year, by its vehicle id.
    Overview,
    /// Every model year of one model.
    ModelYears,
    /// NHTSA's crash test ratings for one model year, by its vehicle id.
    Safety,
    /// What owners reported to NHTSA about one model year, by its vehicle id.
    Complaints,
}

impl Action {
    pub const ALL: [Action; 11] = [
        Action::Years,
        Action::Makes,
        Action::Models,
        Action::Submodels,
        Action::Engines,
        Action::Search,
        Action::Entry,
        Action::Overview,
        Action::ModelYears,
        Action::Safety,
        Action::Complaints,
    ];

    /// The name of the action in the `wenmar_vehicles` tool. For the first
    /// six it is also the last part of the API's path.
    pub fn name(self) -> &'static str {
        match self {
            Action::Years => "years",
            Action::Makes => "makes",
            Action::Models => "models",
            Action::Submodels => "submodels",
            Action::Engines => "engines",
            Action::Search => "search",
            Action::Entry => "entry",
            Action::Overview => "overview",
            Action::ModelYears => "model_years",
            Action::Safety => "safety",
            Action::Complaints => "complaints",
        }
    }

    fn parse(name: &str) -> Option<Action> {
        Action::ALL.into_iter().find(|action| action.name() == name)
    }
}

/// What a catalog question may carry. Which fields matter depends on the
/// [`Action`].
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Lookup {
    pub year: Option<u16>,
    pub make: Option<String>,
    pub model: Option<String>,
    pub submodel: Option<String>,
    pub term: Option<String>,
    pub query: Option<String>,
    pub id: Option<String>,
    pub scope: Option<String>,
    pub limit: Option<usize>,
}

/// A question about jobs or labour estimates.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LaborAction {
    /// The job list, or one job of it.
    Jobs,
    /// The estimates of a model year, by its vehicle id.
    Vehicle,
    /// The estimates of the vehicle a VIN decodes to.
    Vin,
}

impl LaborAction {
    pub const ALL: [LaborAction; 3] = [LaborAction::Jobs, LaborAction::Vehicle, LaborAction::Vin];

    /// The name of the action in the `wenmar_labor` tool.
    pub fn name(self) -> &'static str {
        match self {
            LaborAction::Jobs => "jobs",
            LaborAction::Vehicle => "vehicle",
            LaborAction::Vin => "vin",
        }
    }

    fn parse(name: &str) -> Option<LaborAction> {
        LaborAction::ALL
            .into_iter()
            .find(|action| action.name() == name)
    }
}

/// What a question about labour is asked with. Which fields are used
/// depends on the [`LaborAction`].
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct LaborLookup {
    pub job: Option<String>,
    pub id: Option<String>,
    pub vin: Option<String>,
    pub year: Option<u16>,
}

/// The fields an action uses, each present. A VIN is checked for its
/// form here, as every VIN is; a job and a vehicle id are judged where
/// the answer is made.
fn checked_labor(
    action: LaborAction,
    lookup: LaborLookup,
    current_year: u16,
) -> Result<LaborLookup, CliError> {
    let job = present(lookup.job.as_ref());
    Ok(match action {
        LaborAction::Jobs => LaborLookup {
            job,
            ..LaborLookup::default()
        },
        LaborAction::Vehicle => {
            let Some(id) = present(lookup.id.as_ref()) else {
                return Err(CliError::validation(
                    "id",
                    "id is required for action=vehicle",
                ));
            };
            LaborLookup {
                job,
                id: Some(id),
                ..LaborLookup::default()
            }
        }
        LaborAction::Vin => {
            let Some(vin) = present(lookup.vin.as_ref()) else {
                return Err(CliError::validation(
                    "vin",
                    "vin is required for action=vin",
                ));
            };
            vin_year(lookup.year, current_year)?;
            LaborLookup {
                job,
                vin: Some(well_formed(&vin)?.as_str().to_owned()),
                year: lookup.year,
                ..LaborLookup::default()
            }
        }
    })
}

/// A question about trouble codes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CodeAction {
    /// What one generic code means.
    Get,
    /// The codes that are described, by code.
    Search,
    /// A code on one model year.
    Vehicle,
}

impl CodeAction {
    pub const ALL: [CodeAction; 3] = [CodeAction::Get, CodeAction::Search, CodeAction::Vehicle];

    /// The name of the action in the `wenmar_codes` tool.
    pub fn name(self) -> &'static str {
        match self {
            CodeAction::Get => "get",
            CodeAction::Search => "search",
            CodeAction::Vehicle => "vehicle",
        }
    }

    fn parse(name: &str) -> Option<CodeAction> {
        CodeAction::ALL
            .into_iter()
            .find(|action| action.name() == name)
    }
}

/// What a question about trouble codes is asked with. Which fields are
/// used depends on the [`CodeAction`].
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct CodeLookup {
    pub code: Option<String>,
    pub id: Option<String>,
    pub query: Option<String>,
    pub system: Option<String>,
    pub limit: Option<usize>,
    pub cursor: Option<String>,
}

/// The fields an action uses, each present. What a code or a system may
/// be is judged where the answer is made, as the API does it.
fn checked_codes(action: CodeAction, lookup: CodeLookup) -> Result<CodeLookup, CliError> {
    let needed = |field: &'static str, value: Option<&String>| {
        present(value).ok_or_else(|| {
            CliError::validation(
                field,
                format!("{field} is required for action={}", action.name()),
            )
        })
    };
    Ok(match action {
        CodeAction::Get => CodeLookup {
            code: Some(needed("code", lookup.code.as_ref())?),
            ..CodeLookup::default()
        },
        CodeAction::Search => CodeLookup {
            query: present(lookup.query.as_ref()),
            system: present(lookup.system.as_ref()),
            limit: lookup.limit,
            cursor: present(lookup.cursor.as_ref()),
            ..CodeLookup::default()
        },
        CodeAction::Vehicle => CodeLookup {
            id: Some(needed("id", lookup.id.as_ref())?),
            code: Some(needed("code", lookup.code.as_ref())?),
            ..CodeLookup::default()
        },
    })
}

/// A question about recalls.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RecallAction {
    /// The recalls that may apply to a model year, by its vehicle id.
    Vehicle,
    /// The same, for the model year a VIN decodes to.
    Vin,
    /// One campaign with every vehicle it lists.
    Campaign,
    /// Campaigns, newest first, narrowed by what is given.
    Search,
    /// Campaigns reported on or after a day, each with the vehicles it
    /// lists.
    Recent,
}

impl RecallAction {
    pub const ALL: [RecallAction; 5] = [
        RecallAction::Vehicle,
        RecallAction::Vin,
        RecallAction::Campaign,
        RecallAction::Search,
        RecallAction::Recent,
    ];

    /// The name of the action in the `wenmar_recalls` tool.
    pub fn name(self) -> &'static str {
        match self {
            RecallAction::Vehicle => "vehicle",
            RecallAction::Vin => "vin",
            RecallAction::Campaign => "campaign",
            RecallAction::Search => "search",
            RecallAction::Recent => "recent",
        }
    }

    fn parse(name: &str) -> Option<RecallAction> {
        RecallAction::ALL
            .into_iter()
            .find(|action| action.name() == name)
    }
}

/// What a recall question may carry. Which fields matter depends on the
/// [`RecallAction`].
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct RecallLookup {
    pub id: Option<String>,
    pub vin: Option<String>,
    pub year: Option<u16>,
    pub source: Option<String>,
    pub campaign: Option<String>,
    pub make: Option<String>,
    pub model: Option<String>,
    pub component: Option<String>,
    pub query: Option<String>,
    pub country: Option<String>,
    pub limit: Option<usize>,
    pub cursor: Option<String>,
    /// For the recent list: a day, such as `2026-09-01`.
    pub since: Option<String>,
}

/// A question about manufacturer bulletins.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BulletinAction {
    /// The bulletins that list a model year, by its vehicle id.
    Vehicle,
    /// The same, for the model year a VIN decodes to.
    Vin,
    /// One bulletin with every vehicle it lists.
    Get,
    /// Bulletins, newest first, narrowed by what is given.
    Search,
}

impl BulletinAction {
    pub const ALL: [BulletinAction; 4] = [
        BulletinAction::Vehicle,
        BulletinAction::Vin,
        BulletinAction::Get,
        BulletinAction::Search,
    ];

    /// The name of the action in the `wenmar_bulletins` tool.
    pub fn name(self) -> &'static str {
        match self {
            BulletinAction::Vehicle => "vehicle",
            BulletinAction::Vin => "vin",
            BulletinAction::Get => "get",
            BulletinAction::Search => "search",
        }
    }

    fn parse(name: &str) -> Option<BulletinAction> {
        BulletinAction::ALL
            .into_iter()
            .find(|action| action.name() == name)
    }
}

/// What a bulletin question may carry. Which fields matter depends on the
/// [`BulletinAction`].
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct BulletinLookup {
    pub id: Option<String>,
    pub vin: Option<String>,
    pub year: Option<u16>,
    pub nhtsa_id: Option<u64>,
    pub make: Option<String>,
    pub model: Option<String>,
    /// The `type` of the API.
    pub kind: Option<String>,
    pub component: Option<String>,
    pub code: Option<String>,
    pub topic: Option<String>,
    pub query: Option<String>,
    pub limit: Option<usize>,
    pub cursor: Option<String>,
}

/// What is asked about a model year, or about the model year a VIN decodes
/// to: one of the four answers of `/v1/vehicles/{id}/…` and `/v1/vin/{vin}/…`
/// that the shop file holds beside the recalls.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Topic {
    /// Fuel consumption ratings, from EPA and Natural Resources Canada.
    Fuel,
    /// Specifications from EPA's files.
    Specs,
    /// The engine oil grade EPA records, and the fluids read by hand.
    Fluids,
    /// Original-equipment tire fitments, read by hand.
    Tires,
}

impl Topic {
    pub const ALL: [Topic; 4] = [Topic::Fuel, Topic::Specs, Topic::Fluids, Topic::Tires];

    /// The name of the action in the two tools, of the command, and of the
    /// last part of the API's path.
    pub fn name(self) -> &'static str {
        match self {
            Topic::Fuel => "fuel",
            Topic::Specs => "specs",
            Topic::Fluids => "fluids",
            Topic::Tires => "tires",
        }
    }

    pub fn parse(name: &str) -> Option<Topic> {
        Topic::ALL.into_iter().find(|topic| topic.name() == name)
    }

    /// The list every answer about the topic has, empty or not.
    pub fn list(self) -> &'static str {
        match self {
            Topic::Fuel => "ratings",
            Topic::Specs => "specs",
            Topic::Fluids => "engine_oil",
            Topic::Tires => "fitments",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Request {
    VinDecode {
        vin: String,
        year: Option<u16>,
    },
    VinBatch {
        vins: Vec<String>,
        year: Option<u16>,
    },
    /// Everything held for the model year a VIN decodes to.
    VinOverview {
        vin: String,
        year: Option<u16>,
    },
    /// Fuel ratings, specifications, fluids or tire fitments of a model
    /// year, by its vehicle id. `country` is for fuel only.
    VehicleFacts {
        topic: Topic,
        id: String,
        country: Option<String>,
    },
    /// The same for the model year a VIN decodes to.
    VinFacts {
        topic: Topic,
        vin: String,
        year: Option<u16>,
        country: Option<String>,
    },
    Vehicles {
        action: Action,
        lookup: Lookup,
    },
    Recalls {
        action: RecallAction,
        lookup: RecallLookup,
    },
    /// Trouble codes, as this project describes them.
    Codes {
        action: CodeAction,
        lookup: CodeLookup,
    },
    /// The job list and Wenmar's labour estimates.
    Labor {
        action: LaborAction,
        lookup: LaborLookup,
    },
    /// Manufacturer bulletins. Served by the hosted API only.
    Bulletins {
        action: BulletinAction,
        lookup: BulletinLookup,
    },
    /// Ontario payroll deductions for one pay. Worked out by
    /// `payroll_calc`, here or by the API.
    Payroll {
        query: payroll_calc::Query,
    },
}

/// The calendar year, worked out the way `wenmar-vin` does. It is only an
/// upper bound on model years, so a day's error at New Year does not matter.
pub fn current_year() -> u16 {
    const SECONDS_PER_YEAR: u64 = 31_556_952;
    let seconds = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |elapsed| elapsed.as_secs());
    u16::try_from(1970 + seconds / SECONDS_PER_YEAR).unwrap_or(u16::MAX)
}

/// The well-formed VIN in `input`, or the error the API gives for it.
/// Nothing of the input is repeated in the error but single characters.
pub fn well_formed(input: &str) -> Result<Vin, CliError> {
    if input.len() > LONGEST_INPUT {
        return Err(
            CliError::new(INVALID_VIN, "a VIN has 17 characters, this is far longer")
                .with_details(json!({ "suggestions": [] })),
        );
    }
    Vin::parse(input).map_err(|error| {
        let mut details = json!({ "suggestions": suggest::for_malformed(input) });
        if let VinError::InvalidCharacters(characters) = &error {
            details["invalid_characters"] = json!(characters);
        }
        CliError::new(INVALID_VIN, error.to_string()).with_details(details)
    })
}

fn clip(text: &str) -> String {
    text.trim().chars().take(LONGEST_TEXT).collect()
}

/// A text field that is present and not blank, cut to [`LONGEST_TEXT`].
fn present(value: Option<&String>) -> Option<String> {
    value.map(|text| clip(text)).filter(|text| !text.is_empty())
}

fn required(field: &'static str, value: Option<&String>) -> Result<Option<String>, CliError> {
    match present(value) {
        Some(text) => Ok(Some(text)),
        None => Err(CliError::validation(field, format!("{field} is required"))),
    }
}

fn scope(value: Option<&String>) -> Result<Option<String>, CliError> {
    match present(value) {
        None => Ok(None),
        Some(text) if Scope::parse(&text).is_some() => Ok(Some(text)),
        Some(_) => Err(CliError::validation(
            "scope",
            "scope must be light, all, or a vehicle type id",
        )),
    }
}

fn vin_year(year: Option<u16>, current_year: u16) -> Result<(), CliError> {
    let latest = current_year.saturating_add(2);
    match year {
        Some(year) if !(FIRST_VIN_YEAR..=latest).contains(&year) => Err(CliError::new(
            VALIDATION_FAILED,
            format!("year must be between {FIRST_VIN_YEAR} and {latest}"),
        )
        .with_details(json!({ "field": "year", "min": FIRST_VIN_YEAR, "max": latest }))),
        _ => Ok(()),
    }
}

/// `country`: `us` or `ca` in either case, or nothing for both.
fn country(value: Option<&String>) -> Result<Option<String>, CliError> {
    match present(value).map(|text| text.to_ascii_lowercase()) {
        None => Ok(None),
        Some(text) if text == "us" || text == "ca" => Ok(Some(text)),
        Some(_) => Err(CliError::validation("country", "country must be us or ca")),
    }
}

/// `country` of a question about `topic`: checked for fuel, and dropped
/// for the others, which have no such choice.
fn fuel_country(topic: Topic, value: Option<&String>) -> Result<Option<String>, CliError> {
    match topic {
        Topic::Fuel => country(value),
        _ => Ok(None),
    }
}

/// A recall question with the API's rules applied.
fn checked_recalls(
    action: RecallAction,
    lookup: RecallLookup,
    current_year: u16,
) -> Result<RecallLookup, CliError> {
    let country = country(lookup.country.as_ref())?;
    Ok(match action {
        RecallAction::Vehicle => RecallLookup {
            id: required("id", lookup.id.as_ref())?,
            country,
            ..RecallLookup::default()
        },
        RecallAction::Vin => {
            let Some(vin) = lookup.vin.as_ref() else {
                return Err(CliError::validation("vin", "vin is required"));
            };
            vin_year(lookup.year, current_year)?;
            RecallLookup {
                vin: Some(well_formed(vin)?.as_str().to_owned()),
                year: lookup.year,
                country,
                ..RecallLookup::default()
            }
        }
        RecallAction::Campaign => {
            let source = required("source", lookup.source.as_ref())?;
            let campaign = required("campaign", lookup.campaign.as_ref())?;
            // The API answers 404 for a publisher it does not know and for
            // a number that is not letters and digits. Neither is ever put
            // in an address.
            let known = matches!(source.as_deref(), Some("nhtsa" | "tc"));
            let sound = campaign.as_deref().is_some_and(|number| {
                number.len() <= LONGEST_CAMPAIGN
                    && number.bytes().all(|byte| byte.is_ascii_alphanumeric())
            });
            if !known || !sound {
                return Err(CliError::new(NOT_FOUND, NO_CAMPAIGN));
            }
            RecallLookup {
                source,
                campaign,
                ..RecallLookup::default()
            }
        }
        RecallAction::Search => {
            let make = present(lookup.make.as_ref());
            let model = present(lookup.model.as_ref());
            if model.is_some() && make.is_none() {
                return Err(CliError::validation("make", "model needs make"));
            }
            RecallLookup {
                make,
                model,
                year: lookup.year,
                component: present(lookup.component.as_ref()),
                query: present(lookup.query.as_ref()),
                country,
                limit: Some(
                    lookup
                        .limit
                        .unwrap_or(RECALLS_DEFAULT)
                        .clamp(1, RECALLS_MOST),
                ),
                cursor: present(lookup.cursor.as_ref()),
                ..RecallLookup::default()
            }
        }
        // `since` is judged where the answer is made, after the recalls
        // are looked for, as the API does it. It goes in the query string
        // and never in a path.
        RecallAction::Recent => RecallLookup {
            since: present(lookup.since.as_ref()),
            make: present(lookup.make.as_ref()),
            country,
            limit: Some(
                lookup
                    .limit
                    .unwrap_or(RECALLS_DEFAULT)
                    .clamp(1, RECALLS_MOST),
            ),
            cursor: present(lookup.cursor.as_ref()),
            ..RecallLookup::default()
        },
    })
}

/// The filters every bulletin list may carry, with the API's limit.
fn bulletin_filters(lookup: &BulletinLookup) -> BulletinLookup {
    BulletinLookup {
        kind: present(lookup.kind.as_ref()),
        component: present(lookup.component.as_ref()),
        code: present(lookup.code.as_ref()),
        topic: present(lookup.topic.as_ref()),
        query: present(lookup.query.as_ref()),
        limit: Some(
            lookup
                .limit
                .unwrap_or(BULLETINS_DEFAULT)
                .clamp(1, BULLETINS_MOST),
        ),
        cursor: present(lookup.cursor.as_ref()),
        ..BulletinLookup::default()
    }
}

/// A bulletin question with the API's rules applied.
fn checked_bulletins(
    action: BulletinAction,
    lookup: BulletinLookup,
    current_year: u16,
) -> Result<BulletinLookup, CliError> {
    Ok(match action {
        BulletinAction::Vehicle => BulletinLookup {
            id: required("id", lookup.id.as_ref())?,
            ..bulletin_filters(&lookup)
        },
        BulletinAction::Vin => {
            let Some(vin) = lookup.vin.as_ref() else {
                return Err(CliError::validation("vin", "vin is required"));
            };
            vin_year(lookup.year, current_year)?;
            BulletinLookup {
                vin: Some(well_formed(vin)?.as_str().to_owned()),
                year: lookup.year,
                ..bulletin_filters(&lookup)
            }
        }
        BulletinAction::Get => match lookup.nhtsa_id {
            Some(nhtsa_id) => BulletinLookup {
                nhtsa_id: Some(nhtsa_id),
                ..BulletinLookup::default()
            },
            None => {
                return Err(CliError::validation("nhtsa_id", "nhtsa_id is required"));
            }
        },
        BulletinAction::Search => {
            let make = present(lookup.make.as_ref());
            let model = present(lookup.model.as_ref());
            if model.is_some() && make.is_none() {
                return Err(CliError::validation("make", "model needs make"));
            }
            BulletinLookup {
                make,
                model,
                year: lookup.year,
                ..bulletin_filters(&lookup)
            }
        }
    })
}

impl Request {
    /// The request with the API's rules applied: required fields present,
    /// text cut to length, limits within bounds. Fields the action does not
    /// use are dropped.
    pub fn checked(self, current_year: u16) -> Result<Request, CliError> {
        match self {
            Request::VinDecode { vin, year } => {
                vin_year(year, current_year)?;
                let vin = well_formed(&vin)?.as_str().to_owned();
                Ok(Request::VinDecode { vin, year })
            }
            Request::VinBatch { vins, year } => {
                if vins.len() > MOST_VINS {
                    return Err(CliError::new(
                        VALIDATION_FAILED,
                        format!("a batch holds at most {MOST_VINS} VINs"),
                    )
                    .with_details(
                        json!({ "field": "vins", "max": MOST_VINS, "received": vins.len() }),
                    ));
                }
                vin_year(year, current_year)?;
                Ok(Request::VinBatch { vins, year })
            }
            Request::VinOverview { vin, year } => {
                vin_year(year, current_year)?;
                let vin = well_formed(&vin)?.as_str().to_owned();
                Ok(Request::VinOverview { vin, year })
            }
            Request::VehicleFacts { topic, id, country } => {
                let id = required("id", Some(&id))?.unwrap_or_default();
                Ok(Request::VehicleFacts {
                    topic,
                    id,
                    country: fuel_country(topic, country.as_ref())?,
                })
            }
            Request::VinFacts {
                topic,
                vin,
                year,
                country,
            } => {
                vin_year(year, current_year)?;
                let vin = well_formed(&vin)?.as_str().to_owned();
                Ok(Request::VinFacts {
                    topic,
                    vin,
                    year,
                    country: fuel_country(topic, country.as_ref())?,
                })
            }
            Request::Recalls { action, lookup } => Ok(Request::Recalls {
                action,
                lookup: checked_recalls(action, lookup, current_year)?,
            }),
            Request::Codes { action, lookup } => Ok(Request::Codes {
                action,
                lookup: checked_codes(action, lookup)?,
            }),
            Request::Labor { action, lookup } => Ok(Request::Labor {
                action,
                lookup: checked_labor(action, lookup, current_year)?,
            }),
            Request::Bulletins { action, lookup } => Ok(Request::Bulletins {
                action,
                lookup: checked_bulletins(action, lookup, current_year)?,
            }),
            // `payroll_calc` checks the query when it works the pay out.
            Request::Payroll { query } => Ok(Request::Payroll { query }),
            Request::Vehicles { action, lookup } => {
                let term = present(lookup.term.as_ref());
                let lists = |limit: Option<usize>| Some(limit.unwrap_or(MOST).clamp(1, MOST));
                let checked = match action {
                    Action::Years => Lookup {
                        scope: scope(lookup.scope.as_ref())?,
                        term,
                        ..Lookup::default()
                    },
                    Action::Makes => Lookup {
                        year: lookup.year,
                        scope: scope(lookup.scope.as_ref())?,
                        term,
                        limit: lists(lookup.limit),
                        ..Lookup::default()
                    },
                    Action::Models => Lookup {
                        make: required("make", lookup.make.as_ref())?,
                        year: lookup.year,
                        scope: scope(lookup.scope.as_ref())?,
                        term,
                        limit: lists(lookup.limit),
                        ..Lookup::default()
                    },
                    Action::Submodels | Action::Engines => {
                        let make = required("make", lookup.make.as_ref())?;
                        let model = required("model", lookup.model.as_ref())?;
                        if lookup.year.is_none() {
                            return Err(CliError::validation("year", "year is required"));
                        }
                        Lookup {
                            make,
                            model,
                            year: lookup.year,
                            submodel: match action {
                                Action::Engines => present(lookup.submodel.as_ref()),
                                _ => None,
                            },
                            term,
                            ..Lookup::default()
                        }
                    }
                    Action::Search => Lookup {
                        query: required("query", lookup.query.as_ref())?,
                        scope: scope(lookup.scope.as_ref())?,
                        limit: Some(lookup.limit.unwrap_or(SEARCH_DEFAULT).clamp(1, SEARCH_MOST)),
                        ..Lookup::default()
                    },
                    Action::ModelYears => Lookup {
                        make: required("make", lookup.make.as_ref())?,
                        model: required("model", lookup.model.as_ref())?,
                        ..Lookup::default()
                    },
                    Action::Entry | Action::Overview | Action::Safety | Action::Complaints => {
                        Lookup {
                            id: required("id", lookup.id.as_ref())?,
                            ..Lookup::default()
                        }
                    }
                };
                Ok(Request::Vehicles {
                    action,
                    lookup: checked,
                })
            }
        }
    }

    /// Reads an MCP tool call. `arguments` must be an object; the caller
    /// has checked that, and that the tool is one of `open_mcp::TOOL_NAMES`.
    pub fn from_tool(tool: &str, arguments: &Value) -> Result<Request, CliError> {
        let action = match arguments.get("action") {
            Some(Value::String(action)) => action.as_str(),
            _ => "",
        };
        if tool == "wenmar_vin" {
            let year = number::<u16>(arguments, "year")?;
            return match action {
                "decode" => match text(arguments, "vin")? {
                    Some(vin) => Ok(Request::VinDecode { vin, year }),
                    None => Err(CliError::validation(
                        "vin",
                        "vin is required for action=decode",
                    )),
                },
                "batch" => {
                    let vins = match arguments.get("vins") {
                        None | Some(Value::Null) => Vec::new(),
                        Some(Value::Array(items)) => items
                            .iter()
                            .map(|item| item.as_str().map(str::to_owned))
                            .collect::<Option<Vec<String>>>()
                            .ok_or_else(|| {
                                CliError::validation("vins", "vins must be a list of text")
                            })?,
                        Some(_) => {
                            return Err(CliError::validation(
                                "vins",
                                "vins must be a list of text",
                            ));
                        }
                    };
                    Ok(Request::VinBatch { vins, year })
                }
                "overview" => match text(arguments, "vin")? {
                    Some(vin) => Ok(Request::VinOverview { vin, year }),
                    None => Err(CliError::validation(
                        "vin",
                        "vin is required for action=overview",
                    )),
                },
                "fuel" | "specs" | "fluids" | "tires" => {
                    match (Topic::parse(action), text(arguments, "vin")?) {
                        (Some(topic), Some(vin)) => Ok(Request::VinFacts {
                            topic,
                            vin,
                            year,
                            country: text(arguments, "country")?,
                        }),
                        _ => Err(CliError::validation(
                            "vin",
                            format!("vin is required for action={action}"),
                        )),
                    }
                }
                _ => Err(CliError::validation(
                    "action",
                    "action must be decode, batch, overview, fuel, specs, fluids or tires",
                )),
            };
        }
        if tool == "wenmar_payroll_deductions" {
            let query: payroll_calc::Query = serde_json::from_value(arguments.clone())
                .map_err(|error| CliError::validation("arguments", error.to_string()))?;
            return Ok(Request::Payroll { query });
        }
        if tool == "wenmar_labor" {
            let Some(action) = LaborAction::parse(action) else {
                return Err(CliError::validation(
                    "action",
                    "action must be jobs, vehicle or vin",
                ));
            };
            return Ok(Request::Labor {
                action,
                lookup: LaborLookup {
                    job: text(arguments, "job")?,
                    id: text(arguments, "id")?,
                    vin: text(arguments, "vin")?,
                    year: number(arguments, "year")?,
                },
            });
        }
        if tool == "wenmar_codes" {
            let Some(action) = CodeAction::parse(action) else {
                return Err(CliError::validation(
                    "action",
                    "action must be get, search or vehicle",
                ));
            };
            return Ok(Request::Codes {
                action,
                lookup: CodeLookup {
                    code: text(arguments, "code")?,
                    id: text(arguments, "id")?,
                    query: text(arguments, "query")?,
                    system: text(arguments, "system")?,
                    limit: number(arguments, "limit")?,
                    cursor: text(arguments, "cursor")?,
                },
            });
        }
        if tool == "wenmar_recalls" {
            let Some(action) = RecallAction::parse(action) else {
                return Err(CliError::validation(
                    "action",
                    "action must be vehicle, vin, campaign, search or recent",
                ));
            };
            return Ok(Request::Recalls {
                action,
                lookup: RecallLookup {
                    id: text(arguments, "id")?,
                    vin: text(arguments, "vin")?,
                    year: number(arguments, "year")?,
                    source: text(arguments, "source")?,
                    campaign: text(arguments, "campaign")?,
                    make: text(arguments, "make")?,
                    model: text(arguments, "model")?,
                    component: text(arguments, "component")?,
                    query: text(arguments, "query")?,
                    country: text(arguments, "country")?,
                    limit: number(arguments, "limit")?,
                    cursor: text(arguments, "cursor")?,
                    since: text(arguments, "since")?,
                },
            });
        }
        if tool == "wenmar_bulletins" {
            let Some(action) = BulletinAction::parse(action) else {
                return Err(CliError::validation(
                    "action",
                    "action must be vehicle, vin, get or search",
                ));
            };
            return Ok(Request::Bulletins {
                action,
                lookup: BulletinLookup {
                    id: text(arguments, "id")?,
                    vin: text(arguments, "vin")?,
                    year: number(arguments, "year")?,
                    nhtsa_id: number(arguments, "nhtsa_id")?,
                    make: text(arguments, "make")?,
                    model: text(arguments, "model")?,
                    kind: text(arguments, "type")?,
                    component: text(arguments, "component")?,
                    code: text(arguments, "code")?,
                    topic: text(arguments, "topic")?,
                    query: text(arguments, "query")?,
                    limit: number(arguments, "limit")?,
                    cursor: text(arguments, "cursor")?,
                },
            });
        }
        if let Some(topic) = Topic::parse(action) {
            return match text(arguments, "id")? {
                Some(id) => Ok(Request::VehicleFacts {
                    topic,
                    id,
                    country: text(arguments, "country")?,
                }),
                None => Err(CliError::validation(
                    "id",
                    format!("id is required for action={action}"),
                )),
            };
        }
        let Some(action) = Action::parse(action) else {
            return Err(CliError::validation(
                "action",
                "action must be years, makes, models, submodels, engines, search, entry, overview, model_years, safety, complaints, fuel, specs, fluids or tires",
            ));
        };
        Ok(Request::Vehicles {
            action,
            lookup: Lookup {
                year: number(arguments, "year")?,
                make: text(arguments, "make")?,
                model: text(arguments, "model")?,
                submodel: text(arguments, "submodel")?,
                term: text(arguments, "term")?,
                query: text(arguments, "query")?,
                id: text(arguments, "id")?,
                scope: text(arguments, "scope")?,
                limit: number(arguments, "limit")?,
            },
        })
    }
}

/// A text argument. Missing and `null` are `None`; anything but text is
/// refused.
fn text(arguments: &Value, field: &'static str) -> Result<Option<String>, CliError> {
    match arguments.get(field) {
        None | Some(Value::Null) => Ok(None),
        Some(Value::String(text)) => Ok(Some(text.clone())),
        Some(_) => Err(CliError::validation(field, format!("{field} must be text"))),
    }
}

/// A whole-number argument, given as a number or as its digits. Missing,
/// `null` and blank text are `None`.
fn number<T: TryFrom<u64>>(arguments: &Value, field: &'static str) -> Result<Option<T>, CliError> {
    let refuse = || CliError::validation(field, format!("{field} must be a whole number"));
    let number = match arguments.get(field) {
        None | Some(Value::Null) => return Ok(None),
        Some(Value::String(text)) if text.trim().is_empty() => return Ok(None),
        Some(Value::String(text)) => text.trim().parse::<u64>().map_err(|_| refuse())?,
        Some(Value::Number(number)) => number.as_u64().ok_or_else(refuse)?,
        Some(_) => return Err(refuse()),
    };
    T::try_from(number).map(Some).map_err(|_| refuse())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn vehicles(action: Action, lookup: Lookup) -> Request {
        Request::Vehicles { action, lookup }
    }

    fn text(value: &str) -> Option<String> {
        Some(value.to_owned())
    }

    #[test]
    fn a_vin_is_normalized_and_a_malformed_one_gets_the_api_error() {
        let request = Request::VinDecode {
            vin: "km8-k2cab4 pu001140".to_owned(),
            year: None,
        };
        assert_eq!(
            request.checked(2026).unwrap(),
            Request::VinDecode {
                vin: "KM8K2CAB4PU001140".to_owned(),
                year: None
            }
        );

        let error = Request::VinDecode {
            vin: "KM8K2CAB4PUO01140".to_owned(),
            year: None,
        }
        .checked(2026)
        .unwrap_err();
        assert_eq!(
            error.body(),
            json!({
                "error": {
                    "code": "invalid_vin",
                    "message": "a VIN uses only digits and letters other than I, O and Q",
                    "details": {
                        "invalid_characters": [{ "character": "O", "position": 12 }],
                        "suggestions": ["KM8K2CAB4PU001140"]
                    }
                }
            })
        );

        let error = Request::VinDecode {
            vin: "nope".to_owned(),
            year: None,
        }
        .checked(2026)
        .unwrap_err();
        assert_eq!(error.message, "a VIN has 17 characters, this has 4");
        assert_eq!(error.details, json!({ "suggestions": [] }));
    }

    #[test]
    fn a_very_long_vin_is_refused_unread_and_not_repeated() {
        let long = "<script>".repeat(10_000);
        let error = Request::VinDecode {
            vin: long,
            year: None,
        }
        .checked(2026)
        .unwrap_err();
        assert_eq!(error.code, "invalid_vin");
        assert_eq!(error.message, "a VIN has 17 characters, this is far longer");
        assert!(!error.body().to_string().contains("script"));
    }

    #[test]
    fn a_vin_year_must_be_one_a_vin_can_mean() {
        for year in [1979, 2029, 0, u16::MAX] {
            let error = Request::VinDecode {
                vin: "KM8K2CAB4PU001140".to_owned(),
                year: Some(year),
            }
            .checked(2026)
            .unwrap_err();
            assert_eq!(error.code, "validation_failed", "{year}");
            assert_eq!(
                error.details,
                json!({ "field": "year", "min": 1980, "max": 2028 })
            );
        }
        for year in [1980, 2028] {
            let request = Request::VinDecode {
                vin: "KM8K2CAB4PU001140".to_owned(),
                year: Some(year),
            };
            assert!(request.checked(2026).is_ok(), "{year}");
        }
    }

    #[test]
    fn a_batch_holds_at_most_fifty() {
        let batch = |count: usize| Request::VinBatch {
            vins: vec!["KM8K2CAB4PU001140".to_owned(); count],
            year: None,
        };
        assert!(batch(50).checked(2026).is_ok());
        let error = batch(51).checked(2026).unwrap_err();
        assert_eq!(
            error.details,
            json!({ "field": "vins", "max": 50, "received": 51 })
        );
        // Each VIN is judged when it is decoded, not here.
        let mixed = Request::VinBatch {
            vins: vec!["nope".to_owned()],
            year: None,
        };
        assert!(mixed.checked(2026).is_ok());
    }

    #[test]
    fn required_fields_are_named() {
        let cases = [
            (Action::Models, Lookup::default(), "make"),
            (
                Action::Models,
                Lookup {
                    make: text("   "),
                    ..Lookup::default()
                },
                "make",
            ),
            (
                Action::Submodels,
                Lookup {
                    make: text("honda"),
                    ..Lookup::default()
                },
                "model",
            ),
            (
                Action::Engines,
                Lookup {
                    make: text("honda"),
                    model: text("civic"),
                    ..Lookup::default()
                },
                "year",
            ),
            (Action::Search, Lookup::default(), "query"),
            (Action::Entry, Lookup::default(), "id"),
            (Action::Complaints, Lookup::default(), "id"),
            (
                Action::Makes,
                Lookup {
                    scope: text("heavy"),
                    ..Lookup::default()
                },
                "scope",
            ),
        ];
        for (action, lookup, field) in cases {
            let error = vehicles(action, lookup).checked(2026).unwrap_err();
            assert_eq!(error.code, "validation_failed", "{action:?}");
            assert_eq!(error.details["field"], field, "{action:?}");
        }
        let error = vehicles(Action::Models, Lookup::default())
            .checked(2026)
            .unwrap_err();
        assert_eq!(error.message, "make is required");
    }

    #[test]
    fn limits_are_lowered_not_refused_and_text_is_cut() {
        let Request::Vehicles { lookup, .. } = vehicles(
            Action::Makes,
            Lookup {
                limit: Some(100_000),
                term: text(&"x".repeat(5_000)),
                scope: text(" ALL "),
                ..Lookup::default()
            },
        )
        .checked(2026)
        .unwrap() else {
            panic!("a vehicles request stays one");
        };
        assert_eq!(lookup.limit, Some(500));
        assert_eq!(lookup.term.unwrap().len(), 200);
        assert_eq!(lookup.scope.as_deref(), Some("ALL"));

        let limit = |given: Option<usize>| {
            let Request::Vehicles { lookup, .. } = vehicles(
                Action::Search,
                Lookup {
                    query: text("civic"),
                    limit: given,
                    ..Lookup::default()
                },
            )
            .checked(2026)
            .unwrap() else {
                panic!("a vehicles request stays one");
            };
            lookup.limit
        };
        assert_eq!(limit(None), Some(10));
        assert_eq!(limit(Some(0)), Some(1));
        assert_eq!(limit(Some(51)), Some(50));
    }

    #[test]
    fn fields_an_action_does_not_use_are_dropped() {
        let Request::Vehicles { lookup, .. } = vehicles(
            Action::Years,
            Lookup {
                make: text("honda"),
                id: text("x"),
                limit: Some(3),
                ..Lookup::default()
            },
        )
        .checked(2026)
        .unwrap() else {
            panic!("a vehicles request stays one");
        };
        assert_eq!(lookup, Lookup::default());
    }

    #[test]
    fn a_tool_call_is_read_into_a_request() {
        assert_eq!(
            Request::from_tool(
                "wenmar_vin",
                &json!({ "action": "decode", "vin": "KM8K2CAB4PU001140", "year": "2023" })
            )
            .unwrap(),
            Request::VinDecode {
                vin: "KM8K2CAB4PU001140".to_owned(),
                year: Some(2023)
            }
        );
        assert_eq!(
            Request::from_tool(
                "wenmar_vin",
                &json!({ "action": "batch", "vins": ["a", "b"] })
            )
            .unwrap(),
            Request::VinBatch {
                vins: vec!["a".to_owned(), "b".to_owned()],
                year: None
            }
        );
        assert_eq!(
            Request::from_tool(
                "wenmar_vehicles",
                &json!({ "action": "engines", "make": "honda", "model": "civic", "year": 2019, "submodel": "si", "limit": "" })
            )
            .unwrap(),
            vehicles(
                Action::Engines,
                Lookup {
                    make: text("honda"),
                    model: text("civic"),
                    year: Some(2019),
                    submodel: text("si"),
                    ..Lookup::default()
                }
            )
        );
    }

    fn recalls(action: RecallAction, lookup: RecallLookup) -> Request {
        Request::Recalls { action, lookup }
    }

    #[test]
    fn a_recall_question_is_checked_as_the_api_checks_it() {
        // A vehicle id is required; the country is lowered.
        let Request::Recalls { lookup, .. } = recalls(
            RecallAction::Vehicle,
            RecallLookup {
                id: text(" 2023_hyundai_kona "),
                country: text("CA"),
                make: text("dropped"),
                ..RecallLookup::default()
            },
        )
        .checked(2026)
        .unwrap() else {
            panic!("a recall request stays one");
        };
        assert_eq!(
            lookup,
            RecallLookup {
                id: text("2023_hyundai_kona"),
                country: text("ca"),
                ..RecallLookup::default()
            }
        );

        // A VIN is normalized, and a malformed one gets the API's error.
        let Request::Recalls { lookup, .. } = recalls(
            RecallAction::Vin,
            RecallLookup {
                vin: text("km8-k2cab4 pu001140"),
                year: Some(2023),
                ..RecallLookup::default()
            },
        )
        .checked(2026)
        .unwrap() else {
            panic!("a recall request stays one");
        };
        assert_eq!(lookup.vin.as_deref(), Some("KM8K2CAB4PU001140"));
        assert_eq!(lookup.year, Some(2023));
        let error = recalls(
            RecallAction::Vin,
            RecallLookup {
                vin: text("nope"),
                ..RecallLookup::default()
            },
        )
        .checked(2026)
        .unwrap_err();
        assert_eq!(error.code, "invalid_vin");

        // A list has the API's default and its ceiling.
        let limit = |given: Option<usize>| {
            let Request::Recalls { lookup, .. } = recalls(
                RecallAction::Search,
                RecallLookup {
                    limit: given,
                    ..RecallLookup::default()
                },
            )
            .checked(2026)
            .unwrap() else {
                panic!("a recall request stays one");
            };
            lookup.limit
        };
        assert_eq!(limit(None), Some(25));
        assert_eq!(limit(Some(0)), Some(1));
        assert_eq!(limit(Some(101)), Some(100));
    }

    #[test]
    fn a_recall_question_that_is_wrong_is_refused_before_anything_is_read() {
        let cases = [
            (RecallAction::Vehicle, RecallLookup::default(), "id"),
            (RecallAction::Vin, RecallLookup::default(), "vin"),
            (RecallAction::Campaign, RecallLookup::default(), "source"),
            (
                RecallAction::Campaign,
                RecallLookup {
                    source: text("nhtsa"),
                    ..RecallLookup::default()
                },
                "campaign",
            ),
            (
                RecallAction::Search,
                RecallLookup {
                    model: text("kona"),
                    ..RecallLookup::default()
                },
                "make",
            ),
            (
                RecallAction::Search,
                RecallLookup {
                    country: text("mx"),
                    ..RecallLookup::default()
                },
                "country",
            ),
            (
                RecallAction::Vin,
                RecallLookup {
                    vin: text("KM8K2CAB4PU001140"),
                    year: Some(1979),
                    ..RecallLookup::default()
                },
                "year",
            ),
        ];
        for (action, lookup, field) in cases {
            let error = recalls(action, lookup).checked(2026).unwrap_err();
            assert_eq!(error.code, "validation_failed", "{action:?}");
            assert_eq!(error.details["field"], field, "{action:?}");
        }
        // A publisher the API does not know, and a number that could not be
        // a campaign's, are what the API answers 404 for.
        for (source, campaign) in [
            ("epa", "19V888000"),
            ("nhtsa", "19V/888"),
            ("nhtsa", "../../etc"),
            ("tc", "123456789012345678901"),
        ] {
            let error = recalls(
                RecallAction::Campaign,
                RecallLookup {
                    source: text(source),
                    campaign: text(campaign),
                    ..RecallLookup::default()
                },
            )
            .checked(2026)
            .unwrap_err();
            assert_eq!(error.code, "not_found", "{source} {campaign}");
            assert_eq!(error.message, "No recall campaign has that number.");
        }
    }

    fn bulletins(action: BulletinAction, lookup: BulletinLookup) -> Request {
        Request::Bulletins { action, lookup }
    }

    #[test]
    fn a_bulletin_question_is_checked_as_the_api_checks_it() {
        // A model needs a make; the list's limit is set and blank text dropped.
        let cases = [
            (BulletinAction::Vehicle, BulletinLookup::default(), "id"),
            (BulletinAction::Vin, BulletinLookup::default(), "vin"),
            (BulletinAction::Get, BulletinLookup::default(), "nhtsa_id"),
            (
                BulletinAction::Search,
                BulletinLookup {
                    model: text("kona"),
                    ..BulletinLookup::default()
                },
                "make",
            ),
        ];
        for (action, lookup, field) in cases {
            let error = bulletins(action, lookup).checked(2026).unwrap_err();
            assert_eq!(error.code, "validation_failed", "{field}");
            assert_eq!(error.details["field"], field, "{field}");
        }
        assert_eq!(
            bulletins(BulletinAction::Search, BulletinLookup::default())
                .checked(2026)
                .unwrap(),
            bulletins(
                BulletinAction::Search,
                BulletinLookup {
                    limit: Some(BULLETINS_DEFAULT),
                    ..BulletinLookup::default()
                }
            )
        );
        let Request::Bulletins { lookup, .. } = bulletins(
            BulletinAction::Vehicle,
            BulletinLookup {
                id: text(" 2023_hyundai_kona "),
                kind: text("   "),
                limit: Some(500),
                make: text("dropped"),
                ..BulletinLookup::default()
            },
        )
        .checked(2026)
        .unwrap() else {
            panic!("a bulletins request");
        };
        assert_eq!(lookup.id, text("2023_hyundai_kona"));
        assert_eq!(lookup.kind, None);
        assert_eq!(lookup.limit, Some(BULLETINS_MOST));
        assert_eq!(lookup.make, None);
    }

    #[test]
    fn a_bulletins_tool_call_is_read_into_a_request() {
        assert_eq!(
            Request::from_tool(
                "wenmar_bulletins",
                &json!({ "action": "get", "nhtsa_id": 10152456 })
            )
            .unwrap(),
            bulletins(
                BulletinAction::Get,
                BulletinLookup {
                    nhtsa_id: Some(10_152_456),
                    ..BulletinLookup::default()
                }
            )
        );
        assert_eq!(
            Request::from_tool(
                "wenmar_bulletins",
                &json!({ "action": "vehicle", "id": "2018_ford_f-150", "type": "warranty_extension", "query": "noise" })
            )
            .unwrap(),
            bulletins(
                BulletinAction::Vehicle,
                BulletinLookup {
                    id: text("2018_ford_f-150"),
                    kind: text("warranty_extension"),
                    query: text("noise"),
                    ..BulletinLookup::default()
                }
            )
        );
        for (arguments, field) in [
            (json!({ "action": "recall" }), "action"),
            (json!({ "action": "get", "nhtsa_id": "ten" }), "nhtsa_id"),
        ] {
            let error = Request::from_tool("wenmar_bulletins", &arguments).unwrap_err();
            assert_eq!(error.code, "validation_failed", "{arguments}");
            assert_eq!(error.details["field"], field, "{arguments}");
        }
    }

    #[test]
    fn a_recalls_tool_call_is_read_into_a_request() {
        assert_eq!(
            Request::from_tool(
                "wenmar_recalls",
                &json!({ "action": "search", "make": "ford", "year": "2018", "query": "tailgate", "limit": 5, "cursor": "2019-12-12_41" })
            )
            .unwrap(),
            recalls(
                RecallAction::Search,
                RecallLookup {
                    make: text("ford"),
                    year: Some(2018),
                    query: text("tailgate"),
                    limit: Some(5),
                    cursor: text("2019-12-12_41"),
                    ..RecallLookup::default()
                }
            )
        );
        assert_eq!(
            Request::from_tool(
                "wenmar_recalls",
                &json!({ "action": "campaign", "source": "tc", "campaign": "2023101" })
            )
            .unwrap(),
            recalls(
                RecallAction::Campaign,
                RecallLookup {
                    source: text("tc"),
                    campaign: text("2023101"),
                    ..RecallLookup::default()
                }
            )
        );
        for (arguments, field) in [
            (json!({ "action": "recall" }), "action"),
            (json!({}), "action"),
            (json!({ "action": "vehicle", "id": 7 }), "id"),
            (json!({ "action": "search", "limit": "many" }), "limit"),
        ] {
            let error = Request::from_tool("wenmar_recalls", &arguments).unwrap_err();
            assert_eq!(error.code, "validation_failed", "{arguments}");
            assert_eq!(error.details["field"], field, "{arguments}");
        }
    }

    #[test]
    fn the_overview_questions_are_checked_and_read_from_tool_calls() {
        // A vehicle id, or a make and a model, is required.
        let error = vehicles(Action::Overview, Lookup::default())
            .checked(2026)
            .unwrap_err();
        assert_eq!(error.details["field"], "id");
        let error = vehicles(
            Action::ModelYears,
            Lookup {
                make: text("honda"),
                ..Lookup::default()
            },
        )
        .checked(2026)
        .unwrap_err();
        assert_eq!(error.details["field"], "model");
        // The overview of a VIN is checked as a decode is.
        let overview = |vin: &str, year: Option<u16>| Request::VinOverview {
            vin: vin.to_owned(),
            year,
        };
        assert_eq!(
            overview("km8-k2cab4 pu001140", None).checked(2026).unwrap(),
            overview("KM8K2CAB4PU001140", None)
        );
        assert_eq!(
            overview("nope", None).checked(2026).unwrap_err().code,
            "invalid_vin"
        );
        let error = overview("KM8K2CAB4PU001140", Some(1979))
            .checked(2026)
            .unwrap_err();
        assert_eq!(error.details["field"], "year");
        // The recent list has the API's default and ceiling. `since` is
        // judged where the answer is made, so it is only tidied here.
        let Request::Recalls { lookup, .. } = recalls(
            RecallAction::Recent,
            RecallLookup {
                since: text(" 2026-09-01 "),
                country: text("CA"),
                year: Some(2020),
                ..RecallLookup::default()
            },
        )
        .checked(2026)
        .unwrap() else {
            panic!("a recall request stays one");
        };
        assert_eq!(
            lookup,
            RecallLookup {
                since: text("2026-09-01"),
                country: text("ca"),
                limit: Some(25),
                ..RecallLookup::default()
            }
        );
        // The same three actions, as tool calls.
        assert_eq!(
            Request::from_tool(
                "wenmar_vin",
                &json!({ "action": "overview", "vin": "KM8K2CAB4PU001140", "year": 2023 })
            )
            .unwrap(),
            overview("KM8K2CAB4PU001140", Some(2023))
        );
        assert_eq!(
            Request::from_tool(
                "wenmar_vehicles",
                &json!({ "action": "model_years", "make": "ford", "model": "f-150" })
            )
            .unwrap(),
            vehicles(
                Action::ModelYears,
                Lookup {
                    make: text("ford"),
                    model: text("f-150"),
                    ..Lookup::default()
                }
            )
        );
        assert_eq!(
            Request::from_tool(
                "wenmar_recalls",
                &json!({ "action": "recent", "since": "2026-09-01", "make": "ford" })
            )
            .unwrap(),
            recalls(
                RecallAction::Recent,
                RecallLookup {
                    since: text("2026-09-01"),
                    make: text("ford"),
                    ..RecallLookup::default()
                }
            )
        );
        let error = Request::from_tool("wenmar_vin", &json!({ "action": "overview" })).unwrap_err();
        assert_eq!(error.details["field"], "vin");
    }

    #[test]
    fn fuel_specs_fluids_and_tires_are_checked_and_read_from_a_tool_call() {
        for topic in Topic::ALL {
            assert_eq!(Topic::parse(topic.name()), Some(topic));
            // By vehicle id.
            let asked = Request::from_tool(
                "wenmar_vehicles",
                &json!({ "action": topic.name(), "id": " 2018_ford_f-150 ", "country": "CA" }),
            )
            .unwrap();
            assert_eq!(
                asked.checked(2026).unwrap(),
                Request::VehicleFacts {
                    topic,
                    id: "2018_ford_f-150".to_owned(),
                    // Only a fuel question has a country.
                    country: (topic == Topic::Fuel).then(|| "ca".to_owned()),
                }
            );
            let error = Request::from_tool("wenmar_vehicles", &json!({ "action": topic.name() }))
                .unwrap_err();
            assert_eq!(error.code, "validation_failed");
            assert_eq!(error.details["field"], "id");
            // By VIN.
            let asked = Request::from_tool(
                "wenmar_vin",
                &json!({ "action": topic.name(), "vin": "km8k2cab4pu001140", "year": 2023 }),
            )
            .unwrap();
            assert_eq!(
                asked.checked(2026).unwrap(),
                Request::VinFacts {
                    topic,
                    vin: "KM8K2CAB4PU001140".to_owned(),
                    year: Some(2023),
                    country: None,
                }
            );
            let error =
                Request::from_tool("wenmar_vin", &json!({ "action": topic.name() })).unwrap_err();
            assert_eq!(error.details["field"], "vin");
            let malformed = Request::VinFacts {
                topic,
                vin: "NOTAVIN".to_owned(),
                year: None,
                country: None,
            };
            assert_eq!(malformed.checked(2026).unwrap_err().code, "invalid_vin");
        }
        // A country that is neither is refused for fuel, with the API's words.
        let error = Request::VehicleFacts {
            topic: Topic::Fuel,
            id: "2018_ford_f-150".to_owned(),
            country: Some("mx".to_owned()),
        }
        .checked(2026)
        .unwrap_err();
        assert_eq!(error.message, "country must be us or ca");
        assert_eq!(error.details["field"], "country");
        assert_eq!(Topic::Fluids.list(), "engine_oil");
    }

    #[test]
    fn the_safety_question_needs_a_vehicle_id_and_is_read_from_a_tool_call() {
        let error = vehicles(Action::Safety, Lookup::default())
            .checked(2026)
            .unwrap_err();
        assert_eq!(error.code, "validation_failed");
        assert_eq!(error.details["field"], "id");
        let asked = Request::from_tool(
            "wenmar_vehicles",
            &json!({ "action": "safety", "id": " 2018_ford_f-150 ", "make": "dropped" }),
        )
        .unwrap();
        assert_eq!(
            asked.checked(2026).unwrap(),
            vehicles(
                Action::Safety,
                Lookup {
                    id: text("2018_ford_f-150"),
                    ..Lookup::default()
                }
            )
        );
        assert_eq!(Action::Safety.name(), "safety");
        assert!(Action::ALL.contains(&Action::Safety));
    }

    #[test]
    fn the_complaints_question_needs_a_vehicle_id_and_is_read_from_a_tool_call() {
        let error = vehicles(Action::Complaints, Lookup::default())
            .checked(2026)
            .unwrap_err();
        assert_eq!(error.code, "validation_failed");
        assert_eq!(error.details["field"], "id");
        let asked = Request::from_tool(
            "wenmar_vehicles",
            &json!({ "action": "complaints", "id": " 2016_honda_civic ", "make": "dropped" }),
        )
        .unwrap();
        assert_eq!(
            asked.checked(2026).unwrap(),
            vehicles(
                Action::Complaints,
                Lookup {
                    id: text("2016_honda_civic"),
                    ..Lookup::default()
                }
            )
        );
        assert_eq!(Action::Complaints.name(), "complaints");
        assert!(Action::ALL.contains(&Action::Complaints));
    }

    #[test]
    fn a_tool_call_that_is_wrong_is_validation_failed() {
        let cases = [
            ("wenmar_vin", json!({ "action": "decode" }), "vin"),
            ("wenmar_vin", json!({ "action": "explode" }), "action"),
            ("wenmar_vin", json!({}), "action"),
            ("wenmar_vin", json!({ "action": 7 }), "action"),
            (
                "wenmar_vin",
                json!({ "action": "decode", "vin": 17 }),
                "vin",
            ),
            (
                "wenmar_vin",
                json!({ "action": "batch", "vins": "KM8K2CAB4PU001140" }),
                "vins",
            ),
            (
                "wenmar_vin",
                json!({ "action": "batch", "vins": [1, 2] }),
                "vins",
            ),
            ("wenmar_vehicles", json!({ "action": "fly" }), "action"),
            (
                "wenmar_vehicles",
                json!({ "action": "makes", "year": "soon" }),
                "year",
            ),
            (
                "wenmar_vehicles",
                json!({ "action": "makes", "year": 70_000 }),
                "year",
            ),
            (
                "wenmar_vehicles",
                json!({ "action": "makes", "year": -1 }),
                "year",
            ),
            (
                "wenmar_vehicles",
                json!({ "action": "makes", "year": 2019.5 }),
                "year",
            ),
            (
                "wenmar_vehicles",
                json!({ "action": "makes", "limit": [1] }),
                "limit",
            ),
            (
                "wenmar_vehicles",
                json!({ "action": "models", "make": { "name": "honda" } }),
                "make",
            ),
        ];
        for (tool, arguments, field) in cases {
            let error = Request::from_tool(tool, &arguments).unwrap_err();
            assert_eq!(error.code, "validation_failed", "{arguments}");
            assert_eq!(error.details["field"], field, "{arguments}");
        }
    }
}
