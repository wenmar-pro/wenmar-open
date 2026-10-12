use rusqlite::Connection;
use serde_json::json;
use wenmar_shopdata::fuel::FuelSource;
use wenmar_shopdata::schema::{FUEL, META, SCHEMA_VERSION};
use wenmar_shopdata::sqlite::ShopSource;
use wenmar_shopdata::{Dataset, ShopData, ShopError};

const US: &str =
    r#""fuel_us":{"as_of":"2026-09-29","rows":5,"licence":"public-domain","attribution":""}"#;
const CA: &str = r#""fuel_ca":{"as_of":"2026-10-06","rows":2,"licence":"ogl-canada","attribution":"Contains information licensed under the Open Government Licence – Canada."}"#;
const SPECS: &str =
    r#""specs":{"as_of":"2026-09-29","rows":5,"licence":"public-domain","attribution":""}"#;
const OIL: &str =
    r#""oil_grade":{"as_of":"2026-09-29","rows":5,"licence":"public-domain","attribution":""}"#;

/// Ratings of the 2018 Ford F-150 (make 460, model 1801), with the real
/// EPA ids and catalog engine ids, and one row each for a vehicle with no
/// catalog model and for another model. The columns are those of
/// `schema::FUEL`, in order. Row 5's oil lines and the second line of
/// 39449 are made up, to have an engine with two grades.
pub const ROWS: &str = "
INSERT INTO fuel_rating VALUES
  (1, 'epa', 39238, 2018, 460, 1801, 'Ford', 'F150 Pickup 2WD', 'exact', 144958, NULL, 'engine', '2WD', 'Pickup', NULL, 3.5, 6, 'Automatic (S10)', 'Rear-Wheel Drive', 'Regular Gasoline', NULL, 13.1, 9.4, 11.2, 18, 25, 21, NULL, NULL, 265),
  (2, 'epa', 39241, 2018, 460, 1801, 'Ford', 'F150 Pickup 2WD', 'exact', 144955, NULL, 'engine', '2WD', 'Pickup', NULL, 2.7, 6, 'Automatic (S10)', 'Rear-Wheel Drive', 'Regular Gasoline', NULL, 11.8, 9.0, 10.7, 20, 26, 22, NULL, NULL, 249),
  (3, 'epa', 39449, 2018, 460, 1801, 'Ford', 'F150 Pickup 2WD FFV', 'base', 144957, NULL, 'engine', '2WD', NULL, NULL, 3.3, 6, 'Automatic (S6)', 'Rear-Wheel Drive', 'Regular Gasoline', 'E85', 11.8, 9.4, 10.7, 20, 25, 22, NULL, NULL, 253),
  (4, 'epa', 39600, 2018, 460, 1801, 'Ford', 'F150 RAPTOR 4WD', 'base', 144958, 47514, 'submodel', '4WD', NULL, NULL, 3.5, 6, 'Automatic (S10)', 'Part-time 4-Wheel Drive', 'Premium Gasoline', NULL, 15.7, 13.1, 14.7, 15, 18, 16, NULL, NULL, 345),
  (5, 'epa', 39700, 2018, 460, 1801, 'Ford', 'F150 Pickup 4WD', 'exact', NULL, NULL, 'model', '4WD', 'Pickup', NULL, 4.6, 8, 'Automatic (S10)', '4-Wheel Drive', 'Regular Gasoline', NULL, 14.7, 11.2, 13.1, 16, 21, 18, NULL, NULL, 306),
  (6, 'epa', 41189, 2019, NULL, NULL, 'Tesla', 'Model 3 Long Range', NULL, NULL, NULL, NULL, NULL, NULL, NULL, NULL, NULL, 'Automatic (A1)', 'Rear-Wheel Drive', 'Electricity', NULL, NULL, NULL, NULL, NULL, NULL, NULL, 16.1, 499, 0),
  (7, 'epa', 41833, 2020, 467, 1850, 'Chevrolet', 'Silverado 4WD', 'exact', 155277, NULL, 'engine', '4WD', NULL, NULL, 5.3, 8, 'Automatic 8-spd', '4-Wheel Drive', 'Regular Gasoline', NULL, 14.7, 10.7, 13.1, 16, 22, 18, NULL, NULL, 307),
  (8, 'nrcan', NULL, 2018, 460, 1801, 'Ford', 'F-150', 'exact', 144955, NULL, 'engine', NULL, NULL, NULL, 2.7, 6, 'AS10', NULL, 'Regular Gasoline', NULL, 11.9, 9.0, 10.6, 20, 26, 22, NULL, NULL, 249),
  (9, 'nrcan', NULL, 2018, 460, 1801, 'Ford', 'F-150 FFV 4X4', 'base', 144957, NULL, 'engine', '4WD', NULL, NULL, 3.3, 6, 'AS6', NULL, 'E85', NULL, 17.6, 13.7, 15.8, 13, 17, 15, NULL, NULL, 263);
