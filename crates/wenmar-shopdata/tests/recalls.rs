use rusqlite::Connection;
use serde_json::json;
use wenmar_shopdata::recalls::{MOST_RECALLS, RecallCursor, RecallQuery, RecallSource};
use wenmar_shopdata::schema::{META, RECALLS, SCHEMA_VERSION};
use wenmar_shopdata::sqlite::ShopSource;
use wenmar_shopdata::{Dataset, ShopData, ShopError};

const US: &str =
    r#""recalls_us":{"as_of":"2026-10-11","rows":7,"licence":"public-domain","attribution":""}"#;
const CA: &str = r#""recalls_ca":{"as_of":"2026-10-06","rows":3,"licence":"ogl-canada","attribution":"Contains information licensed under the Open Government Licence – Canada."}"#;

/// Ten campaigns. The columns are those of `schema::RECALLS`, in order.
/// The catalog ids are the real ones: Ford 460 and F-150 1801, Chevrolet
/// 467 and Silverado 1850, GMC 472, BMW 452 with 328i 1709 and 335i 1711.
/// Campaign 70V001000 and the second Silverado line of 19V888000 are made
/// up, to have a campaign with no date and a model reached twice.
const ROWS: &str = "
INSERT INTO recall VALUES
  (1, 'nhtsa', '17V669000', 'Ford Motor Company', '2017-10-23', '2017-12-05', 'POWER TRAIN:AUTOMATIC TRANSMISSION:LEVER AND LINKAGE:COLUMN SHIFT; POWER TRAIN:AUTOMATIC TRANSMISSION:CONTROL MODULE (TCM/PCM/TECM)', NULL, NULL, NULL, 'The shift lever may not be in the gear shown.', 'The vehicle may roll.', 'Dealers will update the software.', 'Owners may call Ford.', NULL, '17S35', NULL, 12429, 0, 0),
  (2, 'nhtsa', '19V888000', 'General Motors LLC', '2019-12-12', '2020-03-09', 'ELECTRICAL SYSTEM:12V/24V/48V BATTERY:CABLES', NULL, NULL, NULL, 'The battery cable nut may be loose.', 'A fire may start.', 'Dealers will tighten the nut.', NULL, NULL, 'N192273510', NULL, 350371, 0, 0),
  (3, 'nhtsa', '21V983000', 'Ford Motor Company', '2021-12-16', '2022-05-31', 'LATCHES/LOCKS/LINKAGES:TAILGATE:LATCH', NULL, NULL, NULL, 'The tailgate may open while driving.', 'Cargo may fall out.', 'Dealers will fix the latch.', NULL, NULL, '21S53', 'JL3A-9943150-BC, JL3A-9943150-AC', 57591, 0, 0),
  (4, 'nhtsa', '26V057000', 'Grand Design RV, LLC', '2026-01-30', '2026-03-04', 'EQUIPMENT:RECREATIONAL VEHICLE/TRAILER:PLUMBING:FREEZE PREVENTION HEATER', NULL, NULL, NULL, 'A heater may overheat at 100% load.', 'A fire may start.', 'Dealers will replace the heater.', NULL, NULL, '910055/M910056', 'SL-T1218G-10', 109, 1, 1),
  (5, 'nhtsa', '68V023000', 'CHRYSLER CORPORATION', '1968-03-19', '1968-03-14', 'SERVICE BRAKES, HYDRAULIC:FOUNDATION COMPONENTS:HOSES, LINES/PIPING, AND FITTINGS', NULL, NULL, NULL, NULL, NULL, NULL, 'POSSIBILITY THE FRONT WHEEL FLEXIBLE BRAKE HOSE WAS IMPROPERLY INSTALLED.', NULL, NULL, NULL, 2800, 0, 0),
  (6, 'tc', '2019631', NULL, '2019-12-12', NULL, 'Electrical', 'Circuit électrique', 'Safety Mfr', 'Sécurité - fabricant', 'Issue:\nThe nut may be loose.', NULL, NULL, NULL, 'Problème :\nUn écrou peut être desserré.', 'N192273510', NULL, 24289, NULL, NULL),
  (7, 'tc', '2021771', NULL, '2021-12-17', NULL, 'Structure', 'Structure', 'Safety Mfr', 'Sécurité - fabricant', 'Issue:\nThe tailgate may open.', NULL, NULL, NULL, 'Problème :\nLe hayon peut ouvrir.', '21S53', NULL, 14401, NULL, NULL),
  (8, 'tc', '2016438', NULL, '2016-09-08', NULL, 'Suspension', 'Suspension', 'Safety Mfr', 'Sécurité - fabricant', 'On certain M-series vehicles a bolt may be loose.', NULL, NULL, NULL, 'Un boulon peut être desserré.', NULL, NULL, 24, NULL, NULL),
  (9, 'nhtsa', '26V375000', 'Ford Motor Company', '2026-06-09', '2026-07-09', 'POWER TRAIN:AUTOMATIC TRANSMISSION:CONTROL MODULE (TCM/PCM/TECM)', NULL, NULL, NULL, 'The transmission may downshift.', 'The rear wheels may lock.', 'Dealers will update the software.', NULL, NULL, '26S42', 'JL3A-14C204-***', 10742, 0, 0),
  (10, 'nhtsa', '70V001000', 'OLD MAKER', '', NULL, NULL, NULL, NULL, NULL, NULL, NULL, NULL, NULL, NULL, NULL, NULL, NULL, 0, 0);
