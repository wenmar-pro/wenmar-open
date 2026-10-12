//! `data/codes/`: trouble codes described in this project's own words,
//! built into the `code` and `code_component` tables.

use std::collections::BTreeMap;
use std::path::Path;

use anyhow::{Context, Result, bail};
use rusqlite::params;
use serde::Deserialize;
use wenmar_shopdata::codes::{RANGES, is_code, is_generic, system_fits};
use wenmar_shopdata::{Dataset, DatasetEntry, licence, schema};
use wenmar_vehicles::text::squeeze;

use crate::shop::build::ShapeContext;
use crate::shop::components::Components;

/// The longest a title may be, in characters.
pub const LONGEST_TITLE: usize = 120;
/// The longest a meaning may be.
pub const LONGEST_MEANING: usize = 1500;
/// The longest an entry of `common_causes` or `checks` may be.
pub const LONGEST_ENTRY: usize = 200;
/// The most entries `common_causes`, `checks` and `related` may have.
pub const MOST_ENTRIES: usize = 10;
/// The most components a record may name.
pub const MOST_COMPONENTS: usize = 8;

/// One record of a file, with its text tidied.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CodeRecord {
    pub code: String,
    pub system: String,
    pub title: String,
    pub meaning: String,
    pub common_causes: Vec<String>,
    pub checks: Vec<String>,
    #[serde(default)]
    pub components: Vec<String>,
    #[serde(default)]
    pub related: Vec<String>,
    pub author: String,
    pub reviewer: String,
    pub checked: String,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct CodesFile {
    codes: Vec<CodeRecord>,
}

/// Whether `text` is a day written `2026-10-12`.
pub fn is_day(text: &str) -> bool {
    let bytes = text.as_bytes();
    let shaped = bytes.len() == 10
        && bytes.iter().enumerate().all(|(index, byte)| match index {
            4 | 7 => *byte == b'-',
            _ => byte.is_ascii_digit(),
        });
    if !shaped {
        return false;
    }
    let number = |range: std::ops::Range<usize>| text[range].parse::<u32>().unwrap_or(0);
    (1..=12).contains(&number(5..7)) && (1..=31).contains(&number(8..10))
}

/// Whether `text` could be a GitHub user name: 1 to 39 letters, digits
/// and single hyphens, not starting or ending with a hyphen.
pub fn is_github_name(text: &str) -> bool {
    (1..=39).contains(&text.len())
        && !text.starts_with('-')
        && !text.ends_with('-')
        && !text.contains("--")
        && text
            .chars()
            .all(|character| character.is_ascii_alphanumeric() || character == '-')
}

/// The `.yaml` files of a directory as (name without `.yaml`, text), by
/// name. A file whose name starts with `_` is left out, and a directory
/// that does not exist has none.
pub fn files(directory: &Path) -> Result<Vec<(String, String)>> {
    if !directory.is_dir() {
        return Ok(Vec::new());
    }
    let mut found = Vec::new();
    for entry in
        std::fs::read_dir(directory).with_context(|| format!("reading {}", directory.display()))?
    {
        let path = entry?.path();
        let Some(name) = path
            .file_name()
            .and_then(|name| name.to_str())
            .and_then(|name| name.strip_suffix(".yaml"))
        else {
            continue;
        };
        if name.starts_with('_') {
            continue;
        }
        let text = std::fs::read_to_string(&path)
            .with_context(|| format!("reading {}", path.display()))?;
        found.push((name.to_owned(), text));
    }
    found.sort();
    Ok(found)
}

/// A list of short texts, tidied: `least` to [`MOST_ENTRIES`] entries,
/// none empty, none longer than [`LONGEST_ENTRY`].
fn entries(code: &str, field: &str, list: &[String], least: usize) -> Result<Vec<String>> {
    if list.len() < least || list.len() > MOST_ENTRIES {
        bail!("{code} needs {least} to {MOST_ENTRIES} {field}");
    }
    let mut tidy = Vec::with_capacity(list.len());
    for entry in list {
        let entry = squeeze(entry);
        if entry.is_empty() {
            bail!("{code} has an empty entry in {field}");
        }
        if entry.chars().count() > LONGEST_ENTRY {
            bail!("{code}: an entry in {field} is longer than {LONGEST_ENTRY} characters");
        }
        tidy.push(entry);
    }
    Ok(tidy)
}

