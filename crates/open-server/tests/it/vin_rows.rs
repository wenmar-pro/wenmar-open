use open_server::vin_rows;
use rusqlite::{Connection, OpenFlags};
use wenmar_vehicles::sqlite::SqliteSource;
use wenmar_vin::sqlite::SqliteData;
use wenmar_vin::{DecodeOptions, Decoder, Vin};

use crate::common;

const CURRENT_YEAR: u16 = 2026;

fn open(fixture: &common::Fixture) -> Connection {
    Connection::open_with_flags(fixture.path(), OpenFlags::SQLITE_OPEN_READ_ONLY).unwrap()
}

/// Decodes one VIN twice: straight from the file with `rusqlite`, and from
/// rows fetched first. The two must agree on every field.
fn both_ways(input: &str, model_year: Option<u16>) -> serde_json::Value {
    let fixture = common::data_file();
    let options = DecodeOptions {
        model_year,
        current_year: Some(CURRENT_YEAR),
    };
    let direct = Decoder::new(SqliteData::from_connection(open(&fixture)).unwrap())
        .decode(input, options)
        .unwrap();

    let source = SqliteSource::from_connection(open(&fixture)).unwrap();
    let vin = Vin::parse(input).unwrap();
    let rows = vin_rows::fetch(&source, &vin, model_year, CURRENT_YEAR).unwrap();
    let fetched = Decoder::new(&rows).decode(input, options).unwrap();

    assert_eq!(fetched, direct, "{input}");
    serde_json::to_value(&fetched).unwrap()
}

#[test]
fn a_light_vehicle_decodes_the_same_from_fetched_rows() {
    let decoded = both_ways(common::KONA, None);
    assert_eq!(decoded["year"], 2023);
    assert_eq!(decoded["make"], "Hyundai");
    assert_eq!(decoded["model"], "Kona");
    assert_eq!(decoded["trim"], "SE");
    // From the engine model the pattern names.
    assert_eq!(decoded["engine"]["cylinders"], 4);
    // From the specification sheet for this model and year.
    assert_eq!(decoded["safety"]["abs"], "Standard");
    assert_eq!(decoded["safety"]["tpms"], "Direct");
    assert_eq!(decoded["plant"]["city"], "Ulsan");
}

#[test]
fn a_heavy_vehicle_tries_both_cycles_from_fetched_rows() {
    let decoded = both_ways(common::COACH, None);
    assert_eq!(decoded["model"], "D-Series");
    assert_eq!(decoded["year"], 2025);
}

#[test]
fn a_low_volume_maker_is_found_by_its_six_character_code() {
    let decoded = both_ways(common::TRAILER, None);
    assert_eq!(decoded["manufacturer"]["wmi"], "1A9881");
    assert_eq!(decoded["model"], "Tilt Deck");
}

#[test]
fn a_year_override_is_fetched_for_that_year() {
    let decoded = both_ways(common::KONA, Some(1993));
    assert_eq!(decoded["year"], 1993);
    assert_eq!(decoded["model"], "Old Model");
    let none = both_ways(common::KONA, Some(2005));
    assert!(none.get("model").is_none());
}

#[test]
fn a_wrong_check_digit_still_decodes() {
    let decoded = both_ways(common::KONA_BAD_CHECK, None);
    assert_eq!(decoded["valid"], false);
    assert_eq!(decoded["model"], "Kona");
}

#[test]
fn an_unknown_manufacturer_fetches_nothing_and_the_decoder_says_so() {
    let fixture = common::data_file();
    let source = SqliteSource::from_connection(open(&fixture)).unwrap();
    let vin = Vin::parse(common::UNKNOWN).unwrap();
    let rows = vin_rows::fetch(&source, &vin, None, CURRENT_YEAR).unwrap();
    let error = Decoder::new(&rows)
        .decode(common::UNKNOWN, DecodeOptions::default())
        .unwrap_err();
    assert!(matches!(
        error,
        wenmar_vin::DecodeError::UnknownManufacturer { .. }
    ));
}
