use open_data::shop::build::{build_with, open_catalog};
use open_data::shop::coverage;
use open_data::shop::nhtsa_ratings::{self, FILE, HEADER, RatingCounts, RatingRow, parse};
use rusqlite::Connection;
use wenmar_shopdata::sqlite::ShopSource;
use wenmar_shopdata::{Dataset, ShopData, licence};

use crate::common::{Scratch, fixture, info, nhtsa_ratings_dir, rows};

fn read(directory: &std::path::Path) -> anyhow::Result<(Vec<RatingRow>, RatingCounts)> {
    let mut found = Vec::new();
    let counts = parse(directory, &mut |row| {
        found.push(row);
        Ok(())
    })?;
    Ok((found, counts))
}

fn fixture_rows(name: &str) -> (Vec<RatingRow>, RatingCounts) {
    let scratch = Scratch::new(name);
    read(&nhtsa_ratings_dir(&scratch)).unwrap()
}

/// The rows of one make, model and year, in file order.
fn of<'r>(found: &'r [RatingRow], year: i32, make: &str, model: &str) -> Vec<&'r RatingRow> {
    found
        .iter()
        .filter(|row| row.year == year && row.make_text == make && row.model_text == model)
        .collect()
}

#[test]
fn the_header_is_the_128_columns_the_file_has() {
    let text = std::fs::read_to_string(fixture("nhtsa-ratings").join(FILE)).unwrap();
    let first = text.lines().next().unwrap();
    assert_eq!(first, HEADER.join(","));
    assert_eq!(HEADER.len(), 128);
    assert_eq!(HEADER[0], "MAKE");
    assert_eq!(HEADER[67], "OVERALL_STARS");
    assert_eq!(HEADER[77], "CURB_WEIGHT");
    assert_eq!(HEADER[127], "BACKUP_CAMERA");
}

#[test]
fn reads_every_row_of_the_fixture() {
    let (found, counts) = fixture_rows("ratings-counts");
    assert_eq!(
        counts,
        RatingCounts {
            records: 29,
            rated: 27
        }
    );
    assert_eq!(found.len(), 29);
    let years: Vec<i32> = found.iter().map(|row| row.year).collect();
    assert_eq!(years.iter().min(), Some(&1999));
    assert_eq!(years.iter().max(), Some(&2026));
    // One model year has a row for each body style and drive.
    let trucks = of(&found, 2018, "FORD", "F-150 SUPER CREW");
    assert_eq!(trucks.len(), 2);
    assert_eq!(trucks[0].body_style.as_deref(), Some("PU/CC"));
    assert_eq!(trucks[0].vehicle_type.as_deref(), Some("TRUCK"));
    assert_eq!(trucks[0].drive.as_deref(), Some("4x2"));
    assert_eq!(trucks[1].drive.as_deref(), Some("4x4"));
    // The file holds one row twice over; `parse` hands on both.
    assert_eq!(of(&found, 2024, "HYUNDAI", "ELANTRA N").len(), 2);
}

#[test]
fn reads_the_stars_the_weight_and_the_rollover_figures() {
    let (found, _) = fixture_rows("ratings-stars");
    let civic = of(&found, 2016, "HONDA", "CIVIC");
    assert_eq!(civic.len(), 2);
    let coupe = civic[1];
    assert_eq!(coupe.body_style.as_deref(), Some("2 DR"));
    let stars = &coupe.stars;
    assert_eq!(
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
        ],
        [
            Some(5),
            Some(4),
            Some(5),
            Some(4),
            Some(5),
            Some(5),
            Some(4),
            Some(5),
            Some(5),
            Some(4),
            Some(5),
            Some(5),
        ]
    );
    assert_eq!(coupe.rollover_risk_permille, Some(93));
    assert_eq!(coupe.rollover_tip, Some(false));
    assert_eq!(coupe.curb_weight_lb, Some(2735));
    assert!(coupe.notes.is_empty());

    // A vehicle rated before 2011 has no overall rating.
    let old = of(&found, 1999, "TOYOTA", "COROLLA")[0];
    assert_eq!(old.stars.overall, None);
    assert_eq!(old.stars.frontal_driver, Some(4));
    assert_eq!(old.stars.side_passenger, Some(3));
    assert_eq!(old.drive, None);
    assert_eq!(old.curb_weight_lb, None);
    assert!(old.rated());

    // One that tipped up in the dynamic test.
    let escape = of(&found, 2007, "FORD", "ESCAPE")[0];
    assert_eq!(escape.rollover_tip, Some(true));
    assert_eq!(escape.stars.rollover, Some(3));
    assert_eq!(escape.rollover_risk_permille, Some(209));

    // A vehicle NHTSA lists and has not tested.
    let porsche = of(&found, 2024, "PORSCHE", "911 CARRERA S")[0];
    assert!(!porsche.rated());
    assert_eq!(porsche.rollover_risk_permille, None);

    // A later release of a model year.
    let tesla = of(&found, 2026, "TESLA", "MODEL Y")[0];
    assert_eq!(tesla.release, 2);
    assert_eq!(tesla.stars.overall, Some(5));
}

