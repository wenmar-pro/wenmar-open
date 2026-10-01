use serde_json::json;
use wenmar_vin::{
    DataError, DecodeError, DecodeOptions, Decoder, Element, Engine, Manufacturer, MemoryData,
    Pattern, VinData, VinError, WarningCode,
};

const KONA: &str = "KM8K2CAB4PU001140";

/// Synthetic data shaped like vPIC. The values are made up for these tests.
fn data() -> MemoryData {
    MemoryData::new()
        .with_manufacturer(Manufacturer {
            wmi: "KM8".to_owned(),
            name: "Hyundai Motor Co".to_owned(),
            make: Some("Hyundai".to_owned()),
            country: Some("South Korea".to_owned()),
            vehicle_type: Some("Multipurpose Passenger Vehicle (MPV)".to_owned()),
            light_vehicle: true,
        })
        .with_schema("KM8", 1, 2022, None)
        .with_schema("KM8", 2, 2022, None)
        .with_schema("KM8", 3, 1990, Some(1995))
        // Schema 1: the model line this VIN belongs to.
        .with_pattern(1, "K2***", Element::Model, "Kona")
        .with_pattern(1, "K2***", Element::Make, "Hyundai")
        .with_pattern(1, "K2CA*", Element::Trim, "SE")
        .with_pattern(1, "K2***", Element::DriveType, "FWD/Front-Wheel Drive")
        .with_pattern(1, "K2***", Element::Doors, "4")
        .with_pattern(1, "K2***", Element::TpmsType, "Direct")
        .with_pattern(1, "K2***", Element::Series, "Not Applicable")
        .with_pattern(1, "****B", Element::DisplacementL, "2.0")
        .with_pattern(1, "****B", Element::EngineCylinders, "4")
        .with_pattern(1, "****B", Element::FuelTypePrimary, "Gasoline")
        // Schema 2: another model line, plus plant details shared by both.
        .with_pattern(2, "K3***", Element::Model, "Kona N")
        .with_pattern(2, "K[2-3]CAB", Element::DriveType, "AWD/All-Wheel Drive")
        .with_pattern(2, "*****|*U", Element::PlantCity, "Ulsan")
        .with_pattern(2, "*****|*U", Element::PlantCountry, "South Korea")
        // Schema 3: the same code thirty years earlier.
        .with_pattern(3, "K2***", Element::Model, "Old Model")
}

fn options() -> DecodeOptions {
    DecodeOptions {
        model_year: None,
        current_year: Some(2026),
    }
}

fn codes(decoded: &wenmar_vin::Decoded) -> Vec<WarningCode> {
    decoded
        .warnings
        .iter()
        .map(|warning| warning.code)
        .collect()
}

#[test]
fn decodes_a_vin_into_the_documented_json() {
    let decoded = Decoder::new(data()).decode(KONA, options()).unwrap();
    assert_eq!(
        serde_json::to_value(&decoded).unwrap(),
        json!({
            "vin": "KM8K2CAB4PU001140",
            "valid": true,
            "check_digit": { "valid": true, "expected": "4", "actual": "4" },
            "year": 2023,
            "make": "Hyundai",
            "model": "Kona",
            "trim": "SE",
            "doors": 4,
            "drivetrain": "FWD",
            "engine": {
                "label": "2.0L",
                "displacement_l": 2.0,
                "displacement_cc": 2000,
                "cylinders": 4,
                "fuel": "Gasoline"
            },
            "safety": { "tpms": "Direct" },
            "manufacturer": {
                "wmi": "KM8",
                "name": "Hyundai Motor Co",
                "country": "South Korea",
                "vehicle_type": "Multipurpose Passenger Vehicle (MPV)"
            },
            "plant": { "code": "U", "city": "Ulsan", "country": "South Korea" },
            "warnings": []
        })
    );
}

#[test]
fn other_schemas_fill_in_what_the_primary_schema_lacks() {
    let decoded = Decoder::new(data()).decode(KONA, options()).unwrap();
    assert_eq!(decoded.plant.city.as_deref(), Some("Ulsan"));
}

#[test]
fn placeholder_values_are_left_out() {
    let decoded = Decoder::new(data()).decode(KONA, options()).unwrap();
    assert_eq!(decoded.series, None);
    assert!(
        serde_json::to_value(&decoded)
            .unwrap()
            .get("series")
            .is_none()
    );
}

#[test]
fn pasted_input_decodes_the_same() {
    let decoder = Decoder::new(data());
    let clean = decoder.decode(KONA, options()).unwrap();
    let pasted = decoder.decode(" km8k2-cab4 pu001140\n", options()).unwrap();
    assert_eq!(clean, pasted);
}

#[test]
fn a_malformed_vin_is_an_error_with_suggestions() {
    let error = Decoder::new(data())
        .decode("KM8K2CAB4PU0O1140", options())
        .unwrap_err();
    let DecodeError::InvalidVin { error, suggestions } = error else {
        panic!("expected InvalidVin, got {error:?}");
    };
    assert!(matches!(error, VinError::InvalidCharacters(_)));
    assert_eq!(suggestions, vec![KONA]);
}

#[test]
fn non_ascii_input_is_an_error_not_a_panic() {
    let error = Decoder::new(data())
        .decode("ＫM8K2CAB4PU001140", options())
        .unwrap_err();
    assert!(matches!(error, DecodeError::InvalidVin { .. }));
}

#[test]
fn an_unknown_manufacturer_is_an_error() {
    let error = Decoder::new(data())
        .decode("1HGCM82633A004352", options())
        .unwrap_err();
    let DecodeError::UnknownManufacturer { wmi } = error else {
        panic!("expected UnknownManufacturer, got {error:?}");
    };
    assert_eq!(wmi, "1HG");
}

#[test]
fn a_wrong_check_digit_still_decodes_with_a_warning() {
    let decoded = Decoder::new(data())
        .decode("KM8K2CA84PU001140", options())
        .unwrap();
    assert!(!decoded.valid);
    assert_eq!(decoded.model.as_deref(), Some("Kona"));
    assert_eq!(codes(&decoded), vec![WarningCode::InvalidCheckDigit]);
    assert!(decoded.warnings[0].suggestions.contains(&KONA.to_owned()));
}

