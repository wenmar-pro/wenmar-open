use rusqlite::Connection;
use serde_json::json;
use wenmar_shopdata::bulletins::{
    BulletinCursor, BulletinQuery, BulletinType, FIRST_DOCUMENT_ID, MOST_BULLETINS, code_form,
    document_urls,
};
use wenmar_shopdata::schema::{
    BULLETIN_COMPONENTS, BULLETIN_TEXT, BULLETINS, COMPONENTS, META, SCHEMA_VERSION,
};
use wenmar_shopdata::sqlite::ShopSource;
use wenmar_shopdata::{Dataset, ShopData, ShopError};

const HELD: &str =
    r#""bulletins":{"as_of":"2026-10-11","rows":10,"licence":"public-domain","attribution":""}"#;

/// Ten bulletins, shaped as `open-data build-shop` stores them. The text
/// is made up. The catalog ids are those of the server's test catalog:
/// Hyundai 498 and Kona 900, Chevrolet 467 and Silverado 1850. The Kona of
/// model year 2023 has six, two of them published on the same day twice
/// over; bulletin 11000002 reaches it through two names.
const ROWS: &str = r#"
INSERT INTO bulletin VALUES
  (11000001, 'service_bulletin', '2025-03-10', '2025-03-01', 'TSB 25-01-012', NULL, NULL, 'ENGINE AND ENGINE COOLING:ENGINE; ELECTRICAL SYSTEM:SOFTWARE', NULL, 'Some 2022-2023 Kona vehicles may set DTC P0420 and P0300 with a rough idle. Update the ECM software.'),
  (11000002, 'warranty_extension', '2025-06-02', '2025-05-20', 'WX-25-003', 'WX003', NULL, 'POWER TRAIN:AUTOMATIC TRANSMISSION', NULL, 'Warranty extension: the transmission control module is covered for 10 years.'),
  (11000003, 'over_the_air', '2026-01-15', '2026-01-10', 'OTA 26.1', '26.1.4', NULL, 'ELECTRICAL SYSTEM:SOFTWARE', 'Infotainment: Head Unit', 'Infotainment update: improved navigation and voice control.'),
  (11000004, 'service_bulletin', '2024-02-01', '2024-01-20', 'TSB 24-BR-001', NULL, NULL, 'SERVICE BRAKES, HYDRAULIC:FOUNDATION COMPONENTS:DISC:PADS', NULL, 'Brake squeal noise at low speed. Replace the front brake pads.'),
  (11000005, 'service_bulletin', '2024-02-01', NULL, NULL, NULL, NULL, 'UNKNOWN OR OTHER', NULL, 'Maintenance schedule update: the oil change service interval is now 8,000 miles.'),
  (11000006, 'emissions', '2023-07-07', '2023-07-01', 'EM-23', NULL, NULL, 'ENGINE AND ENGINE COOLING:EXHAUST SYSTEM:EMISSION CONTROL:CATALYTIC CONVERTOR', NULL, 'Emissions recall: catalytic converter. DTC P0420 may set.'),
  (11000007, 'service_campaign', '2026-04-23', '2026-04-23', 'N262548050', 'N262548050', NULL, 'ENGINE', NULL, 'Certain vehicles may have a condition that could cause the catalytic converter to fail.'),
  (10100001, 'service_bulletin', '2017-03-01', '2017-02-20', 'sb2652', NULL, '95-011', 'EQUIPMENT', NULL, 'DT12 PTO Upfit'),
  (11000009, 'other', '2022-09-22', '2022-09-16', 'A2090B', NULL, NULL, 'VISIBILITY', NULL, 'Dealer message: washer pumps were 100% tested.'),
  (11000010, 'service_bulletin', '2025-03-10', '2025-03-02', '"><img src=x onerror=alert(1)>', NULL, NULL, '<b>Bold</b> SYSTEM:PART', NULL, 'A part named <script>alert(1)</script> may fail.');
