use rusqlite::Connection;
use wenmar_shopdata::schema::{META, SCHEMA_VERSION, VIN_GUIDES};
use wenmar_shopdata::sqlite::ShopSource;
use wenmar_shopdata::vin_guides::{MOST_VIN_GUIDES, VinGuide};
use wenmar_shopdata::{Dataset, ShopData, ShopError};

/// Five filings by three manufacturers. Manufacturer 976 is Ford Motor
/// Company, whose code `1FM` builds Fords (make 460) and whose code `4F2`
/// built Mazdas (473); 987 is Honda Motor Co., Ltd. (`JHM`, make 474);
/// 958 filed and reaches nothing. The document ids are made up.
const ROWS: &str = "
INSERT INTO vin_guide VALUES
  (1, '01-22-N11B-1670', 976, 'FORD MOTOR COMPANY', '565', '1984-10-09', NULL, NULL, 'https://vpic.nhtsa.dot.gov/mid/home/displayfile/00000000-0000-4000-8000-000000000001'),
  (2, 'ORG21569', 958, 'JAGUAR LAND ROVER NA, LLC', '565', '2023-04-03', 2024, 2024, 'https://vpic.nhtsa.dot.gov/mid/home/displayfile/00000000-0000-4000-8000-000000000002'),
  (3, 'ORG26392', 976, 'FORD MOTOR COMPANY', '565', '2026-03-26', 2027, 2027, 'https://vpic.nhtsa.dot.gov/mid/home/displayfile/00000000-0000-4000-8000-000000000003'),
  (4, 'ORG26393', 976, 'FORD MOTOR COMPANY', '565', '2026-03-26', 2026, NULL, 'https://vpic.nhtsa.dot.gov/mid/home/displayfile/00000000-0000-4000-8000-000000000004'),
  (5, 'ORG27020', 987, 'HONDA MOTOR CO., LTD.', '565', '2026-10-06', NULL, 2027, 'https://vpic.nhtsa.dot.gov/mid/home/displayfile/00000000-0000-4000-8000-000000000005');
INSERT INTO vin_guide_wmi VALUES ('1FM', 976), ('4F2', 976), ('JHM', 987);
INSERT INTO vin_guide_make VALUES (460, 976), (473, 976), (474, 987);
";

/// A shop file in memory that holds the list, or with `held` false a shop
/// file that holds no dataset.
fn shop(held: bool) -> ShopData<ShopSource> {
    let connection = Connection::open_in_memory().unwrap();
    connection.execute_batch(META).unwrap();
    let datasets = if held {
        r#"{"vin_guides":{"as_of":"2026-10-12","rows":5,"licence":"public-domain","attribution":""}}"#
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
        connection.execute_batch(VIN_GUIDES).unwrap();
        connection.execute_batch(ROWS).unwrap();
    }
    ShopData::new(ShopSource::from_connection(connection)).unwrap()
}

fn names(guides: &[VinGuide]) -> Vec<&str> {
    guides.iter().map(|guide| guide.name.as_str()).collect()
}

#[test]
fn a_makes_filings_are_its_manufacturers_newest_first() {
    let shop = shop(true);
    let ford = shop.vin_guides_for_make(460, 10).unwrap();
    assert_eq!(ford.total, 3);
    // Two letters of one day: the higher id first.
    assert_eq!(
        names(&ford.guides),
        ["ORG26393", "ORG26392", "01-22-N11B-1670"]
    );
    assert_eq!(
        ford.guides[1],
        VinGuide {
            name: "ORG26392".to_owned(),
            manufacturer: "FORD MOTOR COMPANY".to_owned(),
            kind: "565".to_owned(),
            letter_date: "2026-03-26".to_owned(),
            year_from: Some(2027),
            year_to: Some(2027),
            url: "https://vpic.nhtsa.dot.gov/mid/home/displayfile/00000000-0000-4000-8000-000000000003"
                .to_owned(),
        }
    );
    assert_eq!(
        (ford.guides[0].year_from, ford.guides[0].year_to),
        (Some(2026), None)
    );
    assert_eq!(
        (ford.guides[2].year_from, ford.guides[2].year_to),
        (None, None)
    );
    // Ford's letters reach the Mazdas it built too.
    assert_eq!(shop.vin_guides_for_make(473, 10).unwrap().total, 3);
    // The limit cuts the list, not the count; 0 is 1 and the most is 100.
    let two = shop.vin_guides_for_make(460, 2).unwrap();
    assert_eq!(
        (two.total, names(&two.guides)),
        (3, vec!["ORG26393", "ORG26392"])
    );
    assert_eq!(shop.vin_guides_for_make(460, 0).unwrap().guides.len(), 1);
    assert_eq!(
        shop.vin_guides_for_make(460, MOST_VIN_GUIDES + 50)
            .unwrap()
            .guides
            .len(),
        3
    );
    // A make no filing reaches.
    let none = shop.vin_guides_for_make(999, 10).unwrap();
    assert_eq!((none.total, none.guides.len()), (0, 0));
}

#[test]
fn a_codes_filings_are_its_manufacturers() {
    let shop = shop(true);
    let honda = shop.vin_guides_for_wmi("JHM", 10).unwrap();
    assert_eq!((honda.total, names(&honda.guides)), (1, vec!["ORG27020"]));
    assert_eq!(honda.guides[0].year_to, Some(2027));
    assert_eq!(shop.vin_guides_for_wmi("4F2", 10).unwrap().total, 3);
    // Codes are matched as they are written, and text is only ever bound.
    for code in ["jhm", "ZZZ", "", "' OR 1=1 --"] {
        assert_eq!(
            shop.vin_guides_for_wmi(code, 10).unwrap().total,
            0,
            "{code}"
        );
    }
    assert_eq!(shop.vin_guides_as_of(), Some("2026-10-12"));
}

#[test]
fn a_file_without_the_list_says_so_and_runs_no_query() {
    let shop = shop(false);
    for error in [
        shop.vin_guides_for_make(460, 10).unwrap_err(),
        shop.vin_guides_for_wmi("1FM", 10).unwrap_err(),
    ] {
        assert!(
            matches!(error, ShopError::DatasetUnavailable(Dataset::VinGuides)),
            "{error}"
        );
        assert_eq!(
            error.to_string(),
            "The vin_guides dataset is not available."
        );
    }
    assert_eq!(shop.vin_guides_as_of(), None);
}
