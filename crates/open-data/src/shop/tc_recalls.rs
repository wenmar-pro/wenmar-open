//! Transport Canada's Vehicle Recalls Database, read row by row. Nothing is
//! matched to the catalog here: `parse` only says what each row holds.
//!
//! The file is one CSV in UTF-8 with a header and 15 columns. A campaign
//! has one row for each year, make and model it lists. A comment may be
//! quoted and run over several lines.

use std::collections::{BTreeMap, BTreeSet};
use std::fs::File;
use std::io;
use std::path::Path;

use anyhow::{Context, Result, bail};
use wenmar_shopdata::{Dataset, DatasetEntry, licence};

use crate::shop::build::ShapeContext;
use crate::shop::recalls::{self, OGL_CANADA_ATTRIBUTION, RecallRecord, VehicleLine};
use crate::sources::SourceRecord;

/// The file, as `open-data fetch tc-recalls` saves it.
pub const FILE: &str = "vrdb_full_monthly.csv";

/// The 15 columns, in order, as the file's first line names them.
pub const HEADER: [&str; 15] = [
    "RECALL_NUMBER_NUM",
    "YEAR",
    "MANUFACTURER_RECALL_NO_TXT",
    "CATEGORY_ETXT",
    "CATEGORY_FTXT",
    "MAKE_NAME_NM",
    "MODEL_NAME_NM",
    "UNIT_AFFECTED_NBR",
    "SYSTEM_TYPE_ETXT",
    "SYSTEM_TYPE_FTXT",
    "NOTIFICATION_TYPE_ETXT",
    "NOTIFICATION_TYPE_FTXT",
    "COMMENT_ETXT",
    "COMMENT_FTXT",
    "RECALL_DATE_DTE",
];

/// Every value of `CATEGORY_ETXT` in the file, and whether its rows are
/// kept. Vehicles that go on the road are kept, trailers and motorcycles
/// among them, as NHTSA's vehicle records include them. Tires, child
/// seats, equipment and vehicles not built for the road are dropped.
pub const CATEGORIES: [(&str, bool); 28] = [
    ("Car", true),
    ("SUV", true),
    ("Light Truck & Van", true),
    ("Minivan", true),
    ("Truck - Med. & H.D.", true),
    ("Chassis Cab", true),
    ("Bus", true),
    ("School Bus", true),
    ("Motorhome", true),
    ("RV Chassis", true),
    ("RV Trailer", true),
    ("Light Trailer", true),
    ("Heavy Trailer", true),
    ("Motorcycle", true),
    ("Low Speed Motorcycle", true),
    ("Minibike/ Moped /Scooter", true),
    ("Motor Tricycle", true),
    ("Three Wheeled Vehicle", true),
    ("Low Speed Vehicle", true),
    ("Tire", false),
    ("Equipment", false),
    ("Child Car Seat", false),
    ("Booster Seat", false),
    ("A.T.V.", false),
    ("Snowmobile", false),
    ("Snowmobile Cutter", false),
    ("Restricted-Use Vehicle", false),
    ("Competition Vehicle", false),
];

/// What the file writes where a field was never filled in.
pub const NOT_ENTERED: &str = "Not Entered";
pub const NOT_ENTERED_FR: &str = "Non Saisie";

/// What the file writes where a comment has no French version.
pub const NO_TRANSLATION: &str = "Translation not available";