INSERT INTO vehicle_spec VALUES
  (39238, 'Standard Pickup Trucks 2WD', 1, 0, 1, NULL, NULL, NULL, NULL, NULL, NULL, NULL, NULL, NULL, 10, 'Semi-Automatic', 'Gasoline (Regular Unleaded Recommended)', 0, NULL, 1, 0, 2, 2, NULL, NULL, NULL),
  (39241, 'Standard Pickup Trucks 2WD', 1, 0, 1, NULL, NULL, NULL, NULL, NULL, NULL, NULL, NULL, NULL, 10, 'Semi-Automatic', 'Gasoline (Regular Unleaded Recommended)', 0, NULL, 1, 0, 2, 2, NULL, NULL, NULL),
  (39449, 'Standard Pickup Trucks 2WD', 0, 0, 1, 'FFV', NULL, NULL, NULL, NULL, NULL, NULL, NULL, NULL, 6, 'Semi-Automatic', 'Gasoline (Regular Unleaded Recommended)', 0, NULL, 1, 0, 2, 2, NULL, NULL, NULL),
  (39600, 'Standard Pickup Trucks 4WD', 1, 0, 1, NULL, NULL, NULL, NULL, NULL, NULL, NULL, NULL, NULL, NULL, NULL, NULL, NULL, NULL, NULL, NULL, NULL, NULL, NULL, NULL, NULL),
  (39700, 'Standard Pickup Trucks 4WD', 0, 0, NULL, NULL, NULL, NULL, 60, 130, NULL, 10, 15, NULL, 10, 'Semi-Automatic', NULL, 1, 'Active Fuel Management', 1, 0, 2, 2, 'Lithium Ion', 48, '8 and 30'),
  (41189, 'Midsize Cars', 0, 0, 0, 'EV', '211 kW AC 3-Phase', 10.0, NULL, 97, NULL, NULL, 15, NULL, NULL, NULL, NULL, NULL, NULL, NULL, NULL, NULL, NULL, NULL, NULL, NULL),
  (41833, 'Standard Pickup Trucks 4WD', 0, 0, 1, NULL, NULL, NULL, NULL, NULL, NULL, NULL, NULL, NULL, 8, 'Automatic', NULL, 1, 'Active Fuel Management', 1, 0, 1, 1, NULL, NULL, NULL);
INSERT INTO oil_grade VALUES
  (39238, '5W-30', '5W30 GF-5'),
  (39241, '5W-30', '5W-30 GF-5'),
  (39449, '5W-20', '5W-20 GF-5'),
  (39449, '5W-20, 0W-20', '5W-20/0W-20'),
  (39600, '5W-30', '5W30 GF-5'),
  (39700, '5W-20', '5W20'),
  (41833, '0W-20', '0W20');
INSERT INTO emission_standard VALUES
  (39241, 3, 'T3B70', 'Federal Tier 3 Bin 70', 5, 'JFMXT02.73JK'),
  (39241, 7, 'L3ULEV70', 'California LEV-III ULEV70', 5, 'JFMXT02.73JK'),
  (39700, 3, 'T3B70', NULL, NULL, 'JFMXT04.63JK'),
  (41189, 7, 'ZEV', 'California ZEV', 10, 'KTSLV00.0L13');
";

/// A shop file in memory holding `datasets` (the inside of the JSON
/// object) and, when `with_rows`, the fuel tables and [`ROWS`].
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
        connection.execute_batch(FUEL).unwrap();
        connection.execute_batch(ROWS).unwrap();
    }
    ShopData::new(ShopSource::from_connection(connection)).unwrap()
}

fn all() -> ShopData<ShopSource> {
    shop(&format!("{US},{CA},{SPECS},{OIL}"), true)
}

