use std::collections::BTreeMap;
use std::path::Path;

use anyhow::Result;
use open_data::shop::epa_guide::{
    COLUMNS, FIRST_YEAR, GuideCounts, GuideJoin, GuideRow, fetch_with, parse, parse_sheet,
};
use open_data::shop::epa_vehicles::{self, EpaVehicleRow};
use open_data::shop::oil_grade::{self, CuratedOil};
use open_data::shop::xlsx;
use open_data::sources::{Downloaded, SourceRecord};

use crate::common::{Scratch, epa_guide_dir, epa_vehicles_dir, xlsx_bytes};

fn read(directory: &Path) -> Result<(Vec<GuideRow>, GuideCounts)> {
    let mut rows = Vec::new();
    let counts = parse(directory, &mut |row| {
        rows.push(row);
        Ok(())
    })?;
    Ok((rows, counts))
}

fn vehicles(directory: &Path) -> Vec<EpaVehicleRow> {
    let mut rows = Vec::new();
    epa_vehicles::parse(directory, &mut |row| {
        rows.push(row);
        Ok(())
    })
    .unwrap();
    rows
}

fn cells(rows: &[&[&str]]) -> Vec<Vec<String>> {
    rows.iter()
        .map(|row| row.iter().map(|cell| (*cell).to_owned()).collect())
        .collect()
}

#[test]
fn a_workbook_reads_back_cell_for_cell() {
    let rows = cells(&[
        &["Model Year", "Division", "", "Oil Viscosity"],
        &["2018", "Ford", "", "5W-30 & <GF-5>"],
        &["", "", "", "0W20"],
    ]);
    assert_eq!(
        xlsx::first_sheet(&xlsx_bytes(&rows)).unwrap(),
        cells(&[
            &["Model Year", "Division", "", "Oil Viscosity"],
            &["2018", "Ford", "", "5W-30 & <GF-5>"],
            &["", "", "", "0W20"],
        ])
    );
}

#[test]
fn the_fixture_files_read_whole() {
    let scratch = Scratch::new("epa-guide-rows");
    let (rows, counts) = read(&epa_guide_dir(&scratch)).unwrap();
    assert_eq!(
        counts,
        GuideCounts {
            files: 2,
            rows: 7,
            skipped: 0,
            // The fixture leaves out the stop-start column.
            missing: vec![(2018, "stop_start"), (2020, "stop_start")],
        }
    );
    // Oldest year first, rows in sheet order.
    let keys: Vec<(i32, &str, u32)> = rows
        .iter()
        .map(|row| (row.year, row.mfr_code.as_str(), row.index))
        .collect();
    assert_eq!(
        keys,
        [
            (2018, "FMX", 2),
            (2018, "FMX", 5),
            (2018, "FMX", 15),
            (2018, "FMX", 25),
            (2018, "HNX", 20),
            (2020, "HNX", 39),
            (2020, "GMX", 550),
        ]
    );
    assert_eq!(
        rows[3],
        GuideRow {
            year: 2018,
            mfr_code: "FMX".to_owned(),
            division: "Ford".to_owned(),
            carline: "F150 PICKUP 2WD".to_owned(),
            index: 25,
            displacement_l: Some(2.7),
            cylinders: Some(6),
            transmission: "Auto(S10)".to_owned(),
            aspiration: Some("Turbocharged".to_owned()),
            transmission_type: Some("Semi-Automatic".to_owned()),
            gears: Some(10),
            drive: Some("2-Wheel Drive, Rear".to_owned()),
            fuel_usage: Some("Gasoline (Regular Unleaded Recommended)".to_owned()),
            intake_valves: Some(2),
            exhaust_valves: Some(2),
            cylinder_deactivation: Some(false),
            variable_valve_timing: Some(true),
            variable_valve_lift: Some(false),
            oil_text: Some("5W-30 GF-5".to_owned()),
            ..GuideRow::default()
        }
    );
    // A hatchback's volumes.
    assert_eq!(rows[4].passenger_ft3_hatch, Some(97));
    assert_eq!(rows[4].luggage_ft3_hatch, Some(26));
    assert_eq!(rows[4].passenger_ft3_4door, None);
    // A hybrid: battery and motor. A whole number of litres is written `2`.
    let hybrid = &rows[5];
    assert_eq!(hybrid.displacement_l, Some(2.0));
    assert_eq!(hybrid.battery_type.as_deref(), Some("Lithium Ion"));
    assert_eq!(hybrid.battery_voltage, Some(259));
    assert_eq!(hybrid.motor_kw.as_deref(), Some("135"));
    assert_eq!(hybrid.gears, Some(1));
    assert_eq!(hybrid.passenger_ft3_4door, Some(106));
    // Cylinder deactivation with its name.
    let truck = &rows[6];
    assert_eq!(truck.cylinder_deactivation, Some(true));
    assert_eq!(
        truck.cylinder_deactivation_name.as_deref(),
        Some("Active Fuel Management")
    );
    assert_eq!(truck.oil_text.as_deref(), Some("0W20"));
}

