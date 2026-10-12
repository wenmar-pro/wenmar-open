//! NHTSA's recall quarterly and annual reports: how many of a campaign's
//! vehicles the manufacturer says it has remedied.
//!
//! A manufacturer files a report each quarter for a recall, normally six
//! or eight of them, and then one a year. Each report gives totals for the
//! whole campaign at the end of its period. `parse` hands on every report
//! of a vehicle campaign; `shape` keeps the newest one of each campaign.
//!
//! The two archives are read as `open-data fetch nhtsa-recall-reports`
//! saved them, without unpacking them to disk. Each holds one text file:
//! one report per line, 13 fields separated by tabs, no header, lines
//! ended by CR LF. Nothing is quoted.

use std::collections::BTreeMap;
use std::fs::File;
use std::io::{BufRead, BufReader};
use std::path::Path;

use anyhow::{Context, Result, bail};
use rusqlite::params;
use wenmar_shopdata::{Dataset, DatasetEntry, licence, schema};

use crate::shop::build::ShapeContext;
use crate::shop::nhtsa_recalls::{iso_date, layout, tidy};
use crate::sources::SourceRecord;

/// The archive of quarterly reports and NHTSA's description of its fields.
/// NHTSA spells the archive `Qrtly` and the description `Qtrly`.
pub const QUARTERLY_FILE: &str = "FLAT_RCL_Qrtly_Rpts.zip";
pub const QUARTERLY_LAYOUT: &str = "RCL_Qtrly_Rpts.txt";

/// The archive of annual reports and the description of its fields.
pub const ANNUAL_FILE: &str = "FLAT_RCL_Annual_Rpts.zip";
pub const ANNUAL_LAYOUT: &str = "RCL_Annual_Rpts.txt";

/// The 13 fields of a quarterly report, in order, as the layout names them.
pub const QUARTERLY_FIELDS: [&str; 13] = [
    "MFGTXT",
    "CAMPNO",
    "MFGCAMPNO",
    "RCLSUBJ",
    "ODATE",
    "ODATEEND",
    "RPTNO",
    "RPTQTR",
    "INVOLVED",
    "TTLREMEDIED",
    "TTLUNREACH",
    "TTLREMOVED",
    "SUBMDATE",
];

/// The 13 fields of an annual report. Only field 8 differs: a span of four
/// quarters, `2025-2 to 2026-1`, where a quarterly report has one.
pub const ANNUAL_FIELDS: [&str; 13] = [
    "MFGTXT",
    "CAMPNO",
    "MFGCAMPNO",
    "RCLSUBJ",
    "ODATE",
    "ODATEEND",
    "RPTNO",
    "RPT_PERIOD",
    "INVOLVED",
    "TTLREMEDIED",
    "TTLUNREACH",
    "TTLREMOVED",
    "SUBMDATE",
];

/// The third character of a campaign number that marks a vehicle campaign,
/// as in `19V888000`. The others are `E` (equipment), `T` (tires) and `C`
/// (child restraints).
pub const VEHICLE: char = 'V';

/// Which of the two files a report is from.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum ReportKind {
    Quarterly,
    Annual,
}

impl ReportKind {
    /// The `kind` column: `quarterly` or `annual`.
    pub fn as_str(self) -> &'static str {
        match self {
            ReportKind::Quarterly => "quarterly",
            ReportKind::Annual => "annual",
        }
    }
}

/// One report of a vehicle campaign.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RecallReportRow {
    pub kind: ReportKind,
    /// Field 2, `CAMPNO`, such as `19V888000`.
    pub campaign: String,
    /// Field 7, `RPTNO`: 1 for the first report of its kind.
    pub report_number: u32,
    /// Field 8: the first and the last quarter the report covers, each as
    /// `2026-2`. A quarterly report has the same quarter in both.
    pub period_from: String,
    pub period_to: String,
    /// Field 9, `INVOLVED`: vehicles in the campaign.
    pub involved: u32,
    /// Field 10, `TTLREMEDIED`: of those, remedied so far.
    pub remedied: u32,
    /// Field 11, `TTLUNREACH`: what the layout calls "total unreachable".
    pub unreachable: u32,
    /// Field 12, `TTLREMOVED`: what the layout calls "total removed". The
    /// layout says no more of either than its name.
    pub removed: u32,
    /// Field 13, `SUBMDATE`: the day the report was filed, as `2026-07-22`.
    pub submitted: Option<String>,
}

/// What `parse` read.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct ReportCounts {
    /// Lines with 13 fields.
    pub records: u64,
    /// Of those, the reports of vehicle campaigns handed on.
    pub vehicles: u64,
    /// Of those, the reports of equipment, tire and child restraint
    /// campaigns left out.
    pub other_types: u64,
    /// Of those, the reports of vehicle campaigns left out because the
    /// period is not a quarter. The files hold two.
    pub malformed: u64,
    /// Lines that are empty but for the line end.
    pub empty: u64,
}

