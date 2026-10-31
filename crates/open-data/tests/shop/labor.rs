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

// ----- measured times -----

fn observed_fixture() -> String {
    std::fs::read_to_string(fixture("labor").join("observed.yaml")).unwrap()
}

/// A scratch directory with the expert fixtures and `table` as the
/// measured table.
fn with_observed(name: &str, table: &str) -> Scratch {
    let files = fixtures();
    let files: Vec<(&str, &str)> = files
        .iter()
        .map(|(name, text)| (*name, text.as_str()))
        .collect();
    let scratch = prepared(name, &files);
    let directory = scratch.curated().join("labor/observed");
    std::fs::create_dir_all(&directory).unwrap();
    std::fs::write(directory.join("observed.yaml"), table).unwrap();
    std::fs::write(directory.join("_example.yaml"), "rows: [nonsense").unwrap();
    scratch
}

#[test]
fn a_measured_table_is_built_beside_the_written_estimates() {
    let scratch = with_observed("labor-observed", &observed_fixture());
    let summary = build(&scratch, true).unwrap();
    let entry = summary.datasets.get(Dataset::Labor).unwrap();
    // Five written estimates and two measured rows; the newest day is the
    // table's.
    assert_eq!((entry.as_of.as_str(), entry.rows), ("2026-10-13", 7));
    let connection = Connection::open(scratch.out()).unwrap();
    assert_eq!(
        rows(
            &connection,
            "SELECT job_id, scope, class, model_id, year_from, year_to, hours_low, hours_typical, hours_high, basis, sample_jobs, sample_shops, author, reviewer, updated FROM labor_estimate WHERE basis = 'observed' ORDER BY id"
        ),
        [
            "test-job-a|class|compact-car|-|-|-|9500|9600|9700|observed|40|6|-|-|2026-10-13",
            "test-job-b|generation|-|7001|2002|2003|4700|4800|4900|observed|20|5|-|-|2026-10-13",
        ]
    );
    drop(connection);
    let shop = ShopData::new(ShopSource::open(scratch.out()).unwrap()).unwrap();
    let answer = |year: u16, engine: Option<&str>, job: &str| {
        let found = shop
            .labor_for_vehicle(7001, year, engine, Some(job))
            .unwrap();
        found.first().map(|estimate| {
            (
                estimate.scope.clone(),
                estimate.basis.clone(),
                estimate.hours_typical,
            )
        })
    };
    let one =
        |scope: &str, basis: &str, hours: i64| Some((scope.to_owned(), basis.to_owned(), hours));
    // The 2001 Sampler is a compact car. Job A has narrower written
    // figures, and they still answer first.
    assert_eq!(
        answer(2001, None, "test-job-a"),
        one("generation", "expert", 7800)
    );
    // Job B on the 2003: the measured generation figure, before the
    // written figure for its class.
    assert_eq!(
        answer(2003, None, "test-job-b"),
        one("generation", "observed", 4800)
    );
    // The class figure of job A: measured before written, same scope.
    let class = shop.class_estimates_for_job("test-job-a").unwrap();
    assert_eq!(
        (
            class[0].basis.as_str(),
            class[0].hours_typical,
            class[0].sample_jobs,
            class[0].sample_shops
        ),
        ("observed", 9600, Some(40), Some(6))
    );
}

#[test]
fn a_measured_table_alone_is_a_dataset() {
    let scratch = prepared("labor-observed-alone", &[]);
    let directory = scratch.curated().join("labor/observed");
    std::fs::create_dir_all(&directory).unwrap();
    std::fs::write(directory.join("observed.yaml"), observed_fixture()).unwrap();
    let summary = build(&scratch, true).unwrap();
    let entry = summary.datasets.get(Dataset::Labor).unwrap();
    assert_eq!((entry.as_of.as_str(), entry.rows), ("2026-10-13", 2));
    assert_eq!(entry.licence, "cc-by-4.0");
}

