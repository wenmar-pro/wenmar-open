use open_data::shop::build::build_with;
use open_data::shop::nhtsa_recall_reports::{
    ANNUAL_FIELDS, QUARTERLY_FIELDS, RecallReportRow, ReportCounts, ReportKind, check_layout,
    newest, parse, shape,
};
use open_data::shop::nhtsa_recalls::layout;
use rusqlite::Connection;
use wenmar_shopdata::sqlite::ShopSource;
use wenmar_shopdata::{Dataset, ShopData, licence};

use crate::common::{Scratch, fixture, info, nhtsa_recall_reports_dir, rows};

fn read(directory: &std::path::Path) -> anyhow::Result<(Vec<RecallReportRow>, ReportCounts)> {
    let mut found = Vec::new();
    let counts = parse(directory, &mut |row| {
        found.push(row);
        Ok(())
    })?;
    Ok((found, counts))
}

#[test]
fn the_field_order_is_the_one_each_layout_file_gives() {
    let quarterly =
        std::fs::read_to_string(fixture("nhtsa-recall-reports").join("RCL_Qtrly_Rpts.txt"))
            .unwrap();
    let annual =
        std::fs::read_to_string(fixture("nhtsa-recall-reports").join("RCL_Annual_Rpts.txt"))
            .unwrap();
    assert_eq!(layout(&quarterly), QUARTERLY_FIELDS);
    assert_eq!(layout(&annual), ANNUAL_FIELDS);
    check_layout(&quarterly, &QUARTERLY_FIELDS).unwrap();
    check_layout(&annual, &ANNUAL_FIELDS).unwrap();
    // The two differ in the name of field 8 only.
    assert!(check_layout(&annual, &QUARTERLY_FIELDS).is_err());
    // The positions this build relies on, counted from 1 as the layouts do.
    for (number, name) in [
        (2, "CAMPNO"),
        (7, "RPTNO"),
        (9, "INVOLVED"),
        (10, "TTLREMEDIED"),
        (11, "TTLUNREACH"),
        (12, "TTLREMOVED"),
        (13, "SUBMDATE"),
    ] {
        assert_eq!(QUARTERLY_FIELDS[number - 1], name);
        assert_eq!(ANNUAL_FIELDS[number - 1], name);
    }
    assert_eq!(QUARTERLY_FIELDS[7], "RPTQTR");
    assert_eq!(ANNUAL_FIELDS[7], "RPT_PERIOD");
}

#[test]
fn reads_both_archives_and_keeps_only_reports_of_vehicle_campaigns() {
    let scratch = Scratch::new("reports-counts");
    let (found, counts) = read(&nhtsa_recall_reports_dir(&scratch)).unwrap();
    assert_eq!(
        counts,
        ReportCounts {
            records: 36,
            vehicles: 33,
            other_types: 2,
            malformed: 1,
            empty: 0
        }
    );
    assert_eq!(found.len(), 33);
    // The quarterly file first, then the annual one.
    assert_eq!(
        found
            .iter()
            .filter(|row| row.kind == ReportKind::Quarterly)
            .count(),
        24
    );
    assert!(
        found[..24]
            .iter()
            .all(|row| row.kind == ReportKind::Quarterly)
    );
    assert!(found[24..].iter().all(|row| row.kind == ReportKind::Annual));
    // The child seat campaigns are gone, and so is the report whose
    // quarter is written `2099-`.
    assert!(found.iter().all(|row| row.campaign != "09C010000"));
    assert!(found.iter().all(|row| row.campaign != "18C001000"));
    let mclaren: Vec<u32> = found
        .iter()
        .filter(|row| row.campaign == "21V561000")
        .map(|row| row.report_number)
        .collect();
    assert_eq!(mclaren, [12]);
    assert_eq!(
        found[0],
        RecallReportRow {
            kind: ReportKind::Quarterly,
            campaign: "15V687000".to_owned(),
            report_number: 1,
            period_from: "2016-4".to_owned(),
            period_to: "2016-4".to_owned(),
            involved: 0,
            remedied: 22,
            unreachable: 0,
            removed: 584,
            submitted: Some("2018-03-09".to_owned()),
        }
    );
    assert_eq!(
        found[32],
        RecallReportRow {
            kind: ReportKind::Annual,
            campaign: "21V983000".to_owned(),
            report_number: 2,
            period_from: "2025-2".to_owned(),
            period_to: "2026-1".to_owned(),
            involved: 57_591,
            remedied: 38_917,
            unreachable: 1_011,
            removed: 23,
            submitted: Some("2026-04-13".to_owned()),
        }
    );
}

