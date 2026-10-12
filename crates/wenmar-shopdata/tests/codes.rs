use rusqlite::Connection;
use wenmar_shopdata::codes::{CodeQuery, is_code, is_generic, system_fits};
use wenmar_shopdata::schema::{CODES, COMPONENTS, META, SCHEMA_VERSION};
use wenmar_shopdata::sqlite::ShopSource;
use wenmar_shopdata::{Dataset, ShopData, ShopError};

/// Four records. The numbers are real generic numbers; every word of text
/// is made up and describes no code.
const ROWS: &str = r#"
INSERT INTO component VALUES
  ('engine', 'Engine', NULL),
  ('timing-belt', 'Timing belt or chain', 'engine'),
  ('electrical', 'Electrical system', NULL);
INSERT INTO code VALUES
  ('P0001', 'powertrain/fuel-air', 'Made-up title one', 'Made-up meaning one.', '["Made-up cause A","Made-up cause B"]', '["Made-up check A"]', '["P0002","U0001"]', 'test-author', 'test-reviewer', '2026-10-10'),
  ('P0002', 'powertrain/emissions', 'Made-up title two', 'Made-up meaning two, with the word zebra.', '["Made-up cause C"]', '["Made-up check B","Made-up check C"]', '["P0001"]', 'test-author', 'test-reviewer', '2026-10-12'),
  ('P2000', 'powertrain', 'Made-up title three', 'Made-up meaning three.', '["Made-up cause D"]', '["Made-up check D"]', '[]', 'test-author', 'test-reviewer', '2026-10-11'),
  ('U0001', 'network', 'Made-up title four', 'Made-up meaning four.', '["Made-up cause E"]', '["Made-up check E"]', '[]', 'test-author', 'test-reviewer', '2026-10-11');
INSERT INTO code_component VALUES
  ('P0001', 'timing-belt', 0), ('P0001', 'engine', 1), ('U0001', 'electrical', 0);
"#;

fn shop(held: bool) -> ShopData<ShopSource> {
    let connection = Connection::open_in_memory().unwrap();
    connection.execute_batch(META).unwrap();
    let datasets = if held {
        r#"{"codes":{"as_of":"2026-10-12","rows":4,"licence":"mit","attribution":""}}"#
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
        connection.execute_batch(CODES).unwrap();
        connection.execute_batch(ROWS).unwrap();
    }
    ShopData::new(ShopSource::from_connection(connection)).unwrap()
}

fn listed(shop: &ShopData<ShopSource>, query: &CodeQuery) -> Vec<String> {
    shop.codes(query)
        .unwrap()
        .codes
        .into_iter()
        .map(|code| code.code)
        .collect()
}

#[test]
fn only_the_generic_ranges_are_generic() {
    for code in [
        "P0000", "P0FFF", "P2A00", "P3400", "P3FFF", "B0001", "C0001", "U0001", "U3000",
    ] {
        assert!(is_code(code) && is_generic(code), "{code}");
    }
    for code in [
        "P1000", "P3000", "P33FF", "B1000", "B2000", "B3000", "C1000", "C2000", "C3000", "U1000",
        "U2000",
    ] {
        assert!(is_code(code) && !is_generic(code), "{code}");
    }
    for text in ["p0001", "P001", "P00011", "X0001", "P4000", "P000G", ""] {
        assert!(!is_code(text) && !is_generic(text), "{text}");
    }
    assert!(system_fits("P0001", "powertrain"));
    assert!(system_fits("P0001", "powertrain/emissions"));
    assert!(system_fits("U0001", "network"));
    assert!(!system_fits("U0001", "powertrain"));
    assert!(!system_fits("P0001", "powertrain/made-up"));
    assert!(!system_fits("B0001", "chassis"));
}