#[test]
fn a_table_that_is_not_fit_to_publish_stops_the_build() {
    let table = observed_fixture();
    let privacy = "nothing that could identify a shop, a customer, a VIN or a date";
    let cases = [
        // Under the threshold.
        (
            "shops: 6",
            "shops: 4",
            "row 1 has 4 shops behind it; the least is 5",
        ),
        (
            "jobs: 20",
            "jobs: 19",
            "row 2 has 19 repair orders behind it; the least is 20",
        ),
        (
            "jobs: 40\n    shops: 6",
            "jobs: 40\n    shops: 41",
            "row 1 has more shops than repair orders",
        ),
        // Anything that could identify someone or something.
        ("    shops: 6\n", "    shops: 6\n    shop_id: 17\n", privacy),
        (
            "    shops: 6\n",
            "    shops: 6\n    shop_name: Test Garage\n",
            privacy,
        ),
        (
            "    shops: 6\n",
            "    shops: 6\n    customer: A. Person\n",
            privacy,
        ),
        (
            "    shops: 6\n",
            "    shops: 6\n    vin: 1FTEW1EP5JFA00000\n",
            privacy,
        ),
        (
            "    shops: 6\n",
            "    shops: 6\n    completed: 2026-09-30\n",
            privacy,
        ),
        (
            "    shops: 6\n",
            "    shops: 6\n    hours: [9500, 9600]\n",
            privacy,
        ),
        (
            "generated: 2026-10-13\n",
            "generated: 2026-10-13\nshop: Test Garage\n",
            privacy,
        ),
        (
            "model: sampler",
            "model: 1ftew1ep5jfa00000",
            "row 2 holds what looks like a VIN",
        ),
        // The figures.
        (
            "median: 9600",
            "median: 9400",
            "row 1: low, median and high must be in that order",
        ),
        (
            "low: 9500",
            "low: 0",
            "row 1: low must be hundredths of an hour from 1 to 99999",
        ),
        ("median: 9600", "median: 96.5", "observed.yaml"),
        (
            "generated: 2026-10-13",
            "generated: last month",
            "generated must be a day",
        ),
        // The same checks an expert estimate gets.
        (
            "class: compact-car",
            "class: half-ton-pickup",
            "half-ton-pickup is not a class",
        ),
        (
            "job: test-job-b",
            "job: test-job-z",
            "test-job-z is not a job of jobs.yaml",
        ),
        (
            "year_to: 2003",
            "year_to: 2004",
            "the catalog has no 2004 testmake sampler",
        ),
    ];
    for (index, (from, to, expected)) in cases.into_iter().enumerate() {
        assert!(table.contains(from), "{from}");
        let scratch = with_observed(
            &format!("labor-observed-bad-{index}"),
            &table.replacen(from, to, 1),
        );
        let message = format!("{:#}", build(&scratch, true).unwrap_err());
        assert!(message.contains(expected), "{expected}: {message}");
        assert!(!scratch.out().exists(), "{expected}");
    }
    // The same row twice, and two tables.
    let twice = format!(
        "{table}  - job: test-job-a\n    scope: class\n    class: compact-car\n    median: 9600\n    low: 9500\n    high: 9700\n    jobs: 40\n    shops: 6\n"
    );
    let scratch = with_observed("labor-observed-twice", &twice);
    let message = format!("{:#}", build(&scratch, true).unwrap_err());
    assert!(
        message.contains("test-job-a has two measured class rows for compact-car"),
        "{message}"
    );
    let scratch = with_observed("labor-observed-two", &table);
    std::fs::write(scratch.curated().join("labor/observed/older.yaml"), &table).unwrap();
    let message = format!("{:#}", build(&scratch, true).unwrap_err());
    assert!(
        message.contains("holds 2 tables; a new table replaces the old one"),
        "{message}"
    );
}

#[test]
fn the_threshold_is_the_one_the_reader_names() {
    assert_eq!(
        (
            wenmar_shopdata::labor::MIN_SHOPS,
            wenmar_shopdata::labor::MIN_JOBS
        ),
        (5, 20)
    );
    let data = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../data");
    let example = std::fs::read_to_string(data.join("labor/observed/_example.yaml")).unwrap();
    let table = labor::parse_observed(&example).unwrap();
    assert_eq!(table.generated, "2026-10-12");
    assert_eq!(table.rows.len(), 2);
    assert!(
        table
            .rows
            .iter()
            .all(|row| row.shops >= 5 && row.jobs >= 20)
    );
}

