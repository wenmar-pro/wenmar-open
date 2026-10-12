//! NHTSA's index of the letters manufacturers file under 49 CFR Part 565
//! to explain how their VINs are built, from vPIC's `GetParts`.
//!
//! Only the index is fetched and stored: who filed, when, for which model
//! years, and the address of NHTSA's copy of the letter. The letters are
//! the manufacturers' own documents and are never downloaded.
//!
//! `GetParts` answers with at most 1,000 filings and no way to tell that
//! more were left out, so the fetch asks for one range of dates at a time
//! and never keeps an answer that holds 1,000: a year that does is asked
//! for again month by month.

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};
use std::time::Duration;

use anyhow::{Context, Result, bail};
use serde::Deserialize;
use wenmar_shopdata::vin_guides::{DOCUMENTS, is_document_url};
use wenmar_shopdata::{Dataset, DatasetEntry, licence, schema};
use wenmar_vehicles::text::normalize;

use crate::shop::build::ShapeContext;
use crate::shop::nhtsa_recalls::tidy;
use crate::sources::{Downloaded, FileRecord, SourceRecord, directory, download};

/// The source's id: `open-data fetch nhtsa-vin-guides`.
pub const SOURCE_ID: &str = "nhtsa-vin-guides";

/// vPIC's list of the documents manufacturers have filed.
pub const API: &str = "https://vpic.nhtsa.dot.gov/api/vehicles/GetParts";

/// The kind of filing asked for: letters under 49 CFR Part 565. The other
/// kind, `566`, is manufacturer identification and says nothing of a VIN.
pub const KIND: &str = "565";

/// The most filings one answer holds. An answer with this many may have
/// left some out.
pub const CAP: usize = 1000;

/// The first year asked for on its own. Everything older is one range.
pub const FIRST_YEAR: u32 = 1985;

/// How long the fetch waits after each request.
pub const PAUSE: Duration = Duration::from_millis(1000);

/// One range of letter dates, both ends included, and the file its answer
/// is saved as.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Range {
    /// As the API reads a date: `1/1/2026`.
    pub from: String,
    pub to: String,
    pub file: String,
}

impl Range {
    /// Every letter dated before [`FIRST_YEAR`].
    pub fn before() -> Range {
        let last = FIRST_YEAR - 1;
        Range {
            from: "1/1/1900".to_owned(),
            to: format!("12/31/{last}"),
            file: format!("{KIND}-1900-{last}.json"),
        }
    }

    pub fn year(year: u32) -> Range {
        Range {
            from: format!("1/1/{year}"),
            to: format!("12/31/{year}"),
            file: format!("{KIND}-{year}.json"),
        }
    }

    pub fn month(year: u32, month: u32) -> Range {
        Range {
            from: format!("{month}/1/{year}"),
            to: format!("{month}/{}/{year}", days_in_month(year, month)),
            file: format!("{KIND}-{year}-{month:02}.json"),
        }
    }

    /// The address that answers with the filings of this range.
    pub fn url(&self) -> String {
        format!(
            "{API}?type={KIND}&fromDate={}&toDate={}&format=json",
            self.from, self.to
        )
    }
}

/// The days of a month, 1 to 12.
pub fn days_in_month(year: u32, month: u32) -> u32 {
    match month {
        4 | 6 | 9 | 11 => 30,
        2 if year.is_multiple_of(4) && (!year.is_multiple_of(100) || year.is_multiple_of(400)) => {
            29
        }
        2 => 28,
        _ => 31,
    }
}

/// One answer of `GetParts`. A field this build does not know is an error:
/// the API has changed and the build must be looked at.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct Answer {
    #[serde(rename = "Count")]
    count: usize,
    #[serde(rename = "Message")]
    _message: String,
    #[serde(rename = "SearchCriteria")]
    _search_criteria: Option<String>,
    #[serde(rename = "Results")]
    results: Vec<Filing>,
}

