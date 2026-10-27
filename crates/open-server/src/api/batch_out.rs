//! What a batch of decodes can be narrowed to and written as: only the
//! fields asked for, and CSV for a spreadsheet.

use serde_json::{Map, Value, json};

use crate::error::ApiError;

/// Most fields one request may name.
pub const MOST_FIELDS: usize = 60;

/// The columns of a CSV answer when no `fields` are named: what a service
/// counter or a fleet list reads first.
pub const DEFAULT_COLUMNS: &[&str] = &[
    "vin",
    "valid",
    "year",
    "make",
    "model",
    "series",
    "trim",
    "body_style",
    "drive",
    "transmission",
    "engine.label",
    "engine.displacement_l",
    "engine.cylinders",
    "engine.fuel",
    "engine.hp",
    "generation.name",
    "safety.tpms",
    "plant.country",
    "catalog.vehicle_id",
];

/// The fields named in `fields`: dotted paths such as `engine.hp` or
/// `attributes.EngineHP.value`. `None` when nothing is named.
pub fn parse_fields(fields: Option<&str>) -> Result<Option<Vec<String>>, ApiError> {
    let Some(fields) = fields.map(str::trim).filter(|fields| !fields.is_empty()) else {
        return Ok(None);
    };
    let paths: Vec<String> = fields
        .split(',')
        .map(str::trim)
        .filter(|path| !path.is_empty())
        .map(str::to_owned)
        .collect();
    if paths.len() > MOST_FIELDS {
        return Err(ApiError::Validation {
            message: format!("fields names at most {MOST_FIELDS} fields"),
            details: json!({ "field": "fields", "max": MOST_FIELDS, "received": paths.len() }),
        });
    }
    let well_formed = |path: &str| {
        path.split('.').all(|part| {
            !part.is_empty()
                && part.len() <= 64
                && part
                    .bytes()
                    .all(|byte| byte.is_ascii_alphanumeric() || byte == b'_')
        })
    };
    if let Some(bad) = paths.iter().find(|path| !well_formed(path)) {
        return Err(ApiError::Validation {
            message: format!(
                "{bad:?} is not a field: name fields such as year, engine.hp or attributes.EngineHP.value, separated by commas"
            ),
            details: json!({ "field": "fields", "value": bad }),
        });
    }
    Ok(Some(paths))
}

fn at<'v>(value: &'v Value, path: &str) -> Option<&'v Value> {
    path.split('.')
        .try_fold(value, |found, part| found.as_object()?.get(part))
}

/// A decode with only the fields named, in the same nesting. `vin` is
/// always kept, so each item still says which VIN it is. An error item is
/// kept whole.
pub fn project(item: Value, fields: &[String]) -> Value {
    if item.get("error").is_some() {
        return item;
    }
    let mut kept = Value::Object(Map::new());
    for path in std::iter::once("vin").chain(fields.iter().map(String::as_str)) {
        let Some(found) = at(&item, path) else {
            continue;
        };
        let mut place = &mut kept;
        let parts: Vec<&str> = path.split('.').collect();
        for (index, part) in parts.iter().enumerate() {
            let Some(object) = place.as_object_mut() else {
                break;
            };
            if index + 1 == parts.len() {
                object.insert((*part).to_owned(), found.clone());
                break;
            }
            place = object
                .entry((*part).to_owned())
                .or_insert_with(|| Value::Object(Map::new()));
        }
    }
    kept
}

/// One cell: a value as text. A list is joined with `; `, an object is
/// written as JSON.
fn cell(value: Option<&Value>) -> String {
    match value {
        None | Some(Value::Null) => String::new(),
        Some(Value::String(text)) => text.clone(),
        Some(Value::Bool(flag)) => flag.to_string(),
        Some(Value::Number(number)) => number.to_string(),
        Some(Value::Array(items)) => items
            .iter()
            .map(|item| cell(Some(item)))
            .collect::<Vec<_>>()
            .join("; "),
        Some(object) => object.to_string(),
    }
}

/// A cell as RFC 4180 writes it. A cell a spreadsheet would read as a
/// formula is given a leading `'`, so a value from the data never runs.
fn quoted(text: &str) -> String {
    let text = if text.starts_with(['=', '+', '-', '@', '\t', '\r']) {
        format!("'{text}")
    } else {
        text.to_owned()
    };
    if text.contains([',', '"', '\n', '\r']) {
        format!("\"{}\"", text.replace('"', "\"\""))
    } else {
        text
    }
}

