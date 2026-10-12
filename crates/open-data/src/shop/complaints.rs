//! NHTSA's owner complaints, reduced to counts.
//!
//! The file holds what owners wrote, where they live and part of each
//! VIN. None of that is read into a row here, and none of it is stored:
//! `parse` hands on the vehicle, the component, the mileage and three
//! flags, and `shape` stores only how many complaints a model year has
//! under each component.

use std::collections::{BTreeMap, HashMap, HashSet};
use std::fs::File;
use std::io::{BufRead, BufReader};
use std::path::Path;

use anyhow::{Context, Result, bail};
use rusqlite::params;
use wenmar_shopdata::{Dataset, DatasetEntry, licence, schema};

use crate::shop::build::ShapeContext;
use crate::shop::components::Components;
use crate::shop::matcher::NO_YEAR;
use crate::shop::nhtsa_recalls::{layout, tidy};
use crate::sources::SourceRecord;

/// The archive, and NHTSA's description of its fields.
pub const FILE: &str = "FLAT_CMPL.zip";
pub const LAYOUT_FILE: &str = "CMPL.txt";

/// The 51 fields of a record, in order, as `CMPL.txt` names them.
/// `OCCURENCES` is NHTSA's spelling.
pub const FIELDS: [&str; 51] = [
    "CMPLID",
    "ODINO",
    "MFR_NAME",
    "MAKETXT",
    "MODELTXT",
    "YEARTXT",
    "CRASH",
    "FAILDATE",
    "FIRE",
    "INJURED",
    "DEATHS",
    "COMPDESC",
    "CITY",
    "STATE",
    "VIN",
    "DATEA",
    "LDATE",
    "MILES",
    "OCCURENCES",
    "CDESCR",
    "CMPL_TYPE",
    "POLICE_RPT_YN",
    "PURCH_DT",
    "ORIG_OWNER_YN",
    "ANTI_BRAKES_YN",
    "CRUISE_CONT_YN",
    "NUM_CYLS",
    "DRIVE_TRAIN",
    "FUEL_SYS",
    "FUEL_TYPE",
    "TRANS_TYPE",
    "VEH_SPEED",
    "DOT",
    "TIRE_SIZE",
    "LOC_OF_TIRE",
    "TIRE_FAIL_TYPE",
    "ORIG_EQUIP_YN",
    "MANUF_DT",
    "SEAT_TYPE",
    "RESTRAINT_TYPE",
    "DEALER_NAME",
    "DEALER_TEL",
    "DEALER_CITY",
    "DEALER_STATE",
    "DEALER_ZIP",
    "PROD_TYPE",
    "REPAIRED_YN",
    "MEDICAL_ATTN",
    "VEHICLES_TOWED_YN",
    "STATE_OF_INCIDENT",
    "VEHICLE_OPERATOR",
];

/// The fields of a record that this build reads, counted from 0. Every
/// other field is skipped unread: the owner's words, city and state, the
/// VIN, the dealer and the operator's name among them.
pub const READ: [usize; 11] = [0, 1, 3, 4, 5, 6, 8, 9, 11, 17, 45];

/// The value of `PROD_TYPE` that marks a vehicle. The others are `T`
/// (tires), `E` (equipment) and `C` (child restraints).
pub const VEHICLE: &str = "V";

/// The most miles a complaint may give and be believed.
pub const MOST_MILES: u32 = 999_999;

/// One record: one complaint about one vehicle under one component. A
/// complaint that names three components has three records with the same
/// `odino`. Nothing an owner wrote is in it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ComplaintRow {
    /// Field 1, `CMPLID`.
    pub record_id: u32,
    /// Field 2, `ODINO`: the complaint's number.
    pub odino: u32,
    /// Fields 4 and 5, `MAKETXT` and `MODELTXT`.
    pub make_text: String,
    pub model_text: String,
    /// Field 6, `YEARTXT`. `None` for `9999`.
    pub year: Option<i32>,
    /// Fields 7 and 9, `CRASH` and `FIRE`: `Y` or `N`.
    pub crash: bool,
    pub fire: bool,
    /// Field 10, `INJURED`: persons injured.
    pub injured: u32,
    /// Field 12, `COMPDESC`: one NHTSA component path.
    pub component: Option<String>,
    /// Field 18, `MILES`: the mileage at failure. `None` for 0, which is
    /// what the file holds when the owner gave none, and for a figure over
    /// [`MOST_MILES`].
    pub miles: Option<u32>,
}

