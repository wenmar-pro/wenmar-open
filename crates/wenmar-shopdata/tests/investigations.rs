use rusqlite::Connection;
use serde_json::json;
use wenmar_shopdata::investigations::NOT_A_FINDING;
use wenmar_shopdata::schema::{INVESTIGATIONS, META, SCHEMA_VERSION};
use wenmar_shopdata::sqlite::ShopSource;
use wenmar_shopdata::{Dataset, ShopData, ShopError};

const HELD: &str = r#""investigations":{"as_of":"2026-10-12","rows":6,"licence":"public-domain","attribution":""}"#;
const OTHER: &str =
    r#""recalls_us":{"as_of":"2026-10-11","rows":7,"licence":"public-domain","attribution":""}"#;

/// Six investigations. The columns are those of `schema::INVESTIGATIONS`,
/// in order. The catalog ids are real: Ford 460 and F-150 1801, Ram 496
/// and 1500 13620. The texts are made up.
const ROWS: &str = "
INSERT INTO investigation VALUES
  (1, 'AQ18006', 'AQ', '2018-12-01', NULL, 1, 'Ford Motor Company', 'SEATS', 'Sale of noncompliant vehicles', 'A dealer sold them.'),
  (2, 'EA23001', 'EA', '2023-03-17', NULL, 1, 'Chrysler (FCA US, LLC)', 'FUEL SYSTEM, DIESEL:DELIVERY:FUEL PUMP', 'High pressure fuel pump failure', 'The pump may fail.'),
  (3, 'PE18008', 'PE', '2018-08-03', '2018-12-17', 0, 'Ford Motor Company', 'SEAT BELTS:PRETENSIONER', 'Seatbelt Pretensioner Fire', 'A fire may start.'),
  (4, 'PE25040', 'PE', '2025-06-01', NULL, 1, 'Ford Motor Company', 'STEERING; SUSPENSION', 'Loss of steering', NULL),
  (5, 'RQ98022', 'RQ', '1998-11-13', NULL, 0, 'Ford Motor Company', 'SUSPENSION:REAR:SPRINGS', 'REAR SUSPENSION SPRING BREAKAGE', NULL),
  (6, 'ID94001', 'ID', NULL, NULL, 0, NULL, NULL, NULL, NULL);
INSERT INTO investigation_vehicle VALUES
  (1, 2018, 460, 1801, 'FORD', 'F-150', 'exact'),
  (2, 2019, 496, 13620, 'RAM', '1500', 'exact'),
  (2, 2019, 496, NULL, 'RAM', '2500', NULL),
  (3, 2018, 460, 1801, 'FORD', 'F-150', 'exact'),
  (3, 2017, 460, 1801, 'FORD', 'F-150', 'exact'),
  (4, 2018, 460, 1801, 'FORD', 'F-150 LIGHTNING', 'base'),
  (4, 2018, 460, 1801, 'FORD', 'F-150', 'exact'),
  (5, 2018, 460, 1801, 'FORD', 'F-150', 'exact');
INSERT INTO investigation_campaign VALUES
  (2, '24V957000'),
  (3, '18V568000'),
  (4, '18V568000'),
  (6, '94E013000'),
  (6, '94E013001');
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
        connection.execute_batch(INVESTIGATIONS).unwrap();
        connection.execute_batch(ROWS).unwrap();
    }
    ShopData::new(ShopSource::from_connection(connection)).unwrap()
}

fn actions<T>(found: &[T], action: impl Fn(&T) -> &str) -> Vec<String> {
    found.iter().map(|one| action(one).to_owned()).collect()
}

