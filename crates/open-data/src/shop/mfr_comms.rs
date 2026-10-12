//! NHTSA's manufacturer communications (service bulletins, warranty
//! extensions, over-the-air updates), read row by row. Nothing is matched
//! to the catalog here: `parse` only says what each row of the files holds.
//!
//! The archives are read as `open-data fetch nhtsa-mfr-comms` saved them,
//! without unpacking them to disk. Each holds one text file: one record
//! per line, 14 fields separated by tabs, no header, lines ended by LF.
//! The text is UTF-8, but some of it was UTF-8 read as Latin-1 and written
//! out again, once or twice over, and some was typed in Windows-1252:
//! [`tidy`] puts both right.

use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};
use std::fs::File;
use std::io::{BufRead, BufReader};
use std::path::Path;

use anyhow::{Context, Result, bail};
use rusqlite::{Transaction, params};
use wenmar_shopdata::bulletins::BulletinType;
use wenmar_shopdata::{Dataset, DatasetEntry, licence, schema};
use wenmar_vehicles::text::squeeze;

use crate::shop::build::ShapeContext;
use crate::shop::matcher::{Matcher, NO_YEAR};
use crate::shop::nhtsa_recalls::{COMPONENT_SEPARATOR, iso_date};
use crate::sources::SourceRecord;

/// NHTSA's description of the fields, fetched beside the archives.
pub const LAYOUT_FILE: &str = "TSBS.txt";

/// What the name of every archive starts and ends with.
pub const ARCHIVE_PREFIX: &str = "TSBS_RECEIVED_";
pub const ARCHIVE_SUFFIX: &str = ".zip";

/// The 14 fields of a record, in order, as `TSBS.txt` names them.
pub const FIELDS: [&str; 14] = [
    "NHTSA ID Number",
    "Replacement Service Bulletin Number",
    "Date Added to File",
    "TSB/Document ID",
    "Mfr Communication Date",
    "Mfr Internal Campaign ID/Software Version",
    "Communication Type",
    "Make",
    "Model",
    "Model Year",
    "NHTSA Components",
    "Mfr Component System",
    "Mfr Component Subsystem",
    "Summary",
];

/// The six communication types the files hold, as the files spell them.
/// `TSBS.txt` writes the first and the third with spaces around the slash;
/// the records have none.
pub const TYPES: [&str; 6] = [
    "Service Bulletin/Repair Instructions",
    "Service Campaign",
    "Warranty Program/Extension",
    "Over The Air",
    "Emissions",
    "Other",
];

/// One record: one communication, make, model, model year and component.
/// A communication has one record for each vehicle it lists times each
/// component it names; what it says of itself is the same on every one.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MfrCommRow {
    /// Field 1, `NHTSA ID Number`, such as `10137505`.
    pub nhtsa_id: u32,
    /// Field 2, `Replacement Service Bulletin Number`. NHTSA no longer
    /// fills it.
    pub replaces: Option<String>,
    /// Field 3, `Date Added to File`, as `2018-06-21`: the day NHTSA
    /// published the communication.
    pub added: String,
    /// Field 4, `TSB/Document ID`: the manufacturer's own number.
    pub document_id: Option<String>,
    /// Field 5, `Mfr Communication Date`, as `2018-06-01`.
    pub date: Option<String>,
    /// Field 6, `Mfr Internal Campaign ID/Software Version`.
    pub mfr_campaign: Option<String>,
    /// Field 7, `Communication Type`, as the file words it. One of
    /// [`TYPES`] in every record of 2026-10-11.
    pub kind: String,
    /// Field 8, `Make`.
    pub make_text: String,
    /// Field 9, `Model`.
    pub model_text: String,
    /// Field 10, `Model Year`. `None` for `9999`, NHTSA's "unknown".
    pub year: Option<i32>,
    /// Field 11, `NHTSA Components`: one component, written as a path such
    /// as `ENGINE AND ENGINE COOLING:ENGINE:OIL/LUBRICATION`. A comma is
    /// part of a name (`FUEL SYSTEM, GASOLINE`), never a separator.
    pub component: Option<String>,
    /// Fields 12 and 13, `Mfr Component System` and `Mfr Component
    /// Subsystem`: the manufacturer's own words.
    pub mfr_system: Option<String>,
    pub mfr_subsystem: Option<String>,
    /// Field 14, `Summary`.
    pub summary: Option<String>,
}