#[test]
fn reads_the_features_and_the_notes() {
    let (found, _) = fixture_rows("ratings-features");
    let prius = of(&found, 2013, "TOYOTA", "PRIUS")[0];
    assert_eq!(prius.features.esc, Some("standard"));
    assert_eq!(prius.features.forward_collision_warning, Some("no"));
    assert_eq!(prius.features.forward_collision_warning_evaluation, None);
    assert_eq!(prius.features.head_side_airbag, Some("standard"));
    assert_eq!(prius.features.backup_camera, None);

    let civic = of(&found, 2016, "HONDA", "CIVIC")[0];
    assert_eq!(civic.features.forward_collision_warning, Some("optional"));
    assert_eq!(civic.features.crash_imminent_braking, Some("no"));
    assert_eq!(civic.features.backup_camera, Some("standard"));

    // From model year 2023 the file says what NHTSA's own test found.
    let mazda = of(&found, 2025, "MAZDA", "CX-5")[0];
    assert_eq!(mazda.features.lane_departure_warning, Some("standard"));
    assert_eq!(
        mazda.features.lane_departure_warning_evaluation,
        Some("meets")
    );
    assert_eq!(mazda.features.blind_spot_detection, Some("standard"));
    let pending = of(&found, 2025, "MAZDA", "CX-50")[0];
    assert_eq!(
        pending.features.dynamic_brake_support_evaluation,
        Some("pending")
    );
    let cadillac = of(&found, 2025, "CADILLAC", "CT5-V BLACKWING")[0];
    assert_eq!(cadillac.features.crash_imminent_braking, Some("both"));
    assert_eq!(
        cadillac.features.crash_imminent_braking_evaluation,
        Some("upgrade_only")
    );
    let porsche = of(&found, 2024, "PORSCHE", "911 CARRERA S")[0];
    assert_eq!(
        porsche.features.forward_collision_warning_evaluation,
        Some("does_not_meet")
    );
    let genesis = of(&found, 2018, "GENESIS", "G80")[0];
    assert_eq!(genesis.features.forward_collision_warning, Some("both"));

    // Footnotes and safety concerns, each once, in column order.
    let ford = of(&found, 2018, "FORD", "F-150 REGULAR CAB")[0];
    assert_eq!(
        ford.notes,
        ["These ratings do not apply to vehicles produced after January 31, 2018"]
    );
    let small = of(&found, 2013, "TOYOTA", "PRIUS C")[0];
    assert_eq!(small.notes.len(), 3);
    assert!(small.notes[0].starts_with("Frontal and overall ratings do not apply"));
    assert!(small.notes[1].starts_with("Due to the intrusion of the left rear door"));
    let equinox = of(&found, 2026, "CHEVROLET", "EQUINOX")[0];
    assert!(equinox.notes[0].starts_with("During the side impact test, the driver door"));
    // A field that runs over two lines in the file does not break the row.
    let suburban = of(&found, 2025, "CHEVROLET", "SUBURBAN")[0];
    assert_eq!(suburban.stars.overall, Some(5));
    assert_eq!(suburban.curb_weight_lb, Some(3837));
}

#[test]
fn refuses_a_directory_it_cannot_read() {
    let scratch = Scratch::new("ratings-refuse");
    let message = |directory: &std::path::Path| format!("{:#}", read(directory).unwrap_err());
    let empty = scratch.path("empty");
    std::fs::create_dir_all(&empty).unwrap();
    assert!(
        message(&empty).ends_with(
            "run open-data fetch nhtsa-ratings first: No such file or directory (os error 2)"
        ),
        "{}",
        message(&empty)
    );
    // A file with a column added is one this build was not written for.
    let directory = nhtsa_ratings_dir(&scratch);
    let path = directory.join(FILE);
    let text = std::fs::read_to_string(&path).unwrap();
    std::fs::write(&path, text.replacen("MAKE,MODEL,", "MAKE,MODEL,TRIM,", 1)).unwrap();
    let error = message(&directory);
    assert!(
        error.starts_with(
            "Safercar_data.csv: the columns are not the ones this build reads: found 129 ("
        ),
        "{error}"
    );
}

