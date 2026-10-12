//! NHTSA's defect investigations flat file, read row by row, and the
//! `investigations` dataset built from it.
//!
//! The archive is read as `open-data fetch nhtsa-investigations` saved it,
//! without unpacking it to disk: it holds one text file of 390 MB, because
//! every row repeats its investigation's whole summary. One row is one
//! investigation, make, model, model year and component; 11 fields
//! separated by tabs, no header, lines ended by LF, UTF-8.

use std::collections::{BTreeMap, BTreeSet};
use std::fs::File;
use std::io::{BufRead, BufReader};
use std::path::Path;

use anyhow::{Context, Result, bail};
use rusqlite::params;
use wenmar_shopdata::investigations::{OPEN_FROM, kind_of};
use wenmar_shopdata::{Dataset, DatasetEntry, licence, schema};

use crate::shop::build::ShapeContext;
use crate::shop::matcher::NO_YEAR;
use crate::shop::nhtsa_recalls::{given, iso_date, tidy};
use crate::sources::SourceRecord;

/// The archive, and the one text file it holds.
pub const FILE: &str = "FLAT_INV.zip";

/// NHTSA's description of the fields, fetched beside the archive.
pub const LAYOUT_FILE: &str = "INV.txt";

/// The 11 fields of a row, in order, as `INV.txt` names them.
pub const FIELDS: [&str; 11] = [
    "NHTSA ACTION NUMBER",
    "MAKE",
    "MODEL",
    "YEAR",
    "COMPNAME",
    "MFR_NAME",
    "ODATE",
    "CDATE",
    "CAMPNO",
    "SUBJECT",
    "SUMMARY",
];

/// What the file writes where an investigation has no summary.
pub const NO_SUMMARY: [&str; 2] = ["There is no summary currently available", "==="];

/// What separates the manufacturers and the components of an investigation
/// that names several.
pub const SEPARATOR: &str = "; ";

/// One row: one investigation and one vehicle and component it lists. What
/// an investigation says of itself (its dates, subject and summary) is the
/// same on every one of its rows.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InvestigationRow {
    /// Field 1, the action number, such as `PE24009`.
    pub action: String,
    /// Fields 2 and 3, `MAKE` and `MODEL`. `None` on the rows of an
    /// investigation the file lists no vehicle for.
    pub make_text: Option<String>,
    pub model_text: Option<String>,
    /// Field 4, `YEAR`. `None` for `9999` and for an empty field.
    pub year: Option<i32>,
    /// Field 5, `COMPNAME`.
    pub component: Option<String>,
    /// Field 6, `MFR_NAME`.
    pub manufacturer: Option<String>,
    /// Fields 7 and 8, `ODATE` and `CDATE`: the day opened and the day
    /// closed, as `2024-03-27`.
    pub opened: Option<String>,
    pub closed: Option<String>,
    /// Field 9, `CAMPNO`: the recall campaign the investigation led to, as
    /// the recall files write it. The file writes some without their last
    /// three digits (`24V957` for `24V957000`); they are put back.
    pub campaign: Option<String>,
    /// Field 10, `SUBJECT`. `None` when it is only the kind's letters.
    pub subject: Option<String>,
    /// Field 11, `SUMMARY`. `None` for the placeholders in [`NO_SUMMARY`].
    pub summary: Option<String>,
}

/// What `parse` read.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct InvestigationCounts {
    /// Lines with 11 fields.
    pub records: u64,
    /// Lines that are empty but for the line end.
    pub junk: u64,
}

/// The field names a copy of `INV.txt` lists, in order. A name may be
/// several words: it ends where the type, `CHAR(` or `NUMBER(`, begins.
pub fn layout(inv_txt: &str) -> Vec<String> {
    inv_txt
        .lines()
        .filter_map(|line| {
            let mut words = line.split_whitespace();
            let number: usize = words.next()?.parse().ok()?;
            let mut name = Vec::new();
            let mut typed = false;
            for word in words {
                if word.starts_with("CHAR(") || word.starts_with("NUMBER(") {
                    typed = true;
                    break;
                }
                name.push(word);
            }
            (number >= 1 && typed && !name.is_empty()).then(|| name.join(" "))
        })
        .collect()
}

