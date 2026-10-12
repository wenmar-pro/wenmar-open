use rusqlite::Connection;
use wenmar_shopdata::labor::{CLASSES, class_name, class_of_epa};
use wenmar_shopdata::schema::{COMPONENTS, JOBS, LABOR, META, SCHEMA_VERSION};
use wenmar_shopdata::sqlite::ShopSource;
use wenmar_shopdata::{Dataset, ShopData, ShopError};

const ENTRIES: &str = r#""jobs":{"as_of":"2026-10-11","rows":2,"licence":"mit","attribution":""},"labor":{"as_of":"2026-10-12","rows":7,"licence":"cc-by-4.0","attribution":"Labour estimates by Wenmar Pro, CC BY 4.0"}"#;

/// Two made-up jobs and seven made-up estimates for a vehicle that does
/// not exist: model 7001, model years 2001 to 2003. Every figure is
/// impossible on purpose. Model year 2001 is a compact car, 2002 and 2003
/// are small pickups, 2003 by its nearest year.
const ROWS: &str = r#"
INSERT INTO component VALUES ('engine', 'Engine', NULL);
INSERT INTO job VALUES
  ('test-job-b', 'Test job B', 'engine', '["made-up step"]', '[]', '[]', 1),
  ('test-job-a', 'Test job A', 'engine', '["made-up step"]', '[]', '[]', 0);
INSERT INTO vehicle_class VALUES
  (7001, 2001, 'compact-car', 'epa'), (7001, 2002, 'small-pickup', 'epa'), (7001, 2003, 'small-pickup', 'nearest');
INSERT INTO labor_estimate VALUES
  (1, 'test-job-a', 'class', 'compact-car', NULL, NULL, NULL, NULL, NULL, 9750, 9800, 9925, 'expert', NULL, NULL, 'Made-up note.', 'test-author', 'test-reviewer', '2026-10-10'),
  (2, 'test-job-a', 'model', NULL, 7000, 7001, NULL, NULL, NULL, 8700, 8800, 8900, 'expert', NULL, NULL, NULL, 'test-author', 'test-reviewer', '2026-10-10'),
  (3, 'test-job-a', 'generation', NULL, 7000, 7001, 2001, 2001, NULL, 7700, 7800, 7900, 'expert', NULL, NULL, NULL, 'test-author', 'test-reviewer', '2026-10-11'),
  (4, 'test-job-a', 'engine', NULL, 7000, 7001, 2001, 2002, '9-9l-test', 6700, 6800, 6900, 'expert', NULL, NULL, NULL, 'test-author', 'test-reviewer', '2026-10-12'),
  (5, 'test-job-b', 'class', 'small-pickup', NULL, NULL, NULL, NULL, NULL, 5700, 5800, 5900, 'expert', NULL, NULL, NULL, 'test-author', 'test-reviewer', '2026-10-12'),
  (6, 'test-job-b', 'class', 'small-pickup', NULL, NULL, NULL, NULL, NULL, 4700, 4800, 4900, 'observed', 40, 6, NULL, NULL, NULL, '2026-10-12'),
  (7, 'test-job-b', 'class', 'two-seater', NULL, NULL, NULL, NULL, NULL, 3700, 3800, 3900, 'expert', NULL, NULL, NULL, 'test-author', 'test-reviewer', '2026-10-12');
"#;

fn shop(datasets: &str) -> ShopData<ShopSource> {
    let connection = Connection::open_in_memory().unwrap();
    connection.execute_batch(META).unwrap();
    for (key, value) in [
        ("schema_version", SCHEMA_VERSION),
        ("data_version", "2026.10.12"),
        ("catalog_data_version", "2026.09"),
        ("built_at", "2026-10-12 06:00:00"),
        ("datasets", format!("{{{datasets}}}").as_str()),
    ] {
        connection
            .execute(
                "INSERT INTO meta (key, value) VALUES (?1, ?2)",
                [key, value],
            )
            .unwrap();
    }
    if !datasets.is_empty() {
        for ddl in [COMPONENTS, JOBS, LABOR] {
            connection.execute_batch(ddl).unwrap();
        }
        connection.execute_batch(ROWS).unwrap();
    }
    ShopData::new(ShopSource::from_connection(connection)).unwrap()
}

/// Job, scope and typical hundredths of each estimate of an answer.
fn answered(
    shop: &ShopData<ShopSource>,
    year: u16,
    engine: Option<&str>,
    job: Option<&str>,
) -> Vec<(String, String, i64)> {
    shop.labor_for_vehicle(7001, year, engine, job)
        .unwrap()
        .into_iter()
        .map(|estimate| (estimate.job_id, estimate.scope, estimate.hours_typical))
        .collect()
}

fn one(job: &str, scope: &str, typical: i64) -> (String, String, i64) {
    (job.to_owned(), scope.to_owned(), typical)
}

#[test]
fn the_narrowest_scope_that_has_a_figure_answers() {
    let shop = shop(ENTRIES);
    // Engine, then generation, then model, then class.
    assert_eq!(
        answered(&shop, 2001, Some("9-9l-test"), Some("test-job-a")),
        [one("test-job-a", "engine", 6800)]
    );
    assert_eq!(
        answered(&shop, 2001, Some("8-8l-test-diesel"), Some("test-job-a")),
        [one("test-job-a", "generation", 7800)]
    );
    assert_eq!(
        answered(&shop, 2001, None, Some("test-job-a")),
        [one("test-job-a", "generation", 7800)]
    );
    assert_eq!(
        answered(&shop, 2002, Some("9-9l-test"), Some("test-job-a")),
        [one("test-job-a", "engine", 6800)]
    );
    assert_eq!(
        answered(&shop, 2002, None, Some("test-job-a")),
        [one("test-job-a", "model", 8800)]
    );
    assert_eq!(
        answered(&shop, 2003, Some("9-9l-test"), Some("test-job-a")),
        [one("test-job-a", "model", 8800)]
    );
    // Another model has only its class to go by, and model 7002 has none.
    assert!(
        shop.labor_for_vehicle(7002, 2001, None, None)
            .unwrap()
            .is_empty()
    );
}