/// What `parse` read.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct MfrCommCounts {
    /// Archives read.
    pub files: u64,
    /// Records handed on.
    pub records: u64,
}

/// The field names a copy of `TSBS.txt` lists, in order: on each line that
/// starts with a number, the text between the first pair of double quotes.
pub fn layout(tsbs_txt: &str) -> Vec<String> {
    tsbs_txt
        .lines()
        .filter_map(|line| {
            let line = line.trim_start();
            let (number, rest) = line.split_once(char::is_whitespace)?;
            let number: usize = number.parse().ok()?;
            let rest = rest.trim_start().strip_prefix('"')?;
            let (name, _) = rest.split_once('"')?;
            (number >= 1).then(|| squeeze(name))
        })
        .collect()
}

/// Refuses a copy of `TSBS.txt` whose fields are not [`FIELDS`].
pub fn check_layout(tsbs_txt: &str) -> Result<()> {
    let found = layout(tsbs_txt);
    if found != FIELDS {
        bail!(
            "the fields are not the ones this build reads: found {} ({}), expected {} ({})",
            found.len(),
            found.join(", "),
            FIELDS.len(),
            FIELDS.join(", ")
        );
    }
    Ok(())
}

/// One pass of [`repair`]: every run of characters that is the bytes of
/// one UTF-8 character, each byte written as the Latin-1 character of the
/// same number, becomes that character.
fn repair_once(text: &str) -> Option<String> {
    let characters: Vec<char> = text.chars().collect();
    let mut repaired = String::with_capacity(text.len());
    let mut changed = false;
    let mut index = 0;
    while index < characters.len() {
        let lead = characters[index] as u32;
        let following = match lead {
            0xC2..=0xDF => 1,
            0xE0..=0xEF => 2,
            0xF0..=0xF4 => 3,
            _ => 0,
        };
        let end = index + 1 + following;
        if following != 0
            && end <= characters.len()
            && characters[index + 1..end]
                .iter()
                .all(|character| (0x80..=0xBF).contains(&(*character as u32)))
        {
            let bytes: Vec<u8> = characters[index..end]
                .iter()
                .map(|character| *character as u32 as u8)
                .collect();
            if let Ok(decoded) = std::str::from_utf8(&bytes) {
                repaired.push_str(decoded);
                changed = true;
                index = end;
                continue;
            }
        }
        repaired.push(characters[index]);
        index += 1;
    }
    changed.then_some(repaired)
}

/// Text that was UTF-8 read as Latin-1, up to three times over, put back:
/// `â\u{80}\u{99}` is a right single quote.
pub fn repair(text: &str) -> String {
    let mut text = text.to_owned();
    for _ in 0..3 {
        match repair_once(&text) {
            Some(repaired) => text = repaired,
            None => break,
        }
    }
    text
}

/// A text field repaired, with its Windows-1252 punctuation put right, its
/// control characters dropped and every run of spaces made one. `None`
/// when nothing is left.
pub fn tidy(text: &str) -> Option<String> {
    let repaired = repair(text);
    let mut tidy = String::with_capacity(repaired.len());
    for character in repaired.chars() {
        match character {
            '\u{85}' => tidy.push('\u{2026}'),
            '\u{91}' => tidy.push('\u{2018}'),
            '\u{92}' => tidy.push('\u{2019}'),
            '\u{93}' => tidy.push('\u{201c}'),
            '\u{94}' => tidy.push('\u{201d}'),
            '\u{95}' => tidy.push('\u{2022}'),
            '\u{96}' => tidy.push('\u{2013}'),
            '\u{97}' => tidy.push('\u{2014}'),
            '\u{99}' => tidy.push('\u{2122}'),
            '\u{a0}' | '\u{3000}' => tidy.push(' '),
            // A byte-order mark and a zero-width space say nothing.
            '\u{feff}' | '\u{200b}' => {}
            // A bullet typed in a symbol font arrives as a private-use
            // character.
            '\u{e000}'..='\u{f8ff}' => tidy.push('\u{2022}'),
            other if other.is_control() => {}
            other => tidy.push(other),
        }
    }
    let tidy = squeeze(&tidy);
    (!tidy.is_empty()).then_some(tidy)
}