#[test]
fn columns_are_found_by_name_wherever_they_are() {
    let mut counts = GuideCounts::default();
    let mut rows = Vec::new();
    // Another order, an older header for the year, padding in a header,
    // and columns the parser does not read.
    let sheet = cells(&[
        &[
            "Oil Viscosity",
            "Unread",
            "# Cyl",
            " Eng  Displ ",
            "Index (Model Type Index)",
            "Carline",
            "Division",
            "Verify Mfr Cd",
            "Model Yr",
            "Trans in FE Guide (MFR entered for data entered after May 13 2011)",
            "Cyl Deact?",
            "Cyl Deact Desc",
            "Battery Type Desc",
        ],
        &[
            "SAE 5W-30",
            "x",
            "8",
            "4.4000000000000004",
            "7",
            "X5",
            "BMW",
            "BMX",
            "2012",
            "Auto(S8)",
            "Y",
            "Deactivation of cylinders 2, 3, 5 and 8 by setting the inlet and exhaust valves to zero lift",
            "Other",
        ],
        &["", "", "", "", "", "", "", "", "", "", "", "", ""],
        &[
            "0W20", "", "4", "2", "", "Sum of", "", "", "", "", "N", "N.A.", "48",
        ],
        &[
            "", "", "", "", "9", "i3", "BMW", "BMX", "", "Auto(A1)", "N", "MDS", "NiMH",
        ],
    ]);
    parse_sheet(&sheet, 2012, &mut counts, &mut |row| {
        rows.push(row);
        Ok(())
    })
    .unwrap();
    assert_eq!((counts.rows, counts.skipped), (2, 2));
    assert_eq!(rows[0].year, 2012);
    assert_eq!(rows[0].displacement_l, Some(4.4));
    assert_eq!(rows[0].transmission, "Auto(S8)");
    assert_eq!(rows[0].oil_text.as_deref(), Some("SAE 5W-30"));
    assert_eq!(rows[0].cylinder_deactivation, Some(true));
    // A sentence is not a name, and `Other` is not a battery type.
    assert_eq!(rows[0].cylinder_deactivation_name, None);
    assert_eq!(rows[0].battery_type, None);
    // A row with no year takes the file's; no displacement is None; a
    // name is kept only when deactivation is `Y`.
    assert_eq!(rows[1].year, 2012);
    assert_eq!(rows[1].displacement_l, None);
    assert_eq!(rows[1].cylinders, None);
    assert_eq!(rows[1].cylinder_deactivation_name, None);
    assert_eq!(rows[1].battery_type.as_deref(), Some("NiMH"));
    assert_eq!(rows[1].oil_text, None);
    // Every optional column the sheet lacks is reported, once.
    let missing: Vec<&str> = counts.missing.iter().map(|(_, field)| *field).collect();
    assert!(missing.contains(&"gears") && missing.contains(&"stop_start"));
    assert!(!missing.contains(&"oil") && !missing.contains(&"battery_type"));
    assert_eq!(
        missing.len(),
        COLUMNS.iter().filter(|column| !column.required).count() - 4
    );

    // A required column missing is an error that names it.
    let error = parse_sheet(
        &cells(&[&["Model Year", "Division", "Carline"]]),
        2030,
        &mut GuideCounts::default(),
        &mut |_| Ok(()),
    )
    .unwrap_err();
    let message = format!("{error:#}");
    assert!(message.contains("2030"), "{message}");
    assert!(message.contains("Verify Mfr Cd"), "{message}");
}

