//! Reads the rates file, `data/rates/canada.toml`, checks it, and writes
//! it out as Rust for the server to compile in.
//!
//! The server reads no rates file when it runs. The build script reads it
//! once, here, and a file that is not as its own first lines describe
//! stops the build with the place and the reason. So a mistake in the
//! file is found by whoever changed it, and never by a visitor.
//!
//! This file is compiled into the build script, and into the crate's unit
//! tests, which run the same checks on samples that are wrong.

use std::collections::{BTreeMap, BTreeSet};

use serde::Deserialize;
use shop_math::{Money, Percent};

/// Where the rates file is, from this crate's directory.
pub const PATH: &str = "../../data/rates/canada.toml";

/// The schema this code reads.
const SCHEMA: u32 = 1;

/// The name of the file the build script writes in `OUT_DIR`.
pub const OUT: &str = "canada_rates.rs";

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct File {
    schema: u32,
    currency: String,
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
    tax: Vec<Tax>,
    #[serde(default)]
    tire_fee: Vec<TireFee>,
    #[serde(default)]
    no_tire_fee: Option<NoTireFee>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct Tax {
    name: String,
    rate: String,
    on_labour: bool,
    on_parts: bool,
    on_supplies: bool,
    on_tire_fee: bool,
    on_other_taxes: bool,
    sources: Vec<String>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct TireFee {
    class: String,
    label: String,
    amount: String,
    #[serde(default)]
    effective: Option<String>,
    sources: Vec<String>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct NoTireFee {
    statement: String,
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

/// Checks the ids of the sources of one tax or fee, and notes them as used.
fn check_sources<'a>(
    file: &File,
    ids: &'a [String],
    used: &mut BTreeSet<&'a str>,
) -> Result<(), String> {
    if ids.is_empty() {
        return Err("has no source".to_owned());
    }
    for id in ids {
        if !file.source.contains_key(id) {
            return Err(format!("names the source \"{id}\", which is not defined"));
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
    if province.tax.is_empty() {
        return Err("has no tax".to_owned());
    }
    let mut names = BTreeSet::new();
    for tax in &province.tax {
        let name = &tax.name;
        if !is_sentence(name) || !names.insert(name.as_str()) {
            return Err(format!(
                "the name of the tax \"{name}\" is empty or is there twice"
            ));
        }
        match Percent::parse_between(&tax.rate, Percent::from_thousandths(1), Percent::HUNDRED) {
            Ok(_) => {}
            Err(error) => {
                return Err(format!(
                    "the tax {name}: the rate \"{}\" cannot be used. {error}",
                    tax.rate
                ));
            }
        }
        if !(tax.on_labour || tax.on_parts || tax.on_supplies || tax.on_tire_fee) {
            return Err(format!(
                "the tax {name} is charged on nothing: every on_ switch is false"
            ));
        }
        if tax.on_other_taxes {
            return Err(format!(
                "the tax {name}: on_other_taxes is true, and the calculator has no arithmetic for a tax on a tax"
            ));
        }
        check_sources(file, &tax.sources, used).map_err(|why| format!("the tax {name} {why}"))?;
    }
    if names.contains("HST") && province.tax.len() > 1 {
        return Err("has a tax named HST and another tax: HST is the only tax there".to_owned());
    }
    let mut classes = BTreeSet::new();
    for fee in &province.tire_fee {
        let class = &fee.class;
        if !is_key(class) || !classes.insert(class.as_str()) {
            return Err(format!(
                "the tire class \"{class}\" is not small letters, digits and _, or is there twice"
            ));
        }
        if !is_sentence(&fee.label) {
            return Err(format!("the tire class {class} has no label"));
        }
        match Money::parse(&fee.amount) {
            Ok(amount) if amount > Money::ZERO && !fee.amount.contains('$') => {}
            _ => {
                return Err(format!(
                    "the tire class {class}: the amount \"{}\" is not an amount above zero written like 6.50",
                    fee.amount
                ));
            }
        }
        if let Some(effective) = &fee.effective
            && !is_date(effective, false)
        {
            return Err(format!(
                "the tire class {class}: effective \"{effective}\" is not a day written like 2026-10-03"
            ));
        }
        check_sources(file, &fee.sources, used)
            .map_err(|why| format!("the tire class {class} {why}"))?;
    }
    if let Some(none) = &province.no_tire_fee {
        if !is_sentence(&none.statement) {
            return Err("no_tire_fee has no statement".to_owned());
        }
        if !province.tire_fee.is_empty() {
            return Err("has both tire fees and no_tire_fee".to_owned());
        }
        check_sources(file, &none.sources, used).map_err(|why| format!("no_tire_fee {why}"))?;
    }
    let settled = !province.tire_fee.is_empty() || province.no_tire_fee.is_some();
    match (province.confirmed, province.missing.is_empty(), settled) {
        (true, false, _) => Err("is confirmed and still says something is missing".to_owned()),
        (true, true, false) => Err(
            "is confirmed and has neither a tire fee nor no_tire_fee: say which it is".to_owned(),
        ),
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
    if file.currency != "CAD" {
        return Err(format!("currency is \"{}\", not \"CAD\"", file.currency));
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
        "// Written by rates_build.rs from data/rates/canada.toml. Change that file, not this one.\n",
    );
    code.push_str(&format!(
        "pub const CURRENCY: &str = {};\n",
        text(&file.currency)
    ));
    code.push_str(&format!(
        "pub static PROVINCES: [Province; {}] = [\n",
        file.province.len()
    ));
    for province in &file.province {
        code.push_str(&format!(
            "Province {{\n    code: {},\n    name: {},\n    as_of: {},\n    confirmed: {},\n    missing: {},\n    notes: {},\n",
            text(&province.code),
            text(&province.name),
            text(&province.as_of),
            province.confirmed,
            texts(&province.missing),
            texts(&province.notes),
        ));
        code.push_str("    taxes: &[\n");
        for tax in &province.tax {
            let rate = Percent::parse(&tax.rate).map_or(0, Percent::thousandths);
            code.push_str(&format!(
                "        TaxRule {{ name: {}, tax: Tax {{ rate: Percent::from_thousandths({rate}), on_labour: {}, on_parts: {}, on_supplies: {}, on_tire_fee: {} }}, sources: {} }},\n",
                text(&tax.name),
                tax.on_labour,
                tax.on_parts,
                tax.on_supplies,
                tax.on_tire_fee,
                sources(file, &tax.sources),
            ));
        }
        code.push_str("    ],\n    tire_classes: &[\n");
        for fee in &province.tire_fee {
            let cents = Money::parse(&fee.amount).map_or(0, Money::cents);
            let effective = match &fee.effective {
                Some(effective) => format!("Some({})", text(effective)),
                None => "None".to_owned(),
            };
            code.push_str(&format!(
                "        TireClass {{ key: {}, label: {}, fee: Money::from_cents({cents}), effective: {effective}, sources: {} }},\n",
                text(&fee.class),
                text(&fee.label),
                sources(file, &fee.sources),
            ));
        }
        code.push_str("    ],\n");
        match &province.no_tire_fee {
            Some(none) => code.push_str(&format!(
                "    no_tire_fee: Some(NoTireFee {{ statement: {}, sources: {} }}),\n",
                text(&none.statement),
                sources(file, &none.sources),
            )),
            None => code.push_str("    no_tire_fee: None,\n"),
        }
        code.push_str("},\n");
    }
    code.push_str("];\n");
    code
}

/// The rates file as Rust, or why it cannot be used.
pub fn generate(toml: &str) -> Result<String, String> {
    let file: File = basic_toml::from_str(toml).map_err(|error| error.to_string())?;
    check(&file)?;
    Ok(write(&file))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A file of one made-up source and two made-up places, as the schema
    /// asks for it. None of it is a real rate.
    const SAMPLE: &str = r#"
schema = 1
currency = "CAD"

[source.one]
title = "A page: \"quoted\""
url = "https://example.org/one"

[source.two]
title = "An older page"
url = "https://example.org/two"
archived = "2025-08"

[[province]]
code = "XA"
name = "First Place"
as_of = "2026-01-31"
confirmed = true
missing = []
notes = ["A note the page shows."]
to_check = ["A weak point."]

[[province.tax]]
name = "TAX"
rate = "1.5"
on_labour = true
on_parts = true
on_supplies = false
on_tire_fee = false
on_other_taxes = false
sources = ["one", "two"]

[[province.tire_fee]]
class = "small_1"
label = "small"
amount = "2.25"
effective = "2020-02-29"
sources = ["two"]

[[province]]
code = "XB"
name = "Second Place"
as_of = "2026-01-31"
confirmed = false
missing = ["Whether a fee exists."]

[[province.tax]]
name = "TAX"
rate = "2"
on_labour = true
on_parts = true
on_supplies = true
on_tire_fee = true
on_other_taxes = false
sources = ["one"]
"#;

    #[test]
    fn the_rates_file_is_read_and_is_what_was_compiled_in() {
        let file = include_str!("../../data/rates/canada.toml");
        let code = generate(file).unwrap();
        assert_eq!(
            code,
            include_str!(concat!(env!("OUT_DIR"), "/canada_rates.rs"))
        );
        assert!(code.contains("pub const CURRENCY: &str = \"CAD\";"));
        assert_eq!(OUT, "canada_rates.rs");
        // The path the build script reads is the path of this test's file.
        assert!(PATH.ends_with("data/rates/canada.toml"));
        // A change to the file, or to this reader, is a new build id: a
        // browser or a CDN that kept a page asks again.
        for part in ["../../data/rates", "rates_build.rs"] {
            assert!(crate::build_id::PARTS.contains(&part), "{part}");
        }
    }

    #[test]
    fn a_file_is_written_out_as_the_data_the_server_compiles_in() {
        let code = generate(SAMPLE).unwrap();
        assert!(
            code.contains("pub static PROVINCES: [Province; 2] = [\n"),
            "{code}"
        );
        for part in [
            "code: \"XA\",\n    name: \"First Place\",\n    as_of: \"2026-01-31\",\n    confirmed: true,\n    missing: &[],\n    notes: &[\"A note the page shows.\"],\n",
            // A percent is whole thousandths and an amount is whole cents.
            "TaxRule { name: \"TAX\", tax: Tax { rate: Percent::from_thousandths(1500), on_labour: true, on_parts: true, on_supplies: false, on_tire_fee: false }, sources: &[Source { title: \"A page: \\\"quoted\\\"\", url: \"https://example.org/one\", archived: None }, Source { title: \"An older page\", url: \"https://example.org/two\", archived: Some(\"2025-08\") }, ] },\n",
            "TireClass { key: \"small_1\", label: \"small\", fee: Money::from_cents(225), effective: Some(\"2020-02-29\"), sources: &[Source { title: \"An older page\"",
            "confirmed: false,\n    missing: &[\"Whether a fee exists.\"],\n    notes: &[],\n",
            "    tire_classes: &[\n    ],\n    no_tire_fee: None,\n},\n];\n",
        ] {
            assert!(code.contains(part), "missing: {part}\n{code}");
        }
        // What is only for whoever checks the file is not compiled in.
        assert!(!code.contains("A weak point."));
    }

    #[test]
    fn a_place_with_no_fee_says_so_with_a_source() {
        let toml = SAMPLE.replace(
            "[[province.tire_fee]]\nclass = \"small_1\"\nlabel = \"small\"\namount = \"2.25\"\neffective = \"2020-02-29\"\nsources = [\"two\"]",
            "[province.no_tire_fee]\nstatement = \"First Place sets no fee.\"\nsources = [\"two\"]",
        );
        assert_ne!(toml, SAMPLE);
        let code = generate(&toml).unwrap();
        assert!(code.contains(
            "no_tire_fee: Some(NoTireFee { statement: \"First Place sets no fee.\", sources: &[Source { title: \"An older page\""
        ), "{code}");
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
                "currency = \"CAD\"",
                "currency = \"USD\"",
                "currency is \"USD\"",
            ),
            // A key the schema does not have, and one it must have.
            (
                "code = \"XA\"",
                "code = \"XA\"\nrate = \"5\"",
                "unknown field `rate`",
            ),
            ("confirmed = true\n", "", "missing field `confirmed`"),
            (
                "on_other_taxes = false\nsources = [\"one\", \"two\"]",
                "sources = [\"one\", \"two\"]",
                "missing field `on_other_taxes`",
            ),
            ("missing = []\n", "", "missing field `missing`"),
            // Not TOML at all.
            (
                "[[province]]\ncode = \"XB\"",
                "[[province\ncode = \"XB\"",
                "expected a right bracket, found a newline at line 40",
            ),
            // A source.
            (
                "url = \"https://example.org/one\"",
                "url = \"http://example.org/one\"",
                "the source one: \"http://example.org/one\" is not an address that starts with https://",
            ),
            (
                "url = \"https://example.org/two\"\n",
                "",
                "missing field `url`",
            ),
            (
                "archived = \"2025-08\"",
                "archived = \"last August\"",
                "the source two: archived \"last August\" is not a year, a month or a day",
            ),
            (
                "sources = [\"two\"]",
                "sources = [\"twoo\"]",
                "XA: the tire class small_1 names the source \"twoo\", which is not defined",
            ),
            (
                "sources = [\"two\"]",
                "sources = []",
                "XA: the tire class small_1 has no source",
            ),
            (
                "sources = [\"one\", \"two\"]",
                "sources = []",
                "XA: the tax TAX has no source",
            ),
            (
                "sources = [\"one\"]",
                "sources = [\"three\"]",
                "XB: the tax TAX names the source \"three\", which is not defined",
            ),
            // A province.
            (
                "code = \"XB\"",
                "code = \"XA\"",
                "the province XA is there twice",
            ),
            (
                "code = \"XB\"",
                "code = \"xb\"",
                "the province code \"xb\" is not two capital letters",
            ),
            (
                "as_of = \"2026-01-31\"\nconfirmed = true",
                "as_of = \"31 January 2026\"\nconfirmed = true",
                "XA: as_of \"31 January 2026\" is not a day written like 2026-10-03",
            ),
            (
                "as_of = \"2026-01-31\"\nconfirmed = true",
                "as_of = \"2026-13-01\"\nconfirmed = true",
                "XA: as_of \"2026-13-01\"",
            ),
            (
                "missing = []",
                "missing = [\"The fee.\"]",
                "XA: is confirmed and still says something is missing",
            ),
            (
                "missing = [\"Whether a fee exists.\"]",
                "missing = []",
                "XB: is not confirmed and does not say what is missing",
            ),
            (
                "confirmed = false\nmissing = [\"Whether a fee exists.\"]",
                "confirmed = true\nmissing = []",
                "XB: is confirmed and has neither a tire fee nor no_tire_fee: say which it is",
            ),
            (
                "notes = [\"A note the page shows.\"]",
                "notes = [\" \"]",
                "XA: a line of notes is empty or is not one line",
            ),
            // A tax.
            (
                "rate = \"1.5\"",
                "rate = \"one and a half\"",
                "XA: the tax TAX: the rate \"one and a half\" cannot be used. This is not a number.",
            ),
            (
                "rate = \"1.5\"",
                "rate = \"0\"",
                "XA: the tax TAX: the rate \"0\" cannot be used. This must be from 0.001% to 100%.",
            ),
            (
                "rate = \"1.5\"",
                "rate = \"150\"",
                "XA: the tax TAX: the rate \"150\" cannot be used.",
            ),
            (
                "rate = \"1.5\"",
                "rate = \"1.5555\"",
                "Use at most 3 decimal places.",
            ),
            ("rate = \"1.5\"", "rate = 1.5", "invalid type"),
            (
                "on_other_taxes = false\nsources = [\"one\", \"two\"]",
                "on_other_taxes = true\nsources = [\"one\", \"two\"]",
                "XA: the tax TAX: on_other_taxes is true, and the calculator has no arithmetic for a tax on a tax",
            ),
            (
                "on_labour = true\non_parts = true\non_supplies = false",
                "on_labour = \"yes\"\non_parts = true\non_supplies = false",
                "invalid type",
            ),
            (
                "on_labour = true\non_parts = true\non_supplies = false\non_tire_fee = false",
                "on_labour = false\non_parts = false\non_supplies = false\non_tire_fee = false",
                "XA: the tax TAX is charged on nothing",
            ),
            (
                "[[province.tire_fee]]\nclass = \"small_1\"",
                "[[province.tax]]\nname = \"HST\"\nrate = \"5\"\non_labour = true\non_parts = true\non_supplies = true\non_tire_fee = true\non_other_taxes = false\nsources = [\"one\"]\n\n[[province.tire_fee]]\nclass = \"small_1\"",
                "XA: has a tax named HST and another tax",
            ),
            // A tire fee.
            (
                "class = \"small_1\"",
                "class = \"Small tires\"",
                "XA: the tire class \"Small tires\" is not small letters, digits and _, or is there twice",
            ),
            (
                "amount = \"2.25\"",
                "amount = \"$2.25\"",
                "XA: the tire class small_1: the amount \"$2.25\" is not an amount above zero written like 6.50",
            ),
            (
                "amount = \"2.25\"",
                "amount = \"0.00\"",
                "is not an amount above zero",
            ),
            (
                "amount = \"2.25\"",
                "amount = \"2.255\"",
                "is not an amount above zero",
            ),
            (
                "effective = \"2020-02-29\"",
                "effective = \"2020\"",
                "XA: the tire class small_1: effective \"2020\" is not a day written like 2026-10-03",
            ),
            (
                "label = \"small\"",
                "label = \"\"",
                "XA: the tire class small_1 has no label",
            ),
        ] {
            let toml = SAMPLE.replacen(from, to, 1);
            assert_ne!(toml, SAMPLE, "{from}");
            let error = generate(&toml).unwrap_err();
            assert!(error.contains(why), "{from} -> {to}: {error}");
        }
        // A source nothing uses, a fee beside no_tire_fee, and no province
        // at all.
        let unused = SAMPLE.replace("[[province]]\ncode = \"XA\"", "[source.three]\ntitle = \"Third\"\nurl = \"https://example.org/3\"\n\n[[province]]\ncode = \"XA\"");
        assert_eq!(
            generate(&unused).unwrap_err(),
            "the source three is used by nothing: use it or take it out"
        );
        let both = SAMPLE.replace(
            "[[province]]\ncode = \"XB\"",
            "[province.no_tire_fee]\nstatement = \"No fee.\"\nsources = [\"one\"]\n\n[[province]]\ncode = \"XB\"",
        );
        assert_eq!(
            generate(&both).unwrap_err(),
            "XA: has both tire fees and no_tire_fee"
        );
        let none = SAMPLE.split("[[province]]").next().unwrap();
        assert!(
            generate(none)
                .unwrap_err()
                .contains("missing field `province`")
        );
        assert!(generate("").is_err());
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