#[test]
fn the_other_thirty_year_cycle_is_used_when_only_it_has_data() {
    let only_old = MemoryData::new()
        .with_manufacturer(Manufacturer {
            wmi: "KM8".to_owned(),
            name: "Hyundai Motor Co".to_owned(),
            make: None,
            country: None,
            vehicle_type: None,
            light_vehicle: true,
        })
        .with_schema("KM8", 3, 1990, Some(1995))
        .with_pattern(3, "K2***", Element::Model, "Old Model");
    let decoded = Decoder::new(only_old).decode(KONA, options()).unwrap();
    assert_eq!(decoded.year, Some(1993));
    assert_eq!(decoded.model.as_deref(), Some("Old Model"));
}

#[test]
fn the_model_year_can_be_overridden() {
    let overridden = DecodeOptions {
        model_year: Some(1993),
        current_year: Some(2026),
    };
    let decoded = Decoder::new(data()).decode(KONA, overridden).unwrap();
    assert_eq!(decoded.year, Some(1993));
    assert_eq!(decoded.model.as_deref(), Some("Old Model"));
}

#[test]
fn a_year_with_no_data_returns_the_manufacturer_and_a_warning() {
    let overridden = DecodeOptions {
        model_year: Some(2005),
        current_year: Some(2026),
    };
    let decoded = Decoder::new(data()).decode(KONA, overridden).unwrap();
    assert_eq!(decoded.year, Some(2005));
    assert_eq!(decoded.make.as_deref(), Some("Hyundai"));
    assert_eq!(decoded.model, None);
    assert_eq!(decoded.manufacturer.name, "Hyundai Motor Co");
    assert_eq!(codes(&decoded), vec![WarningCode::NoPatterns]);
}

#[test]
fn a_vin_with_no_year_code_still_returns_what_is_known() {
    let decoded = Decoder::new(data())
        .decode("KM8K2CAB40U001140", options())
        .unwrap();
    assert_eq!(decoded.year, None);
    assert_eq!(decoded.manufacturer.name, "Hyundai Motor Co");
    let codes = codes(&decoded);
    assert!(codes.contains(&WarningCode::ModelYearUnknown));
    assert!(codes.contains(&WarningCode::NoPatterns));
}

#[test]
fn details_without_a_model_are_flagged() {
    let decoded = Decoder::new(data())
        .decode("KM8ZZZZZ4PU001140", options())
        .unwrap();
    assert_eq!(decoded.model, None);
    assert_eq!(decoded.plant.city.as_deref(), Some("Ulsan"));
    assert!(codes(&decoded).contains(&WarningCode::ModelUnresolved));
}

#[test]
fn malformed_patterns_in_the_data_are_ignored() {
    let messy = data()
        .with_pattern(1, "K[2", Element::Trim, "Broken")
        .with_pattern(1, "", Element::Trim, "Empty")
        .with_pattern(1, "K2***", Element::BodyClass, "   ");
    let decoded = Decoder::new(messy).decode(KONA, options()).unwrap();
    assert_eq!(decoded.trim.as_deref(), Some("SE"));
    assert_eq!(decoded.body, None);
}

#[test]
fn the_six_character_manufacturer_code_is_tried_first() {
    let low_volume = MemoryData::new()
        .with_manufacturer(Manufacturer {
            wmi: "1Z9".to_owned(),
            name: "Shared Code".to_owned(),
            make: None,
            country: None,
            vehicle_type: None,
            light_vehicle: true,
        })
        .with_manufacturer(Manufacturer {
            wmi: "1Z9456".to_owned(),
            name: "Small Builder".to_owned(),
            make: Some("Small".to_owned()),
            country: None,
            vehicle_type: None,
            light_vehicle: true,
        });
    let decoded = Decoder::new(low_volume)
        .decode("1Z9AB1C23DE456789", options())
        .unwrap();
    assert_eq!(decoded.manufacturer.name, "Small Builder");
    assert_eq!(decoded.manufacturer.wmi, "1Z9456");
}

#[test]
fn a_data_failure_is_reported_as_such() {
    struct Broken;
    impl VinData for Broken {
        fn manufacturer(&self, _wmi: &str) -> Result<Option<Manufacturer>, DataError> {
            Err("the data file is unreadable".into())
        }
        fn schemas(&self, _wmi: &str, _year: u16) -> Result<Vec<wenmar_vin::SchemaRef>, DataError> {
            Ok(Vec::new())
        }
        fn patterns(
            &self,
            _schema_ids: &[i64],
            _match_key: &str,
        ) -> Result<Vec<Pattern>, DataError> {
            Ok(Vec::new())
        }
    }
    let error = Decoder::new(Broken).decode(KONA, options()).unwrap_err();
    assert!(matches!(error, DecodeError::Data(_)));
}

#[test]
fn the_default_options_use_the_clock() {
    let decoded = Decoder::new(data())
        .decode(KONA, DecodeOptions::default())
        .unwrap();
    assert_eq!(decoded.year, Some(2023));
}

fn maker() -> Manufacturer {
    Manufacturer::new("KM8", "Maker")
}

/// A manufacturer of vehicles that are not cars, MPVs or light trucks, with
/// schema 1 current from 2022 and schema 2 for 1990 to 1995.
fn heavy_two_cycles() -> MemoryData {
    let heavy = Manufacturer {
        light_vehicle: false,
        ..Manufacturer::new("KM8", "Heavy Maker")
    };
    MemoryData::new()
        .with_manufacturer(heavy)
        .with_schema("KM8", 1, 2022, None)
        .with_schema("KM8", 2, 1990, Some(1995))
}

/// One manufacturer with a single schema current from 2022.
fn one_schema() -> MemoryData {
    MemoryData::new()
        .with_manufacturer(maker())
        .with_schema("KM8", 1, 2022, None)
}

/// One manufacturer with schema 1 current from 2022 and schema 2 for 1990 to 1995.
fn two_cycles() -> MemoryData {
    one_schema().with_schema("KM8", 2, 1990, Some(1995))
}

fn decode(data: MemoryData) -> wenmar_vin::Decoded {
    Decoder::new(data).decode(KONA, options()).unwrap()
}

#[test]
fn placeholder_manufacturer_fields_are_left_out() {
    let data = MemoryData::new().with_manufacturer(Manufacturer {
        wmi: "KM8".to_owned(),
        name: " Hyundai Motor Co ".to_owned(),
        make: Some(String::new()),
        country: Some("  ".to_owned()),
        vehicle_type: Some("Not Applicable".to_owned()),
        light_vehicle: true,
    });
    let decoded = decode(data);
    assert_eq!(decoded.make, None);
    assert_eq!(decoded.manufacturer.country, None);
    assert_eq!(decoded.manufacturer.vehicle_type, None);
    assert_eq!(decoded.manufacturer.name, "Hyundai Motor Co");
}