/// What `parse` read.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct ComplaintCounts {
    /// Lines read.
    pub records: u64,
    /// Of those, the vehicle records handed on.
    pub vehicles: u64,
    /// Of those, the tire, equipment and child restraint records left out.
    pub other_types: u64,
}

/// Refuses a copy of `CMPL.txt` whose fields are not [`FIELDS`].
pub fn check_layout(cmpl_txt: &str) -> Result<()> {
    let found = layout(cmpl_txt);
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

fn flag(text: &str) -> bool {
    text.trim().eq_ignore_ascii_case("y")
}

/// Reads the fields of [`READ`] from one vehicle record.
fn read_row(fields: &[&str]) -> Result<ComplaintRow> {
    let number = |index: usize| -> Result<u32> {
        fields[index]
            .trim()
            .parse()
            .with_context(|| format!("{} {:?} is not a number", FIELDS[index], fields[index]))
    };
    let year = match fields[5].trim().parse::<i32>() {
        Ok(year) if year != NO_YEAR && (1900..=2100).contains(&year) => Some(year),
        _ => None,
    };
    Ok(ComplaintRow {
        record_id: number(0)?,
        odino: number(1)?,
        make_text: tidy(fields[3]).unwrap_or_default(),
        model_text: tidy(fields[4]).unwrap_or_default(),
        year,
        crash: flag(fields[6]),
        fire: flag(fields[8]),
        injured: fields[9].trim().parse().unwrap_or(0),
        component: tidy(fields[11]),
        miles: fields[17]
            .trim()
            .parse::<u32>()
            .ok()
            .filter(|miles| (1..=MOST_MILES).contains(miles)),
    })
}

/// Reads the file's text, a line at a time, and hands each vehicle record
/// to `row`. `origin` names the file in an error.
pub fn parse_reader(
    mut reader: impl BufRead,
    origin: &str,
    counts: &mut ComplaintCounts,
    row: &mut dyn FnMut(ComplaintRow) -> Result<()>,
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
        if fields[45].trim() != VEHICLE {
            counts.other_types += 1;
            continue;
        }
        counts.vehicles += 1;
        row(read_row(&fields).with_context(|| format!("{origin} line {number}"))?)?;
    }
}