/// Refuses a copy of `INV.txt` whose fields are not [`FIELDS`].
pub fn check_layout(inv_txt: &str) -> Result<()> {
    let found = layout(inv_txt);
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

/// A campaign number as the recall files write it: nine characters, in
/// capitals. `24V957` becomes `24V957000`. `None` for anything that is not
/// two digits, a letter and three or six digits.
pub fn campaign_number(text: &str) -> Option<String> {
    let text = text.trim().to_ascii_uppercase();
    let bytes = text.as_bytes();
    let shaped = matches!(bytes.len(), 6 | 9)
        && bytes.iter().enumerate().all(|(index, byte)| match index {
            2 => byte.is_ascii_uppercase(),
            _ => byte.is_ascii_digit(),
        });
    if !shaped {
        return None;
    }
    Some(if text.len() == 6 {
        format!("{text}000")
    } else {
        text
    })
}

/// Reads the 11 fields of one row.
fn read_row(fields: &[&str]) -> Result<InvestigationRow> {
    let action = tidy(fields[0])
        .map(|action| action.to_ascii_uppercase())
        .context("NHTSA ACTION NUMBER is empty")?;
    let year = match fields[3].trim().parse::<i32>() {
        Ok(year) if year != NO_YEAR && (1900..=2100).contains(&year) => Some(year),
        _ => None,
    };
    let kind = kind_of(&action);
    Ok(InvestigationRow {
        make_text: tidy(fields[1]),
        model_text: tidy(fields[2]),
        year,
        component: given(fields[4]),
        manufacturer: tidy(fields[5]),
        opened: iso_date(fields[6]),
        closed: iso_date(fields[7]),
        campaign: campaign_number(fields[8]),
        subject: tidy(fields[9]).filter(|subject| *subject != kind),
        summary: tidy(fields[10]).filter(|summary| !NO_SUMMARY.contains(&summary.as_str())),
        action,
    })
}

/// Reads the file's text, a line at a time, and hands each row to `row`.
/// `origin` names the file in an error.
pub fn parse_reader(
    mut reader: impl BufRead,
    origin: &str,
    counts: &mut InvestigationCounts,
    row: &mut dyn FnMut(InvestigationRow) -> Result<()>,
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
        match fields.len() {
            1 if fields[0].trim().is_empty() => {
                counts.junk += 1;
                continue;
            }
            11 => {}
            other => bail!(
                "{origin} line {number} has {other} fields, expected {}",
                FIELDS.len()
            ),
        }
        counts.records += 1;
        row(read_row(&fields).with_context(|| format!("{origin} line {number}"))?)?;
    }
}