#[test]
fn ratings_are_stored_once_each_with_their_vehicles_matched() {
    let scratch = Scratch::new("ratings-shape");
    let directory = nhtsa_ratings_dir(&scratch);
    let summary = build_with(
        &scratch.core(),
        &scratch.out(),
        &scratch.curated(),
        &info(),
        |context| nhtsa_ratings::shape(context, &directory),
    )
    .unwrap();
    let entry = summary.datasets.get(Dataset::Ratings).unwrap();
    assert_eq!(entry.as_of, "2026-08-07");
    // 29 rows in the file, one of them twice.
    assert_eq!(entry.rows, 28);
    assert_eq!(entry.licence, licence::PUBLIC_DOMAIN);
    assert_eq!(entry.attribution, "");

    let connection = Connection::open(scratch.out()).unwrap();
    assert_eq!(
        rows(
            &connection,
            "SELECT id, year, make_text, model_text, body_style, vehicle_type, drive, release, overall, frontal, side, rollover, rollover_risk_permille, rollover_tip, curb_weight_lb FROM rating WHERE id IN (1, 6, 8, 14, 15, 17, 21, 27) ORDER BY id"
        ),
        [
            "1|2018|FORD|F-150 REGULAR CAB|PU/RC|TRUCK|4x2|1|4|4|5|4|191|0|5276",
            "6|2018|FORD|F-150 SUPER CREW|PU/CC|TRUCK|4x4|1|5|5|5|4|191|0|5060",
            "8|2016|HONDA|CIVIC|2 DR|PC|FWD|1|5|4|5|5|93|0|2735",
            "14|1999|TOYOTA|COROLLA|4-DR W/SAB|LIGHT PASSENGER CAR|-|1|-|-|-|-|-|-|-",
            "15|2024|PORSCHE|911 CARRERA S|2 DR|PC|RWD|1|-|-|-|-|-|-|-",
            "17|2026|TESLA|MODEL Y|SUV|MPV|AWD|2|5|5|5|5|86|0|4360",
            "21|2007|FORD|ESCAPE|4 DR|SPORT UTILITY VEHICLE|FWD/AWD|1|-|-|-|3|209|1|-",
            "27|2024|HYUNDAI|ELANTRA N|4 DR|PC|FWD|1|-|-|-|5|101|0|2801",
        ]
    );
    assert_eq!(
        rows(
            &connection,
            "SELECT id, frontal_driver, frontal_passenger, side_driver, side_passenger, side_barrier, side_front_seat, side_rear_seat, side_pole FROM rating WHERE id IN (8, 14) ORDER BY id"
        ),
        ["8|5|4|5|4|5|5|4|5", "14|4|4|4|3|-|-|-|-"]
    );
    assert_eq!(
        rows(
            &connection,
            "SELECT id, esc, fcw, fcw_evaluation, ldw, ldw_evaluation, cib, cib_evaluation, dbs, dbs_evaluation, backup_camera, blind_spot_detection, head_side_airbag, torso_side_airbag FROM rating WHERE id IN (7, 13, 22, 23) ORDER BY id"
        ),
        [
            "7|-|optional|-|optional|-|no|-|no|-|standard|-|standard|standard",
            "13|-|standard|meets|standard|meets|standard|meets|standard|meets|-|standard|standard|standard",
            "22|-|both|upgrade_only|both|upgrade_only|both|upgrade_only|both|upgrade_only|-|optional|standard|standard",
            "23|standard|no|-|no|-|no|-|no|-|-|-|standard|standard",
        ]
    );
    // Notes are one to a line; a rating without any has NULL.
    assert_eq!(
        rows(
            &connection,
            "SELECT id, length(notes) - length(replace(notes, char(10), '')) + 1 FROM rating WHERE notes IS NOT NULL ORDER BY id"
        ),
        [
            "1|1", "2|1", "3|1", "4|1", "5|1", "6|1", "19|2", "20|1", "24|3"
        ]
    );
    assert_eq!(
        rows(
            &connection,
            "SELECT rating_id, year, make_id, model_id, make_text, model_text, \"match\" FROM rating_vehicle ORDER BY rowid"
        ),
        [
            "1|2018|460|1801|FORD|F-150 REGULAR CAB|base",
            "2|2018|460|1801|FORD|F-150 REGULAR CAB|base",
            "3|2018|460|1801|FORD|F-150 SUPERCAB|base",
            "4|2018|460|1801|FORD|F-150 SUPERCAB|base",
            "5|2018|460|1801|FORD|F-150 SUPER CREW|base",
            "6|2018|460|1801|FORD|F-150 SUPER CREW|base",
            "7|2016|474|1863|HONDA|CIVIC|exact",
            "8|2016|474|1863|HONDA|CIVIC|exact",
            "9|2020|474|1861|HONDA|ACCORD|exact",
            "10|2019|467|1850|CHEVROLET|SILVERADO 1500|base",
            "11|2019|467|1850|CHEVROLET|SILVERADO 1500|base",
            "12|2025|473|30046|MAZDA|CX-50|exact",
            "13|2025|473|2369|MAZDA|CX-5|exact",
            // The make is in the fixture catalog and the model is not.
            "14|1999|448|-|TOYOTA|COROLLA|-",
            // Neither is.
            "15|2024|-|-|PORSCHE|911 CARRERA S|-",
            "16|2015|-|-|SRT|CHALLENGER|-",
            "17|2026|-|-|TESLA|MODEL Y|-",
            "18|2018|-|-|GENESIS|G80|-",
            "19|2022|-|-|CADILLAC|CT5|-",
            "20|2026|467|-|CHEVROLET|EQUINOX|-",
            "21|2007|460|-|FORD|ESCAPE|-",
            "22|2025|-|-|CADILLAC|CT5-V BLACKWING|-",
            "23|2013|448|2209|TOYOTA|PRIUS|exact",
            // The Prius C is not in the fixture catalog; the Prius is.
            "24|2013|448|2209|TOYOTA|PRIUS C|base",
            "25|2019|496|13620|RAM|1500|exact",
            "26|2019|496|13620|RAM|1500|exact",
            "27|2024|-|-|HYUNDAI|ELANTRA N|-",
            "28|2025|467|-|CHEVROLET|SUBURBAN|-",
        ]
    );
    assert_eq!(
        rows(
            &connection,
            "SELECT name FROM sqlite_master WHERE name LIKE 'rating%' ORDER BY name"
        ),
        [
            "rating",
            "rating_vehicle",
            "rating_vehicle_rating",
            "rating_vehicle_ymm"
        ]
    );
    drop(connection);

    // The reader accepts the file, and coverage measures the dataset.
    let shop = ShopData::new(ShopSource::open(scratch.out()).unwrap()).unwrap();
    shop.require(Dataset::Ratings).unwrap();
    let (catalog, _) = open_catalog(&scratch.core()).unwrap();
    let report = coverage::run(
        &shop,
        &coverage::light_makes(&catalog).unwrap(),
        coverage::TABLES,
    )
    .unwrap();
    assert_eq!(report.datasets.len(), 1);
    let found = &report.datasets[0];
    assert_eq!(found.dataset, "ratings");
    assert_eq!(
        (
            found.rows,
            found.measured,
            found.exact,
            found.base,
            found.curated,
            found.unmatched
        ),
        (28, 20, 8, 9, 0, 3)
    );
    assert_eq!(found.share(), "85.0");
}

