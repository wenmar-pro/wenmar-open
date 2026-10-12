use open_data::shop::build::{ShopInputs, build_shop};
use open_data::shop::complaints::{
    ComplaintCounts, ComplaintRow, FIELDS, READ, check_layout, parse,
};
use rusqlite::Connection;
use wenmar_shopdata::{Dataset, licence, schema};

use crate::common::{Scratch, complaints_dir, fixture, info, rows};

fn read(directory: &std::path::Path) -> anyhow::Result<(Vec<ComplaintRow>, ComplaintCounts)> {
    let mut found = Vec::new();
    let counts = parse(directory, &mut |row| {
        found.push(row);
        Ok(())
    })?;
    Ok((found, counts))
}

fn built(name: &str) -> (Scratch, Connection) {
    let scratch = Scratch::new(name);
    let inputs = ShopInputs {
        nhtsa_complaints: Some(complaints_dir(&scratch)),
        ..ShopInputs::default()
    };
    let summary = build_shop(
        &scratch.core(),
        &scratch.out(),
        &scratch.curated(),
        &info(),
        &inputs,
    )
    .unwrap();
    let entry = summary.datasets.get(Dataset::Complaints).unwrap();
    assert_eq!(entry.as_of, "2026-10-11");
    assert_eq!(entry.rows, 6);
    assert_eq!(entry.licence, licence::PUBLIC_DOMAIN);
    assert!(!Dataset::Complaints.hosted_only());
    let connection = Connection::open(scratch.out()).unwrap();
    (scratch, connection)
}

#[test]
fn the_field_order_is_the_one_cmpl_txt_gives() {
    let text = std::fs::read_to_string(fixture("nhtsa-complaints").join("CMPL.txt")).unwrap();
    check_layout(&text).unwrap();
    // The positions this build reads, counted from 1 as CMPL.txt does.
    for (number, name) in [
        (1, "CMPLID"),
        (2, "ODINO"),
        (4, "MAKETXT"),
        (5, "MODELTXT"),
        (6, "YEARTXT"),
        (7, "CRASH"),
        (9, "FIRE"),
        (10, "INJURED"),
        (12, "COMPDESC"),
        (18, "MILES"),
        (46, "PROD_TYPE"),
    ] {
        assert_eq!(FIELDS[number - 1], name);
        assert!(READ.contains(&(number - 1)), "{name}");
    }
    assert_eq!(READ.len(), 11);
    // What must never be read.
    for name in [
        "CITY",
        "STATE",
        "VIN",
        "CDESCR",
        "DEALER_NAME",
        "DEALER_TEL",
        "DEALER_CITY",
        "DEALER_STATE",
        "DEALER_ZIP",
        "STATE_OF_INCIDENT",
        "VEHICLE_OPERATOR",
    ] {
        let index = FIELDS.iter().position(|field| *field == name).unwrap();
        assert!(!READ.contains(&index), "{name} is read");
    }
    let changed = text.replace(" MILES ", " MILEAGE ");
    assert!(check_layout(&changed).is_err());
}

#[test]
fn reads_vehicle_records_and_nothing_an_owner_wrote() {
    let scratch = Scratch::new("complaints-parse");
    let (found, counts) = read(&complaints_dir(&scratch)).unwrap();
    assert_eq!(
        counts,
        ComplaintCounts {
            records: 18,
            vehicles: 17,
            other_types: 1
        }
    );
    // One complaint under three components is three records.
    let three: Vec<&ComplaintRow> = found.iter().filter(|row| row.odino == 10917484).collect();
    let components: Vec<&str> = three
        .iter()
        .map(|row| row.component.as_deref().unwrap())
        .collect();
    assert_eq!(
        components,
        [
            "SERVICE BRAKES",
            "ELECTRONIC STABILITY CONTROL (ESC)",
            "STEERING"
        ]
    );
    assert!(three.iter().all(|row| row.miles == Some(4400)));
    let crash = found.iter().find(|row| row.odino == 10943869).unwrap();
    assert_eq!(
        crash,
        &ComplaintRow {
            record_id: 1347214,
            odino: 10943869,
            make_text: "HONDA".to_owned(),
            model_text: "CIVIC".to_owned(),
            year: Some(2016),
            crash: true,
            fire: true,
            injured: 0,
            component: Some("AIR BAGS".to_owned()),
            miles: Some(45000),
        }
    );
    // No mileage given, and no model year.
    assert_eq!(
        found
            .iter()
            .find(|row| row.odino == 10816192)
            .unwrap()
            .miles,
        None
    );
    assert_eq!(
        found.iter().find(|row| row.odino == 481184).unwrap().year,
        None
    );
    // The row has ten fields and none of them is text an owner wrote: the
    // fixture's placeholders for it are nowhere in what was read.
    let debug = format!("{found:?}");
    for private in [
        "TEXT REMOVED",
        "VINREMOVED0",
        "CITY",
        "DEALER REMOVED",
        "NAME REMOVED",
    ] {
        assert!(!debug.contains(private), "{private}");
    }
}

