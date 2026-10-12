//! NHTSA's recall flat files, read row by row. Nothing is matched to the
//! catalog here: `parse` only says what each row of the files holds.
//!
//! The two archives are read as `open-data fetch nhtsa-recalls` saved them,
//! without unpacking them to disk. Each holds one text file: one record
//! per line, 29 fields separated by tabs, no header, lines ended by CR LF.
//! The text is UTF-8. Some of it was typed in Windows-1252 and converted
//! as if it were Latin-1, so a curly quote arrives as a control character
//! (U+0092 for a right single quote); [`tidy`] puts the quote back.

use std::collections::BTreeMap;
use std::fs::File;
use std::io::{BufRead, BufReader};
use std::path::Path;

use anyhow::{Context, Result, bail};
use wenmar_shopdata::{Dataset, DatasetEntry, licence};

use crate::shop::build::ShapeContext;
use crate::shop::matcher::NO_YEAR;
use crate::shop::recalls::{self, RecallRecord, VehicleLine};
use crate::sources::SourceRecord;

/// The archives, in the order they are read.
pub const FILES: [&str; 2] = ["FLAT_RCL_PRE_2010.zip", "FLAT_RCL_POST_2010.zip"];

/// NHTSA's description of the fields, fetched beside the archives.
pub const LAYOUT_FILE: &str = "RCL.txt";

/// The 29 fields of a record, in order, as `RCL.txt` names them.
/// `CONEQUENCE_DEFECT` is NHTSA's spelling.
pub const FIELDS: [&str; 29] = [
    "RECORD_ID",
    "CAMPNO",
    "MAKETXT",
    "MODELTXT",
    "YEARTXT",
    "MFGCAMPNO",
    "COMPNAME",
    "MFGNAME",
    "BGMAN",
    "ENDMAN",
    "RCLTYPECD",
    "POTAFF",
    "ODATE",
    "INFLUENCED_BY",
    "MFGTXT",
    "RCDATE",
    "DATEA",
    "RPNO",
    "FMVSS",
    "DESC_DEFECT",
    "CONEQUENCE_DEFECT",
    "CORRECTIVE_ACTION",
    "NOTES",
    "RCL_CMPT_ID",
    "MFR_COMP_NAME",
    "MFR_COMP_DESC",
    "MFR_COMP_PTNO",
    "DO_NOT_DRIVE",
    "PARK_OUTSIDE",
];

/// The value of `RCLTYPECD` that marks a vehicle record. The others are
/// `T` (tires), `E` (equipment), `C` (child restraints), `I` and `X`.
pub const VEHICLE: &str = "V";

/// One vehicle record: one campaign, make, model and model year. A campaign
/// has one record for each vehicle it lists, and often several for the
/// same vehicle, one per component.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NhtsaRecallRow {
    /// Field 1, `RECORD_ID`.
    pub record_id: u32,
    /// Field 2, `CAMPNO`, such as `19V888000`.
    pub campaign: String,
    /// Field 3, `MAKETXT`.
    pub make_text: String,
    /// Field 4, `MODELTXT`.
    pub model_text: String,
    /// Field 5, `YEARTXT`. `None` for `9999`, NHTSA's "unknown or not
    /// applicable".
    pub year: Option<i32>,
    /// Field 6, `MFGCAMPNO`: the manufacturer's own campaign number.
    pub mfr_campaign: Option<String>,
    /// Field 7, `COMPNAME`, such as `AIR BAGS:FRONTAL:DRIVER SIDE`.
    pub component: Option<String>,
    /// Field 8, `MFGNAME`: the manufacturer that filed the report.
    pub manufacturer: Option<String>,
    /// Fields 9 and 10, `BGMAN` and `ENDMAN`: the first and last build
    /// date, as `2018-07-11`.
    pub built_from: Option<String>,
    pub built_to: Option<String>,
    /// Field 12, `POTAFF`: units potentially affected. `None` for 0.
    pub units: Option<u32>,
    /// Field 13, `ODATE`: the day owners were notified.
    pub owner_notified: Option<String>,
    /// Field 16, `RCDATE`: the day NHTSA received the report.
    pub report_date: Option<String>,
    /// Fields 20 to 23: `DESC_DEFECT`, `CONEQUENCE_DEFECT`,
    /// `CORRECTIVE_ACTION` and `NOTES`.
    pub summary: Option<String>,
    pub consequence: Option<String>,
    pub remedy: Option<String>,
    pub notes: Option<String>,
    /// Field 27, `MFR_COMP_PTNO`: the manufacturer's part number.
    pub part_number: Option<String>,
    /// Fields 28 and 29, `DO_NOT_DRIVE` and `PARK_OUTSIDE`: `Yes` or `No`.
    pub do_not_drive: bool,
    pub park_outside: bool,
}