/// One kept row: one campaign, year, make and model.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TcRecallRow {
    /// `RECALL_NUMBER_NUM`, as text: `2019631`, or `197801T` for an old
    /// tire campaign.
    pub campaign: String,
    /// `YEAR`, which the file writes `1975.0`. `None` for `-9999.0`.
    pub year: Option<i32>,
    /// `MANUFACTURER_RECALL_NO_TXT`: the manufacturer's own number.
    pub mfr_campaign: Option<String>,
    /// `CATEGORY_ETXT` and `CATEGORY_FTXT`, such as `Car` and `Voiture`.
    pub category_en: String,
    pub category_fr: String,
    /// `MAKE_NAME_NM` and `MODEL_NAME_NM`.
    pub make_text: String,
    pub model_text: String,
    /// `UNIT_AFFECTED_NBR`, which the file writes `2700.0`. `None` when
    /// empty or 0.
    pub units: Option<u32>,
    /// `SYSTEM_TYPE_ETXT` and `SYSTEM_TYPE_FTXT`, such as `Brakes` and
    /// `Freins`. `None` for `Not Entered`.
    pub system_en: Option<String>,
    pub system_fr: Option<String>,
    /// `NOTIFICATION_TYPE_ETXT` and `NOTIFICATION_TYPE_FTXT`, as written:
    /// `Safety Mfr`, `Compliance Mfr`, `Safety TC`, `Compliance TC`,
    /// `Recalls Audit`, `Inconsequential`, `Service Campaign Mfr`,
    /// `Superseded` or `Not Entered`.
    pub notification_en: String,
    pub notification_fr: String,
    /// `COMMENT_ETXT`, with line ends written as `\n`.
    pub comment_en: Option<String>,
    /// `COMMENT_FTXT`. `None` for `Translation not available`.
    pub comment_fr: Option<String>,
    /// `RECALL_DATE_DTE`, as `2019-12-12`.
    pub recall_date: Option<String>,
}

/// What `parse` read.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct TcCounts {
    /// Rows after the header.
    pub records: u64,
    /// Of those, the rows handed on.
    pub kept: u64,
    /// Of those, the rows left out for their category.
    pub dropped: u64,
}

/// Whether rows of a category are kept. `None` for a category that is not
/// in [`CATEGORIES`].
pub fn keeps(category_en: &str) -> Option<bool> {
    CATEGORIES
        .iter()
        .find(|(name, _)| *name == category_en)
        .map(|(_, kept)| *kept)
}

/// A whole number the file writes with a decimal point: `1975.0` is 1975.
pub fn whole_number(text: &str) -> Option<i64> {
    let text = text.trim();
    let (whole, fraction) = text.split_once('.').unwrap_or((text, ""));
    if whole.is_empty() || !fraction.bytes().all(|byte| byte == b'0') {
        return None;
    }
    whole.parse().ok()
}

/// A text field with its line ends written as `\n`, the blanks at the end
/// of each line removed and stray characters put right. `None` when
/// nothing is left.
pub fn text(field: &str) -> Option<String> {
    let mut tidy = String::with_capacity(field.len());
    for line in field.replace("\r\n", "\n").replace('\r', "\n").split('\n') {
        let mut kept = String::with_capacity(line.len());
        for character in line.chars() {
            match character {
                // An old word processor's hyphen: `Harley\u{1e}Davidson`.
                '\u{1e}' => kept.push('-'),
                '\u{a0}' | '\t' => kept.push(' '),
                '\u{ad}' => {}
                other if other.is_control() => {}
                other => kept.push(other),
            }
        }
        tidy.push_str(kept.trim_end());
        tidy.push('\n');
    }
    let tidy = tidy.trim();
    (!tidy.is_empty()).then(|| tidy.to_owned())
}

fn date(field: &str) -> Option<String> {
    let field = field.trim();
    let shaped = field.len() == 10
        && field.bytes().enumerate().all(|(index, byte)| match index {
            4 | 7 => byte == b'-',
            _ => byte.is_ascii_digit(),
        });
    shaped.then(|| field.to_owned())
}

fn required(record: &csv::StringRecord, index: usize) -> Result<String> {
    text(&record[index]).with_context(|| format!("{} is empty", HEADER[index]))
}

/// Reads the 15 fields of one kept row.
fn read_row(record: &csv::StringRecord) -> Result<TcRecallRow> {
    let year = whole_number(&record[1])
        .and_then(|year| i32::try_from(year).ok())
        .filter(|year| (1900..=2100).contains(year));
    let units = whole_number(&record[7])
        .and_then(|units| u32::try_from(units).ok())
        .filter(|units| *units > 0);
    Ok(TcRecallRow {
        campaign: required(record, 0)?,
        year,
        mfr_campaign: text(&record[2]),
        category_en: required(record, 3)?,
        category_fr: text(&record[4]).unwrap_or_default(),
        make_text: required(record, 5)?,
        model_text: required(record, 6)?,
        units,
        system_en: text(&record[8]).filter(|system| system != NOT_ENTERED),
        system_fr: text(&record[9]).filter(|system| system != NOT_ENTERED_FR),
        notification_en: text(&record[10]).unwrap_or_default(),
        notification_fr: text(&record[11]).unwrap_or_default(),
        comment_en: text(&record[12]),
        comment_fr: text(&record[13]).filter(|comment| comment != NO_TRANSLATION),
        recall_date: date(&record[14]),
    })
}

