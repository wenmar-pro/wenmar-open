//! Reads the refrigerant chart's data file, `data/reference/refrigerant-pt.toml`,
//! checks it, and writes it out as Rust for the server to compile in.
//!
//! The server reads no reference file when it runs. The build script reads it
//! once, here, and a file that is not as its own first lines describe
//! stops the build with the place and the reason. So a mistake in the
//! file is found by whoever changed it, and never by a visitor.
//!
//! This file is compiled into the build script, and into the crate's unit
//! tests, which run the same checks on samples that are wrong.

use std::collections::{BTreeMap, BTreeSet};

use serde::Deserialize;
use shop_math::chart::parse_tenths;

/// Where the reference file is, from this crate's directory.
pub const PATH: &str = "../../data/reference/refrigerant-pt.toml";

/// The schema this code reads.
const SCHEMA: u32 = 1;

/// The name of the file the build script writes in `OUT_DIR`.
pub const OUT: &str = "refrigerant_pt.rs";

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct File {
    schema: u32,
    #[serde(default)]
    source: BTreeMap<String, Source>,
    refrigerant: Vec<Refrigerant>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct Source {
    title: String,
    url: String,
    #[serde(default)]
    archived: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct Refrigerant {
    key: String,
    name: String,
    as_of: String,
    confirmed: bool,
    missing: Vec<String>,
    #[serde(default)]
    notes: Vec<String>,
    #[serde(default)]
    to_check: Vec<String>,
    temperature_unit: String,
    pressure_unit: String,
    pressure_is: String,
    sources: Vec<String>,
    #[serde(default)]
    row: Vec<Row>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct Row {
    temperature: String,
    pressure: String,
}

fn digits(text: &str) -> bool {
    !text.is_empty() && text.bytes().all(|byte| byte.is_ascii_digit())
}

/// A day written `2026-10-03`. With `part`, a year or a month will do:
/// `2026` or `2026-10`.
fn is_date(text: &str, part: bool) -> bool {
    let mut pieces = text.split('-');
    let (year, month, day) = (pieces.next(), pieces.next(), pieces.next());
    if pieces.next().is_some() {
        return false;
    }
    let within = |piece: &str, most: u32| {
        piece.len() == 2
            && digits(piece)
            && piece
                .parse::<u32>()
                .is_ok_and(|number| (1..=most).contains(&number))
    };
    let year = year.is_some_and(|year| year.len() == 4 && digits(year));
    match (month, day) {
        (Some(month), Some(day)) => year && within(month, 12) && within(day, 31),
        (Some(month), None) => part && year && within(month, 12),
        (None, _) => part && year,
    }
}

/// Text a page can show: something, on one line.
fn is_sentence(text: &str) -> bool {
    !text.trim().is_empty() && !text.contains('\n') && text.trim() == text
}

/// A name that can be part of a field's name in an address.
fn is_key(text: &str) -> bool {
    !text.is_empty()
        && text.len() <= 48
        && text
            .bytes()
            .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'_')
}

fn is_address(text: &str) -> bool {
    text.strip_prefix("https://").is_some_and(|rest| {
        rest.contains('.')
            && rest
                .chars()
                .all(|c| !c.is_whitespace() && !c.is_control() && !"\"<>".contains(c))
    })
}

/// Checks one refrigerant's rows, units and sources, and notes its sources as used.
fn check_refrigerant<'a>(
    file: &File,
    refrigerant: &'a Refrigerant,
    used: &mut BTreeSet<&'a str>,
) -> Result<(), String> {
    if !is_sentence(&refrigerant.name) {
        return Err("has no name".to_owned());
    }
    if !is_date(&refrigerant.as_of, false) {
        return Err(format!(
            "as_of \"{}\" is not a day written like 2026-10-03",
            refrigerant.as_of
        ));
    }
    let lists = [
        ("missing", &refrigerant.missing),
        ("notes", &refrigerant.notes),
        ("to_check", &refrigerant.to_check),
    ];
    for (list, lines) in lists {
        if lines.iter().any(|line| !is_sentence(line)) {
            return Err(format!("a line of {list} is empty or is not one line"));
        }
    }
    if !["C", "F"].contains(&refrigerant.temperature_unit.as_str()) {
        return Err(format!(
            "temperature_unit \"{}\" is not \"C\" or \"F\"",
            refrigerant.temperature_unit
        ));
    }
    if !["kPa", "psi"].contains(&refrigerant.pressure_unit.as_str()) {
        return Err(format!(
            "pressure_unit \"{}\" is not \"kPa\" or \"psi\"",
            refrigerant.pressure_unit
        ));
    }
    if !["gauge", "absolute"].contains(&refrigerant.pressure_is.as_str()) {
        return Err(format!(
            "pressure_is \"{}\" is not \"gauge\" or \"absolute\"",
            refrigerant.pressure_is
        ));
    }
    let mut previous: Option<(i64, i64)> = None;
    for (index, row) in refrigerant.row.iter().enumerate() {
        let number = index + 1;
        let temperature = parse_tenths(&row.temperature).map_err(|_| {
            format!(
                "row {number}: the temperature \"{}\" is not a number with at most one decimal place",
                row.temperature
            )
        })?;
        let pressure = parse_tenths(&row.pressure).map_err(|_| {
            format!(
                "row {number}: the pressure \"{}\" is not a number with at most one decimal place",
                row.pressure
            )
        })?;
        if let Some((last_temperature, last_pressure)) = previous {
            if temperature <= last_temperature {
                return Err(format!(
                    "row {number}: the temperature does not rise from the row before"
                ));
            }
            if pressure < last_pressure {
                return Err(format!(
                    "row {number}: the pressure is below the row before"
                ));
            }
        }
        previous = Some((temperature, pressure));
    }
    if !refrigerant.row.is_empty() && refrigerant.sources.is_empty() {
        return Err("has rows and no source".to_owned());
    }
    for id in &refrigerant.sources {
        if !file.source.contains_key(id) {
            return Err(format!("names the source \"{id}\", which is not defined"));
        }
        used.insert(id.as_str());
    }
    match (
        refrigerant.confirmed,
        refrigerant.missing.is_empty(),
        refrigerant.row.len(),
    ) {
        (true, false, _) => Err("is confirmed and still says something is missing".to_owned()),
        (true, true, rows) if rows < 2 => {
            Err("is confirmed and has fewer than two rows".to_owned())
        }
        (false, true, _) => Err("is not confirmed and does not say what is missing".to_owned()),
        _ => Ok(()),
    }
}

fn check(file: &File) -> Result<(), String> {
    if file.schema != SCHEMA {
        return Err(format!(
            "schema is {}, and this build reads schema {SCHEMA}",
            file.schema
        ));
    }
    for (id, source) in &file.source {
        if !is_key(id) {
            return Err(format!(
                "the source \"{id}\": an id is small letters, digits and _"
            ));
        }
        if !is_sentence(&source.title) {
            return Err(format!("the source {id} has no title"));
        }
        if !is_address(&source.url) {
            return Err(format!(
                "the source {id}: \"{}\" is not an address that starts with https://",
                source.url
            ));
        }
        if let Some(archived) = &source.archived
            && !is_date(archived, true)
        {
            return Err(format!(
                "the source {id}: archived \"{archived}\" is not a year, a month or a day"
            ));
        }
    }
    if file.refrigerant.is_empty() {
        return Err("there is no refrigerant".to_owned());
    }
    let mut keys = BTreeSet::new();
    for refrigerant in &file.refrigerant {
        let key = &refrigerant.key;
        if !is_key(key) || !keys.insert(key.as_str()) {
            return Err(format!(
                "the key \"{key}\" is not small letters, digits and _, or is there twice"
            ));
        }
    }
    let mut used = BTreeSet::new();
    for refrigerant in &file.refrigerant {
        check_refrigerant(file, refrigerant, &mut used)
            .map_err(|why| format!("{}: {why}", refrigerant.key))?;
    }
    match file.source.keys().find(|id| !used.contains(id.as_str())) {
        Some(id) => Err(format!(
            "the source {id} is used by nothing: use it or take it out"
        )),
        None => Ok(()),
    }
}

/// Text as a Rust string.
fn text(text: &str) -> String {
    format!("{text:?}")
}

fn texts(lines: &[String]) -> String {
    let lines: Vec<String> = lines.iter().map(|line| text(line)).collect();
    format!("&[{}]", lines.join(", "))
}

fn sources(file: &File, ids: &[String]) -> String {
    let mut code = String::from("&[");
    for source in ids.iter().filter_map(|id| file.source.get(id)) {
        let archived = match &source.archived {
            Some(archived) => format!("Some({})", text(archived)),
            None => "None".to_owned(),
        };
        code.push_str(&format!(
            "Source {{ title: {}, url: {}, archived: {archived} }}, ",
            text(&source.title),
            text(&source.url)
        ));
    }
    code.push(']');
    code
}

fn write(file: &File) -> String {
    let mut code = String::from(
        "// Written by refrigerant_build.rs from data/reference/refrigerant-pt.toml. Change that file, not this one.\n",
    );
    code.push_str(&format!(
        "pub static REFRIGERANTS: [Refrigerant; {}] = [\n",
        file.refrigerant.len()
    ));
    for refrigerant in &file.refrigerant {
        let temperature = match refrigerant.temperature_unit.as_str() {
            "C" => "Degrees::Celsius",
            _ => "Degrees::Fahrenheit",
        };
        let pressure = match refrigerant.pressure_unit.as_str() {
            "kPa" => "Pressure::Kilopascals",
            _ => "Pressure::PoundsPerSquareInch",
        };
        let reading = match refrigerant.pressure_is.as_str() {
            "gauge" => "Reading::Gauge",
            _ => "Reading::Absolute",
        };
        code.push_str(&format!(
            "Refrigerant {{\n    key: {},\n    name: {},\n    as_of: {},\n    confirmed: {},\n    missing: {},\n    notes: {},\n    temperature: {temperature},\n    pressure: {pressure},\n    reading: {reading},\n    rows: &[",
            text(&refrigerant.key),
            text(&refrigerant.name),
            text(&refrigerant.as_of),
            refrigerant.confirmed,
            texts(&refrigerant.missing),
            texts(&refrigerant.notes),
        ));
        for row in &refrigerant.row {
            let temperature = parse_tenths(&row.temperature).unwrap_or(0);
            let pressure = parse_tenths(&row.pressure).unwrap_or(0);
            code.push_str(&format!("({temperature}, {pressure}), "));
        }
        code.push_str(&format!(
            "],\n    sources: {},\n}},\n",
            sources(file, &refrigerant.sources)
        ));
    }
    code.push_str("];\n");
    code
}

/// The reference file as Rust, or why it cannot be used.
pub fn generate(toml: &str) -> Result<String, String> {
    let file: File = basic_toml::from_str(toml).map_err(|error| error.to_string())?;
    check(&file)?;
    Ok(write(&file))
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE: &str = include_str!("tests/fixtures/refrigerant-pt.toml");

    #[test]
    fn the_file_is_read_and_is_what_was_compiled_in() {
        let code = generate(include_str!("../../data/reference/refrigerant-pt.toml")).unwrap();
        assert_eq!(
            code,
            include_str!(concat!(env!("OUT_DIR"), "/refrigerant_pt.rs"))
        );
        assert_eq!(OUT, "refrigerant_pt.rs");
        // The path the build script reads is the path of this test's file.
        assert!(PATH.ends_with("data/reference/refrigerant-pt.toml"));
        for part in ["../../data/reference", "refrigerant_build.rs"] {
            assert!(crate::build_id::PARTS.contains(&part), "{part}");
        }
    }

    #[test]
    fn a_file_is_written_out_as_the_data_the_server_compiles_in() {
        let code = generate(SAMPLE).unwrap();
        assert!(
            code.contains("pub static REFRIGERANTS: [Refrigerant; 2] = [\n"),
            "{code}"
        );
        for part in [
            "Refrigerant {\n    key: \"x000\",\n    name: \"X-000 (made up)\",\n    as_of: \"2026-01-31\",\n    confirmed: true,\n    missing: &[],\n    notes: &[\"A note the page shows.\"],\n    temperature: Degrees::Celsius,\n    pressure: Pressure::Kilopascals,\n    reading: Reading::Gauge,\n    rows: &[(-100, 0), (0, 1000), (100, 2000), ],\n    sources: &[Source { title: \"A made-up page: \\\"quoted\\\"\", url: \"https://example.org/pt\", archived: Some(\"2025-08\") }, ],\n},\n",
            "    key: \"x001\",\n    name: \"X-001 (made up)\",\n    as_of: \"2026-01-31\",\n    confirmed: false,\n    missing: &[\"No table has been read.\"],\n    notes: &[],\n    temperature: Degrees::Fahrenheit,\n    pressure: Pressure::PoundsPerSquareInch,\n    reading: Reading::Absolute,\n    rows: &[],\n    sources: &[],\n},\n];\n",
        ] {
            assert!(code.contains(part), "missing: {part}\n{code}");
        }
        assert!(!code.contains("A weak point."));
    }

    #[test]
    fn a_malformed_file_is_refused_with_the_place_and_the_reason() {
        for (from, to, why) in [
            (
                "schema = 1",
                "schema = 2",
                "schema is 2, and this build reads schema 1",
            ),
            (
                "key = \"x001\"",
                "key = \"x000\"",
                "the key \"x000\" is not small letters, digits and _, or is there twice",
            ),
            (
                "key = \"x000\"",
                "key = \"x000\"\ncolour = \"blue\"",
                "unknown field",
            ),
            (
                "as_of = \"2026-01-31\"\nconfirmed = true",
                "as_of = \"31 January\"\nconfirmed = true",
                "x000: as_of \"31 January\" is not a day written like 2026-10-03",
            ),
            (
                "temperature_unit = \"C\"",
                "temperature_unit = \"K\"",
                "x000: temperature_unit \"K\" is not \"C\" or \"F\"",
            ),
            (
                "pressure_unit = \"kPa\"",
                "pressure_unit = \"bar\"",
                "x000: pressure_unit \"bar\" is not \"kPa\" or \"psi\"",
            ),
            (
                "pressure_is = \"gauge\"",
                "pressure_is = \"sealed\"",
                "x000: pressure_is \"sealed\" is not \"gauge\" or \"absolute\"",
            ),
            (
                "temperature = \"0\"",
                "temperature = \"0.25\"",
                "x000: row 2: the temperature \"0.25\" is not a number with at most one decimal place",
            ),
            (
                "pressure = \"100\"",
                "pressure = \"about 100\"",
                "x000: row 2: the pressure \"about 100\" is not a number with at most one decimal place",
            ),
            (
                "temperature = \"0\"",
                "temperature = \"-10\"",
                "x000: row 2: the temperature does not rise from the row before",
            ),
            (
                "pressure = \"200\"",
                "pressure = \"99.9\"",
                "x000: row 3: the pressure is below the row before",
            ),
            (
                "sources = [\"fake\"]",
                "sources = [\"other\"]",
                "x000: names the source \"other\", which is not defined",
            ),
            (
                "sources = [\"fake\"]",
                "sources = []",
                "x000: has rows and no source",
            ),
            (
                "missing = []",
                "missing = [\"Something.\"]",
                "x000: is confirmed and still says something is missing",
            ),
            (
                "missing = [\"No table has been read.\"]",
                "missing = []",
                "x001: is not confirmed and does not say what is missing",
            ),
            (
                "url = \"https://example.org/pt\"",
                "url = \"http://example.org/pt\"",
                "the source fake: \"http://example.org/pt\" is not an address that starts with https://",
            ),
        ] {
            let toml = SAMPLE.replacen(from, to, 1);
            assert_ne!(toml, SAMPLE, "{from}");
            let error = generate(&toml).unwrap_err();
            assert!(error.contains(why), "{from} -> {to}: {error}");
        }
        // Two rows are the least a confirmed table holds, and a source nothing uses is taken out.
        let one_row = SAMPLE.replacen("[[refrigerant.row]]\ntemperature = \"0\"\npressure = \"100\"\n\n[[refrigerant.row]]\ntemperature = \"10\"\npressure = \"200\"\n", "", 1);
        assert!(
            generate(&one_row)
                .unwrap_err()
                .contains("x000: is confirmed and has fewer than two rows")
        );
        let unused = format!(
            "{SAMPLE}\n[source.spare]\ntitle = \"Spare\"\nurl = \"https://example.org/spare\"\n"
        );
        assert!(
            generate(&unused)
                .unwrap_err()
                .contains("the source spare is used by nothing: use it or take it out")
        );
    }

    #[test]
    fn a_date_is_a_day_and_an_archive_date_may_be_a_year_or_a_month() {
        for day in ["2026-10-03", "2002-04-01", "2024-12-31"] {
            assert!(is_date(day, false) && is_date(day, true), "{day}");
        }
        for part in ["2026", "2025-08"] {
            assert!(is_date(part, true) && !is_date(part, false), "{part}");
        }
        for wrong in [
            "",
            "2026-",
            "26-10-03",
            "2026-00-10",
            "2026-10-32",
            "2026-1-3",
            "2026-10-03-01",
            "2026/10/03",
            "abcd-ef-gh",
        ] {
            assert!(!is_date(wrong, true) && !is_date(wrong, false), "{wrong}");
        }
    }
}