/// A number or a code a manufacturer may have left as a placeholder:
/// `N/A`, `NA`, `NONE` and `TBD` are `None`.
pub fn given(text: &str) -> Option<String> {
    let value = tidy(text)?;
    let placeholder = ["N/A", "NA", "NONE", "TBD"]
        .iter()
        .any(|word| value.eq_ignore_ascii_case(word));
    (!placeholder).then_some(value)
}

fn required(fields: &[&str], index: usize) -> Result<String> {
    tidy(fields[index]).with_context(|| format!("{} is empty", FIELDS[index]))
}

/// Reads the 14 fields of one record.
fn read_row(fields: &[&str]) -> Result<MfrCommRow> {
    let nhtsa_id = fields[0]
        .trim()
        .parse()
        .with_context(|| format!("NHTSA ID Number {:?} is not a number", fields[0]))?;
    let added = iso_date(fields[2])
        .with_context(|| format!("Date Added to File {:?} is not a date", fields[2]))?;
    let year = match fields[9].trim().parse::<i32>() {
        Ok(year) if year != NO_YEAR && (1900..=2100).contains(&year) => Some(year),
        _ => None,
    };
    Ok(MfrCommRow {
        nhtsa_id,
        replaces: given(fields[1]),
        added,
        document_id: given(fields[3]),
        date: iso_date(fields[4]),
        mfr_campaign: given(fields[5]),
        kind: required(fields, 6)?,
        make_text: required(fields, 7)?,
        model_text: required(fields, 8)?,
        year,
        component: tidy(fields[10]),
        mfr_system: tidy(fields[11]),
        mfr_subsystem: tidy(fields[12]),
        summary: tidy(fields[13]),
    })
}

/// Reads one file's text, a line at a time, and hands each record to
/// `row`. `origin` names the file in an error.
pub fn parse_reader(
    mut reader: impl BufRead,
    origin: &str,
    counts: &mut MfrCommCounts,
    row: &mut dyn FnMut(MfrCommRow) -> Result<()>,
) -> Result<()> {
    let mut bytes = Vec::new();
    let mut number = 0u64;
    loop {
        bytes.clear();
        let read = reader
            .read_until(b'\n', &mut bytes)
            .with_context(|| format!("reading {origin}"))?;
        if read == 0 {
            return Ok(());
        }
        number += 1;
        let line = String::from_utf8_lossy(&bytes);
        let line = line.trim_end_matches(['\n', '\r']);
        let fields: Vec<&str> = line.split('\t').collect();
        if fields.len() != FIELDS.len() {
            bail!(
                "{origin} line {number} has {} fields, expected {}",
                fields.len(),
                FIELDS.len()
            );
        }
        counts.records += 1;
        row(read_row(&fields).with_context(|| format!("{origin} line {number}"))?)?;
    }
}

/// The archives a fetched directory holds, in the order `source.json`
/// lists them: oldest first. A file left from an earlier fetch under
/// another name is not read.
pub fn archives(directory: &Path) -> Result<Vec<String>> {
    let record = SourceRecord::read(directory)?;
    let names: Vec<String> = record
        .files
        .into_iter()
        .map(|file| file.name)
        .filter(|name| name.starts_with(ARCHIVE_PREFIX) && name.ends_with(ARCHIVE_SUFFIX))
        .collect();
    if names.is_empty() {
        bail!(
            "{} lists no {ARCHIVE_PREFIX}*{ARCHIVE_SUFFIX} file; run open-data fetch nhtsa-mfr-comms again",
            directory.join("source.json").display()
        );
    }
    Ok(names)
}

