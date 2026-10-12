use rusqlite::Connection;
use wenmar_shopdata::ShopData;
use wenmar_shopdata::schema::{META, SCHEMA_VERSION, SIBLINGS};
use wenmar_shopdata::siblings::{MOST_SIBLINGS, Sibling};
use wenmar_shopdata::sqlite::ShopSource;

fn shop(with_table: bool) -> ShopData<ShopSource> {
    let connection = Connection::open_in_memory().unwrap();
    connection.execute_batch(META).unwrap();
    for (key, value) in [
        ("schema_version", SCHEMA_VERSION),
        ("data_version", "2026.10.12"),
        ("catalog_data_version", "2026.09"),
        ("built_at", "2026-10-12 06:00:00"),
        ("datasets", "{}"),
    ] {
        connection
            .execute(
                "INSERT INTO meta (key, value) VALUES (?1, ?2)",
                [key, value],
            )
            .unwrap();
    }
    if with_table {
        connection.execute_batch(SIBLINGS).unwrap();
        connection
            .execute_batch(
                "INSERT INTO vehicle_sibling VALUES
                   (1850, 2019, 1857, 773, 953), (1850, 2019, 10, 9, 40), (1850, 2019, 11, 9, 40),
                   (1850, 2019, 12, 8, 40), (1850, 2019, 13, 7, 40), (1850, 2019, 14, 6, 30),
                   (1850, 2020, 1857, 500, 600), (1857, 2019, 1850, 773, 900);",
            )
            .unwrap();
    }
    ShopData::new(ShopSource::from_connection(connection)).unwrap()
}

#[test]
fn a_model_year_gets_its_siblings_most_shared_first() {
    let shop = shop(true);
    let found = shop.siblings(1850, 2019).unwrap();
    assert_eq!(found.len(), MOST_SIBLINGS);
    assert_eq!(
        found[0],
        Sibling {
            model_id: 1857,
            shared: 773,
            total: 953
        }
    );
    // Equal counts: the lower model id first.
    let ids: Vec<i64> = found.iter().map(|sibling| sibling.model_id).collect();
    assert_eq!(ids, [1857, 10, 11, 12, 13]);
    assert!(shop.siblings(1850, 2018).unwrap().is_empty());
    assert!(shop.siblings(1, 2019).unwrap().is_empty());
}

#[test]
fn a_file_without_the_table_has_no_siblings() {
    assert!(shop(false).siblings(1850, 2019).unwrap().is_empty());
}