#[test]
fn a_heavy_vehicle_uses_the_other_cycle_when_the_likeliest_year_matches_nothing() {
    let decoded = decode(
        heavy_two_cycles()
            .with_pattern(1, "Z9***", Element::Model, "Newer Model")
            .with_pattern(2, "K2***", Element::Model, "Old Model"),
    );
    assert_eq!(decoded.year, Some(1993));
    assert_eq!(decoded.model.as_deref(), Some("Old Model"));
    assert_eq!(codes(&decoded), Vec::new());
}

#[test]
fn a_light_vehicle_keeps_its_year_even_when_only_the_other_cycle_has_a_model() {
    // Position 7 settles the cycle for cars, MPVs and light trucks. A new
    // model missing from the data must not be reported as a 30-year-old one.
    let decoded = decode(
        two_cycles()
            .with_pattern(1, "Z9***", Element::Model, "Newer Model")
            .with_pattern(2, "K2***", Element::Model, "Old Model"),
    );
    assert_eq!(decoded.year, Some(2023));
    assert_eq!(decoded.model, None);
    assert_eq!(codes(&decoded), vec![WarningCode::NoPatterns]);
}

#[test]
fn for_a_heavy_vehicle_a_model_outweighs_a_plant_city() {
    let decoded = decode(
        heavy_two_cycles()
            .with_pattern(1, "*****|*U", Element::PlantCity, "Ulsan")
            .with_pattern(2, "K2***", Element::Model, "Old Model"),
    );
    assert_eq!(decoded.year, Some(1993));
    assert_eq!(decoded.model.as_deref(), Some("Old Model"));
    assert_eq!(decoded.plant.city, None);
}

#[test]
fn the_likeliest_year_is_kept_when_no_cycle_has_a_model() {
    let decoded = decode(
        two_cycles()
            .with_pattern(1, "*****|*U", Element::PlantCity, "Ulsan")
            .with_pattern(2, "*****|*U", Element::PlantCity, "Asan"),
    );
    assert_eq!(decoded.year, Some(2023));
    assert_eq!(decoded.plant.city.as_deref(), Some("Ulsan"));
    assert_eq!(codes(&decoded), vec![WarningCode::ModelUnresolved]);
}

#[test]
fn the_make_comes_from_the_same_pattern_as_the_model() {
    let decoded = decode(
        one_schema()
            .with_pattern(1, "K2***", Element::Model, "Camry")
            .with_pattern(1, "*2C**", Element::Model, "ES 350")
            // The wrong make comes first, so matching on the schema alone picks it.
            .with_pattern(1, "K2***", Element::Make, "Toyota")
            .with_pattern(1, "*2C**", Element::Make, "Lexus"),
    );
    // "*2C**" sorts before "K2***", so NHTSA's order picks the ES 350.
    assert_eq!(decoded.model.as_deref(), Some("ES 350"));
    assert_eq!(decoded.make.as_deref(), Some("Lexus"));
}

#[test]
fn the_lowest_pattern_id_wins_a_tie() {
    let decoded = decode(
        one_schema()
            .with_pattern(1, "K2***", Element::Model, "Kona")
            .with_pattern(1, "K2***", Element::Trim, "First")
            .with_pattern(1, "K2***", Element::Trim, "Second"),
    );
    assert_eq!(decoded.trim.as_deref(), Some("First"));
}

#[test]
fn displacement_keeps_its_decimal_value_in_json() {
    let decoded = decode(
        one_schema()
            .with_pattern(1, "K2***", Element::Model, "Kona")
            .with_pattern(1, "K2***", Element::DisplacementL, "2.4"),
    );
    let json = serde_json::to_value(&decoded).unwrap();
    assert_eq!(json["engine"]["displacement_l"], json!(2.4));
}

#[test]
fn only_a_spelled_out_drive_type_is_shortened() {
    let drivetrain = |value: &str| {
        decode(one_schema().with_pattern(1, "K2***", Element::DriveType, value)).drivetrain
    };
    assert_eq!(drivetrain("4WD/4-Wheel Drive/4x4").as_deref(), Some("4WD"));
    assert_eq!(drivetrain("2WD/4WD").as_deref(), Some("2WD/4WD"));
    assert_eq!(drivetrain("4x2").as_deref(), Some("4x2"));
    assert_eq!(drivetrain("/Front").as_deref(), Some("/Front"));
}

/// Wraps the fixture data and records the match key each pattern request carries.
struct Recording {
    inner: MemoryData,
    keys: std::cell::RefCell<Vec<String>>,
}

impl Recording {
    fn new() -> Self {
        Self {
            inner: data(),
            keys: std::cell::RefCell::default(),
        }
    }
}

impl VinData for Recording {
    fn manufacturer(&self, wmi: &str) -> Result<Option<Manufacturer>, DataError> {
        self.inner.manufacturer(wmi)
    }
    fn schemas(&self, wmi: &str, year: u16) -> Result<Vec<wenmar_vin::SchemaRef>, DataError> {
        self.inner.schemas(wmi, year)
    }
    fn patterns(&self, schema_ids: &[i64], match_key: &str) -> Result<Vec<Pattern>, DataError> {
        self.keys.borrow_mut().push(match_key.to_owned());
        self.inner.patterns(schema_ids, match_key)
    }
}

#[test]
fn the_data_source_is_given_the_match_key_so_it_can_pre_filter() {
    let recording = Recording::new();
    Decoder::new(&recording).decode(KONA, options()).unwrap();
    assert_eq!(*recording.keys.borrow(), vec!["K2CAB|PU001140"]);
}

#[test]
fn patterns_are_not_requested_when_no_schema_applies() {
    let recording = Recording::new();
    let overridden = DecodeOptions {
        model_year: Some(2005),
        current_year: Some(2026),
    };
    Decoder::new(&recording).decode(KONA, overridden).unwrap();
    assert_eq!(*recording.keys.borrow(), Vec::<String>::new());
}

#[test]
fn the_schema_with_the_later_start_year_wins() {
    let data = MemoryData::new()
        .with_manufacturer(maker())
        .with_schema("KM8", 1, 2020, None)
        .with_schema("KM8", 2, 2022, None)
        .with_pattern(1, "K2***", Element::Trim, "From The 2020 Schema")
        .with_pattern(2, "K2***", Element::Trim, "From The 2022 Schema");
    assert_eq!(decode(data).trim.as_deref(), Some("From The 2022 Schema"));
}

