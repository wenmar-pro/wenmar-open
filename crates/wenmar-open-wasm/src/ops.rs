//! The operations, each as the hosted API answers it: the same checks, the
//! same limits, the same JSON and the same errors.

use serde::Serialize;
use serde_json::{Value, json};
use wenmar_shopdata::answer::{self, AnswerError, Answers, CodeSearch, Overviews, Recent, Search};
use wenmar_shopdata::recalls::RecallSource;
use wenmar_shopdata::{Dataset, ShopError};
use wenmar_vehicles::{Catalog, Scope, vin_rows};
use wenmar_vin::{DecodeOptions, Decoder, Vin};

use crate::error::{DATA_INVALID, INVALID_VIN, OpError};
use crate::replay::Replay;

/// Longest text read as a VIN, in bytes. Anything longer is refused unread.
pub const LONGEST_INPUT: usize = 64;
/// The first model year a 17-character VIN can have.
pub const FIRST_YEAR: u16 = 1980;
/// Most items in a list, whatever `limit` says.
pub const MOST: usize = 500;
/// Search results when `limit` is not given, and the most it may be.
pub const SEARCH_DEFAULT: usize = 10;
pub const SEARCH_MOST: usize = 50;
/// Longest search text read, in characters. The rest is ignored.
pub const LONGEST_QUERY: usize = 200;

fn json<T: Serialize>(value: T) -> Result<Value, OpError> {
    serde_json::to_value(value)
        .map_err(|error| OpError::internal(format!("an answer could not be written: {error}")))
}

/// A text argument. Nothing, `null` and blank text are all "not given". A
/// number is read as its digits, as a query string would carry it.
fn text(args: &Value, field: &str) -> Result<Option<String>, OpError> {
    match args.get(field) {
        None | Some(Value::Null) => Ok(None),
        Some(Value::String(text)) if text.trim().is_empty() => Ok(None),
        Some(Value::String(text)) => Ok(Some(text.clone())),
        Some(Value::Number(number)) => Ok(Some(number.to_string())),
        Some(_) => Err(OpError::validation(field, format!("{field} must be text"))),
    }
}

/// A whole number that is not negative, given as a number or as digits.
fn whole(args: &Value, field: &str) -> Result<Option<u64>, OpError> {
    let wrong = || OpError::validation(field, format!("{field} must be a whole number"));
    match args.get(field) {
        None | Some(Value::Null) => Ok(None),
        Some(Value::String(text)) if text.trim().is_empty() => Ok(None),
        Some(Value::String(text)) => text.trim().parse().map(Some).map_err(|_| wrong()),
        Some(Value::Number(number)) => number.as_u64().map(Some).ok_or_else(wrong),
        Some(_) => Err(wrong()),
    }
}

fn year(args: &Value) -> Result<Option<u16>, OpError> {
    match whole(args, "year")? {
        None => Ok(None),
        Some(year) => u16::try_from(year)
            .map(Some)
            .map_err(|_| OpError::validation("year", "year must be a whole number")),
    }
}

fn required(args: &Value, field: &str) -> Result<String, OpError> {
    text(args, field)?.ok_or_else(|| OpError::validation(field, format!("{field} is required")))
}

fn required_year(args: &Value) -> Result<u16, OpError> {
    year(args)?.ok_or_else(|| OpError::validation("year", "year is required"))
}

fn scope(args: &Value) -> Result<Scope, OpError> {
    match text(args, "scope")? {
        None => Ok(Scope::Light),
        Some(text) => Scope::parse(&text).ok_or_else(|| {
            OpError::validation("scope", "scope must be light, all, or a vehicle type id")
        }),
    }
}

fn limit(args: &Value, default: usize, most: usize) -> Result<usize, OpError> {
    let given = whole(args, "limit")?.map(|limit| usize::try_from(limit).unwrap_or(usize::MAX));
    Ok(given.unwrap_or(default).clamp(1, most))
}

