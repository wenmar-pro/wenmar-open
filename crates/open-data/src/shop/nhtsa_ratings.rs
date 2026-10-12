//! NHTSA's crash test ratings file, read row by row, and the step that
//! stores it.
//!
//! The file is one CSV in UTF-8 with a header and 128 columns, as
//! `open-data fetch nhtsa-ratings` saves it. A row is one vehicle as NHTSA
//! rated it: a make, a model, a model year, a body style and a drive. One
//! model year often has several rows, and a row may have no stars at all:
//! NHTSA lists vehicles it has not tested. A field may be quoted and run
//! over several lines.
//!
//! `Safercar_data_READ_ME_file.txt`, fetched beside it, names the columns.
//! It says the fields are separated by tabs; they are separated by commas.

use std::collections::HashSet;
use std::fs::File;
use std::io;
use std::path::Path;

use anyhow::{Context, Result, bail};
use rusqlite::params;
use wenmar_shopdata::{Dataset, DatasetEntry, licence, schema};

use crate::shop::build::ShapeContext;
use crate::sources::SourceRecord;

/// The file, as `open-data fetch nhtsa-ratings` saves it.
pub const FILE: &str = "Safercar_data.csv";

/// NHTSA's description of the columns, fetched beside the file. It is kept
/// for people; the build reads the header of [`FILE`] instead.
pub const README_FILE: &str = "Safercar_data_READ_ME_file.txt";

/// The 128 columns, in order, as the file's first line names them.
pub const HEADER: [&str; 128] = [
    "MAKE",
    "MODEL",
    "MODEL_YR",
    "BODY_STYLE",
    "VEHICLE_TYPE",
    "DRIVE_TRAIN",
    "PRODUCTION_RELEASE",
    "VEHICLE_CLASS",
    "BODY_FRAME",
    "NUM_OF_SEATING",
    "SEAT_LOC",
    "SEAT_LOC_COMMENTS",
    "MIN_GROSS_WEIGHT",
    "MAX_GROSS_WEIGHT",
    "UPPER_BELT_ANCHORAGE",
    "UPPER_BELT_ANCHORAGE_LOC",
    "SEAT_BELT_PRETENSIONER",
    "SEAT_BELT_PRETENSIONER_LOC",
    "LOAD_LIMITERS",
    "LOAD_LIMITERS_LOC",
    "FRNT_BELT_INDICATOR",
    "FRNT_BELT_LOC",
    "REAR_BELT_INDICATOR",
    "LATCH_REAR_POSITION",
    "HEAD_SAB",
    "HEAD_SAB_TYPE",
    "HEAD_SAB_LOC",
    "HEAD_SAB_MOUNT_LOC",
    "HEAD_SAB_MEET_REQUIREMENTS",
    "HEAD_SAB_DEPLOY_IN_ROLLOVER",
    "TORSO_SAB",
    "TORSO_SAB_TYPE",
    "TORSO_SAB_LOC",
    "TORSO_SAB_MOUNT_LOC",
    "KNEE_BOLSTERS",
    "KNEE_BOLSTERS_LOC",
    "ADL",
    "HEAD_RESTRAINT_IND",
    "DYNAMIC_HEAD_RESTRAINT_IND",
    "BETI",
    "BLIND_SPOT_DETECTION",
    "DAY_RUN_LIGHTS",
    "ADAPTIVE_CRUISE_CONTROL",
    "ABS",
    "ARS",
    "ARS_LOC",
    "AUTO_CRASH_NOTIFICATION",
    "CRASH_DATA_RECORDER",
    "ANTI_THEFT_DEVICE",
    "ANTI_THEFT_DEVICE_TYPE",
    "FRNT_COLLISION_WARNING",
    "NHTSA_FRNT_COLLISION_WARNING",
    "NHTSA_FCW_EVALUATION",
    "LANE_DEPARTURE_WARNING",
    "NHTSA_LANE_DEPARTURE_WARNING",
    "NHTSA_LDW_EVALUATION",
    "CRASH_IMMINENT_BRAKE",
    "NHTSA_CRASH_IMMINENT_BRAKE",
    "NHTSA_CIB_EVALUATION",
    "DYNAMIC_BRAKE_SUPPORT",
    "NHTSA_DYNAMIC_BRAKE_SUPPORT",
    "NHTSA_DBS_EVALUATION",
    "NHTSA_ESC",
    "PELVIS_SAB",
    "PELVIS_SAB_TYPE",
    "PELVIS_SAB_LOC",
    "PELVIS_SAB_MOUNT_LOC",
    "OVERALL_STARS",
    "FRNT_TEST_NO",
    "FRNT_VIN",
    "FRNT_DRIV_STARS",
    "FRNT_PASS_STARS",
    "FRNT_SAFETY_CONCERN_DRIV",
    "FRNT_SAFETY_CONCERN_PASS",
    "FRNT_FOOT_NOTES",
    "FRNT_FOOT_NOTES_PASS",
    "OVERALL_FRNT_STARS",
    "CURB_WEIGHT",
    "FRNT_TESTED_WITH",
    "HIC15_DRIV",
    "CHEST_DEFL_DRIV",
    "LEFT_FEMUR_DRIV",
    "RIGHT_FEMUR_DRIV",
    "NIJ_DRIV",
    "NECK_TENS_DRIV",
    "NET_COMP_DRIV",
    "HIC15_PASS",
    "CHEST_DEFL_PASS",
    "LEFT_FEMUR_PASS",
    "RIGHT_FEMUR_PASS",
    "NIJ_PASS",
    "NECK_TENS_PASS",
    "NET_COMP_PASS",
    "SIDE_TEST_NO",
    "SIDE_VIN",
    "SIDE_DRIV_STARS",
    "SIDE_PASS_STARS",
    "SIDE_BARRIER_STAR",
    "COMB_FRNT_STAR",
    "COMB_REAR_STAR",
    "SIDE_SAFETY_CONCERN_DRIV",
    "SIDE_SAFETY_CONCERN_PASS",
    "SIDE_FOOT_NOTES",
    "SIDE_FOOT_NOTES_PASS",
    "OVERALL_SIDE_STARS",
    "SIDE_TESTED_WITH",
    "SIDE_HIC_36_DRIV",
    "RIB_DEFLECTION_DRIV",
    "ABDOMEN_FORCE_DRIV",
    "SYMPHYSIS_FORCE_DRIV",
    "SIDE_HIC_36_PASS",
    "PELVIC_FORCE_PASS",
    "POLE_TEST_NO",
    "POLE_VIN",
    "SIDE_POLE_STARS",
    "POLE_SAFETY_CONCERN",
    "POLE_FOOT_NOTES",
    "POLE_TESTED_WITH",
    "POLE_HIC_36_DRIV",
    "PELVIC_FORCE",
    "ROLLOVER_POSSIBILITY",
    "STATIC_STABI_FACTOR",
    "TIP",
    "ROLL_SAFETY_CONCERN",
    "ROLL_FOOT_NOTES",
    "ROLLOVER_STARS",
    "NHTSA_BACKUP_CAMERA",
    "BACKUP_CAMERA",
];