#[test]
fn a_model_year_gets_its_ratings_epa_first() {
    let shop = all();
    let ratings = shop.fuel_for_vehicle(460, 1801, 2018, None).unwrap();
    let lines: Vec<String> = ratings
        .iter()
        .map(|rating| {
            format!(
                "{} {} {:?} {} {} {:?}",
                rating.source,
                rating.country,
                rating.source_id,
                rating.confidence,
                rating.model_text,
                rating.displacement_l
            )
        })
        .collect();
    assert_eq!(
        lines,
        [
            "epa us Some(39241) engine F150 Pickup 2WD Some(2.7)",
            "epa us Some(39449) engine F150 Pickup 2WD FFV Some(3.3)",
            "epa us Some(39238) engine F150 Pickup 2WD Some(3.5)",
            "epa us Some(39600) submodel F150 RAPTOR 4WD Some(3.5)",
            "epa us Some(39700) model F150 Pickup 4WD Some(4.6)",
            "nrcan ca None engine F-150 Some(2.7)",
            "nrcan ca None engine F-150 FFV 4X4 Some(3.3)",
        ]
    );
    // One country alone.
    let canada = shop
        .fuel_for_vehicle(460, 1801, 2018, Some(FuelSource::Nrcan))
        .unwrap();
    assert_eq!(canada.len(), 2);
    assert!(canada.iter().all(|rating| rating.source == "nrcan"));
    // A model year with nothing, and a vehicle that matched no model.
    assert!(
        shop.fuel_for_vehicle(460, 1801, 2017, None)
            .unwrap()
            .is_empty()
    );
    assert!(shop.fuel_for_vehicle(0, 0, 2019, None).unwrap().is_empty());
    assert_eq!(shop.fuel_as_of(FuelSource::Epa), Some("2026-09-29"));
    assert_eq!(shop.fuel_as_of(FuelSource::Nrcan), Some("2026-10-06"));
}

#[test]
fn a_rating_is_one_json_object_with_units_apart() {
    let shop = all();
    let ratings = shop.fuel_for_vehicle(460, 1801, 2018, None).unwrap();
    assert_eq!(
        serde_json::to_value(&ratings[1]).unwrap(),
        json!({
            "source": "epa", "country": "us", "source_id": 39449,
            "match": "base", "confidence": "engine",
            "engine_id": 144957, "submodel_id": null,
            "model_text": "F150 Pickup 2WD FFV",
            "drive_text": "2WD", "body_text": null, "note_text": null,
            "displacement_l": 3.3, "cylinders": 6,
            "transmission": "Automatic (S6)", "drive": "Rear-Wheel Drive",
            "fuel": "Regular Gasoline", "fuel2": "E85",
            "l_100km": { "city": 11.8, "highway": 9.4, "combined": 10.7 },
            "mpg": { "city": 20, "highway": 25, "combined": 22 },
            "kwh_100km": null, "range_km": null, "co2_g_km": 253
        })
    );
    // A whole number stored in a REAL column still reads as a number.
    assert_eq!(ratings[0].l_100km.unwrap().highway, Some(9.0));
    // An electric vehicle has neither litres nor miles per gallon.
    let electric = shop
        .query(
            "SELECT count(*) FROM fuel_rating WHERE fuel = 'Electricity' AND combined_mpg IS NULL",
            &[],
        )
        .unwrap();
    assert_eq!(electric[0][0].integer(), Some(1));
}

#[test]
fn specs_come_with_the_rating_they_belong_to() {
    let shop = all();
    let specs = shop.specs_for_vehicle(460, 1801, 2018).unwrap();
    assert_eq!(specs.len(), 5);
    let ids: Vec<u32> = specs.iter().map(|spec| spec.epa_id).collect();
    assert_eq!(ids, [39241, 39449, 39238, 39600, 39700]);
    let turbo = &specs[0];
    assert!(turbo.turbo && !turbo.supercharger);
    assert_eq!(turbo.start_stop, Some(true));
    assert_eq!(turbo.gears, Some(10));
    assert_eq!(turbo.engine_id, Some(144955));
    assert_eq!(turbo.cylinder_deactivation, Some(false));
    assert_eq!(turbo.fuel, "Regular Gasoline");
    assert_eq!(specs[1].electrification.as_deref(), Some("FFV"));
    // Nothing from the guide file: every guide column is None.
    assert_eq!(specs[3].gears, None);
    assert_eq!(specs[3].cylinder_deactivation, None);
    assert_eq!(specs[3].submodel_id, Some(47514));
    // The four-door volumes are preferred to the two-door ones.
    let last = &specs[4];
    assert_eq!(
        (last.passenger_ft3, last.luggage_ft3),
        (Some(130), Some(15))
    );
    assert_eq!(last.start_stop, None);
    assert_eq!(
        last.cylinder_deactivation_name.as_deref(),
        Some("Active Fuel Management")
    );
    assert_eq!(last.battery_voltage, Some(48));
    assert_eq!(last.motor_kw.as_deref(), Some("8 and 30"));
    assert_eq!(last.confidence, "model");
}

