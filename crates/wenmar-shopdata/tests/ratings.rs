use rusqlite::Connection;
use serde_json::json;
use wenmar_shopdata::ratings::RatingStars;
use wenmar_shopdata::schema::{META, RATINGS, SCHEMA_VERSION};
use wenmar_shopdata::sqlite::ShopSource;
use wenmar_shopdata::{Dataset, ShopData, ShopError};

const HELD: &str =
    r#""ratings":{"as_of":"2026-08-07","rows":6,"licence":"public-domain","attribution":""}"#;
const OTHER: &str =
    r#""recalls_us":{"as_of":"2026-10-11","rows":1,"licence":"public-domain","attribution":""}"#;

/// Six ratings. The columns are those of `schema::RATINGS`, in order. The
/// figures of the F-150, the Civic and the Corolla are NHTSA's for those
/// vehicles; the catalog ids are the real ones (Ford 460 and F-150 1801,
/// Honda 474 and Civic 1863, Toyota 448). Rating 5 is made up, to have a
/// later release and a rating matched to two models.
const ROWS: &str = "
INSERT INTO rating VALUES
  (1, 2018, 'FORD', 'F-150 SUPER CREW', 'PU/CC', 'TRUCK', '4x4', 1, 5, 5, 5, 5, 5, 5, 5, 5, 5, 5, 5, 4, 191, 0, 5060, NULL, 'optional', NULL, 'optional', NULL, 'optional', NULL, 'optional', NULL, NULL, 'optional', 'standard', 'standard', 'The entire rear seat pan lifts up during the test.'),
  (2, 2018, 'FORD', 'F-150 REGULAR CAB', 'PU/RC', 'TRUCK', '4x2', 1, 4, 4, 4, 4, 5, 5, NULL, 5, 5, NULL, 5, 4, 191, 0, 5276, NULL, 'no', NULL, 'no', NULL, 'no', NULL, 'no', NULL, NULL, 'optional', 'standard', 'standard', 'These ratings do not apply to vehicles produced after January 31, 2018'),
  (3, 2016, 'HONDA', 'CIVIC', '2 DR', 'PC', 'FWD', 1, 5, 4, 5, 4, 5, 5, 4, 5, 5, 4, 5, 5, 93, 0, 2735, NULL, 'optional', NULL, 'optional', NULL, 'no', NULL, 'no', NULL, 'standard', NULL, 'standard', 'standard', NULL),
  (4, 1999, 'TOYOTA', 'COROLLA', '4-DR W/SAB', 'LIGHT PASSENGER CAR', NULL, 1, NULL, NULL, 4, 4, NULL, 4, 3, NULL, NULL, NULL, NULL, NULL, NULL, NULL, NULL, 'no', 'no', NULL, 'no', NULL, 'no', NULL, 'no', NULL, NULL, NULL, NULL, 'optional', NULL),
  (5, 2018, 'FORD', 'F-150 SUPER CREW', 'PU/CC', 'TRUCK', '4x4', 2, NULL, NULL, NULL, NULL, NULL, NULL, NULL, NULL, NULL, NULL, NULL, 3, 219, 1, NULL, 'standard', 'both', 'upgrade_only', 'standard', 'meets', 'standard', 'pending', 'standard', 'does_not_meet', NULL, NULL, NULL, NULL, 'First note.' || char(10) || 'Second note.'),
  (6, 2024, 'PORSCHE', '911 CARRERA S', '2 DR', 'PC', 'RWD', 1, NULL, NULL, NULL, NULL, NULL, NULL, NULL, NULL, NULL, NULL, NULL, NULL, NULL, NULL, NULL, NULL, 'standard', 'does_not_meet', 'standard', 'does_not_meet', NULL, NULL, NULL, NULL, NULL, 'optional', 'standard', 'standard', NULL);
INSERT INTO rating_vehicle VALUES
  (1, 2018, 460, 1801, 'FORD', 'F-150 SUPER CREW', 'base'),
  (2, 2018, 460, 1801, 'FORD', 'F-150 REGULAR CAB', 'base'),
  (3, 2016, 474, 1863, 'HONDA', 'CIVIC', 'exact'),
  (4, 1999, 448, NULL, 'TOYOTA', 'COROLLA', NULL),
  (5, 2018, 460, 1801, 'FORD', 'F-150 SUPER CREW', 'curated'),
  (5, 2018, 460, 7331, 'FORD', 'F-150 SUPER CREW', 'curated'),
  (6, 2024, NULL, NULL, 'PORSCHE', '911 CARRERA S', NULL);
";

/// A shop file in memory holding `datasets` (the inside of the JSON
/// object) and, when `with_rows`, the rating tables and [`ROWS`].
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
        connection.execute_batch(RATINGS).unwrap();
        connection.execute_batch(ROWS).unwrap();
    }
    ShopData::new(ShopSource::from_connection(connection)).unwrap()
}

