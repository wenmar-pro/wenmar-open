use open_data::shop::tc_recalls::{CATEGORIES, HEADER, TcCounts, TcRecallRow, keeps, parse};

use crate::common::{Scratch, fixture, tc_recalls_dir};

fn read(directory: &std::path::Path) -> anyhow::Result<(Vec<TcRecallRow>, TcCounts)> {
    let mut rows = Vec::new();
    let counts = parse(directory, &mut |row| {
        rows.push(row);
        Ok(())
    })?;
    Ok((rows, counts))
}

fn fixture_rows(name: &str) -> (Vec<TcRecallRow>, TcCounts) {
    let scratch = Scratch::new(name);
    read(&tc_recalls_dir(&scratch)).unwrap()
}

/// The rows of one campaign, in file order.
fn campaign<'r>(rows: &'r [TcRecallRow], campaign: &str) -> Vec<&'r TcRecallRow> {
    rows.iter().filter(|row| row.campaign == campaign).collect()
}

#[test]
fn the_header_is_the_fifteen_columns_the_file_has() {
    let text =
        std::fs::read_to_string(fixture("tc-recalls").join("vrdb_full_monthly.csv")).unwrap();
    let first = text.lines().next().unwrap();
    assert_eq!(first, HEADER.join(","));
    assert_eq!(
        first,
        "RECALL_NUMBER_NUM,YEAR,MANUFACTURER_RECALL_NO_TXT,CATEGORY_ETXT,CATEGORY_FTXT,MAKE_NAME_NM,MODEL_NAME_NM,UNIT_AFFECTED_NBR,SYSTEM_TYPE_ETXT,SYSTEM_TYPE_FTXT,NOTIFICATION_TYPE_ETXT,NOTIFICATION_TYPE_FTXT,COMMENT_ETXT,COMMENT_FTXT,RECALL_DATE_DTE"
    );
}

#[test]
fn keeps_road_vehicles_and_drops_the_rest() {
    let (rows, counts) = fixture_rows("tc-counts");
    assert_eq!(
        counts,
        TcCounts {
            records: 23,
            kept: 18,
            dropped: 5
        }
    );
    assert_eq!(rows.len(), 18);
    // A snowmobile, a piece of equipment, a tire and a child seat.
    for dropped in ["1976007", "1976101", "197801T", "1985044"] {
        assert!(campaign(&rows, dropped).is_empty(), "{dropped}");
    }
    // One campaign lists the same car under two categories. Only the row
    // for the car is kept.
    let mixed = campaign(&rows, "2015621");
    assert_eq!(mixed.len(), 1);
    assert_eq!(mixed[0].category_en, "Car");
    assert_eq!(mixed[0].mfr_campaign.as_deref(), Some("R0105 (51C014)"));
    // Every category of a kept row is one the list keeps.
    for row in &rows {
        assert_eq!(keeps(&row.category_en), Some(true), "{}", row.category_en);
    }
    let kept: Vec<&str> = rows.iter().map(|row| row.category_en.as_str()).collect();
    for category in [
        "Car",
        "SUV",
        "Light Truck & Van",
        "Truck - Med. & H.D.",
        "School Bus",
    ] {
        assert!(kept.contains(&category), "{category}");
    }
    assert_eq!(CATEGORIES.len(), 28);
}

#[test]
fn reads_the_year_the_units_and_the_missing_french() {
    let (rows, _) = fixture_rows("tc-fields");
    // YEAR is written 1975.0, and the comment has no French version.
    let first = campaign(&rows, "1975001")[0];
    assert_eq!(first.year, Some(1975));
    assert_eq!(
        (first.make_text.as_str(), first.model_text.as_str()),
        ("VOLVO", "200 SERIES")
    );
    assert_eq!(first.units, Some(2700));
    assert_eq!(first.system_en.as_deref(), Some("Steering"));
    assert_eq!(first.system_fr.as_deref(), Some("Direction"));
    assert_eq!(first.notification_en, "Safety Mfr");
    assert_eq!(first.notification_fr, "Sécurité - fabricant");
    assert_eq!(
        first.comment_en.as_deref(),
        Some("On certain Volvo 242, 244 and 245, the steering shaft nut could loosen")
    );
    assert_eq!(first.comment_fr, None);
    assert_eq!(first.recall_date.as_deref(), Some("1975-01-14"));
    assert_eq!(first.mfr_campaign, None);

    // -9999.0 means no model year.
    assert_eq!(campaign(&rows, "1975063")[0].year, None);
    // A quoted comment with quotation marks inside it, and fields nobody
    // filled in.
    let quoted = campaign(&rows, "1976151")[0];
    assert!(
        quoted
            .comment_en
            .as_deref()
            .unwrap()
            .starts_with("THE \"SPOKER\" WHEEL")
    );
    assert_eq!(quoted.system_en, None);
    assert_eq!(quoted.notification_en, "Not Entered");
    assert_eq!(quoted.notification_fr, "Non Saisie");
    // An empty unit count.
    let bus = campaign(&rows, "2025479")[0];
    assert_eq!(bus.units, None);
    assert_eq!(bus.category_fr, "Autobus scolaire");
    // A notification type other than a safety recall is kept as written.
    let superseded = campaign(&rows, "2012218")[0];
    assert_eq!(superseded.notification_en, "Superseded");
    assert_eq!(superseded.notification_fr, "Remplacé");
    assert_eq!(superseded.mfr_campaign.as_deref(), Some("12S35"));
}