/// One filing as the API gives it.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct Filing {
    #[serde(rename = "CoverLetterURL")]
    _cover_letter_url: Option<String>,
    #[serde(rename = "LetterDate")]
    letter_date: String,
    #[serde(rename = "ManufacturerId")]
    manufacturer_id: i64,
    #[serde(rename = "ManufacturerName")]
    manufacturer_name: String,
    #[serde(rename = "ModelYearFrom")]
    model_year_from: Option<i64>,
    #[serde(rename = "ModelYearTo")]
    model_year_to: Option<i64>,
    #[serde(rename = "Name")]
    name: String,
    #[serde(rename = "Type")]
    kind: String,
    #[serde(rename = "URL")]
    url: String,
}

fn read_answer(bytes: &[u8], origin: &str) -> Result<Answer> {
    let answer: Answer = serde_json::from_slice(bytes)
        .with_context(|| format!("{origin} is not an answer of GetParts as this build reads it"))?;
    if answer.count != answer.results.len() {
        bail!(
            "{origin} says it holds {} filings and holds {}",
            answer.count,
            answer.results.len()
        );
    }
    Ok(answer)
}

/// One filing, as it is stored.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VinGuideRow {
    /// `Name`: NHTSA's name for the document, such as `ORG26610`.
    pub name: String,
    /// `ManufacturerId`: vPIC's number for the manufacturer.
    pub manufacturer_id: i64,
    /// `ManufacturerName`, trimmed.
    pub manufacturer: String,
    /// `LetterDate`, as `2026-06-01`.
    pub letter_date: String,
    /// `ModelYearFrom` and `ModelYearTo`: given for about a third of the
    /// filings. `None` when not given or not a year from 1900 to 2100.
    pub year_from: Option<u16>,
    pub year_to: Option<u16>,
    /// `URL`, as an `https` address: NHTSA's copy of the document.
    pub url: String,
}

/// What `parse` read.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct VinGuideCounts {
    pub files: u64,
    pub filings: u64,
}

/// A date written `6/1/2026`, as `2026-06-01`.
pub fn iso_date(text: &str) -> Option<String> {
    let mut parts = text.trim().split('/');
    let month: u32 = parts.next()?.parse().ok()?;
    let day: u32 = parts.next()?.parse().ok()?;
    let year: u32 = parts.next()?.parse().ok()?;
    if parts.next().is_some() {
        return None;
    }
    ((1900..=2100).contains(&year)
        && (1..=12).contains(&month)
        && day >= 1
        && day <= days_in_month(year, month))
    .then(|| format!("{year:04}-{month:02}-{day:02}"))
}

/// The address of NHTSA's copy, as `https`. The API writes it with `http`,
/// which NHTSA redirects. `None` for any address that is not NHTSA's copy
/// of a filing.
pub fn document_url(url: &str) -> Option<String> {
    let url = url.trim();
    let rest = url
        .strip_prefix("http://")
        .or_else(|| url.strip_prefix("https://"))?;
    let url = format!("https://{rest}");
    is_document_url(&url).then_some(url)
}

fn model_year(year: Option<i64>) -> Option<u16> {
    year.filter(|year| (1900..=2100).contains(year))
        .and_then(|year| u16::try_from(year).ok())
}

fn read_filing(filing: Filing) -> Result<VinGuideRow> {
    let name = tidy(&filing.name).context("Name is empty")?;
    if filing.kind != KIND {
        bail!("{name} is of type {:?}, expected {KIND}", filing.kind);
    }
    let Some(letter_date) = iso_date(&filing.letter_date) else {
        bail!("{name} has the letter date {:?}", filing.letter_date);
    };
    let Some(url) = document_url(&filing.url) else {
        bail!(
            "{name} has the address {:?}, which is not under {DOCUMENTS}",
            filing.url
        );
    };
    let manufacturer = tidy(&filing.manufacturer_name)
        .with_context(|| format!("{name} has no manufacturer name"))?;
    Ok(VinGuideRow {
        name,
        manufacturer_id: filing.manufacturer_id,
        manufacturer,
        letter_date,
        year_from: model_year(filing.model_year_from),
        year_to: model_year(filing.model_year_to),
        url,
    })
}