#[test]
fn the_more_recently_changed_pattern_wins() {
    // The older pattern has the lower id, so only the change date can pick "Newer".
    let data = one_schema()
        .with_pattern_changed(
            1,
            "K2***",
            Element::Trim,
            "Older",
            "2015-03-04 10:05:33.893",
        )
        .with_pattern_changed(1, "K2***", Element::Trim, "Newer", "2020-01-01 00:00:00");
    assert_eq!(decode(data).trim.as_deref(), Some("Newer"));
}

#[test]
fn the_schema_year_outranks_the_change_date() {
    let data = MemoryData::new()
        .with_manufacturer(maker())
        .with_schema("KM8", 1, 2020, None)
        .with_schema("KM8", 2, 2022, None)
        .with_pattern_changed(
            1,
            "K2***",
            Element::Trim,
            "Old Schema, New Edit",
            "2024-01-01 00:00:00",
        )
        .with_pattern_changed(
            2,
            "K2***",
            Element::Trim,
            "New Schema, Old Edit",
            "2015-01-01 00:00:00",
        );
    assert_eq!(decode(data).trim.as_deref(), Some("New Schema, Old Edit"));
}

#[test]
fn with_equal_dates_the_pattern_with_fewer_fixed_characters_wins() {
    // "*2C**" sorts first as text and has the lower id, so only its extra
    // fixed character can make it lose.
    let data = one_schema()
        .with_pattern(1, "*2C**", Element::Trim, "Two Fixed")
        .with_pattern(1, "K****", Element::Trim, "One Fixed");
    assert_eq!(decode(data).trim.as_deref(), Some("One Fixed"));
}

#[test]
fn with_equal_length_the_keys_that_sort_first_win() {
    let data = one_schema()
        .with_pattern(1, "K2***", Element::Trim, "K First")
        .with_pattern(1, "*2C**", Element::Trim, "Star First");
    assert_eq!(decode(data).trim.as_deref(), Some("Star First"));
}

#[test]
fn keys_are_compared_with_their_brackets_removed() {
    // Both keys are five characters once `*` is removed. As written,
    // "K2[C]**" sorts first; with brackets removed, "K2*A*" sorts before
    // "K2C**". The lower id also belongs to the loser.
    let data = one_schema()
        .with_pattern(1, "K2[C]**", Element::Trim, "Sorts First As Written")
        .with_pattern(1, "[K]2*A*", Element::Trim, "Sorts First Without Brackets");
    assert_eq!(
        decode(data).trim.as_deref(),
        Some("Sorts First Without Brackets")
    );
}

#[test]
fn a_schema_linked_twice_ranks_by_its_latest_start_year() {
    // The later link is listed first, so "last link seen" would rank schema 1
    // at 2015 and lose to schema 2.
    let data = MemoryData::new()
        .with_manufacturer(maker())
        .with_schema("KM8", 1, 2023, None)
        .with_schema("KM8", 1, 2015, None)
        .with_schema("KM8", 2, 2022, None)
        .with_pattern(1, "K2***", Element::Trim, "Linked Twice")
        .with_pattern(2, "K2***", Element::Trim, "Linked Once");
    assert_eq!(decode(data).trim.as_deref(), Some("Linked Twice"));
}

#[test]
fn heavy_vehicles_try_the_later_cycle_first() {
    let truck = Manufacturer {
        light_vehicle: false,
        ..Manufacturer::new("1M8", "Truck Maker")
    };
    let data = MemoryData::new()
        .with_manufacturer(truck)
        .with_schema("1M8", 1, 1985, None)
        .with_pattern(1, "GDM9A", Element::Model, "Coach");
    let decoded = Decoder::new(data)
        .decode("1M8GDM9AXKP042788", options())
        .unwrap();
    assert_eq!(decoded.year, Some(2019));
}

#[test]
fn vpic_element_ids_map_to_elements() {
    assert_eq!(Element::from_vpic_id(28), Some(Element::Model));
    assert_eq!(Element::from_vpic_id(168), Some(Element::TpmsType));
    assert_eq!(Element::from_vpic_id(11), Some(Element::DisplacementCc));
    assert_eq!(Element::from_vpic_id(96), None);
    assert_eq!(Element::from_vpic_id(-1), None);
}
#[test]
fn a_heavy_vehicle_takes_the_year_that_explains_more_of_the_vin() {
    // Like a 1989 coach whose code is also used in 2019: both cycles resolve
    // a model, but only the earlier one also explains the engine.
    let decoded = decode(
        heavy_two_cycles()
            .with_pattern(1, "K2***", Element::Model, "Newer Coach")
            .with_pattern(2, "K2***", Element::Model, "Older Coach")
            .with_pattern(2, "K2***", Element::FuelTypePrimary, "Diesel")
            .with_pattern(2, "K2***", Element::DisplacementCi, "736"),
    );
    assert_eq!(decoded.year, Some(1993));
    assert_eq!(decoded.model.as_deref(), Some("Older Coach"));
}

#[test]
fn with_equal_weight_a_heavy_vehicle_takes_the_year_with_more_matching_patterns() {
    // Doors carries no weight, so only the pattern count separates the years.
    let decoded = decode(
        heavy_two_cycles()
            .with_pattern(1, "K2***", Element::Model, "Newer Coach")
            .with_pattern(2, "K2***", Element::Model, "Older Coach")
            .with_pattern(2, "K2***", Element::Doors, "2"),
    );
    assert_eq!(decoded.year, Some(1993));
}

#[test]
fn with_everything_equal_a_heavy_vehicle_takes_the_later_year() {
    let decoded = decode(
        heavy_two_cycles()
            .with_pattern(1, "K2***", Element::Model, "Newer Coach")
            .with_pattern(2, "K2***", Element::Model, "Older Coach"),
    );
    assert_eq!(decoded.year, Some(2023));
}

#[test]
fn displacement_counts_once_whatever_the_unit() {
    // The later year weighs 99 + 99 + 61 = 259. The earlier weighs 99 + 98 =
    // 197 if displacement counts once, and 393 if each unit were counted.
    let decoded = decode(
        heavy_two_cycles()
            .with_pattern(1, "K2***", Element::Model, "Newer Coach")
            .with_pattern(1, "K2***", Element::BodyClass, "Bus")
            .with_pattern(1, "K2***", Element::Trim, "Deluxe")
            .with_pattern(2, "K2***", Element::Model, "Older Coach")
            .with_pattern(2, "K2***", Element::DisplacementL, "12.1")
            .with_pattern(2, "K2***", Element::DisplacementCc, "12061")
            .with_pattern(2, "K2***", Element::DisplacementCi, "736"),
    );
    assert_eq!(decoded.year, Some(2023));
}