fn term(args: &Value) -> Result<String, OpError> {
    Ok(text(args, "term")?.unwrap_or_default())
}

/// `{ "vin", "year"? }`: one decode, with the catalog entry it reaches.
pub fn decode(
    catalog: &Catalog<Replay>,
    replay: &Replay,
    args: &Value,
    current_year: u16,
) -> Result<Value, OpError> {
    let input = match args.get("vin") {
        Some(Value::String(text)) => text.as_str(),
        _ => "",
    };
    if input.len() > LONGEST_INPUT {
        return Err(
            OpError::new(INVALID_VIN, "a VIN has 17 characters, this is far longer")
                .with_details(json!({ "suggestions": [] })),
        );
    }
    let model_year = year(args)?;
    let latest = current_year.saturating_add(2);
    if let Some(year) = model_year
        && !(FIRST_YEAR..=latest).contains(&year)
    {
        return Err(OpError::new(
            crate::error::VALIDATION_FAILED,
            format!("year must be between {FIRST_YEAR} and {latest}"),
        )
        .with_details(json!({ "field": "year", "min": FIRST_YEAR, "max": latest })));
    }
    // The rows are fetched for the same years the decoder will try, so both
    // are given the same current year. It is never read from a clock: this
    // build has none.
    let options = DecodeOptions {
        model_year,
        current_year: Some(current_year),
    };
    let rows = match Vin::parse(input) {
        Ok(vin) => vin_rows::fetch(replay, &vin, model_year, current_year)
            .map_err(|error| OpError::new(DATA_INVALID, format!("The data has an {error}.")))?,
        // The decoder says what is wrong with it and suggests corrections.
        Err(_) => vin_rows::VinRows::default(),
    };
    let decoded = Decoder::new(&rows).decode(input, options)?;
    let selection = catalog.selection(&decoded)?;
    let mut answer = json(&decoded)?;
    if let (Some(selection), Some(object)) = (selection, answer.as_object_mut()) {
        object.insert("catalog".to_owned(), json(selection)?);
    }
    Ok(answer)
}

/// `{ "scope"?, "term"? }`: model years, newest first.
pub fn years(catalog: &Catalog<Replay>, args: &Value) -> Result<Value, OpError> {
    let mut years = catalog.years(scope(args)?, &term(args)?)?;
    years.sort_unstable_by(|left, right| right.cmp(left));
    json(years)
}

/// `{ "year"?, "scope"?, "term"?, "limit"? }`
pub fn makes(catalog: &Catalog<Replay>, args: &Value) -> Result<Value, OpError> {
    let makes = catalog.makes(
        year(args)?,
        scope(args)?,
        &term(args)?,
        limit(args, MOST, MOST)?,
    )?;
    json(makes)
}

/// `{ "make", "year"?, "scope"?, "term"?, "limit"? }`
pub fn models(catalog: &Catalog<Replay>, args: &Value) -> Result<Value, OpError> {
    let make = required(args, "make")?;
    let models = catalog.models(
        &make,
        year(args)?,
        scope(args)?,
        &term(args)?,
        limit(args, MOST, MOST)?,
    )?;
    json(models)
}

/// `{ "make", "model", "year", "term"? }`
pub fn submodels(catalog: &Catalog<Replay>, args: &Value) -> Result<Value, OpError> {
    let make = required(args, "make")?;
    let model = required(args, "model")?;
    let year = required_year(args)?;
    json(catalog.submodels(&make, &model, year, &term(args)?)?)
}

/// `{ "make", "model", "year", "submodel"?, "term"? }`
pub fn engines(catalog: &Catalog<Replay>, args: &Value) -> Result<Value, OpError> {
    let make = required(args, "make")?;
    let model = required(args, "model")?;
    let year = required_year(args)?;
    let submodel = text(args, "submodel")?;
    json(catalog.engines(&make, &model, year, submodel.as_deref(), &term(args)?)?)
}

