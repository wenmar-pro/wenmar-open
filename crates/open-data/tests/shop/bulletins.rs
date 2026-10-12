use open_data::shop::build::{ShopInputs, build_shop, build_with, offline_copy};
use open_data::shop::coverage;
use open_data::shop::mfr_comms;
use rusqlite::Connection;
use wenmar_shopdata::sqlite::ShopSource;
use wenmar_shopdata::{Dataset, ShopData, licence, schema};

use crate::common::{Scratch, info, mfr_comms_dir, nhtsa_recalls_dir, rows};

fn built(name: &str) -> (Scratch, Connection) {
    let scratch = Scratch::new(name);
    let directory = mfr_comms_dir(&scratch);
    let summary = build_with(
        &scratch.core(),
        &scratch.out(),
        &scratch.curated(),
        &info(),
        |context| mfr_comms::shape(context, &directory),
    )
    .unwrap();
    let entry = summary.datasets.get(Dataset::Bulletins).unwrap();
    assert_eq!(entry.as_of, "2026-10-11");
    assert_eq!(entry.rows, 13);
    assert_eq!(entry.licence, licence::PUBLIC_DOMAIN);
    assert_eq!(entry.attribution, "");
    let connection = Connection::open(scratch.out()).unwrap();
    (scratch, connection)
}

#[test]
fn a_communication_becomes_one_bulletin() {
    let (_scratch, connection) = built("bulletins-rows");
    assert_eq!(
        rows(
            &connection,
            "SELECT id, type, added, date, document_id, mfr_campaign, replaces, component, mfr_system FROM bulletin ORDER BY id"
        ),
        [
            "609932|service_bulletin|2000-04-11|1998-08-04|96057|-|-|ENGINE AND ENGINE COOLING:ENGINE|-",
            "10085226|service_bulletin|2016-11-30|2016-08-18|A15086|-|-|EQUIPMENT; ELECTRICAL SYSTEM:SOFTWARE|-",
            "10106674|service_bulletin|2017-06-16|2016-02-10|A16-018|-|-|ELECTRICAL SYSTEM:SOFTWARE; POWER TRAIN:AUTOMATIC TRANSMISSION:CONTROL MODULE (TCM/PCM/TECM); EQUIPMENT:ELECTRICAL|-",
            "10152456|service_bulletin|2019-01-07|2018-11-02|TSB 18-2324|-|-|ENGINE AND ENGINE COOLING:EXHAUST SYSTEM|-",
            "10158006|service_bulletin|2019-04-08|2019-02-25|sb2652 DT12 PTO|-|-|EQUIPMENT; EQUIPMENT ADAPTIVE/MOBILITY; POWER TRAIN:AUTOMATIC TRANSMISSION; EQUIPMENT:OTHER:OWNERS/SERVICE/OTHER MANUAL; ELECTRICAL SYSTEM:WIRING|-",
            "10194916|service_bulletin|2021-06-03|2021-05-28|A19-091|-|-|VISIBILITY|-",
            "10220350|service_bulletin|2022-09-22|2022-09-16|A2090B|-|-|VISIBILITY|-",
            "11006904|service_bulletin|2024-09-04|2024-08-08|SA-051-24|-|-|UNKNOWN OR OTHER|Heater",
            "11009827|emissions|2024-11-05|2024-10-22|24E12|-|-|UNKNOWN OR OTHER|-",
            "11011137|warranty_extension|2024-12-04|2024-11-09|SSPD8 Owner Letter|SSPD8|-|ENGINE|-",
            "11026354|service_bulletin|2025-12-05|2025-11-27|SA-056-25|-|-|POWER TRAIN|Rear Differential",
            "11031510|service_campaign|2026-04-23|2026-04-23|N262548050|N262548050|-|ENGINE|-",
            "11031644|over_the_air|2026-04-28|2026-04-14|IPMA-25.22.7.3.10B F-150|IPMA-25.22.7.3.10B|-|ELECTRICAL SYSTEM; LANE DEPARTURE: ASSIST; UNKNOWN OR OTHER; FORWARD COLLISION AVOIDANCE: ADAPTIVE CRUISE CONTROL|-",
        ]
    );
    // The summary is stored once, tidied.
    assert_eq!(
        rows(
            &connection,
            "SELECT summary FROM bulletin WHERE id = 11031644"
        ),
        ["Quality improvements include: -Enhanced BlueCruise performance"]
    );
    assert_eq!(
        rows(
            &connection,
            "SELECT COUNT(*) FROM bulletin WHERE summary IS NULL OR summary LIKE '%  %'"
        ),
        ["0"]
    );
    // bulletin_year holds every year once.
    assert_eq!(
        rows(
            &connection,
            "SELECT COUNT(*), MIN(year), MAX(year) FROM bulletin_year"
        ),
        ["201|1900|2100"]
    );
}