#[test]
fn guide_rows_join_their_vehicles() {
    let scratch = Scratch::new("epa-guide-join");
    let vehicles = vehicles(&epa_vehicles_dir(&scratch));
    let join = GuideJoin::new(&vehicles);
    let (rows, _) = read(&epa_guide_dir(&scratch)).unwrap();
    let ids: Vec<Option<u32>> = rows.iter().map(|row| join.find(row)).collect();
    assert_eq!(
        ids,
        [
            Some(39238),
            Some(39252),
            Some(39449),
            Some(39241),
            Some(39348),
            Some(42135),
            Some(41833),
        ]
    );
    // Another maker's row with the same index joins nothing.
    let mut other = rows[0].clone();
    other.mfr_code = "GMX".to_owned();
    assert_eq!(join.find(&other), None);

    // Before 2012 `vehicles.csv` has no maker code: the index joins only
    // with the same litres, cylinders and transmission, and only one row.
    let mut old = vehicles
        .iter()
        .find(|vehicle| vehicle.id == 39241)
        .unwrap()
        .clone();
    old.year = 2011;
    old.mfr_code = None;
    let mut twin = old.clone();
    twin.id = 1;
    twin.cylinders = Some(8);
    let join = GuideJoin::new([&old, &twin]);
    let mut row = rows[3].clone();
    row.year = 2011;
    assert_eq!(join.find(&row), Some(39241));
    row.displacement_l = Some(3.5);
    assert_eq!(join.find(&row), None);
    let mut same = old.clone();
    same.id = 2;
    let join = GuideJoin::new([&old, &same]);
    row.displacement_l = Some(2.7);
    assert_eq!(join.find(&row), None);
}

#[test]
fn a_fetch_asks_for_each_year_and_stops_at_the_first_missing_one() {
    let scratch = Scratch::new("epa-guide-fetch");
    let mut asked = Vec::new();
    // 2011 to 2027 exist; 2028 does not.
    let mut fake = |url: &str, path: &Path| -> Result<Downloaded> {
        asked.push(url.to_owned());
        if url.ends_with("/28data.zip") {
            anyhow::bail!("http status: 404");
        }
        std::fs::write(path, b"PK\x03\x04")?;
        Ok(Downloaded {
            last_modified: url
                .ends_with("/27data.zip")
                .then(|| "Tue, 29 Sep 2026 19:40:31 GMT".to_owned()),
            bytes: 4,
        })
    };
    let build = scratch.path("build");
    let paths = fetch_with(&build, "2026-10-12 09:59:30", 2026, &mut fake).unwrap();
    assert_eq!(asked.len(), 18);
    assert_eq!(
        asked[0],
        "https://www.fueleconomy.gov/feg/epadata/11data.zip"
    );
    assert_eq!(paths.len(), 18);
    let directory = build.join("sources/epa-guide");
    assert_eq!(paths[0], directory.join("11data.zip"));
    assert_eq!(paths[16], directory.join("27data.zip"));
    assert_eq!(paths[17], directory.join("source.json"));
    assert!(!directory.join("28data.zip").exists());
    assert!(!directory.join("28data.zip.part").exists());
    let record = SourceRecord::read(&directory).unwrap();
    assert_eq!(record.source, "epa-guide");
    assert_eq!(record.as_of, "2026-09-29");
    assert_eq!(record.files.len(), 17);
    assert_eq!(record.files[16].name, "27data.zip");

    // A year that must be there and is not is an error.
    let scratch = Scratch::new("epa-guide-fetch-gap");
    let mut fake = |url: &str, path: &Path| -> Result<Downloaded> {
        if url.ends_with("/19data.zip") {
            anyhow::bail!("http status: 404");
        }
        std::fs::write(path, b"PK\x03\x04")?;
        Ok(Downloaded {
            last_modified: None,
            bytes: 4,
        })
    };
    let error = fetch_with(
        &scratch.path("build"),
        "2026-10-12 09:59:30",
        2026,
        &mut fake,
    )
    .unwrap_err();
    assert!(
        format!("{error:#}").contains("2019 guide file"),
        "{error:#}"
    );
    assert!(!scratch.path("build/sources/epa-guide/source.json").exists());

    // An error page saved with status 200 is not a guide file.
    let scratch = Scratch::new("epa-guide-fetch-html");
    let mut fake = |_: &str, path: &Path| -> Result<Downloaded> {
        std::fs::write(path, b"<html>")?;
        Ok(Downloaded {
            last_modified: None,
            bytes: 6,
        })
    };
    let error = fetch_with(
        &scratch.path("build"),
        "2026-10-12 09:59:30",
        2026,
        &mut fake,
    )
    .unwrap_err();
    assert!(
        format!("{error:#}").contains("not a zip archive"),
        "{error:#}"
    );
}

