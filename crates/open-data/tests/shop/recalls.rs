use open_data::shop::build::{ShopInputs, build_shop, build_with, open_catalog};
use open_data::shop::coverage::{self, DatasetCoverage};
use open_data::shop::recalls::OGL_CANADA_ATTRIBUTION;
use open_data::shop::{nhtsa_recalls, tc_recalls};
use rusqlite::Connection;
use wenmar_shopdata::sqlite::ShopSource;
use wenmar_shopdata::{Dataset, ShopData, licence};

use crate::common::{Scratch, info, nhtsa_recalls_dir, rows, tc_recalls_dir};

/// The vehicle rows of one campaign, in the order they were stored.
fn vehicles(connection: &Connection, source: &str, campaign: &str) -> Vec<String> {
    rows(
        connection,
        &format!(
            "SELECT v.year, v.make_id, v.model_id, v.make_text, v.model_text, v.\"match\", v.built_from, v.built_to
             FROM recall_vehicle v JOIN recall r ON r.id = v.recall_id
             WHERE r.source = '{source}' AND r.campaign = '{campaign}'
             ORDER BY v.rowid"
        ),
    )
}

#[test]
fn nhtsa_campaigns_become_recalls_with_their_vehicles_matched() {
    let scratch = Scratch::new("recalls-us");
    let directory = nhtsa_recalls_dir(&scratch);
    let summary = build_with(
        &scratch.core(),
        &scratch.out(),
        &scratch.curated(),
        &info(),
        |context| nhtsa_recalls::shape(context, &directory),
    )
    .unwrap();
    let entry = summary.datasets.get(Dataset::RecallsUs).unwrap();
    assert_eq!(entry.as_of, "2026-10-11");
    assert_eq!(entry.rows, 8);
    assert_eq!(entry.licence, licence::PUBLIC_DOMAIN);
    assert_eq!(entry.attribution, "");
    assert!(!summary.datasets.contains(Dataset::RecallsCa));

    let connection = Connection::open(scratch.out()).unwrap();
    assert_eq!(
        rows(
            &connection,
            "SELECT id, source, campaign, manufacturer, report_date, owner_notified, mfr_campaign, units, do_not_drive, park_outside FROM recall ORDER BY id"
        ),
        [
            "1|nhtsa|05V483000|TOYOTA MOTOR NORTH AMERICA, INC.|2005-10-18|2006-10-27|50S|71392|0|0",
            "2|nhtsa|17V669000|Ford Motor Company|2017-10-23|2017-12-05|17S35|12429|0|0",
            "3|nhtsa|19V888000|General Motors LLC|2019-12-12|2020-03-09|N192273510|350371|0|0",
            "4|nhtsa|21V983000|Ford Motor Company|2021-12-16|2022-05-31|21S53|57591|0|0",
            "5|nhtsa|26V057000|Grand Design RV, LLC|2026-01-30|2026-03-04|910055/M910056|109|1|1",
            "6|nhtsa|26V375000|Ford Motor Company|2026-06-09|2026-07-09|26S42|10742|0|0",
            "7|nhtsa|68V023000|CHRYSLER CORPORATION|1968-03-19|1968-03-14|-|2800|0|0",
            "8|nhtsa|88V117000|AMERICAN HONDA MOTOR CO.|1988-07-22|1988-08-07|-|279821|0|0",
        ]
    );
    assert_eq!(
        rows(
            &connection,
            "SELECT campaign, component, part_number FROM recall WHERE component LIKE '%;%' OR part_number IS NOT NULL ORDER BY id"
        ),
        [
            "17V669000|POWER TRAIN:AUTOMATIC TRANSMISSION:LEVER AND LINKAGE:COLUMN SHIFT; POWER TRAIN:AUTOMATIC TRANSMISSION:CONTROL MODULE (TCM/PCM/TECM)|-",
            "21V983000|LATCHES/LOCKS/LINKAGES:TAILGATE:LATCH|JL3A-9943150-BC, JL3A-9943150-AC",
            "26V057000|EQUIPMENT:RECREATIONAL VEHICLE/TRAILER:PLUMBING:FREEZE PREVENTION HEATER|SL-T1218G-10",
            "26V375000|POWER TRAIN:AUTOMATIC TRANSMISSION:CONTROL MODULE (TCM/PCM/TECM)|JL3A-14C204-***",
        ]
    );
    assert_eq!(
        rows(
            &connection,
            "SELECT campaign, summary_en IS NULL, consequence_en IS NULL, remedy_en IS NULL, substr(notes_en, 1, 27), summary_fr, component_fr, notification FROM recall WHERE campaign IN ('68V023000', '88V117000') ORDER BY id"
        ),
        [
            "68V023000|1|1|1|POSSIBILITY THE FRONT WHEEL|-|-|-",
            "88V117000|0|0|0|SYSTEM: FUEL PUMP.VEHICLE D|-|-|-",
        ]
    );
    let expected: [(&str, &[&str]); 8] = [
        (
            "05V483000",
            &[
                "2005|-|-|SCION|TC|-|-|-",
                "2006|-|-|SCION|TC|-|2004-03-12|2005-07-21",
            ],
        ),
        // Two records for one vehicle are one row.
        (
            "17V669000",
            &["2018|460|1801|FORD|F-150|exact|2017-01-12|2017-10-09"],
        ),
        (
            "19V888000",
            &[
                "2019|467|1850|CHEVROLET|SILVERADO 1500|base|2018-07-11|2019-11-22",
                "2020|467|1850|CHEVROLET|SILVERADO 1500|base|2018-07-11|2019-11-22",
                // The make is in the catalog; the fixture has no Sierra
                // for these years.
                "2019|472|-|GMC|SIERRA 1500|-|2018-07-11|2019-11-22",
                "2020|472|-|GMC|SIERRA 1500|-|2018-07-11|2019-11-22",
            ],
        ),
        ("21V983000", &["2018|460|1801|FORD|F-150|exact|-|-"]),
        (
            "26V057000",
            &["2026|-|-|GRAND DESIGN|LINEAGE|-|2026-01-09|2026-01-27"],
        ),
        (
            "26V375000",
            &["2018|460|1801|FORD|F-150|exact|2017-02-13|2018-07-11"],
        ),
        // 9999 is stored as no year.
        (
            "68V023000",
            &[
                "-|-|-|AMERICAN MOTORS|GLADIATOR|-|-|-",
                "-|-|-|JEEP|WAGONEER|-|-|-",
            ],
        ),
        (
            "88V117000",
            &[
                "1984|474|-|HONDA|CIVIC|-|1983-07-01|1985-03-01",
                "1985|474|1863|HONDA|CIVIC|exact|1983-07-01|1985-03-01",
            ],
        ),
    ];
    for (campaign, lines) in expected {
        assert_eq!(
            vehicles(&connection, "nhtsa", campaign),
            lines,
            "{campaign}"
        );
    }
}