INSERT INTO bulletin_model VALUES
  (1, 'HYUNDAI', 'KONA', 498, 900, 'exact'),
  (2, 'HYUNDAI', 'KONA ELECTRIC', 498, 900, 'base'),
  (3, 'HYUNDAI', 'KONA', 498, NULL, NULL),
  (4, 'CHEVROLET', 'SILVERADO 1500', 467, 1850, 'base'),
  (5, 'GMC', 'SIERRA 1500', NULL, NULL, NULL),
  (6, 'FREIGHTLINER', 'CASCADIA', NULL, NULL, NULL),
  (7, 'HYUNDAI', 'HYUNDAI', 498, NULL, NULL);
INSERT INTO bulletin_vehicle VALUES
  (11000001, 1, 2022, 2023),
  (11000002, 1, 2023, 2023),
  (11000002, 2, 2023, 2023),
  (11000003, 1, 2023, 2023),
  (11000003, 3, 2024, 2025),
  (11000004, 1, 2023, 2023),
  (11000005, 1, 2023, 2023),
  (11000006, 1, 2022, 2022),
  (11000006, 4, 2019, 2019),
  (11000007, 4, 2019, 2020),
  (11000007, 5, 2019, 2020),
  (10100001, 6, 2020, 2020),
  (11000009, 7, 0, 0),
  (11000010, 1, 2023, 2023);
INSERT INTO component VALUES
  ('brakes', 'Brakes', NULL),
  ('brake-pad', 'Brake pads', 'brakes'),
  ('electrical', 'Electrical system', NULL),
  ('software', 'Software', 'electrical'),
  ('engine', 'Engine', NULL),
  ('exhaust-emissions', 'Exhaust and emissions', NULL),
  ('catalytic-converter', 'Catalytic converter', 'exhaust-emissions'),
  ('powertrain', 'Transmission and driveline', NULL),
  ('automatic-transmission', 'Automatic transmission', 'powertrain'),
  ('visibility', 'Glass, wipers and mirrors', NULL),
  ('equipment', 'Equipment and accessories', NULL);
INSERT INTO bulletin_component VALUES
  (11000001, 'engine'), (11000001, 'electrical'), (11000001, 'software'),
  (11000002, 'powertrain'), (11000002, 'automatic-transmission'),
  (11000003, 'electrical'), (11000003, 'software'),
  (11000004, 'brakes'), (11000004, 'brake-pad'),
  (11000006, 'engine'), (11000006, 'exhaust-emissions'), (11000006, 'catalytic-converter'),
  (11000007, 'engine'),
  (10100001, 'equipment'),
  (11000009, 'visibility');
INSERT INTO bulletin_code VALUES
  (11000001, 'P0300'), (11000001, 'P0420'), (11000006, 'P0420');
INSERT INTO bulletin_topic VALUES
  (11000005, 'maintenance');
INSERT INTO bulletin_year (year)
  WITH RECURSIVE years(year) AS (SELECT 1900 UNION ALL SELECT year + 1 FROM years WHERE year < 2100)
  SELECT year FROM years;
"#;

/// A shop file in memory holding `datasets` (the inside of the JSON
/// object) and, when `with_rows`, the bulletin tables and [`ROWS`].
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
        for ddl in [BULLETINS, COMPONENTS, BULLETIN_COMPONENTS, BULLETIN_TEXT] {
            connection.execute_batch(ddl).unwrap();
        }
        connection.execute_batch(ROWS).unwrap();
    }
    ShopData::new(ShopSource::from_connection(connection)).unwrap()
}

fn held() -> ShopData<ShopSource> {
    shop(HELD, true)
}

fn query() -> BulletinQuery {
    BulletinQuery {
        limit: MOST_BULLETINS,
        ..BulletinQuery::default()
    }
}

/// The Kona of model year 2023.
fn kona() -> BulletinQuery {
    BulletinQuery {
        make_id: Some(498),
        model_id: Some(900),
        year: Some(2023),
        ..query()
    }
}

/// The ids of a list, in order.
fn listed(shop: &ShopData<ShopSource>, query: BulletinQuery) -> Vec<i64> {
    shop.bulletins(&query)
        .unwrap()
        .bulletins
        .into_iter()
        .map(|bulletin| bulletin.nhtsa_id)
        .collect()
}

