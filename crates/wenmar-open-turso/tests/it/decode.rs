//! Decoding a VIN and reading the catalog, from async code.

use std::sync::Arc;

use wenmar_open_turso::{Db, DecodeFailure, LONGEST_INPUT};
use wenmar_vehicles::Scope;
use wenmar_vin::{DecodeError, DecodeOptions};

use crate::common;

/// The year is fixed, so the tests give the same answers every year.
const OPTIONS: DecodeOptions = DecodeOptions {
    model_year: None,
    current_year: Some(2026),
};

/// The fixture must outlive the handle: dropping it removes the file.
async fn open() -> (common::Fixture, Db) {
    let fixture = common::data_file();
    let db = Db::open(&fixture.path(), 2).await.unwrap();
    (fixture, db)
}

#[tokio::test]
async fn decodes_a_vin_with_the_catalog_entry_it_reaches() {
    let (_fixture, db) = open().await;
    let decode = db.decode_vin(common::KONA, OPTIONS).await.unwrap();
    assert!(decode.decoded.valid);
    assert_eq!(decode.decoded.year, Some(2023));
    assert_eq!(decode.decoded.make.as_deref(), Some("Hyundai"));
    assert_eq!(decode.decoded.model.as_deref(), Some("Kona"));
    assert_eq!(decode.decoded.trim.as_deref(), Some("SE"));
    let catalog = decode.catalog.expect("the catalog has a 2023 Kona");
    assert_eq!(catalog.vehicle_id, "2023_hyundai_kona");
    assert_eq!(catalog.submodel_id.as_deref(), Some("se"));
    assert_eq!(catalog.entry.id, "2023_hyundai_kona_se_2-0l");
}

#[tokio::test]
async fn a_decode_serializes_as_the_api_sends_it() {
    let (_fixture, db) = open().await;
    let decode = db.decode_vin(common::KONA, OPTIONS).await.unwrap();
    let json = serde_json::to_value(&decode).unwrap();
    assert_eq!(json["vin"], common::KONA);
    assert_eq!(json["year"], 2023);
    assert_eq!(json["engine"]["label"], "2.0L");
    assert_eq!(json["safety"]["abs"], "Standard");
    assert_eq!(json["catalog"]["vehicle_id"], "2023_hyundai_kona");
    // The decoder's fields are at the top, not under a key of their own,
    // and a field with no value is left out.
    assert!(json.get("decoded").is_none());
    assert!(json.get("series").is_none());
}

#[tokio::test]
async fn spaces_dashes_and_lowercase_are_accepted() {
    let (_fixture, db) = open().await;
    let decode = db
        .decode_vin(" km8k2-cab4 pu001140 ", OPTIONS)
        .await
        .unwrap();
    assert_eq!(decode.decoded.vin, common::KONA);
}

#[tokio::test]
async fn a_wrong_check_digit_still_decodes() {
    let (_fixture, db) = open().await;
    let decode = db
        .decode_vin(common::KONA_BAD_CHECK, OPTIONS)
        .await
        .unwrap();
    assert!(!decode.decoded.valid);
    assert_eq!(decode.decoded.model.as_deref(), Some("Kona"));
}

#[tokio::test]
async fn a_year_override_is_used_for_the_rows_and_for_the_decode() {
    let (_fixture, db) = open().await;
    let options = DecodeOptions {
        model_year: Some(1993),
        current_year: Some(2026),
    };
    let decode = db.decode_vin(common::KONA, options).await.unwrap();
    assert_eq!(decode.decoded.year, Some(1993));
    assert_eq!(decode.decoded.model.as_deref(), Some("Old Model"));
}

#[tokio::test]
async fn a_malformed_vin_fails_with_suggestions() {
    let (_fixture, db) = open().await;
    // A letter O where a zero belongs.
    match db.decode_vin("KM8K2CAB4PUO01140", OPTIONS).await {
        Err(DecodeFailure::Decode(DecodeError::InvalidVin { suggestions, .. })) => {
            assert_eq!(suggestions, ["KM8K2CAB4PU001140"]);
        }
        other => panic!("{other:?}"),
    }
}

#[tokio::test]
async fn an_unknown_manufacturer_is_its_own_failure() {
    let (_fixture, db) = open().await;
    match db.decode_vin(common::UNKNOWN, OPTIONS).await {
        Err(DecodeFailure::Decode(DecodeError::UnknownManufacturer { wmi })) => {
            assert_eq!(wmi, "ZZZ");
        }
        other => panic!("{other:?}"),
    }
}

#[tokio::test]
async fn text_far_longer_than_a_vin_is_refused_unread() {
    let (_fixture, db) = open().await;
    let over = "K".repeat(LONGEST_INPUT + 1);
    assert!(matches!(
        db.decode_vin(&over, OPTIONS).await,
        Err(DecodeFailure::TooLong)
    ));
    // At the limit the text is read, and is simply not a VIN.
    let at = "K".repeat(LONGEST_INPUT);
    assert!(matches!(
        db.decode_vin(&at, OPTIONS).await,
        Err(DecodeFailure::Decode(DecodeError::InvalidVin { .. }))
    ));
}

#[tokio::test]
async fn the_catalog_is_read_inside_a_closure() {
    let (_fixture, db) = open().await;
    let makes = db
        .catalog(|catalog| catalog.makes(Some(2019), Scope::Light, "", 10))
        .await
        .unwrap()
        .unwrap();
    assert!(makes.iter().any(|make| make.name == "Honda"), "{makes:?}");
    // Several questions in one closure use one connection.
    let (models, found) = db
        .catalog(|catalog| {
            let models = catalog.models("honda", Some(2019), Scope::Light, "", 10);
            let found = catalog.search("2019 civic si", Scope::Light, 5);
            (models, found)
        })
        .await
        .unwrap();
    assert!(models.unwrap().iter().any(|model| model.name == "Civic"));
    let found = found.unwrap();
    let best = found.first().expect("the search finds the Civic");
    assert!(best.id.starts_with("2019_honda_civic"), "{}", best.id);
}

#[tokio::test]
async fn many_decodes_at_once_all_finish() {
    let (_fixture, db) = open().await;
    let db = Arc::new(db);
    let mut tasks = Vec::new();
    for _ in 0..16 {
        let db = Arc::clone(&db);
        tasks.push(tokio::spawn(async move {
            db.decode_vin(common::KONA, OPTIONS).await.unwrap()
        }));
    }
    for task in tasks {
        assert_eq!(task.await.unwrap().decoded.year, Some(2023));
    }
}
