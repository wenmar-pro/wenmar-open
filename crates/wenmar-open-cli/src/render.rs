//! Answers as text, for a person at a terminal.
//!
//! The text is made from the same JSON that `--json` prints, so it is the
//! same whether the answer came from the data file or from the API. Values
//! come from manufacturers' reports and, online, from whatever server
//! `--api` names, so control characters in them never reach the terminal.

use serde_json::Value;

use crate::request::{Action, Request};

/// A value with every control character replaced, so that data cannot move
/// the cursor, change colours or ring the bell.
pub fn clean(text: &str) -> String {
    text.chars()
        .map(|character| {
            if character.is_control() {
                '\u{fffd}'
            } else {
                character
            }
        })
        .collect()
}

/// A JSON value as one piece of text: a string without its quotes, a
/// number or boolean as written. `None` for null, lists and objects.
fn scalar(value: &Value) -> Option<String> {
    match value {
        Value::String(text) => Some(clean(text)),
        Value::Number(number) => Some(number.to_string()),
        Value::Bool(flag) => Some(flag.to_string()),
        _ => None,
    }
}

fn field(value: &Value, key: &str) -> Option<String> {
    value.get(key).and_then(scalar)
}

/// `transmission_speeds` becomes `Transmission speeds`.
fn label(key: &str) -> String {
    let mut words = clean(key).replace('_', " ");
    if let Some(first) = words.get(..1) {
        let first = first.to_uppercase();
        words.replace_range(..1, &first);
    }
    words
}

/// Rows in columns, each as wide as its longest cell. The last column is
/// not padded.
fn table(rows: &[Vec<String>]) -> String {
    let columns = rows.iter().map(Vec::len).max().unwrap_or(0);
    let widths: Vec<usize> = (0..columns)
        .map(|column| {
            rows.iter()
                .filter_map(|row| row.get(column))
                .map(|cell| cell.chars().count())
                .max()
                .unwrap_or(0)
        })
        .collect();
    let mut text = String::new();
    for row in rows {
        let mut line = String::new();
        for (cell, width) in row.iter().zip(&widths) {
            line.push_str(cell);
            let padding = width.saturating_sub(cell.chars().count()) + 2;
            line.push_str(&" ".repeat(padding));
        }
        text.push_str(line.trim_end());
        text.push('\n');
    }
    text
}

/// A list of objects as a table with a heading row.
fn list(value: &Value, headings: &[&str], row: impl Fn(&Value) -> Vec<String>) -> String {
    let items = value.as_array().map(Vec::as_slice).unwrap_or_default();
    if items.is_empty() {
        return "Nothing found.\n".to_owned();
    }
    let mut rows = vec![
        headings
            .iter()
            .map(|heading| (*heading).to_owned())
            .collect(),
    ];
    rows.extend(items.iter().map(row));
    table(&rows)
}

/// The scalar fields of an object as `Label  value` rows, in the order of
/// `first`, then any others by name.
fn fields(value: &Value, first: &[&str], skip: &[&str]) -> Vec<Vec<String>> {
    let Some(object) = value.as_object() else {
        return Vec::new();
    };
    let mut rows = Vec::new();
    for key in first {
        if let Some(text) = field(value, key) {
            rows.push(vec![label(key), text]);
        }
    }
    for (key, item) in object {
        if first.contains(&key.as_str()) || skip.contains(&key.as_str()) {
            continue;
        }
        if let Some(text) = scalar(item) {
            rows.push(vec![label(key), text]);
        }
    }
    rows
}

/// The scalar fields of an object on one line: `2.0L, cylinders 4`.
fn inline(value: &Value, plain: &[&str]) -> Option<String> {
    let object = value.as_object()?;
    let mut parts = Vec::new();
    for key in plain {
        if let Some(text) = field(value, key) {
            parts.push(text);
        }
    }
    for (key, item) in object {
        if plain.contains(&key.as_str()) {
            continue;
        }
        if let Some(text) = scalar(item) {
            parts.push(format!("{} {text}", clean(key).replace('_', " ")));
        }
    }
    (!parts.is_empty()).then(|| parts.join(", "))
}