#[test]
fn a_model_year_gets_its_bulletins_newest_first_each_once() {
    let shop = held();
    assert_eq!(shop.bulletins_as_of(), Some("2026-10-11"));
    assert_eq!(
        listed(&shop, kona()),
        // Two published on 2025-03-10 and two on 2024-02-01: the higher id
        // first. 11000002 reaches the Kona through two names and is
        // listed once.
        [11000003, 11000002, 11000010, 11000001, 11000005, 11000004]
    );
    // Another model year of the same model.
    assert_eq!(
        listed(
            &shop,
            BulletinQuery {
                year: Some(2022),
                ..kona()
            }
        ),
        [11000001, 11000006]
    );
    // 2024 is listed for the Kona by name, but matched no catalog model.
    assert!(
        listed(
            &shop,
            BulletinQuery {
                year: Some(2024),
                ..kona()
            }
        )
        .is_empty()
    );
    let page = shop.bulletins(&kona()).unwrap();
    assert_eq!(
        serde_json::to_value(&page.bulletins[1]).unwrap(),
        json!({
            "nhtsa_id": 11000002,
            "type": "warranty_extension",
            "date_added": "2025-06-02",
            "date": "2025-05-20",
            "document_id": "WX-25-003",
            "mfr_campaign": "WX003",
            "component": "POWER TRAIN:AUTOMATIC TRANSMISSION",
            "summary": "Warranty extension: the transmission control module is covered for 10 years."
        })
    );
    assert_eq!(page.next_cursor, None);
}

#[test]
fn a_list_pages_without_gaps_or_repeats() {
    let shop = held();
    let mut seen = Vec::new();
    let mut cursor = None;
    let mut pages = 0;
    loop {
        let page = shop
            .bulletins(&BulletinQuery {
                limit: 4,
                cursor: cursor.clone(),
                ..query()
            })
            .unwrap();
        pages += 1;
        seen.extend(page.bulletins.iter().map(|bulletin| bulletin.nhtsa_id));
        match page.next_cursor {
            Some(next) => {
                // A cursor survives being written out and read back.
                cursor = Some(BulletinCursor::parse(&next.to_string()).unwrap());
            }
            None => break,
        }
    }
    assert_eq!(pages, 3);
    assert_eq!(
        seen,
        [
            11000007, 11000003, 11000002, 11000010, 11000001, 11000005, 11000004, 11000006,
            11000009, 10100001
        ]
    );
    // A page that ends exactly at the end has no cursor.
    let page = shop
        .bulletins(&BulletinQuery {
            limit: 10,
            ..query()
        })
        .unwrap();
    assert_eq!(page.bulletins.len(), 10);
    assert_eq!(page.next_cursor, None);
    // The limit is held between 1 and MOST_BULLETINS.
    let page = shop
        .bulletins(&BulletinQuery {
            limit: 0,
            ..query()
        })
        .unwrap();
    assert_eq!(page.bulletins.len(), 1);
    assert_eq!(page.next_cursor.unwrap().to_string(), "2026-04-23_11000007");
    for bad in [
        "",
        "11000007",
        "2026-04-23",
        "2026-04-23_",
        "_11000007",
        "2026-04-23_x",
        "2026/04/23_1",
        "' OR 1=1 --_1",
        "2026-04-23_9999999999999999999",
    ] {
        assert_eq!(BulletinCursor::parse(bad), None, "{bad}");
    }
}

#[test]
fn a_list_is_narrowed_by_what_is_asked() {
    let shop = held();
    let narrowed = |change: fn(&mut BulletinQuery)| {
        let mut asked = query();
        change(&mut asked);
        listed(&shop, asked)
    };
    assert_eq!(
        narrowed(|asked| asked.kind = Some(BulletinType::WarrantyExtension)),
        [11000002]
    );
    assert_eq!(
        narrowed(|asked| asked.component = Some("engine".to_owned())),
        [11000007, 11000001, 11000006]
    );
    // A part, and the system above it.
    assert_eq!(
        narrowed(|asked| asked.component = Some("brake-pad".to_owned())),
        [11000004]
    );
    assert_eq!(
        narrowed(|asked| asked.component = Some("brakes".to_owned())),
        [11000004]
    );
    assert_eq!(
        narrowed(|asked| asked.code = Some("P0420".to_owned())),
        [11000001, 11000006]
    );
    assert_eq!(
        narrowed(|asked| asked.topic = Some("maintenance".to_owned())),
        [11000005]
    );
    // By make alone, by model alone, by year alone.
    assert_eq!(
        narrowed(|asked| asked.make_id = Some(467)),
        [11000007, 11000006]
    );
    assert_eq!(
        narrowed(|asked| asked.model_id = Some(1850)),
        [11000007, 11000006]
    );
    assert_eq!(
        narrowed(|asked| asked.year = Some(2020)),
        [11000007, 10100001]
    );
    // Narrowings combine.
    assert_eq!(
        narrowed(|asked| {
            asked.make_id = Some(498);
            asked.model_id = Some(900);
            asked.year = Some(2023);
            asked.component = Some("software".to_owned());
            asked.kind = Some(BulletinType::ServiceBulletin);
        }),
        [11000001]
    );
    // Something nothing has.
    assert!(narrowed(|asked| asked.component = Some("no-such-part".to_owned())).is_empty());
    assert!(narrowed(|asked| asked.code = Some("P9999".to_owned())).is_empty());
    assert!(narrowed(|asked| asked.model_id = Some(1)).is_empty());
}