INSERT INTO recall_vehicle VALUES
  (1, 2018, 460, 1801, 'FORD', 'F-150', 'exact', '2017-01-12', '2017-10-09'),
  (2, 2019, 467, 1850, 'CHEVROLET', 'SILVERADO 1500', 'base', '2018-07-11', '2019-11-22'),
  (2, 2019, 467, 1850, 'CHEVROLET', 'SILVERADO', 'exact', '2018-06-01', NULL),
  (2, 2020, 467, 1850, 'CHEVROLET', 'SILVERADO 1500', 'base', '2018-07-11', '2019-11-22'),
  (2, 2019, 472, NULL, 'GMC', 'SIERRA 1500', NULL, '2018-07-11', '2019-11-22'),
  (3, 2018, 460, 1801, 'FORD', 'F-150', 'exact', NULL, NULL),
  (4, 2026, NULL, NULL, 'GRAND DESIGN', 'LINEAGE', NULL, '2026-01-09', '2026-01-27'),
  (5, NULL, NULL, NULL, 'JEEP', 'WAGONEER', NULL, NULL, NULL),
  (6, 2019, 467, 1850, 'CHEVROLET', 'SILVERADO', 'exact', NULL, NULL),
  (6, 2019, 472, NULL, 'GMC', 'SIERRA', NULL, NULL, NULL),
  (7, 2018, 460, 1801, 'FORD', 'F-150', 'exact', NULL, NULL),
  (8, 2015, 452, 1709, 'BMW', '3 SERIES', 'curated', NULL, NULL),
  (8, 2015, 452, 1711, 'BMW', '3 SERIES', 'curated', NULL, NULL),
  (8, 2016, 452, NULL, 'BMW', '3 SERIES', NULL, NULL, NULL),
  (9, 2018, 460, 1801, 'FORD', 'F-150', 'exact', '2017-02-13', '2018-07-11');
";

/// A shop file in memory holding `datasets` (the inside of the JSON
/// object) and, when `with_rows`, the recall tables and [`ROWS`].
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
        connection.execute_batch(RECALLS).unwrap();
        connection.execute_batch(ROWS).unwrap();
    }
    ShopData::new(ShopSource::from_connection(connection)).unwrap()
}

fn both() -> ShopData<ShopSource> {
    shop(&format!("{US},{CA}"), true)
}

fn listed(shop: &ShopData<ShopSource>, query: RecallQuery) -> Vec<String> {
    shop.recalls(&query)
        .unwrap()
        .recalls
        .into_iter()
        .map(|recall| recall.campaign)
        .collect()
}

fn query() -> RecallQuery {
    RecallQuery {
        limit: MOST_RECALLS,
        ..RecallQuery::default()
    }
}

