use std::path::Path;

use open_data::shop::build::{ShapeContext, build_with};
use open_data::shop::components::Components;
use open_data::shop::jobs::Jobs;
use open_data::shop::labor::{self, hundredths, parse};
use rusqlite::Connection;
use wenmar_shopdata::sqlite::ShopSource;
use wenmar_shopdata::{Dataset, DatasetEntry, ShopData, licence, schema};

use crate::common::{COMPONENTS_YAML, Scratch, fixture, info, rows};

/// A make, two models and their engines that do not exist. Model 7001
/// has model years 2001 to 2003; only 2001 and 2002 list engines.
const TEST_VEHICLE: &str = "
INSERT INTO catalog_make VALUES (7000, 'testmake', 'Testmake', 'testmake', NULL, 4, 1);
INSERT INTO catalog_model VALUES
  (7001, 7000, 'sampler', 'Sampler', 'sampler', 2001, 2003, 4, 1),
  (7002, 7000, 'other', 'Other', 'other', 2001, 2001, 4, 1);
INSERT INTO catalog_vehicle VALUES
  (9001, 2001, 7000, 7001, 4, 1, 70001),
  (9002, 2002, 7000, 7001, 4, 1, 70002),
  (9003, 2003, 7000, 7001, 4, 1, NULL),
  (9004, 2001, 7000, 7002, 4, 1, NULL);
INSERT INTO catalog_engine VALUES
  (70011, 70001, '9.9L Test', NULL, 'preset'),
  (70012, 70001, '8.8L Test Diesel', NULL, 'preset'),
  (70021, 70002, '9.9L Test', NULL, 'preset');
";

const JOBS_YAML: &str = "
status: reviewed
updated: 2026-10-11
jobs:
  - id: test-job-a
    name: Test job A
    system: engine
    includes: [made-up step]
  - id: test-job-b
    name: Test job B
    system: engine
    includes: [made-up step]
";

/// EPA rows for the made-up vehicle: the Sampler of 2001 is rated twice
/// as a compact car and once as a midsize car, that of 2002 as a small
/// pickup, and the Other as a special purpose vehicle.
const FUEL_ROWS: &str = "
INSERT INTO fuel_rating (id, source, source_id, year, make_id, model_id, make_text, model_text, fuel) VALUES
  (1, 'epa', 101, 2001, 7000, 7001, 'Testmake', 'Sampler', 'Regular Gasoline'),
  (2, 'epa', 102, 2001, 7000, 7001, 'Testmake', 'Sampler', 'Regular Gasoline'),
  (3, 'epa', 103, 2001, 7000, 7001, 'Testmake', 'Sampler Wagon', 'Regular Gasoline'),
  (4, 'epa', 104, 2002, 7000, 7001, 'Testmake', 'Sampler', 'Regular Gasoline'),
  (5, 'epa', 105, 2001, 7000, 7002, 'Testmake', 'Other', 'Regular Gasoline'),
  (6, 'epa', 106, 2001, 7000, NULL, 'Testmake', 'Unmatched', 'Regular Gasoline');
INSERT INTO vehicle_spec (epa_id, size_class, turbo, supercharger) VALUES
  (101, 'Compact Cars', 0, 0), (102, 'Compact Cars', 0, 0), (103, 'Midsize Cars', 0, 0),
  (104, 'Small Pickup Trucks 2WD', 0, 0), (105, 'Special Purpose Vehicles', 0, 0),
  (106, 'Large Cars', 0, 0);
";

/// A scratch directory whose catalog has the made-up vehicle and whose
/// curated directory has the reviewed job list and `files` under `labor`.
fn prepared(name: &str, files: &[(&str, &str)]) -> Scratch {
    let scratch = Scratch::new(name);
    let connection = Connection::open(scratch.core()).unwrap();
    connection.execute_batch(TEST_VEHICLE).unwrap();
    connection.close().unwrap();
    std::fs::write(scratch.curated().join("jobs.yaml"), JOBS_YAML).unwrap();
    let directory = scratch.curated().join("labor");
    std::fs::create_dir_all(&directory).unwrap();
    for (file, text) in files {
        std::fs::write(directory.join(file), text).unwrap();
    }
    scratch
}