/// Where each column this build reads is, counted from 0.
mod at {
    pub const MAKE: usize = 0;
    pub const MODEL: usize = 1;
    pub const MODEL_YR: usize = 2;
    pub const BODY_STYLE: usize = 3;
    pub const VEHICLE_TYPE: usize = 4;
    pub const DRIVE_TRAIN: usize = 5;
    pub const PRODUCTION_RELEASE: usize = 6;
    pub const HEAD_SAB: usize = 24;
    pub const TORSO_SAB: usize = 30;
    pub const BLIND_SPOT_DETECTION: usize = 40;
    pub const NHTSA_FRNT_COLLISION_WARNING: usize = 51;
    pub const NHTSA_FCW_EVALUATION: usize = 52;
    pub const NHTSA_LANE_DEPARTURE_WARNING: usize = 54;
    pub const NHTSA_LDW_EVALUATION: usize = 55;
    pub const NHTSA_CRASH_IMMINENT_BRAKE: usize = 57;
    pub const NHTSA_CIB_EVALUATION: usize = 58;
    pub const NHTSA_DYNAMIC_BRAKE_SUPPORT: usize = 60;
    pub const NHTSA_DBS_EVALUATION: usize = 61;
    pub const NHTSA_ESC: usize = 62;
    pub const OVERALL_STARS: usize = 67;
    pub const FRNT_DRIV_STARS: usize = 70;
    pub const FRNT_PASS_STARS: usize = 71;
    pub const FRNT_SAFETY_CONCERN_DRIV: usize = 72;
    pub const FRNT_SAFETY_CONCERN_PASS: usize = 73;
    pub const FRNT_FOOT_NOTES: usize = 74;
    pub const FRNT_FOOT_NOTES_PASS: usize = 75;
    pub const OVERALL_FRNT_STARS: usize = 76;
    pub const CURB_WEIGHT: usize = 77;
    pub const SIDE_DRIV_STARS: usize = 95;
    pub const SIDE_PASS_STARS: usize = 96;
    pub const SIDE_BARRIER_STAR: usize = 97;
    pub const COMB_FRNT_STAR: usize = 98;
    pub const COMB_REAR_STAR: usize = 99;
    pub const SIDE_SAFETY_CONCERN_DRIV: usize = 100;
    pub const SIDE_SAFETY_CONCERN_PASS: usize = 101;
    pub const SIDE_FOOT_NOTES: usize = 102;
    pub const SIDE_FOOT_NOTES_PASS: usize = 103;
    pub const OVERALL_SIDE_STARS: usize = 104;
    pub const SIDE_POLE_STARS: usize = 114;
    pub const POLE_SAFETY_CONCERN: usize = 115;
    pub const POLE_FOOT_NOTES: usize = 116;
    pub const ROLLOVER_POSSIBILITY: usize = 120;
    pub const TIP: usize = 122;
    pub const ROLL_SAFETY_CONCERN: usize = 123;
    pub const ROLL_FOOT_NOTES: usize = 124;
    pub const ROLLOVER_STARS: usize = 125;
    pub const NHTSA_BACKUP_CAMERA: usize = 126;
}