#[test]
fn a_model_year_gets_its_campaigns_newest_first() {
    let shop = both();
    let found = shop.recalls_for_vehicle(460, 1801, 2018, None).unwrap();
    let numbers: Vec<&str> = found
        .iter()
        .map(|found| found.recall.campaign.as_str())
        .collect();
    assert_eq!(numbers, ["26V375000", "2021771", "21V983000", "17V669000"]);
    let newest = &found[0];
    assert_eq!(newest.recall.source, "nhtsa");
    assert_eq!(newest.recall.country, "us");
    assert_eq!(newest.recall.date.as_deref(), Some("2026-06-09"));
    assert_eq!(newest.recall.do_not_drive, Some(false));
    assert_eq!(newest.matched, "exact");
    assert_eq!(newest.models, ["F-150"]);
    assert_eq!(newest.built_from.as_deref(), Some("2017-02-13"));
    assert_eq!(newest.built_to.as_deref(), Some("2018-07-11"));
    // A Canadian campaign has no advisories and no build dates.
    let canadian = &found[1];
    assert_eq!(canadian.recall.source, "tc");
    assert_eq!(canadian.recall.country, "ca");
    assert_eq!(canadian.recall.do_not_drive, None);
    assert_eq!(canadian.recall.notification.as_deref(), Some("Safety Mfr"));
    assert_eq!(canadian.built_from, None);

    // One country at a time.
    let us = shop
        .recalls_for_vehicle(460, 1801, 2018, Some(RecallSource::Nhtsa))
        .unwrap();
    assert_eq!(us.len(), 3);
    let ca = shop
        .recalls_for_vehicle(460, 1801, 2018, Some(RecallSource::Tc))
        .unwrap();
    assert_eq!(ca.len(), 1);
    assert_eq!(ca[0].recall.campaign, "2021771");

    // Another year, another model, a make with no model: nothing.
    assert!(
        shop.recalls_for_vehicle(460, 1801, 2017, None)
            .unwrap()
            .is_empty()
    );
    assert!(
        shop.recalls_for_vehicle(460, 9999, 2018, None)
            .unwrap()
            .is_empty()
    );
    // A campaign with no year is never given for a model year.
    assert!(shop.recalls_for_vehicle(0, 0, 0, None).unwrap().is_empty());
}

#[test]
fn a_campaign_that_reaches_a_model_twice_is_given_once() {
    let shop = both();
    let found = shop.recalls_for_vehicle(467, 1850, 2019, None).unwrap();
    assert_eq!(found.len(), 2);
    // Same day: the higher row id first.
    assert_eq!(found[0].recall.campaign, "2019631");
    let merged = &found[1];
    assert_eq!(merged.recall.campaign, "19V888000");
    assert_eq!(merged.matched, "exact", "the better match is the one said");
    assert_eq!(merged.models, ["SILVERADO", "SILVERADO 1500"]);
    assert_eq!(merged.built_from.as_deref(), Some("2018-06-01"));
    assert_eq!(merged.built_to.as_deref(), Some("2019-11-22"));
    // One of its lines alone, for the next year.
    let next = shop.recalls_for_vehicle(467, 1850, 2020, None).unwrap();
    assert_eq!(next.len(), 1);
    assert_eq!(next[0].matched, "base");
    assert_eq!(next[0].models, ["SILVERADO 1500"]);
}

#[test]
fn a_vehicle_recall_is_one_flat_json_object() {
    let shop = both();
    let found = shop
        .recalls_for_vehicle(460, 1801, 2018, Some(RecallSource::Nhtsa))
        .unwrap();
    assert_eq!(
        serde_json::to_value(&found[1]).unwrap(),
        json!({
            "source": "nhtsa",
            "country": "us",
            "campaign": "21V983000",
            "date": "2021-12-16",
            "manufacturer": "Ford Motor Company",
            "component": "LATCHES/LOCKS/LINKAGES:TAILGATE:LATCH",
            "summary": "The tailgate may open while driving.",
            "mfr_campaign": "21S53",
            "units": 57591,
            "do_not_drive": false,
            "park_outside": false,
            "notification": null,
            "match": "exact",
            "models": ["F-150"],
            "built_from": null,
            "built_to": null
        })
    );
}