#[test]
fn transport_canada_campaigns_become_recalls_with_their_vehicles_matched() {
    let scratch = Scratch::new("recalls-ca");
    let directory = tc_recalls_dir(&scratch);
    let summary = build_with(
        &scratch.core(),
        &scratch.out(),
        &scratch.curated(),
        &info(),
        |context| tc_recalls::shape(context, &directory),
    )
    .unwrap();
    let entry = summary.datasets.get(Dataset::RecallsCa).unwrap();
    assert_eq!(entry.as_of, "2026-10-06");
    assert_eq!(entry.rows, 11);
    assert_eq!(entry.licence, licence::OGL_CANADA);
    assert_eq!(entry.attribution, OGL_CANADA_ATTRIBUTION);

    let connection = Connection::open(scratch.out()).unwrap();
    assert_eq!(
        rows(
            &connection,
            "SELECT id, source, campaign, manufacturer, report_date, owner_notified, mfr_campaign, units, do_not_drive, park_outside, component, component_fr, notification, notification_fr, summary_fr IS NULL FROM recall ORDER BY id"
        ),
        [
            "1|tc|1975001|-|1975-01-14|-|-|2700|-|-|Steering|Direction|Safety Mfr|Sécurité - fabricant|1",
            "2|tc|1975063|-|1975-06-17|-|-|500|-|-|Suspension|Suspension|Safety Mfr|Sécurité - fabricant|1",
            "3|tc|1976151|-|1977-02-02|-|-|175|-|-|-|-|-|-|1",
            "4|tc|2012218|-|2012-07-18|-|12S35|2181|-|-|Fuel System|Système d'alimentation en carburant|Superseded|Remplacé|0",
            "5|tc|2015621|-|2015-12-30|-|R0105 (51C014)|780|-|-|Tires|Pneus|Safety Mfr|Sécurité - fabricant|0",
            "6|tc|2016438|-|2016-09-08|-|-|24|-|-|Suspension|Suspension|Safety Mfr|Sécurité - fabricant|0",
            "7|tc|2018419|-|2018-08-09|-|FL-784|76|-|-|Steering|Direction|Safety Mfr|Sécurité - fabricant|0",
            "8|tc|2018601|-|2018-11-01|-|SRC R68|3051|-|-|Engine|Moteur|Safety Mfr|Sécurité - fabricant|0",
            "9|tc|2019631|-|2019-12-12|-|N192273510|24289|-|-|Electrical|Circuit électrique|Safety Mfr|Sécurité - fabricant|0",
            "10|tc|2021771|-|2021-12-17|-|21S53|14401|-|-|Structure|Structure|Safety Mfr|Sécurité - fabricant|0",
            "11|tc|2025479|-|2025-09-11|-|F1023|-|-|-|Brakes|Freins|Safety Mfr|Sécurité - fabricant|0",
        ]
    );
    let expected: [(&str, &[&str]); 7] = [
        ("1975063", &["-|-|-|BF GOODRICH|SPACE SAVER F78-14|-|-|-"]),
        ("2012218", &["2013|460|-|FORD|ESCAPE|-|-|-"]),
        // The row filed under Tire was dropped; the car's row is stored once.
        ("2015621", &["2015|-|-|HYUNDAI|GENESIS|-|-|-"]),
        // A family name is one row for each catalog model of that year.
        (
            "2016438",
            &[
                "2016|452|-|BMW|2 SERIES|-|-|-",
                "2015|452|1709|BMW|3 SERIES|curated|-|-",
                "2015|452|1711|BMW|3 SERIES|curated|-|-",
                "2015|452|2172|BMW|3 SERIES|curated|-|-",
                "2015|452|2173|BMW|3 SERIES|curated|-|-",
                "2016|452|-|BMW|3 SERIES|-|-|-",
                "2017|452|-|BMW|3 SERIES|-|-|-",
                "2016|452|-|BMW|4 SERIES|-|-|-",
            ],
        ),
        ("2018601", &["2013|-|-|SCION|FR-S|-|-|-"]),
        (
            "2019631",
            &[
                "2019|467|1850|CHEVROLET|SILVERADO|exact|-|-",
                "2020|467|1850|CHEVROLET|SILVERADO|exact|-|-",
                "2019|472|-|GMC|SIERRA|-|-|-",
                "2020|472|-|GMC|SIERRA|-|-|-",
            ],
        ),
        ("2021771", &["2018|460|1801|FORD|F-150|exact|-|-"]),
    ];
    for (campaign, lines) in expected {
        assert_eq!(vehicles(&connection, "tc", campaign), lines, "{campaign}");
    }
    assert_eq!(
        rows(
            &connection,
            "SELECT substr(summary_en, 1, 40), substr(summary_fr, 1, 40) FROM recall WHERE campaign = '2019631'"
        ),
        ["Issue:
On certain trucks, the nut that a|Problème :
Dans certaines camionnettes, ",]
    );
}

#[test]
fn both_sources_share_the_tables_and_coverage_measures_each() {
    let scratch = Scratch::new("recalls-both");
    let inputs = ShopInputs {
        nhtsa_recalls: Some(nhtsa_recalls_dir(&scratch)),
        tc_recalls: Some(tc_recalls_dir(&scratch)),
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
    let held: Vec<Dataset> = summary
        .datasets
        .iter()
        .map(|(dataset, _)| dataset)
        .collect();
    assert_eq!(held, [Dataset::RecallsUs, Dataset::RecallsCa]);

    let connection = Connection::open(scratch.out()).unwrap();
    assert_eq!(
        rows(
            &connection,
            "SELECT source, COUNT(*), MIN(id), MAX(id) FROM recall GROUP BY source ORDER BY source"
        ),
        ["nhtsa|8|1|8", "tc|11|9|19"]
    );
    assert_eq!(
        rows(
            &connection,
            "SELECT name FROM sqlite_master WHERE type = 'index' AND name LIKE 'recall%' ORDER BY name"
        ),
        [
            "recall_component_component",
            "recall_report_date",
            "recall_vehicle_recall",
            "recall_vehicle_year",
            "recall_vehicle_ymm"
        ]
    );
    // \"match\" is NULL exactly when the model is.
    assert_eq!(
        rows(
            &connection,
            "SELECT COUNT(*) FROM recall_vehicle WHERE (model_id IS NULL) <> (\"match\" IS NULL)"
        ),
        ["0"]
    );
    // The manufacturer's own campaign number is the same on both sides of
    // the border for these two.
    assert_eq!(
        rows(
            &connection,
            "SELECT a.campaign, b.campaign FROM recall a JOIN recall b ON b.mfr_campaign = a.mfr_campaign AND b.source = 'tc' WHERE a.source = 'nhtsa' ORDER BY a.campaign"
        ),
        ["19V888000|2019631", "21V983000|2021771"]
    );
    drop(connection);

    // The reader accepts the file, and coverage finds both datasets.
    let shop = ShopData::new(ShopSource::open(scratch.out()).unwrap()).unwrap();
    shop.require(Dataset::RecallsUs).unwrap();
    shop.require(Dataset::RecallsCa).unwrap();
    let (catalog, _) = open_catalog(&scratch.core()).unwrap();
    let report = coverage::run(
        &shop,
        &coverage::light_makes(&catalog).unwrap(),
        coverage::TABLES,
    )
    .unwrap();
    #[allow(clippy::type_complexity)]
    let figures: Vec<(&str, u64, u64, u64, u64, u64, u64, String)> = report
        .datasets
        .iter()
        .map(|found: &DatasetCoverage| {
            (
                found.dataset.as_str(),
                found.rows,
                found.measured,
                found.exact,
                found.base,
                found.curated,
                found.unmatched,
                found.share(),
            )
        })
        .collect();
    assert_eq!(
        figures,
        [
            ("recalls_us", 14, 7, 3, 2, 0, 2, "71.4".to_owned()),
            ("recalls_ca", 21, 14, 3, 0, 4, 7, "50.0".to_owned()),
        ]
    );
}

#[test]
fn a_step_fails_without_the_record_of_the_fetch() {
    let scratch = Scratch::new("recalls-no-record");
    let directory = nhtsa_recalls_dir(&scratch);
    std::fs::remove_file(directory.join("source.json")).unwrap();
    let error = build_with(
        &scratch.core(),
        &scratch.out(),
        &scratch.curated(),
        &info(),
        |context| nhtsa_recalls::shape(context, &directory),
    )
    .unwrap_err();
    assert!(
        format!("{error:#}").contains("run open-data fetch first"),
        "{error:#}"
    );
    assert!(!scratch.out().exists());
}