#[test]
fn free_text_is_found_anywhere_in_a_bulletin() {
    let shop = held();
    let found = |text: &str| {
        listed(
            &shop,
            BulletinQuery {
                q: Some(text.to_owned()),
                ..query()
            },
        )
    };
    // In the summary, in either case.
    assert_eq!(found("catalytic converter"), [11000007, 11000006]);
    assert_eq!(found("NOISE"), [11000004]);
    // In the document number, the campaign id, the component and the
    // manufacturer's own words.
    assert_eq!(found("tsb 24-br"), [11000004]);
    assert_eq!(found("WX003"), [11000002]);
    assert_eq!(found("disc:pads"), [11000004]);
    assert_eq!(found("head unit"), [11000003]);
    // `%` and `_` mean themselves.
    assert_eq!(found("100%"), [11000009]);
    assert!(found("1_0%").is_empty());
    // Blank text narrows nothing.
    assert_eq!(found("  ").len(), 10);
    // With a model year.
    assert_eq!(
        listed(
            &shop,
            BulletinQuery {
                q: Some("software".to_owned()),
                ..kona()
            }
        ),
        [11000003, 11000001]
    );
}

#[test]
fn one_bulletin_comes_with_everything_about_it() {
    let shop = held();
    let detail = shop.bulletin(11000006).unwrap().unwrap();
    assert_eq!(
        serde_json::to_value(&detail).unwrap(),
        json!({
            "nhtsa_id": 11000006,
            "type": "emissions",
            "date_added": "2023-07-07",
            "date": "2023-07-01",
            "document_id": "EM-23",
            "mfr_campaign": null,
            "component": "ENGINE AND ENGINE COOLING:EXHAUST SYSTEM:EMISSION CONTROL:CATALYTIC CONVERTOR",
            "summary": "Emissions recall: catalytic converter. DTC P0420 may set.",
            "replaces": null,
            "mfr_system": null,
            "components": [
                { "id": "engine", "name": "Engine", "parent": null },
                { "id": "exhaust-emissions", "name": "Exhaust and emissions", "parent": null },
                { "id": "catalytic-converter", "name": "Catalytic converter", "parent": "exhaust-emissions" }
            ],
            "codes": ["P0420"],
            "topics": [],
            "vehicles": [
                { "make_text": "CHEVROLET", "model_text": "SILVERADO 1500", "year_from": 2019, "year_to": 2019, "make_id": 467, "model_id": 1850, "match": "base" },
                { "make_text": "HYUNDAI", "model_text": "KONA", "year_from": 2022, "year_to": 2022, "make_id": 498, "model_id": 900, "match": "exact" }
            ]
        })
    );
    // No model year given: both years are null.
    let dealer = shop.bulletin(11000009).unwrap().unwrap();
    assert_eq!(dealer.vehicles.len(), 1);
    assert_eq!(dealer.vehicles[0].year_from, None);
    assert_eq!(dealer.vehicles[0].year_to, None);
    assert_eq!(dealer.vehicles[0].make_id, Some(498));
    assert_eq!(dealer.vehicles[0].model_id, None);
    // A name that matched in some years and not in others is two lines.
    let update = shop.bulletin(11000003).unwrap().unwrap();
    let lines: Vec<(Option<u16>, Option<u16>, Option<i64>)> = update
        .vehicles
        .iter()
        .map(|vehicle| (vehicle.year_from, vehicle.year_to, vehicle.model_id))
        .collect();
    assert_eq!(
        lines,
        [
            (Some(2023), Some(2023), Some(900)),
            (Some(2024), Some(2025), None)
        ]
    );
    assert_eq!(
        update.mfr_system.as_deref(),
        Some("Infotainment: Head Unit")
    );
    assert_eq!(
        shop.bulletin(10100001)
            .unwrap()
            .unwrap()
            .replaces
            .as_deref(),
        Some("95-011")
    );
    assert_eq!(
        shop.bulletin(11000005).unwrap().unwrap().topics,
        ["maintenance"]
    );
    assert!(shop.bulletin(1).unwrap().is_none());
    assert!(shop.bulletin(-5).unwrap().is_none());
}