/// Reads the fetched `nhtsa-mfr-comms` directory: checks `TSBS.txt`, then
/// hands every record of every archive to `row`, in file order.
pub fn parse(
    directory: &Path,
    row: &mut dyn FnMut(MfrCommRow) -> Result<()>,
) -> Result<MfrCommCounts> {
    let layout_path = directory.join(LAYOUT_FILE);
    let layout_bytes = std::fs::read(&layout_path).with_context(|| {
        format!(
            "reading {}; run open-data fetch nhtsa-mfr-comms first",
            layout_path.display()
        )
    })?;
    check_layout(&String::from_utf8_lossy(&layout_bytes))
        .with_context(|| layout_path.display().to_string())?;

    let mut counts = MfrCommCounts::default();
    for name in archives(directory)? {
        let path = directory.join(&name);
        let file = File::open(&path).with_context(|| {
            format!(
                "opening {}; run open-data fetch nhtsa-mfr-comms first",
                path.display()
            )
        })?;
        let mut archive = zip::ZipArchive::new(file)
            .with_context(|| format!("{} is not a zip file", path.display()))?;
        if archive.len() != 1 {
            bail!(
                "{} holds {} files, expected one",
                path.display(),
                archive.len()
            );
        }
        let entry = archive
            .by_index(0)
            .with_context(|| format!("reading {}", path.display()))?;
        parse_reader(BufReader::new(entry), &name, &mut counts, row)?;
        counts.files += 1;
    }
    Ok(counts)
}

/// What separates the parts of a manufacturer's own component words.
pub const SYSTEM_SEPARATOR: &str = ": ";

const INSERT_BULLETIN: &str = "
INSERT INTO bulletin (id, type, added, date, document_id, mfr_campaign, replaces, component, mfr_system, summary)
VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)";