#[test]
fn oil_grades_are_grouped_by_engine() {
    let shop = all();
    let oil = shop.oil_for_vehicle(460, 1801, 2018).unwrap();
    assert_eq!(
        serde_json::to_value(&oil).unwrap(),
        json!([
            { "confidence": "engine", "engine_id": 144955, "displacement_l": 2.7, "cylinders": 6,
              "grades": ["5W-30"], "texts": ["5W-30 GF-5"] },
            // Two spellings on one engine, the second naming a second grade.
            { "confidence": "engine", "engine_id": 144957, "displacement_l": 3.3, "cylinders": 6,
              "grades": ["5W-20", "0W-20"], "texts": ["5W-20 GF-5", "5W-20/0W-20"] },
            // Two ratings of one engine with the same spelling: one entry.
            { "confidence": "engine", "engine_id": 144958, "displacement_l": 3.5, "cylinders": 6,
              "grades": ["5W-30"], "texts": ["5W30 GF-5"] },
            // No engine settled: by engine size, marked `model`.
            { "confidence": "model", "engine_id": null, "displacement_l": 4.6, "cylinders": 8,
              "grades": ["5W-20"], "texts": ["5W20"] }
        ])
    );
    assert!(shop.oil_for_vehicle(460, 1801, 2017).unwrap().is_empty());
}

#[test]
fn emission_standards_come_by_sales_area() {
    let shop = all();
    let standards = shop.emissions_for_vehicle(460, 1801, 2018).unwrap();
    assert_eq!(
        serde_json::to_value(&standards).unwrap(),
        json!([
            { "epa_id": 39241, "confidence": "engine", "engine_id": 144955, "sales_area": 3,
              "standard": "T3B70", "standard_text": "Federal Tier 3 Bin 70", "smog_score": 5,
              "engine_family": "JFMXT02.73JK" },
            { "epa_id": 39241, "confidence": "engine", "engine_id": 144955, "sales_area": 7,
              "standard": "L3ULEV70", "standard_text": "California LEV-III ULEV70", "smog_score": 5,
              "engine_family": "JFMXT02.73JK" },
            { "epa_id": 39700, "confidence": "model", "engine_id": null, "sales_area": 3,
              "standard": "T3B70", "standard_text": null, "smog_score": null,
              "engine_family": "JFMXT04.63JK" }
        ])
    );
}

#[test]
fn each_reader_asks_for_its_dataset() {
    let unavailable = |result: Result<(), ShopError>, dataset: Dataset| match result {
        Err(ShopError::DatasetUnavailable(missing)) => assert_eq!(missing, dataset),
        other => panic!("expected {dataset} to be unavailable, got {other:?}"),
    };
    // No fuel at all: the United States dataset is the one named.
    let none = shop(SPECS, true);
    unavailable(
        none.fuel_for_vehicle(460, 1801, 2018, None).map(|_| ()),
        Dataset::FuelUs,
    );
    unavailable(
        none.oil_for_vehicle(460, 1801, 2018).map(|_| ()),
        Dataset::OilGrade,
    );
    assert_eq!(none.fuel_as_of(FuelSource::Epa), None);
    // One country held: the other is unavailable only when asked for.
    let us = shop(US, true);
    assert_eq!(us.fuel_for_vehicle(460, 1801, 2018, None).unwrap().len(), 5);
    unavailable(
        us.fuel_for_vehicle(460, 1801, 2018, Some(FuelSource::Nrcan))
            .map(|_| ()),
        Dataset::FuelCa,
    );
    unavailable(
        us.specs_for_vehicle(460, 1801, 2018).map(|_| ()),
        Dataset::Specs,
    );
    unavailable(
        us.emissions_for_vehicle(460, 1801, 2018).map(|_| ()),
        Dataset::Specs,
    );
    assert_eq!(us.require_fuel(None).unwrap(), [FuelSource::Epa]);
    // The names.
    assert_eq!(FuelSource::parse("nrcan"), Some(FuelSource::Nrcan));
    assert_eq!(FuelSource::from_country("us"), Some(FuelSource::Epa));
    assert_eq!(FuelSource::parse("tc"), None);
    assert_eq!(FuelSource::Nrcan.to_string(), "nrcan");
    assert_eq!(FuelSource::Epa.dataset(), Dataset::FuelUs);
}