fn fixtures() -> Vec<(&'static str, String)> {
    ["test-job-a.yaml", "test-job-b.yaml"]
        .into_iter()
        .map(|file| {
            (
                file,
                std::fs::read_to_string(fixture("labor").join(file)).unwrap(),
            )
        })
        .collect()
}

/// The jobs, the fuel tables with `FUEL_ROWS` when `with_fuel`, then the
/// labour step: what `shape_all` does in that order.
fn shape(context: &mut ShapeContext<'_>, with_fuel: bool) -> anyhow::Result<()> {
    open_data::shop::jobs::shape(context)?;
    if with_fuel {
        context.transaction.execute_batch(schema::FUEL)?;
        context.transaction.execute_batch(FUEL_ROWS)?;
        context.register(
            Dataset::Specs,
            DatasetEntry {
                as_of: "2026-09-29".to_owned(),
                rows: 6,
                licence: licence::PUBLIC_DOMAIN.to_owned(),
                attribution: String::new(),
            },
        )?;
    }
    labor::shape(context)?;
    open_data::shop::build::after_sources(context)
}

fn build(
    scratch: &Scratch,
    with_fuel: bool,
) -> anyhow::Result<open_data::shop::build::ShopSummary> {
    build_with(
        &scratch.core(),
        &scratch.out(),
        &scratch.curated(),
        &info(),
        |context| shape(context, with_fuel),
    )
}

#[test]
fn the_estimates_are_built_and_the_classes_worked_out() {
    let files = fixtures();
    let files: Vec<(&str, &str)> = files
        .iter()
        .map(|(name, text)| (*name, text.as_str()))
        .collect();
    let scratch = prepared("labor-built", &files);
    let summary = build(&scratch, true).unwrap();
    let entry = summary.datasets.get(Dataset::Labor).unwrap();
    assert_eq!(
        (entry.as_of.as_str(), entry.rows, entry.licence.as_str()),
        ("2026-10-12", 5, "cc-by-4.0")
    );
    assert_eq!(
        entry.attribution,
        "Labour estimates by Wenmar Pro, CC BY 4.0"
    );
    let connection = Connection::open(scratch.out()).unwrap();
    assert_eq!(
        rows(
            &connection,
            "SELECT job_id, scope, class, make_id, model_id, year_from, year_to, engine, hours_low, hours_typical, hours_high, basis, sample_jobs, sample_shops, notes, author, reviewer, updated FROM labor_estimate ORDER BY id"
        ),
        [
            "test-job-a|class|compact-car|-|-|-|-|-|9750|9800|9925|expert|-|-|Made-up note.|test-author|test-reviewer|2026-10-10",
            "test-job-a|model|-|7000|7001|-|-|-|8700|8800|8900|expert|-|-|-|test-author|test-reviewer|2026-10-10",
            "test-job-a|generation|-|7000|7001|2001|2001|-|7700|7800|7900|expert|-|-|-|test-author|test-reviewer|2026-10-11",
            "test-job-a|engine|-|7000|7001|2001|2002|9-9l-test|6700|6800|6900|expert|-|-|-|test-author|test-reviewer|2026-10-12",
            "test-job-b|class|small-pickup|-|-|-|-|-|5700|5800|5900|expert|-|-|-|test-author|other-reviewer|2026-10-12",
        ]
    );
    // The class with the most EPA rows; the nearest year where a model
    // year has none; nothing for a model EPA put in no mapped class.
    assert_eq!(
        rows(
            &connection,
            "SELECT model_id, year, class, source FROM vehicle_class ORDER BY model_id, year"
        ),
        [
            "7001|2001|compact-car|epa",
            "7001|2002|small-pickup|epa",
            "7001|2003|small-pickup|nearest",
        ]
    );
    drop(connection);
    // The reader's lookup over the built file.
    let shop = ShopData::new(ShopSource::open(scratch.out()).unwrap()).unwrap();
    let typical = |year: u16, engine: Option<&str>| -> Vec<(String, String, i64)> {
        shop.labor_for_vehicle(7001, year, engine, None)
            .unwrap()
            .into_iter()
            .map(|estimate| (estimate.job_id, estimate.scope, estimate.hours_typical))
            .collect()
    };
    let one = |job: &str, scope: &str, hours: i64| (job.to_owned(), scope.to_owned(), hours);
    assert_eq!(
        typical(2001, Some("9-9l-test")),
        [one("test-job-a", "engine", 6800)]
    );
    assert_eq!(typical(2001, None), [one("test-job-a", "generation", 7800)]);
    assert_eq!(
        typical(2003, None),
        [
            one("test-job-a", "model", 8800),
            one("test-job-b", "class", 5800)
        ]
    );
}