#[test]
fn an_empty_directory_says_to_fetch() {
    let scratch = Scratch::new("epa-guide-empty");
    let error = read(&scratch.path("sources/epa-guide")).unwrap_err();
    assert!(
        format!("{error:#}").contains("open-data fetch epa-guide"),
        "{error:#}"
    );
    std::fs::create_dir_all(scratch.path("sources/epa-guide")).unwrap();
    std::fs::write(scratch.path("sources/epa-guide/10data.zip"), b"PK").unwrap();
    let error = read(&scratch.path("sources/epa-guide")).unwrap_err();
    assert!(
        format!("{error:#}").contains("holds no guide file"),
        "{error:#}"
    );
    assert_eq!(FIRST_YEAR, 2011);
}

/// Reads every real guide file and measures the oil grades and the join.
/// Run after `open-data fetch epa-guide` and `open-data fetch epa-vehicles`:
/// `cargo test --release -p open-data --test shop the_fetched_guide_files_parse -- --ignored --nocapture`
#[test]
#[ignore = "needs data/build/sources/epa-guide and epa-vehicles"]
fn the_fetched_guide_files_parse() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let curated = CuratedOil::load(&root.join("data")).unwrap();
    let vehicles = vehicles(&root.join("data/build/sources/epa-vehicles"));
    let join = GuideJoin::new(&vehicles);
    let (rows, counts) = read(&root.join("data/build/sources/epa-guide")).unwrap();
    println!(
        "{} files, {} rows, {} skipped, missing columns {:?}",
        counts.files, counts.rows, counts.skipped, counts.missing
    );
    // year -> (rows, with oil text, joined)
    let mut years: BTreeMap<i32, (u64, u64, u64)> = BTreeMap::new();
    let mut spellings: BTreeMap<String, u64> = BTreeMap::new();
    let (mut joined, mut one, mut several, mut none_listed, mut unread) =
        (0u64, 0u64, 0u64, 0u64, 0u64);
    let mut unknown: BTreeMap<String, u64> = BTreeMap::new();
    let mut seen_ids: BTreeMap<u32, u64> = BTreeMap::new();
    for row in &rows {
        let year = years.entry(row.year).or_default();
        year.0 += 1;
        if let Some(id) = join.find(row) {
            joined += 1;
            year.2 += 1;
            *seen_ids.entry(id).or_default() += 1;
        }
        let Some(oil) = &row.oil_text else { continue };
        year.1 += 1;
        *spellings.entry(oil.clone()).or_default() += 1;
        match oil_grade::read(oil, &curated) {
            Some(grades) if grades.len() == 1 => one += 1,
            Some(grades) if grades.is_empty() => none_listed += 1,
            Some(_) => several += 1,
            None => {
                unread += 1;
                *unknown.entry(oil.clone()).or_default() += 1;
            }
        }
    }
    for (year, (rows, oil, joined)) in &years {
        println!("{year}: {rows} rows, {oil} with an oil grade, {joined} joined");
    }
    let with_oil: u64 = spellings.values().sum();
    println!(
        "oil: {with_oil} rows, {} spellings; one grade {one}, several {several}, curated as no grade {none_listed}, unread {unread}",
        spellings.len()
    );
    println!("unread spellings: {unknown:?}");
    println!(
        "join: {joined} of {} rows ({:.1}%), to {} vehicles; {} vehicles have more than one guide row",
        counts.rows,
        joined as f64 * 100.0 / counts.rows as f64,
        seen_ids.len(),
        seen_ids.values().filter(|rows| **rows > 1).count()
    );
    assert_eq!(
        unread, 0,
        "add the unread spellings to data/shop/oil-grades.yaml"
    );
    assert!(
        counts.missing.iter().all(|(year, _)| *year < 2013),
        "{:?}",
        counts.missing
    );
}