#[test]
fn a_model_year_is_counted_by_component_with_the_median_mileage() {
    let (_scratch, connection) = built("complaints-counts");
    assert_eq!(
        rows(
            &connection,
            "SELECT make_id, model_id, year, component_id, complaints, crashes, fires, injuries, with_miles, median_miles
             FROM complaint_count ORDER BY model_id, year, component_id"
        ),
        [
            // The 2018 F-150: three complaints, two under the powertrain.
            "460|1801|2018||3|0|0|0|3|2500",
            "460|1801|2018|engine|1|0|0|0|1|1519",
            "460|1801|2018|powertrain|2|0|0|0|2|2500",
            // The 2016 Civic: nine complaints over eleven records. The
            // median of eight mileages is the lower of the middle two.
            "474|1863|2016||9|3|1|2|8|3000",
            "474|1863|2016|air-bags|2|2|1|1|2|13000",
            "474|1863|2016|electrical|1|1|0|0|1|6",
        ]
    );
    // The 1999 Civic, the Honda with no year, the Freightliner and the
    // tire are not counted: no catalog model year.
    assert_eq!(
        rows(
            &connection,
            "SELECT SUM(complaints) FROM complaint_count WHERE component_id = ''"
        ),
        ["12"]
    );
    // The components the counts name are in the file.
    assert_eq!(
        rows(
            &connection,
            "SELECT COUNT(*) FROM complaint_count WHERE component_id <> '' AND component_id NOT IN (SELECT id FROM component)"
        ),
        ["0"]
    );
}

#[test]
fn the_file_holds_counts_and_nothing_else_about_a_complaint() {
    let (scratch, connection) = built("complaints-private");
    // Exactly these columns.
    assert_eq!(
        rows(
            &connection,
            "SELECT name FROM pragma_table_info('complaint_count')"
        ),
        schema::COMPLAINT_COLUMNS
    );
    // No other table came from the complaints.
    assert_eq!(
        rows(
            &connection,
            "SELECT name FROM sqlite_schema WHERE type = 'table' ORDER BY name"
        ),
        ["complaint_count", "component", "meta"]
    );
    // No column anywhere is named for what must not be kept.
    let columns = rows(
        &connection,
        "SELECT lower(p.name) FROM sqlite_schema s, pragma_table_info(s.name) p WHERE s.type = 'table'",
    );
    for banned in [
        "description",
        "cdescr",
        "text",
        "summary",
        "city",
        "state",
        "vin",
        "dealer",
        "name_of",
        "operator",
        "odino",
        "cmplid",
        "zip",
        "tel",
    ] {
        assert!(
            !columns.iter().any(|column| column.contains(banned)),
            "a column is named {banned}: {columns:?}"
        );
    }
    drop(connection);
    // And the fixture's placeholders for an owner's words, city, VIN,
    // dealer and name are nowhere in the bytes of the file.
    let bytes = std::fs::read(scratch.out()).unwrap();
    let holds = |needle: &str| {
        bytes
            .windows(needle.len())
            .any(|window| window == needle.as_bytes())
    };
    for private in [
        "TEXT REMOVED",
        "VINREMOVED0",
        "DEALER REMOVED",
        "NAME REMOVED",
        "10943869",
    ] {
        assert!(!holds(private), "{private} is in the shop file");
    }
    assert!(holds("complaint_count"));
}

#[test]
fn a_directory_that_is_not_a_fetch_is_refused() {
    let scratch = Scratch::new("complaints-refused");
    let directory = complaints_dir(&scratch);
    std::fs::write(directory.join("FLAT_CMPL.zip"), b"not a zip").unwrap();
    let error = format!("{:#}", read(&directory).unwrap_err());
    assert!(error.contains("FLAT_CMPL.zip is not a zip file"), "{error}");
    std::fs::remove_file(directory.join("CMPL.txt")).unwrap();
    let error = format!("{:#}", read(&directory).unwrap_err());
    assert!(
        error.contains("run open-data fetch nhtsa-complaints first"),
        "{error}"
    );
}

/// Run by hand after `open-data fetch nhtsa-complaints` (374 MB):
/// `cargo test --release -p open-data --test shop the_fetched_complaints_file_parses -- --ignored --nocapture`
#[test]
#[ignore = "needs the fetched files in data/build/sources/nhtsa-complaints"]
fn the_fetched_complaints_file_parses() {
    let directory = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../data/build/sources/nhtsa-complaints");
    let mut complaints = std::collections::HashSet::new();
    let counts = parse(&directory, &mut |row| {
        complaints.insert(row.odino);
        Ok(())
    })
    .unwrap();
    println!("{counts:?}, {} complaints", complaints.len());
    assert!(counts.vehicles > 2_100_000, "{counts:?}");
    assert!(complaints.len() > 1_500_000, "{}", complaints.len());
}