#[test]
fn a_heavy_vehicle_that_matches_nothing_keeps_the_later_year() {
    let decoded = decode(
        heavy_two_cycles()
            .with_pattern(1, "Z9***", Element::Model, "Newer Coach")
            .with_pattern(2, "Z9***", Element::Model, "Older Coach"),
    );
    assert_eq!(decoded.year, Some(2023));
    assert_eq!(codes(&decoded), vec![WarningCode::NoPatterns]);
}

#[test]
fn elements_carry_nhtsa_weights() {
    let expected = [
        (Element::Make, 99),
        (Element::Model, 99),
        (Element::BodyClass, 99),
        (Element::PlantCountry, 99),
        (Element::PlantCity, 98),
        (Element::PlantState, 98),
        (Element::FuelTypePrimary, 91),
        (Element::EngineCylinders, 88),
        (Element::AirbagsFront, 64),
        (Element::Series, 61),
        (Element::Trim, 61),
        (Element::EngineModel, 55),
        // Weighted by NHTSA but not named by this crate: GVWR, seat belt
        // type, axles.
        (Element::Gvwr, 70),
        (Element::Other(79), 65),
        (Element::Other(41), 15),
        (Element::Doors, 0),
        (Element::Other(40), 0),
    ];
    for (element, weight) in expected {
        assert_eq!(element.weight(), weight, "{element:?}");
    }
}

#[test]
fn elements_the_crate_does_not_name_still_count_towards_the_year() {
    // The later year weighs 99 + 61 = 160. The earlier weighs 99 + 65 = 164,
    // because seat belt type (vPIC element 79) weighs 65.
    let decoded = decode(
        heavy_two_cycles()
            .with_pattern(1, "K2***", Element::Model, "Newer Coach")
            .with_pattern(1, "K2***", Element::Trim, "Deluxe")
            .with_pattern(2, "K2***", Element::Model, "Older Coach")
            .with_pattern(2, "K2***", Element::Other(79), "Class 8"),
    );
    assert_eq!(decoded.year, Some(1993));
}

#[test]
fn patterns_are_counted_once_per_element() {
    // Equal weight. The later year has four matching rows but two elements;
    // the earlier has three rows and three elements.
    let decoded = decode(
        heavy_two_cycles()
            .with_pattern(1, "K2***", Element::Model, "Newer Coach")
            .with_pattern(1, "K2***", Element::Doors, "2")
            .with_pattern(1, "K****", Element::Doors, "4")
            .with_pattern(1, "*2***", Element::Doors, "5")
            .with_pattern(2, "K2***", Element::Model, "Older Coach")
            .with_pattern(2, "K2***", Element::Doors, "2")
            .with_pattern(2, "K2***", Element::Other(40), "6"),
    );
    assert_eq!(decoded.year, Some(1993));
}

#[test]
fn displacement_weighs_ninety_eight() {
    let earlier_has_displacement = |later: Element| {
        decode(
            heavy_two_cycles()
                .with_pattern(1, "K2***", Element::Model, "Newer Coach")
                .with_pattern(1, "K2***", later, "x")
                .with_pattern(2, "K2***", Element::Model, "Older Coach")
                .with_pattern(2, "K2***", Element::DisplacementCi, "736"),
        )
        .year
    };
    // 98 beats fuel type's 91 and loses to body class's 99.
    assert_eq!(
        earlier_has_displacement(Element::FuelTypePrimary),
        Some(1993)
    );
    assert_eq!(earlier_has_displacement(Element::BodyClass), Some(2023));
}

#[test]
fn litres_are_worked_out_from_cubic_centimetres() {
    let decoded = decode(one_schema().with_pattern(1, "K2***", Element::DisplacementCc, "1998"));
    let engine = decoded.engine.unwrap();
    assert_eq!(engine.displacement_l, Some(2.0));
    assert_eq!(engine.displacement_cc, Some(1998));
    assert_eq!(engine.label.as_deref(), Some("2.0L"));
}

#[test]
fn litres_are_worked_out_from_cubic_inches() {
    let decoded = decode(one_schema().with_pattern(1, "K2***", Element::DisplacementCi, "350"));
    let engine = decoded.engine.unwrap();
    assert_eq!(engine.displacement_l, Some(5.7));
    assert_eq!(engine.displacement_cc, Some(5735));
}

#[test]
fn stated_litres_beat_a_conversion() {
    let decoded = decode(
        one_schema()
            .with_pattern(1, "K2***", Element::DisplacementL, "2.0")
            .with_pattern(1, "K2***", Element::DisplacementCc, "1998"),
    );
    let engine = decoded.engine.unwrap();
    assert_eq!(engine.displacement_l, Some(2.0));
    assert_eq!(engine.displacement_cc, Some(1998));
}

#[test]
fn litres_are_rounded_to_one_decimal_place() {
    let decoded =
        decode(one_schema().with_pattern(1, "K2***", Element::DisplacementL, "2.998832712"));
    let engine = decoded.engine.unwrap();
    assert_eq!(engine.displacement_l, Some(3.0));
    assert_eq!(engine.displacement_cc, Some(2999));
    assert_eq!(engine.label.as_deref(), Some("3.0L"));
}

#[test]
fn a_small_engine_is_labelled_in_cubic_centimetres() {
    let decoded = decode(one_schema().with_pattern(1, "K2***", Element::DisplacementCc, "649"));
    let engine = decoded.engine.unwrap();
    assert_eq!(engine.displacement_l, Some(0.6));
    assert_eq!(engine.displacement_cc, Some(649));
    assert_eq!(engine.label.as_deref(), Some("649cc"));
}

#[test]
fn an_engine_too_small_for_a_tenth_of_a_litre_has_no_litre_figure() {
    for (element, value, cc) in [
        (Element::DisplacementCc, "49", 49),
        (Element::DisplacementL, "0.04", 40),
    ] {
        let decoded = decode(one_schema().with_pattern(1, "K2***", element, value));
        let engine = decoded.engine.unwrap();
        assert_eq!(engine.displacement_l, None, "{value}");
        assert_eq!(engine.displacement_cc, Some(cc), "{value}");
        assert_eq!(engine.label, Some(format!("{cc}cc")), "{value}");
    }
}

