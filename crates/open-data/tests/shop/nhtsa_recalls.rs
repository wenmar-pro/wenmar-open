use open_data::shop::nhtsa_recalls::{
    FIELDS, NhtsaCounts, NhtsaRecallRow, check_layout, layout, parse,
};

use crate::common::{Scratch, fixture, nhtsa_recalls_dir};

fn read(directory: &std::path::Path) -> anyhow::Result<(Vec<NhtsaRecallRow>, NhtsaCounts)> {
    let mut rows = Vec::new();
    let counts = parse(directory, &mut |row| {
        rows.push(row);
        Ok(())
    })?;
    Ok((rows, counts))
}

fn fixture_rows(name: &str) -> (Vec<NhtsaRecallRow>, NhtsaCounts) {
    let scratch = Scratch::new(name);
    read(&nhtsa_recalls_dir(&scratch)).unwrap()
}

/// The rows of one campaign, in file order.
fn campaign<'r>(rows: &'r [NhtsaRecallRow], campaign: &str) -> Vec<&'r NhtsaRecallRow> {
    rows.iter().filter(|row| row.campaign == campaign).collect()
}

#[test]
fn the_field_order_is_the_one_rcl_txt_gives() {
    let text = std::fs::read_to_string(fixture("nhtsa-recalls").join("RCL.txt")).unwrap();
    assert_eq!(layout(&text), FIELDS);
    check_layout(&text).unwrap();
    // The positions this build relies on, counted from 1 as RCL.txt does.
    for (number, name) in [
        (2, "CAMPNO"),
        (5, "YEARTXT"),
        (6, "MFGCAMPNO"),
        (11, "RCLTYPECD"),
        (13, "ODATE"),
        (16, "RCDATE"),
        (20, "DESC_DEFECT"),
        (27, "MFR_COMP_PTNO"),
        (28, "DO_NOT_DRIVE"),
        (29, "PARK_OUTSIDE"),
    ] {
        assert_eq!(FIELDS[number - 1], name);
    }
}

#[test]
fn reads_both_archives_and_keeps_only_vehicle_records() {
    let (rows, counts) = fixture_rows("nhtsa-counts");
    assert_eq!(
        counts,
        NhtsaCounts {
            records: 18,
            vehicles: 16,
            other_types: 2,
            junk: 1
        }
    );
    assert_eq!(rows.len(), 16);
    // File order: the archive from before 2010 first, each in its own order.
    let ids: Vec<u32> = rows.iter().map(|row| row.record_id).collect();
    assert_eq!(
        ids,
        [
            714, 715, 18978, 18979, 54055, 54056, 124047, 124048, 144677, 144678, 144679, 144680,
            177965, 177971, 323722, 325433
        ]
    );
    // The tire and the child seat are gone.
    assert!(campaign(&rows, "70T006000").is_empty());
    assert!(campaign(&rows, "72C001000").is_empty());
}

#[test]
fn a_campaign_spans_a_row_for_each_vehicle() {
    let (rows, _) = fixture_rows("nhtsa-campaign");
    let found = campaign(&rows, "19V888000");
    let vehicles: Vec<(&str, &str, Option<i32>)> = found
        .iter()
        .map(|row| (row.make_text.as_str(), row.model_text.as_str(), row.year))
        .collect();
    assert_eq!(
        vehicles,
        [
            ("GMC", "SIERRA 1500", Some(2019)),
            ("CHEVROLET", "SILVERADO 1500", Some(2020)),
            ("GMC", "SIERRA 1500", Some(2020)),
            ("CHEVROLET", "SILVERADO 1500", Some(2019)),
        ]
    );
    let first = found[0];
    assert_eq!(first.mfr_campaign.as_deref(), Some("N192273510"));
    assert_eq!(first.manufacturer.as_deref(), Some("General Motors LLC"));
    assert_eq!(
        first.component.as_deref(),
        Some("ELECTRICAL SYSTEM:12V/24V/48V BATTERY:CABLES")
    );
    assert_eq!(first.built_from.as_deref(), Some("2018-07-11"));
    assert_eq!(first.built_to.as_deref(), Some("2019-11-22"));
    assert_eq!(first.units, Some(350_371));
    assert_eq!(first.owner_notified.as_deref(), Some("2020-03-09"));
    assert_eq!(first.report_date.as_deref(), Some("2019-12-12"));
    assert!(
        first
            .summary
            .as_deref()
            .unwrap()
            .starts_with("General Motors LLC (GM)")
    );
    assert!(first.consequence.is_some() && first.remedy.is_some() && first.notes.is_some());
    assert!(!first.do_not_drive && !first.park_outside);
    // What a campaign says of itself is the same on every row.
    for row in &found {
        assert_eq!(row.summary, first.summary);
        assert_eq!(row.units, first.units);
    }
}