#[test]
fn a_model_year_has_its_open_investigations_newest_first() {
    let shop = shop(HELD, true);
    assert_eq!(shop.investigations_as_of(), Some("2026-10-12"));
    let open = shop
        .investigations_for_vehicle(460, 1801, 2018, true)
        .unwrap();
    // The audit query is open and is not about the vehicle; the 1998 recall
    // query has no day closed and is not open; PE18008 is closed.
    assert_eq!(actions(&open, |one| &one.investigation.action), ["PE25040"]);
    // Two of its lines reach the model: it is given once, with the better
    // match and both names.
    assert_eq!(open[0].matched, "exact");
    assert_eq!(open[0].models, ["F-150", "F-150 LIGHTNING"]);
    assert_eq!(
        serde_json::to_value(&open[0]).unwrap(),
        json!({
            "action": "PE25040",
            "kind": "PE",
            "kind_name": "Preliminary evaluation",
            "opened": "2025-06-01",
            "closed": null,
            "open": true,
            "manufacturer": "Ford Motor Company",
            "component": "STEERING; SUSPENSION",
            "subject": "Loss of steering",
            "summary": null,
            "campaigns": ["18V568000"],
            "match": "exact",
            "models": ["F-150", "F-150 LIGHTNING"]
        })
    );

    let all = shop
        .investigations_for_vehicle(460, 1801, 2018, false)
        .unwrap();
    assert_eq!(
        actions(&all, |one| &one.investigation.action),
        ["PE25040", "AQ18006", "PE18008", "RQ98022"]
    );
    assert_eq!(
        all[1].investigation.kind_name.as_deref(),
        Some("Audit query")
    );
    assert!(!all[3].investigation.open);

    // Another model year, another model, and one with none.
    let ram = shop
        .investigations_for_vehicle(496, 13620, 2019, true)
        .unwrap();
    assert_eq!(actions(&ram, |one| &one.investigation.action), ["EA23001"]);
    assert_eq!(ram[0].investigation.campaigns, ["24V957000"]);
    assert!(
        shop.investigations_for_vehicle(460, 1801, 2017, true)
            .unwrap()
            .is_empty()
    );
    assert!(
        shop.investigations_for_vehicle(460, 1801, 2030, false)
            .unwrap()
            .is_empty()
    );
}

#[test]
fn a_campaign_has_the_investigations_that_led_to_it_oldest_first() {
    let shop = shop(HELD, true);
    let found = shop.investigations_for_campaign("18V568000").unwrap();
    assert_eq!(actions(&found, |one| &one.action), ["PE18008", "PE25040"]);
    assert_eq!(found[0].closed.as_deref(), Some("2018-12-17"));
    assert!(!found[0].open);
    assert_eq!(found[0].summary.as_deref(), Some("A fire may start."));
    // Letters in either case, and blanks around the number.
    let lower = shop.investigations_for_campaign(" 18v568000 ").unwrap();
    assert_eq!(lower, found);
    // An investigation of a kind with no name, with nothing but campaigns.
    let stub = shop.investigations_for_campaign("94E013001").unwrap();
    assert_eq!(stub.len(), 1);
    assert_eq!(stub[0].kind, "ID");
    assert_eq!(stub[0].kind_name, None);
    assert_eq!(stub[0].opened, None);
    assert_eq!(stub[0].campaigns, ["94E013000", "94E013001"]);
    assert!(
        shop.investigations_for_campaign("19V888000")
            .unwrap()
            .is_empty()
    );
    assert!(shop.investigations_for_campaign("").unwrap().is_empty());
}

#[test]
fn a_file_without_the_dataset_says_so_and_runs_no_sql() {
    // The tables are not there at all: a query would be an SQL error.
    let shop = shop(OTHER, false);
    assert_eq!(shop.investigations_as_of(), None);
    for error in [
        shop.investigations_for_vehicle(460, 1801, 2018, true)
            .unwrap_err(),
        shop.investigations_for_campaign("18V568000").unwrap_err(),
    ] {
        assert!(
            matches!(
                error,
                ShopError::DatasetUnavailable(Dataset::Investigations)
            ),
            "{error}"
        );
        assert_eq!(
            error.to_string(),
            "The investigations dataset is not available."
        );
    }
    assert!(NOT_A_FINDING.contains("not a finding"));
}
