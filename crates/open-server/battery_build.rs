//! Reads the battery table's data file, `data/reference/battery-soc.toml`,
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

/// Where the battery file is, from this crate's directory.
pub const PATH: &str = "../../data/reference/battery-soc.toml";

/// The schema this code reads.
const SCHEMA: u32 = 1;

/// The name of the file the build script writes in `OUT_DIR`.
pub const OUT: &str = "battery_soc.rs";

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct File {
    schema: u32,
    #[serde(default)]
    source: BTreeMap<String, Source>,
    battery: Vec<Battery>,
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
struct Battery {
    key: String,
    name: String,
    as_of: String,
    confirmed: bool,
    missing: Vec<String>,
    sources: Vec<String>,
    #[serde(default)]
    notes: Vec<String>,
    #[serde(default)]
    conditions: Vec<String>,
    #[serde(default)]
    to_check: Vec<String>,
    #[serde(default)]
    row: Vec<Row>,
    #[serde(default)]
    correction: Option<Correction>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct Row {
    volts: String,
    percent: u32,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct Correction {
    reference_c: String,
    add_millivolts_per_c_below: String,
    sources: Vec<String>,
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

fn check_battery<'a>(
    file: &File,
    battery: &'a Battery,
    used: &mut BTreeSet<&'a str>,
) -> Result<(), String> {
    if !is_sentence(&battery.name) {
        return Err("has no name".to_owned());
    }
    if !is_date(&battery.as_of, false) {
        return Err(format!(
            "as_of \"{}\" is not a day written like 2026-10-03",
            battery.as_of
        ));
    }
    let lists = [
        ("missing", &battery.missing),
        ("notes", &battery.notes),
        ("conditions", &battery.conditions),
        ("to_check", &battery.to_check),
    ];
    for (list, lines) in lists {
        if lines.iter().any(|line| !is_sentence(line)) {
            return Err(format!("a line of {list} is empty or is not one line"));
        }
    }
    let mut rows: Vec<(i64, u32)> = Vec::new();
    for (index, row) in battery.row.iter().enumerate() {
        let number = index + 1;
        let volts = shop_math::battery_soc::parse_volts(&row.volts).map_err(|_| {
            format!(
                "row {number}: \"{}\" is not a voltage written like 12.60",
                row.volts
            )
        })?;
        if row.percent > 100 {
            return Err(format!(
                "row {number}: the percent {} is over 100",
                row.percent
            ));
        }
        if let Some(&(last_volts, last_percent)) = rows.last() {
            if volts <= last_volts {
                return Err(format!(
                    "row {number}: the voltage does not rise from the row before"
                ));
            }
            if row.percent <= last_percent {
                return Err(format!(
                    "row {number}: the percent does not rise from the row before"
                ));
            }
        }
        rows.push((volts, row.percent));
    }
    if let Some(correction) = &battery.correction {
        if shop_math::chart::parse_tenths(&correction.reference_c).is_err() {
            return Err(format!(
                "correction: reference_c \"{}\" is not a number with at most one decimal place",
                correction.reference_c
            ));
        }
        if shop_math::battery_soc::parse_volts(&correction.add_millivolts_per_c_below).is_err() {
            return Err(format!(
                "correction: add_millivolts_per_c_below \"{}\" is not a number of millivolts with at most three decimal places",
                correction.add_millivolts_per_c_below
            ));
        }
        if correction.sources.is_empty() {
            return Err("correction has no source".to_owned());
        }
        for id in &correction.sources {
            if !file.source.contains_key(id) {
                return Err(format!(
                    "correction names the source \"{id}\", which is not defined"
                ));
            }
            used.insert(id);
        }
    }
    if battery.sources.is_empty() && !battery.row.is_empty() {
        return Err("has rows and no source".to_owned());
    }
    for id in &battery.sources {
        if !file.source.contains_key(id) {
            return Err(format!("names the source \"{id}\", which is not defined"));
        }
        used.insert(id);
    }
    match (battery.confirmed, battery.missing.is_empty()) {
        (true, false) => Err("is confirmed and still says something is missing".to_owned()),
        (false, true) => Err("is not confirmed and does not say what is missing".to_owned()),
        (true, true) if rows.len() < 2 => {
            Err("is confirmed and has fewer than two rows".to_owned())
        }
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
    if file.battery.is_empty() {
        return Err("there is no battery".to_owned());
    }
    let mut keys = BTreeSet::new();
    let mut used = BTreeSet::new();
    for battery in &file.battery {
        let key = &battery.key;
        if !is_key(key) || !keys.insert(key.as_str()) {
            return Err(format!(
                "the key \"{key}\" is not small letters, digits and _, or is there twice"
            ));
        }
        check_battery(file, battery, &mut used).map_err(|why| format!("{key}: {why}"))?;
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
        "// Written by battery_build.rs from data/reference/battery-soc.toml. Change that file, not this one.\n",
    );
    code.push_str(&format!(
        "pub static BATTERIES: [Battery; {}] = [\n",
        file.battery.len()
    ));
    for battery in &file.battery {
        let mut rows = String::from("&[");
        for row in &battery.row {
            let volts = shop_math::battery_soc::parse_volts(&row.volts).unwrap_or(0);
            rows.push_str(&format!("({volts}, {}), ", row.percent));
        }
        rows.push(']');
        let (correction, correction_sources) = match &battery.correction {
            Some(correction) => {
                let reference =
                    shop_math::chart::parse_tenths(&correction.reference_c).unwrap_or(0);
                let per_degree =
                    shop_math::battery_soc::parse_volts(&correction.add_millivolts_per_c_below)
                        .unwrap_or(0);
                (
                    format!(
                        "Some(Correction {{ reference_tenths_c: {reference}, microvolts_per_degree: {per_degree} }})"
                    ),
                    sources(file, &correction.sources),
                )
            }
            None => ("None".to_owned(), "&[]".to_owned()),
        };
        code.push_str(&format!(
            "Battery {{\n    key: {},\n    name: {},\n    as_of: {},\n    confirmed: {},\n    missing: {},\n    notes: {},\n    conditions: {},\n    rows: {rows},\n    correction: {correction},\n    correction_sources: {correction_sources},\n    sources: {},\n}},\n",
            text(&battery.key),
            text(&battery.name),
            text(&battery.as_of),
            battery.confirmed,
            texts(&battery.missing),
            texts(&battery.notes),
            texts(&battery.conditions),
            sources(file, &battery.sources),
        ));
    }
    code.push_str("];\n");
    code
}

/// The battery file as Rust, or why it cannot be used.
pub fn generate(toml: &str) -> Result<String, String> {
    let file: File = basic_toml::from_str(toml).map_err(|error| error.to_string())?;
    check(&file)?;
    Ok(write(&file))
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE: &str = include_str!("tests/fixtures/battery-soc.toml");

    #[test]
    fn the_file_is_read_and_is_what_was_compiled_in() {
        let code = generate(include_str!("../../data/reference/battery-soc.toml")).unwrap();
        assert_eq!(
            code,
            include_str!(concat!(env!("OUT_DIR"), "/battery_soc.rs"))
        );
        assert_eq!(OUT, "battery_soc.rs");
        assert!(PATH.ends_with("data/reference/battery-soc.toml"));
        for part in ["../../data/reference", "battery_build.rs"] {
            assert!(crate::build_id::PARTS.contains(&part), "{part}");
        }
    }

    #[test]
    fn a_file_is_written_out_as_the_data_the_server_compiles_in() {
        let code = generate(SAMPLE).unwrap();
        assert!(
            code.contains("pub static BATTERIES: [Battery; 2] = [\n"),
            "{code}"
        );
        for part in [
            "Battery {\n    key: \"made_up\",\n    name: \"Made-up battery\",\n    as_of: \"2026-01-31\",\n    confirmed: true,\n    missing: &[],\n    notes: &[],\n    conditions: &[\"A condition the page shows.\"],\n    rows: &[(10000, 0), (11000, 50), (12000, 100), ],\n    correction: Some(Correction { reference_tenths_c: 250, microvolts_per_degree: 10000 }),\n    correction_sources: &[Source { title: \"A made-up page\", url: \"https://example.org/battery\", archived: None }, ],\n    sources: &[Source { title: \"A made-up page\", url: \"https://example.org/battery\", archived: None }, ],\n},\n",
            "    key: \"not_read\",\n    name: \"Battery not read\",\n    as_of: \"2026-01-31\",\n    confirmed: false,\n    missing: &[\"No table has been read.\"],\n    notes: &[],\n    conditions: &[],\n    rows: &[],\n    correction: None,\n    correction_sources: &[],\n    sources: &[],\n},\n];\n",
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
                "key = \"not_read\"",
                "key = \"made_up\"",
                "the key \"made_up\" is not small letters, digits and _, or is there twice",
            ),
            (
                "key = \"made_up\"",
                "key = \"made_up\"\nvolts = \"12\"",
                "unknown field",
            ),
            (
                "volts = \"11.00\"",
                "volts = \"eleven\"",
                "made_up: row 2: \"eleven\" is not a voltage written like 12.60",
            ),
            (
                "volts = \"11.00\"",
                "volts = \"10.00\"",
                "made_up: row 2: the voltage does not rise from the row before",
            ),
            (
                "percent = 50",
                "percent = 0",
                "made_up: row 2: the percent does not rise from the row before",
            ),
            (
                "percent = 100",
                "percent = 101",
                "made_up: row 3: the percent 101 is over 100",
            ),
            (
                "reference_c = \"25\"",
                "reference_c = \"room\"",
                "made_up: correction: reference_c \"room\" is not a number with at most one decimal place",
            ),
            (
                "add_millivolts_per_c_below = \"10\"",
                "add_millivolts_per_c_below = \"-10\"",
                "made_up: correction: add_millivolts_per_c_below \"-10\" is not a number of millivolts with at most three decimal places",
            ),
            (
                "reference_c = \"25\"\nadd_millivolts_per_c_below = \"10\"\nsources = [\"fake\"]",
                "reference_c = \"25\"\nadd_millivolts_per_c_below = \"10\"\nsources = []",
                "made_up: correction has no source",
            ),
            (
                "to_check = [\"A weak point.\"]\nsources = [\"fake\"]",
                "to_check = [\"A weak point.\"]\nsources = [\"other\"]",
                "made_up: names the source \"other\", which is not defined",
            ),
            (
                "to_check = [\"A weak point.\"]\nsources = [\"fake\"]",
                "to_check = [\"A weak point.\"]\nsources = []",
                "made_up: has rows and no source",
            ),
            (
                "missing = []",
                "missing = [\"Something.\"]",
                "made_up: is confirmed and still says something is missing",
            ),
            (
                "missing = [\"No table has been read.\"]",
                "missing = []",
                "not_read: is not confirmed and does not say what is missing",
            ),
        ] {
            let toml = SAMPLE.replacen(from, to, 1);
            assert_ne!(toml, SAMPLE, "{from}");
            let error = generate(&toml).unwrap_err();
            assert!(error.contains(why), "{from} -> {to}: {error}");
        }
        let one_row = SAMPLE.replacen("[[battery.row]]\nvolts = \"11.00\"\npercent = 50\n\n[[battery.row]]\nvolts = \"12.00\"\npercent = 100\n", "", 1);
        assert!(
            generate(&one_row)
                .unwrap_err()
                .contains("made_up: is confirmed and has fewer than two rows")
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
}