#[test]
fn reads_the_advisories_the_unknown_year_the_build_dates_and_the_part_number() {
    let (rows, _) = fixture_rows("nhtsa-cases");

    // Do not drive, and park outside.
    let advisory = campaign(&rows, "26V057000");
    assert_eq!(advisory.len(), 1);
    assert!(advisory[0].do_not_drive && advisory[0].park_outside);
    assert_eq!(advisory[0].make_text, "GRAND DESIGN");
    assert_eq!(advisory[0].part_number.as_deref(), Some("SL-T1218G-10"));
    assert_eq!(advisory[0].mfr_campaign.as_deref(), Some("910055/M910056"));

    // 9999 means no model year. This old campaign has only notes.
    let old = campaign(&rows, "68V023000");
    assert_eq!(old.len(), 2);
    assert_eq!(old[0].year, None);
    assert_eq!(
        (old[0].make_text.as_str(), old[0].model_text.as_str()),
        ("JEEP", "WAGONEER")
    );
    assert_eq!(old[1].make_text, "AMERICAN MOTORS");
    assert_eq!(old[0].summary, None);
    assert_eq!(old[0].consequence, None);
    assert_eq!(old[0].remedy, None);
    assert!(
        old[0]
            .notes
            .as_deref()
            .unwrap()
            .starts_with("POSSIBILITY THE FRONT WHEEL")
    );
    assert_eq!(old[0].mfr_campaign, None);
    assert_eq!(old[0].built_from, None);
    assert_eq!(old[0].report_date.as_deref(), Some("1968-03-19"));

    // Build dates and a part number.
    let dated = campaign(&rows, "26V375000");
    assert_eq!(dated.len(), 1);
    assert_eq!(dated[0].built_from.as_deref(), Some("2017-02-13"));
    assert_eq!(dated[0].built_to.as_deref(), Some("2018-07-11"));
    assert_eq!(dated[0].part_number.as_deref(), Some("JL3A-14C204-***"));
    assert_eq!(
        (
            dated[0].make_text.as_str(),
            dated[0].model_text.as_str(),
            dated[0].year
        ),
        ("FORD", "F-150", Some(2018))
    );

    // One vehicle listed twice, once for each component, and once for each
    // part number.
    let components: Vec<&str> = campaign(&rows, "17V669000")
        .iter()
        .map(|row| row.component.as_deref().unwrap())
        .collect();
    assert_eq!(
        components,
        [
            "POWER TRAIN:AUTOMATIC TRANSMISSION:LEVER AND LINKAGE:COLUMN SHIFT",
            "POWER TRAIN:AUTOMATIC TRANSMISSION:CONTROL MODULE (TCM/PCM/TECM)"
        ]
    );
    let parts: Vec<&str> = campaign(&rows, "21V983000")
        .iter()
        .map(|row| row.part_number.as_deref().unwrap())
        .collect();
    assert_eq!(parts, ["JL3A-9943150-BC", "JL3A-9943150-AC"]);

    // One row of a campaign may have build dates and the next none.
    let scion = campaign(&rows, "05V483000");
    assert_eq!(scion[0].built_from, None);
    assert_eq!(scion[1].built_from.as_deref(), Some("2004-03-12"));
    assert_eq!(scion[1].built_to.as_deref(), Some("2005-07-21"));
}

#[test]
fn refuses_a_directory_it_cannot_read() {
    let scratch = Scratch::new("nhtsa-refuse");
    let message = |directory: &std::path::Path| format!("{:#}", read(directory).unwrap_err());

    let empty = scratch.path("empty");
    std::fs::create_dir_all(&empty).unwrap();
    assert!(
        message(&empty).ends_with(
            "run open-data fetch nhtsa-recalls first: No such file or directory (os error 2)"
        ),
        "{}",
        message(&empty)
    );

    // A layout with a field added is one this build was not written for.
    let directory = nhtsa_recalls_dir(&scratch);
    let layout_path = directory.join("RCL.txt");
    let mut text = std::fs::read_to_string(&layout_path).unwrap();
    text.push_str("30 NEW_FIELD CHAR(3) Something NHTSA added\n");
    std::fs::write(&layout_path, text).unwrap();
    let error = message(&directory);
    assert!(
        error.contains("RCL.txt: the fields are not the ones this build reads: found 30 ("),
        "{error}"
    );

    // An archive that is missing, and one that is not an archive.
    let directory = nhtsa_recalls_dir(&scratch);
    std::fs::write(directory.join("FLAT_RCL_POST_2010.zip"), b"not a zip").unwrap();
    assert!(message(&directory).contains("FLAT_RCL_POST_2010.zip is not a zip file"));
    std::fs::remove_file(directory.join("FLAT_RCL_PRE_2010.zip")).unwrap();
    assert!(
        message(&directory)
            .contains("FLAT_RCL_PRE_2010.zip; run open-data fetch nhtsa-recalls first")
    );
}

/// Run by hand after `open-data fetch nhtsa-recalls`:
/// `cargo test -p open-data --test shop the_fetched_nhtsa_files_parse -- --ignored --nocapture`
#[test]
#[ignore = "needs the fetched files in data/build/sources/nhtsa-recalls"]
fn the_fetched_nhtsa_files_parse() {
    let directory = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../data/build/sources/nhtsa-recalls");
    let mut campaigns = std::collections::BTreeSet::new();
    let counts = parse(&directory, &mut |row| {
        campaigns.insert(row.campaign);
        Ok(())
    })
    .unwrap();
    println!("{counts:?}, {} campaigns", campaigns.len());
    assert!(counts.vehicles > 280_000, "{counts:?}");
    assert!(counts.other_types > 40_000, "{counts:?}");
    assert!(counts.junk < 100, "{counts:?}");
    assert!(campaigns.len() > 26_000, "{}", campaigns.len());
}