#[test]
fn one_campaign_comes_with_every_vehicle_it_lists() {
    let shop = both();
    // Letters in the number may be in either case.
    let detail = shop
        .recall(RecallSource::Nhtsa, " 19v888000 ")
        .unwrap()
        .unwrap();
    assert_eq!(detail.recall.campaign, "19V888000");
    assert_eq!(
        detail.recall.manufacturer.as_deref(),
        Some("General Motors LLC")
    );
    assert_eq!(detail.owner_notified.as_deref(), Some("2020-03-09"));
    assert_eq!(detail.consequence.as_deref(), Some("A fire may start."));
    assert_eq!(
        detail.remedy.as_deref(),
        Some("Dealers will tighten the nut.")
    );
    assert_eq!(detail.summary_fr, None);
    let lines: Vec<String> = detail
        .vehicles
        .iter()
        .map(|vehicle| {
            format!(
                "{:?} {} {} {:?} {:?}",
                vehicle.year,
                vehicle.make_text,
                vehicle.model_text,
                vehicle.model_id,
                vehicle.matched
            )
        })
        .collect();
    assert_eq!(
        lines,
        [
            "Some(2019) CHEVROLET SILVERADO Some(1850) Some(\"exact\")",
            "Some(2019) CHEVROLET SILVERADO 1500 Some(1850) Some(\"base\")",
            "Some(2020) CHEVROLET SILVERADO 1500 Some(1850) Some(\"base\")",
            "Some(2019) GMC SIERRA 1500 None None",
        ]
    );
    assert_eq!(detail.vehicles[3].make_id, Some(472));

    // The part numbers and both advisories.
    let advisory = shop
        .recall(RecallSource::Nhtsa, "26V057000")
        .unwrap()
        .unwrap();
    assert_eq!(advisory.recall.do_not_drive, Some(true));
    assert_eq!(advisory.recall.park_outside, Some(true));
    assert_eq!(advisory.part_number.as_deref(), Some("SL-T1218G-10"));
    assert_eq!(advisory.vehicles[0].make_id, None);

    // An old campaign with only notes: they are its summary, said once.
    let old = shop
        .recall(RecallSource::Nhtsa, "68V023000")
        .unwrap()
        .unwrap();
    assert!(
        old.recall
            .summary
            .as_deref()
            .unwrap()
            .starts_with("POSSIBILITY THE FRONT WHEEL")
    );
    assert_eq!(old.notes, None);
    assert_eq!(old.vehicles[0].year, None);
    let noted = shop
        .recall(RecallSource::Nhtsa, "17V669000")
        .unwrap()
        .unwrap();
    assert_eq!(noted.notes.as_deref(), Some("Owners may call Ford."));

    // A Canadian campaign carries its French text. Curated matches are one
    // line for each catalog model.
    let canadian = shop.recall(RecallSource::Tc, "2016438").unwrap().unwrap();
    assert_eq!(canadian.component_fr.as_deref(), Some("Suspension"));
    assert_eq!(
        canadian.notification_fr.as_deref(),
        Some("Sécurité - fabricant")
    );
    assert_eq!(
        canadian.summary_fr.as_deref(),
        Some("Un boulon peut être desserré.")
    );
    assert_eq!(canadian.recall.manufacturer, None);
    let models: Vec<Option<i64>> = canadian
        .vehicles
        .iter()
        .map(|vehicle| vehicle.model_id)
        .collect();
    assert_eq!(models, [Some(1709), Some(1711), None]);

    // A number is a campaign of one source only.
    assert!(
        shop.recall(RecallSource::Tc, "19V888000")
            .unwrap()
            .is_none()
    );
    assert!(
        shop.recall(RecallSource::Nhtsa, "2016438")
            .unwrap()
            .is_none()
    );
    assert!(shop.recall(RecallSource::Nhtsa, "").unwrap().is_none());
    assert!(
        shop.recall(RecallSource::Nhtsa, "' OR 1=1 --")
            .unwrap()
            .is_none()
    );
}

