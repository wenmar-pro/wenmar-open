use rusqlite::Connection;
use wenmar_shopdata::schema::{META, SCHEMA_VERSION};
use wenmar_shopdata::sqlite::ShopSource;
use wenmar_shopdata::{Dataset, ShopData, ShopError, check_pairing};
use wenmar_vehicles::Source;

/// A shop file holding only `meta`, with one dataset listed.
fn connection() -> Connection {
    let connection = Connection::open_in_memory().unwrap();
    connection.execute_batch(META).unwrap();
    connection
        .execute_batch(&format!(
            r#"INSERT INTO meta VALUES
                 ('schema_version', '{SCHEMA_VERSION}'),
                 ('data_version', '2026.10.12'),
                 ('catalog_data_version', '2026.09'),
                 ('built_at', '2026-10-12 06:00:00'),
                 ('datasets', '{{"recalls_us":{{"as_of":"2026-10-11","rows":3,"licence":"public-domain","attribution":""}}}}');"#
        ))
        .unwrap();
    connection
}

fn shop(connection: Connection) -> Result<ShopData<ShopSource>, ShopError> {
    ShopData::new(ShopSource::from_connection(connection))
}

#[test]
fn reads_meta_and_the_datasets() {
    let shop = shop(connection()).unwrap();
    assert_eq!(shop.meta().schema_version, "1");
    assert_eq!(shop.data_version(), "2026.10.12");
    assert_eq!(shop.catalog_data_version(), "2026.09");
    assert_eq!(shop.built_at(), "2026-10-12 06:00:00");
    assert_eq!(shop.datasets().len(), 1);
    assert_eq!(shop.datasets().get(Dataset::RecallsUs).unwrap().rows, 3);
}

#[test]
fn require_passes_for_a_held_dataset_and_names_a_missing_one() {
    let shop = shop(connection()).unwrap();
    shop.require(Dataset::RecallsUs).unwrap();
    let error = shop.require(Dataset::Bulletins).unwrap_err();
    assert!(matches!(
        error,
        ShopError::DatasetUnavailable(Dataset::Bulletins)
    ));
    assert_eq!(error.to_string(), "The bulletins dataset is not available.");
}

#[test]
fn refuses_a_file_of_another_schema_version() {
    let connection = connection();
    connection
        .execute(
            "UPDATE meta SET value = '2' WHERE key = 'schema_version'",
            [],
        )
        .unwrap();
    assert_eq!(
        shop(connection).unwrap_err().to_string(),
        "shop file has schema version 2, this build reads version 1; rebuild it with open-data build-shop"
    );
}

#[test]
fn refuses_a_file_that_is_not_a_shop_file() {
    let empty = Connection::open_in_memory().unwrap();
    assert_eq!(
        shop(empty).unwrap_err().to_string(),
        "not a Wenmar Open shop file: it has no meta table"
    );
    for key in [
        "schema_version",
        "data_version",
        "catalog_data_version",
        "datasets",
    ] {
        let connection = connection();
        connection
            .execute("DELETE FROM meta WHERE key = ?1", [key])
            .unwrap();
        assert_eq!(
            shop(connection).unwrap_err().to_string(),
            format!("not a Wenmar Open shop file: meta has no {key}")
        );
    }
}

#[test]
fn a_file_without_built_at_still_opens() {
    let connection = connection();
    connection
        .execute("DELETE FROM meta WHERE key = 'built_at'", [])
        .unwrap();
    assert_eq!(shop(connection).unwrap().built_at(), "");
}

#[test]
fn a_shop_file_pairs_only_with_the_core_file_it_was_matched_against() {
    let shop = shop(connection()).unwrap();
    shop.check_catalog("2026.09").unwrap();
    assert_eq!(
        shop.check_catalog("2026.10").unwrap_err().to_string(),
        "shop file was matched against data version 2026.09, but the data file is version 2026.10; rebuild it with open-data build-shop --catalog <the data file>"
    );
    check_pairing("2026.09", "2026.09").unwrap();
    assert!(matches!(
        check_pairing("2026.09", "2026.09.1"),
        Err(ShopError::Pairing { .. })
    ));
}

#[test]
fn queries_run_through_the_source() {
    let shop = shop(connection()).unwrap();
    let rows = shop
        .query(
            "SELECT value FROM meta WHERE key = ?1",
            &["data_version".into()],
        )
        .unwrap();
    assert_eq!(rows[0][0].text(), Some("2026.10.12"));
    let error = shop.query("SELECT * FROM no_such_table", &[]).unwrap_err();
    assert!(matches!(error, ShopError::Source(_)));
    assert!(shop.source().query("SELECT 1", &[]).is_ok());
}