/// `{ "q", "scope"?, "limit"? }`: the catalog's own reading of the text. The
/// hosted API also has a full-text index for words in any order; that index
/// is not in the data file, so text the catalog cannot read finds nothing.
pub fn search(catalog: &Catalog<Replay>, args: &Value) -> Result<Value, OpError> {
    let text: String = required(args, "q")?.chars().take(LONGEST_QUERY).collect();
    let entries = catalog.search(
        &text,
        scope(args)?,
        limit(args, SEARCH_DEFAULT, SEARCH_MOST)?,
    )?;
    json(entries)
}

/// `{ "id" }`: one catalog entry.
pub fn vehicle(catalog: &Catalog<Replay>, args: &Value) -> Result<Value, OpError> {
    let id = text(args, "id")?.unwrap_or_default();
    match catalog.entry(&id)? {
        Some(entry) => json(entry),
        None => Err(OpError::not_found("No vehicle has that id.")),
    }
}

// ----- recalls -----

/// The answers about recalls: the shop file, the catalog, and the data
/// file itself, all read through the rows the caller has sent.
pub type RecallAnswers<'a> = Answers<'a, Replay, Replay>;

/// What the recall operation `op` gets when no shop file has been opened:
/// the API's own `dataset_unavailable`, naming the dataset asked for. What
/// the API refuses before it looks for the recalls is refused first here
/// too: a country that is not one, and a publisher it does not know.
pub fn no_recalls(op: &str, args: &Value) -> OpError {
    let missing =
        |dataset| OpError::from(AnswerError::Shop(ShopError::DatasetUnavailable(dataset)));
    let asked = |field: &str| text(args, field).map(Option::unwrap_or_default);
    if op == "recall" {
        return match asked("source") {
            Err(error) => error,
            Ok(source) => match RecallSource::parse(&source) {
                Some(source) => missing(source.dataset()),
                None => AnswerError::NotFound("No recall campaign has that number.").into(),
            },
        };
    }
    match asked("country") {
        Err(error) => error,
        Ok(country) => match answer::country(Some(&country)) {
            Err(error) => error.into(),
            Ok(_) => missing(answer::dataset_asked(Some(&country), None)),
        },
    }
}

/// `{ "id", "country"? }`: the recalls that may apply to a model year.
pub fn recalls_vehicle(answers: &RecallAnswers, args: &Value) -> Result<Value, OpError> {
    let id = text(args, "id")?.unwrap_or_default();
    let country = text(args, "country")?;
    json(answers.vehicle(&id, country.as_deref())?)
}

/// `{ "vin", "year"?, "country"? }`: the recalls that may apply to the
/// model year a VIN decodes to. The country is read and the recalls looked
/// for before the VIN is decoded, as the API does it, so that the same
/// error comes first.
pub fn recalls_vin(
    answers: &RecallAnswers,
    args: &Value,
    current_year: u16,
) -> Result<Value, OpError> {
    let country = text(args, "country")?;
    let source = answer::country(country.as_deref())?;
    answers
        .shop
        .require_recalls(source)
        .map_err(AnswerError::Shop)?;
    let decoded = decode(answers.catalog, answers.core, args, current_year)?;
    json(answers.vin(&decoded, country.as_deref())?)
}

/// `{ "source", "campaign" }`: one campaign with every vehicle it lists.
pub fn recall(answers: &RecallAnswers, args: &Value) -> Result<Value, OpError> {
    let source = text(args, "source")?.unwrap_or_default();
    let campaign = text(args, "campaign")?.unwrap_or_default();
    json(answers.campaign(&source, &campaign)?)
}

/// `{ "make"?, "model"?, "year"?, "component"?, "q"?, "country"?,
/// "limit"?, "cursor"? }`: campaigns, newest first.
pub fn recalls_search(answers: &RecallAnswers, args: &Value) -> Result<Value, OpError> {
    let query = Search {
        make: text(args, "make")?,
        model: text(args, "model")?,
        year: year(args)?,
        component: text(args, "component")?,
        q: text(args, "q")?,
        country: text(args, "country")?,
        limit: whole(args, "limit")?.map(|limit| usize::try_from(limit).unwrap_or(usize::MAX)),
        cursor: text(args, "cursor")?,
    };
    json(answers.search(&query)?)
}