#[test]
fn a_name_is_stored_once_for_each_model_it_matched() {
    let (_scratch, connection) = built("bulletins-models");
    assert_eq!(
        rows(
            &connection,
            "SELECT id, make_text, model_text, make_id, model_id, \"match\" FROM bulletin_model ORDER BY id"
        ),
        [
            // The catalog has no 1996 to 1998 Civic.
            "1|HONDA|CIVIC|474|-|-",
            "2|HONDA|CR-V|474|-|-",
            "3|HONDA|CIVIC|474|1863|exact",
            "4|FORD|F-150|460|1801|exact",
            // A make the catalog does not have.
            "5|FREIGHTLINER|CASCADIA|-|-|-",
            "6|HONDA|HONDA|474|-|-",
            "7|MAZDA|CX-5|473|-|-",
            "8|MAZDA|CX-5|473|2369|exact",
            "9|FORD|F-150|460|-|-",
            "10|MAZDA|CX-30|473|-|-",
            "11|MAZDA|MAZDA3|473|-|-",
            // A trim of the model: the base step.
            "12|MAZDA|CX-50 HYBRID|473|30046|base",
            "13|MAZDA|CX-50|473|30046|exact",
            "14|CHEVROLET|SILVERADO 1500|467|1850|base",
            "15|GMC|SIERRA 1500|472|-|-",
        ]
    );
}

#[test]
fn model_years_are_stored_as_runs() {
    let (_scratch, connection) = built("bulletins-runs");
    assert_eq!(
        rows(
            &connection,
            "SELECT bulletin_id, bulletin_model_id, year_from, year_to FROM bulletin_vehicle ORDER BY 1, 2, 3"
        ),
        [
            "609932|1|1996|1998",
            // 2012 to 2014 and 2016: two runs.
            "10085226|2|2012|2014",
            "10085226|2|2016|2016",
            "10106674|3|2016|2016",
            "10152456|4|2018|2018",
            "10158006|5|2020|2020",
            // A Civic of 2016 to 2021: the catalog has 2016 and 2018.
            "10194916|1|2017|2017",
            "10194916|1|2019|2021",
            "10194916|3|2016|2016",
            "10194916|3|2018|2018",
            // No model year given.
            "10220350|6|0|0",
            "11006904|7|2021|2021",
            "11006904|7|2024|2024",
            "11006904|8|2025|2025",
            "11009827|4|2018|2018",
            "11009827|9|2019|2020",
            "11011137|7|2024|2024",
            "11011137|8|2025|2025",
            "11011137|10|2023|2023",
            "11011137|11|2023|2023",
            "11026354|7|2020|2020",
            "11026354|12|2025|2025",
            "11026354|13|2025|2025",
            "11031510|14|2019|2019",
            "11031510|15|2019|2019",
            "11031644|9|2024|2024",
        ]
    );
    // The bulletins of one model year: the lookup every reader makes.
    assert_eq!(
        rows(
            &connection,
            "SELECT DISTINCT v.bulletin_id FROM bulletin_model m
             JOIN bulletin_vehicle v ON v.bulletin_model_id = m.id
             WHERE m.model_id = 1801 AND v.year_from <= 2018 AND v.year_to >= 2018
             ORDER BY 1"
        ),
        ["10152456", "11009827"]
    );
}

#[test]
fn coverage_reads_one_row_for_each_model_year() {
    let (scratch, connection) = built("bulletins-coverage");
    // 46 records are 33 distinct communication, name and year; the name
    // that matched two ways (none here) would add rows.
    assert_eq!(
        rows(
            &connection,
            &format!(
                "SELECT COUNT(*), SUM(t.year IS NULL) FROM {} AS t",
                schema::BULLETIN_VEHICLE_YEARS
            )
        ),
        ["33|1"]
    );
    drop(connection);
    let source = ShopSource::open(scratch.out()).unwrap();
    let shop = ShopData::new(source).unwrap();
    let catalog = Connection::open(scratch.core()).unwrap();
    let report = coverage::run(
        &shop,
        &coverage::light_makes(&catalog).unwrap(),
        coverage::TABLES,
    )
    .unwrap();
    assert_eq!(report.datasets.len(), 1);
    let bulletins = &report.datasets[0];
    assert_eq!(bulletins.dataset, "bulletins");
    assert_eq!(bulletins.rows, 33);
    // Light makes from 2000 on: everything but the three old Civics, the
    // Freightliner and the row with no year.
    assert_eq!(bulletins.measured, 28);
    assert_eq!(
        (bulletins.exact, bulletins.base, bulletins.curated),
        (8, 2, 0)
    );
    assert_eq!(bulletins.unmatched, 18);
    assert_eq!(bulletins.top_unmatched[0].make_text, "HONDA");
    assert_eq!(bulletins.top_unmatched[0].model_text, "CIVIC");
    assert_eq!(bulletins.top_unmatched[0].rows, 4);
}