#[test]
fn the_step_fails_without_the_record_of_the_fetch() {
    let scratch = Scratch::new("ratings-no-record");
    let directory = nhtsa_ratings_dir(&scratch);
    std::fs::remove_file(directory.join("source.json")).unwrap();
    let error = build_with(
        &scratch.core(),
        &scratch.out(),
        &scratch.curated(),
        &info(),
        |context| nhtsa_ratings::shape(context, &directory),
    )
    .unwrap_err();
    assert!(
        format!("{error:#}").contains("run open-data fetch first"),
        "{error:#}"
    );
    assert!(!scratch.out().exists());
}

/// Run by hand after `open-data fetch nhtsa-ratings`:
/// `cargo test -p open-data --test shop the_fetched_ratings_file_parses -- --ignored --nocapture`
#[test]
#[ignore = "needs the fetched file in data/build/sources/nhtsa-ratings"]
fn the_fetched_ratings_file_parses() {
    let directory = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../data/build/sources/nhtsa-ratings");
    let mut overall = 0u64;
    let mut weighed = 0u64;
    let counts = parse(&directory, &mut |row| {
        overall += u64::from(row.stars.overall.is_some());
        weighed += u64::from(row.curb_weight_lb.is_some());
        Ok(())
    })
    .unwrap();
    println!("{counts:?}, {overall} with an overall rating, {weighed} with a curb weight");
    assert!(counts.records > 17_000, "{counts:?}");
    assert!(counts.rated > 11_000, "{counts:?}");
    assert!(overall > 5_000, "{overall}");
    assert!(weighed > 6_000, "{weighed}");
}
