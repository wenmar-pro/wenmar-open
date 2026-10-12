use rusqlite::Connection;
use wenmar_shopdata::schema::{
    BULLETIN_COMPONENTS, BULLETIN_TEXT, BULLETINS, COMPONENTS, META, RECALL_COMPONENTS, RECALLS,
    SCHEMA_VERSION,
};
use wenmar_shopdata::sqlite::ShopSource;
use wenmar_shopdata::{Dataset, ShopData, ShopError};

const BULLETINS_ENTRY: &str =
    r#""bulletins":{"as_of":"2026-10-11","rows":4,"licence":"public-domain","attribution":""}"#;
const RECALLS_ENTRY: &str =
    r#""recalls_us":{"as_of":"2026-10-11","rows":3,"licence":"public-domain","attribution":""}"#;

/// Four bulletins and three campaigns for a vehicle that does not exist:
/// make 7000, model 7001, model years 2001 and 2002. All text is made up.
const ROWS: &str = r#"
INSERT INTO component VALUES
  ('engine', 'Engine', NULL), ('timing-belt', 'Timing belt or chain', 'engine'),
  ('brakes', 'Brakes', NULL);
INSERT INTO bulletin VALUES
  (11, 'service_bulletin', '2025-01-01', NULL, NULL, NULL, NULL, 'ENGINE', NULL, 'Made-up summary naming P0001.'),
  (12, 'service_bulletin', '2025-02-01', NULL, NULL, NULL, NULL, 'ENGINE', NULL, 'Made-up summary about the engine.'),
  (13, 'service_bulletin', '2025-03-01', NULL, NULL, NULL, NULL, 'ENGINE', NULL, 'Made-up summary about the belt, naming P0001.'),
  (14, 'service_bulletin', '2025-04-01', NULL, NULL, NULL, NULL, 'SERVICE BRAKES', NULL, 'Made-up summary about the brakes.');
INSERT INTO bulletin_model VALUES (1, 'TESTMAKE', 'SAMPLER', 7000, 7001, 'exact');
INSERT INTO bulletin_vehicle VALUES (11, 1, 2001, 2002), (12, 1, 2001, 2001), (13, 1, 2002, 2002), (14, 1, 2001, 2001);
INSERT INTO bulletin_component VALUES (11, 'engine'), (12, 'engine'), (13, 'engine'), (13, 'timing-belt'), (14, 'brakes');
INSERT INTO bulletin_code VALUES (11, 'P0001'), (13, 'P0001');
INSERT INTO recall VALUES
  (1, 'nhtsa', '01V000001', 'Test Motors', '2001-05-01', NULL, 'ENGINE', NULL, NULL, NULL, 'Made-up summary one.', NULL, NULL, NULL, NULL, NULL, NULL, 10, 0, 0),
  (2, 'nhtsa', '01V000002', 'Test Motors', '2001-06-01', NULL, 'SERVICE BRAKES', NULL, NULL, NULL, 'Made-up summary two.', NULL, NULL, NULL, NULL, NULL, NULL, 10, 0, 0),
  (3, 'nhtsa', '02V000003', 'Test Motors', '2002-01-01', NULL, 'ENGINE', NULL, NULL, NULL, 'Made-up summary three.', NULL, NULL, NULL, NULL, NULL, NULL, 10, 0, 0);
INSERT INTO recall_vehicle VALUES
  (1, 2001, 7000, 7001, 'TESTMAKE', 'SAMPLER', 'exact', NULL, NULL),
  (2, 2001, 7000, 7001, 'TESTMAKE', 'SAMPLER', 'exact', NULL, NULL),
  (3, 2002, 7000, 7001, 'TESTMAKE', 'SAMPLER', 'exact', NULL, NULL);
