use serde_json::json;
use wenmar_vin::{
    DataError, DecodeError, DecodeOptions, Decoder, Element, Manufacturer, MemoryData, Pattern,
    VinData, VinError, WarningCode,
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
fn the_primary_schema_beats_a_more_specific_pattern_elsewhere() {
    let decoded = Decoder::new(data()).decode(KONA, options()).unwrap();
    assert_eq!(decoded.drivetrain.as_deref(), Some("FWD"));
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
        })
        .with_manufacturer(Manufacturer {
            wmi: "1Z9456".to_owned(),
            name: "Small Builder".to_owned(),
            make: Some("Small".to_owned()),
            country: None,
            vehicle_type: None,
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
        fn schemas(&self, _wmi: &str, _year: u16) -> Result<Vec<i64>, DataError> {
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
    Manufacturer {
        wmi: "KM8".to_owned(),
        name: "Maker".to_owned(),
        make: None,
        country: None,
        vehicle_type: None,
    }
}

/// One manufacturer with a single schema current from 2022.
fn one_schema() -> MemoryData {
    MemoryData::new()
        .with_manufacturer(maker())
        .with_schema("KM8", 1, 2022, None)
}

/// One manufacturer with two schemas current from 2022.
fn two_schemas() -> MemoryData {
    one_schema().with_schema("KM8", 2, 2022, None)
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
    });
    let decoded = decode(data);
    assert_eq!(decoded.make, None);
    assert_eq!(decoded.manufacturer.country, None);
    assert_eq!(decoded.manufacturer.vehicle_type, None);
    assert_eq!(decoded.manufacturer.name, "Hyundai Motor Co");
}

#[test]
fn the_other_cycle_is_used_when_the_likeliest_year_matches_nothing() {
    let decoded = decode(
        two_cycles()
            .with_pattern(1, "Z9***", Element::Model, "Newer Model")
            .with_pattern(2, "K2***", Element::Model, "Old Model"),
    );
    assert_eq!(decoded.year, Some(1993));
    assert_eq!(decoded.model.as_deref(), Some("Old Model"));
    assert_eq!(codes(&decoded), Vec::new());
}

#[test]
fn a_year_with_a_model_beats_a_year_with_only_other_details() {
    let decoded = decode(
        two_cycles()
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
            .with_pattern(1, "*2C**", Element::Make, "Lexus")
            .with_pattern(1, "K2***", Element::Make, "Toyota"),
    );
    assert_eq!(decoded.model.as_deref(), Some("Camry"));
    assert_eq!(decoded.make.as_deref(), Some("Toyota"));
}

#[test]
fn the_more_specific_pattern_wins_within_a_schema() {
    let decoded = decode(
        one_schema()
            .with_pattern(1, "K2***", Element::Model, "Kona")
            .with_pattern(1, "K****", Element::Trim, "Loose")
            .with_pattern(1, "K2CA*", Element::Trim, "Exact")
            .with_pattern(1, "K2***", Element::Trim, "Middling"),
    );
    assert_eq!(decoded.trim.as_deref(), Some("Exact"));
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
fn the_primary_schema_is_the_one_with_the_most_specific_model() {
    let decoded = decode(
        two_schemas()
            .with_pattern(1, "K****", Element::Model, "Loose Model")
            .with_pattern(1, "K2CAB", Element::Trim, "Trim From The Loose Schema")
            .with_pattern(1, "K****", Element::Doors, "2")
            .with_pattern(1, "K****", Element::BodyClass, "Coupe")
            .with_pattern(2, "K2CAB", Element::Model, "Exact Model")
            .with_pattern(2, "*****", Element::Trim, "Trim From The Exact Schema"),
    );
    assert_eq!(decoded.model.as_deref(), Some("Exact Model"));
    assert_eq!(decoded.trim.as_deref(), Some("Trim From The Exact Schema"));
}

#[test]
fn equally_specific_models_are_settled_by_how_much_else_matches() {
    let decoded = decode(
        two_schemas()
            .with_pattern(1, "K2***", Element::Model, "Sparse")
            .with_pattern(2, "K2***", Element::Model, "Rich")
            .with_pattern(2, "K2***", Element::Trim, "SE")
            .with_pattern(2, "K2***", Element::Doors, "4"),
    );
    assert_eq!(decoded.model.as_deref(), Some("Rich"));
}

#[test]
fn equal_schemas_are_settled_by_the_lowest_schema_id() {
    let decoded = decode(
        two_schemas()
            .with_pattern(2, "K2***", Element::Model, "Second Schema")
            .with_pattern(1, "K2***", Element::Model, "First Schema"),
    );
    assert_eq!(decoded.model.as_deref(), Some("First Schema"));
}

#[test]
fn without_a_model_the_schema_with_the_most_matches_is_primary() {
    let decoded = decode(
        two_schemas()
            .with_pattern(1, "K2CAB", Element::Trim, "From The Quiet Schema")
            .with_pattern(2, "K****", Element::Trim, "From The Busy Schema")
            .with_pattern(2, "K****", Element::Doors, "4")
            .with_pattern(2, "K****", Element::BodyClass, "SUV"),
    );
    assert_eq!(decoded.trim.as_deref(), Some("From The Busy Schema"));
    assert_eq!(codes(&decoded), vec![WarningCode::ModelUnresolved]);
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
    fn schemas(&self, wmi: &str, year: u16) -> Result<Vec<i64>, DataError> {
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
