use rusqlite::Connection;
use wenmar_shopdata::schema::{COMPONENTS, JOBS, META, SCHEMA_VERSION};
use wenmar_shopdata::sqlite::ShopSource;
use wenmar_shopdata::{Dataset, ShopData, ShopError};

/// Three made-up jobs, in the order of a file that lists the last first.
const ROWS: &str = r#"
INSERT INTO component VALUES
  ('engine', 'Engine', NULL),
  ('timing-belt', 'Timing belt or chain', 'engine'),
  ('electrical', 'Electrical system', NULL);
INSERT INTO job VALUES
  ('test-job-b', 'Test job B', 'engine', '["made-up step one","made-up step two"]', '["made-up exclusion"]', '["test-job-a"]', 1),
  ('test-job-a', 'Test job A', 'engine', '["made-up step"]', '[]', '[]', 0),
  ('test-job-c', 'Test job C', 'electrical', '["made-up step"]', '[]', '["test-job-b","test-job-a"]', 2);
INSERT INTO job_component VALUES
  ('test-job-b', 'timing-belt', 0), ('test-job-b', 'engine', 1), ('test-job-c', 'electrical', 0);
"#;

fn shop(held: bool) -> ShopData<ShopSource> {
    let connection = Connection::open_in_memory().unwrap();
    connection.execute_batch(META).unwrap();
    let datasets = if held {
        r#"{"jobs":{"as_of":"2026-10-12","rows":3,"licence":"mit","attribution":""}}"#
    } else {
        "{}"
    };
    for (key, value) in [
        ("schema_version", SCHEMA_VERSION),
        ("data_version", "2026.10.12"),
        ("catalog_data_version", "2026.09"),
        ("built_at", "2026-10-12 06:00:00"),
        ("datasets", datasets),
    ] {
        connection
            .execute(
                "INSERT INTO meta (key, value) VALUES (?1, ?2)",
                [key, value],
            )
            .unwrap();
    }
    if held {
        connection.execute_batch(COMPONENTS).unwrap();
        connection.execute_batch(JOBS).unwrap();
        connection.execute_batch(ROWS).unwrap();
    }
    ShopData::new(ShopSource::from_connection(connection)).unwrap()
}

#[test]
fn the_jobs_are_listed_in_the_order_of_the_file() {
    let shop = shop(true);
    assert_eq!(shop.jobs_as_of(), Some("2026-10-12"));
    let listed: Vec<(String, String, String)> = shop
        .jobs()
        .unwrap()
        .into_iter()
        .map(|job| (job.id, job.name, job.system))
        .collect();
    assert_eq!(
        listed,
        [
            (
                "test-job-a".to_owned(),
                "Test job A".to_owned(),
                "engine".to_owned()
            ),
            (
                "test-job-b".to_owned(),
                "Test job B".to_owned(),
                "engine".to_owned()
            ),
            (
                "test-job-c".to_owned(),
                "Test job C".to_owned(),
                "electrical".to_owned()
            ),
        ]
    );
}

#[test]
fn a_job_is_read_with_its_components_and_related_jobs() {
    let shop = shop(true);
    let job = shop.job("test-job-b").unwrap().unwrap();
    assert_eq!(job.name, "Test job B");
    assert_eq!(job.system, "engine");
    assert_eq!(job.system_name, "Engine");
    assert_eq!(job.includes, ["made-up step one", "made-up step two"]);
    assert_eq!(job.excludes, ["made-up exclusion"]);
    let components: Vec<(&str, Option<&str>)> = job
        .components
        .iter()
        .map(|component| (component.id.as_str(), component.parent.as_deref()))
        .collect();
    assert_eq!(
        components,
        [("timing-belt", Some("engine")), ("engine", None)]
    );
    assert_eq!(job.related.len(), 1);
    assert_eq!(
        (job.related[0].id.as_str(), job.related[0].name.as_str()),
        ("test-job-a", "Test job A")
    );
    // Related jobs come in the record's order.
    let last = shop.job("test-job-c").unwrap().unwrap();
    let related: Vec<&str> = last.related.iter().map(|job| job.id.as_str()).collect();
    assert_eq!(related, ["test-job-b", "test-job-a"]);
    assert!(shop.job("test-job-z").unwrap().is_none());
}

#[test]
fn a_file_without_the_dataset_says_so() {
    let shop = shop(false);
    assert_eq!(shop.jobs_as_of(), None);
    for error in [
        shop.jobs().unwrap_err(),
        shop.job("test-job-a").unwrap_err(),
    ] {
        assert!(
            matches!(error, ShopError::DatasetUnavailable(Dataset::Jobs)),
            "{error}"
        );
        assert_eq!(error.to_string(), "The jobs dataset is not available.");
    }
}
