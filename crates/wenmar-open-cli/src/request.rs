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

use crate::error::{CliError, INVALID_VIN, VALIDATION_FAILED};

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
}

impl Action {
    pub const ALL: [Action; 7] = [
        Action::Years,
        Action::Makes,
        Action::Models,
        Action::Submodels,
        Action::Engines,
        Action::Search,
        Action::Entry,
    ];

    /// The name of the action in the `wenmar_vehicles` tool, and the last
    /// part of the API's path.
    pub fn name(self) -> &'static str {
        match self {
            Action::Years => "years",
            Action::Makes => "makes",
            Action::Models => "models",
            Action::Submodels => "submodels",
            Action::Engines => "engines",
            Action::Search => "search",
            Action::Entry => "entry",
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
    Vehicles {
        action: Action,
        lookup: Lookup,
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
                    Action::Entry => Lookup {
                        id: required("id", lookup.id.as_ref())?,
                        ..Lookup::default()
                    },
                };
                Ok(Request::Vehicles {
                    action,
                    lookup: checked,
                })
            }
        }
    }

    /// Reads an MCP tool call. `arguments` must be an object; the caller
    /// has checked that, and that the tool is one of the two.
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
                _ => Err(CliError::validation(
                    "action",
                    "action must be decode or batch",
                )),
            };
        }
        let Some(action) = Action::parse(action) else {
            return Err(CliError::validation(
                "action",
                "action must be years, makes, models, submodels, engines, search or entry",
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