/// Reads one saved answer and hands each filing to `row`. Returns how many
/// it held. `origin` names the file in an error.
pub fn parse_answer(
    bytes: &[u8],
    origin: &str,
    row: &mut dyn FnMut(VinGuideRow) -> Result<()>,
) -> Result<u64> {
    let answer = read_answer(bytes, origin)?;
    if answer.results.len() >= CAP {
        bail!(
            "{origin} holds {CAP} filings, the most one answer can: some may be missing. Run open-data fetch {SOURCE_ID} again"
        );
    }
    let mut filings = 0;
    for filing in answer.results {
        row(read_filing(filing).with_context(|| origin.to_owned())?)?;
        filings += 1;
    }
    Ok(filings)
}

/// Reads the fetched `nhtsa-vin-guides` directory: every answer that
/// `source.json` lists, in the order it lists them.
pub fn parse(
    directory: &Path,
    row: &mut dyn FnMut(VinGuideRow) -> Result<()>,
) -> Result<VinGuideCounts> {
    let record = SourceRecord::read(directory)?;
    if record.source != SOURCE_ID {
        bail!(
            "{} holds the source {}, expected {SOURCE_ID}",
            directory.display(),
            record.source
        );
    }
    let mut counts = VinGuideCounts::default();
    for file in &record.files {
        let path = directory.join(&file.name);
        let bytes = std::fs::read(&path).with_context(|| {
            format!(
                "reading {}; run open-data fetch {SOURCE_ID} first",
                path.display()
            )
        })?;
        counts.filings += parse_answer(&bytes, &file.name, row)?;
        counts.files += 1;
    }
    Ok(counts)
}

/// Fetches the whole index through `download`, waiting `pause` after each
/// request, and writes `source.json`. Returns the saved answers' paths in
/// order, then `source.json`.
///
/// The ranges: everything before [`FIRST_YEAR`], then each year up to the
/// year of `fetched_at`. A year whose answer holds [`CAP`] filings is not
/// kept; its twelve months are asked for instead. A month, or the range of
/// old letters, that holds [`CAP`] is an error: nothing narrower is asked
/// for, and a file that may lack filings is never left behind.
pub fn fetch_with(
    build_dir: &Path,
    fetched_at: &str,
    pause: Duration,
    download: &mut dyn FnMut(&str, &Path) -> Result<Downloaded>,
) -> Result<Vec<PathBuf>> {
    let last_year: u32 = fetched_at
        .get(0..4)
        .and_then(|year| year.parse().ok())
        .filter(|year| (FIRST_YEAR..=2100).contains(year))
        .with_context(|| format!("{fetched_at:?} does not start with a year"))?;
    let directory = directory(build_dir, SOURCE_ID);
    std::fs::create_dir_all(&directory)
        .with_context(|| format!("creating {}", directory.display()))?;

    let mut paths = Vec::new();
    let mut files = Vec::new();
    // Asks for one range. `Ok(false)` when the answer is full and was not
    // kept.
    let mut ask = |range: &Range| -> Result<bool> {
        let url = range.url();
        let part = directory.join(format!("{}.part", range.file));
        let downloaded = download(&url, &part)?;
        std::thread::sleep(pause);
        let bytes = std::fs::read(&part).with_context(|| format!("reading {}", part.display()))?;
        let held = read_answer(&bytes, &url)?.results.len();
        if held >= CAP {
            std::fs::remove_file(&part).with_context(|| format!("removing {}", part.display()))?;
            return Ok(false);
        }
        let path = directory.join(&range.file);
        std::fs::rename(&part, &path).with_context(|| format!("writing {}", path.display()))?;
        files.push(FileRecord {
            name: range.file.clone(),
            url,
            last_modified: downloaded.last_modified,
            bytes: downloaded.bytes,
        });
        paths.push(path);
        Ok(true)
    };
    let full = |range: &Range| {
        anyhow::anyhow!(
            "the answer for {} to {} holds {CAP} filings, the most one answer can, so some may be missing; this build asks for nothing narrower",
            range.from,
            range.to
        )
    };

    let before = Range::before();
    if !ask(&before)? {
        return Err(full(&before));
    }
    for year in FIRST_YEAR..=last_year {
        if ask(&Range::year(year))? {
            continue;
        }
        for month in 1..=12 {
            let range = Range::month(year, month);
            if !ask(&range)? {
                return Err(full(&range));
            }
        }
    }

    let record = SourceRecord {
        source: SOURCE_ID.to_owned(),
        fetched_at: fetched_at.to_owned(),
        // The index has no date of its own: it is as of the day it was read.
        as_of: fetched_at.chars().take(10).collect(),
        files,
    };
    let record_path = directory.join("source.json");
    let mut text = serde_json::to_string_pretty(&record)?;
    text.push('\n');
    std::fs::write(&record_path, text)
        .with_context(|| format!("writing {}", record_path.display()))?;
    paths.push(record_path);
    Ok(paths)
}

