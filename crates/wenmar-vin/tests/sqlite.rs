use rusqlite::Connection;
use wenmar_vin::sqlite::{SCHEMA, SCHEMA_VERSION, SqliteData};
use wenmar_vin::{DecodeOptions, Decoder, Element, VinData};

const KEY: &str = "K2CAB|PU001140";

fn connection() -> Connection {
    let connection = Connection::open_in_memory().unwrap();
    connection.execute_batch(SCHEMA).unwrap();
    connection
        .execute_batch(&format!(
            "INSERT INTO meta VALUES ('schema_version', '{SCHEMA_VERSION}'), ('data_version', '2026.09');
             INSERT INTO wmi VALUES ('KM8', 'Hyundai Motor Co', 'Hyundai', 'South Korea', 'Multipurpose Passenger Vehicle (MPV)', 1);
             INSERT INTO wmi VALUES ('1M8', 'Motor Coach Industries', NULL, NULL, 'Bus', 0);
             INSERT INTO wmi_schema VALUES ('KM8', 1, 2022, NULL), ('KM8', 2, 1990, 1995);
             INSERT INTO pattern VALUES
               (10, 1, 'K2***', 28, 'Kona', '2022-05-01 00:00:00', 'Hyundai'),
               (11, 1, 'K[2-3]CA', 38, 'SE', '2022-05-01 00:00:00', NULL),
               (12, 1, 'K9***', 38, 'Does Not Match', '2022-05-01 00:00:00', NULL),
               (13, 1, '*****|*U', 31, 'Ulsan', '2022-05-01 00:00:00', NULL),
               (14, 1, 'K2***', 96, 'Internal Element', '2022-05-01 00:00:00', NULL),
               (15, 2, 'K2***', 28, 'Old Model', '1995-01-01 00:00:00', 'Hyundai');"
        ))
        .unwrap();
    connection
}

fn data() -> SqliteData {
    SqliteData::from_connection(connection()).unwrap()
}

#[test]
fn reads_a_manufacturer() {
    let found = data().manufacturer("KM8").unwrap().unwrap();
    assert_eq!(found.wmi, "KM8");
    assert_eq!(found.name, "Hyundai Motor Co");
    assert_eq!(found.make.as_deref(), Some("Hyundai"));
    assert_eq!(found.country.as_deref(), Some("South Korea"));
    assert!(found.light_vehicle);
    assert!(!data().manufacturer("1M8").unwrap().unwrap().light_vehicle);
    assert_eq!(data().manufacturer("ZZZ").unwrap(), None);
}

#[test]
fn finds_schemas_for_a_year() {
    let data = data();
    let schemas = data.schemas("KM8", 2023).unwrap();
    assert_eq!(schemas.len(), 1);
    assert_eq!((schemas[0].id, schemas[0].year_from), (1, 2022));
    assert_eq!(data.schemas("KM8", 1995).unwrap()[0].id, 2);
    assert!(data.schemas("KM8", 2005).unwrap().is_empty());
}

#[test]
fn returns_matching_patterns_with_the_make_beside_the_model() {
    let patterns = data().patterns(&[1], KEY).unwrap();
    let mut found: Vec<(Element, &str)> = patterns
        .iter()
        .map(|row| (row.element, row.value.as_str()))
        .collect();
    found.sort();
    assert_eq!(
        found,
        vec![
            (Element::Make, "Hyundai"),
            (Element::Model, "Kona"),
            (Element::Trim, "SE"),
            (Element::PlantCity, "Ulsan"),
        ]
    );
    let model = patterns
        .iter()
        .find(|row| row.element == Element::Model)
        .unwrap();
    let make = patterns
        .iter()
        .find(|row| row.element == Element::Make)
        .unwrap();
    assert_eq!((make.schema_id, &make.keys), (model.schema_id, &model.keys));
    assert_eq!(model.changed_on, "2022-05-01 00:00:00");
}

#[test]
fn an_empty_schema_list_returns_nothing() {
    assert!(data().patterns(&[], KEY).unwrap().is_empty());
}

#[test]
fn decodes_through_the_decoder() {
    let options = DecodeOptions {
        model_year: None,
        current_year: Some(2026),
    };
    let decoded = Decoder::new(data())
        .decode("KM8K2CAB4PU001140", options)
        .unwrap();
    assert_eq!(decoded.year, Some(2023));
    assert_eq!(decoded.make.as_deref(), Some("Hyundai"));
    assert_eq!(decoded.model.as_deref(), Some("Kona"));
    assert_eq!(decoded.trim.as_deref(), Some("SE"));
    assert_eq!(decoded.plant.city.as_deref(), Some("Ulsan"));
    assert!(decoded.warnings.is_empty());
}

#[test]
fn reads_meta() {
    assert_eq!(
        data().meta("data_version").unwrap().as_deref(),
        Some("2026.09")
    );
    assert_eq!(data().meta("missing").unwrap(), None);
}

#[test]
fn refuses_a_database_that_is_not_a_data_file() {
    let empty = Connection::open_in_memory().unwrap();
    let error = SqliteData::from_connection(empty).unwrap_err().to_string();
    assert!(error.contains("not a Wenmar Open data file"), "{error}");
}

#[test]
fn refuses_a_data_file_with_another_schema_version() {
    let connection = connection();
    connection
        .execute(
            "UPDATE meta SET value = '999' WHERE key = 'schema_version'",
            [],
        )
        .unwrap();
    let error = SqliteData::from_connection(connection)
        .unwrap_err()
        .to_string();
    assert!(error.contains("schema version 999"), "{error}");
}

#[test]
fn opening_a_missing_file_is_an_error() {
    assert!(SqliteData::open("/nonexistent/wenmar-open.sqlite3").is_err());
}