// ----- figures worked out by a model -----

/// An AI file for test-job-a: a model figure for the Other (narrower than
/// any written figure it has) and one for the Sampler (where a written
/// model figure exists and wins). Every figure is impossible on purpose.
const AI_FILE: &str = r#"
job: test-job-a
method: test-method-1
model: test-model
generated: 2026-10-30
estimates:
  - scope: model
    make: testmake
    model: other
    low: "2.7"
    typical: "2.8"
    high: "2.9"
    confidence: 45
    inputs: [vehicle, job, procedure]
    notes: "Made-up assumption."
  - scope: model
    make: testmake
    model: sampler
    low: "1"
    typical: "1.1"
    high: "1.2"
    confidence: 20
    inputs: [vehicle, job]
"#;

/// The expert fixtures, plus `text` as `labor/ai/test-job-a.yaml`.
fn with_ai(name: &str, text: &str) -> Scratch {
    let files = fixtures();
    let files: Vec<(&str, &str)> = files
        .iter()
        .map(|(name, text)| (*name, text.as_str()))
        .collect();
    let scratch = prepared(name, &files);
    let directory = scratch.curated().join("labor/ai");
    std::fs::create_dir_all(&directory).unwrap();
    std::fs::write(directory.join("test-job-a.yaml"), text).unwrap();
    std::fs::write(directory.join("_example.yaml"), "job: [nonsense").unwrap();
    scratch
}

#[test]
fn ai_figures_are_built_with_how_they_were_worked_out() {
    let scratch = with_ai("labor-ai-built", AI_FILE);
    let summary = build(&scratch, true).unwrap();
    let labor = summary.datasets.get(Dataset::Labor).unwrap();
    assert_eq!((labor.as_of.as_str(), labor.rows), ("2026-10-30", 7));
    let ai = summary.datasets.get(Dataset::LaborAi).unwrap();
    assert_eq!(
        (ai.as_of.as_str(), ai.rows, ai.licence.as_str()),
        ("2026-10-30", 2, "cc-by-4.0")
    );
    let connection = Connection::open(scratch.out()).unwrap();
    assert_eq!(
        rows(
            &connection,
            "SELECT e.job_id, e.scope, e.model_id, e.hours_typical, e.basis, e.notes, e.author, e.updated, a.confidence_pct, a.model, a.method, a.inputs
             FROM labor_estimate e JOIN labor_ai a ON a.estimate_id = e.id ORDER BY e.id"
        ),
        [
            "test-job-a|model|7002|280|ai|Made-up assumption.|-|2026-10-30|45|test-model|test-method-1|vehicle,job,procedure",
            "test-job-a|model|7001|110|ai|-|-|2026-10-30|20|test-model|test-method-1|vehicle,job",
        ]
    );
    drop(connection);
    let shop = ShopData::new(ShopSource::open(scratch.out()).unwrap()).unwrap();
    // The Other has only the AI figure; the Sampler's written model figure
    // comes before the AI one of the same scope.
    let other = shop
        .labor_for_vehicle(7002, 2001, None, Some("test-job-a"))
        .unwrap();
    assert_eq!(
        (other[0].basis.as_str(), other[0].hours_typical),
        ("ai", 280)
    );
    assert_eq!(other[0].confidence_pct, Some(45));
    assert_eq!(other[0].inputs, ["vehicle", "job", "procedure"]);
    let sampler = shop
        .labor_for_vehicle(7001, 2003, None, Some("test-job-a"))
        .unwrap();
    assert_eq!(
        (sampler[0].basis.as_str(), sampler[0].hours_typical),
        ("expert", 8800)
    );
}

#[test]
fn without_ai_files_there_is_no_labor_ai_dataset() {
    let files = fixtures();
    let files: Vec<(&str, &str)> = files
        .iter()
        .map(|(name, text)| (*name, text.as_str()))
        .collect();
    let scratch = prepared("labor-no-ai", &files);
    let summary = build(&scratch, true).unwrap();
    assert!(!summary.datasets.contains(Dataset::LaborAi));
    let connection = Connection::open(scratch.out()).unwrap();
    assert!(
        rows(
            &connection,
            "SELECT name FROM sqlite_master WHERE name = 'labor_ai'"
        )
        .is_empty()
    );
}

