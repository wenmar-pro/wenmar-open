//! The operations, each as the hosted API answers it: the same checks, the
//! same limits, the same JSON and the same errors.

use serde::Serialize;
use serde_json::{Value, json};
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