/// The columns of notes, in the order their text is kept: what NHTSA says
/// about a test, and which vehicles a rating does not cover.
const NOTES: [usize; 12] = [
    at::FRNT_SAFETY_CONCERN_DRIV,
    at::FRNT_SAFETY_CONCERN_PASS,
    at::FRNT_FOOT_NOTES,
    at::FRNT_FOOT_NOTES_PASS,
    at::SIDE_SAFETY_CONCERN_DRIV,
    at::SIDE_SAFETY_CONCERN_PASS,
    at::SIDE_FOOT_NOTES,
    at::SIDE_FOOT_NOTES_PASS,
    at::POLE_SAFETY_CONCERN,
    at::POLE_FOOT_NOTES,
    at::ROLL_SAFETY_CONCERN,
    at::ROLL_FOOT_NOTES,
];

/// What separates the notes of one rating where they are stored.
pub const NOTE_SEPARATOR: &str = "\n";

/// The stars of one rating, each from 1 to 5. `None` where NHTSA gave none.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
pub struct Stars {
    /// `OVERALL_STARS`.
    pub overall: Option<u8>,
    /// `OVERALL_FRNT_STARS`, `FRNT_DRIV_STARS` and `FRNT_PASS_STARS`.
    pub frontal: Option<u8>,
    pub frontal_driver: Option<u8>,
    pub frontal_passenger: Option<u8>,
    /// `OVERALL_SIDE_STARS`, `SIDE_DRIV_STARS` and `SIDE_PASS_STARS`.
    pub side: Option<u8>,
    pub side_driver: Option<u8>,
    pub side_passenger: Option<u8>,
    /// `SIDE_BARRIER_STAR`, `COMB_FRNT_STAR`, `COMB_REAR_STAR` and
    /// `SIDE_POLE_STARS`.
    pub side_barrier: Option<u8>,
    pub side_front_seat: Option<u8>,
    pub side_rear_seat: Option<u8>,
    pub side_pole: Option<u8>,
    /// `ROLLOVER_STARS`.
    pub rollover: Option<u8>,
}

/// Whether a vehicle has a feature: `standard`, `optional`, `both` (on
/// some versions standard and on others optional) or `no`. `None` where
/// the file does not say, or says it in words this build does not read.
pub type Fitted = Option<&'static str>;

/// The safety features of one rating.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
pub struct Features {
    /// `NHTSA_ESC`: electronic stability control.
    pub esc: Fitted,
    /// `NHTSA_FRNT_COLLISION_WARNING` and `NHTSA_FCW_EVALUATION`.
    pub forward_collision_warning: Fitted,
    pub forward_collision_warning_evaluation: Option<&'static str>,
    /// `NHTSA_LANE_DEPARTURE_WARNING` and `NHTSA_LDW_EVALUATION`.
    pub lane_departure_warning: Fitted,
    pub lane_departure_warning_evaluation: Option<&'static str>,
    /// `NHTSA_CRASH_IMMINENT_BRAKE` and `NHTSA_CIB_EVALUATION`.
    pub crash_imminent_braking: Fitted,
    pub crash_imminent_braking_evaluation: Option<&'static str>,
    /// `NHTSA_DYNAMIC_BRAKE_SUPPORT` and `NHTSA_DBS_EVALUATION`.
    pub dynamic_brake_support: Fitted,
    pub dynamic_brake_support_evaluation: Option<&'static str>,
    /// `NHTSA_BACKUP_CAMERA`.
    pub backup_camera: Fitted,
    /// `BLIND_SPOT_DETECTION`.
    pub blind_spot_detection: Fitted,
    /// `HEAD_SAB` and `TORSO_SAB`: side air bags for the head and the torso.
    pub head_side_airbag: Fitted,
    pub torso_side_airbag: Fitted,
}

/// One row of the file: one vehicle as NHTSA rated it.
#[derive(Debug, Clone, Default, PartialEq, Eq, Hash)]
pub struct RatingRow {
    /// `MAKE`, `MODEL` and `MODEL_YR`.
    pub make_text: String,
    pub model_text: String,
    pub year: i32,
    /// `BODY_STYLE`, such as `4 DR`, `SUV` or `PU/CC`.
    pub body_style: Option<String>,
    /// `VEHICLE_TYPE`, such as `PC`, `MPV` or `TRUCK`.
    pub vehicle_type: Option<String>,
    /// `DRIVE_TRAIN`, such as `FWD`, `AWD` or `4x4`.
    pub drive: Option<String>,
    /// `PRODUCTION_RELEASE`: 1 for the first release of a model year, 2
    /// and 3 for a later one that NHTSA rated again.
    pub release: u8,
    pub stars: Stars,
    /// `ROLLOVER_POSSIBILITY`: the chance of a rollover in a single-vehicle
    /// crash, in thousandths. 191 is 19.1%. `None` for 0.
    pub rollover_risk_permille: Option<u32>,
    /// `TIP`: whether the vehicle tipped up in the dynamic test.
    pub rollover_tip: Option<bool>,
    /// `CURB_WEIGHT`: the weight of the tested vehicle, in pounds.
    pub curb_weight_lb: Option<u32>,
    pub features: Features,
    /// The safety concerns and footnotes of the tests, each once.
    pub notes: Vec<String>,
}