#[test]
fn without_the_fuel_tables_no_vehicle_has_a_class() {
    let files = fixtures();
    let files: Vec<(&str, &str)> = files
        .iter()
        .map(|(name, text)| (*name, text.as_str()))
        .collect();
    let scratch = prepared("labor-no-fuel", &files);
    build(&scratch, false).unwrap();
    let connection = Connection::open(scratch.out()).unwrap();
    assert_eq!(
        rows(&connection, "SELECT COUNT(*) FROM vehicle_class"),
        ["0"]
    );
    assert_eq!(
        rows(&connection, "SELECT COUNT(*) FROM labor_estimate"),
        ["5"]
    );
}

#[test]
fn a_directory_with_no_estimate_adds_nothing() {
    let scratch = prepared("labor-none", &[("_example.yaml", "job: [nonsense")]);
    let summary = build(&scratch, true).unwrap();
    assert!(!summary.datasets.contains(Dataset::Labor));
    let connection = Connection::open(scratch.out()).unwrap();
    assert!(
        rows(
            &connection,
            "SELECT name FROM sqlite_master WHERE name IN ('labor_estimate', 'vehicle_class')"
        )
        .is_empty()
    );
}

#[test]
fn hours_are_read_as_whole_hundredths() {
    for (text, expected) in [
        ("1", Some(100)),
        ("1.5", Some(150)),
        ("0.25", Some(25)),
        ("0.01", Some(1)),
        ("999.99", Some(99_999)),
        ("12.30", Some(1230)),
        ("0", None),
        ("0.00", None),
        ("1000", None),
        ("1.234", None),
        ("1.", None),
        (".5", None),
        ("-1", None),
        ("1,5", None),
        ("one", None),
        ("", None),
    ] {
        assert_eq!(hundredths(text), expected, "{text}");
    }
}

/// One estimate of the fixture's kinds, with one line changed.
fn changed(from: &str, to: &str) -> String {
    let text = std::fs::read_to_string(fixture("labor").join("test-job-a.yaml")).unwrap();
    assert!(text.contains(from), "{from}");
    text.replacen(from, to, 1)
}

#[test]
fn a_mistake_in_a_file_is_named() {
    let cases = [
        (
            "job: test-job-a",
            "job: test-job-b",
            "test-job-a.yaml holds the job test-job-b",
        ),
        ("scope: class", "scope: fleet", "the scope fleet"),
        (
            "class: compact-car",
            "class: half-ton-pickup",
            "half-ton-pickup is not a class",
        ),
        (
            "    class: compact-car\n",
            "",
            "a class estimate needs class",
        ),
        (
            "    class: compact-car\n",
            "    class: compact-car\n    make: testmake\n",
            "a class estimate takes no make",
        ),
        (
            "    model: sampler\n    low: \"87\"",
            "    low: \"87\"",
            "a model estimate needs make and model",
        ),
        (
            "    year_to: 2001\n",
            "",
            "a generation estimate needs year_from and year_to",
        ),
        (
            "year_from: 2001\n    year_to: 2001",
            "year_from: 2002\n    year_to: 2001",
            "year_from is after year_to",
        ),
        (
            "    engine: 9-9l-test\n",
            "",
            "an engine estimate needs engine",
        ),
        (
            "low: \"97.5\"",
            "low: \"98.5\"",
            "low, typical and high must be in that order",
        ),
        (
            "typical: \"98\"",
            "typical: \"1.234\"",
            "typical must be hours like",
        ),
        ("basis: expert", "basis: observed", "basis must be expert"),
        (
            "reviewer: test-reviewer",
            "reviewer: test-author",
            "the reviewer is the author",
        ),
        (
            "updated: 2026-10-10",
            "updated: soon",
            "updated must be a day",
        ),
        (
            "    basis: expert\n",
            "    basis: expert\n    vin: 1FTEW1EP5JFA00000\n",
            "test-job-a.yaml",
        ),
    ];
    for (from, to, expected) in cases {
        let error = parse("test-job-a", &changed(from, to)).unwrap_err();
        let message = format!("{error:#}");
        assert!(message.contains(expected), "{expected}: {message}");
    }
    assert!(parse("test-job-a", &changed("scope: class", "scope: class")).is_ok());
}

