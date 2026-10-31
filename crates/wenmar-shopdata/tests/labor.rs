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
    shop_with(datasets, &[])
}

/// [`shop`], then `extra` SQL batches run on the file.
fn shop_with(datasets: &str, extra: &[&str]) -> ShopData<ShopSource> {
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
    for batch in extra {
        connection.execute_batch(batch).unwrap();
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

/// A file with the `labor_ai` dataset: three made-up model figures for
/// model 7002, model year 2001 a compact car. Every figure is impossible
/// on purpose.
const AI_ENTRIES: &str = r#""jobs":{"as_of":"2026-10-11","rows":2,"licence":"mit","attribution":""},"labor":{"as_of":"2026-10-30","rows":9,"licence":"cc-by-4.0","attribution":"Labour estimates by Wenmar Pro, CC BY 4.0"},"labor_ai":{"as_of":"2026-10-30","rows":2,"licence":"cc-by-4.0","attribution":"Labour estimates by Wenmar Pro, CC BY 4.0"}"#;

const AI_ROWS: &str = r#"
INSERT INTO vehicle_class VALUES (7002, 2001, 'compact-car', 'epa');
INSERT INTO labor_estimate VALUES
  (8, 'test-job-a', 'model', NULL, 7000, 7002, NULL, NULL, NULL, 2700, 2800, 2900, 'ai', NULL, NULL, 'Made-up assumption.', NULL, NULL, '2026-10-30'),
  (9, 'test-job-a', 'model', NULL, 7000, 7002, NULL, NULL, NULL, 1700, 1800, 1900, 'expert', NULL, NULL, NULL, 'test-author', 'test-reviewer', '2026-10-30'),
  (10, 'test-job-b', 'model', NULL, 7000, 7002, NULL, NULL, NULL, 600, 700, 800, 'ai', NULL, NULL, NULL, NULL, NULL, '2026-10-30');
INSERT INTO labor_ai VALUES
  (8, 45, 'test-model', 'test-method-1', 'vehicle,job,procedure'),
  (10, 30, 'test-model', 'test-method-1', 'vehicle,job');
"#;

fn ai_shop(with_dataset: bool) -> ShopData<ShopSource> {
    shop_with(
        if with_dataset { AI_ENTRIES } else { ENTRIES },
        &[wenmar_shopdata::schema::LABOR_AI, AI_ROWS],
    )
}

#[test]
fn a_written_figure_comes_before_a_models_of_the_same_scope() {
    let shop = ai_shop(true);
    let found = shop.labor_for_vehicle(7002, 2001, None, None).unwrap();
    let answers: Vec<(&str, &str, i64)> = found
        .iter()
        .map(|e| (e.job_id.as_str(), e.basis.as_str(), e.hours_typical))
        .collect();
    // Job A: the expert model figure, not the AI one. Job B: the AI model
    // figure, narrower than its class figures (none for a compact car).
    assert_eq!(
        answers,
        [("test-job-a", "expert", 1800), ("test-job-b", "ai", 700)]
    );
    let ai = &found[1];
    assert_eq!(ai.confidence_pct, Some(30));
    assert_eq!(ai.model.as_deref(), Some("test-model"));
    assert_eq!(ai.method.as_deref(), Some("test-method-1"));
    assert_eq!(ai.inputs, ["vehicle", "job"]);
    assert_eq!(found[0].confidence_pct, None);
    assert!(found[0].inputs.is_empty());
}

#[test]
fn a_file_without_the_labor_ai_dataset_still_reads() {
    let shop = ai_shop(false);
    assert!(!shop.holds_labor_ai());
    let found = shop
        .labor_for_vehicle(7002, 2001, None, Some("test-job-b"))
        .unwrap();
    assert_eq!(found[0].basis, "ai");
    assert_eq!(found[0].confidence_pct, None);
    assert!(found[0].inputs.is_empty());
}

#[test]
fn a_jobs_vehicles_are_its_figures_narrower_than_class() {
    // A third job with nothing but a class figure: a class is not a
    // vehicle of its own.
    let class_only = r#"
INSERT INTO job VALUES ('test-job-c', 'Test job C', 'engine', '[]', '[]', '[]', 2);
INSERT INTO labor_estimate VALUES
  (11, 'test-job-c', 'class', 'compact-car', NULL, NULL, NULL, NULL, NULL, 500, 600, 700, 'expert', NULL, NULL, NULL, 'test-author', 'test-reviewer', '2026-10-30');
"#;
    let shop = shop_with(
        AI_ENTRIES,
        &[wenmar_shopdata::schema::LABOR_AI, AI_ROWS, class_only],
    );
    let found = shop.job_vehicles("test-job-a", 0, 100).unwrap();
    let answers: Vec<(i64, &str, &str, Option<i64>)> = found
        .iter()
        .map(|estimate| {
            (
                estimate.model_id.unwrap(),
                estimate.scope.as_str(),
                estimate.basis.as_str(),
                estimate.confidence_pct,
            )
        })
        .collect();
    // The model, generation and engine figures, in make, model and year
    // order; a model's written figure before a model's.
    assert_eq!(
        answers,
        [
            (7001, "model", "expert", None),
            (7001, "generation", "expert", None),
            (7001, "engine", "expert", None),
            (7002, "model", "ai", Some(45)),
            (7002, "model", "expert", None),
        ]
    );
    let years: Vec<(Option<u16>, Option<u16>)> = found
        .iter()
        .map(|estimate| (estimate.year_from, estimate.year_to))
        .collect();
    assert_eq!(
        years,
        [
            (None, None),
            (Some(2001), Some(2001)),
            (Some(2001), Some(2002)),
            (None, None),
            (None, None),
        ]
    );
    assert_eq!(found[0].make_id, Some(7000));
    assert_eq!(found[2].engine.as_deref(), Some("9-9l-test"));
    assert_eq!(found[2].hours_typical, 6800);
    assert_eq!(found[3].job_name, "Test job A");
    assert_eq!(found[3].model.as_deref(), Some("test-model"));
    // One page, and a job whose figures are all class figures has no
    // vehicles.
    let page = shop.job_vehicles("test-job-a", 2, 2).unwrap();
    assert_eq!(page.len(), 2);
    assert_eq!(page[0].scope, "engine");
    assert_eq!(page[1].confidence_pct, Some(45));
    assert!(shop.job_vehicles("test-job-c", 0, 100).unwrap().is_empty());
}

#[test]
fn the_method_counts_say_what_the_file_holds() {
    let shop = ai_shop(true);
    let counts = shop.labor_counts().unwrap();
    assert_eq!(counts.figures, 10);
    assert_eq!(
        counts.by_basis,
        [
            ("expert".to_owned(), 7),
            ("observed".to_owned(), 1),
            ("ai".to_owned(), 2),
        ]
    );
    assert_eq!(counts.jobs, 2);
    assert_eq!(counts.models, 2);
    assert_eq!(counts.makes, 1);
    assert_eq!(counts.ai_with_steps, 1);
    assert_eq!(counts.ai_answered_by, ["test-model"]);
    assert_eq!(counts.ai_methods, ["test-method-1"]);
    assert_eq!(counts.newest.as_deref(), Some("2026-10-30"));
    // A file built before the AI figures said how they were made counts
    // the figures and says nothing about how they were worked out.
    let old = ai_shop(false).labor_counts().unwrap();
    assert_eq!(old.figures, 10);
    assert_eq!(old.by_basis, counts.by_basis);
    assert_eq!(old.ai_with_steps, 0);
    assert!(old.ai_answered_by.is_empty());
    assert!(old.ai_methods.is_empty());
}
