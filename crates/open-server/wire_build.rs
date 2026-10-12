//! Reads the wire table's data file, `data/reference/wire-resistance.toml`,
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

/// Where the wire file is, from this crate's directory.
pub const PATH: &str = "../../data/reference/wire-resistance.toml";

/// The schema this code reads.
const SCHEMA: u32 = 1;

/// The name of the file the build script writes in `OUT_DIR`.
pub const OUT: &str = "wire_resistance.rs";

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct File {
    schema: u32,
    name: String,
    as_of: String,
    confirmed: bool,
    missing: Vec<String>,
    unit: String,
    sources: Vec<String>,
    #[serde(default)]
    notes: Vec<String>,
    #[serde(default)]
    conditions: Vec<String>,
    #[serde(default)]
    to_check: Vec<String>,
    #[serde(default)]
    source: BTreeMap<String, Source>,
    #[serde(default)]
    gauge: Vec<Gauge>,
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
struct Gauge {
    name: String,
    resistance: String,
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
    if !is_sentence(&file.name) {
        return Err("has no name".to_owned());
    }
    if !is_date(&file.as_of, false) {
        return Err(format!(
            "as_of \"{}\" is not a day written like 2026-10-03",
            file.as_of
        ));
    }
    let lists = [
        ("missing", &file.missing),
        ("notes", &file.notes),
        ("conditions", &file.conditions),
        ("to_check", &file.to_check),
    ];
    for (list, lines) in lists {
        if lines.iter().any(|line| !is_sentence(line)) {
            return Err(format!("a line of {list} is empty or is not one line"));
        }
    }
    if file.unit != "ohm_per_km" && file.unit != "ohm_per_1000ft" {
        return Err(format!(
            "unit \"{}\" is not \"ohm_per_km\" or \"ohm_per_1000ft\"",
            file.unit
        ));
    }
    let mut names = BTreeSet::new();
    let mut used: BTreeSet<&str> = BTreeSet::new();
    let mut before: Option<i64> = None;
    for gauge in &file.gauge {
        if !is_sentence(&gauge.name) || !names.insert(gauge.name.as_str()) {
            return Err(format!(
                "the name of the gauge \"{}\" is empty or is there twice",
                gauge.name
            ));
        }
        let ohms = shop_math::voltage_drop::parse_resistance(&gauge.resistance).map_err(|_| {
            format!(
                "the gauge {}: \"{}\" is not a resistance above zero with at most four decimal places",
                gauge.name, gauge.resistance
            )
        })?;
        let nano_ohms = if file.unit == "ohm_per_km" {
            shop_math::voltage_drop::from_ohms_per_km(ohms)
        } else {
            shop_math::voltage_drop::from_ohms_per_1000_ft(ohms)
        };
        if before.is_some_and(|last| nano_ohms >= last) {
            return Err(format!(
                "the gauge {}: the resistance does not fall from the gauge before",
                gauge.name
            ));
        }
        before = Some(nano_ohms);
    }
    if !file.gauge.is_empty() && file.sources.is_empty() {
        return Err("has gauges and no source".to_owned());
    }
    for id in &file.sources {
        if !file.source.contains_key(id) {
            return Err(format!("names the source \"{id}\", which is not defined"));
        }
        used.insert(id);
    }
    match (file.confirmed, file.missing.is_empty()) {
        (true, false) => Err("is confirmed and still says something is missing".to_owned()),
        (false, true) => Err("is not confirmed and does not say what is missing".to_owned()),
        (true, true) if file.gauge.len() < 2 => {
            Err("is confirmed and has fewer than two gauges".to_owned())
        }
        _ => match file.source.keys().find(|id| !used.contains(id.as_str())) {
            Some(id) => Err(format!(
                "the source {id} is used by nothing: use it or take it out"
            )),
            None => Ok(()),
        },
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
    let unit = match file.unit.as_str() {
        "ohm_per_km" => "OhmsPerKilometre",
        _ => "OhmsPerThousandFeet",
    };
    let mut gauges = String::from("&[");
    for gauge in &file.gauge {
        let ohms = shop_math::voltage_drop::parse_resistance(&gauge.resistance).unwrap_or(0);
        let nano_ohms = if file.unit == "ohm_per_km" {
            shop_math::voltage_drop::from_ohms_per_km(ohms)
        } else {
            shop_math::voltage_drop::from_ohms_per_1000_ft(ohms)
        };
        gauges.push_str(&format!(
            "Gauge {{ name: {}, resistance: {}, nano_ohms_per_metre: {nano_ohms} }}, ",
            text(&gauge.name),
            text(&gauge.resistance)
        ));
    }
    gauges.push(']');
    format!(
        "// Written by wire_build.rs from data/reference/wire-resistance.toml. Change that file, not this one.\n\
         pub static WIRE: WireTable = WireTable {{\n\
         \x20   name: {},\n\
         \x20   as_of: {},\n\
         \x20   confirmed: {},\n\
         \x20   missing: {},\n\
         \x20   notes: {},\n\
         \x20   conditions: {},\n\
         \x20   unit: Unit::{unit},\n\
         \x20   gauges: {gauges},\n\
         \x20   sources: {},\n\
         }};\n",
        text(&file.name),
        text(&file.as_of),
        file.confirmed,
        texts(&file.missing),
        texts(&file.notes),
        texts(&file.conditions),
        sources(file, &file.sources),
    )
}

/// The wire file as Rust, or why it cannot be used.
pub fn generate(toml: &str) -> Result<String, String> {
    let file: File = basic_toml::from_str(toml).map_err(|error| error.to_string())?;
    check(&file)?;
    Ok(write(&file))
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE: &str = include_str!("tests/fixtures/wire-resistance.toml");

    #[test]
    fn the_file_is_read_and_is_what_was_compiled_in() {
        let code = generate(include_str!("../../data/reference/wire-resistance.toml")).unwrap();
        assert_eq!(
            code,
            include_str!(concat!(env!("OUT_DIR"), "/wire_resistance.rs"))
        );
        assert_eq!(OUT, "wire_resistance.rs");
        assert!(PATH.ends_with("data/reference/wire-resistance.toml"));
        for part in ["../../data/reference", "wire_build.rs"] {
            assert!(crate::build_id::PARTS.contains(&part), "{part}");
        }
    }

    #[test]
    fn a_file_is_written_out_as_the_data_the_server_compiles_in() {
        let code = generate(SAMPLE).unwrap();
        assert_eq!(
            code,
            "// Written by wire_build.rs from data/reference/wire-resistance.toml. Change that file, not this one.\npub static WIRE: WireTable = WireTable {\n    name: \"Made-up wire\",\n    as_of: \"2026-01-31\",\n    confirmed: true,\n    missing: &[],\n    notes: &[],\n    conditions: &[\"A condition the page shows.\"],\n    unit: Unit::OhmsPerKilometre,\n    gauges: &[Gauge { name: \"T1 (made up)\", resistance: \"10\", nano_ohms_per_metre: 10000000 }, Gauge { name: \"T2 (made up)\", resistance: \"5\", nano_ohms_per_metre: 5000000 }, Gauge { name: \"T3 (made up)\", resistance: \"2\", nano_ohms_per_metre: 2000000 }, ],\n    sources: &[Source { title: \"A made-up page\", url: \"https://example.org/wire\", archived: None }, ],\n};\n"
        );
        // 30.48 ohms per 1000 feet is 100 ohms per kilometre, and 2 ohms per 1000 feet is 6.56168 per kilometre.
        let feet = SAMPLE.replacen("ohm_per_km", "ohm_per_1000ft", 1).replacen(
            "resistance = \"10\"",
            "resistance = \"30.4800\"",
            1,
        );
        let code = generate(&feet).unwrap();
        assert!(
            code.contains("unit: Unit::OhmsPerThousandFeet,\n"),
            "{code}"
        );
        assert!(code.contains("Gauge { name: \"T1 (made up)\", resistance: \"30.4800\", nano_ohms_per_metre: 100000000 }"), "{code}");
        assert!(
            code.contains(
                "Gauge { name: \"T3 (made up)\", resistance: \"2\", nano_ohms_per_metre: 6561680 }"
            ),
            "{code}"
        );
    }

    #[test]
    fn a_malformed_file_is_refused_with_the_reason() {
        for (from, to, why) in [
            (
                "schema = 1",
                "schema = 2",
                "schema is 2, and this build reads schema 1",
            ),
            (
                "unit = \"ohm_per_km\"",
                "unit = \"ohm_per_mile\"",
                "unit \"ohm_per_mile\" is not \"ohm_per_km\" or \"ohm_per_1000ft\"",
            ),
            (
                "unit = \"ohm_per_km\"",
                "unit = \"ohm_per_km\"\ncolour = \"red\"",
                "unknown field",
            ),
            (
                "as_of = \"2026-01-31\"",
                "as_of = \"January\"",
                "as_of \"January\" is not a day written like 2026-10-03",
            ),
            (
                "resistance = \"10\"",
                "resistance = \"ten\"",
                "the gauge T1 (made up): \"ten\" is not a resistance above zero with at most four decimal places",
            ),
            (
                "resistance = \"5\"",
                "resistance = \"10\"",
                "the gauge T2 (made up): the resistance does not fall from the gauge before",
            ),
            (
                "name = \"T2 (made up)\"",
                "name = \"T1 (made up)\"",
                "the name of the gauge \"T1 (made up)\" is empty or is there twice",
            ),
            (
                "sources = [\"fake\"]",
                "sources = [\"other\"]",
                "names the source \"other\", which is not defined",
            ),
            (
                "sources = [\"fake\"]",
                "sources = []",
                "has gauges and no source",
            ),
            (
                "missing = []",
                "missing = [\"Something.\"]",
                "is confirmed and still says something is missing",
            ),
            (
                "confirmed = true",
                "confirmed = false",
                "is not confirmed and does not say what is missing",
            ),
        ] {
            let toml = SAMPLE.replacen(from, to, 1);
            assert_ne!(toml, SAMPLE, "{from}");
            let error = generate(&toml).unwrap_err();
            assert!(error.contains(why), "{from} -> {to}: {error}");
        }
        let one = SAMPLE.replacen("[[gauge]]\nname = \"T2 (made up)\"\nresistance = \"5\"\n\n[[gauge]]\nname = \"T3 (made up)\"\nresistance = \"2\"\n", "", 1);
        assert!(
            generate(&one)
                .unwrap_err()
                .contains("is confirmed and has fewer than two gauges")
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