/// `{ "since", "country"?, "make"?, "limit"?, "cursor"? }`: the campaigns
/// reported on or after a day, newest first, each with the vehicles it
/// lists.
pub fn recalls_recent(answers: &RecallAnswers, args: &Value) -> Result<Value, OpError> {
    let query = Recent {
        since: text(args, "since")?,
        country: text(args, "country")?,
        make: text(args, "make")?,
        limit: whole(args, "limit")?.map(|limit| usize::try_from(limit).unwrap_or(usize::MAX)),
        cursor: text(args, "cursor")?,
    };
    json(answers.recent(&query)?)
}

// ----- the overview -----

/// The answers that need no shop file: the catalog and the data file, and
/// the shop file when one has been opened. All are read through the rows
/// the caller has sent.
pub type OverviewAnswers<'a> = Overviews<'a, Replay, Replay>;

/// `{ "id" }`: everything held for one model year.
pub fn overview_vehicle(overviews: &OverviewAnswers, args: &Value) -> Result<Value, OpError> {
    let id = text(args, "id")?.unwrap_or_default();
    json(overviews.vehicle(&id)?)
}

/// `{ "vin", "year"? }`: everything held for the model year a VIN decodes
/// to. The VIN is decoded first, as the API does it, so that a VIN that
/// cannot be decoded fails as a decode does.
pub fn overview_vin(
    overviews: &OverviewAnswers,
    args: &Value,
    current_year: u16,
) -> Result<Value, OpError> {
    let decoded = decode(overviews.catalog, overviews.core, args, current_year)?;
    json(overviews.vin(&decoded)?)
}

/// `{ "make", "model" }`: every model year of one model, newest first.
pub fn model_years(overviews: &OverviewAnswers, args: &Value) -> Result<Value, OpError> {
    let make = text(args, "make")?.unwrap_or_default();
    let model = text(args, "model")?.unwrap_or_default();
    json(overviews.model_years(&make, &model)?)
}

// ----- crash test ratings -----

/// What `safety` gets when no shop file has been opened: the API's own
/// `dataset_unavailable`, naming the ratings. The API looks for the
/// ratings before it reads the vehicle id, so nothing else is checked
/// first.
pub fn no_ratings() -> OpError {
    AnswerError::Shop(ShopError::DatasetUnavailable(Dataset::Ratings)).into()
}

/// `{ "id" }`: NHTSA's crash test ratings for a model year, every one of
/// them.
pub fn safety(answers: &RecallAnswers, args: &Value) -> Result<Value, OpError> {
    let id = text(args, "id")?.unwrap_or_default();
    json(answers.safety(&id)?)
}

// ----- what owners report -----

/// What `complaints` gets when no shop file has been opened: the API's own
/// `dataset_unavailable`, naming the complaints. The API looks for them
/// before it reads the vehicle id, so nothing else is checked first.
pub fn no_complaints() -> OpError {
    AnswerError::Shop(ShopError::DatasetUnavailable(Dataset::Complaints)).into()
}

/// `{ "id" }`: what owners reported to NHTSA about a model year, as counts
/// by component, with the median mileage they gave.
pub fn complaints(answers: &RecallAnswers, args: &Value) -> Result<Value, OpError> {
    let id = text(args, "id")?.unwrap_or_default();
    json(answers.complaints(&id)?)
}

// ----- fuel, specifications, fluids and tire fitments -----

/// The four topics an operation name may start with.
pub const TOPICS: [&str; 4] = ["fuel", "specs", "fluids", "tires"];

