use rusqlite::Connection;
use wenmar_shopdata::schema::{FLUIDS, META, SCHEMA_VERSION, TIRES};
use wenmar_shopdata::sqlite::ShopSource;
use wenmar_shopdata::{Dataset, ShopData, ShopError};

const FLUIDS_ENTRY: &str =
    r#""fluids":{"as_of":"2026-10-12","rows":6,"licence":"mit","attribution":""}"#;
const TIRES_ENTRY: &str =
    r#""tires":{"as_of":"2026-10-12","rows":3,"licence":"mit","attribution":""}"#;

/// Records of a vehicle that does not exist: make 7000, model 7001, model
/// years 2001 and 2002, engines 70001 and 70002. Every figure is
/// impossible on purpose. The columns are those of `schema::FLUIDS` and
/// `schema::TIRES`, in order.
const ROWS: &str = r#"
INSERT INTO fluid VALUES
  (1, 7000, 7001, 2001, 'brake', NULL, NULL, NULL, 'TEST BRAKE 9', NULL, NULL, NULL, NULL, NULL, NULL, NULL, NULL, 'Test manual, page 5', '2026-10-10', 'test-reviewer'),
  (2, 7000, 7001, 2001, 'engine_oil', 70002, NULL, '0W-99', 'TEST-SPEC-2', 88.8, 93.8, 'l', 0, NULL, NULL, NULL, 'ca', 'Test manual, page 2', '2026-10-11', 'test-reviewer'),
  (3, 7000, 7001, 2001, 'engine_oil', 70001, NULL, '0W-99', 'TEST-SPEC-1', 99.9, 105.6, 'l', 1, NULL, NULL, NULL, NULL, 'Test manual, page 1', '2026-10-12', 'test-reviewer'),
  (4, 7000, 7001, 2001, 'refrigerant', NULL, NULL, NULL, 'R-TEST', NULL, NULL, NULL, NULL, 9990, 352.4, 'kg', NULL, 'Test under-hood label', '2026-10-12', 'test-reviewer'),
  (5, 7000, 7001, 2001, 'transmission', NULL, 'Test automatic', NULL, 'TEST ATF 9', NULL, NULL, NULL, NULL, NULL, NULL, NULL, NULL, 'Test manual, page 3', '2026-10-12', 'test-reviewer'),
  (6, 7000, 7001, 2002, 'coolant', NULL, NULL, NULL, 'TEST COOLANT 9', NULL, NULL, NULL, NULL, NULL, NULL, NULL, NULL, 'Test manual, page 4', '2026-10-12', 'test-reviewer');
INSERT INTO tire_fitment VALUES
  (1, 7000, 7001, 2001, '999/99R99', NULL, '["Base","Deluxe"]', 990, 980, 144, 142, 'kpa', 999, 737, 'nm', '9x999', NULL, 'Test door placard', '2026-10-12', 'test-reviewer'),
  (2, 7000, 7001, 2001, '998/98R98', '997/97R97', '[]', NULL, NULL, NULL, NULL, NULL, NULL, NULL, NULL, NULL, 'us', 'Test manual, page 6', '2026-10-11', 'test-reviewer'),
  (3, 7000, 7001, 2002, '999/99R99', NULL, '["Base"]', 990, 990, 144, 144, 'psi', NULL, NULL, NULL, NULL, NULL, 'Test door placard', '2026-10-12', 'test-reviewer');
"#;

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
        connection.execute_batch(FLUIDS).unwrap();
        connection.execute_batch(TIRES).unwrap();
        connection.execute_batch(ROWS).unwrap();
    }
    ShopData::new(ShopSource::from_connection(connection)).unwrap()
}