/// The batch as CSV: a header of the columns, then one row per VIN asked,
/// in order, with `error` and `error_message` last. An error row has the
/// text that was sent as its `vin`.
pub fn csv(items: &[Value], inputs: &[String], fields: Option<&[String]>) -> String {
    let mut columns: Vec<&str> = vec!["vin"];
    match fields {
        Some(fields) => columns.extend(
            fields
                .iter()
                .map(String::as_str)
                .filter(|field| *field != "vin"),
        ),
        None => columns.extend(DEFAULT_COLUMNS.iter().skip(1)),
    }
    let mut text = String::new();
    let header: Vec<String> = columns
        .iter()
        .chain(["error", "error_message"].iter())
        .map(|column| quoted(column))
        .collect();
    text.push_str(&header.join(","));
    text.push_str("\r\n");
    for (index, item) in items.iter().enumerate() {
        let mut row: Vec<String> = Vec::with_capacity(columns.len() + 2);
        if let Some(error) = item.get("error") {
            row.push(quoted(inputs.get(index).map_or("", String::as_str)));
            row.extend(columns.iter().skip(1).map(|_| String::new()));
            row.push(quoted(&cell(error.get("code"))));
            row.push(quoted(&cell(error.get("message"))));
        } else {
            row.extend(columns.iter().map(|column| quoted(&cell(at(item, column)))));
            row.push(String::new());
            row.push(String::new());
        }
        text.push_str(&row.join(","));
        text.push_str("\r\n");
    }
    text
}

#[cfg(test)]
mod tests {
    use super::*;

    fn decode() -> Value {
        json!({
            "vin": "KM8K2CAB4PU001140",
            "year": 2023,
            "make": "Hyundai",
            "engine": { "label": "2.0L", "hp": 147.0 },
            "notes": ["one", "two"],
            "attributes": { "EngineHP": { "name": "Engine Brake (hp) From", "value": "147" } }
        })
    }

    #[test]
    fn only_the_fields_named_are_kept_with_the_vin() {
        let fields = parse_fields(Some(
            "year, engine.hp,attributes.EngineHP.value,nothing.here",
        ))
        .unwrap()
        .unwrap();
        assert_eq!(
            project(decode(), &fields),
            json!({
                "vin": "KM8K2CAB4PU001140",
                "year": 2023,
                "engine": { "hp": 147.0 },
                "attributes": { "EngineHP": { "value": "147" } }
            })
        );
        let error = json!({ "error": { "code": "invalid_vin" } });
        assert_eq!(project(error.clone(), &fields), error);
    }

    #[test]
    fn fields_that_are_not_paths_are_refused() {
        assert!(parse_fields(None).unwrap().is_none());
        assert!(parse_fields(Some(" ")).unwrap().is_none());
        for bad in ["engine..hp", "year;drop", "a b", "engine.hp=1"] {
            assert!(parse_fields(Some(bad)).is_err(), "{bad}");
        }
        let many = vec!["year"; MOST_FIELDS + 1].join(",");
        assert!(parse_fields(Some(&many)).is_err());
    }

    #[test]
    fn csv_has_a_row_per_vin_in_order_with_errors() {
        let items = vec![
            decode(),
            json!({ "error": { "code": "invalid_vin", "message": "a VIN has 17 characters, this has 4" } }),
        ];
        let inputs = vec!["KM8K2CAB4PU001140".to_owned(), "nope".to_owned()];
        let fields = vec![
            "year".to_owned(),
            "engine.hp".to_owned(),
            "notes".to_owned(),
        ];
        assert_eq!(
            csv(&items, &inputs, Some(&fields)),
            "vin,year,engine.hp,notes,error,error_message\r\n\
             KM8K2CAB4PU001140,2023,147.0,one; two,,\r\n\
             nope,,,,invalid_vin,\"a VIN has 17 characters, this has 4\"\r\n"
        );
        let default = csv(&items[..1], &inputs[..1], None);
        assert!(
            default.starts_with("vin,valid,year,make,model,"),
            "{default}"
        );
    }

    #[test]
    fn a_cell_never_becomes_a_formula_or_breaks_the_row() {
        assert_eq!(quoted("=HYPERLINK(\"x\")"), "\"'=HYPERLINK(\"\"x\"\")\"");
        assert_eq!(quoted("@SUM(A1)"), "'@SUM(A1)");
        assert_eq!(quoted("two\nlines"), "\"two\nlines\"");
        assert_eq!(quoted("plain"), "plain");
    }
}