/// A quarter written `2026-2`, as year and quarter. `None` for anything
/// else.
pub fn quarter(text: &str) -> Option<(u16, u8)> {
    let (year, quarter) = text.trim().split_once('-')?;
    if year.len() != 4 || quarter.len() != 1 {
        return None;
    }
    let year: u16 = year.parse().ok()?;
    let quarter: u8 = quarter.parse().ok()?;
    ((1949..=2100).contains(&year) && (1..=4).contains(&quarter)).then_some((year, quarter))
}

/// The first and last quarter of field 8, each written back as `2026-2`.
/// A quarterly report gives one quarter; an annual one gives
/// `2025-2 to 2026-1`. `None` when it is neither, or runs backwards.
pub fn period(kind: ReportKind, text: &str) -> Option<(String, String)> {
    let written = |(year, quarter): (u16, u8)| format!("{year:04}-{quarter}");
    match kind {
        ReportKind::Quarterly => {
            let only = quarter(text)?;
            Some((written(only), written(only)))
        }
        ReportKind::Annual => {
            let (from, to) = text.split_once(" to ")?;
            let (from, to) = (quarter(from)?, quarter(to)?);
            (from <= to).then(|| (written(from), written(to)))
        }
    }
}

/// Whether `campaign` is the number of a vehicle campaign: nine letters
/// and digits with `V` third.
pub fn is_vehicle_campaign(campaign: &str) -> bool {
    campaign.len() == 9
        && campaign.bytes().all(|byte| byte.is_ascii_alphanumeric())
        && campaign.chars().nth(2) == Some(VEHICLE)
}

fn count(fields: &[&str], index: usize) -> Result<u32> {
    fields[index].trim().parse().with_context(|| {
        format!(
            "{} {:?} is not a count",
            QUARTERLY_FIELDS[index], fields[index]
        )
    })
}

/// Refuses a layout file whose fields are not `expected`.
pub fn check_layout(text: &str, expected: &[&str; 13]) -> Result<()> {
    let found = layout(text);
    if found != expected {
        bail!(
            "the fields are not the ones this build reads: found {} ({}), expected {} ({})",
            found.len(),
            found.join(", "),
            expected.len(),
            expected.join(", ")
        );
    }
    Ok(())
}

/// Reads one file's text, a line at a time, and hands each report of a
/// vehicle campaign to `row`. `origin` names the file in an error.
pub fn parse_reader(
    mut reader: impl BufRead,
    kind: ReportKind,
    origin: &str,
    counts: &mut ReportCounts,
    row: &mut dyn FnMut(RecallReportRow) -> Result<()>,
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
                counts.empty += 1;
                continue;
            }
            13 => {}
            other => bail!("{origin} line {number} has {other} fields, expected 13"),
        }
        counts.records += 1;
        let Some(campaign) = tidy(fields[1]).map(|campaign| campaign.to_ascii_uppercase()) else {
            bail!("{origin} line {number}: CAMPNO is empty");
        };
        if !is_vehicle_campaign(&campaign) {
            counts.other_types += 1;
            continue;
        }
        let Some((period_from, period_to)) = period(kind, fields[7]) else {
            counts.malformed += 1;
            continue;
        };
        let read = || -> Result<RecallReportRow> {
            Ok(RecallReportRow {
                kind,
                campaign: campaign.clone(),
                report_number: count(&fields, 6)?,
                period_from: period_from.clone(),
                period_to: period_to.clone(),
                involved: count(&fields, 8)?,
                remedied: count(&fields, 9)?,
                unreachable: count(&fields, 10)?,
                removed: count(&fields, 11)?,
                submitted: iso_date(fields[12]),
            })
        };
        counts.vehicles += 1;
        row(read().with_context(|| format!("{origin} line {number}"))?)?;
    }
}

fn hint(path: &Path, verb: &str) -> String {
    format!(
        "{verb} {}; run open-data fetch nhtsa-recall-reports first",
        path.display()
    )
}