#[test]
fn a_model_year_is_counted_by_type_component_code_and_topic() {
    let shop = held();
    let counts = shop.bulletin_counts(498, 900, 2023).unwrap();
    assert_eq!(
        serde_json::to_value(&counts).unwrap(),
        json!({
            "count": 6,
            "by_type": [
                { "id": "service_bulletin", "name": "Service bulletin", "count": 4 },
                { "id": "warranty_extension", "name": "Warranty extension", "count": 1 },
                { "id": "over_the_air", "name": "Over-the-air update", "count": 1 }
            ],
            "components": [
                { "id": "electrical", "name": "Electrical system", "count": 2 },
                { "id": "software", "name": "Software", "count": 2 },
                { "id": "automatic-transmission", "name": "Automatic transmission", "count": 1 },
                { "id": "brake-pad", "name": "Brake pads", "count": 1 },
                { "id": "brakes", "name": "Brakes", "count": 1 },
                { "id": "engine", "name": "Engine", "count": 1 },
                { "id": "powertrain", "name": "Transmission and driveline", "count": 1 }
            ],
            "codes": [
                { "id": "P0300", "name": "P0300", "count": 1 },
                { "id": "P0420", "name": "P0420", "count": 1 }
            ],
            "topics": [{ "id": "maintenance", "name": "maintenance", "count": 1 }]
        })
    );
    // A model year with none is all zeros, not an error.
    let none = shop.bulletin_counts(498, 900, 1999).unwrap();
    assert_eq!(none.count, 0);
    assert!(none.by_type.is_empty() && none.components.is_empty());
}

#[test]
fn a_document_address_follows_the_rule_or_is_not_given() {
    assert_eq!(
        document_urls(11031644, Some("2026-04-14"), "2026-04-28"),
        Some((
            "https://static.nhtsa.gov/odi/tsbs/2026/MC-11031644-0001.pdf".to_owned(),
            "https://static.nhtsa.gov/odi/tsbs/2026/MC-11031644-9999.pdf".to_owned()
        ))
    );
    // Published up to 2018: the address ending 9999 first.
    assert_eq!(
        document_urls(10126708, Some("2017-10-02"), "2018-01-05"),
        Some((
            "https://static.nhtsa.gov/odi/tsbs/2017/MC-10126708-9999.pdf".to_owned(),
            "https://static.nhtsa.gov/odi/tsbs/2017/MC-10126708-0001.pdf".to_owned()
        ))
    );
    // The year is the manufacturer's, not the year NHTSA added it.
    assert!(
        document_urls(10205713, Some("2013-11-13"), "2021-12-27")
            .unwrap()
            .0
            .contains("/2013/MC-10205713-0001")
    );
    assert!(document_urls(FIRST_DOCUMENT_ID, Some("2017-05-27"), "2017-07-31").is_some());
    // No date, an id from before the rule held, a date that is not one.
    assert_eq!(document_urls(11031644, None, "2026-04-28"), None);
    assert_eq!(
        document_urls(FIRST_DOCUMENT_ID - 1, Some("2017-05-27"), "2017-07-31"),
        None
    );
    assert_eq!(document_urls(11031644, Some("n/a"), "2026-04-28"), None);
    assert_eq!(document_urls(11031644, Some("20"), "2026-04-28"), None);
}

