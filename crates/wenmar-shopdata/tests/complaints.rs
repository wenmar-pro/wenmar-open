use rusqlite::Connection;
use serde_json::json;
use wenmar_shopdata::complaints::{FEWEST_SHOWN, NOTE, km};
use wenmar_shopdata::schema::{COMPLAINTS, COMPONENTS, META, SCHEMA_VERSION};
use wenmar_shopdata::sqlite::ShopSource;
use wenmar_shopdata::{Dataset, ShopData, ShopError};

const HELD: &str =
    r#""complaints":{"as_of":"2026-10-11","rows":9,"licence":"public-domain","attribution":""}"#;

/// Counts for two model years of the server's test catalog: the Kona of
/// 2023 (model 900) and of 2022. The figures are made up.
pub const ROWS: &str = "
INSERT INTO component VALUES
  ('steering', 'Steering', NULL),
  ('power-steering', 'Power steering', 'steering'),
  ('electrical', 'Electrical system', NULL),
  ('engine', 'Engine', NULL),
  ('brakes', 'Brakes', NULL);
INSERT INTO complaint_count VALUES
  (498, 900, 2023, '', 60, 3, 1, 2, 40, 30000),
  (498, 900, 2023, 'steering', 25, 1, 0, 1, 20, 39000),
  (498, 900, 2023, 'power-steering', 12, 0, 0, 0, 4, 41000),
  (498, 900, 2023, 'electrical', 25, 2, 1, 0, 12, 25500),
  (498, 900, 2023, 'engine', 5, 0, 0, 1, 5, 61000),
  (498, 900, 2023, 'brakes', 4, 1, 0, 0, 4, 12000),
  (498, 900, 2022, '', 3, 0, 0, 0, 2, 8000),
  (498, 900, 2022, 'engine', 3, 0, 0, 0, 2, 8000);
";

fn shop(datasets: &str, with_rows: bool) -> ShopData<ShopSource> {
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
    if with_rows {
        connection.execute_batch(COMPONENTS).unwrap();
        connection.execute_batch(COMPLAINTS).unwrap();
        connection.execute_batch(ROWS).unwrap();
    }
    ShopData::new(ShopSource::from_connection(connection)).unwrap()
}

#[test]
fn a_model_year_gets_its_total_and_its_most_reported_components() {
    let shop = shop(HELD, true);
    assert_eq!(shop.complaints_as_of(), Some("2026-10-11"));
    let found = shop.complaints_for_vehicle(900, 2023).unwrap();
    assert_eq!(
        serde_json::to_value(&found.total).unwrap(),
        json!({ "complaints": 60, "crashes": 3, "fires": 1, "injuries": 2, "median_km": 48280, "median_miles": 30000 })
    );
    assert_eq!(
        serde_json::to_value(&found.components).unwrap(),
        json!([
            // Equal counts: by name.
            { "id": "electrical", "name": "Electrical system", "parent": null,
              "complaints": 25, "crashes": 2, "fires": 1, "injuries": 0, "median_km": 41038, "median_miles": 25500 },
            { "id": "steering", "name": "Steering", "parent": null,
              "complaints": 25, "crashes": 1, "fires": 0, "injuries": 1, "median_km": 62764, "median_miles": 39000 },
            // Four mileages are too few for a median.
            { "id": "power-steering", "name": "Power steering", "parent": "steering",
              "complaints": 12, "crashes": 0, "fires": 0, "injuries": 0, "median_km": null, "median_miles": null },
            { "id": "engine", "name": "Engine", "parent": null,
              "complaints": 5, "crashes": 0, "fires": 0, "injuries": 1, "median_km": 98170, "median_miles": 61000 }
        ])
    );
    // Brakes, with four complaints, is not listed.
    assert!(
        found
            .components
            .iter()
            .all(|entry| entry.figures.complaints >= FEWEST_SHOWN)
    );
}

#[test]
fn few_complaints_show_a_total_and_no_component() {
    let shop = shop(HELD, true);
    let found = shop.complaints_for_vehicle(900, 2022).unwrap();
    assert_eq!(found.total.complaints, 3);
    // Two mileages: no median.
    assert_eq!(found.total.median_miles, None);
    assert!(found.components.is_empty());
    // A model year with no row is all zeros, not an error.
    let none = shop.complaints_for_vehicle(900, 1999).unwrap();
    assert_eq!(none.total.complaints, 0);
    assert_eq!(none.total.median_km, None);
    assert!(none.components.is_empty());
}

#[test]
fn kilometres_are_rounded_from_miles() {
    assert_eq!(km(0), 0);
    assert_eq!(km(1), 2);
    assert_eq!(km(30_000), 48_280);
    assert_eq!(km(100_000), 160_934);
}

#[test]
fn the_note_says_what_a_complaint_is_not() {
    for phrase in ["owners", "NHTSA", "not verified", "not a rate"] {
        assert!(NOTE.contains(phrase), "{phrase}");
    }
    for banned in ["defect is", "known problem", "common failure", "unreliable"] {
        assert!(!NOTE.to_lowercase().contains(banned), "{banned}");
    }
}

#[test]
fn a_file_with_no_complaints_names_the_dataset_and_runs_no_sql() {
    let shop = shop("", false);
    assert_eq!(shop.complaints_as_of(), None);
    let error = shop.complaints_for_vehicle(900, 2023).unwrap_err();
    assert!(matches!(
        error,
        ShopError::DatasetUnavailable(Dataset::Complaints)
    ));
    assert_eq!(
        error.to_string(),
        "The complaints dataset is not available."
    );
}
