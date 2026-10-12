use rusqlite::Connection;
use serde_json::json;
use wenmar_shopdata::recall_reports::RecallReport;
use wenmar_shopdata::schema::{META, RECALL_REPORTS, SCHEMA_VERSION};
use wenmar_shopdata::sqlite::ShopSource;
use wenmar_shopdata::{Dataset, ShopData, ShopError};

const REPORTS: &str = r#""recall_reports":{"as_of":"2026-10-12","rows":3,"licence":"public-domain","attribution":""}"#;
const US: &str =
    r#""recalls_us":{"as_of":"2026-10-11","rows":7,"licence":"public-domain","attribution":""}"#;

/// Three reports as NHTSA's files of 2026-10-12 have them. The columns are
/// those of `schema::RECALL_REPORTS`, in order.
const ROWS: &str = "
INSERT INTO recall_report VALUES
  ('15V687000', 'quarterly', 1, '2016-4', '2016-4', NULL, 0, 22, 0, 584),
  ('19V888000', 'quarterly', 6, '2021-2', '2021-2', '2021-07-19', 350371, 333125, 551, 46),
  ('21V983000', 'annual', 2, '2025-2', '2026-1', '2026-04-13', 57591, 38917, 1011, 23);
";

/// A shop file in memory holding `datasets` (the inside of the JSON
/// object) and, when `with_rows`, the report table and [`ROWS`].
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
        connection.execute_batch(RECALL_REPORTS).unwrap();
        connection.execute_batch(ROWS).unwrap();
    }
    ShopData::new(ShopSource::from_connection(connection)).unwrap()
}

#[test]
fn a_campaign_has_its_newest_report_or_none() {
    let shop = shop(REPORTS, true);
    assert_eq!(shop.recall_reports_as_of(), Some("2026-10-12"));
    let annual = shop.recall_report("21V983000").unwrap().unwrap();
    assert_eq!(
        annual,
        RecallReport {
            campaign: "21V983000".to_owned(),
            kind: "annual".to_owned(),
            report_number: 2,
            period_from: "2025-2".to_owned(),
            period_to: "2026-1".to_owned(),
            submitted: Some("2026-04-13".to_owned()),
            involved: 57_591,
            remedied: 38_917,
            unreachable: 1_011,
            removed: 23,
        }
    );
    assert_eq!(annual.remedied_percent(), Some(67));
    assert_eq!(
        serde_json::to_value(&annual).unwrap(),
        json!({
            "campaign": "21V983000", "kind": "annual", "report_number": 2,
            "period_from": "2025-2", "period_to": "2026-1", "submitted": "2026-04-13",
            "involved": 57591, "remedied": 38917, "unreachable": 1011, "removed": 23
        })
    );
    // Letters in either case, and blanks around the number.
    assert_eq!(
        shop.recall_report(" 19v888000 ")
            .unwrap()
            .unwrap()
            .remedied_percent(),
        Some(95)
    );
    // A report that gives no vehicles involved has no share.
    let none = shop.recall_report("15V687000").unwrap().unwrap();
    assert_eq!(none.submitted, None);
    assert_eq!(none.remedied_percent(), None);
    // A campaign with no report, and text that is no campaign number.
    assert_eq!(shop.recall_report("26V375000").unwrap(), None);
    assert_eq!(shop.recall_report("' OR 1=1 --").unwrap(), None);
    assert_eq!(shop.recall_report("").unwrap(), None);
}

#[test]
fn without_the_dataset_the_reader_says_so_and_runs_no_query() {
    // The recalls alone: no `recall_report` table exists, and the reader
    // must not find that out from SQLite.
    let shop = shop(US, false);
    assert_eq!(shop.recall_reports_as_of(), None);
    match shop.recall_report("21V983000") {
        Err(ShopError::DatasetUnavailable(Dataset::RecallReports)) => {}
        other => panic!("{other:?}"),
    }
    assert_eq!(
        shop.recall_report("21V983000").unwrap_err().to_string(),
        "The recall_reports dataset is not available."
    );
}