/// What `parse` read.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct RatingCounts {
    /// Rows of the file.
    pub records: u64,
    /// Of those, the rows with at least one star.
    pub rated: u64,
}

/// A text field as one line: line ends, tabs and other control characters
/// become a blank, runs of blanks become one, and the ends are trimmed.
/// `None` when nothing is left.
pub fn clean(field: &str) -> Option<String> {
    let mut tidy = String::with_capacity(field.len());
    for word in field
        .split(|character: char| {
            character.is_whitespace() || character.is_control() || character == '\u{a0}'
        })
        .filter(|word| !word.is_empty())
    {
        if !tidy.is_empty() {
            tidy.push(' ');
        }
        tidy.push_str(word);
    }
    (!tidy.is_empty()).then_some(tidy)
}

/// A star rating: one digit from 1 to 5.
pub fn stars(field: &str) -> Option<u8> {
    field
        .trim()
        .parse()
        .ok()
        .filter(|stars| (1..=5).contains(stars))
}

/// Whether a feature is fitted, from the words the file uses for it. Case
/// and the blanks at the ends do not matter. Anything not listed here,
/// `Not Available` and `TBD` among it, is `None`.
pub fn fitted(field: &str) -> Fitted {
    match field.trim().to_ascii_lowercase().as_str() {
        "standard" => Some("standard"),
        "optional" => Some("optional"),
        "standard & optional" | "standard/optional" | "both" => Some("both"),
        "no" | "none" | "not equipped" => Some("no"),
        _ => None,
    }
}

/// What NHTSA found when it tested a driver-assistance feature: `meets`,
/// `does_not_meet`, `standard_only`, `upgrade_only` or `pending`.
pub fn evaluation(field: &str) -> Option<&'static str> {
    match field.trim().to_ascii_lowercase().as_str() {
        "meets performance criteria" => Some("meets"),
        "does not meet performance criteria" | "failed test" => Some("does_not_meet"),
        "only standard meets performance criteria" => Some("standard_only"),
        "only upgrade meets performance criteria" | "only upgrade passed" => Some("upgrade_only"),
        "test pending" | "testing pending" => Some("pending"),
        _ => None,
    }
}

/// `ROLLOVER_POSSIBILITY`, written as a share such as `0.191`, in
/// thousandths. `None` for 0, for nothing and for a share over 1.
pub fn permille(field: &str) -> Option<u32> {
    let share: f64 = field.trim().parse().ok()?;
    (share > 0.0 && share <= 1.0).then(|| (share * 1000.0).round() as u32)
}

fn tip(field: &str) -> Option<bool> {
    match field.trim().to_ascii_lowercase().as_str() {
        "tip" => Some(true),
        "no tip" | "n" => Some(false),
        _ => None,
    }
}

fn required(record: &csv::StringRecord, index: usize) -> Result<String> {
    clean(&record[index]).with_context(|| format!("{} is empty", HEADER[index]))
}

/// Reads the kept fields of one row.
fn read_row(record: &csv::StringRecord) -> Result<RatingRow> {
    let year = record[at::MODEL_YR]
        .trim()
        .parse::<i32>()
        .ok()
        .filter(|year| (1950..=2100).contains(year))
        .with_context(|| format!("MODEL_YR {:?} is not a model year", &record[at::MODEL_YR]))?;
    let release = record[at::PRODUCTION_RELEASE]
        .trim()
        .parse::<u8>()
        .ok()
        .filter(|release| (1..=9).contains(release))
        .with_context(|| {
            format!(
                "PRODUCTION_RELEASE {:?} is not a number from 1 to 9",
                &record[at::PRODUCTION_RELEASE]
            )
        })?;
    let mut notes: Vec<String> = Vec::new();
    for index in NOTES {
        if let Some(note) = clean(&record[index])
            && !notes.contains(&note)
        {
            notes.push(note);
        }
    }
    Ok(RatingRow {
        make_text: required(record, at::MAKE)?,
        model_text: required(record, at::MODEL)?,
        year,
        body_style: clean(&record[at::BODY_STYLE]),
        vehicle_type: clean(&record[at::VEHICLE_TYPE]),
        drive: clean(&record[at::DRIVE_TRAIN]),
        release,
        stars: Stars {
            overall: stars(&record[at::OVERALL_STARS]),
            frontal: stars(&record[at::OVERALL_FRNT_STARS]),
            frontal_driver: stars(&record[at::FRNT_DRIV_STARS]),
            frontal_passenger: stars(&record[at::FRNT_PASS_STARS]),
            side: stars(&record[at::OVERALL_SIDE_STARS]),
            side_driver: stars(&record[at::SIDE_DRIV_STARS]),
            side_passenger: stars(&record[at::SIDE_PASS_STARS]),
            side_barrier: stars(&record[at::SIDE_BARRIER_STAR]),
            side_front_seat: stars(&record[at::COMB_FRNT_STAR]),
            side_rear_seat: stars(&record[at::COMB_REAR_STAR]),
            side_pole: stars(&record[at::SIDE_POLE_STARS]),
            rollover: stars(&record[at::ROLLOVER_STARS]),
        },
        rollover_risk_permille: permille(&record[at::ROLLOVER_POSSIBILITY]),
        rollover_tip: tip(&record[at::TIP]),
        curb_weight_lb: record[at::CURB_WEIGHT]
            .trim()
            .parse()
            .ok()
            .filter(|pounds| (500..=20_000).contains(pounds)),
        features: Features {
            esc: fitted(&record[at::NHTSA_ESC]),
            forward_collision_warning: fitted(&record[at::NHTSA_FRNT_COLLISION_WARNING]),
            forward_collision_warning_evaluation: evaluation(&record[at::NHTSA_FCW_EVALUATION]),
            lane_departure_warning: fitted(&record[at::NHTSA_LANE_DEPARTURE_WARNING]),
            lane_departure_warning_evaluation: evaluation(&record[at::NHTSA_LDW_EVALUATION]),
            crash_imminent_braking: fitted(&record[at::NHTSA_CRASH_IMMINENT_BRAKE]),
            crash_imminent_braking_evaluation: evaluation(&record[at::NHTSA_CIB_EVALUATION]),
            dynamic_brake_support: fitted(&record[at::NHTSA_DYNAMIC_BRAKE_SUPPORT]),
            dynamic_brake_support_evaluation: evaluation(&record[at::NHTSA_DBS_EVALUATION]),
            backup_camera: fitted(&record[at::NHTSA_BACKUP_CAMERA]),
            blind_spot_detection: fitted(&record[at::BLIND_SPOT_DETECTION]),
            head_side_airbag: fitted(&record[at::HEAD_SAB]),
            torso_side_airbag: fitted(&record[at::TORSO_SAB]),
        },
        notes,
    })
}