#[test]
fn the_list_is_newest_first_and_pages_without_gaps() {
    let shop = both();
    let everything = [
        "26V375000",
        "26V057000",
        "2021771",
        "21V983000",
        "2019631",
        "19V888000",
        "17V669000",
        "2016438",
        "68V023000",
        "70V001000",
    ];
    assert_eq!(listed(&shop, query()), everything);

    let mut seen: Vec<String> = Vec::new();
    let mut cursor: Option<RecallCursor> = None;
    let mut pages = 0;
    loop {
        let page = shop
            .recalls(&RecallQuery {
                limit: 3,
                cursor: cursor.clone(),
                ..RecallQuery::default()
            })
            .unwrap();
        pages += 1;
        assert!(page.recalls.len() <= 3);
        seen.extend(page.recalls.into_iter().map(|recall| recall.campaign));
        // A cursor survives being written out and read back.
        cursor = page
            .next_cursor
            .map(|next| RecallCursor::parse(&next.to_string()).unwrap());
        if cursor.is_none() {
            break;
        }
    }
    assert_eq!(seen, everything);
    assert_eq!(pages, 4);
    // A page that ends exactly at the last campaign has no cursor.
    let page = shop
        .recalls(&RecallQuery {
            limit: 10,
            ..RecallQuery::default()
        })
        .unwrap();
    assert_eq!(page.recalls.len(), 10);
    assert_eq!(page.next_cursor, None);
    // The limit is at least one and at most MOST_RECALLS.
    assert_eq!(listed(&shop, RecallQuery::default()), ["26V375000"]);
    assert_eq!(
        listed(
            &shop,
            RecallQuery {
                limit: 10_000,
                ..RecallQuery::default()
            }
        )
        .len(),
        10
    );
    // The campaign with no date says so.
    let last = shop.recalls(&query()).unwrap().recalls.pop().unwrap();
    assert_eq!(last.campaign, "70V001000");
    assert_eq!(last.date, None);
}

#[test]
fn the_list_is_narrowed_by_what_is_asked() {
    let shop = both();
    let with = |change: fn(&mut RecallQuery)| {
        let mut asked = query();
        change(&mut asked);
        listed(&shop, asked)
    };
    assert_eq!(
        with(|asked| asked.source = Some(RecallSource::Tc)),
        ["2021771", "2019631", "2016438"]
    );
    assert_eq!(
        with(|asked| asked.make_id = Some(460)),
        ["26V375000", "2021771", "21V983000", "17V669000"]
    );
    assert_eq!(
        with(|asked| {
            asked.make_id = Some(467);
            asked.model_id = Some(1850);
            asked.year = Some(2020);
        }),
        ["19V888000"]
    );
    assert_eq!(with(|asked| asked.year = Some(2015)), ["2016438"]);
    // The make id alone finds a campaign whose model did not match.
    assert_eq!(
        with(|asked| asked.make_id = Some(472)),
        ["2019631", "19V888000"]
    );
    // A component, in either case, anywhere in the text.
    assert_eq!(
        with(|asked| asked.component = Some("power train".to_owned())),
        ["26V375000", "17V669000"]
    );
    assert_eq!(
        with(|asked| asked.component = Some("STRUCTURE".to_owned())),
        ["2021771"]
    );
    // Reported on or after a day.
    assert_eq!(
        with(|asked| asked.since = Some("2021-12-17".to_owned())),
        ["26V375000", "26V057000", "2021771"]
    );
    assert_eq!(with(|asked| asked.since = Some("  ".to_owned())).len(), 10);
    // Everything together.
    assert_eq!(
        with(|asked| {
            asked.source = Some(RecallSource::Nhtsa);
            asked.make_id = Some(460);
            asked.year = Some(2018);
            asked.component = Some("tailgate".to_owned());
            asked.since = Some("2020-01-01".to_owned());
        }),
        ["21V983000"]
    );
    assert!(with(|asked| asked.model_id = Some(424_242)).is_empty());
}

#[test]
fn free_text_is_found_anywhere_in_a_campaign() {
    let shop = both();
    let find = |text: &str| {
        listed(
            &shop,
            RecallQuery {
                q: Some(text.to_owned()),
                ..query()
            },
        )
    };
    // The campaign number, in either case.
    assert_eq!(find("19v888"), ["19V888000"]);
    // The manufacturer's own number is the same in both countries.
    assert_eq!(find("21S53"), ["2021771", "21V983000"]);
    assert_eq!(find("n192273510"), ["2019631", "19V888000"]);
    // The manufacturer, the component, the summary, the consequence, the
    // remedy and the notes.
    assert_eq!(find("grand design"), ["26V057000"]);
    assert_eq!(find("freeze prevention"), ["26V057000"]);
    assert_eq!(find("TAILGATE"), ["2021771", "21V983000"]);
    assert_eq!(find("rear wheels may lock"), ["26V375000"]);
    assert_eq!(find("tighten the nut"), ["19V888000"]);
    assert_eq!(find("brake hose"), ["68V023000"]);
    // The make or the model of a vehicle it lists, matched or not.
    assert_eq!(find("sierra"), ["2019631", "19V888000"]);
    assert_eq!(find("wagoneer"), ["68V023000"]);
    // A percent sign and an underscore mean themselves.
    assert_eq!(find("100%"), ["26V057000"]);
    assert!(find("1_V").is_empty());
    assert!(find("no such words").is_empty());
    // Blank text narrows nothing.
    assert_eq!(find("   ").len(), 10);
    // Text and a filter together.
    assert_eq!(
        listed(
            &shop,
            RecallQuery {
                q: Some("tailgate".to_owned()),
                source: Some(RecallSource::Nhtsa),
                ..query()
            }
        ),
        ["21V983000"]
    );
}