fn decode(value: &Value) -> String {
    let mut text = String::new();
    let mut headline = Vec::new();
    for key in ["year", "make", "model", "trim"] {
        if let Some(part) = field(value, key) {
            headline.push(part);
        }
    }
    text.push_str(&field(value, "vin").unwrap_or_default());
    text.push('\n');
    if !headline.is_empty() {
        text.push_str(&headline.join(" "));
        text.push('\n');
    }
    text.push('\n');

    let check = match value.get("valid").and_then(Value::as_bool) {
        Some(true) => "valid".to_owned(),
        _ => match field(&value["check_digit"], "expected") {
            Some(expected) => format!("wrong (position 9 should be {expected})"),
            None => "wrong".to_owned(),
        },
    };
    let mut rows = vec![vec!["Check digit".to_owned(), check]];
    rows.extend(fields(
        value,
        &[
            "series",
            "body",
            "doors",
            "drivetrain",
            "transmission",
            "transmission_speeds",
        ],
        &["vin", "valid", "year", "make", "model", "trim"],
    ));
    if let Some(engine) = inline(&value["engine"], &["label"]) {
        rows.push(vec!["Engine".to_owned(), engine]);
    }
    if let Some(maker) = inline(&value["manufacturer"], &["name", "country"]) {
        rows.push(vec!["Manufacturer".to_owned(), maker]);
    }
    if let Some(plant) = inline(&value["plant"], &["city", "state", "country"]) {
        rows.push(vec!["Plant".to_owned(), plant]);
    }
    if let Some(id) = field(&value["catalog"]["entry"], "id") {
        rows.push(vec!["Vehicle id".to_owned(), id]);
    }
    text.push_str(&table(&rows));

    let safety = fields(&value["safety"], &[], &[]);
    if !safety.is_empty() {
        text.push_str("\nSafety equipment\n");
        let indented: Vec<Vec<String>> = safety
            .into_iter()
            .map(|mut row| {
                if let Some(first) = row.first_mut() {
                    first.insert_str(0, "  ");
                }
                row
            })
            .collect();
        text.push_str(&table(&indented));
    }

    for warning in value["warnings"].as_array().into_iter().flatten() {
        if let Some(message) = field(warning, "message") {
            text.push_str(&format!("\nWarning: {message}\n"));
        }
        let suggestions: Vec<String> = warning["suggestions"]
            .as_array()
            .into_iter()
            .flatten()
            .filter_map(scalar)
            .collect();
        if !suggestions.is_empty() {
            text.push_str(&format!("Did you mean: {}\n", suggestions.join(", ")));
        }
    }
    text
}