impl RatingRow {
    /// Whether NHTSA gave this vehicle any star.
    pub fn rated(&self) -> bool {
        let stars = &self.stars;
        [
            stars.overall,
            stars.frontal,
            stars.frontal_driver,
            stars.frontal_passenger,
            stars.side,
            stars.side_driver,
            stars.side_passenger,
            stars.side_barrier,
            stars.side_front_seat,
            stars.side_rear_seat,
            stars.side_pole,
            stars.rollover,
        ]
        .iter()
        .any(Option::is_some)
    }
}

/// Reads CSV text and hands each row to `row`. `origin` names the file in
/// an error.
pub fn parse_reader(
    reader: impl io::Read,
    origin: &str,
    row: &mut dyn FnMut(RatingRow) -> Result<()>,
) -> Result<RatingCounts> {
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
    let mut counts = RatingCounts::default();
    let mut record = csv::StringRecord::new();
    while csv
        .read_record(&mut record)
        .with_context(|| format!("reading {origin}"))?
    {
        counts.records += 1;
        let line = record.position().map_or(0, csv::Position::line);
        let found = read_row(&record).with_context(|| format!("{origin} line {line}"))?;
        if found.rated() {
            counts.rated += 1;
        }
        row(found)?;
    }
    Ok(counts)
}

/// Reads the fetched `nhtsa-ratings` directory and hands every row to
/// `row`, in file order.
pub fn parse(
    directory: &Path,
    row: &mut dyn FnMut(RatingRow) -> Result<()>,
) -> Result<RatingCounts> {
    let path = directory.join(FILE);
    let file = File::open(&path).with_context(|| {
        format!(
            "opening {}; run open-data fetch nhtsa-ratings first",
            path.display()
        )
    })?;
    parse_reader(io::BufReader::new(file), FILE, row)
}

const INSERT_RATING: &str = "
INSERT INTO rating (
    id, year, make_text, model_text, body_style, vehicle_type, drive, release,
    overall, frontal, frontal_driver, frontal_passenger,
    side, side_driver, side_passenger, side_barrier, side_front_seat, side_rear_seat, side_pole,
    rollover, rollover_risk_permille, rollover_tip, curb_weight_lb,
    esc, fcw, fcw_evaluation, ldw, ldw_evaluation, cib, cib_evaluation, dbs, dbs_evaluation,
    backup_camera, blind_spot_detection, head_side_airbag, torso_side_airbag, notes
) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16, ?17, ?18, ?19,
          ?20, ?21, ?22, ?23, ?24, ?25, ?26, ?27, ?28, ?29, ?30, ?31, ?32, ?33, ?34, ?35, ?36, ?37)";

const INSERT_VEHICLE: &str = "
INSERT INTO rating_vehicle (
    rating_id, year, make_id, model_id, make_text, model_text, \"match\"
) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)";