/// Reads the fetched `nhtsa-recall-reports` directory: checks both layout
/// files, then hands every report of a vehicle campaign to `row`, the
/// quarterly file first, each in file order.
pub fn parse(
    directory: &Path,
    row: &mut dyn FnMut(RecallReportRow) -> Result<()>,
) -> Result<ReportCounts> {
    let files = [
        (
            ReportKind::Quarterly,
            QUARTERLY_FILE,
            QUARTERLY_LAYOUT,
            &QUARTERLY_FIELDS,
        ),
        (
            ReportKind::Annual,
            ANNUAL_FILE,
            ANNUAL_LAYOUT,
            &ANNUAL_FIELDS,
        ),
    ];
    for (_, _, layout_name, fields) in files {
        let path = directory.join(layout_name);
        let bytes = std::fs::read(&path).with_context(|| hint(&path, "reading"))?;
        check_layout(&String::from_utf8_lossy(&bytes), fields)
            .with_context(|| path.display().to_string())?;
    }
    let mut counts = ReportCounts::default();
    for (kind, name, _, _) in files {
        let path = directory.join(name);
        let file = File::open(&path).with_context(|| hint(&path, "opening"))?;
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
        parse_reader(BufReader::new(entry), kind, name, &mut counts, row)?;
    }
    Ok(counts)
}

/// What makes one report newer than another of the same campaign: the
/// later last quarter; then the later day filed; then annual over
/// quarterly; then the higher report number.
fn rank(row: &RecallReportRow) -> (Option<(u16, u8)>, Option<&str>, ReportKind, u32) {
    (
        quarter(&row.period_to),
        row.submitted.as_deref(),
        row.kind,
        row.report_number,
    )
}

/// The newest report of each campaign, by campaign number.
pub fn newest(directory: &Path) -> Result<(BTreeMap<String, RecallReportRow>, ReportCounts)> {
    let mut newest: BTreeMap<String, RecallReportRow> = BTreeMap::new();
    let counts = parse(directory, &mut |row| {
        match newest.get(&row.campaign) {
            Some(known) if rank(known) >= rank(&row) => {}
            _ => {
                newest.insert(row.campaign.clone(), row);
            }
        }
        Ok(())
    })?;
    Ok((newest, counts))
}

const INSERT: &str = "
INSERT INTO recall_report (
    campaign, kind, report_number, period_from, period_to, submitted,
    involved, remedied, unreachable, removed
) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)";