const INSERT_MODEL: &str = "
INSERT INTO bulletin_model (id, make_text, model_text, make_id, model_id, \"match\")
VALUES (?1, ?2, ?3, ?4, ?5, ?6)";

const INSERT_VEHICLE: &str = "
INSERT INTO bulletin_vehicle (bulletin_id, bulletin_model_id, year_from, year_to)
VALUES (?1, ?2, ?3, ?4)";

/// A communication gathered from its records.
struct Communication {
    /// The first record. What a communication says of itself is the same
    /// on every one of its records.
    first: MfrCommRow,
    /// Every component and every manufacturer's system its records name,
    /// each once, in file order.
    components: Vec<String>,
    systems: Vec<String>,
    /// Each name and model year it lists, once. A name is an index into
    /// [`Names::list`].
    vehicles: BTreeSet<(u32, Option<i32>)>,
}

/// Every make and model text met so far, and what each matched.
#[derive(Default)]
struct Names {
    ids: HashMap<(String, String), u32>,
    list: Vec<(String, String)>,
    /// `(name, model year)` to the `bulletin_model` rows it matched.
    resolved: HashMap<(u32, Option<i32>), Vec<i64>>,
    /// `(name, catalog model)` to its `bulletin_model` row.
    models: HashMap<(u32, Option<i64>), i64>,
}

impl Names {
    fn id(&mut self, make_text: &str, model_text: &str) -> u32 {
        let key = (make_text.to_owned(), model_text.to_owned());
        if let Some(id) = self.ids.get(&key) {
            return *id;
        }
        let id = self.list.len() as u32;
        self.list.push(key.clone());
        self.ids.insert(key, id);
        id
    }

    /// The `bulletin_model` rows one name matches in one model year,
    /// stored the first time each is met.
    fn rows(
        &mut self,
        transaction: &Transaction<'_>,
        matcher: &Matcher,
        name: u32,
        year: Option<i32>,
    ) -> Result<Vec<i64>> {
        if let Some(rows) = self.resolved.get(&(name, year)) {
            return Ok(rows.clone());
        }
        let (make_text, model_text) = &self.list[name as usize];
        let resolved = matcher.resolve(year, make_text, model_text);
        let model_ids: Vec<Option<i64>> = if resolved.model_ids.is_empty() {
            vec![None]
        } else {
            resolved.model_ids.iter().copied().map(Some).collect()
        };
        let mut rows = Vec::with_capacity(model_ids.len());
        for model_id in model_ids {
            let row = match self.models.get(&(name, model_id)) {
                Some(row) => *row,
                None => {
                    let row = self.models.len() as i64 + 1;
                    transaction.prepare_cached(INSERT_MODEL)?.execute(params![
                        row,
                        make_text,
                        model_text,
                        resolved.make_id,
                        model_id,
                        model_id.and(resolved.match_text()),
                    ])?;
                    self.models.insert((name, model_id), row);
                    row
                }
            };
            rows.push(row);
        }
        self.resolved.insert((name, year), rows.clone());
        Ok(rows)
    }
}

/// The runs of consecutive years in a sorted list without repeats:
/// `[2016, 2017, 2018, 2021]` is `[(2016, 2018), (2021, 2021)]`.
pub fn year_runs(years: &[i32]) -> Vec<(i32, i32)> {
    let mut runs: Vec<(i32, i32)> = Vec::new();
    for year in years {
        match runs.last_mut() {
            Some((_, last)) if *last + 1 == *year => *last = *year,
            _ => runs.push((*year, *year)),
        }
    }
    runs
}

fn add_once(list: &mut Vec<String>, value: Option<String>) {
    if let Some(value) = value
        && !list.contains(&value)
    {
        list.push(value);
    }
}

/// What `shape` has stored so far.
#[derive(Default)]
struct Stored {
    bulletins: u64,
}

fn store(
    context: &ShapeContext<'_>,
    names: &mut Names,
    stored: &mut Stored,
    communication: Communication,
) -> Result<()> {
    let transaction = context.transaction;
    let first = &communication.first;
    let kind = match BulletinType::from_source(&first.kind) {
        Some(kind) => kind,
        None => {
            eprintln!(
                "warning: communication {} has the type {:?}, stored as other",
                first.nhtsa_id, first.kind
            );
            BulletinType::Other
        }
    };
    let component = communication.components.join(COMPONENT_SEPARATOR);
    let system = communication.systems.join(COMPONENT_SEPARATOR);
    transaction
        .prepare_cached(INSERT_BULLETIN)?
        .execute(params![
            first.nhtsa_id,
            kind.id(),
            first.added,
            first.date,
            first.document_id,
            first.mfr_campaign,
            first.replaces,
            (!component.is_empty()).then_some(component),
            (!system.is_empty()).then_some(system),
            first.summary,
        ])
        .with_context(|| format!("storing communication {}", first.nhtsa_id))?;
    stored.bulletins += 1;

    // bulletin_model row to the model years listed for it.
    let mut years: BTreeMap<i64, BTreeSet<Option<i32>>> = BTreeMap::new();
    for (name, year) in &communication.vehicles {
        for row in names.rows(transaction, context.matcher, *name, *year)? {
            years.entry(row).or_default().insert(*year);
        }
    }
    let mut statement = transaction.prepare_cached(INSERT_VEHICLE)?;
    for (row, listed) in &years {
        if listed.contains(&None) {
            statement.execute(params![
                first.nhtsa_id,
                row,
                schema::BULLETIN_NO_YEAR,
                schema::BULLETIN_NO_YEAR
            ])?;
        }
        let known: Vec<i32> = listed.iter().flatten().copied().collect();
        for (from, to) in year_runs(&known) {
            statement.execute(params![first.nhtsa_id, row, from, to])?;
        }
    }
    Ok(())
}

/// Builds the `bulletins` dataset from the fetched `nhtsa-mfr-comms`
/// directory: one `bulletin` row for each communication, one
/// `bulletin_model` row for each make and model text and catalog model it
/// matched, and one `bulletin_vehicle` row for each run of model years.
///
/// The records of one communication are together in the files, so each
/// communication is stored as soon as the next one starts.
pub fn shape(context: &mut ShapeContext<'_>, directory: &Path) -> Result<()> {
    let as_of = SourceRecord::read(directory)?.as_of;
    context.transaction.execute_batch(schema::BULLETINS)?;
    for year in schema::BULLETIN_YEARS.0..=schema::BULLETIN_YEARS.1 {
        context
            .transaction
            .execute("INSERT INTO bulletin_year (year) VALUES (?1)", [year])?;
    }
    let mut names = Names::default();
    let mut stored = Stored::default();
    let mut current: Option<Communication> = None;
    let mut done: HashSet<u32> = HashSet::new();
    {
        let context = &*context;
        parse(directory, &mut |row| {
            if current
                .as_ref()
                .is_some_and(|communication| communication.first.nhtsa_id != row.nhtsa_id)
                && let Some(finished) = current.take()
            {
                done.insert(finished.first.nhtsa_id);
                store(context, &mut names, &mut stored, finished)?;
            }
            if done.contains(&row.nhtsa_id) {
                bail!(
                    "the records of communication {} are not together in the files",
                    row.nhtsa_id
                );
            }
            let name = names.id(&row.make_text, &row.model_text);
            let year = row.year;
            let system = match (row.mfr_system.clone(), row.mfr_subsystem.clone()) {
                (Some(system), Some(subsystem)) => {
                    Some(format!("{system}{SYSTEM_SEPARATOR}{subsystem}"))
                }
                (system, subsystem) => system.or(subsystem),
            };
            let component = row.component.clone();
            let communication = current.get_or_insert_with(|| Communication {
                first: row,
                components: Vec::new(),
                systems: Vec::new(),
                vehicles: BTreeSet::new(),
            });
            add_once(&mut communication.components, component);
            add_once(&mut communication.systems, system);
            communication.vehicles.insert((name, year));
            Ok(())
        })?;
        if let Some(finished) = current.take() {
            store(context, &mut names, &mut stored, finished)?;
        }
    }
    context.register(
        Dataset::Bulletins,
        DatasetEntry {
            as_of,
            rows: stored.bulletins,
            licence: licence::PUBLIC_DOMAIN.to_owned(),
            attribution: String::new(),
        },
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn line(change: impl FnOnce(&mut Vec<String>)) -> String {
        let mut fields: Vec<String> = vec![String::new(); 14];
        for (index, value) in [
            (0, "10152456"),
            (2, "20190107"),
            (3, "TSB 18-2324"),
            (4, "20181102"),
            (6, "Service Bulletin/Repair Instructions"),
            (7, "FORD"),
            (8, "F-150"),
            (9, "2018"),
            (10, "ENGINE"),
            (13, "A summary."),
        ] {
            fields[index] = value.to_owned();
        }
        change(&mut fields);
        fields.join("\t")
    }

    fn rows(text: &str) -> Result<(Vec<MfrCommRow>, MfrCommCounts)> {
        let mut found = Vec::new();
        let mut counts = MfrCommCounts::default();
        parse_reader(text.as_bytes(), "test.txt", &mut counts, &mut |row| {
            found.push(row);
            Ok(())
        })?;
        Ok((found, counts))
    }

    #[test]
    fn text_read_as_latin_1_is_put_back() {
        // Once, twice and three times over.
        assert_eq!(repair("state\u{e2}\u{80}\u{99}s"), "state\u{2019}s");
        assert_eq!(
            repair("state\u{c3}\u{a2}\u{c2}\u{80}\u{c2}\u{99}s"),
            "state\u{2019}s"
        );
        assert_eq!(
            repair("\u{c3}\u{83}\u{c2}\u{a2}\u{c3}\u{82}\u{c2}\u{80}\u{c3}\u{82}\u{c2}\u{a2} one"),
            "\u{2022} one"
        );
        assert_eq!(repair("AutoVue\u{c2}\u{ae}"), "AutoVue\u{ae}");
        // Text that is not damaged is left alone, accents included.
        for text in [
            "",
            "plain",
            "caf\u{e9} \u{2013} \u{e9}t\u{e9}",
            "\u{c2}",
            "\u{e2}\u{80}",
        ] {
            assert_eq!(repair(text), text);
        }
    }

    #[test]
    fn text_is_tidied() {
        assert_eq!(
            tidy("  THE SENSOR\u{92}S   SIGNALS \u{96} \u{93}ON\u{94}\u{85} ").as_deref(),
            Some("THE SENSOR\u{2019}S SIGNALS \u{2013} \u{201c}ON\u{201d}\u{2026}")
        );
        assert_eq!(
            tidy("Fusion\u{99} \u{95} one\u{a0}two").as_deref(),
            Some("Fusion\u{2122} \u{2022} one two")
        );
        // A bullet from a symbol font, a byte-order mark, a zero-width
        // space and control characters.
        assert_eq!(
            tidy("\u{feff}A\u{200b}B \u{f0b7} C\u{1}\u{1a}\u{81}D").as_deref(),
            Some("AB \u{2022} CD")
        );
        assert_eq!(tidy("   "), None);
        assert_eq!(tidy("\u{1}\u{2}"), None);
        assert_eq!(given("TSB 18-2324").as_deref(), Some("TSB 18-2324"));
        for placeholder in ["N/A", "n/a", "NA", "None", "TBD", " "] {
            assert_eq!(given(placeholder), None, "{placeholder}");
        }
    }

    #[test]
    fn a_line_is_a_record_or_an_error() {
        let text = format!(
            "{}\n{}\r\n{}",
            line(|_| {}),
            line(|fields| {
                fields[9] = "9999".to_owned();
                fields[4] = String::new();
                fields[6] = "Something New".to_owned();
                fields[11] = "Engine".to_owned();
                fields[12] = "Oil Pump".to_owned();
            }),
            line(|fields| fields[10] = "FUEL SYSTEM, GASOLINE:DELIVERY".to_owned()),
        );
        let (found, counts) = rows(&text).unwrap();
        assert_eq!(counts.records, 3);
        assert_eq!(found[0].nhtsa_id, 10_152_456);
        assert_eq!(found[0].year, Some(2018));
        assert_eq!(found[0].added, "2019-01-07");
        assert_eq!(found[0].date.as_deref(), Some("2018-11-02"));
        assert_eq!(found[1].year, None);
        assert_eq!(found[1].date, None);
        // A type this build does not know is kept as it is written.
        assert_eq!(found[1].kind, "Something New");
        assert_eq!(found[1].mfr_system.as_deref(), Some("Engine"));
        assert_eq!(found[1].mfr_subsystem.as_deref(), Some("Oil Pump"));
        assert_eq!(
            found[2].component.as_deref(),
            Some("FUEL SYSTEM, GASOLINE:DELIVERY")
        );

        let short = format!("{}\n1\t2\t3\n", line(|_| {}));
        assert_eq!(
            format!("{:#}", rows(&short).unwrap_err()),
            "test.txt line 2 has 3 fields, expected 14"
        );
        assert_eq!(
            format!("{:#}", rows("\n").unwrap_err()),
            "test.txt line 1 has 1 fields, expected 14"
        );
        for (index, value, expected) in [
            (
                0,
                "x",
                "test.txt line 1: NHTSA ID Number \"x\" is not a number: invalid digit found in string",
            ),
            (
                2,
                "",
                "test.txt line 1: Date Added to File \"\" is not a date",
            ),
            (6, " ", "test.txt line 1: Communication Type is empty"),
            (7, " ", "test.txt line 1: Make is empty"),
            (8, "", "test.txt line 1: Model is empty"),
        ] {
            let bad = line(|fields| fields[index] = value.to_owned());
            assert_eq!(format!("{:#}", rows(&bad).unwrap_err()), expected);
        }
        // Bytes that are not UTF-8 do not stop the build.
        let mut bytes = line(|_| {}).into_bytes();
        bytes.extend_from_slice(b"\xff\n");
        let mut counts = MfrCommCounts::default();
        parse_reader(&bytes[..], "test.txt", &mut counts, &mut |_| Ok(())).unwrap();
        assert_eq!(counts.records, 1);
    }

    #[test]
    fn the_layout_is_read_from_tsbs_txt() {
        let text = "Manufacturer Communications\n\nChange Log\n1. Sep 14, 2007: Changed flat file extension.\n2. Aug 22, 2008: Field \"BULNO\" was changed.\n   2) Renamed \"ID\" to \"NHTSA ID Number\".\n\nField#  Name   Type\n1       \"NHTSA ID Number\"     NUMBER(9)   NHTSA identifier.\n2       \"Replacement  Service Bulletin Number\"  CHAR(16)\n14\t\"Summary\"\tCHAR(4000)\n";
        assert_eq!(
            layout(text),
            [
                "NHTSA ID Number",
                "Replacement Service Bulletin Number",
                "Summary"
            ]
        );
        let error = check_layout(text).unwrap_err().to_string();
        assert!(
            error.starts_with("the fields are not the ones this build reads: found 3 (NHTSA ID Number, Replacement Service Bulletin Number, Summary), expected 14 ("),
            "{error}"
        );
    }

    #[test]
    fn consecutive_years_are_one_run() {
        assert_eq!(year_runs(&[]), []);
        assert_eq!(year_runs(&[2018]), [(2018, 2018)]);
        assert_eq!(
            year_runs(&[2012, 2013, 2014, 2016, 2019, 2020]),
            [(2012, 2014), (2016, 2016), (2019, 2020)]
        );
    }
}