INSERT INTO recall_component VALUES (1, 'engine'), (2, 'brakes'), (3, 'engine'), (3, 'timing-belt');
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
    for ddl in [
        RECALLS,
        BULLETINS,
        COMPONENTS,
        RECALL_COMPONENTS,
        BULLETIN_COMPONENTS,
        BULLETIN_TEXT,
    ] {
        connection.execute_batch(ddl).unwrap();
    }
    connection.execute_batch(ROWS).unwrap();
    ShopData::new(ShopSource::from_connection(connection)).unwrap()
}

fn list(ids: &[&str]) -> Vec<String> {
    ids.iter().map(|id| (*id).to_owned()).collect()
}

#[test]
fn a_model_years_bulletins_are_counted_by_the_code_they_name() {
    let shop = shop(&format!("{BULLETINS_ENTRY},{RECALLS_ENTRY}"));
    assert_eq!(
        shop.bulletins_naming_code_count(7001, 2001, "P0001")
            .unwrap(),
        1
    );
    assert_eq!(
        shop.bulletins_naming_code_count(7001, 2002, "P0001")
            .unwrap(),
        2
    );
    assert_eq!(
        shop.bulletins_naming_code_count(7001, 2001, "P0002")
            .unwrap(),
        0
    );
    assert_eq!(
        shop.bulletins_naming_code_count(1, 2001, "P0001").unwrap(),
        0
    );
}

#[test]
fn a_model_years_bulletins_about_components_leave_out_those_naming_the_code() {
    let shop = shop(&format!("{BULLETINS_ENTRY},{RECALLS_ENTRY}"));
    let ids = |year: u16, components: &[&str], limit: usize| -> Vec<i64> {
        shop.bulletins_about_for_vehicle(7001, year, &list(components), "P0001", limit)
            .unwrap()
            .into_iter()
            .map(|bulletin| bulletin.nhtsa_id)
            .collect()
    };
    // 11 is about the engine too, but it names the code.
    assert_eq!(ids(2001, &["engine"], 10), [12]);
    assert_eq!(ids(2001, &["engine", "brakes"], 10), [14, 12]);
    assert_eq!(ids(2001, &["engine", "brakes"], 1), [14]);
    assert_eq!(ids(2001, &["timing-belt"], 10), [] as [i64; 0]);
    assert_eq!(ids(2002, &["engine"], 10), [] as [i64; 0]);
    assert_eq!(ids(2001, &[], 10), [] as [i64; 0]);
}

#[test]
fn a_model_years_recalls_are_narrowed_to_components() {
    let shop = shop(&format!("{BULLETINS_ENTRY},{RECALLS_ENTRY}"));
    let campaigns = |year: u16, components: &[&str]| -> Vec<String> {
        shop.recalls_about_for_vehicle(7000, 7001, year, &list(components))
            .unwrap()
            .into_iter()
            .map(|recall| recall.recall.campaign)
            .collect()
    };
    assert_eq!(campaigns(2001, &["engine"]), ["01V000001"]);
    assert_eq!(
        campaigns(2001, &["brakes", "engine"]),
        ["01V000002", "01V000001"]
    );
    assert_eq!(campaigns(2002, &["timing-belt"]), ["02V000003"]);
    assert!(campaigns(2001, &["timing-belt"]).is_empty());
    assert!(campaigns(2001, &[]).is_empty());
}

#[test]
fn each_reader_needs_its_dataset() {
    let without_bulletins = shop(RECALLS_ENTRY);
    for error in [
        without_bulletins
            .bulletins_naming_code_count(7001, 2001, "P0001")
            .unwrap_err(),
        without_bulletins
            .bulletins_about_for_vehicle(7001, 2001, &list(&["engine"]), "P0001", 10)
            .unwrap_err(),
    ] {
        assert!(matches!(
            error,
            ShopError::DatasetUnavailable(Dataset::Bulletins)
        ));
    }
    let without_recalls = shop(BULLETINS_ENTRY);
    assert!(matches!(
        without_recalls
            .recalls_about_for_vehicle(7000, 7001, 2001, &list(&["engine"]))
            .unwrap_err(),
        ShopError::DatasetUnavailable(Dataset::RecallsUs)
    ));
}