/// A 2.3 L, 140 cubic inch engine, each unit changed at the given time.
fn two_units(litres_changed: &str, cubic_inches_changed: &str) -> Engine {
    decode(
        one_schema()
            .with_pattern_changed(1, "K2***", Element::DisplacementL, "2.3", litres_changed)
            .with_pattern_changed(
                1,
                "K2***",
                Element::DisplacementCi,
                "140",
                cubic_inches_changed,
            ),
    )
    .engine
    .unwrap()
}

#[test]
fn cubic_centimetres_come_from_the_more_recently_changed_unit() {
    // NHTSA converts from whichever pattern ranks first, so 140 cubic inches
    // gives 2294 cc and 2.3 litres gives 2300.
    assert_eq!(
        two_units("2020-01-01", "2010-01-01").displacement_cc,
        Some(2300)
    );
    assert_eq!(
        two_units("2010-01-01", "2020-01-01").displacement_cc,
        Some(2294)
    );
}

#[test]
fn with_equal_dates_cubic_centimetres_come_from_cubic_inches() {
    let engine = two_units("2015-01-01", "2015-01-01");
    assert_eq!(engine.displacement_cc, Some(2294));
    assert_eq!(engine.displacement_l, Some(2.3));
}

#[test]
fn litres_come_from_the_more_recently_changed_unit_when_not_stated() {
    // 2354 cc is 2.4 L; 140 cubic inches is 2.3 L.
    let litres = |cc_changed: &str, ci_changed: &str| {
        decode(
            one_schema()
                .with_pattern_changed(1, "K2***", Element::DisplacementCc, "2354", cc_changed)
                .with_pattern_changed(1, "K2***", Element::DisplacementCi, "140", ci_changed),
        )
        .engine
        .unwrap()
        .displacement_l
    };
    assert_eq!(litres("2010-01-01", "2020-01-01"), Some(2.3));
    assert_eq!(litres("2020-01-01", "2010-01-01"), Some(2.4));
    assert_eq!(litres("2015-01-01", "2015-01-01"), Some(2.4));
}

#[test]
fn an_absurd_displacement_is_left_out() {
    for value in ["1e308", "1e300", "500"] {
        let decoded = decode(one_schema().with_pattern(1, "K2***", Element::DisplacementL, value));
        assert_eq!(decoded.engine, None, "{value:?} should give no engine");
    }
    let decoded = decode(one_schema().with_pattern(1, "K2***", Element::DisplacementCc, "9000000"));
    assert_eq!(decoded.engine, None);
}

#[test]
fn a_displacement_that_is_not_a_positive_number_is_left_out() {
    for value in ["0", "-2.0", "NaN", "inf", "two litres"] {
        let decoded = decode(one_schema().with_pattern(1, "K2***", Element::DisplacementL, value));
        assert_eq!(decoded.engine, None, "{value:?} should give no engine");
    }
}

fn kona_with_engine() -> MemoryData {
    one_schema()
        .with_pattern(1, "K2***", Element::Model, "Kona")
        .with_pattern(1, "K2***", Element::EngineModel, "G4NH")
}

#[test]
fn an_engine_model_fills_in_what_the_vin_does_not_say() {
    let decoded = decode(
        kona_with_engine()
            .with_engine_row("G4NH", Element::EngineCylinders, "4", "2020-01-01")
            .with_engine_row("G4NH", Element::FuelTypePrimary, "Gasoline", "2020-01-01"),
    );
    let engine = decoded.engine.unwrap();
    assert_eq!(engine.cylinders, Some(4));
    assert_eq!(engine.fuel.as_deref(), Some("Gasoline"));
}

#[test]
fn an_engine_model_never_replaces_a_value_from_the_vin() {
    let decoded = decode(
        kona_with_engine()
            .with_pattern(1, "K2***", Element::EngineCylinders, "6")
            .with_engine_row("G4NH", Element::EngineCylinders, "4", "2030-01-01"),
    );
    assert_eq!(decoded.engine.unwrap().cylinders, Some(6));
}

#[test]
fn the_engine_model_name_is_matched_without_regard_to_case_or_spaces() {
    let decoded = decode(
        one_schema()
            .with_pattern(1, "K2***", Element::EngineModel, " g4nh ")
            .with_engine_row("G4NH", Element::EngineCylinders, "4", "2020-01-01"),
    );
    assert_eq!(decoded.engine.unwrap().cylinders, Some(4));
}

#[test]
fn among_engine_model_rows_the_latest_change_wins() {
    let decoded = decode(
        kona_with_engine()
            .with_engine_row("G4NH", Element::EngineCylinders, "3", "2015-01-01")
            .with_engine_row("G4NH", Element::EngineCylinders, "4", "2020-01-01"),
    );
    assert_eq!(decoded.engine.unwrap().cylinders, Some(4));
}

#[test]
fn exact_cubic_centimetres_can_come_from_the_engine_model() {
    // The VIN says 2.4 L; the engine model says 2354 cc.
    let decoded = decode(
        kona_with_engine()
            .with_pattern(1, "K2***", Element::DisplacementL, "2.4")
            .with_engine_row("G4NH", Element::DisplacementCc, "2354", "2020-01-01"),
    );
    let engine = decoded.engine.unwrap();
    assert_eq!(
        (engine.displacement_l, engine.displacement_cc),
        (Some(2.4), Some(2354))
    );
}

#[test]
fn an_unknown_engine_model_changes_nothing() {
    let decoded = decode(kona_with_engine());
    assert_eq!(decoded.engine.unwrap().cylinders, None);
}

/// A 2023 Kona SE. The model's raw attribute is its vPIC id, 900.
fn kona_se() -> MemoryData {
    one_schema()
        .with_pattern_attribute(1, "K2***", Element::Model, "900", "Kona")
        .with_pattern(1, "K2***", Element::Trim, "SE")
        .with_pattern_attribute(1, "K2***", Element::DriveType, "1", "FWD/Front-Wheel Drive")
}

#[test]
fn a_spec_sheet_applies_when_all_its_keys_match() {
    let decoded = decode(
        kona_se()
            .with_spec_key("900", Some(2023), 10, Element::Trim, "SE")
            .with_spec_key("900", Some(2023), 10, Element::DriveType, "1")
            .with_spec_value(
                "900",
                Some(2023),
                10,
                Element::Abs,
                "Standard",
                "2023-01-01",
            )
            .with_spec_value(
                "900",
                Some(2023),
                10,
                Element::TransmissionStyle,
                "Automatic",
                "2023-01-01",
            ),
    );
    assert_eq!(decoded.safety.unwrap().abs.as_deref(), Some("Standard"));
    assert_eq!(decoded.transmission.as_deref(), Some("Automatic"));
}