/// Reads CSV text and hands each kept row to `row`. `origin` names the
/// file in an error.
pub fn parse_reader(
    reader: impl io::Read,
    origin: &str,
    row: &mut dyn FnMut(TcRecallRow) -> Result<()>,
) -> Result<TcCounts> {
    let mut csv = csv::ReaderBuilder::new()
        .has_headers(true)
        .from_reader(reader);
    let header: Vec<String> = csv
        .headers()
        .with_context(|| format!("reading the header of {origin}"))?
        .iter()
        .map(str::to_owned)
        .collect();
    if header != HEADER {
        bail!(
            "{origin}: the columns are not the ones this build reads: found {} ({}), expected {} ({})",
            header.len(),
            header.join(", "),
            HEADER.len(),
            HEADER.join(", ")
        );
    }
    let mut counts = TcCounts::default();
    let mut record = csv::StringRecord::new();
    while csv
        .read_record(&mut record)
        .with_context(|| format!("reading {origin}"))?
    {
        counts.records += 1;
        let line = record.position().map_or(0, csv::Position::line);
        let category = record[3].trim();
        match keeps(category) {
            Some(true) => {}
            Some(false) => {
                counts.dropped += 1;
                continue;
            }
            None => bail!(
                "{origin} line {line}: the category {category:?} is not one this build knows; add it to CATEGORIES in crates/open-data/src/shop/tc_recalls.rs as kept or dropped"
            ),
        }
        counts.kept += 1;
        row(read_row(&record).with_context(|| format!("{origin} line {line}"))?)?;
    }
    Ok(counts)
}

/// Reads the fetched `tc-recalls` directory and hands every kept row to
/// `row`, in file order.
pub fn parse(directory: &Path, row: &mut dyn FnMut(TcRecallRow) -> Result<()>) -> Result<TcCounts> {
    let path = directory.join(FILE);
    let file = File::open(&path).with_context(|| {
        format!(
            "opening {}; run open-data fetch tc-recalls first",
            path.display()
        )
    })?;
    parse_reader(io::BufReader::new(file), FILE, row)
}

/// The `source` column of a campaign from this file.
pub const SOURCE: &str = "tc";

/// A campaign gathered from its rows.
struct Campaign {
    /// The first kept row. What a campaign says of itself is the same on
    /// every one of its rows.
    first: TcRecallRow,
    /// Each make, model and year it lists, once.
    vehicles: BTreeSet<(String, String, Option<i32>)>,
}