#[test]
fn a_code_is_read_with_its_components_and_related_codes() {
    let shop = shop(true);
    assert_eq!(shop.codes_as_of(), Some("2026-10-12"));
    let code = shop.code("P0001").unwrap().unwrap();
    assert_eq!(code.system, "powertrain/fuel-air");
    assert_eq!(code.title, "Made-up title one");
    assert_eq!(code.meaning, "Made-up meaning one.");
    assert_eq!(code.common_causes, ["Made-up cause A", "Made-up cause B"]);
    assert_eq!(code.checks, ["Made-up check A"]);
    // In the order of the record, not of the alphabet.
    let components: Vec<(&str, Option<&str>)> = code
        .components
        .iter()
        .map(|component| (component.id.as_str(), component.parent.as_deref()))
        .collect();
    assert_eq!(
        components,
        [("timing-belt", Some("engine")), ("engine", None)]
    );
    assert_eq!(code.components[0].name, "Timing belt or chain");
    let related: Vec<(&str, &str)> = code
        .related
        .iter()
        .map(|other| (other.code.as_str(), other.title.as_str()))
        .collect();
    assert_eq!(
        related,
        [
            ("P0002", "Made-up title two"),
            ("U0001", "Made-up title four")
        ]
    );
    assert_eq!(
        (
            code.author.as_str(),
            code.reviewer.as_str(),
            code.checked.as_str()
        ),
        ("test-author", "test-reviewer", "2026-10-10")
    );
    assert!(shop.code("P0003").unwrap().is_none());
    assert!(shop.code("P1000").unwrap().is_none());
    assert_eq!(shop.code_components("U0001").unwrap()[0].id, "electrical");
    assert!(shop.code_components("P2000").unwrap().is_empty());
}

#[test]
fn a_list_is_by_code_and_narrowed_by_system_and_text() {
    let shop = shop(true);
    assert_eq!(
        listed(&shop, &CodeQuery::default()),
        ["P0001", "P0002", "P2000", "U0001"]
    );
    let all: Vec<String> = shop
        .all_codes()
        .unwrap()
        .into_iter()
        .map(|code| code.code)
        .collect();
    assert_eq!(all, ["P0001", "P0002", "P2000", "U0001"]);
    let system = |system: &str| CodeQuery {
        system: Some(system.to_owned()),
        ..CodeQuery::default()
    };
    // A group holds its areas; an area holds itself.
    assert_eq!(
        listed(&shop, &system("powertrain")),
        ["P0001", "P0002", "P2000"]
    );
    assert_eq!(listed(&shop, &system("powertrain/emissions")), ["P0002"]);
    assert_eq!(listed(&shop, &system("network")), ["U0001"]);
    assert!(listed(&shop, &system("body")).is_empty());
    let text = |q: &str| CodeQuery {
        q: Some(q.to_owned()),
        ..CodeQuery::default()
    };
    assert_eq!(listed(&shop, &text("ZEBRA")), ["P0002"]);
    assert_eq!(listed(&shop, &text("title three")), ["P2000"]);
    assert_eq!(listed(&shop, &text("p00")), ["P0001", "P0002"]);
    assert!(listed(&shop, &text("100%")).is_empty());
    // Pages.
    let first = shop
        .codes(&CodeQuery {
            limit: 3,
            ..CodeQuery::default()
        })
        .unwrap();
    assert_eq!(first.codes.len(), 3);
    assert_eq!(first.next.as_deref(), Some("P2000"));
    let second = shop
        .codes(&CodeQuery {
            limit: 3,
            after: first.next,
            ..CodeQuery::default()
        })
        .unwrap();
    assert_eq!(second.codes[0].code, "U0001");
    assert_eq!(second.next, None);
}

#[test]
fn a_file_without_the_dataset_says_so() {
    let shop = shop(false);
    assert_eq!(shop.codes_as_of(), None);
    for error in [
        shop.code("P0001").unwrap_err(),
        shop.codes(&CodeQuery::default()).unwrap_err(),
        shop.all_codes().unwrap_err(),
        shop.code_components("P0001").unwrap_err(),
    ] {
        assert!(
            matches!(error, ShopError::DatasetUnavailable(Dataset::Codes)),
            "{error}"
        );
        assert_eq!(error.to_string(), "The codes dataset is not available.");
    }
}
