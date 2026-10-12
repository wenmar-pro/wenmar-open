use open_data::shop::coverage::{
    CoverageTable, DatasetCoverage, Report, Thresholds, Unmatched, failures, light_makes, run,
    table, yaml,
};
use rusqlite::Connection;
use wenmar_shopdata::schema::{META, SCHEMA_VERSION};
use wenmar_shopdata::sqlite::ShopSource;
use wenmar_shopdata::{Dataset, ShopData};

use crate::common::Scratch;

/// Light makes in the fixture: BMW, Ford, Chevrolet, Honda. THOR (12503)
/// is in the catalog and is not light.
const LIGHT: [i64; 4] = [452, 460, 467, 474];

/// A table that meets the contract, plus a column the tests select on.
const ROWS: &str = "
CREATE TABLE fixture_vehicle (
    kind TEXT NOT NULL, year INTEGER, make_id INTEGER, model_id INTEGER,
    make_text TEXT NOT NULL, model_text TEXT NOT NULL, \"match\" TEXT
);
INSERT INTO fixture_vehicle VALUES
  ('us', 2019, 474, 1863, 'HONDA', 'CIVIC', 'exact'),
  ('us', 2018, 474, 1863, 'HONDA', 'CIVIC SI', 'base'),
  ('us', 2019, 467, 1850, 'CHEVROLET', 'SILVERADO 1500', 'base'),
  ('us', 2015, 452, 1709, 'BMW', '3 SERIES', 'curated'),
  ('us', 2015, 452, 1711, 'BMW', '3 SERIES', 'curated'),
  ('us', 2016, 452, NULL, 'BMW', 'M3 CONVERTIBLE', NULL),
  ('us', 2017, 452, NULL, 'BMW', 'M3 CONVERTIBLE', NULL),
  ('us', 2016, 474, NULL, 'HONDA', 'CIVIC DEL SOL', NULL),
  ('us', 1999, 474, 1863, 'HONDA', 'CIVIC', 'exact'),
  ('us', 9999, 474, 1863, 'HONDA', 'CIVIC', 'exact'),
  ('us', NULL, 474, NULL, 'HONDA', 'UNKNOWN', NULL),
  ('us', 2019, 12503, NULL, 'THOR', 'ACE', NULL),
  ('us', 2019, NULL, NULL, 'NEWMAR', 'DUTCH STAR', NULL),
  ('ca', 2019, 474, 1863, 'HONDA', 'CIVIC', 'exact');
";

const US: CoverageTable = CoverageTable {
    dataset: Dataset::RecallsUs,
    table: "(SELECT * FROM fixture_vehicle WHERE kind = 'us')",
};
const CA: CoverageTable = CoverageTable {
    dataset: Dataset::RecallsCa,
    table: "(SELECT * FROM fixture_vehicle WHERE kind = 'ca')",
};
/// Registered, but the fixture shop file does not hold it.
const BULLETINS: CoverageTable = CoverageTable {
    dataset: Dataset::Bulletins,
    table: "fixture_vehicle",
};

fn shop(extra: &str) -> ShopData<ShopSource> {
    let connection = Connection::open_in_memory().unwrap();
    connection.execute_batch(META).unwrap();
    connection.execute_batch(ROWS).unwrap();
    connection.execute_batch(extra).unwrap();
    let entry = r#"{"as_of":"2026-10-11","rows":1,"licence":"public-domain","attribution":""}"#;
    connection
        .execute_batch(&format!(
            "INSERT INTO meta VALUES
               ('schema_version', '{SCHEMA_VERSION}'),
               ('data_version', '2026.10.12'),
               ('catalog_data_version', '2026.09'),
               ('built_at', '2026-10-12 06:00:00'),
               ('datasets', '{{\"recalls_us\":{entry},\"recalls_ca\":{entry}}}');"
        ))
        .unwrap();
    ShopData::new(ShopSource::from_connection(connection)).unwrap()
}

fn report() -> Report {
    run(&shop(""), &LIGHT, &[US, CA, BULLETINS]).unwrap()
}

fn minimums(yaml: &str) -> Thresholds {
    Thresholds::parse(yaml).unwrap()
}

#[test]
fn measures_light_makes_from_model_year_2000() {
    let report = report();
    assert_eq!(
        report.datasets,
        [
            DatasetCoverage {
                dataset: "recalls_us".to_owned(),
                rows: 13,
                measured: 8,
                exact: 1,
                base: 2,
                curated: 2,
                unmatched: 3,
                top_unmatched: vec![
                    Unmatched {
                        make_text: "BMW".to_owned(),
                        model_text: "M3 CONVERTIBLE".to_owned(),
                        rows: 2,
                    },
                    Unmatched {
                        make_text: "HONDA".to_owned(),
                        model_text: "CIVIC DEL SOL".to_owned(),
                        rows: 1,
                    },
                ],
            },
            DatasetCoverage {
                dataset: "recalls_ca".to_owned(),
                rows: 1,
                measured: 1,
                exact: 1,
                ..DatasetCoverage::default()
            },
        ]
    );
    assert_eq!(report.datasets[0].share_tenths(), 625);
    assert_eq!(report.datasets[0].share(), "62.5");
    assert_eq!(report.datasets[1].share(), "100.0");
    assert_eq!(DatasetCoverage::default().share(), "0.0");
}