#[test]
fn a_model_year_has_every_rating_nhtsa_gave_it() {
    let shop = shop(HELD, true);
    assert_eq!(shop.ratings_as_of(), Some("2026-08-07"));
    let found = shop.ratings_for_vehicle(460, 1801, 2018).unwrap();
    let described: Vec<String> = found.iter().map(|rating| rating.description()).collect();
    assert_eq!(
        described,
        [
            "F-150 REGULAR CAB, PU/RC, 4x2",
            "F-150 SUPER CREW, PU/CC, 4x4",
            "F-150 SUPER CREW, PU/CC, 4x4, later release",
        ]
    );
    let crew = &found[1];
    assert_eq!(crew.matched, "base");
    assert_eq!(crew.vehicle_type.as_deref(), Some("TRUCK"));
    assert_eq!(crew.release, 1);
    assert_eq!(crew.stars.overall, Some(5));
    assert_eq!(crew.stars.rollover, Some(4));
    assert_eq!(crew.rollover_risk_permille, Some(191));
    assert_eq!(crew.rollover_tip, Some(false));
    assert_eq!(crew.curb_weight_lb, Some(5060));
    assert_eq!(
        crew.features.forward_collision_warning.as_deref(),
        Some("optional")
    );
    assert_eq!(crew.features.esc, None);
    assert_eq!(crew.features.head_side_airbag.as_deref(), Some("standard"));
    assert_eq!(
        crew.notes,
        ["The entire rear seat pan lifts up during the test."]
    );
    // A star NHTSA did not give is None, not zero.
    let regular = &found[0];
    assert_eq!(regular.stars.side_passenger, None);
    assert_eq!(regular.stars.side_rear_seat, None);
    assert_eq!(regular.stars.side, Some(5));

    let later = &found[2];
    assert_eq!(later.matched, "curated");
    assert!(later.stars.any());
    assert_eq!(later.stars.overall, None);
    assert_eq!(later.rollover_tip, Some(true));
    assert_eq!(later.curb_weight_lb, None);
    assert_eq!(later.features.esc.as_deref(), Some("standard"));
    assert_eq!(
        later
            .features
            .forward_collision_warning_evaluation
            .as_deref(),
        Some("upgrade_only")
    );
    assert_eq!(
        later.features.dynamic_brake_support_evaluation.as_deref(),
        Some("does_not_meet")
    );
    assert_eq!(later.notes, ["First note.", "Second note."]);

    // The same rating reaches the other model it was matched to.
    let heritage = shop.ratings_for_vehicle(460, 7331, 2018).unwrap();
    assert_eq!(heritage.len(), 1);
    assert_eq!(heritage[0].release, 2);
}

#[test]
fn a_rating_is_json_with_every_field_and_null_for_nothing() {
    let shop = shop(HELD, true);
    let found = shop.ratings_for_vehicle(474, 1863, 2016).unwrap();
    assert_eq!(found.len(), 1);
    assert_eq!(
        serde_json::to_value(&found[0]).unwrap(),
        json!({
            "model": "CIVIC",
            "body_style": "2 DR",
            "vehicle_type": "PC",
            "drive": "FWD",
            "release": 1,
            "match": "exact",
            "stars": {
                "overall": 5,
                "frontal": 4,
                "frontal_driver": 5,
                "frontal_passenger": 4,
                "side": 5,
                "side_driver": 5,
                "side_passenger": 4,
                "side_barrier": 5,
                "side_front_seat": 5,
                "side_rear_seat": 4,
                "side_pole": 5,
                "rollover": 5
            },
            "rollover_risk_permille": 93,
            "rollover_tip": false,
            "curb_weight_lb": 2735,
            "features": {
                "esc": null,
                "forward_collision_warning": "optional",
                "forward_collision_warning_evaluation": null,
                "lane_departure_warning": "optional",
                "lane_departure_warning_evaluation": null,
                "crash_imminent_braking": "no",
                "crash_imminent_braking_evaluation": null,
                "dynamic_brake_support": "no",
                "dynamic_brake_support_evaluation": null,
                "backup_camera": "standard",
                "blind_spot_detection": null,
                "head_side_airbag": "standard",
                "torso_side_airbag": "standard"
            },
            "notes": []
        })
    );
}

#[test]
fn a_model_year_nhtsa_does_not_list_has_none() {
    let shop = shop(HELD, true);
    // Another year of a rated model, a model with no rating, and ids that
    // are nothing.
    assert!(
        shop.ratings_for_vehicle(460, 1801, 2019)
            .unwrap()
            .is_empty()
    );
    assert!(
        shop.ratings_for_vehicle(474, 1861, 2016)
            .unwrap()
            .is_empty()
    );
    assert!(shop.ratings_for_vehicle(0, 0, 0).unwrap().is_empty());
    // A rating that matched no model is reached by no model.
    assert!(shop.ratings_for_vehicle(448, 0, 1999).unwrap().is_empty());
    assert_eq!(RatingStars::default().overall, None);
}

#[test]
fn a_file_without_ratings_says_so_and_runs_no_sql() {
    // No rating tables at all: a query would fail, so none may run.
    for datasets in ["", OTHER] {
        let shop = shop(datasets, false);
        assert_eq!(shop.ratings_as_of(), None);
        let error = shop.ratings_for_vehicle(460, 1801, 2018).unwrap_err();
        assert!(
            matches!(error, ShopError::DatasetUnavailable(Dataset::Ratings)),
            "{error}"
        );
        assert_eq!(error.to_string(), "The ratings dataset is not available.");
    }
}