#[test]
fn a_spec_sheet_with_one_unmatched_key_is_ignored() {
    let decoded = decode(
        kona_se()
            .with_spec_key("900", Some(2023), 10, Element::Trim, "SE")
            .with_spec_key("900", Some(2023), 10, Element::DriveType, "2")
            .with_spec_value(
                "900",
                Some(2023),
                10,
                Element::Abs,
                "Standard",
                "2023-01-01",
            ),
    );
    assert_eq!(decoded.safety, None);
}

#[test]
fn spec_keys_match_without_regard_to_case() {
    let decoded = decode(
        kona_se()
            .with_spec_key("900", Some(2023), 10, Element::Trim, "se")
            .with_spec_value(
                "900",
                Some(2023),
                10,
                Element::Abs,
                "Standard",
                "2023-01-01",
            ),
    );
    assert_eq!(decoded.safety.unwrap().abs.as_deref(), Some("Standard"));
}

#[test]
fn a_spec_sheet_with_no_keys_is_never_applied() {
    let decoded = decode(kona_se().with_spec_value(
        "900",
        Some(2023),
        10,
        Element::Abs,
        "Standard",
        "2023-01-01",
    ));
    assert_eq!(decoded.safety, None);
}

#[test]
fn a_spec_sheet_never_replaces_a_value_from_the_vin() {
    let decoded = decode(
        kona_se()
            .with_pattern(1, "K2***", Element::TransmissionStyle, "Manual/Standard")
            .with_spec_key("900", Some(2023), 10, Element::Trim, "SE")
            .with_spec_value(
                "900",
                Some(2023),
                10,
                Element::TransmissionStyle,
                "Automatic",
                "2030-01-01",
            ),
    );
    assert_eq!(decoded.transmission.as_deref(), Some("Manual/Standard"));
}

#[test]
fn among_matching_spec_sheets_the_latest_change_wins() {
    let decoded = decode(
        kona_se()
            .with_spec_key("900", Some(2023), 10, Element::Trim, "SE")
            .with_spec_value(
                "900",
                Some(2023),
                10,
                Element::TpmsType,
                "Indirect",
                "2022-01-01",
            )
            .with_spec_key("900", Some(2023), 11, Element::DriveType, "1")
            .with_spec_value(
                "900",
                Some(2023),
                11,
                Element::TpmsType,
                "Direct",
                "2023-06-01",
            ),
    );
    assert_eq!(decoded.safety.unwrap().tpms.as_deref(), Some("Direct"));
}

#[test]
fn a_spec_sheet_for_another_year_or_model_is_not_used() {
    let other_year = kona_se()
        .with_spec_key("900", Some(2021), 10, Element::Trim, "SE")
        .with_spec_value(
            "900",
            Some(2021),
            10,
            Element::Abs,
            "Standard",
            "2023-01-01",
        );
    assert_eq!(decode(other_year).safety, None);
    let other_model = kona_se()
        .with_spec_key("901", Some(2023), 10, Element::Trim, "SE")
        .with_spec_value(
            "901",
            Some(2023),
            10,
            Element::Abs,
            "Standard",
            "2023-01-01",
        );
    assert_eq!(decode(other_model).safety, None);
}

#[test]
fn a_spec_sheet_with_no_years_applies_to_every_year() {
    let decoded = decode(
        kona_se()
            .with_spec_key("900", None, 10, Element::Trim, "SE")
            .with_spec_value("900", None, 10, Element::Abs, "Standard", "2023-01-01"),
    );
    assert_eq!(decoded.safety.unwrap().abs.as_deref(), Some("Standard"));
}

#[test]
fn a_key_can_match_a_value_that_came_from_the_engine_model() {
    let decoded = decode(
        kona_se()
            .with_pattern(1, "K2***", Element::EngineModel, "G4NH")
            .with_engine_row("G4NH", Element::FuelTypePrimary, "Gasoline", "2020-01-01")
            .with_spec_key("900", Some(2023), 10, Element::FuelTypePrimary, "gasoline")
            .with_spec_value(
                "900",
                Some(2023),
                10,
                Element::Abs,
                "Standard",
                "2023-01-01",
            ),
    );
    assert_eq!(decoded.safety.unwrap().abs.as_deref(), Some("Standard"));
}

#[test]
fn without_a_model_no_spec_sheet_is_consulted() {
    let decoded = decode(
        one_schema()
            .with_pattern(1, "K2***", Element::Trim, "SE")
            .with_spec_key("900", Some(2023), 10, Element::Trim, "SE")
            .with_spec_value(
                "900",
                Some(2023),
                10,
                Element::Abs,
                "Standard",
                "2023-01-01",
            ),
    );
    assert_eq!(decoded.safety, None);
}

#[test]
fn what_the_spec_sheet_adds_counts_towards_a_heavy_vehicles_year() {
    // Both years resolve a model (99). Only the earlier one gets front air
    // bag locations (64) from its spec sheet.
    let decoded = decode(
        heavy_two_cycles()
            .with_pattern_attribute(1, "K2***", Element::Model, "901", "Newer Coach")
            .with_pattern_attribute(2, "K2***", Element::Model, "900", "Older Coach")
            .with_pattern(2, "K2***", Element::Trim, "Base")
            .with_pattern(1, "K2***", Element::Trim, "Base")
            .with_spec_key("900", None, 10, Element::Trim, "Base")
            .with_spec_value(
                "900",
                None,
                10,
                Element::AirbagsFront,
                "1st Row",
                "2000-01-01",
            ),
    );
    assert_eq!(decoded.year, Some(1993));
}

#[test]
fn what_the_engine_model_adds_counts_as_patterns_for_a_heavy_vehicles_year() {
    // Equal weight (model 99, engine model 55) and two matching patterns in
    // each year. Only the earlier year's engine model implies anything, and
    // engine configuration carries no weight.
    let decoded = decode(
        heavy_two_cycles()
            .with_pattern(1, "K2***", Element::Model, "Newer Coach")
            .with_pattern(1, "K2***", Element::EngineModel, "N14")
            .with_pattern(2, "K2***", Element::Model, "Older Coach")
            .with_pattern(2, "K2***", Element::EngineModel, "L10")
            .with_engine_row("L10", Element::EngineConfiguration, "In-Line", "2000-01-01"),
    );
    assert_eq!(decoded.year, Some(1993));
}