/// Reads the fetched `nhtsa-complaints` directory: checks `CMPL.txt`, then
/// hands every vehicle record of the archive to `row`, in file order.
pub fn parse(
    directory: &Path,
    row: &mut dyn FnMut(ComplaintRow) -> Result<()>,
) -> Result<ComplaintCounts> {
    let layout_path = directory.join(LAYOUT_FILE);
    let layout_bytes = std::fs::read(&layout_path).with_context(|| {
        format!(
            "reading {}; run open-data fetch nhtsa-complaints first",
            layout_path.display()
        )
    })?;
    check_layout(&String::from_utf8_lossy(&layout_bytes))
        .with_context(|| layout_path.display().to_string())?;
    let path = directory.join(FILE);
    let file = File::open(&path).with_context(|| {
        format!(
            "opening {}; run open-data fetch nhtsa-complaints first",
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
    let mut counts = ComplaintCounts::default();
    parse_reader(BufReader::new(entry), FILE, &mut counts, row)?;
    Ok(counts)
}

/// The `component_id` of the row that counts every complaint of a model
/// year, whatever its component.
pub const ALL_COMPONENTS: &str = "";

/// The middle of a list of mileages: for an even number, the lower of the
/// two in the middle. `None` for an empty list.
pub fn median(miles: &mut [u32]) -> Option<u32> {
    if miles.is_empty() {
        return None;
    }
    miles.sort_unstable();
    Some(miles[(miles.len() - 1) / 2])
}

/// What is known of the complaints of one model year under one component.
#[derive(Default)]
struct Tally {
    complaints: u32,
    crashes: u32,
    fires: u32,
    injuries: u32,
    miles: Vec<u32>,
}

const INSERT: &str = "
INSERT INTO complaint_count (make_id, model_id, year, component_id, complaints, crashes, fires, injuries, with_miles, median_miles)
VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)";

/// Builds the `complaints` dataset from the fetched `nhtsa-complaints`
/// directory: for each catalog model year and each component of
/// `components.yaml`, how many complaints there are, how many of them
/// report a crash, a fire or an injury, and the median mileage at failure.
/// A complaint is counted once under each component it names and once in
/// the row for all components. A vehicle that matched no catalog model is
/// not counted.
pub fn shape(context: &mut ShapeContext<'_>, directory: &Path) -> Result<()> {
    let as_of = SourceRecord::read(directory)?.as_of;
    let components = Components::load(context.curated)?;
    // Component text to the ids it is about, each text looked up once.
    let mut component_ids: HashMap<String, Vec<u16>> = HashMap::new();
    let mut ids: Vec<String> = vec![ALL_COMPONENTS.to_owned()];
    // Make and model text and year to the catalog models matched.
    let mut vehicles: HashMap<(String, String, i32), Vec<(i64, i64)>> = HashMap::new();
    // A complaint is counted once for a model year and component.
    let mut seen: HashSet<(u32, i64, i32, u16)> = HashSet::new();
    let mut tallies: BTreeMap<(i64, i64, i32, u16), Tally> = BTreeMap::new();
    let (mut matched, mut unmatched) = (0u64, 0u64);
    let matcher = context.matcher;
    parse(directory, &mut |row| {
        let Some(year) = row.year else {
            unmatched += 1;
            return Ok(());
        };
        let key = (row.make_text, row.model_text, year);
        let models = match vehicles.get(&key) {
            Some(models) => models,
            None => {
                let resolved = matcher.resolve(Some(year), &key.0, &key.1);
                let models: Vec<(i64, i64)> = match resolved.make_id {
                    Some(make_id) => resolved
                        .model_ids
                        .iter()
                        .map(|model_id| (make_id, *model_id))
                        .collect(),
                    None => Vec::new(),
                };
                vehicles.entry(key).or_insert(models)
            }
        };
        if models.is_empty() {
            unmatched += 1;
            return Ok(());
        }
        matched += 1;
        let mut under: Vec<u16> = vec![0];
        if let Some(text) = &row.component {
            if !component_ids.contains_key(text) {
                let mut found = Vec::new();
                for id in components.for_nhtsa(text) {
                    let index = match ids.iter().position(|known| known == id) {
                        Some(index) => index,
                        None => {
                            ids.push(id.to_owned());
                            ids.len() - 1
                        }
                    };
                    found.push(index as u16);
                }
                component_ids.insert(text.clone(), found);
            }
            under.extend(&component_ids[text]);
        }
        for (make_id, model_id) in models {
            for component in &under {
                if !seen.insert((row.odino, *model_id, year, *component)) {
                    continue;
                }
                let tally = tallies
                    .entry((*make_id, *model_id, year, *component))
                    .or_default();
                tally.complaints += 1;
                tally.crashes += u32::from(row.crash);
                tally.fires += u32::from(row.fire);
                tally.injuries += u32::from(row.injured > 0);
                if let Some(miles) = row.miles {
                    tally.miles.push(miles);
                }
            }
        }
        Ok(())
    })?;

    let transaction = context.transaction;
    transaction.execute_batch(schema::COMPLAINTS)?;
    let mut insert = transaction.prepare(INSERT)?;
    let rows = tallies.len() as u64;
    for ((make_id, model_id, year, component), mut tally) in tallies {
        let with_miles = tally.miles.len() as u32;
        insert.execute(params![
            make_id,
            model_id,
            year,
            ids[component as usize],
            tally.complaints,
            tally.crashes,
            tally.fires,
            tally.injuries,
            with_miles,
            median(&mut tally.miles),
        ])?;
    }
    drop(insert);
    eprintln!(
        "complaints: {matched} vehicle records matched a catalog model year, {unmatched} did not"
    );
    context.register(
        Dataset::Complaints,
        DatasetEntry {
            as_of,
            rows,
            licence: licence::PUBLIC_DOMAIN.to_owned(),
            attribution: String::new(),
        },
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn line(change: impl FnOnce(&mut Vec<String>)) -> String {
        let mut fields: Vec<String> = vec![String::new(); 51];
        for (index, value) in [
            (0, "1347214"),
            (1, "10943869"),
            (3, "HONDA"),
            (4, "CIVIC"),
            (5, "2016"),
            (6, "N"),
            (8, "N"),
            (9, "0"),
            (11, "STEERING"),
            (12, "SOMEWHERE"),
            (14, "19XFC2F5XGE"),
            (17, "45000"),
            (19, "WHAT THE OWNER WROTE"),
            (45, "V"),
        ] {
            fields[index] = value.to_owned();
        }
        change(&mut fields);
        fields.join("\t")
    }

    fn rows(text: &str) -> Result<(Vec<ComplaintRow>, ComplaintCounts)> {
        let mut found = Vec::new();
        let mut counts = ComplaintCounts::default();
        parse_reader(text.as_bytes(), "test.txt", &mut counts, &mut |row| {
            found.push(row);
            Ok(())
        })?;
        Ok((found, counts))
    }

    #[test]
    fn a_line_is_a_vehicle_record_another_product_or_an_error() {
        let text = format!(
            "{}\n{}\r\n{}\n",
            line(|_| {}),
            line(|fields| fields[45] = "T".to_owned()),
            line(|fields| {
                fields[5] = "9999".to_owned();
                fields[6] = "Y".to_owned();
                fields[8] = "y".to_owned();
                fields[9] = "2".to_owned();
                fields[11] = String::new();
                fields[17] = "0".to_owned();
            }),
        );
        let (found, counts) = rows(&text).unwrap();
        assert_eq!(
            counts,
            ComplaintCounts {
                records: 3,
                vehicles: 2,
                other_types: 1
            }
        );
        assert_eq!(found[0].odino, 10_943_869);
        assert_eq!(found[0].year, Some(2016));
        assert_eq!(found[0].miles, Some(45_000));
        assert!(!found[0].crash && !found[0].fire);
        assert_eq!(found[1].year, None);
        assert!(found[1].crash && found[1].fire);
        assert_eq!(found[1].injured, 2);
        assert_eq!(found[1].component, None);
        // 0 is how the file writes "not given".
        assert_eq!(found[1].miles, None);
        // A mileage nobody would believe is not kept.
        let far = line(|fields| fields[17] = "9999999".to_owned());
        assert_eq!(rows(&far).unwrap().0[0].miles, None);
        // What the owner wrote is in no field of the row.
        let debug = format!("{found:?}");
        assert!(
            !debug.contains("OWNER") && !debug.contains("SOMEWHERE") && !debug.contains("19XFC")
        );

        assert_eq!(
            format!("{:#}", rows("1\t2\t3\n").unwrap_err()),
            "test.txt line 1 has 3 fields, expected 51"
        );
        let bad = line(|fields| fields[1] = "x".to_owned());
        assert!(
            format!("{:#}", rows(&bad).unwrap_err())
                .starts_with("test.txt line 1: ODINO \"x\" is not a number"),
        );
    }

    #[test]
    fn the_median_is_the_middle_mileage() {
        assert_eq!(median(&mut []), None);
        assert_eq!(median(&mut [7]), Some(7));
        assert_eq!(median(&mut [9, 1, 5]), Some(5));
        // An even number: the lower of the two in the middle.
        assert_eq!(median(&mut [40, 10, 30, 20]), Some(20));
    }
}