#[test]
fn a_model_year_gets_its_fluids_by_kind_then_engine() {
    let shop = shop(&format!("{FLUIDS_ENTRY},{TIRES_ENTRY}"), true);
    let fluids = shop.fluids_for_vehicle(7000, 7001, 2001).unwrap();
    let lines: Vec<String> = fluids
        .iter()
        .map(|fluid| {
            format!(
                "{} {:?} {:?} {:?}",
                fluid.kind, fluid.engine_id, fluid.specification, fluid.source
            )
        })
        .collect();
    assert_eq!(
        lines,
        [
            r#"engine_oil Some(70001) Some("TEST-SPEC-1") "Test manual, page 1""#,
            r#"engine_oil Some(70002) Some("TEST-SPEC-2") "Test manual, page 2""#,
            r#"transmission None Some("TEST ATF 9") "Test manual, page 3""#,
            r#"refrigerant None Some("R-TEST") "Test under-hood label""#,
            r#"brake None Some("TEST BRAKE 9") "Test manual, page 5""#,
        ]
    );
    let oil = &fluids[0];
    assert_eq!(oil.grade.as_deref(), Some("0W-99"));
    assert_eq!((oil.capacity_l, oil.capacity_qt), (Some(99.9), Some(105.6)));
    assert_eq!(oil.capacity_unit.as_deref(), Some("l"));
    assert_eq!(oil.with_filter, Some(true));
    assert_eq!(oil.market, None);
    assert_eq!(
        (oil.checked.as_str(), oil.reviewer.as_str()),
        ("2026-10-12", "test-reviewer")
    );
    assert_eq!(fluids[1].with_filter, Some(false));
    assert_eq!(fluids[1].market.as_deref(), Some("ca"));
    assert_eq!(fluids[2].applies_to.as_deref(), Some("Test automatic"));
    assert_eq!(
        (fluids[3].charge_g, fluids[3].charge_oz),
        (Some(9990), Some(352.4))
    );
    assert_eq!(fluids[3].charge_unit.as_deref(), Some("kg"));
    assert_eq!(fluids[4].with_filter, None);
    // Another year has only its own rows; a year with none has none.
    let next = shop.fluids_for_vehicle(7000, 7001, 2002).unwrap();
    assert_eq!(next.len(), 1);
    assert_eq!(next[0].kind, "coolant");
    assert!(
        shop.fluids_for_vehicle(7000, 7001, 2003)
            .unwrap()
            .is_empty()
    );
    assert!(shop.fluids_for_vehicle(1, 1, 2001).unwrap().is_empty());
}

#[test]
fn a_model_year_gets_every_fitment_in_the_order_of_the_file() {
    let shop = shop(&format!("{FLUIDS_ENTRY},{TIRES_ENTRY}"), true);
    let tires = shop.tires_for_vehicle(7000, 7001, 2001).unwrap();
    assert_eq!(tires.len(), 2);
    let first = &tires[0];
    assert_eq!(first.size, "999/99R99");
    assert_eq!(first.size_rear, None);
    assert_eq!(first.trims, ["Base", "Deluxe"]);
    assert_eq!(
        (
            first.pressure_front_kpa,
            first.pressure_rear_kpa,
            first.pressure_front_psi,
            first.pressure_rear_psi
        ),
        (Some(990), Some(980), Some(144), Some(142))
    );
    assert_eq!(first.pressure_unit.as_deref(), Some("kpa"));
    assert_eq!(
        (first.lug_torque_nm, first.lug_torque_lbft),
        (Some(999), Some(737))
    );
    assert_eq!(first.lug_torque_unit.as_deref(), Some("nm"));
    assert_eq!(first.bolt_pattern.as_deref(), Some("9x999"));
    assert_eq!(first.source, "Test door placard");
    let second = &tires[1];
    assert_eq!(second.size_rear.as_deref(), Some("997/97R97"));
    assert!(second.trims.is_empty());
    assert_eq!(second.pressure_front_kpa, None);
    assert_eq!(second.market.as_deref(), Some("us"));
    assert_eq!(shop.tires_for_vehicle(7000, 7001, 2002).unwrap().len(), 1);
    assert!(shop.tires_for_vehicle(7000, 7001, 2003).unwrap().is_empty());
}

#[test]
fn a_file_without_the_dataset_says_so() {
    let shop = shop(FLUIDS_ENTRY, true);
    assert!(shop.fluids_for_vehicle(7000, 7001, 2001).is_ok());
    assert!(matches!(
        shop.tires_for_vehicle(7000, 7001, 2001),
        Err(ShopError::DatasetUnavailable(Dataset::Tires))
    ));
    let shop = self::shop(TIRES_ENTRY, true);
    assert!(matches!(
        shop.fluids_for_vehicle(7000, 7001, 2001),
        Err(ShopError::DatasetUnavailable(Dataset::Fluids))
    ));
}