/// Builds the `recalls_ca` dataset from the fetched `tc-recalls`
/// directory: one `recall` row for each campaign with a kept row and one
/// `recall_vehicle` row for each make, model and year it lists.
pub fn shape(context: &mut ShapeContext<'_>, directory: &Path) -> Result<()> {
    let as_of = SourceRecord::read(directory)?.as_of;
    let mut campaigns: BTreeMap<String, Campaign> = BTreeMap::new();
    parse(directory, &mut |row| {
        let campaign = campaigns
            .entry(row.campaign.clone())
            .or_insert_with(|| Campaign {
                first: row.clone(),
                vehicles: BTreeSet::new(),
            });
        campaign
            .vehicles
            .insert((row.make_text, row.model_text, row.year));
        Ok(())
    })?;

    recalls::create_tables(context.transaction)?;
    for (number, campaign) in &campaigns {
        let first = &campaign.first;
        let recall_id = recalls::insert_recall(
            context.transaction,
            &RecallRecord {
                source: SOURCE,
                campaign: number,
                report_date: first.recall_date.as_deref(),
                component: first.system_en.as_deref(),
                component_fr: first.system_fr.as_deref(),
                notification: Some(first.notification_en.as_str())
                    .filter(|kind| !kind.is_empty() && *kind != NOT_ENTERED),
                notification_fr: Some(first.notification_fr.as_str())
                    .filter(|kind| !kind.is_empty() && *kind != NOT_ENTERED_FR),
                summary_en: first.comment_en.as_deref(),
                summary_fr: first.comment_fr.as_deref(),
                mfr_campaign: first.mfr_campaign.as_deref(),
                units: first.units,
                ..RecallRecord::default()
            },
        )
        .with_context(|| format!("storing campaign {number}"))?;
        for (make_text, model_text, year) in &campaign.vehicles {
            recalls::insert_vehicle(
                context.transaction,
                context.matcher,
                recall_id,
                &VehicleLine {
                    year: *year,
                    make_text,
                    model_text,
                    built_from: None,
                    built_to: None,
                },
            )?;
        }
    }
    context.register(
        Dataset::RecallsCa,
        DatasetEntry {
            as_of,
            rows: campaigns.len() as u64,
            licence: licence::OGL_CANADA.to_owned(),
            attribution: OGL_CANADA_ATTRIBUTION.to_owned(),
        },
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    const HEAD: &str = "RECALL_NUMBER_NUM,YEAR,MANUFACTURER_RECALL_NO_TXT,CATEGORY_ETXT,CATEGORY_FTXT,MAKE_NAME_NM,MODEL_NAME_NM,UNIT_AFFECTED_NBR,SYSTEM_TYPE_ETXT,SYSTEM_TYPE_FTXT,NOTIFICATION_TYPE_ETXT,NOTIFICATION_TYPE_FTXT,COMMENT_ETXT,COMMENT_FTXT,RECALL_DATE_DTE\n";

    fn rows(body: &str) -> Result<(Vec<TcRecallRow>, TcCounts)> {
        let mut found = Vec::new();
        let csv = format!("{HEAD}{body}");
        let counts = parse_reader(csv.as_bytes(), "test.csv", &mut |row| {
            found.push(row);
            Ok(())
        })?;
        Ok((found, counts))
    }

    #[test]
    fn numbers_written_with_a_decimal_point_are_whole_numbers() {
        assert_eq!(whole_number("1975.0"), Some(1975));
        assert_eq!(whole_number(" 2700.0 "), Some(2700));
        assert_eq!(whole_number("-9999.0"), Some(-9999));
        assert_eq!(whole_number("12"), Some(12));
        for bad in ["", ".0", "1975.5", "19x5.0", "1975.0.0", "nan"] {
            assert_eq!(whole_number(bad), None, "{bad}");
        }
    }

    #[test]
    fn text_keeps_its_lines_and_loses_its_stray_characters() {
        assert_eq!(
            text("Issue: \r\nA part may fail.  \r\n \r\nCorrective Actions:\rNone.\n").as_deref(),
            Some("Issue:\nA part may fail.\n\nCorrective Actions:\nNone.")
        );
        assert_eq!(
            text("Harley\u{1e}Davidson\u{a0}doit\taviser, co\u{ad}op").as_deref(),
            Some("Harley-Davidson doit aviser, coop")
        );
        assert_eq!(text("  \r\n "), None);
        assert_eq!(
            text("Sécurité - fabricant").as_deref(),
            Some("Sécurité - fabricant")
        );
    }

    #[test]
    fn every_category_is_kept_or_dropped() {
        assert_eq!(CATEGORIES.len(), 28);
        let kept: Vec<&str> = CATEGORIES
            .iter()
            .filter(|(_, kept)| *kept)
            .map(|(name, _)| *name)
            .collect();
        assert_eq!(kept.len(), 19);
        for name in [
            "Car",
            "SUV",
            "Light Truck & Van",
            "Minivan",
            "Motorcycle",
            "Heavy Trailer",
            "School Bus",
        ] {
            assert_eq!(keeps(name), Some(true), "{name}");
        }
        for name in [
            "Tire",
            "Equipment",
            "Child Car Seat",
            "Booster Seat",
            "A.T.V.",
            "Snowmobile",
        ] {
            assert_eq!(keeps(name), Some(false), "{name}");
        }
        assert_eq!(keeps("Hovercraft"), None);
        assert_eq!(keeps("car"), None);
        // No name is listed twice.
        let mut names: Vec<&str> = CATEGORIES.iter().map(|(name, _)| *name).collect();
        names.sort_unstable();
        names.dedup();
        assert_eq!(names.len(), 28);
    }

    #[test]
    fn a_row_is_read_field_by_field() {
        let (found, counts) = rows(
            "2019631,2019.0,N192273510,Light Truck & Van,Camionnette et fourgonnette,CHEVROLET,SILVERADO,24289.0,Electrical,Circuit électrique,Safety Mfr,Sécurité - fabricant,\"Issue:\r\nThe nut may be loose, the \"\"B+\"\" one.\",\"Problème :\r\nUn écrou.\",2019-12-12\n\
             1975063,-9999.0,,Car,Voiture,BF GOODRICH,SPACE SAVER,0.0,Not Entered,Non Saisie,Not Entered,Non Saisie,OLD TEXT,Translation not available,1975-06-17\n\
             197801T,1978.0,,Tire,Pneu,MOTOMASTER,SAFETY-99,208.0,Tires,Pneus,Safety Mfr,Sécurité - fabricant,TIRES,Translation not available,1978-04-28\n",
        )
        .unwrap();
        assert_eq!(
            counts,
            TcCounts {
                records: 3,
                kept: 2,
                dropped: 1
            }
        );
        assert_eq!(
            found[0],
            TcRecallRow {
                campaign: "2019631".to_owned(),
                year: Some(2019),
                mfr_campaign: Some("N192273510".to_owned()),
                category_en: "Light Truck & Van".to_owned(),
                category_fr: "Camionnette et fourgonnette".to_owned(),
                make_text: "CHEVROLET".to_owned(),
                model_text: "SILVERADO".to_owned(),
                units: Some(24289),
                system_en: Some("Electrical".to_owned()),
                system_fr: Some("Circuit électrique".to_owned()),
                notification_en: "Safety Mfr".to_owned(),
                notification_fr: "Sécurité - fabricant".to_owned(),
                comment_en: Some("Issue:\nThe nut may be loose, the \"B+\" one.".to_owned()),
                comment_fr: Some("Problème :\nUn écrou.".to_owned()),
                recall_date: Some("2019-12-12".to_owned()),
            }
        );
        let old = &found[1];
        assert_eq!(old.year, None);
        assert_eq!(old.mfr_campaign, None);
        assert_eq!(old.units, None);
        assert_eq!(old.system_en, None);
        assert_eq!(old.system_fr, None);
        assert_eq!(old.notification_en, "Not Entered");
        assert_eq!(old.comment_fr, None);
    }

    #[test]
    fn refuses_a_file_it_was_not_written_for() {
        let message = |csv: &str| {
            format!(
                "{:#}",
                parse_reader(csv.as_bytes(), "test.csv", &mut |_| Ok(())).unwrap_err()
            )
        };
        // A column added, removed or renamed.
        let renamed = format!("{}\n", HEAD.trim_end().replace("YEAR", "MODEL_YEAR"));
        assert!(
            message(&renamed).starts_with("test.csv: the columns are not the ones this build reads: found 15 (RECALL_NUMBER_NUM, MODEL_YEAR, "),
            "{}",
            message(&renamed)
        );
        assert!(message("A,B\n1,2\n").contains("found 2 (A, B), expected 15 ("));
        // A category nobody has decided on.
        let unknown = format!(
            "{HEAD}2026001,2026.0,,Hovercraft,Aéroglisseur,ACME,ONE,1.0,Other,Autres,Safety Mfr,Sécurité - fabricant,TEXT,TEXTE,2026-01-02\n"
        );
        assert_eq!(
            message(&unknown),
            "test.csv line 2: the category \"Hovercraft\" is not one this build knows; add it to CATEGORIES in crates/open-data/src/shop/tc_recalls.rs as kept or dropped"
        );
        // A row with a field missing.
        let short = format!("{HEAD}2026001,2026.0,,Car\n");
        assert!(
            message(&short).starts_with("reading test.csv: "),
            "{}",
            message(&short)
        );
        // A kept row with no make.
        let no_make = format!(
            "{HEAD}2026001,2026.0,,Car,Voiture,,ONE,1.0,Other,Autres,Safety Mfr,Sécurité - fabricant,TEXT,TEXTE,2026-01-02\n"
        );
        assert_eq!(message(&no_make), "test.csv line 2: MAKE_NAME_NM is empty");
    }
}