#[test]
fn a_campaign_spans_a_row_for_each_vehicle_and_a_comment_spans_lines() {
    let (rows, _) = fixture_rows("tc-campaign");
    let trucks = campaign(&rows, "2019631");
    let vehicles: Vec<(Option<i32>, &str, &str)> = trucks
        .iter()
        .map(|row| (row.year, row.make_text.as_str(), row.model_text.as_str()))
        .collect();
    assert_eq!(
        vehicles,
        [
            (Some(2019), "GMC", "SIERRA"),
            (Some(2019), "CHEVROLET", "SILVERADO"),
            (Some(2020), "GMC", "SIERRA"),
            (Some(2020), "CHEVROLET", "SILVERADO"),
        ]
    );
    let first = trucks[0];
    assert_eq!(first.mfr_campaign.as_deref(), Some("N192273510"));
    assert_eq!(first.units, Some(24289));
    assert_eq!(first.system_en.as_deref(), Some("Electrical"));
    let english = first.comment_en.as_deref().unwrap();
    assert!(
        english.starts_with("Issue:\nOn certain trucks, the nut"),
        "{english}"
    );
    assert!(english.lines().count() >= 5, "{english}");
    assert!(!english.contains('\r'));
    let french = first.comment_fr.as_deref().unwrap();
    assert!(
        french.starts_with("Problème :\nDans certaines camionnettes"),
        "{french}"
    );
    // What a campaign says of itself is the same on every row.
    for row in &trucks {
        assert_eq!(row.comment_en, first.comment_en);
        assert_eq!(row.recall_date.as_deref(), Some("2019-12-12"));
    }
    // A family name the catalog lists as several models, over three years.
    let years: Vec<(Option<i32>, &str)> = campaign(&rows, "2016438")
        .iter()
        .map(|row| (row.year, row.model_text.as_str()))
        .collect();
    assert_eq!(
        years,
        [
            (Some(2015), "3 SERIES"),
            (Some(2016), "3 SERIES"),
            (Some(2016), "4 SERIES"),
            (Some(2016), "2 SERIES"),
            (Some(2017), "3 SERIES"),
        ]
    );
}

#[test]
fn refuses_a_directory_without_the_file() {
    let scratch = Scratch::new("tc-refuse");
    let empty = scratch.path("empty");
    std::fs::create_dir_all(&empty).unwrap();
    let error = format!("{:#}", read(&empty).unwrap_err());
    assert!(
        error.contains("vrdb_full_monthly.csv; run open-data fetch tc-recalls first"),
        "{error}"
    );
}

/// Run by hand after `open-data fetch tc-recalls`:
/// `cargo test -p open-data --test shop the_fetched_tc_file_parses -- --ignored --nocapture`
#[test]
#[ignore = "needs the fetched file in data/build/sources/tc-recalls"]
fn the_fetched_tc_file_parses() {
    let directory = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../data/build/sources/tc-recalls");
    let mut campaigns = std::collections::BTreeSet::new();
    let counts = parse(&directory, &mut |row| {
        campaigns.insert(row.campaign);
        Ok(())
    })
    .unwrap();
    println!("{counts:?}, {} campaigns", campaigns.len());
    assert!(counts.records > 140_000, "{counts:?}");
    assert!(counts.kept > 130_000, "{counts:?}");
    assert!(counts.dropped > 10_000, "{counts:?}");
    assert!(campaigns.len() > 16_000, "{}", campaigns.len());
}