fn entry(value: &Value) -> String {
    let mut text = String::new();
    text.push_str(&field(value, "summary").unwrap_or_default());
    text.push_str("\n\n");
    let mut rows = fields(
        value,
        &[
            "id",
            "year",
            "make",
            "model",
            "submodel",
            "engine",
            "transmission",
            "drive",
            "body",
        ],
        &["summary"],
    );
    let types: Vec<String> = value["vehicle_types"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(scalar)
        .collect();
    if !types.is_empty() {
        rows.push(vec!["Vehicle types".to_owned(), types.join(", ")]);
    }
    text.push_str(&table(&rows));
    text
}

fn cell(value: &Value, key: &str) -> String {
    field(value, key).unwrap_or_default()
}

/// One answer of a batch: a decode or an error object.
fn batch(value: &Value) -> String {
    let items = value.as_array().map(Vec::as_slice).unwrap_or_default();
    let parts: Vec<String> = items
        .iter()
        .map(|item| match field(&item["error"], "message") {
            Some(message) => format!("error: {message}\n"),
            None => decode(item),
        })
        .collect();
    parts.join("\n")
}

/// A size in megabytes, as `122.0 MB`.
fn megabytes(value: &Value) -> Option<String> {
    let bytes = value.as_u64()?;
    let tenths = bytes.saturating_mul(10) / (1024 * 1024);
    Some(format!("{}.{} MB", tenths / 10, tenths % 10))
}

/// What `data status` reports, as text.
pub fn status(value: &Value) -> String {
    let path = cell(value, "path");
    if value["installed"] != true {
        return format!(
            "There is no data file at {path}.\nAnswers come from the hosted API. Run `wenmar-open data pull` to download the data file and work without a connection.\n"
        );
    }
    if let Some(problem) = field(value, "problem") {
        return format!(
            "The data file at {path} cannot be used: {problem}.\nRun `wenmar-open data pull` to replace it.\n"
        );
    }
    let mut rows = vec![vec!["Data file".to_owned(), path]];
    rows.push(vec![
        "Version".to_owned(),
        format!(
            "{} (schema {})",
            cell(value, "data_version"),
            cell(value, "schema_version")
        ),
    ]);
    if let Some(built) = field(value, "built_at") {
        let from = field(value, "vpic_release")
            .map(|release| format!(" from {release}"))
            .unwrap_or_default();
        rows.push(vec!["Built".to_owned(), format!("{built}{from}")]);
    }
    if let Some(size) = megabytes(&value["bytes"]) {
        rows.push(vec!["Size".to_owned(), size]);
    }
    table(&rows)
}

/// What `data pull` did, as text.
pub fn pulled(value: &Value) -> String {
    let (version, path) = (cell(value, "data_version"), cell(value, "path"));
    if value["updated"] != true {
        return format!("Data {version} is already in place at {path}.\n");
    }
    let size = megabytes(&value["bytes"])
        .map(|size| format!(" ({size})"))
        .unwrap_or_default();
    let replaced = field(value, "previous")
        .map(|previous| format!(" It replaces {previous}."))
        .unwrap_or_default();
    format!("Data {version} is in place at {path}{size}.{replaced}\n")
}

/// What `setup` did, as text.
pub fn setup(value: &Value) -> String {
    let path = cell(value, "skill");
    let mut text = if value["skill_written"] == true {
        format!("Wrote the skill to {path}.\n")
    } else {
        format!("The skill at {path} is already current.\n")
    };
    if value["mcp_registered"] == true {
        text.push_str("Registered the MCP server.\n");
    } else {
        text.push_str(&format!(
            "To add the MCP server as well, run:\n  {}\n",
            cell(value, "mcp_command")
        ));
    }
    text
}

/// What `doctor` found, as text.
pub fn doctor(value: &Value) -> String {
    let data = &value["data"];
    let data_line = if data["usable"] == true {
        format!("{} at {}", cell(data, "data_version"), cell(data, "path"))
    } else if let Some(problem) = field(data, "problem") {
        format!("cannot be used: {problem}")
    } else {
        format!("none at {}", cell(data, "path"))
    };
    let api = &value["api"];
    let api_line = if api["checked"] != true {
        format!("{}, not checked", cell(api, "url"))
    } else if api["reachable"] == true {
        format!(
            "{}, reachable, data {}",
            cell(api, "url"),
            cell(api, "data_version")
        )
    } else {
        format!("not reachable. {}", cell(api, "problem"))
    };
    let skills: Vec<String> = value["skills"]
        .as_object()
        .into_iter()
        .flatten()
        .map(|(agent, skill)| {
            let state = match (skill["installed"] == true, skill["current"] == true) {
                (false, _) => "not installed",
                (true, true) => "installed",
                (true, false) => "installed, out of date",
            };
            format!("{}: {state}", clean(agent))
        })
        .collect();
    let rows = vec![
        vec!["Data file".to_owned(), data_line],
        vec!["API".to_owned(), api_line],
        vec![
            "Answers from".to_owned(),
            match cell(value, "answers_from").as_str() {
                "data file" => "the data file".to_owned(),
                "api" => "the API".to_owned(),
                _ => "nothing: neither can answer".to_owned(),
            },
        ],
        vec!["Skills".to_owned(), skills.join("; ")],
    ];
    format!("wenmar-open {}\n\n{}", cell(value, "version"), table(&rows))
}

/// The answer to a request, as text.
pub fn text(request: &Request, value: &Value) -> String {
    let action = match request {
        Request::VinDecode { .. } => return decode(value),
        Request::VinBatch { .. } => return batch(value),
        Request::Vehicles { action, .. } => *action,
    };
    match action {
        Action::Years => {
            let years: Vec<String> = value
                .as_array()
                .into_iter()
                .flatten()
                .filter_map(scalar)
                .collect();
            if years.is_empty() {
                "Nothing found.\n".to_owned()
            } else {
                years.join("\n") + "\n"
            }
        }
        Action::Makes => list(value, &["ID", "MAKE"], |make| {
            vec![cell(make, "id"), cell(make, "name")]
        }),
        Action::Models => list(value, &["ID", "MODEL", "YEARS"], |model| {
            let (from, to) = (cell(model, "year_from"), cell(model, "year_to"));
            let years = if from == to {
                from
            } else {
                format!("{from}-{to}")
            };
            vec![cell(model, "id"), cell(model, "name"), years]
        }),
        Action::Submodels => list(value, &["ID", "SUBMODEL", "KIND"], |submodel| {
            vec![
                cell(submodel, "id"),
                cell(submodel, "name"),
                cell(submodel, "kind"),
            ]
        }),
        Action::Engines => list(value, &["ID", "ENGINE", "VIN 8TH"], |engine| {
            vec![
                cell(engine, "id"),
                cell(engine, "label"),
                cell(engine, "vin8"),
            ]
        }),
        Action::Search => list(value, &["ID", "VEHICLE"], |found| {
            vec![cell(found, "id"), cell(found, "summary")]
        }),
        Action::Entry => entry(value),
    }
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;
    use crate::request::Lookup;

    fn vehicles(action: Action) -> Request {
        Request::Vehicles {
            action,
            lookup: Lookup::default(),
        }
    }

    fn decode_request() -> Request {
        Request::VinDecode {
            vin: String::new(),
            year: None,
        }
    }

    #[test]
    fn a_decode_reads_top_to_bottom() {
        let value = json!({
            "vin": "KM8K2CAB4PU001140",
            "valid": true,
            "check_digit": { "valid": true, "expected": "4", "actual": "4" },
            "year": 2023,
            "make": "Hyundai",
            "model": "Kona",
            "trim": "SE",
            "body": "Sport Utility Vehicle (SUV)/Multi-Purpose Vehicle (MPV)",
            "transmission": "Automatic",
            "engine": { "label": "2.0L", "model": "G4NH", "displacement_l": 2.0, "displacement_cc": 2000, "cylinders": 4, "fuel": "Gasoline" },
            "safety": { "abs": "Standard", "tpms": "Direct" },
            "manufacturer": { "wmi": "KM8", "name": "Hyundai Motor Co", "country": "South Korea", "vehicle_type": "Multipurpose Passenger Vehicle (MPV)" },
            "plant": { "code": "U", "city": "Ulsan" },
            "warnings": [],
            "catalog": { "entry": { "id": "2023_hyundai_kona_se_2-0l" }, "vehicle_id": "2023_hyundai_kona" }
        });
        assert_eq!(
            text(&decode_request(), &value),
            "\
KM8K2CAB4PU001140
2023 Hyundai Kona SE

Check digit   valid
Body          Sport Utility Vehicle (SUV)/Multi-Purpose Vehicle (MPV)
Transmission  Automatic
Engine        2.0L, cylinders 4, displacement cc 2000, displacement l 2.0, fuel Gasoline, model G4NH
Manufacturer  Hyundai Motor Co, South Korea, vehicle type Multipurpose Passenger Vehicle (MPV), wmi KM8
Plant         Ulsan, code U
Vehicle id    2023_hyundai_kona_se_2-0l

Safety equipment
  Abs   Standard
  Tpms  Direct
"
        );
    }

    #[test]
    fn a_wrong_check_digit_and_its_warning_are_said() {
        let value = json!({
            "vin": "KM8K2CAB0PU001140",
            "valid": false,
            "check_digit": { "valid": false, "expected": "4", "actual": "0" },
            "manufacturer": { "wmi": "KM8", "name": "Hyundai Motor Co" },
            "plant": { "code": "U" },
            "warnings": [{
                "code": "invalid_check_digit",
                "message": "Position 9 should be 4 for this VIN, but it is 0.",
                "suggestions": ["KM8K2CAB4PU001140"]
            }]
        });
        let text = text(&decode_request(), &value);
        assert!(
            text.contains("Check digit   wrong (position 9 should be 4)\n"),
            "{text}"
        );
        assert!(
            text.ends_with(
                "\nWarning: Position 9 should be 4 for this VIN, but it is 0.\nDid you mean: KM8K2CAB4PU001140\n"
            ),
            "{text}"
        );
    }

    #[test]
    fn a_field_added_to_the_api_later_is_still_shown() {
        let value = json!({
            "vin": "KM8K2CAB4PU001140",
            "valid": true,
            "towing_capacity_lb": 3500,
            "warnings": []
        });
        let text = text(&decode_request(), &value);
        assert!(text.contains("Towing capacity lb  3500\n"), "{text}");
    }

    #[test]
    fn lists_are_tables() {
        assert_eq!(
            text(&vehicles(Action::Years), &json!([2020, 2019])),
            "2020\n2019\n"
        );
        assert_eq!(
            text(
                &vehicles(Action::Makes),
                &json!([{ "id": "ford", "name": "Ford", "popular": true }, { "id": "mercedes-benz", "name": "Mercedes-Benz", "popular": true }])
            ),
            "ID             MAKE\nford           Ford\nmercedes-benz  Mercedes-Benz\n"
        );
        assert_eq!(
            text(
                &vehicles(Action::Models),
                &json!([{ "id": "civic", "name": "Civic", "year_from": 2018, "year_to": 2020 }, { "id": "cr-v", "name": "CR-V", "year_from": 2019, "year_to": 2019 }])
            ),
            "ID     MODEL  YEARS\ncivic  Civic  2018-2020\ncr-v   CR-V   2019\n"
        );
        assert_eq!(
            text(
                &vehicles(Action::Submodels),
                &json!([{ "id": "si", "name": "Si", "kind": "trim" }])
            ),
            "ID  SUBMODEL  KIND\nsi  Si        trim\n"
        );
        assert_eq!(
            text(
                &vehicles(Action::Engines),
                &json!([{ "id": "5-0l-v8", "label": "5.0L V8", "vin8": "5", "preset": false }, { "id": "2-0l", "label": "2.0L", "preset": true }])
            ),
            "ID       ENGINE   VIN 8TH\n5-0l-v8  5.0L V8  5\n2-0l     2.0L\n"
        );
        assert_eq!(
            text(
                &vehicles(Action::Search),
                &json!([{ "id": "2019_honda_civic_si", "summary": "2019 Honda Civic Si, Manual, FWD, Sedan" }])
            ),
            "ID                   VEHICLE\n2019_honda_civic_si  2019 Honda Civic Si, Manual, FWD, Sedan\n"
        );
    }

    #[test]
    fn an_empty_list_says_so() {
        for action in [
            Action::Years,
            Action::Makes,
            Action::Models,
            Action::Submodels,
            Action::Engines,
            Action::Search,
        ] {
            assert_eq!(text(&vehicles(action), &json!([])), "Nothing found.\n");
        }
    }

    #[test]
    fn an_entry_lists_what_is_known() {
        let value = json!({
            "id": "2019_honda_civic_si",
            "year": 2019,
            "make": "Honda",
            "model": "Civic",
            "submodel": "Si",
            "transmission": "Manual",
            "drive": "FWD",
            "body": "Sedan",
            "vehicle_types": ["Passenger Car"],
            "summary": "2019 Honda Civic Si, Manual, FWD, Sedan"
        });
        assert_eq!(
            text(&vehicles(Action::Entry), &value),
            "\
2019 Honda Civic Si, Manual, FWD, Sedan

Id             2019_honda_civic_si
Year           2019
Make           Honda
Model          Civic
Submodel       Si
Transmission   Manual
Drive          FWD
Body           Sedan
Vehicle types  Passenger Car
"
        );
    }

    #[test]
    fn control_characters_in_data_never_reach_the_terminal() {
        let hostile = "Kona\u{1b}[31m\u{7} Red\r\nrm -rf";
        let value = json!({
            "vin": hostile,
            "valid": true,
            "model": hostile,
            "engine": { "label": hostile },
            "safety": { hostile: hostile },
            "warnings": [{ "message": hostile, "suggestions": [hostile] }],
            hostile: hostile
        });
        let decoded = text(&decode_request(), &value);
        let listed = text(
            &vehicles(Action::Makes),
            &json!([{ "id": hostile, "name": hostile }]),
        );
        let found = text(
            &vehicles(Action::Entry),
            &json!({ "summary": hostile, "vehicle_types": [hostile] }),
        );
        for text in [decoded, listed, found] {
            assert!(
                !text
                    .chars()
                    .any(|character| character.is_control() && character != '\n'),
                "{text:?}"
            );
            assert!(text.contains("Kona\u{fffd}[31m\u{fffd} Red"), "{text:?}");
        }
    }

    #[test]
    fn the_data_file_is_described_in_a_few_lines() {
        let installed = json!({
            "installed": true,
            "usable": true,
            "path": "/data/wenmar-open.sqlite3",
            "bytes": 127_926_272,
            "data_version": "2026.09",
            "schema_version": "3",
            "reads_schema_version": "3",
            "vpic_release": "vPICList_lite_2026_09",
            "built_at": "2026-10-01 04:25:57"
        });
        assert_eq!(
            status(&installed),
            "\
Data file  /data/wenmar-open.sqlite3
Version    2026.09 (schema 3)
Built      2026-10-01 04:25:57 from vPICList_lite_2026_09
Size       122.0 MB
"
        );
        let missing = json!({ "installed": false, "usable": false, "path": "/data/wenmar-open.sqlite3", "reads_schema_version": "3" });
        assert_eq!(
            status(&missing),
            "There is no data file at /data/wenmar-open.sqlite3.\nAnswers come from the hosted API. Run `wenmar-open data pull` to download the data file and work without a connection.\n"
        );
        let old = json!({ "installed": true, "usable": false, "path": "/d/f", "problem": "it has schema version 2 and this build reads version 3" });
        assert_eq!(
            status(&old),
            "The data file at /d/f cannot be used: it has schema version 2 and this build reads version 3.\nRun `wenmar-open data pull` to replace it.\n"
        );
    }

    #[test]
    fn doctor_and_setup_read_as_a_few_lines() {
        let report = json!({
            "version": "0.1.0",
            "ok": true,
            "answers_from": "data file",
            "data": { "installed": true, "usable": true, "path": "/d/wenmar-open.sqlite3", "data_version": "2026.09" },
            "api": { "url": "https://open.wenmarpro.com", "checked": true, "reachable": false, "problem": "The API at https://open.wenmarpro.com could not be reached: connection refused." },
            "skills": { "claude": { "installed": true, "current": true }, "codex": { "installed": false } }
        });
        assert_eq!(
            doctor(&report),
            "\
wenmar-open 0.1.0

Data file     2026.09 at /d/wenmar-open.sqlite3
API           not reachable. The API at https://open.wenmarpro.com could not be reached: connection refused.
Answers from  the data file
Skills        claude: installed; codex: not installed
"
        );
        let done = json!({
            "agent": "claude",
            "skill": "/home/pat/.claude/skills/wenmar-open/SKILL.md",
            "skill_written": true,
            "mcp_command": "claude mcp add --scope user wenmar-open -- wenmar-open mcp",
            "mcp_registered": false
        });
        assert_eq!(
            setup(&done),
            "\
Wrote the skill to /home/pat/.claude/skills/wenmar-open/SKILL.md.
To add the MCP server as well, run:
  claude mcp add --scope user wenmar-open -- wenmar-open mcp
"
        );
    }

    #[test]
    fn a_pull_says_what_it_did() {
        assert_eq!(
            pulled(
                &json!({ "updated": true, "data_version": "2026.09", "previous": "2026.08", "path": "/d/f", "bytes": 127_926_272 })
            ),
            "Data 2026.09 is in place at /d/f (122.0 MB). It replaces 2026.08.\n"
        );
        assert_eq!(
            pulled(&json!({ "updated": false, "data_version": "2026.09", "path": "/d/f" })),
            "Data 2026.09 is already in place at /d/f.\n"
        );
    }

    #[test]
    fn json_of_another_shape_is_rendered_without_failing() {
        for value in [json!(null), json!(7), json!({}), json!([1])] {
            let _ = status(&value);
            let _ = pulled(&value);
            let _ = setup(&value);
            let _ = doctor(&value);
        }
        for value in [
            json!(null),
            json!(7),
            json!("text"),
            json!({}),
            json!([[1]]),
            json!([null]),
        ] {
            let _ = text(&decode_request(), &value);
            for action in Action::ALL {
                let _ = text(&vehicles(action), &value);
            }
        }
    }
}