/// The topic and whether the question is by VIN, for an operation such
/// as `fuel_vehicle` or `tires_vin`. `None` for any other operation.
pub fn topic_of(op: &str) -> Option<(&'static str, bool)> {
    let (topic, by) = op.split_once('_')?;
    let topic = TOPICS.into_iter().find(|known| *known == topic)?;
    match by {
        "vehicle" => Some((topic, false)),
        "vin" => Some((topic, true)),
        _ => None,
    }
}

/// What a question about `topic` gets when no shop file has been opened:
/// the API's own `dataset_unavailable`, naming what the hosted API names
/// on a server without one. A `country` of a fuel question that is not
/// `us` or `ca` is refused first, as the API refuses it.
pub fn no_facts(topic: &str, args: &Value) -> OpError {
    let country = match text(args, "country") {
        Err(error) => return error,
        Ok(country) => country,
    };
    if topic == "fuel"
        && let Err(error) = answer::facts::fuel_country(country.as_deref())
    {
        return error.into();
    }
    AnswerError::Shop(ShopError::DatasetUnavailable(answer::facts::dataset_for(
        topic,
        country.as_deref(),
    )))
    .into()
}

/// `{ "id", "country"? }` for a model year, or `{ "vin", "year"?,
/// "country"? }` for the model year a VIN decodes to: the fuel ratings,
/// specifications, fluids or tire fitments, as the API gives them. By
/// VIN, the data is looked for before the VIN is decoded, as in the API.
pub fn facts(
    answers: &RecallAnswers,
    topic: &str,
    by_vin: bool,
    args: &Value,
    current_year: Option<u16>,
) -> Result<Value, OpError> {
    let country = text(args, "country")?;
    let country = country.as_deref();
    if !by_vin {
        let id = text(args, "id")?.unwrap_or_default();
        return Ok(match topic {
            "fuel" => answers.vehicle_fuel(&id, country),
            "specs" => answers.vehicle_specs(&id),
            "fluids" => answers.vehicle_fluids(&id),
            _ => answers.vehicle_tires(&id),
        }?);
    }
    answers.require(topic, country)?;
    let current_year = current_year.ok_or_else(|| {
        OpError::validation("current_year", "current_year must be a whole number")
    })?;
    let decoded = decode(answers.catalog, answers.core, args, current_year)?;
    Ok(match topic {
        "fuel" => answers.vin_fuel(&decoded, country),
        "specs" => answers.vin_specs(&decoded),
        "fluids" => answers.vin_fluids(&decoded),
        _ => answers.vin_tires(&decoded),
    }?)
}

// ----- trouble codes -----

/// What a code operation gets when no shop file has been opened: the
/// API's own `dataset_unavailable`, naming the codes.
pub fn no_codes() -> OpError {
    AnswerError::Shop(ShopError::DatasetUnavailable(Dataset::Codes)).into()
}

/// `{ "code" }`: one generic trouble code as this project describes it.
pub fn code(answers: &RecallAnswers, args: &Value) -> Result<Value, OpError> {
    let code = text(args, "code")?.unwrap_or_default();
    json(answers.code(&code)?)
}

/// `{ "q"?, "system"?, "limit"?, "cursor"? }`: the described codes, by
/// code.
pub fn codes(answers: &RecallAnswers, args: &Value) -> Result<Value, OpError> {
    let search = CodeSearch {
        q: text(args, "q")?,
        system: text(args, "system")?,
        limit: whole(args, "limit")?.map(|limit| usize::try_from(limit).unwrap_or(usize::MAX)),
        cursor: text(args, "cursor")?,
    };
    json(answers.codes(&search)?)
}

/// `{ "id", "code" }`: a code on one model year, with the recalls about
/// the same components. Its bulletin parts are always `null`: bulletins
/// are on the hosted service only.
pub fn vehicle_code(answers: &RecallAnswers, args: &Value) -> Result<Value, OpError> {
    let id = text(args, "id")?.unwrap_or_default();
    let code = text(args, "code")?.unwrap_or_default();
    json(answers.vehicle_code(&id, &code)?)
}