#[test]
fn a_mistake_against_the_catalog_or_the_job_list_is_named() {
    let components = Components::parse(COMPONENTS_YAML).unwrap();
    let cases = [
        (
            "make: testmake\n    model: sampler\n    low: \"87\"",
            "make: nomake\n    model: sampler\n    low: \"87\"",
            "nomake is not a make of the catalog",
        ),
        (
            "model: sampler\n    low: \"87\"",
            "model: nomodel\n    low: \"87\"",
            "testmake has no model nomodel",
        ),
        (
            "year_from: 2001\n    year_to: 2001",
            "year_from: 2001\n    year_to: 2004",
            "the catalog has no 2004 testmake sampler",
        ),
        (
            "engine: 9-9l-test",
            "engine: 8-8l-test-diesel",
            "the 2002 testmake sampler has no engine 8-8l-test-diesel",
        ),
        (
            "year_from: 2001\n    year_to: 2002\n    engine",
            "year_from: 2001\n    year_to: 2003\n    engine",
            "the 2003 testmake sampler has no engine 9-9l-test",
        ),
    ];
    for (index, (from, to, expected)) in cases.into_iter().enumerate() {
        let text = changed(from, to);
        let scratch = prepared(&format!("labor-bad-{index}"), &[("test-job-a.yaml", &text)]);
        let error = build(&scratch, true).unwrap_err();
        let message = format!("{error:#}");
        assert!(message.contains(expected), "{expected}: {message}");
        assert!(!scratch.out().exists());
    }
    // The same scope twice, and model years that overlap.
    let twice = changed(
        "  - scope: model",
        "  - scope: class\n    class: compact-car\n    low: \"1\"\n    typical: \"1\"\n    high: \"1\"\n    basis: expert\n    author: test-author\n    reviewer: test-reviewer\n    updated: 2026-10-10\n  - scope: model",
    );
    let overlap = changed(
        "  - scope: engine",
        "  - scope: generation\n    make: testmake\n    model: sampler\n    year_from: 2001\n    year_to: 2002\n    low: \"1\"\n    typical: \"1\"\n    high: \"1\"\n    basis: expert\n    author: test-author\n    reviewer: test-reviewer\n    updated: 2026-10-10\n  - scope: engine",
    );
    for (name, text, expected) in [
        (
            "labor-twice",
            twice,
            "test-job-a has two class estimates for compact-car",
        ),
        (
            "labor-overlap",
            overlap,
            "test-job-a has two generation estimates that cover the 2001 testmake sampler",
        ),
    ] {
        let scratch = prepared(name, &[("test-job-a.yaml", &text)]);
        let message = format!("{:#}", build(&scratch, true).unwrap_err());
        assert!(message.contains(expected), "{expected}: {message}");
    }
    // A job that is not in the list, and a list that is still a draft.
    let scratch = prepared(
        "labor-no-job",
        &[("test-job-z.yaml", "job: test-job-z\nestimates: []\n")],
    );
    let message = format!("{:#}", build(&scratch, true).unwrap_err());
    assert!(
        message.contains("test-job-z is not a job of jobs.yaml"),
        "{message}"
    );
    let text = changed("scope: class", "scope: class");
    let scratch = prepared("labor-draft", &[("test-job-a.yaml", &text)]);
    std::fs::write(
        scratch.curated().join("jobs.yaml"),
        JOBS_YAML.replace("status: reviewed", "status: draft"),
    )
    .unwrap();
    let message = format!("{:#}", build(&scratch, true).unwrap_err());
    assert!(
        message.contains("data/jobs.yaml is still a draft"),
        "{message}"
    );
    assert!(Jobs::parse(JOBS_YAML, &components).is_ok());
}

#[test]
fn the_directory_in_the_repository_is_sound_and_its_example_is_a_good_file() {
    let data = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../data");
    let example = std::fs::read_to_string(data.join("labor/_example.yaml")).unwrap();
    let records = parse("test-job-a", &example).unwrap();
    assert_eq!(records.len(), 4);
    // Whatever real files exist have the shape; the catalog checks need a
    // data file and run in a build.
    for (name, text) in open_data::shop::codes::files(&data.join("labor")).unwrap() {
        parse(&name, &text).unwrap();
    }
}