/// Builds the `recall_reports` dataset from the fetched
/// `nhtsa-recall-reports` directory: one `recall_report` row for each
/// vehicle campaign, holding its newest report.
pub fn shape(context: &mut ShapeContext<'_>, directory: &Path) -> Result<()> {
    let as_of = SourceRecord::read(directory)?.as_of;
    let (newest, _) = newest(directory)?;
    context.transaction.execute_batch(schema::RECALL_REPORTS)?;
    {
        let mut statement = context.transaction.prepare_cached(INSERT)?;
        for (campaign, row) in &newest {
            statement
                .execute(params![
                    campaign,
                    row.kind.as_str(),
                    row.report_number,
                    row.period_from,
                    row.period_to,
                    row.submitted,
                    row.involved,
                    row.remedied,
                    row.unreachable,
                    row.removed,
                ])
                .with_context(|| format!("storing the report of campaign {campaign}"))?;
        }
    }
    context.register(
        Dataset::RecallReports,
        DatasetEntry {
            as_of,
            rows: newest.len() as u64,
            licence: licence::PUBLIC_DOMAIN.to_owned(),
            attribution: String::new(),
        },
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn line(change: impl FnOnce(&mut Vec<String>)) -> String {
        let mut fields: Vec<String> = [
            "Ford Motor Company",
            "21V983000",
            "21S53",
            "Tailgate May Open Unexpectedly",
            "20220531",
            "",
            "11",
            "2024-4",
            "57591",
            "34286",
            "1201",
            "18",
            "20250124",
        ]
        .iter()
        .map(|field| (*field).to_owned())
        .collect();
        change(&mut fields);
        fields.join("\t")
    }

    fn rows(kind: ReportKind, text: &str) -> Result<(Vec<RecallReportRow>, ReportCounts)> {
        let mut found = Vec::new();
        let mut counts = ReportCounts::default();
        parse_reader(text.as_bytes(), kind, "test.txt", &mut counts, &mut |row| {
            found.push(row);
            Ok(())
        })?;
        Ok((found, counts))
    }

    #[test]
    fn a_quarter_is_a_year_and_one_of_four() {
        assert_eq!(quarter("2026-2"), Some((2026, 2)));
        assert_eq!(quarter(" 2015-1 "), Some((2015, 1)));
        for bad in [
            "", "2026", "2026-", "2026-5", "2026-0", "26-2", "2026-02", "2099-", "9999-1",
        ] {
            assert_eq!(quarter(bad), None, "{bad}");
        }
        assert_eq!(
            period(ReportKind::Quarterly, "2024-4"),
            Some(("2024-4".to_owned(), "2024-4".to_owned()))
        );
        assert_eq!(
            period(ReportKind::Annual, "2025-2 to 2026-1"),
            Some(("2025-2".to_owned(), "2026-1".to_owned()))
        );
        assert_eq!(period(ReportKind::Annual, "2024-4"), None);
        assert_eq!(period(ReportKind::Annual, "2026-1 to 2025-2"), None);
        assert_eq!(period(ReportKind::Quarterly, "2025-2 to 2026-1"), None);
    }

    #[test]
    fn a_vehicle_campaign_has_v_third() {
        for campaign in ["19V888000", "21V00J000", "05V483000"] {
            assert!(is_vehicle_campaign(campaign), "{campaign}");
        }
        for campaign in [
            "09C010000",
            "20E001000",
            "20T001000",
            "19V88800",
            "19V8880000",
            "19V888-00",
            "",
        ] {
            assert!(!is_vehicle_campaign(campaign), "{campaign}");
        }
    }

    #[test]
    fn a_line_is_a_report_or_is_left_out_or_is_an_error() {
        let text = format!(
            "{}\r\n\r\n{}\r\n{}\r\n{}\n",
            line(|_| {}),
            line(|fields| fields[1] = "09C010000".to_owned()),
            line(|fields| fields[7] = "2099-".to_owned()),
            line(|fields| {
                fields[1] = " 21v983000 ".to_owned();
                fields[12] = String::new();
            }),
        );
        let (found, counts) = rows(ReportKind::Quarterly, &text).unwrap();
        assert_eq!(
            counts,
            ReportCounts {
                records: 4,
                vehicles: 2,
                other_types: 1,
                malformed: 1,
                empty: 1
            }
        );
        assert_eq!(
            found[0],
            RecallReportRow {
                kind: ReportKind::Quarterly,
                campaign: "21V983000".to_owned(),
                report_number: 11,
                period_from: "2024-4".to_owned(),
                period_to: "2024-4".to_owned(),
                involved: 57_591,
                remedied: 34_286,
                unreachable: 1_201,
                removed: 18,
                submitted: Some("2025-01-24".to_owned()),
            }
        );
        assert_eq!(found[1].campaign, "21V983000");
        assert_eq!(found[1].submitted, None);

        let annual = line(|fields| fields[7] = "2025-2 to 2026-1".to_owned());
        let (found, _) = rows(ReportKind::Annual, &annual).unwrap();
        assert_eq!(found[0].kind, ReportKind::Annual);
        assert_eq!(
            (found[0].period_from.as_str(), found[0].period_to.as_str()),
            ("2025-2", "2026-1")
        );

        let short = format!("{}\r\n1\t2\t3\r\n", line(|_| {}));
        assert_eq!(
            format!("{:#}", rows(ReportKind::Quarterly, &short).unwrap_err()),
            "test.txt line 2 has 3 fields, expected 13"
        );
        let words = line(|fields| fields[9] = "many".to_owned());
        assert_eq!(
            format!("{:#}", rows(ReportKind::Quarterly, &words).unwrap_err()),
            "test.txt line 1: TTLREMEDIED \"many\" is not a count: invalid digit found in string"
        );
        // A report of another kind of campaign is not read, so its fields
        // are not checked.
        let seat = line(|fields| {
            fields[1] = "09C010000".to_owned();
            fields[9] = "many".to_owned();
        });
        assert_eq!(rows(ReportKind::Quarterly, &seat).unwrap().1.other_types, 1);
    }

    #[test]
    fn the_newer_report_is_the_one_whose_period_ends_later() {
        let report = |kind, number, to: &str, submitted: Option<&str>| RecallReportRow {
            kind,
            campaign: "21V983000".to_owned(),
            report_number: number,
            period_from: to.to_owned(),
            period_to: to.to_owned(),
            involved: 1,
            remedied: 1,
            unreachable: 0,
            removed: 0,
            submitted: submitted.map(str::to_owned),
        };
        let quarterly = report(ReportKind::Quarterly, 11, "2024-4", Some("2025-01-24"));
        // A later quarter wins whatever else differs.
        assert!(rank(&report(ReportKind::Quarterly, 1, "2025-1", None)) > rank(&quarterly));
        assert!(
            rank(&report(ReportKind::Annual, 1, "2024-3", Some("2026-01-01"))) < rank(&quarterly)
        );
        // The same quarter: the later day filed, then annual.
        assert!(
            rank(&report(
                ReportKind::Quarterly,
                9,
                "2024-4",
                Some("2025-02-01")
            )) > rank(&quarterly)
        );
        assert!(
            rank(&report(ReportKind::Annual, 1, "2024-4", Some("2025-01-24"))) > rank(&quarterly)
        );
        assert!(
            rank(&report(
                ReportKind::Quarterly,
                12,
                "2024-4",
                Some("2025-01-24")
            )) > rank(&quarterly)
        );
    }
}