#[test]
fn a_code_is_read_in_the_form_it_is_stored() {
    assert_eq!(code_form("P0420").as_deref(), Some("P0420"));
    assert_eq!(code_form(" p0420 ").as_deref(), Some("P0420"));
    assert_eq!(code_form("u0100:87").as_deref(), Some("U0100"));
    assert_eq!(code_form("P029900").as_deref(), Some("P0299"));
    assert_eq!(code_form("b1a2f").as_deref(), Some("B1A2F"));
    for bad in ["", "P042", "X0420", "P4420", "P0G20", "0420P", "é0420"] {
        assert_eq!(code_form(bad), None, "{bad}");
    }
}

#[test]
fn a_file_with_no_bulletins_names_the_dataset_and_runs_no_sql() {
    // The file has no bulletin tables at all: a query would fail.
    let shop = shop("", false);
    assert_eq!(shop.bulletins_as_of(), None);
    let is_unavailable = |error: ShopError| {
        assert!(
            matches!(error, ShopError::DatasetUnavailable(Dataset::Bulletins)),
            "{error}"
        );
        assert_eq!(error.to_string(), "The bulletins dataset is not available.");
    };
    is_unavailable(shop.bulletins(&query()).unwrap_err());
    is_unavailable(shop.bulletin(11000001).unwrap_err());
    is_unavailable(shop.bulletin_counts(498, 900, 2023).unwrap_err());
}

/// Run by hand against a real shop file:
/// `SHOP_FILE=data/build/shop/wenmar-open-shop-2026.10.12.sqlite3 cargo test --release -p wenmar-shopdata --features sqlite --test bulletins the_real_file_answers_quickly -- --ignored --nocapture`
#[test]
#[ignore = "needs a real shop file named by SHOP_FILE"]
fn the_real_file_answers_quickly() {
    let path = std::env::var("SHOP_FILE").expect("set SHOP_FILE to a shop file with bulletins");
    let shop = ShopData::new(ShopSource::open(&path).unwrap()).unwrap();
    let timed = |name: &str, asked: BulletinQuery| {
        let start = std::time::Instant::now();
        let page = shop.bulletins(&asked).unwrap();
        println!(
            "{name}: {} bulletins in {:?}",
            page.bulletins.len(),
            start.elapsed()
        );
    };
    // Ford 460, F-150 1801 in the catalog of 2026.09.
    let f150 = || BulletinQuery {
        make_id: Some(460),
        model_id: Some(1801),
        year: Some(2018),
        limit: 25,
        ..BulletinQuery::default()
    };
    let all = || BulletinQuery {
        limit: 25,
        ..BulletinQuery::default()
    };
    let text = |text: &str, base: BulletinQuery| BulletinQuery {
        q: Some(text.to_owned()),
        ..base
    };
    timed("newest of everything", all());
    timed("2018 F-150", f150());
    timed("2018 F-150, text noise", text("noise", f150()));
    timed("everything, text noise", text("noise", all()));
    timed("everything, text water pump", text("water pump", all()));
    timed("everything, text found nowhere", text("zzzznothing", all()));
    timed(
        "everything, code P0420",
        BulletinQuery {
            code: Some("P0420".to_owned()),
            ..all()
        },
    );
    timed(
        "everything, component brake-pad",
        BulletinQuery {
            component: Some("brake-pad".to_owned()),
            ..all()
        },
    );
    timed(
        "everything, topic maintenance",
        BulletinQuery {
            topic: Some("maintenance".to_owned()),
            ..all()
        },
    );
    timed(
        "make Ford",
        BulletinQuery {
            make_id: Some(460),
            ..all()
        },
    );
    timed(
        "model year 2018, any make",
        BulletinQuery {
            year: Some(2018),
            ..all()
        },
    );
    let start = std::time::Instant::now();
    let counts = shop.bulletin_counts(460, 1801, 2018).unwrap();
    println!(
        "counts of the 2018 F-150: {} in {:?}",
        counts.count,
        start.elapsed()
    );
    let start = std::time::Instant::now();
    let detail = shop.bulletin(10152456).unwrap().unwrap();
    println!(
        "one bulletin: {} vehicle lines in {:?}",
        detail.vehicles.len(),
        start.elapsed()
    );
    assert!(counts.count > 400);
}