#[test]
fn every_job_of_a_vehicle_is_answered_once_in_the_order_of_the_job_list() {
    let shop = shop(ENTRIES);
    // 2001 is a compact car: job B has no figure for one.
    assert_eq!(
        answered(&shop, 2001, None, None),
        [one("test-job-a", "generation", 7800)]
    );
    // 2002 is a small pickup: job A by its model, job B by the class, and
    // the measured figure before the estimate of the same scope.
    assert_eq!(
        answered(&shop, 2002, None, None),
        [
            one("test-job-a", "model", 8800),
            one("test-job-b", "class", 4800)
        ]
    );
    let found = shop
        .labor_for_vehicle(7001, 2003, None, Some("test-job-b"))
        .unwrap();
    let estimate = &found[0];
    assert_eq!(estimate.job_name, "Test job B");
    assert_eq!(estimate.basis, "observed");
    assert_eq!(estimate.class.as_deref(), Some("small-pickup"));
    assert_eq!(
        (
            estimate.hours_low,
            estimate.hours_typical,
            estimate.hours_high
        ),
        (4700, 4800, 4900)
    );
    assert_eq!(
        (estimate.sample_jobs, estimate.sample_shops),
        (Some(40), Some(6))
    );
    assert_eq!(estimate.updated, "2026-10-12");
    let engine = &shop
        .labor_for_vehicle(7001, 2001, Some("9-9l-test"), None)
        .unwrap()[0];
    assert_eq!(
        (engine.year_from, engine.year_to, engine.engine.as_deref()),
        (Some(2001), Some(2002), Some("9-9l-test"))
    );
    assert_eq!(engine.notes, None);
    assert!(
        shop.labor_for_vehicle(7001, 2002, None, Some("no-such-job"))
            .unwrap()
            .is_empty()
    );
}

#[test]
fn a_model_year_has_a_class_or_none() {
    let shop = shop(ENTRIES);
    assert_eq!(
        shop.vehicle_class(7001, 2001).unwrap(),
        Some(("compact-car".to_owned(), "epa".to_owned()))
    );
    assert_eq!(
        shop.vehicle_class(7001, 2003).unwrap(),
        Some(("small-pickup".to_owned(), "nearest".to_owned()))
    );
    assert_eq!(shop.vehicle_class(7002, 2001).unwrap(), None);
    // What a job's own page lists: its class figures, measured first.
    let classes: Vec<(String, String, i64)> = shop
        .class_estimates_for_job("test-job-b")
        .unwrap()
        .into_iter()
        .map(|estimate| {
            (
                estimate.class.unwrap(),
                estimate.basis,
                estimate.hours_typical,
            )
        })
        .collect();
    assert_eq!(
        classes,
        [
            ("two-seater".to_owned(), "expert".to_owned(), 3800),
            ("small-pickup".to_owned(), "observed".to_owned(), 4800),
        ]
    );
}

#[test]
fn epa_size_classes_become_class_ids() {
    for (epa, class) in [
        ("Minicompact Cars", Some("subcompact-car")),
        ("Subcompact Cars", Some("subcompact-car")),
        ("Compact Cars", Some("compact-car")),
        ("Midsize Cars", Some("midsize-car")),
        ("Large Cars", Some("large-car")),
        ("Two Seaters", Some("two-seater")),
        ("Small Station Wagons", Some("station-wagon")),
        ("Midsize-Large Station Wagons", Some("station-wagon")),
        ("Small Sport Utility Vehicle 4WD", Some("small-suv")),
        ("Standard Sport Utility Vehicle 2WD", Some("standard-suv")),
        ("Sport Utility Vehicle - 4WD", Some("suv")),
        ("Minivan - 2WD", Some("minivan")),
        ("Vans, Cargo Type", Some("van")),
        ("Vans Passenger", Some("van")),
        ("Small Pickup Trucks", Some("small-pickup")),
        ("Standard Pickup Trucks/2wd", Some("standard-pickup")),
        (" standard pickup trucks 4WD ", Some("standard-pickup")),
        ("Special Purpose Vehicles", None),
        ("Special Purpose Vehicle 2WD", None),
        ("", None),
    ] {
        assert_eq!(class_of_epa(epa), class, "{epa}");
    }
    assert_eq!(CLASSES.len(), 13);
    assert_eq!(class_name("small-suv"), Some("Small SUV"));
    assert_eq!(class_name("half-ton-pickup"), None);
}

#[test]
fn a_file_without_the_dataset_says_so() {
    let shop = shop("");
    assert_eq!(shop.labor_as_of(), None);
    for error in [
        shop.labor_for_vehicle(7001, 2001, None, None).unwrap_err(),
        shop.vehicle_class(7001, 2001).unwrap_err(),
        shop.class_estimates_for_job("test-job-a").unwrap_err(),
    ] {
        assert!(
            matches!(error, ShopError::DatasetUnavailable(Dataset::Labor)),
            "{error}"
        );
        assert_eq!(error.to_string(), "The labor dataset is not available.");
    }
}