#[test]
fn a_file_with_one_country_answers_for_that_country() {
    let canada = shop(CA, true);
    assert_eq!(canada.require_recalls(None).unwrap(), [RecallSource::Tc]);
    assert_eq!(canada.recalls_as_of(RecallSource::Tc), Some("2026-10-06"));
    assert_eq!(canada.recalls_as_of(RecallSource::Nhtsa), None);
    // Asked for both, it gives what it holds, and never a US row.
    let found = canada.recalls_for_vehicle(460, 1801, 2018, None).unwrap();
    assert_eq!(found.len(), 1);
    assert_eq!(found[0].recall.source, "tc");
    assert_eq!(listed(&canada, query()), ["2021771", "2019631", "2016438"]);
    // Asked for the other one, it names the dataset that is missing.
    for error in [
        canada
            .recalls_for_vehicle(460, 1801, 2018, Some(RecallSource::Nhtsa))
            .unwrap_err(),
        canada.recall(RecallSource::Nhtsa, "19V888000").unwrap_err(),
        canada
            .recalls(&RecallQuery {
                source: Some(RecallSource::Nhtsa),
                ..query()
            })
            .unwrap_err(),
    ] {
        assert!(
            matches!(error, ShopError::DatasetUnavailable(Dataset::RecallsUs)),
            "{error}"
        );
    }
    assert!(
        canada
            .recall(RecallSource::Tc, "2021771")
            .unwrap()
            .is_some()
    );

    let states = shop(US, true);
    assert_eq!(states.require_recalls(None).unwrap(), [RecallSource::Nhtsa]);
    assert_eq!(
        states.recalls_as_of(RecallSource::Nhtsa),
        Some("2026-10-11")
    );
    assert_eq!(
        states
            .recalls_for_vehicle(460, 1801, 2018, None)
            .unwrap()
            .len(),
        3
    );
    assert!(matches!(
        states.recall(RecallSource::Tc, "2021771").unwrap_err(),
        ShopError::DatasetUnavailable(Dataset::RecallsCa)
    ));
}

#[test]
fn a_file_with_no_recalls_names_the_dataset_and_runs_no_sql() {
    // No recall tables at all: a query would fail, so none may run.
    let empty = shop("", false);
    let errors = [
        empty.require_recalls(None).unwrap_err(),
        empty
            .recalls_for_vehicle(460, 1801, 2018, None)
            .unwrap_err(),
        empty.recall(RecallSource::Nhtsa, "19V888000").unwrap_err(),
        empty.recalls(&query()).unwrap_err(),
        empty.recall_years().unwrap_err(),
        empty.recall_models(2018).unwrap_err(),
    ];
    for error in errors {
        assert_eq!(
            error.to_string(),
            "The recalls_us dataset is not available."
        );
    }
    assert!(matches!(
        empty.recall(RecallSource::Tc, "2021771").unwrap_err(),
        ShopError::DatasetUnavailable(Dataset::RecallsCa)
    ));
    assert_eq!(empty.recalls_as_of(RecallSource::Nhtsa), None);
}

#[test]
fn the_years_and_models_with_a_matched_campaign_are_listed() {
    let shop = both();
    assert_eq!(shop.recall_years().unwrap(), [2020, 2019, 2018, 2015]);
    assert_eq!(shop.recall_models(2018).unwrap(), [(460, 1801)]);
    assert_eq!(shop.recall_models(2019).unwrap(), [(467, 1850)]);
    assert_eq!(
        shop.recall_models(2015).unwrap(),
        [(452, 1709), (452, 1711)]
    );
    // 2016 and 2026 have campaigns, but none matched to a model.
    assert!(shop.recall_models(2016).unwrap().is_empty());
    assert!(shop.recall_models(2026).unwrap().is_empty());
}