/// Fetches the whole index from the network.
pub fn fetch(build_dir: &Path, fetched_at: &str) -> Result<Vec<PathBuf>> {
    fetch_with(build_dir, fetched_at, PAUSE, &mut download)
}

/// The form a manufacturer's name is matched in: its ASCII letters and
/// digits, lowercased. `FORD MOTOR COMPANY` and `Ford Motor Company`
/// are one manufacturer; `FORD MOTOR COMPANY OF CANADA, LTD.` is another.
pub fn manufacturer_form(name: &str) -> String {
    normalize(name)
}

const INSERT_GUIDE: &str = "
INSERT INTO vin_guide (name, manufacturer_id, manufacturer, kind, letter_date, year_from, year_to, url)
VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)";

/// Builds the `vin_guides` dataset from the fetched `nhtsa-vin-guides`
/// directory: one `vin_guide` row for each filing, and for each
/// manufacturer that filed, the manufacturer codes of the core data file
/// that carry its name and the catalog makes built under those codes.
pub fn shape(context: &mut ShapeContext<'_>, directory: &Path) -> Result<()> {
    let as_of = SourceRecord::read(directory)?.as_of;
    let mut rows: Vec<VinGuideRow> = Vec::new();
    parse(directory, &mut |row| {
        rows.push(row);
        Ok(())
    })?;
    // Oldest first, so that a higher id is a later letter.
    rows.sort_by(|left, right| {
        (&left.letter_date, &left.name).cmp(&(&right.letter_date, &right.name))
    });

    // The manufacturer codes of the core file, by the form of their
    // manufacturer's name, and the makes built under each code.
    let mut codes: BTreeMap<String, Vec<String>> = BTreeMap::new();
    {
        let mut statement = context
            .catalog
            .prepare("SELECT code, manufacturer FROM wmi ORDER BY code")?;
        let found = statement.query_map([], |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
        })?;
        for row in found {
            let (code, manufacturer) = row?;
            let form = manufacturer_form(&manufacturer);
            if !form.is_empty() {
                codes.entry(form).or_default().push(code);
            }
        }
    }
    let mut makes: BTreeMap<String, Vec<i64>> = BTreeMap::new();
    {
        let mut statement = context
            .catalog
            .prepare("SELECT wmi, make_id FROM wmi_make ORDER BY wmi, make_id")?;
        let found = statement.query_map([], |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, i64>(1)?))
        })?;
        for row in found {
            let (code, make_id) = row?;
            makes.entry(code).or_default().push(make_id);
        }
    }

    context.transaction.execute_batch(schema::VIN_GUIDES)?;
    let mut manufacturers: BTreeMap<i64, String> = BTreeMap::new();
    {
        let mut insert = context.transaction.prepare(INSERT_GUIDE)?;
        for row in &rows {
            insert
                .execute(rusqlite::params![
                    row.name,
                    row.manufacturer_id,
                    row.manufacturer,
                    KIND,
                    row.letter_date,
                    row.year_from,
                    row.year_to,
                    row.url,
                ])
                .with_context(|| format!("storing the filing {}", row.name))?;
            manufacturers
                .entry(row.manufacturer_id)
                .or_insert_with(|| manufacturer_form(&row.manufacturer));
        }
    }
    {
        let mut insert_code = context
            .transaction
            .prepare("INSERT INTO vin_guide_wmi (wmi, manufacturer_id) VALUES (?1, ?2)")?;
        let mut insert_make = context
            .transaction
            .prepare("INSERT INTO vin_guide_make (make_id, manufacturer_id) VALUES (?1, ?2)")?;
        for (manufacturer_id, form) in &manufacturers {
            let Some(found) = codes.get(form) else {
                continue;
            };
            let mut built: BTreeSet<i64> = BTreeSet::new();
            for code in found {
                insert_code.execute(rusqlite::params![code, manufacturer_id])?;
                built.extend(makes.get(code).into_iter().flatten());
            }
            for make_id in built {
                insert_make.execute(rusqlite::params![make_id, manufacturer_id])?;
            }
        }
    }
    context.register(
        Dataset::VinGuides,
        DatasetEntry {
            as_of,
            rows: rows.len() as u64,
            licence: licence::PUBLIC_DOMAIN.to_owned(),
            attribution: String::new(),
        },
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_range_is_a_span_of_dates_a_file_and_an_address() {
        let before = Range::before();
        assert_eq!(
            (
                before.from.as_str(),
                before.to.as_str(),
                before.file.as_str()
            ),
            ("1/1/1900", "12/31/1984", "565-1900-1984.json")
        );
        let year = Range::year(2026);
        assert_eq!(
            (year.from.as_str(), year.to.as_str(), year.file.as_str()),
            ("1/1/2026", "12/31/2026", "565-2026.json")
        );
        assert_eq!(
            year.url(),
            "https://vpic.nhtsa.dot.gov/api/vehicles/GetParts?type=565&fromDate=1/1/2026&toDate=12/31/2026&format=json"
        );
        let month = Range::month(2024, 2);
        assert_eq!(
            (month.from.as_str(), month.to.as_str(), month.file.as_str()),
            ("2/1/2024", "2/29/2024", "565-2024-02.json")
        );
        assert_eq!(Range::month(2025, 2).to, "2/28/2025");
        assert_eq!(Range::month(2025, 12).to, "12/31/2025");
        assert_eq!(Range::month(2025, 9).to, "9/30/2025");
        assert_eq!(days_in_month(2000, 2), 29);
        assert_eq!(days_in_month(2100, 2), 28);
    }

    #[test]
    fn a_letter_date_becomes_iso_or_nothing() {
        assert_eq!(iso_date("6/1/2026").as_deref(), Some("2026-06-01"));
        assert_eq!(iso_date(" 12/31/1984 ").as_deref(), Some("1984-12-31"));
        assert_eq!(iso_date("2/29/2024").as_deref(), Some("2024-02-29"));
        for bad in [
            "",
            "2026-06-01",
            "13/1/2026",
            "2/30/2026",
            "0/1/2026",
            "6/1/26",
            "6/1/2026/7",
            "6/x/2026",
        ] {
            assert_eq!(iso_date(bad), None, "{bad}");
        }
    }

    #[test]
    fn only_nhtsas_copy_is_kept_as_an_address_and_as_https() {
        assert_eq!(
            document_url(
                "http://vpic.nhtsa.dot.gov/mid/home/displayfile/29dfd65e-a57c-4da0-ac6e-78acd3d30f8b"
            )
            .as_deref(),
            Some(
                "https://vpic.nhtsa.dot.gov/mid/home/displayfile/29dfd65e-a57c-4da0-ac6e-78acd3d30f8b"
            )
        );
        assert!(
            document_url(
                "https://vpic.nhtsa.dot.gov/mid/home/displayfile/29dfd65e-a57c-4da0-ac6e-78acd3d30f8b"
            )
            .is_some()
        );
        for bad in [
            "",
            "ftp://vpic.nhtsa.dot.gov/mid/home/displayfile/29dfd65e-a57c-4da0-ac6e-78acd3d30f8b",
            "http://example.com/mid/home/displayfile/29dfd65e-a57c-4da0-ac6e-78acd3d30f8b",
            "http://vpic.nhtsa.dot.gov/mid/home/displayfile/x",
            "https://vpic.nhtsa.dot.gov/mfrportal/home/coverletter/30f737e1-7478-4f2d-96ea-1b0bbcfecd8c",
        ] {
            assert_eq!(document_url(bad), None, "{bad}");
        }
    }

    fn answer(results: &str, count: usize) -> String {
        format!(
            r#"{{"Count":{count},"Message":"Results returned successfully","SearchCriteria":"Type: 565","Results":[{results}]}}"#
        )
    }

    const FILING: &str = r#"{"CoverLetterURL":"","LetterDate":"10/9/1984","ManufacturerId":976,"ManufacturerName":"FORD MOTOR COMPANY ","ModelYearFrom":null,"ModelYearTo":1986,"Name":" 01-22-N11B-1670 ","Type":"565","URL":"http://vpic.nhtsa.dot.gov/mid/home/displayfile/aaa4a40c-9cf6-4cb2-b95a-e3478ffe84e0"}"#;

    fn rows(text: &str) -> Result<Vec<VinGuideRow>> {
        let mut found = Vec::new();
        parse_answer(text.as_bytes(), "test.json", &mut |row| {
            found.push(row);
            Ok(())
        })?;
        Ok(found)
    }

    #[test]
    fn an_answer_is_read_filing_by_filing() {
        assert_eq!(
            rows(&answer(FILING, 1)).unwrap(),
            [VinGuideRow {
                name: "01-22-N11B-1670".to_owned(),
                manufacturer_id: 976,
                manufacturer: "FORD MOTOR COMPANY".to_owned(),
                letter_date: "1984-10-09".to_owned(),
                year_from: None,
                year_to: Some(1986),
                url: "https://vpic.nhtsa.dot.gov/mid/home/displayfile/aaa4a40c-9cf6-4cb2-b95a-e3478ffe84e0"
                    .to_owned(),
            }]
        );
        assert!(rows(&answer("", 0)).unwrap().is_empty());
        // A model year that cannot be one is left out; the filing is kept.
        let odd = FILING.replace("1986", "19860");
        assert_eq!(rows(&answer(&odd, 1)).unwrap()[0].year_to, None);
    }

    #[test]
    fn an_answer_this_build_cannot_read_is_refused() {
        let message = |text: &str| format!("{:#}", rows(text).unwrap_err());
        assert!(
            message("<html>busy</html>")
                .starts_with("test.json is not an answer of GetParts as this build reads it")
        );
        // A field added, and a field gone.
        let added = FILING.replace(r#""Type":"565""#, r#""Type":"565","Pages":3"#);
        assert!(message(&answer(&added, 1)).contains("unknown field `Pages`"));
        let gone = FILING.replace(r#""Name":" 01-22-N11B-1670 ","#, "");
        assert!(message(&answer(&gone, 1)).contains("missing field `Name`"));
        assert_eq!(
            message(&answer(FILING, 2)),
            "test.json says it holds 2 filings and holds 1"
        );
        assert_eq!(
            message(&answer(&FILING.replace(r#""565""#, r#""566""#), 1)),
            "test.json: 01-22-N11B-1670 is of type \"566\", expected 565"
        );
        assert_eq!(
            message(&answer(&FILING.replace("10/9/1984", "1984-10-09"), 1)),
            "test.json: 01-22-N11B-1670 has the letter date \"1984-10-09\""
        );
        assert!(
            message(&answer(
                &FILING.replace("vpic.nhtsa.dot.gov", "example.com"),
                1
            ))
            .ends_with("which is not under https://vpic.nhtsa.dot.gov/mid/home/displayfile/")
        );
        // A full answer may have left filings out.
        let full = vec![FILING; CAP].join(",");
        assert!(message(&answer(&full, CAP)).starts_with(
            "test.json holds 1000 filings, the most one answer can: some may be missing."
        ));
    }

    #[test]
    fn a_manufacturers_name_is_matched_by_its_letters_and_digits() {
        assert_eq!(manufacturer_form("FORD MOTOR COMPANY"), "fordmotorcompany");
        assert_eq!(
            manufacturer_form("American Honda Motor Co., Inc. "),
            manufacturer_form("AMERICAN HONDA MOTOR CO., INC.")
        );
        assert_ne!(
            manufacturer_form("FORD MOTOR COMPANY"),
            manufacturer_form("FORD MOTOR COMPANY OF CANADA, LTD.")
        );
    }
}