/// Reads the text of one file, `<name>.yaml`, and checks everything that
/// can be checked without the other files.
pub fn parse(name: &str, yaml: &str) -> Result<Vec<CodeRecord>> {
    let file = format!("{name}.yaml");
    let range = name.to_ascii_uppercase();
    if !RANGES.contains(&range.as_str()) || name != range.to_ascii_lowercase() {
        bail!(
            "{file} is not a file of data/codes; the files are {}",
            RANGES
                .iter()
                .map(|range| format!("{}.yaml", range.to_ascii_lowercase()))
                .collect::<Vec<_>>()
                .join(", ")
        );
    }
    let parsed: CodesFile =
        serde_saphyr::from_str(yaml).with_context(|| format!("reading data/codes/{file}"))?;
    let mut records: Vec<CodeRecord> = Vec::with_capacity(parsed.codes.len());
    for record in parsed.codes {
        let code = record.code.clone();
        if !is_code(&code) {
            bail!("{file}: {code:?} is not a code; write it like P0001");
        }
        if !is_generic(&code) {
            bail!("{file}: {code} is not a generic code and may not have a record");
        }
        if !code.starts_with(&range) {
            bail!(
                "{file}: {code} belongs in {}.yaml",
                code[..2].to_ascii_lowercase()
            );
        }
        if records.last().is_some_and(|last| last.code >= code) {
            bail!("{file} lists {code} twice or out of order; keep the codes ascending");
        }
        if !system_fits(&code, &record.system) {
            bail!(
                "{file}: {code}: the system {} is not one a {} code may have",
                record.system,
                &code[..1]
            );
        }
        let title = squeeze(&record.title);
        if title.is_empty() {
            bail!("{file}: {code} has no title");
        }
        if title.chars().count() > LONGEST_TITLE {
            bail!("{file}: {code}: the title is longer than {LONGEST_TITLE} characters");
        }
        let meaning = squeeze(&record.meaning);
        if meaning.is_empty() {
            bail!("{file}: {code} has no meaning");
        }
        if meaning.chars().count() > LONGEST_MEANING {
            bail!("{file}: {code}: the meaning is longer than {LONGEST_MEANING} characters");
        }
        let in_file = |error: anyhow::Error| anyhow::anyhow!("{file}: {error}");
        let common_causes =
            entries(&code, "common_causes", &record.common_causes, 1).map_err(in_file)?;
        let checks = entries(&code, "checks", &record.checks, 1).map_err(in_file)?;
        if record.components.len() > MOST_COMPONENTS {
            bail!("{file}: {code} names more than {MOST_COMPONENTS} components");
        }
        if record.related.len() > MOST_ENTRIES {
            bail!("{file}: {code} has more than {MOST_ENTRIES} related codes");
        }
        for other in &record.related {
            if *other == code {
                bail!("{file}: {code} is related to itself");
            }
            if !is_generic(other) {
                bail!("{file}: {code}: the related code {other} is not a generic code");
            }
        }
        for (field, name) in [("author", &record.author), ("reviewer", &record.reviewer)] {
            if !is_github_name(name) {
                bail!("{file}: {code}: the {field} {name:?} is not a GitHub user name");
            }
        }
        if record.author.eq_ignore_ascii_case(&record.reviewer) {
            bail!("{file}: {code}: the reviewer is the author; someone else must review");
        }
        if !is_day(&record.checked) {
            bail!(
                "{file}: {code}: checked must be a day like 2026-10-12, not {:?}",
                record.checked
            );
        }
        records.push(CodeRecord {
            title,
            meaning,
            common_causes,
            checks,
            ..record
        });
    }
    Ok(records)
}

/// Every record of `<curated>/codes`, by code, with the checks that need
/// all of them: each component is in `components.yaml` and is named once,
/// and each related code has a record.
pub fn load(curated: &Path, components: &Components) -> Result<Vec<CodeRecord>> {
    let mut all: BTreeMap<String, CodeRecord> = BTreeMap::new();
    for (name, yaml) in files(&curated.join("codes"))? {
        for record in parse(&name, &yaml)? {
            all.insert(record.code.clone(), record);
        }
    }
    for record in all.values() {
        let code = &record.code;
        for (index, component) in record.components.iter().enumerate() {
            if !components.list.iter().any(|entry| &entry.id == component) {
                bail!("data/codes: {code}: {component} is not a component of components.yaml");
            }
            if record.components[..index].contains(component) {
                bail!("data/codes: {code} names the component {component} twice");
            }
        }
        for (index, other) in record.related.iter().enumerate() {
            if !all.contains_key(other) {
                bail!("data/codes: {code}: the related code {other} has no record");
            }
            if record.related[..index].contains(other) {
                bail!("data/codes: {code} lists the related code {other} twice");
            }
        }
    }
    Ok(all.into_values().collect())
}

/// Fills `code` and `code_component` from `<curated>/codes` and registers
/// the `codes` dataset. With no record it does nothing at all. Runs in
/// every build: the records are in the repository, not fetched.
pub fn shape(context: &mut ShapeContext<'_>) -> Result<()> {
    if files(&context.curated.join("codes"))?.is_empty() {
        return Ok(());
    }
    let components = Components::load(context.curated)?;
    let records = load(context.curated, &components)?;
    let Some(as_of) = records.iter().map(|record| record.checked.clone()).max() else {
        return Ok(());
    };
    let transaction = context.transaction;
    transaction.execute_batch(schema::CODES)?;
    {
        let mut code = transaction.prepare(
            "INSERT INTO code (code, system, title, meaning, common_causes, checks, related, author, reviewer, checked)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)",
        )?;
        let mut component = transaction.prepare(
            "INSERT INTO code_component (code, component_id, position) VALUES (?1, ?2, ?3)",
        )?;
        for record in &records {
            code.execute(params![
                record.code,
                record.system,
                record.title,
                record.meaning,
                serde_json::to_string(&record.common_causes)?,
                serde_json::to_string(&record.checks)?,
                serde_json::to_string(&record.related)?,
                record.author,
                record.reviewer,
                record.checked,
            ])?;
            for (position, id) in record.components.iter().enumerate() {
                component.execute(params![record.code, id, position as i64])?;
            }
        }
    }
    context.register(
        Dataset::Codes,
        DatasetEntry {
            as_of,
            rows: records.len() as u64,
            licence: licence::MIT.to_owned(),
            attribution: String::new(),
        },
    )
}
