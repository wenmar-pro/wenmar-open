//! Reads the battery group size table, `data/reference/battery-groups.toml`,
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

/// Where the file is, from this crate's directory.
pub const PATH: &str = "../../data/reference/battery-groups.toml";

/// The schema this code reads.
const SCHEMA: u32 = 1;

/// The name of the file the build script writes in `OUT_DIR`.
pub const OUT: &str = "battery_groups.rs";

/// Ten-thousandths of a millimetre in an inch: 25.4 mm exactly.
const INCH: i64 = 254_000;

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
    to_check: Vec<String>,
    #[serde(default)]
    source: BTreeMap<String, Source>,
    #[serde(default)]
    group: Vec<Group>,
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
struct Group {
    name: String,
    length: String,
    width: String,
    height: String,
    terminals: String,
}

impl Group {
    fn dimensions(&self) -> [(&'static str, &str); 3] {
        [
            ("length", &self.length),
            ("width", &self.width),
            ("height", &self.height),
        ]
    }
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

/// A dimension as printed, in thousandths of its unit: digits, then
/// optionally a point and one to three more digits, above zero and under
/// ten thousand. `None` for anything else.
fn thousandths(text: &str) -> Option<i64> {
    let (whole, fraction) = text.split_once('.').unwrap_or((text, ""));
    if !digits(whole) || whole.len() > 4 || fraction.len() > 3 {
        return None;
    }
    if text.contains('.') && !digits(fraction) {
        return None;
    }
    let whole: i64 = whole.parse().ok()?;
    let fraction: i64 = if fraction.is_empty() {
        0
    } else {
        format!("{fraction:0<3}").parse().ok()?
    };
    let value = whole * 1000 + fraction;
    (value > 0).then_some(value)
}

/// A number of thousandths as text with `places` decimal places, rounded
/// half up: `(254_500, 0)` is `255`, `(10_004, 2)` is `10.00`.
fn rounded(thousandths: i64, places: u32) -> String {
    let step = 10_i64.pow(3 - places.min(3));
    let steps = (2 * thousandths + step) / (2 * step);
    if places == 0 {
        return steps.to_string();
    }
    let scale = 10_i64.pow(places);
    format!(
        "{}.{:0width$}",
        steps / scale,
        steps % scale,
        width = places as usize
    )
}

/// A dimension in millimetres to a whole number, and in inches to two
/// decimal places. The side the source printed is only rounded; the other
/// is worked out with whole numbers from 25.4 mm to the inch.
fn both(value: i64, unit: &str) -> (String, String) {
    if unit == "mm" {
        // Thousandths of a millimetre to thousandths of an inch.
        let inches = (2 * value * 10_000 + INCH) / (2 * INCH);
        (rounded(value, 0), rounded(inches, 2))
    } else {
        let millimetres = (2 * value * INCH + 10_000) / (2 * 10_000);
        (rounded(millimetres, 0), rounded(value, 2))
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
    if !is_sentence(&file.name) {
        return Err("has no name".to_owned());
    }
    if !is_date(&file.as_of, false) {
        return Err(format!(
            "as_of \"{}\" is not a day written like 2026-10-03",
            file.as_of
        ));
    }
    for (list, lines) in [
        ("missing", &file.missing),
        ("notes", &file.notes),
        ("to_check", &file.to_check),
    ] {
        if lines.iter().any(|line| !is_sentence(line)) {
            return Err(format!("a line of {list} is empty or is not one line"));
        }
    }
    if file.unit != "mm" && file.unit != "in" {
        return Err(format!("unit \"{}\" is not \"mm\" or \"in\"", file.unit));
    }
    let mut names = BTreeSet::new();
    for group in &file.group {
        if !is_sentence(&group.name) || !names.insert(group.name.as_str()) {
            return Err(format!(
                "the name of the group \"{}\" is empty or is there twice",
                group.name
            ));
        }
        for (field, value) in group.dimensions() {
            if thousandths(value).is_none() {
                return Err(format!(
                    "the group {}: {field} \"{value}\" is not a figure above zero with at most three decimal places",
                    group.name
                ));
            }
        }
        if !is_sentence(&group.terminals) {
            return Err(format!(
                "the group {}: terminals is empty or is not one line",
                group.name
            ));
        }
    }
    if !file.group.is_empty() && file.sources.is_empty() {
        return Err("has groups and no source".to_owned());
    }
    let mut used: BTreeSet<&str> = BTreeSet::new();
    for id in &file.sources {
        if !file.source.contains_key(id) {
            return Err(format!("names the source \"{id}\", which is not defined"));
        }
        used.insert(id);
    }
    match (file.confirmed, file.missing.is_empty()) {
        (true, false) => Err("is confirmed and still says something is missing".to_owned()),
        (false, true) => Err("is not confirmed and does not say what is missing".to_owned()),
        (true, true) if file.group.is_empty() => Err("is confirmed and has no group".to_owned()),
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

fn sources(file: &File) -> String {
    let mut code = String::from("&[");
    for source in file.sources.iter().filter_map(|id| file.source.get(id)) {
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
    let mut groups = String::from("&[");
    for group in &file.group {
        let mut sides = Vec::new();
        for (_, value) in group.dimensions() {
            let (mm, inches) = both(thousandths(value).unwrap_or(0), &file.unit);
            sides.push(format!(
                "Side {{ mm: {}, inches: {} }}",
                text(&mm),
                text(&inches)
            ));
        }
        groups.push_str(&format!(
            "Group {{ name: {}, length: {}, width: {}, height: {}, terminals: {} }}, ",
            text(&group.name),
            sides[0],
            sides[1],
            sides[2],
            text(&group.terminals)
        ));
    }
    groups.push(']');
    format!(
        "// Written by battery_groups_build.rs from data/reference/battery-groups.toml. Change that file, not this one.\n\
         pub static BATTERY_GROUPS: GroupTable = GroupTable {{\n\
         \x20   name: {},\n\
         \x20   as_of: {},\n\
         \x20   confirmed: {},\n\
         \x20   missing: {},\n\
         \x20   notes: {},\n\
         \x20   printed_in: {},\n\
         \x20   groups: {groups},\n\
         \x20   sources: {},\n\
         }};\n",
        text(&file.name),
        text(&file.as_of),
        file.confirmed,
        texts(&file.missing),
        texts(&file.notes),
        text(&file.unit),
        sources(file),
    )
}

/// The file as Rust, or why it cannot be used.
pub fn generate(toml: &str) -> Result<String, String> {
    let file: File = basic_toml::from_str(toml).map_err(|error| error.to_string())?;
    check(&file)?;
    Ok(write(&file))
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE: &str = include_str!("tests/fixtures/battery-groups.toml");

    #[test]
    fn the_file_is_read_and_is_what_was_compiled_in() {
        let code = generate(include_str!("../../data/reference/battery-groups.toml")).unwrap();
        assert_eq!(
            code,
            include_str!(concat!(env!("OUT_DIR"), "/battery_groups.rs"))
        );
        assert_eq!(OUT, "battery_groups.rs");
        assert!(PATH.ends_with("data/reference/battery-groups.toml"));
        for part in ["../../data/reference", "battery_groups_build.rs"] {
            assert!(crate::build_id::PARTS.contains(&part), "{part}");
        }
    }

    #[test]
    fn a_dimension_is_read_and_converted_with_whole_numbers() {
        assert_eq!(thousandths("254"), Some(254_000));
        assert_eq!(thousandths("100.5"), Some(100_500));
        assert_eq!(thousandths("0.125"), Some(125));
        for bad in [
            "", "0", "0.0", "9 1/16", "10.", ".5", "1.2345", "-3", "12345", "1e2",
        ] {
            assert_eq!(thousandths(bad), None, "{bad:?}");
        }
        assert_eq!(rounded(254_500, 0), "255");
        assert_eq!(rounded(254_499, 0), "254");
        assert_eq!(rounded(10_004, 2), "10.00");
        assert_eq!(rounded(10_005, 2), "10.01");
        // 254 mm is 10 inches, and 10 inches is 254 mm.
        assert_eq!(both(254_000, "mm"), ("254".to_owned(), "10.00".to_owned()));
        assert_eq!(both(10_000, "in"), ("254".to_owned(), "10.00".to_owned()));
        // 127 mm is 5 inches; 100.5 mm is 3.9566 inches and shows as 101 mm.
        assert_eq!(both(127_000, "mm"), ("127".to_owned(), "5.00".to_owned()));
        assert_eq!(both(100_500, "mm"), ("101".to_owned(), "3.96".to_owned()));
        // 9.875 inches is 250.825 mm.
        assert_eq!(both(9_875, "in"), ("251".to_owned(), "9.88".to_owned()));
    }

    #[test]
    fn a_file_is_written_out_as_the_data_the_server_compiles_in() {
        let code = generate(SAMPLE).unwrap();
        assert_eq!(
            code,
            "// Written by battery_groups_build.rs from data/reference/battery-groups.toml. Change that file, not this one.\npub static BATTERY_GROUPS: GroupTable = GroupTable {\n    name: \"Made-up battery groups\",\n    as_of: \"2026-01-31\",\n    confirmed: true,\n    missing: &[],\n    notes: &[\"A note the page shows.\"],\n    printed_in: \"mm\",\n    groups: &[Group { name: \"T1 (made up)\", length: Side { mm: \"254\", inches: \"10.00\" }, width: Side { mm: \"127\", inches: \"5.00\" }, height: Side { mm: \"101\", inches: \"3.96\" }, terminals: \"Made-up posts on top, the positive one on the left.\" }, Group { name: \"T2 (made up)\", length: Side { mm: \"999\", inches: \"39.33\" }, width: Side { mm: \"888\", inches: \"34.96\" }, height: Side { mm: \"777\", inches: \"30.59\" }, terminals: \"Made-up terminals on the side.\" }, ],\n    sources: &[Source { title: \"A made-up page\", url: \"https://example.org/groups\", archived: None }, ],\n};\n"
        );
        let inches = SAMPLE
            .replacen("unit = \"mm\"", "unit = \"in\"", 1)
            .replacen("length = \"254\"", "length = \"10\"", 1);
        let code = generate(&inches).unwrap();
        assert!(code.contains("printed_in: \"in\",\n"), "{code}");
        assert!(
            code.contains("length: Side { mm: \"254\", inches: \"10.00\" }"),
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
                "unit = \"mm\"",
                "unit = \"cm\"",
                "unit \"cm\" is not \"mm\" or \"in\"",
            ),
            (
                "unit = \"mm\"",
                "unit = \"mm\"\ncolour = \"red\"",
                "unknown field",
            ),
            (
                "as_of = \"2026-01-31\"",
                "as_of = \"January\"",
                "as_of \"January\" is not a day written like 2026-10-03",
            ),
            (
                "length = \"254\"",
                "length = \"9 1/16\"",
                "the group T1 (made up): length \"9 1/16\" is not a figure above zero with at most three decimal places",
            ),
            (
                "width = \"127\"",
                "width = \"0\"",
                "the group T1 (made up): width \"0\" is not a figure",
            ),
            (
                "height = \"777\"",
                "height = \"tall\"",
                "the group T2 (made up): height \"tall\" is not a figure",
            ),
            (
                "name = \"T2 (made up)\"",
                "name = \"T1 (made up)\"",
                "the name of the group \"T1 (made up)\" is empty or is there twice",
            ),
            (
                "terminals = \"Made-up terminals on the side.\"",
                "terminals = \"\"",
                "the group T2 (made up): terminals is empty or is not one line",
            ),
            (
                "terminals = \"Made-up terminals on the side.\"\n",
                "",
                "missing field",
            ),
            (
                "sources = [\"fake\"]",
                "sources = [\"other\"]",
                "names the source \"other\", which is not defined",
            ),
            (
                "sources = [\"fake\"]",
                "sources = []",
                "has groups and no source",
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
            (
                "url = \"https://example.org/groups\"",
                "url = \"http://example.org/groups\"",
                "is not an address that starts with https://",
            ),
        ] {
            let toml = SAMPLE.replacen(from, to, 1);
            assert_ne!(toml, SAMPLE, "{from}");
            let error = generate(&toml).unwrap_err();
            assert!(error.contains(why), "{from} -> {to}: {error}");
        }
        let at = SAMPLE.find("[[group]]").unwrap();
        let none = format!("{}sources = []\n", &SAMPLE[..at]);
        let none = none.replacen("sources = [\"fake\"]\n", "", 1).replacen(
            "[source.fake]\ntitle = \"A made-up page\"\nurl = \"https://example.org/groups\"\n",
            "",
            1,
        );
        assert!(
            generate(&none)
                .unwrap_err()
                .contains("is confirmed and has no group"),
            "{none}"
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