#[test]
fn an_ai_answer_outside_the_guardrails_stops_the_build() {
    let cases = [
        (
            "confidence: 45",
            "confidence: 61",
            "confidence must be from 0 to 60",
        ),
        ("high: \"2.9\"", "high: \"40.01\"", "from 0.01 to 40"),
        (
            "low: \"2.7\"\n    typical: \"2.8\"\n    high: \"2.9\"",
            "low: \"1\"\n    typical: \"1\"\n    high: \"3.5\"",
            "more than 3 times typical",
        ),
        ("typical: \"2.8\"", "typical: \"3\"", "in that order"),
        (
            "inputs: [vehicle, job, procedure]",
            "inputs: [vehicle, labour-guide]",
            "inputs must be",
        ),
        (
            "inputs: [vehicle, job, procedure]",
            "inputs: []",
            "inputs must be",
        ),
        ("model: test-model", "model: \"\"", "model must be a name"),
        (
            "generated: 2026-10-30",
            "generated: soon",
            "generated must be a day",
        ),
        (
            "job: test-job-a",
            "job: test-job-b",
            "holds the job test-job-b",
        ),
        (
            "    model: other\n",
            "    model: nothing\n",
            "has no model nothing",
        ),
        (
            "    notes: \"Made-up assumption.\"\n",
            "    author: someone\n",
            "author",
        ),
        (
            "    notes: \"Made-up assumption.\"\n",
            "    llm: \"\"\n",
            "llm must be a name",
        ),
    ];
    for (index, (from, to, expected)) in cases.into_iter().enumerate() {
        assert!(AI_FILE.contains(from), "{from}");
        let text = AI_FILE.replacen(from, to, 1);
        let scratch = with_ai(&format!("labor-ai-bad-{index}"), &text);
        let error = format!("{:#}", build(&scratch, true).unwrap_err());
        assert!(error.contains(expected), "{expected}: {error}");
    }
}

#[test]
fn the_ai_directory_in_the_repository_is_sound() {
    let data = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../data");
    for (name, text) in open_data::shop::codes::files(&data.join("labor/ai")).unwrap() {
        labor::parse_ai(&name, &text).unwrap();
    }
}

/// A file as Wenmar's labour generator writes it, kept here so a change to
/// the build that stops reading the generator's files shows.
const GENERATED: &str = r#"# Written by Wenmar's labour generator: AI estimates, not checked by a person.
# Do not edit by hand; run the generator again.
job: test-job-a
method: "open-labour-ai-1"
model: "claude-opus-5-5"
generated: 2026-10-30
estimates:
  - scope: class
    class: compact-car
    low: "0.4"
    typical: "0.5"
    high: "1.25"
    confidence: 45
    inputs: [vehicle, job]
    notes: "A \"quoted\" note."
"#;

#[test]
fn the_generators_file_is_read_by_the_build() {
    let records = labor::parse_ai("test-job-a", GENERATED).unwrap();
    assert_eq!(records.len(), 1);
    let record = &records[0];
    assert_eq!((record.low, record.typical, record.high), (40, 50, 125));
    assert_eq!(record.notes.as_deref(), Some("A \"quoted\" note."));
    assert_eq!(record.llm, "claude-opus-5-5");
    assert_eq!(record.method, "open-labour-ai-1");
}

#[test]
fn a_figure_may_name_its_own_model_and_method() {
    let text = GENERATED.replace(
        "    notes: \"A \\\"quoted\\\" note.\"\n",
        "    notes: \"A \\\"quoted\\\" note.\"\n    llm: \"deepseek-flash\"\n    method: \"open-labour-ai-2\"\n",
    );
    assert_ne!(text, GENERATED);
    let record = &labor::parse_ai("test-job-a", &text).unwrap()[0];
    assert_eq!(record.llm, "deepseek-flash");
    assert_eq!(record.method, "open-labour-ai-2");
}