#[test]
fn lists_at_most_25_unmatched_names_commonest_first() {
    let mut extra = String::new();
    for index in 0..30 {
        for _ in 0..=index {
            extra.push_str(&format!(
                "INSERT INTO fixture_vehicle VALUES ('us', 2020, 460, NULL, 'FORD', 'M{index:02}', NULL);"
            ));
        }
    }
    let report = run(&shop(&extra), &LIGHT, &[US]).unwrap();
    let top = &report.datasets[0].top_unmatched;
    assert_eq!(top.len(), 25);
    assert_eq!((top[0].model_text.as_str(), top[0].rows), ("M29", 30));
    assert_eq!((top[24].model_text.as_str(), top[24].rows), ("M05", 6));
}

#[test]
fn prints_the_report() {
    assert_eq!(
        table(&report()),
        "\
recalls_us: 13 rows, 8 measured (light makes, model year 2000 on)
  matched 62.5%: 1 exact, 2 base, 2 curated; 3 unmatched
  most common unmatched:
         2  BMW / M3 CONVERTIBLE
         1  HONDA / CIVIC DEL SOL
recalls_ca: 1 rows, 1 measured (light makes, model year 2000 on)
  matched 100.0%: 1 exact, 0 base, 0 curated; 0 unmatched
"
    );
    assert_eq!(
        table(&Report::default()),
        "no dataset in this shop file has vehicle rows to measure\n"
    );
}

#[test]
fn reads_the_minimums() {
    let read = minimums("# a comment\ndatasets:\n  recalls_us: 62.5\n  recalls_ca: 100.0\n");
    assert_eq!(read.datasets["recalls_us"], 62.5);
    assert_eq!(read.datasets.len(), 2);
    assert!(minimums("datasets: {}\n").datasets.is_empty());
    for (text, expected) in [
        (
            "datasets:\n  recalls: 50.0\n",
            "coverage.yaml: recalls is not a dataset",
        ),
        (
            "datasets:\n  recalls_us: 101.0\n",
            "coverage.yaml: recalls_us must be from 0 to 100",
        ),
        (
            "datasets:\n  recalls_us: -1.0\n",
            "coverage.yaml: recalls_us must be from 0 to 100",
        ),
        ("dataset: {}\n", "coverage.yaml"),
    ] {
        let error = Thresholds::parse(text).unwrap_err();
        assert!(format!("{error:#}").contains(expected), "{error:#}");
    }
}

#[test]
fn fails_below_a_minimum_and_passes_at_it() {
    let report = report();
    let at = minimums("datasets:\n  recalls_us: 62.5\n  recalls_ca: 100.0\n");
    assert_eq!(failures(&report, &at), Vec::<String>::new());
    let above = minimums("datasets:\n  recalls_us: 62.6\n  recalls_ca: 100.0\n");
    assert_eq!(
        failures(&report, &above),
        ["recalls_us: 62.5% of measured rows matched, below the minimum of 62.6%"]
    );
}

#[test]
fn a_dataset_without_a_minimum_and_a_minimum_without_a_dataset_both_fail() {
    let report = report();
    let partial = minimums("datasets:\n  recalls_us: 10.0\n  fuel_us: 20.0\n");
    assert_eq!(
        failures(&report, &partial),
        [
            "recalls_ca has no minimum in coverage.yaml; run open-data coverage with --write and commit the file",
            "fuel_us has a minimum in coverage.yaml but is not in this shop file",
        ]
    );
}

#[test]
fn writes_minimums_that_read_back_and_pass() {
    let report = report();
    let text = yaml(&report);
    assert!(
        text.ends_with("datasets:\n  recalls_us: 62.5\n  recalls_ca: 100.0\n"),
        "{text}"
    );
    assert!(text.starts_with("# Minimum share"), "{text}");
    assert_eq!(failures(&report, &minimums(&text)), Vec::<String>::new());
    assert!(yaml(&Report::default()).ends_with("datasets: {}\n"));
    assert!(minimums(&yaml(&Report::default())).datasets.is_empty());
}

#[test]
fn a_match_value_nobody_defined_is_refused() {
    let shop = shop(
        "INSERT INTO fixture_vehicle VALUES ('us', 2020, 460, 1801, 'FORD', 'F-150', 'fuzzy');",
    );
    let error = run(&shop, &LIGHT, &[US]).unwrap_err();
    assert!(
        format!("{error:#}").contains("has the match value \"fuzzy\""),
        "{error:#}"
    );
}

#[test]
fn reads_the_light_makes_from_the_core_file() {
    let scratch = Scratch::new("coverage-light");
    let connection = Connection::open(scratch.core()).unwrap();
    let light = light_makes(&connection).unwrap();
    assert_eq!(light.len(), 11);
    assert!(light.contains(&474));
    assert!(!light.contains(&12503));
    // With no light make at all, nothing is measured and nothing fails.
    let report = run(&shop(""), &[], &[US]).unwrap();
    assert_eq!(
        (report.datasets[0].rows, report.datasets[0].measured),
        (13, 0)
    );
}

#[test]
fn the_minimums_in_the_repository_load() {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../data/coverage.yaml");
    Thresholds::load(&path).unwrap();
    // `--write` must not rewrite the comment at the top of the file.
    let header = yaml(&Report::default());
    let header = header.trim_end_matches("datasets: {}\n");
    assert!(std::fs::read_to_string(&path).unwrap().starts_with(header));
    let error = Thresholds::load(std::path::Path::new("/nonexistent/coverage.yaml")).unwrap_err();
    assert!(format!("{error:#}").contains("coverage.yaml"), "{error:#}");
}