#[test]
fn the_newest_report_of_each_campaign_becomes_its_row() {
    let scratch = Scratch::new("reports-shape");
    let directory = nhtsa_recall_reports_dir(&scratch);
    let (found, _) = newest(&directory).unwrap();
    assert_eq!(found.len(), 9);

    let summary = build_with(
        &scratch.core(),
        &scratch.out(),
        &scratch.curated(),
        &info(),
        |context| shape(context, &directory),
    )
    .unwrap();
    let entry = summary.datasets.get(Dataset::RecallReports).unwrap();
    assert_eq!(entry.as_of, "2026-10-12");
    assert_eq!(entry.rows, 9);
    assert_eq!(entry.licence, licence::PUBLIC_DOMAIN);
    assert_eq!(entry.attribution, "");

    let connection = Connection::open(scratch.out()).unwrap();
    assert_eq!(
        rows(
            &connection,
            "SELECT campaign, kind, report_number, period_from, period_to, submitted, involved, remedied, unreachable, removed FROM recall_report ORDER BY campaign"
        ),
        [
            "15V687000|quarterly|6|2018-1|2018-1|2018-04-18|2038|614|0|588",
            "17V669000|quarterly|6|2019-1|2019-1|2019-04-30|12429|11479|52|0",
            // The annual report and the last quarterly one end in the same
            // quarter and were filed the same day: the annual one stands.
            "19V252000|annual|2|2023-3|2024-2|2024-08-05|15|15|0|0",
            "19V888000|quarterly|6|2021-2|2021-2|2021-07-19|350371|333125|551|46",
            // Quarterly reports went on after the last annual one.
            "19V918000|quarterly|23|2026-1|2026-1|2026-04-16|747928|177202|0|83044",
            "21V00J000|annual|2|2025-1|2025-4|2026-01-22|1239|1141|0|0",
            "21V561000|quarterly|12|2024-2|2024-2|2024-07-04|19|15|0|0",
            "21V983000|annual|2|2025-2|2026-1|2026-04-13|57591|38917|1011|23",
            "26V057000|quarterly|2|2026-2|2026-2|2026-07-22|109|57|0|0",
        ]
    );
    drop(connection);
    let shop = ShopData::new(ShopSource::open(scratch.out()).unwrap()).unwrap();
    let report = shop.recall_report("21v983000").unwrap().unwrap();
    assert_eq!(report.remedied, 38_917);
    assert_eq!(report.remedied_percent(), Some(67));
}

#[test]
fn refuses_a_directory_it_cannot_read() {
    let scratch = Scratch::new("reports-refuse");
    let message = |directory: &std::path::Path| format!("{:#}", read(directory).unwrap_err());

    let empty = scratch.path("empty");
    std::fs::create_dir_all(&empty).unwrap();
    assert!(
        message(&empty).ends_with(
            "RCL_Qtrly_Rpts.txt; run open-data fetch nhtsa-recall-reports first: No such file or directory (os error 2)"
        ),
        "{}",
        message(&empty)
    );

    // A layout with a field added is one this build was not written for.
    let directory = nhtsa_recall_reports_dir(&scratch);
    let layout_path = directory.join("RCL_Annual_Rpts.txt");
    let mut text = std::fs::read_to_string(&layout_path).unwrap();
    text.push_str("14 NEW_FIELD CHAR(3) Something NHTSA added\n");
    std::fs::write(&layout_path, text).unwrap();
    let error = message(&directory);
    assert!(
        error.contains(
            "RCL_Annual_Rpts.txt: the fields are not the ones this build reads: found 14 ("
        ),
        "{error}"
    );

    // An archive that is not one, and one that is missing.
    let directory = nhtsa_recall_reports_dir(&scratch);
    std::fs::write(directory.join("FLAT_RCL_Annual_Rpts.zip"), b"not a zip").unwrap();
    assert!(message(&directory).contains("FLAT_RCL_Annual_Rpts.zip is not a zip file"));
    std::fs::remove_file(directory.join("FLAT_RCL_Qrtly_Rpts.zip")).unwrap();
    assert!(
        message(&directory)
            .contains("FLAT_RCL_Qrtly_Rpts.zip; run open-data fetch nhtsa-recall-reports first")
    );

    // Without the record of the fetch there is no "as of" day.
    let directory = nhtsa_recall_reports_dir(&scratch);
    std::fs::remove_file(directory.join("source.json")).unwrap();
    let error = build_with(
        &scratch.core(),
        &scratch.out(),
        &scratch.curated(),
        &info(),
        |context| shape(context, &directory),
    )
    .unwrap_err();
    assert!(
        format!("{error:#}").contains("run open-data fetch first"),
        "{error:#}"
    );
    assert!(!scratch.out().exists());
}

/// Run by hand after `open-data fetch nhtsa-recall-reports`:
/// `cargo test -p open-data --test shop the_fetched_report_files_parse -- --ignored --nocapture`
#[test]
#[ignore = "needs the fetched files in data/build/sources/nhtsa-recall-reports"]
fn the_fetched_report_files_parse() {
    let directory = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../data/build/sources/nhtsa-recall-reports");
    let (found, counts) = newest(&directory).unwrap();
    println!("{counts:?}, {} campaigns", found.len());
    assert!(counts.vehicles > 70_000, "{counts:?}");
    assert!(counts.other_types > 7_000, "{counts:?}");
    assert!(counts.malformed < 20, "{counts:?}");
    assert_eq!(counts.empty, 0, "{counts:?}");
    assert!(found.len() > 10_000, "{}", found.len());
    assert!(
        found
            .values()
            .all(|row| row.remedied <= row.involved || row.involved == 0)
    );
}
