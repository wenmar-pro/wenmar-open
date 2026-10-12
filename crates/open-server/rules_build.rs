//! Reads the rules file, `data/rules/canada.toml`, checks it, and writes
//! it out as Rust for the server to compile in.
//!
//! The server reads no rules file when it runs. The build script reads it
//! once, here, and a file that is not as its own first lines describe
//! stops the build with the place and the reason. So a mistake in the
//! file is found by whoever changed it, and never by a visitor.
//!
//! This file is compiled into the build script, and into the crate's unit
//! tests, which run the same checks on samples that are wrong.

use std::collections::{BTreeMap, BTreeSet};

use serde::Deserialize;

/// Where the rules file is, from this crate's directory.
pub const PATH: &str = "../../data/rules/canada.toml";

/// The schema this code reads.
const SCHEMA: u32 = 1;

/// The name of the file the build script writes in `OUT_DIR`.
pub const OUT: &str = "canada_rules.rs";

/// The topics of a province's page, in the order it shows them. A province
/// is confirmed only when it has a rule for each.
pub const TOPICS: [&str; 9] = [
    "written_estimate",
    "authorisation",
    "estimate_overrun",
    "repair_warranty",
    "invoice_contents",
    "old_parts",
    "winter_tires",
    "inspection",
    "environmental_fees",
];

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct File {
    schema: u32,
    country: String,
    #[serde(default)]
    source: BTreeMap<String, Source>,
    province: Vec<Province>,
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
struct Province {
    code: String,
    name: String,
    as_of: String,
    confirmed: bool,
    missing: Vec<String>,
    #[serde(default)]
    notes: Vec<String>,
    #[serde(default)]
    to_check: Vec<String>,
    #[serde(default)]
    rule: Vec<Rule>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct Rule {
    topic: String,
    applies: bool,
    summary: Vec<String>,
    #[serde(default)]
    law: Option<String>,
    as_of: String,
    #[serde(default)]
    effective: Option<String>,
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

/// Checks one rule of a province, and notes the sources it names as used.
fn check_rule<'a>(file: &File, rule: &'a Rule, used: &mut BTreeSet<&'a str>) -> Result<(), String> {
    let topic = &rule.topic;
    if !TOPICS.contains(&topic.as_str()) {
        return Err(format!(
            "the rule \"{topic}\" is not a topic this build knows"
        ));
    }
    if rule.summary.is_empty() || rule.summary.iter().any(|line| !is_sentence(line)) {
        return Err(format!(
            "the rule {topic} has no summary, or a line of it is empty or is not one line"
        ));
    }
    if let Some(law) = &rule.law
        && !is_sentence(law)
    {
        return Err(format!("the rule {topic}: law is empty or is not one line"));
    }
    if rule.applies && rule.law.is_none() {
        return Err(format!("the rule {topic} applies and names no law"));
    }
    if !is_date(&rule.as_of, false) {
        return Err(format!(
            "the rule {topic}: as_of \"{}\" is not a day written like 2026-10-03",
            rule.as_of
        ));
    }
    if let Some(effective) = &rule.effective
        && !is_date(effective, false)
    {
        return Err(format!(
            "the rule {topic}: effective \"{effective}\" is not a day written like 2026-10-03"
        ));
    }
    if rule.sources.is_empty() {
        return Err(format!("the rule {topic} has no source"));
    }
    for id in &rule.sources {
        if !file.source.contains_key(id) {
            return Err(format!(
                "the rule {topic} names the source \"{id}\", which is not defined"
            ));
        }
        used.insert(id);
    }
    Ok(())
}

fn check_province<'a>(
    file: &File,
    province: &'a Province,
    used: &mut BTreeSet<&'a str>,
) -> Result<(), String> {
    if !is_sentence(&province.name) {
        return Err("has no name".to_owned());
    }
    if !is_date(&province.as_of, false) {
        return Err(format!(
            "as_of \"{}\" is not a day written like 2026-10-03",
            province.as_of
        ));
    }
    let lists = [
        ("missing", &province.missing),
        ("notes", &province.notes),
        ("to_check", &province.to_check),
    ];
    for (list, lines) in lists {
        if lines.iter().any(|line| !is_sentence(line)) {
            return Err(format!("a line of {list} is empty or is not one line"));
        }
    }
    let mut topics = BTreeSet::new();
    for rule in &province.rule {
        if !topics.insert(rule.topic.as_str()) {
            return Err(format!("the rule {} is there twice", rule.topic));
        }
        check_rule(file, rule, used)?;
    }
    match (province.confirmed, province.missing.is_empty()) {
        (true, false) => Err("is confirmed and still says something is missing".to_owned()),
        (true, true) => match TOPICS.iter().find(|topic| !topics.contains(*topic)) {
            Some(topic) => Err(format!("is confirmed and has no rule for {topic}")),
            None => Ok(()),
        },
        (false, true) => Err("is not confirmed and does not say what is missing".to_owned()),
        (false, false) => Ok(()),
    }
}

fn check(file: &File) -> Result<(), String> {
    if file.schema != SCHEMA {
        return Err(format!(
            "schema is {}, and this build reads schema {SCHEMA}",
            file.schema
        ));
    }
    if file.country != "CA" {
        return Err(format!("country is \"{}\", not \"CA\"", file.country));
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
    if file.province.is_empty() {
        return Err("there is no province".to_owned());
    }
    let mut codes = BTreeSet::new();
    let mut used = BTreeSet::new();
    for province in &file.province {
        let code = &province.code;
        if code.len() != 2 || !code.bytes().all(|byte| byte.is_ascii_uppercase()) {
            return Err(format!(
                "the province code \"{code}\" is not two capital letters"
            ));
        }
        if !codes.insert(code.as_str()) {
            return Err(format!("the province {code} is there twice"));
        }
        check_province(file, province, &mut used).map_err(|why| format!("{code}: {why}"))?;
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
        "// Written by rules_build.rs from data/rules/canada.toml. Change that file, not this one.\n",
    );
    code.push_str(&format!(
        "pub static PROVINCES: [Province; {}] = [\n",
        file.province.len()
    ));
    for province in &file.province {
        code.push_str(&format!(
            "Province {{\n    code: {},\n    name: {},\n    as_of: {},\n    confirmed: {},\n    missing: {},\n    notes: {},\n    rules: &[\n",
            text(&province.code),
            text(&province.name),
            text(&province.as_of),
            province.confirmed,
            texts(&province.missing),
            texts(&province.notes),
        ));
        for rule in &province.rule {
            let law = match &rule.law {
                Some(law) => format!("Some({})", text(law)),
                None => "None".to_owned(),
            };
            let effective = match &rule.effective {
                Some(effective) => format!("Some({})", text(effective)),
                None => "None".to_owned(),
            };
            code.push_str(&format!(
                "        Rule {{ topic: {}, applies: {}, summary: {}, law: {law}, as_of: {}, effective: {effective}, sources: {} }},\n",
                text(&rule.topic),
                rule.applies,
                texts(&rule.summary),
                text(&rule.as_of),
                sources(file, &rule.sources),
            ));
        }
        code.push_str("    ],\n},\n");
    }
    code.push_str("];\n");
    code
}

/// The rules file as Rust, or why it cannot be used.
pub fn generate(toml: &str) -> Result<String, String> {
    let file: File = basic_toml::from_str(toml).map_err(|error| error.to_string())?;
    check(&file)?;
    Ok(write(&file))
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE: &str = include_str!("tests/fixtures/canada-rules.toml");

    /// The sample with First Place confirmed: a rule for every topic.
    fn confirmed() -> String {
        let mut toml = SAMPLE.replacen(
            "confirmed = false\nmissing = [\"Seven topics have not been read.\"]",
            "confirmed = true\nmissing = []",
            1,
        );
        for topic in TOPICS
            .iter()
            .filter(|topic| !["written_estimate", "winter_tires"].contains(topic))
        {
            toml.push_str(&format!("\n[[province.rule]]\ntopic = \"{topic}\"\napplies = false\nsummary = [\"Made up: no rule.\"]\nas_of = \"2026-01-30\"\nsources = [\"fake_page\"]\n"));
        }
        toml
    }

    #[test]
    fn the_rules_file_is_read_and_is_what_was_compiled_in() {
        let code = generate(include_str!("../../data/rules/canada.toml")).unwrap();
        assert_eq!(
            code,
            include_str!(concat!(env!("OUT_DIR"), "/canada_rules.rs"))
        );
        assert!(PATH.ends_with("data/rules/canada.toml"));
        assert_eq!(OUT, "canada_rules.rs");
        for part in ["../../data/rules", "rules_build.rs"] {
            assert!(crate::build_id::PARTS.contains(&part), "{part}");
        }
    }

    #[test]
    fn a_file_is_written_out_as_the_data_the_server_compiles_in() {
        let code = generate(SAMPLE).unwrap();
        assert!(code.starts_with("// Written by rules_build.rs from data/rules/canada.toml. Change that file, not this one.\npub static PROVINCES: [Province; 2] = [\n"), "{code}");
        for part in [
            "Province {\n    code: \"XB\",\n    name: \"Second Place\",\n    as_of: \"2026-01-31\",\n    confirmed: false,\n    missing: &[\"Nothing has been read for Second Place yet.\"],\n    notes: &[],\n    rules: &[\n    ],\n},\n",
            "    code: \"XA\",\n    name: \"First Place\",\n    as_of: \"2026-01-31\",\n    confirmed: false,\n    missing: &[\"Seven topics have not been read.\"],\n    notes: &[\"A note the page shows.\"],\n    rules: &[\n",
            "        Rule { topic: \"written_estimate\", applies: true, summary: &[\"A made-up rule about estimates.\", \"Its second sentence.\"], law: Some(\"Made-up Act, s. 1\"), as_of: \"2026-01-30\", effective: Some(\"2020-02-29\"), sources: &[Source { title: \"A made-up act: \\\"quoted\\\"\", url: \"https://example.org/act\", archived: None }, Source { title: \"A made-up regulator's page\", url: \"https://example.org/page\", archived: Some(\"2025-08\") }, ] },\n",
            "        Rule { topic: \"winter_tires\", applies: false, summary: &[\"First Place sets no made-up rule on winter tires.\"], law: None, as_of: \"2026-01-30\", effective: None, sources: &[Source { title: \"A made-up regulator's page\"",
            "    ],\n},\n];\n",
        ] {
            assert!(code.contains(part), "missing: {part}\n{code}");
        }
        assert!(!code.contains("A weak point."));
        // With a rule for every topic a place may be confirmed.
        let code = generate(&confirmed()).unwrap();
        assert!(code.contains("name: \"First Place\",\n    as_of: \"2026-01-31\",\n    confirmed: true,\n    missing: &[],\n"), "{code}");
        assert_eq!(code.matches("Rule { topic: ").count(), 9);
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
                "country = \"CA\"",
                "country = \"US\"",
                "country is \"US\", not \"CA\"",
            ),
            (
                "code = \"XB\"",
                "code = \"xb\"",
                "the province code \"xb\" is not two capital letters",
            ),
            (
                "code = \"XB\"",
                "code = \"XA\"",
                "the province XA is there twice",
            ),
            (
                "code = \"XA\"",
                "code = \"XA\"\nrate = \"5\"",
                "unknown field",
            ),
            (
                "as_of = \"2026-01-31\"",
                "as_of = \"31 January\"",
                "XB: as_of \"31 January\" is not a day written like 2026-10-03",
            ),
            (
                "missing = [\"Nothing has been read for Second Place yet.\"]",
                "missing = []",
                "XB: is not confirmed and does not say what is missing",
            ),
            (
                "topic = \"written_estimate\"",
                "topic = \"parking\"",
                "XA: the rule \"parking\" is not a topic this build knows",
            ),
            (
                "topic = \"winter_tires\"",
                "topic = \"written_estimate\"",
                "XA: the rule written_estimate is there twice",
            ),
            (
                "summary = [\"First Place sets no made-up rule on winter tires.\"]",
                "summary = []",
                "XA: the rule winter_tires has no summary, or a line of it is empty or is not one line",
            ),
            (
                "law = \"Made-up Act, s. 1\"\n",
                "",
                "XA: the rule written_estimate applies and names no law",
            ),
            (
                "as_of = \"2026-01-30\"\neffective",
                "as_of = \"last week\"\neffective",
                "XA: the rule written_estimate: as_of \"last week\" is not a day written like 2026-10-03",
            ),
            (
                "effective = \"2020-02-29\"",
                "effective = \"2020\"",
                "XA: the rule written_estimate: effective \"2020\" is not a day written like 2026-10-03",
            ),
            (
                "sources = [\"fake_act\", \"fake_page\"]",
                "sources = []",
                "XA: the rule written_estimate has no source",
            ),
            (
                "sources = [\"fake_page\"]",
                "sources = [\"other\"]",
                "XA: the rule winter_tires names the source \"other\", which is not defined",
            ),
            (
                "sources = [\"fake_act\", \"fake_page\"]",
                "sources = [\"fake_page\"]",
                "the source fake_act is used by nothing: use it or take it out",
            ),
            (
                "url = \"https://example.org/act\"",
                "url = \"example.org/act\"",
                "the source fake_act: \"example.org/act\" is not an address that starts with https://",
            ),
        ] {
            let toml = SAMPLE.replacen(from, to, 1);
            assert_ne!(toml, SAMPLE, "{from}");
            let error = generate(&toml).unwrap_err();
            assert!(error.contains(why), "{from} -> {to}: {error}");
        }
        // Confirmed means every topic has a rule, and nothing is still missing.
        let short = SAMPLE.replacen(
            "confirmed = false\nmissing = [\"Seven topics have not been read.\"]",
            "confirmed = true\nmissing = []",
            1,
        );
        assert!(
            generate(&short)
                .unwrap_err()
                .contains("XA: is confirmed and has no rule for authorisation")
        );
        let still = confirmed().replacen(
            "confirmed = true\nmissing = []",
            "confirmed = true\nmissing = [\"Something.\"]",
            1,
        );
        assert!(
            generate(&still)
                .unwrap_err()
                .contains("XA: is confirmed and still says something is missing")
        );
    }
}