/// Builds the `ratings` dataset from the fetched `nhtsa-ratings`
/// directory: one `rating` row for each distinct row of the file, in file
/// order, and one `rating_vehicle` row for each catalog model it matched,
/// or one with no model when it matched none.
pub fn shape(context: &mut ShapeContext<'_>, directory: &Path) -> Result<()> {
    let as_of = SourceRecord::read(directory)?.as_of;
    // The file holds a few rows twice over. A rating is stored once.
    let mut seen: HashSet<RatingRow> = HashSet::new();
    let mut ratings: Vec<RatingRow> = Vec::new();
    parse(directory, &mut |row| {
        if seen.insert(row.clone()) {
            ratings.push(row);
        }
        Ok(())
    })?;

    context.transaction.execute_batch(schema::RATINGS)?;
    let mut insert_rating = context.transaction.prepare_cached(INSERT_RATING)?;
    let mut insert_vehicle = context.transaction.prepare_cached(INSERT_VEHICLE)?;
    for (index, rating) in ratings.iter().enumerate() {
        let id = index as i64 + 1;
        let (stars, features) = (&rating.stars, &rating.features);
        let notes = rating.notes.join(NOTE_SEPARATOR);
        insert_rating
            .execute(params![
                id,
                rating.year,
                rating.make_text,
                rating.model_text,
                rating.body_style,
                rating.vehicle_type,
                rating.drive,
                rating.release,
                stars.overall,
                stars.frontal,
                stars.frontal_driver,
                stars.frontal_passenger,
                stars.side,
                stars.side_driver,
                stars.side_passenger,
                stars.side_barrier,
                stars.side_front_seat,
                stars.side_rear_seat,
                stars.side_pole,
                stars.rollover,
                rating.rollover_risk_permille,
                rating.rollover_tip,
                rating.curb_weight_lb,
                features.esc,
                features.forward_collision_warning,
                features.forward_collision_warning_evaluation,
                features.lane_departure_warning,
                features.lane_departure_warning_evaluation,
                features.crash_imminent_braking,
                features.crash_imminent_braking_evaluation,
                features.dynamic_brake_support,
                features.dynamic_brake_support_evaluation,
                features.backup_camera,
                features.blind_spot_detection,
                features.head_side_airbag,
                features.torso_side_airbag,
                (!notes.is_empty()).then_some(notes.as_str()),
            ])
            .with_context(|| {
                format!(
                    "storing the rating of the {} {} {}",
                    rating.year, rating.make_text, rating.model_text
                )
            })?;
        let resolved =
            context
                .matcher
                .resolve(Some(rating.year), &rating.make_text, &rating.model_text);
        let models: Vec<Option<i64>> = if resolved.model_ids.is_empty() {
            vec![None]
        } else {
            resolved.model_ids.iter().copied().map(Some).collect()
        };
        for model_id in models {
            insert_vehicle.execute(params![
                id,
                rating.year,
                resolved.make_id,
                model_id,
                rating.make_text,
                rating.model_text,
                model_id.and(resolved.match_text()),
            ])?;
        }
    }
    drop(insert_rating);
    drop(insert_vehicle);
    context.register(
        Dataset::Ratings,
        DatasetEntry {
            as_of,
            rows: ratings.len() as u64,
            licence: licence::PUBLIC_DOMAIN.to_owned(),
            attribution: String::new(),
        },
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    /// One row with `MAKE`, `MODEL`, `MODEL_YR` and `PRODUCTION_RELEASE`
    /// filled, and whatever `change` sets.
    fn line(change: impl FnOnce(&mut Vec<String>)) -> String {
        let mut fields: Vec<String> = vec![String::new(); HEADER.len()];
        for (index, value) in [
            (at::MAKE, "HONDA"),
            (at::MODEL, "CIVIC"),
            (at::MODEL_YR, "2019"),
            (at::PRODUCTION_RELEASE, "1"),
        ] {
            fields[index] = value.to_owned();
        }
        change(&mut fields);
        let mut writer = csv::Writer::from_writer(Vec::new());
        writer.write_record(&fields).unwrap();
        String::from_utf8(writer.into_inner().unwrap()).unwrap()
    }

    fn rows(body: &str) -> Result<(Vec<RatingRow>, RatingCounts)> {
        let mut found = Vec::new();
        let text = format!("{}\n{body}", HEADER.join(","));
        let counts = parse_reader(text.as_bytes(), "test.csv", &mut |row| {
            found.push(row);
            Ok(())
        })?;
        Ok((found, counts))
    }

    #[test]
    fn the_columns_read_are_where_the_header_names_them() {
        for (index, name) in [
            (at::MAKE, "MAKE"),
            (at::MODEL, "MODEL"),
            (at::MODEL_YR, "MODEL_YR"),
            (at::BODY_STYLE, "BODY_STYLE"),
            (at::VEHICLE_TYPE, "VEHICLE_TYPE"),
            (at::DRIVE_TRAIN, "DRIVE_TRAIN"),
            (at::PRODUCTION_RELEASE, "PRODUCTION_RELEASE"),
            (at::HEAD_SAB, "HEAD_SAB"),
            (at::TORSO_SAB, "TORSO_SAB"),
            (at::BLIND_SPOT_DETECTION, "BLIND_SPOT_DETECTION"),
            (
                at::NHTSA_FRNT_COLLISION_WARNING,
                "NHTSA_FRNT_COLLISION_WARNING",
            ),
            (at::NHTSA_FCW_EVALUATION, "NHTSA_FCW_EVALUATION"),
            (
                at::NHTSA_LANE_DEPARTURE_WARNING,
                "NHTSA_LANE_DEPARTURE_WARNING",
            ),
            (at::NHTSA_LDW_EVALUATION, "NHTSA_LDW_EVALUATION"),
            (at::NHTSA_CRASH_IMMINENT_BRAKE, "NHTSA_CRASH_IMMINENT_BRAKE"),
            (at::NHTSA_CIB_EVALUATION, "NHTSA_CIB_EVALUATION"),
            (
                at::NHTSA_DYNAMIC_BRAKE_SUPPORT,
                "NHTSA_DYNAMIC_BRAKE_SUPPORT",
            ),
            (at::NHTSA_DBS_EVALUATION, "NHTSA_DBS_EVALUATION"),
            (at::NHTSA_ESC, "NHTSA_ESC"),
            (at::OVERALL_STARS, "OVERALL_STARS"),
            (at::FRNT_DRIV_STARS, "FRNT_DRIV_STARS"),
            (at::FRNT_PASS_STARS, "FRNT_PASS_STARS"),
            (at::FRNT_SAFETY_CONCERN_DRIV, "FRNT_SAFETY_CONCERN_DRIV"),
            (at::FRNT_SAFETY_CONCERN_PASS, "FRNT_SAFETY_CONCERN_PASS"),
            (at::FRNT_FOOT_NOTES, "FRNT_FOOT_NOTES"),
            (at::FRNT_FOOT_NOTES_PASS, "FRNT_FOOT_NOTES_PASS"),
            (at::OVERALL_FRNT_STARS, "OVERALL_FRNT_STARS"),
            (at::CURB_WEIGHT, "CURB_WEIGHT"),
            (at::SIDE_DRIV_STARS, "SIDE_DRIV_STARS"),
            (at::SIDE_PASS_STARS, "SIDE_PASS_STARS"),
            (at::SIDE_BARRIER_STAR, "SIDE_BARRIER_STAR"),
            (at::COMB_FRNT_STAR, "COMB_FRNT_STAR"),
            (at::COMB_REAR_STAR, "COMB_REAR_STAR"),
            (at::SIDE_SAFETY_CONCERN_DRIV, "SIDE_SAFETY_CONCERN_DRIV"),
            (at::SIDE_SAFETY_CONCERN_PASS, "SIDE_SAFETY_CONCERN_PASS"),
            (at::SIDE_FOOT_NOTES, "SIDE_FOOT_NOTES"),
            (at::SIDE_FOOT_NOTES_PASS, "SIDE_FOOT_NOTES_PASS"),
            (at::OVERALL_SIDE_STARS, "OVERALL_SIDE_STARS"),
            (at::SIDE_POLE_STARS, "SIDE_POLE_STARS"),
            (at::POLE_SAFETY_CONCERN, "POLE_SAFETY_CONCERN"),
            (at::POLE_FOOT_NOTES, "POLE_FOOT_NOTES"),
            (at::ROLLOVER_POSSIBILITY, "ROLLOVER_POSSIBILITY"),
            (at::TIP, "TIP"),
            (at::ROLL_SAFETY_CONCERN, "ROLL_SAFETY_CONCERN"),
            (at::ROLL_FOOT_NOTES, "ROLL_FOOT_NOTES"),
            (at::ROLLOVER_STARS, "ROLLOVER_STARS"),
            (at::NHTSA_BACKUP_CAMERA, "NHTSA_BACKUP_CAMERA"),
        ] {
            assert_eq!(HEADER[index], name);
        }
        let mut names = HEADER.to_vec();
        names.sort_unstable();
        names.dedup();
        assert_eq!(names.len(), 128, "a column is named twice");
    }

    #[test]
    fn text_becomes_one_line() {
        assert_eq!(clean("  4 DR  ").as_deref(), Some("4 DR"));
        assert_eq!(
            clean("2 or 3- Frt\r\r\n2 or 3-\t2nd\u{a0}row").as_deref(),
            Some("2 or 3- Frt 2 or 3- 2nd row")
        );
        assert_eq!(clean(" \r\n "), None);
        assert_eq!(clean(""), None);
    }

    #[test]
    fn stars_are_one_to_five() {
        assert_eq!(stars("5"), Some(5));
        assert_eq!(stars(" 1 "), Some(1));
        for bad in ["", " ", "0", "6", "5.0", "five", "-1"] {
            assert_eq!(stars(bad), None, "{bad}");
        }
    }

    #[test]
    fn a_feature_is_read_from_the_words_the_file_uses() {
        for (field, expected) in [
            ("Standard", Some("standard")),
            (" optional ", Some("optional")),
            ("OPTIONAL", Some("optional")),
            ("Standard & Optional", Some("both")),
            ("Standard/Optional", Some("both")),
            ("Both", Some("both")),
            ("No", Some("no")),
            ("None", Some("no")),
            ("Not Equipped", Some("no")),
            ("", None),
            ("Not Available", None),
            ("N/A", None),
            ("TBD", None),
            ("Stanard", None),
            ("S", None),
            ("Yes", None),
            ("F = Standard; R = Optional", None),
        ] {
            assert_eq!(fitted(field), expected, "{field:?}");
        }
        for (field, expected) in [
            ("Meets Performance Criteria", Some("meets")),
            ("Meets Performance Criteria\n", Some("meets")),
            ("Does Not Meet Performance Criteria", Some("does_not_meet")),
            ("Failed Test", Some("does_not_meet")),
            (
                "Only Standard Meets Performance Criteria",
                Some("standard_only"),
            ),
            (
                "Only Upgrade Meets Performance Criteria",
                Some("upgrade_only"),
            ),
            ("Only Upgrade Passed", Some("upgrade_only")),
            ("Test Pending", Some("pending")),
            ("Testing Pending", Some("pending")),
            ("Test Results Not Available", None),
            ("", None),
        ] {
            assert_eq!(evaluation(field), expected, "{field:?}");
        }
    }

    #[test]
    fn the_rollover_share_is_kept_in_thousandths() {
        assert_eq!(permille("0.191"), Some(191));
        assert_eq!(permille("0.1"), Some(100));
        assert_eq!(permille("0.093"), Some(93));
        assert_eq!(permille("1"), Some(1000));
        for none in ["0", "", " ", "1.33", "-0.2", "x"] {
            assert_eq!(permille(none), None, "{none}");
        }
    }

    #[test]
    fn a_row_is_read_and_a_bad_one_is_an_error() {
        let text = format!(
            "{}{}",
            line(|fields| {
                fields[at::BODY_STYLE] = "4 DR".to_owned();
                fields[at::OVERALL_STARS] = "5".to_owned();
                fields[at::ROLLOVER_STARS] = "4".to_owned();
                fields[at::ROLLOVER_POSSIBILITY] = "0.102".to_owned();
                fields[at::TIP] = "No Tip".to_owned();
                fields[at::CURB_WEIGHT] = "2906".to_owned();
                fields[at::NHTSA_ESC] = "Standard".to_owned();
                fields[at::NHTSA_FCW_EVALUATION] = "Test Pending".to_owned();
                // A field of two lines, as the real file has.
                fields[at::FRNT_FOOT_NOTES] = "First note.\r\nStill the first.".to_owned();
                fields[at::SIDE_FOOT_NOTES] = "Second note.".to_owned();
                fields[at::POLE_FOOT_NOTES] = "Second note.".to_owned();
            }),
            line(|fields| {
                fields[at::CURB_WEIGHT] = "0".to_owned();
                fields[at::ROLLOVER_POSSIBILITY] = "0".to_owned();
                fields[at::PRODUCTION_RELEASE] = "2".to_owned();
            }),
        );
        let (found, counts) = rows(&text).unwrap();
        assert_eq!(
            counts,
            RatingCounts {
                records: 2,
                rated: 1
            }
        );
        let first = &found[0];
        assert_eq!(
            (
                first.make_text.as_str(),
                first.model_text.as_str(),
                first.year
            ),
            ("HONDA", "CIVIC", 2019)
        );
        assert_eq!(first.body_style.as_deref(), Some("4 DR"));
        assert_eq!(first.drive, None);
        assert_eq!(first.release, 1);
        assert_eq!(first.stars.overall, Some(5));
        assert_eq!(first.stars.rollover, Some(4));
        assert_eq!(first.stars.frontal, None);
        assert_eq!(first.rollover_risk_permille, Some(102));
        assert_eq!(first.rollover_tip, Some(false));
        assert_eq!(first.curb_weight_lb, Some(2906));
        assert_eq!(first.features.esc, Some("standard"));
        assert_eq!(
            first.features.forward_collision_warning_evaluation,
            Some("pending")
        );
        // Each note once, each on one line.
        assert_eq!(
            first.notes,
            ["First note. Still the first.", "Second note."]
        );
        assert!(first.rated());

        let second = &found[1];
        assert_eq!(second.release, 2);
        assert_eq!(second.curb_weight_lb, None);
        assert_eq!(second.rollover_risk_permille, None);
        assert_eq!(second.rollover_tip, None);
        assert!(second.notes.is_empty());
        assert!(!second.rated());

        let no_year = line(|fields| fields[at::MODEL_YR] = "TBD".to_owned());
        assert_eq!(
            format!("{:#}", rows(&no_year).unwrap_err()),
            "test.csv line 2: MODEL_YR \"TBD\" is not a model year"
        );
        let no_model = line(|fields| fields[at::MODEL] = " ".to_owned());
        assert_eq!(
            format!("{:#}", rows(&no_model).unwrap_err()),
            "test.csv line 2: MODEL is empty"
        );
        let no_release = line(|fields| fields[at::PRODUCTION_RELEASE] = String::new());
        assert_eq!(
            format!("{:#}", rows(&no_release).unwrap_err()),
            "test.csv line 2: PRODUCTION_RELEASE \"\" is not a number from 1 to 9"
        );
    }

    #[test]
    fn a_file_with_other_columns_is_refused() {
        let mut names = HEADER.to_vec();
        names.push("NEW_COLUMN");
        let text = format!("{}\n", names.join(","));
        let error = parse_reader(text.as_bytes(), "test.csv", &mut |_| Ok(()))
            .unwrap_err()
            .to_string();
        assert!(
            error.starts_with(
                "test.csv: the columns are not the ones this build reads: found 129 (MAKE, MODEL, "
            ),
            "{error}"
        );
        assert!(error.contains("expected 128 (MAKE, MODEL, "), "{error}");
    }
}