/// Reads the fetched `nhtsa-investigations` directory: checks `INV.txt`,
/// then hands every row of the archive to `row`, in file order.
pub fn parse(
    directory: &Path,
    row: &mut dyn FnMut(InvestigationRow) -> Result<()>,
) -> Result<InvestigationCounts> {
    let layout_path = directory.join(LAYOUT_FILE);
    let layout_bytes = std::fs::read(&layout_path).with_context(|| {
        format!(
            "reading {}; run open-data fetch nhtsa-investigations first",
            layout_path.display()
        )
    })?;
    check_layout(&String::from_utf8_lossy(&layout_bytes))
        .with_context(|| layout_path.display().to_string())?;

    let path = directory.join(FILE);
    let file = File::open(&path).with_context(|| {
        format!(
            "opening {}; run open-data fetch nhtsa-investigations first",
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
    let mut counts = InvestigationCounts::default();
    parse_reader(BufReader::new(entry), FILE, &mut counts, row)?;
    Ok(counts)
}

/// An investigation gathered from its rows.
struct Gathered {
    /// The first row. What an investigation says of itself is the same on
    /// every one of its rows.
    first: InvestigationRow,
    /// Every manufacturer and component its rows name, each once, in file
    /// order.
    manufacturers: Vec<String>,
    components: Vec<String>,
    /// Each make, model and year it lists, once.
    vehicles: BTreeSet<(String, String, Option<i32>)>,
    /// Each campaign it led to, once.
    campaigns: BTreeSet<String>,
}

fn add_once(list: &mut Vec<String>, value: Option<String>) {
    if let Some(value) = value
        && !list.contains(&value)
    {
        list.push(value);
    }
}

/// Whether an investigation counts as open: it has a day opened on or
/// after [`OPEN_FROM`] and no day closed.
pub fn is_open(opened: Option<&str>, closed: Option<&str>) -> bool {
    closed.is_none() && opened.is_some_and(|opened| opened >= OPEN_FROM)
}

const INSERT_INVESTIGATION: &str = "
INSERT INTO investigation (
    action, kind, opened, closed, is_open, manufacturer, component, subject, summary
) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)";

const INSERT_VEHICLE: &str = "
INSERT INTO investigation_vehicle (
    investigation_id, year, make_id, model_id, make_text, model_text, \"match\"
) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)";

const INSERT_CAMPAIGN: &str =
    "INSERT INTO investigation_campaign (investigation_id, campaign) VALUES (?1, ?2)";

/// Builds the `investigations` dataset from the fetched
/// `nhtsa-investigations` directory: one `investigation` row for each
/// action number, one `investigation_vehicle` row for each make, model and
/// year it lists, and one `investigation_campaign` row for each recall
/// campaign it led to.
pub fn shape(context: &mut ShapeContext<'_>, directory: &Path) -> Result<()> {
    let as_of = SourceRecord::read(directory)?.as_of;
    let mut gathered: BTreeMap<String, Gathered> = BTreeMap::new();
    parse(directory, &mut |mut row| {
        let (make_text, model_text) = (row.make_text.take(), row.model_text.take());
        let (manufacturer, component) = (row.manufacturer.take(), row.component.take());
        let campaign = row.campaign.take();
        let year = row.year;
        let found = match gathered.get_mut(&row.action) {
            Some(found) => found,
            None => gathered
                .entry(row.action.clone())
                .or_insert_with(|| Gathered {
                    first: row,
                    manufacturers: Vec::new(),
                    components: Vec::new(),
                    vehicles: BTreeSet::new(),
                    campaigns: BTreeSet::new(),
                }),
        };
        add_once(&mut found.manufacturers, manufacturer);
        add_once(&mut found.components, component);
        if let (Some(make_text), Some(model_text)) = (make_text, model_text) {
            found.vehicles.insert((make_text, model_text, year));
        }
        if let Some(campaign) = campaign {
            found.campaigns.insert(campaign);
        }
        Ok(())
    })?;

    let transaction = context.transaction;
    transaction.execute_batch(schema::INVESTIGATIONS)?;
    for (action, found) in &gathered {
        let first = &found.first;
        let manufacturer = found.manufacturers.join(SEPARATOR);
        let component = found.components.join(SEPARATOR);
        transaction
            .prepare_cached(INSERT_INVESTIGATION)?
            .execute(params![
                action,
                kind_of(action),
                first.opened,
                first.closed,
                is_open(first.opened.as_deref(), first.closed.as_deref()),
                (!manufacturer.is_empty()).then_some(manufacturer.as_str()),
                (!component.is_empty()).then_some(component.as_str()),
                first.subject,
                first.summary,
            ])
            .with_context(|| format!("storing investigation {action}"))?;
        let id = transaction.last_insert_rowid();
        for (make_text, model_text, year) in &found.vehicles {
            let resolved = context.matcher.resolve(*year, make_text, model_text);
            let models: Vec<Option<i64>> = if resolved.model_ids.is_empty() {
                vec![None]
            } else {
                resolved.model_ids.iter().copied().map(Some).collect()
            };
            let mut statement = transaction.prepare_cached(INSERT_VEHICLE)?;
            for model_id in models {
                statement.execute(params![
                    id,
                    year,
                    resolved.make_id,
                    model_id,
                    make_text,
                    model_text,
                    model_id.and(resolved.match_text()),
                ])?;
            }
        }
        for campaign in &found.campaigns {
            transaction
                .prepare_cached(INSERT_CAMPAIGN)?
                .execute(params![id, campaign])?;
        }
    }
    context.register(
        Dataset::Investigations,
        DatasetEntry {
            as_of,
            rows: gathered.len() as u64,
            licence: licence::PUBLIC_DOMAIN.to_owned(),
            attribution: String::new(),
        },
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn line(change: impl FnOnce(&mut Vec<String>)) -> String {
        let mut fields: Vec<String> = vec![String::new(); 11];
        for (index, value) in [
            (0, "PE24009"),
            (1, "RAM"),
            (2, "3500"),
            (3, "2022"),
            (4, "POWER TRAIN"),
            (5, "Chrysler (FCA US, LLC)"),
            (6, "20240327"),
            (9, "Snap Ring Failure"),
            (10, "The ring may fail."),
        ] {
            fields[index] = value.to_owned();
        }
        change(&mut fields);
        fields.join("\t")
    }

    fn rows(text: &str) -> Result<(Vec<InvestigationRow>, InvestigationCounts)> {
        let mut found = Vec::new();
        let mut counts = InvestigationCounts::default();
        parse_reader(text.as_bytes(), "test.txt", &mut counts, &mut |row| {
            found.push(row);
            Ok(())
        })?;
        Ok((found, counts))
    }

    #[test]
    fn a_campaign_number_is_written_as_the_recall_files_write_it() {
        assert_eq!(campaign_number("19V888000").as_deref(), Some("19V888000"));
        assert_eq!(campaign_number("24V957").as_deref(), Some("24V957000"));
        assert_eq!(campaign_number(" 17e002 ").as_deref(), Some("17E002000"));
        for bad in [
            "",
            "24V95",
            "24V9570",
            "2AV957",
            "24V95700A",
            "N/A",
            "24-957",
        ] {
            assert_eq!(campaign_number(bad), None, "{bad}");
        }
    }

    #[test]
    fn open_means_opened_since_2010_and_not_closed() {
        assert!(is_open(Some("2024-03-27"), None));
        assert!(is_open(Some("2010-01-01"), None));
        assert!(!is_open(Some("2009-12-31"), None));
        assert!(!is_open(Some("1998-11-13"), None));
        assert!(!is_open(Some("2024-03-27"), Some("2025-01-02")));
        assert!(!is_open(None, None));
    }

    #[test]
    fn a_line_is_a_row_an_empty_line_or_an_error() {
        let text = format!(
            "{}\n\n{}\r\n{}",
            line(|_| {}),
            line(|fields| {
                fields[3] = "9999".to_owned();
                fields[4] = "NONE".to_owned();
                fields[7] = "20250102".to_owned();
                fields[8] = "24V957".to_owned();
                fields[10] = "There is no summary currently available".to_owned();
            }),
            // A row with no vehicle, whose subject is only the kind. The
            // file's last line has no line end.
            line(|fields| {
                for field in &mut fields[1..=7] {
                    field.clear();
                }
                fields[0] = "ID92004".to_owned();
                fields[8] = "92V184000".to_owned();
                fields[9] = "ID".to_owned();
                fields[10] = "===".to_owned();
            }),
        );
        let (found, counts) = rows(&text).unwrap();
        assert_eq!(
            counts,
            InvestigationCounts {
                records: 3,
                junk: 1
            }
        );
        assert_eq!(found[0].action, "PE24009");
        assert_eq!(found[0].make_text.as_deref(), Some("RAM"));
        assert_eq!(found[0].year, Some(2022));
        assert_eq!(found[0].opened.as_deref(), Some("2024-03-27"));
        assert_eq!(found[0].closed, None);
        assert_eq!(found[0].campaign, None);
        assert_eq!(found[0].summary.as_deref(), Some("The ring may fail."));
        assert_eq!(found[1].year, None);
        assert_eq!(found[1].component, None);
        assert_eq!(found[1].closed.as_deref(), Some("2025-01-02"));
        assert_eq!(found[1].campaign.as_deref(), Some("24V957000"));
        assert_eq!(found[1].summary, None);
        assert_eq!(found[2].action, "ID92004");
        assert_eq!(found[2].make_text, None);
        assert_eq!(found[2].model_text, None);
        assert_eq!(found[2].subject, None);
        assert_eq!(found[2].summary, None);
        assert_eq!(found[2].campaign.as_deref(), Some("92V184000"));

        let short = format!("{}\n1\t2\t3\n", line(|_| {}));
        assert_eq!(
            format!("{:#}", rows(&short).unwrap_err()),
            "test.txt line 2 has 3 fields, expected 11"
        );
        let no_number = line(|fields| fields[0] = " ".to_owned());
        assert_eq!(
            format!("{:#}", rows(&no_number).unwrap_err()),
            "test.txt line 1: NHTSA ACTION NUMBER is empty"
        );
        // Bytes that are not UTF-8 do not stop the build.
        let mut bytes = line(|_| {}).into_bytes();
        bytes.extend_from_slice(b"\xff\n");
        let mut counts = InvestigationCounts::default();
        parse_reader(&bytes[..], "test.txt", &mut counts, &mut |_| Ok(())).unwrap();
        assert_eq!(counts.records, 1);
    }

    #[test]
    fn the_layout_is_read_from_inv_txt() {
        let text = "DEFECT INVESTIGATIONS\r\n\r\nChange log:\r\n1.\tSeptember 14, 2007\r\n2.\tMarch 14, 2008\r\n\t-Field #11 added\r\n\r\nField#   Name                \tType/Size   \tDescription\r\n------   ----------             ---------       -----\r\n1        NHTSA ACTION NUMBER    CHAR(10)        NHTSA Identification Number\r\n4        YEAR                   CHAR(4)         Model Year, \r\n                                                   9999 if Unknown or N/A\r\n11       SUMMARY                CHAR(6000)      Summary Detail";
        assert_eq!(layout(text), ["NHTSA ACTION NUMBER", "YEAR", "SUMMARY"]);
        let error = check_layout(text).unwrap_err().to_string();
        assert!(
            error.starts_with("the fields are not the ones this build reads: found 3 (NHTSA ACTION NUMBER, YEAR, SUMMARY), expected 11 ("),
            "{error}"
        );
    }
}