/// What `parse` read.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct NhtsaCounts {
    /// Lines with 29 fields.
    pub records: u64,
    /// Of those, the vehicle records handed on.
    pub vehicles: u64,
    /// Of those, the tire, equipment and child restraint records left out.
    pub other_types: u64,
    /// Lines with a single field, which the files hold a few of.
    pub junk: u64,
}

/// The field names a copy of `RCL.txt` lists, in order.
pub fn layout(rcl_txt: &str) -> Vec<String> {
    rcl_txt
        .lines()
        .filter_map(|line| {
            let mut words = line.split_whitespace();
            let number: usize = words.next()?.parse().ok()?;
            let name = words.next()?;
            let is_name = name
                .chars()
                .all(|character| character.is_ascii_uppercase() || character == '_');
            (number >= 1 && is_name).then(|| name.to_owned())
        })
        .collect()
}

/// Refuses a copy of `RCL.txt` whose fields are not [`FIELDS`].
pub fn check_layout(rcl_txt: &str) -> Result<()> {
    let found = layout(rcl_txt);
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

/// A date written `20180711`, as `2018-07-11`. `None` for an empty field
/// and for the placeholders the files hold, such as `19000101`,
/// `11111111` and years that cannot be right.
pub fn iso_date(yyyymmdd: &str) -> Option<String> {
    let text = yyyymmdd.trim();
    if text.len() != 8 || !text.bytes().all(|byte| byte.is_ascii_digit()) {
        return None;
    }
    let year: u32 = text[0..4].parse().ok()?;
    let month: u32 = text[4..6].parse().ok()?;
    let day: u32 = text[6..8].parse().ok()?;
    ((1949..=2100).contains(&year) && (1..=12).contains(&month) && (1..=31).contains(&day))
        .then(|| format!("{year:04}-{month:02}-{day:02}"))
}

/// A text field with its ends trimmed and its mis-converted punctuation put
/// right. `None` when nothing is left.
pub fn tidy(text: &str) -> Option<String> {
    let mut tidy = String::with_capacity(text.len());
    for character in text.trim().chars() {
        match character {
            '\u{91}' => tidy.push('\u{2018}'),
            '\u{92}' => tidy.push('\u{2019}'),
            '\u{93}' => tidy.push('\u{201c}'),
            '\u{94}' => tidy.push('\u{201d}'),
            '\u{95}' => tidy.push('\u{2022}'),
            '\u{96}' => tidy.push('\u{2013}'),
            '\u{97}' => tidy.push('\u{2014}'),
            '\u{a0}' | '\u{3000}' => tidy.push(' '),
            other if other.is_control() => {}
            other => tidy.push(other),
        }
    }
    let tidy = tidy.trim();
    (!tidy.is_empty()).then(|| tidy.to_owned())
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

fn flag(text: &str) -> bool {
    text.trim().eq_ignore_ascii_case("yes")
}

fn required(fields: &[&str], index: usize) -> Result<String> {
    tidy(fields[index]).with_context(|| format!("{} is empty", FIELDS[index]))
}

/// Reads the 29 fields of one vehicle record.
fn read_row(fields: &[&str]) -> Result<NhtsaRecallRow> {
    let record_id = fields[0]
        .trim()
        .parse()
        .with_context(|| format!("RECORD_ID {:?} is not a number", fields[0]))?;
    let year = match fields[4].trim().parse::<i32>() {
        Ok(year) if year != NO_YEAR && (1900..=2100).contains(&year) => Some(year),
        _ => None,
    };
    Ok(NhtsaRecallRow {
        record_id,
        campaign: required(fields, 1)?,
        make_text: required(fields, 2)?,
        model_text: required(fields, 3)?,
        year,
        mfr_campaign: given(fields[5]),
        component: tidy(fields[6]),
        manufacturer: tidy(fields[7]),
        built_from: iso_date(fields[8]),
        built_to: iso_date(fields[9]),
        units: fields[11].trim().parse().ok().filter(|units| *units > 0),
        owner_notified: iso_date(fields[12]),
        report_date: iso_date(fields[15]),
        summary: tidy(fields[19]),
        consequence: tidy(fields[20]),
        remedy: tidy(fields[21]),
        notes: tidy(fields[22]),
        part_number: given(fields[26]),
        do_not_drive: flag(fields[27]),
        park_outside: flag(fields[28]),
    })
}

/// Reads one file's text, a line at a time, and hands each vehicle record
/// to `row`. `origin` names the file in an error.
pub fn parse_reader(
    mut reader: impl BufRead,
    origin: &str,
    counts: &mut NhtsaCounts,
    row: &mut dyn FnMut(NhtsaRecallRow) -> Result<()>,
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
            // The files hold a few lines that are empty but for the line end.
            1 => {
                counts.junk += 1;
                continue;
            }
            29 => {}
            other => bail!(
                "{origin} line {number} has {other} fields, expected {}",
                FIELDS.len()
            ),
        }
        counts.records += 1;
        if fields[10].trim() != VEHICLE {
            counts.other_types += 1;
            continue;
        }
        counts.vehicles += 1;
        row(read_row(&fields).with_context(|| format!("{origin} line {number}"))?)?;
    }
}