#[test]
fn records_of_a_communication_that_are_apart_stop_the_build() {
    use std::io::Write;

    let scratch = Scratch::new("bulletins-apart");
    let directory = mfr_comms_dir(&scratch);
    // The newest archive again, with the first communication's first
    // record repeated at the end.
    let text = std::fs::read_to_string(
        crate::common::fixture("nhtsa-mfr-comms").join("TSBS_RECEIVED_2025-2026.txt"),
    )
    .unwrap();
    let first = text.lines().next().unwrap();
    let file = std::fs::File::create(directory.join("TSBS_RECEIVED_2025-2026.zip")).unwrap();
    let mut archive = zip::ZipWriter::new(file);
    archive
        .start_file(
            "TSBS_RECEIVED_2025-2026.txt",
            zip::write::SimpleFileOptions::default(),
        )
        .unwrap();
    archive
        .write_all(format!("{text}{first}\n").as_bytes())
        .unwrap();
    archive.finish().unwrap();
    let error = build_with(
        &scratch.core(),
        &scratch.out(),
        &scratch.curated(),
        &info(),
        |context| mfr_comms::shape(context, &directory),
    )
    .unwrap_err();
    assert_eq!(
        format!("{error:#}"),
        "the records of communication 11026354 are not together in the files"
    );
    assert!(!scratch.out().exists());
}

#[test]
fn the_offline_copy_leaves_out_what_only_the_hosted_service_carries() {
    let scratch = Scratch::new("bulletins-offline");
    let inputs = ShopInputs {
        nhtsa_recalls: Some(nhtsa_recalls_dir(&scratch)),
        nhtsa_mfr_comms: Some(mfr_comms_dir(&scratch)),
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
    assert!(summary.datasets.contains(Dataset::Bulletins));
    assert!(Dataset::Bulletins.hosted_only());
    assert!(!Dataset::RecallsUs.hosted_only());

    let offline = scratch.path("offline.sqlite3");
    let kept = offline_copy(&scratch.out(), &offline).unwrap();
    assert!(kept.contains(Dataset::RecallsUs));
    assert!(!kept.contains(Dataset::Bulletins));

    // The full file is untouched and still holds bulletins.
    let full = ShopData::new(ShopSource::open(scratch.out()).unwrap()).unwrap();
    assert!(full.datasets().contains(Dataset::Bulletins));
    // The copy is a shop file of the same version with one dataset fewer.
    let copy = ShopData::new(ShopSource::open(&offline).unwrap()).unwrap();
    assert_eq!(copy.meta(), full.meta());
    assert_eq!(copy.datasets().len(), 1);
    assert_eq!(
        copy.datasets().get(Dataset::RecallsUs),
        full.datasets().get(Dataset::RecallsUs)
    );
    assert!(copy.require(Dataset::Bulletins).is_err());
    let connection = Connection::open(&offline).unwrap();
    assert_eq!(
        rows(
            &connection,
            "SELECT name FROM sqlite_schema WHERE type = 'table' ORDER BY name"
        ),
        [
            "component",
            "meta",
            "recall",
            "recall_component",
            "recall_vehicle"
        ]
    );
    assert_eq!(rows(&connection, "SELECT COUNT(*) FROM recall"), ["8"]);

    // It never writes over a file that is there.
    let error = offline_copy(&scratch.out(), &offline).unwrap_err();
    assert!(format!("{error:#}").contains("already exists"), "{error:#}");
    // Every table named for a hosted-only dataset is one the schema creates.
    for (id, tables) in schema::HOSTED_ONLY_TABLES {
        assert_eq!(*id, "bulletins");
        for table in *tables {
            let created = format!("CREATE TABLE {table} (");
            assert!(
                [
                    schema::BULLETINS,
                    schema::BULLETIN_COMPONENTS,
                    schema::BULLETIN_TEXT
                ]
                .iter()
                .any(|ddl| ddl.contains(&created)),
                "{table}"
            );
        }
    }
}