#[test]
fn what_the_spec_sheet_adds_does_not_count_as_patterns_for_a_heavy_vehicles_year() {
    // Equal weight and two matching patterns in each year. The earlier year's
    // sheet adds doors, which carry no weight, so the later year stands.
    let decoded = decode(
        heavy_two_cycles()
            .with_pattern_attribute(1, "K2***", Element::Model, "901", "Newer Coach")
            .with_pattern_attribute(2, "K2***", Element::Model, "900", "Older Coach")
            .with_pattern(1, "K2***", Element::Trim, "Base")
            .with_pattern(2, "K2***", Element::Trim, "Base")
            .with_spec_key("900", None, 10, Element::Trim, "Base")
            .with_spec_value("900", None, 10, Element::Doors, "2", "2000-01-01"),
    );
    assert_eq!(decoded.year, Some(2023));
}

#[test]
fn the_make_does_not_count_as_a_pattern_for_a_heavy_vehicles_year() {
    // Equal weight: model and body class (99 + 99) against model and make.
    // NHTSA takes the make from the model, not from a pattern, so each year
    // has two patterns and the later year stands.
    let decoded = decode(
        heavy_two_cycles()
            .with_pattern(1, "K2***", Element::Model, "Newer Coach")
            .with_pattern(1, "K2***", Element::BodyClass, "Bus")
            .with_pattern(2, "K2***", Element::Model, "Older Coach")
            .with_pattern(2, "K2***", Element::Make, "Heavy Make")
            .with_pattern(2, "K2***", Element::Doors, "2"),
    );
    assert_eq!(decoded.year, Some(2023));
}

#[test]
fn every_matching_note_counts_as_a_pattern_for_a_heavy_vehicles_year() {
    // Equal weight. NHTSA keeps every matching row of a note element (vPIC
    // element 114) rather than one value, so the earlier year has three
    // patterns against two.
    let decoded = decode(
        heavy_two_cycles()
            .with_pattern(1, "K2***", Element::Model, "Newer Coach")
            .with_pattern(1, "K2***", Element::Doors, "2")
            .with_pattern(2, "K2***", Element::Model, "Older Coach")
            .with_pattern(2, "K2***", Element::Other(114), "First note")
            .with_pattern(2, "K****", Element::Other(114), "Second note"),
    );
    assert_eq!(decoded.year, Some(1993));
}

#[test]
fn a_note_from_the_engine_model_counts_beside_a_note_from_the_vin() {
    // Equal weight. Each year has a model, an engine model and a third
    // value; the earlier year's engine model also brings a note on an element
    // (129, other engine information) the VIN already gave a note for.
    let decoded = decode(
        heavy_two_cycles()
            .with_pattern(1, "K2***", Element::Model, "Newer Coach")
            .with_pattern(1, "K2***", Element::EngineModel, "N14")
            .with_pattern(1, "K2***", Element::Doors, "2")
            .with_pattern(2, "K2***", Element::Model, "Older Coach")
            .with_pattern(2, "K2***", Element::EngineModel, "L10")
            .with_pattern(2, "K2***", Element::Other(129), "Note from the VIN")
            .with_engine_row(
                "L10",
                Element::Other(129),
                "Note from the engine",
                "2000-01-01",
            ),
    );
    assert_eq!(decoded.year, Some(1993));
}

#[test]
fn equipment_and_dimensions_are_reported() {
    let decoded = decode(
        one_schema()
            .with_pattern(1, "K2***", Element::Model, "Kona")
            .with_pattern(1, "K2***", Element::TractionControl, "Standard")
            .with_pattern(1, "K2***", Element::DynamicBrakeSupport, "Standard")
            .with_pattern(1, "K2***", Element::RearCrossTraffic, "Optional")
            .with_pattern(1, "K2***", Element::ParkAssist, "Optional")
            .with_pattern(1, "K2***", Element::PedestrianBraking, "Standard")
            .with_pattern(1, "K2***", Element::LaneCentering, "Standard")
            .with_pattern(1, "K2***", Element::WheelSizeFront, "18")
            .with_pattern(1, "K2***", Element::WheelSizeRear, "18")
            .with_pattern(1, "K2***", Element::Seats, "5")
            .with_pattern(1, "K2***", Element::SeatRows, "2")
            .with_pattern(
                1,
                "K2***",
                Element::Gvwr,
                "Class 1C: 4,001 - 5,000 lb (1,814 - 2,268 kg)",
            )
            .with_pattern(1, "K2***", Element::BasePrice, "24650.00"),
    );
    let json = serde_json::to_value(&decoded).unwrap();
    assert_eq!(json["safety"]["traction_control"], json!("Standard"));
    assert_eq!(json["safety"]["dynamic_brake_support"], json!("Standard"));
    assert_eq!(json["safety"]["rear_cross_traffic"], json!("Optional"));
    assert_eq!(json["safety"]["park_assist"], json!("Optional"));
    assert_eq!(json["safety"]["pedestrian_braking"], json!("Standard"));
    assert_eq!(json["safety"]["lane_centering"], json!("Standard"));
    assert_eq!(json["wheel_size_front"], json!(18));
    assert_eq!(json["wheel_size_rear"], json!(18));
    assert_eq!(json["seats"], json!(5));
    assert_eq!(json["seat_rows"], json!(2));
    assert_eq!(
        json["gvwr"],
        json!("Class 1C: 4,001 - 5,000 lb (1,814 - 2,268 kg)")
    );
    assert_eq!(json["base_price_usd"], json!(24650.0));
}

#[test]
fn a_base_price_that_is_not_a_positive_amount_is_left_out() {
    for value in ["0", "-1", "free", "NaN", "1e300"] {
        let decoded = decode(
            one_schema()
                .with_pattern(1, "K2***", Element::Model, "Kona")
                .with_pattern(1, "K2***", Element::BasePrice, value),
        );
        assert_eq!(decoded.base_price_usd, None, "{value}");
    }
}

#[test]
fn the_new_elements_have_vpic_ids() {
    for (id, element) in [
        (100, Element::TractionControl),
        (170, Element::DynamicBrakeSupport),
        (183, Element::RearCrossTraffic),
        (105, Element::ParkAssist),
        (171, Element::PedestrianBraking),
        (194, Element::LaneCentering),
        (119, Element::WheelSizeFront),
        (120, Element::WheelSizeRear),
        (33, Element::Seats),
        (61, Element::SeatRows),
        (25, Element::Gvwr),
        (136, Element::BasePrice),
    ] {
        assert_eq!(Element::from_vpic_id(id), Some(element), "{id}");
    }
    assert_eq!(Element::Gvwr.weight(), 70);
}