/// Reads the fetched `nhtsa-recalls` directory: checks `RCL.txt`, then
/// hands every vehicle record of both archives to `row`, in file order.
pub fn parse(
    directory: &Path,
    row: &mut dyn FnMut(NhtsaRecallRow) -> Result<()>,
) -> Result<NhtsaCounts> {
    let layout_path = directory.join(LAYOUT_FILE);
    let layout_bytes = std::fs::read(&layout_path).with_context(|| {
        format!(
            "reading {}; run open-data fetch nhtsa-recalls first",
            layout_path.display()
        )
    })?;
    check_layout(&String::from_utf8_lossy(&layout_bytes))
        .with_context(|| layout_path.display().to_string())?;

    let mut counts = NhtsaCounts::default();
    for name in FILES {
        let path = directory.join(name);
        let file = File::open(&path).with_context(|| {
            format!(
                "opening {}; run open-data fetch nhtsa-recalls first",
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
        parse_reader(BufReader::new(entry), name, &mut counts, row)?;
    }
    Ok(counts)
}

/// The `source` column of a campaign from these files.
pub const SOURCE: &str = "nhtsa";

/// What separates the components and the part numbers of a campaign that
/// has several.
pub const COMPONENT_SEPARATOR: &str = "; ";
pub const PART_SEPARATOR: &str = ", ";

/// The first and last build date of one vehicle of a campaign.
type Built = (Option<String>, Option<String>);

/// A campaign gathered from its records.
struct Campaign {
    /// The first record. What a campaign says of itself is the same on
    /// every one of its records.
    first: NhtsaRecallRow,
    /// Every component and part number its records name, each once, in
    /// file order.
    components: Vec<String>,
    parts: Vec<String>,
    /// Each make, model and year it lists, once, with its build dates.
    vehicles: BTreeMap<(String, String, Option<i32>), Built>,
}

fn add_once(list: &mut Vec<String>, value: Option<&String>) {
    if let Some(value) = value
        && !list.contains(value)
    {
        list.push(value.clone());
    }
}

/// The earlier of two dates, or whichever there is.
fn earliest(left: Option<String>, right: Option<String>) -> Option<String> {
    match (left, right) {
        (Some(left), Some(right)) => Some(left.min(right)),
        (left, right) => left.or(right),
    }
}

/// The later of two dates, or whichever there is.
fn latest(left: Option<String>, right: Option<String>) -> Option<String> {
    match (left, right) {
        (Some(left), Some(right)) => Some(left.max(right)),
        (left, right) => left.or(right),
    }
}

/// Builds the `recalls_us` dataset from the fetched `nhtsa-recalls`
/// directory: one `recall` row for each vehicle campaign and one
/// `recall_vehicle` row for each make, model and year it lists.
pub fn shape(context: &mut ShapeContext<'_>, directory: &Path) -> Result<()> {
    let as_of = SourceRecord::read(directory)?.as_of;
    let mut campaigns: BTreeMap<String, Campaign> = BTreeMap::new();
    parse(directory, &mut |row| {
        let campaign = campaigns
            .entry(row.campaign.clone())
            .or_insert_with(|| Campaign {
                first: row.clone(),
                components: Vec::new(),
                parts: Vec::new(),
                vehicles: BTreeMap::new(),
            });
        add_once(&mut campaign.components, row.component.as_ref());
        add_once(&mut campaign.parts, row.part_number.as_ref());
        let built = campaign
            .vehicles
            .entry((row.make_text, row.model_text, row.year))
            .or_default();
        built.0 = earliest(built.0.take(), row.built_from);
        built.1 = latest(built.1.take(), row.built_to);
        Ok(())
    })?;

    recalls::create_tables(context.transaction)?;
    for (number, campaign) in &campaigns {
        let first = &campaign.first;
        let component = campaign.components.join(COMPONENT_SEPARATOR);
        let part_number = campaign.parts.join(PART_SEPARATOR);
        let recall_id = recalls::insert_recall(
            context.transaction,
            &RecallRecord {
                source: SOURCE,
                campaign: number,
                manufacturer: first.manufacturer.as_deref(),
                report_date: first.report_date.as_deref(),
                owner_notified: first.owner_notified.as_deref(),
                component: (!component.is_empty()).then_some(component.as_str()),
                summary_en: first.summary.as_deref(),
                consequence_en: first.consequence.as_deref(),
                remedy_en: first.remedy.as_deref(),
                notes_en: first.notes.as_deref(),
                mfr_campaign: first.mfr_campaign.as_deref(),
                part_number: (!part_number.is_empty()).then_some(part_number.as_str()),
                units: first.units,
                do_not_drive: Some(first.do_not_drive),
                park_outside: Some(first.park_outside),
                ..RecallRecord::default()
            },
        )
        .with_context(|| format!("storing campaign {number}"))?;
        for ((make_text, model_text, year), (built_from, built_to)) in &campaign.vehicles {
            recalls::insert_vehicle(
                context.transaction,
                context.matcher,
                recall_id,
                &VehicleLine {
                    year: *year,
                    make_text,
                    model_text,
                    built_from: built_from.as_deref(),
                    built_to: built_to.as_deref(),
                },
            )?;
        }
    }
    context.register(
        Dataset::RecallsUs,
        DatasetEntry {
            as_of,
            rows: campaigns.len() as u64,
            licence: licence::PUBLIC_DOMAIN.to_owned(),
            attribution: String::new(),
        },
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn line(change: impl FnOnce(&mut Vec<String>)) -> String {
        let mut fields: Vec<String> = vec![String::new(); 29];
        for (index, value) in [
            (0, "7"),
            (1, "20V001000"),
            (2, "HONDA"),
            (3, "CIVIC"),
            (4, "2019"),
            (10, "V"),
            (27, "No"),
            (28, "No"),
        ] {
            fields[index] = value.to_owned();
        }
        change(&mut fields);
        fields.join("\t")
    }

    fn rows(text: &str) -> Result<(Vec<NhtsaRecallRow>, NhtsaCounts)> {
        let mut found = Vec::new();
        let mut counts = NhtsaCounts::default();
        parse_reader(text.as_bytes(), "test.txt", &mut counts, &mut |row| {
            found.push(row);
            Ok(())
        })?;
        Ok((found, counts))
    }

    #[test]
    fn dates_become_iso_and_placeholders_become_nothing() {
        assert_eq!(iso_date("20180711").as_deref(), Some("2018-07-11"));
        assert_eq!(iso_date(" 19491231 ").as_deref(), Some("1949-12-31"));
        for bad in [
            "",
            "2018071",
            "201807111",
            "2018-07-11",
            "19000101",
            "11111111",
            "30170403",
            "20181301",
            "20180732",
            "20180700",
            "2018071a",
        ] {
            assert_eq!(iso_date(bad), None, "{bad}");
        }
    }

    #[test]
    fn text_is_trimmed_and_its_punctuation_put_right() {
        assert_eq!(
            tidy("  THE DRIVERS\u{92} SEAT  ").as_deref(),
            Some("THE DRIVERS\u{2019} SEAT")
        );
        assert_eq!(
            tidy("\u{93}TIRE SELECTION\u{94} \u{96} PART\u{a0}567").as_deref(),
            Some("\u{201c}TIRE SELECTION\u{201d} \u{2013} PART 567")
        );
        // Any other control character is dropped; ordinary accents stay.
        assert_eq!(tidy("A\u{81}B\u{0}C \u{e9}").as_deref(), Some("ABC \u{e9}"));
        assert_eq!(tidy("   "), None);
        assert_eq!(tidy("\u{a0}"), None);
        assert_eq!(given("21S53").as_deref(), Some("21S53"));
        for placeholder in ["N/A", "n/a", "NA", "None", "TBD", " "] {
            assert_eq!(given(placeholder), None, "{placeholder}");
        }
    }

    #[test]
    fn a_line_is_a_record_a_junk_line_or_an_error() {
        let text = format!(
            "{}\r\n\r\n{}\r\n{}\n",
            line(|_| {}),
            line(|fields| fields[10] = "T".to_owned()),
            line(|fields| {
                fields[4] = "9999".to_owned();
                fields[11] = "0".to_owned();
                fields[27] = "Yes".to_owned();
                fields[28] = "YES".to_owned();
            }),
        );
        let (found, counts) = rows(&text).unwrap();
        assert_eq!(
            counts,
            NhtsaCounts {
                records: 3,
                vehicles: 2,
                other_types: 1,
                junk: 1
            }
        );
        assert_eq!(found[0].year, Some(2019));
        assert_eq!(found[0].record_id, 7);
        assert!(!found[0].do_not_drive && !found[0].park_outside);
        assert_eq!(found[1].year, None);
        assert_eq!(found[1].units, None);
        assert!(found[1].do_not_drive && found[1].park_outside);

        // A line that is neither is a file this build cannot read.
        let short = format!("{}\r\n1\t2\t3\r\n", line(|_| {}));
        assert_eq!(
            format!("{:#}", rows(&short).unwrap_err()),
            "test.txt line 2 has 3 fields, expected 29"
        );
        let no_make = line(|fields| fields[2] = " ".to_owned());
        assert_eq!(
            format!("{:#}", rows(&no_make).unwrap_err()),
            "test.txt line 1: MAKETXT is empty"
        );
        // A non-vehicle record is not read, so its fields are not checked.
        let tire = line(|fields| {
            fields[10] = "T".to_owned();
            fields[0] = "x".to_owned();
        });
        assert_eq!(rows(&tire).unwrap().1.other_types, 1);
        // Bytes that are not UTF-8 do not stop the build.
        let mut bytes = line(|_| {}).into_bytes();
        bytes.extend_from_slice(b"\xff\r\n");
        let mut counts = NhtsaCounts::default();
        parse_reader(&bytes[..], "test.txt", &mut counts, &mut |_| Ok(())).unwrap();
        assert_eq!(counts.vehicles, 1);
    }

    #[test]
    fn the_layout_is_read_from_rcl_txt() {
        let text = "RECALLS\n\n1.Field# 23 added as of Sept. 14, 2007\n5.Fields #19 has been reduced\n\tFields #20 and #22\n\nField#   Name   Type/Size   Description\n------   ----   ---------   -----\n1        RECORD_ID           NUMBER(9)   Running Sequence Number\n2        CAMPNO              CHAR(12)    NHTSA Recall Number\n28\t\t DO_NOT_DRIVE\t\t CHAR(3)\t Consumer Advisory\n";
        assert_eq!(layout(text), ["RECORD_ID", "CAMPNO", "DO_NOT_DRIVE"]);
        let error = check_layout(text).unwrap_err().to_string();
        assert!(
            error.starts_with("the fields are not the ones this build reads: found 3 (RECORD_ID, CAMPNO, DO_NOT_DRIVE), expected 29 ("),
            "{error}"
        );
    }
}
