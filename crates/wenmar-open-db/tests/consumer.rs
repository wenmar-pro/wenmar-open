//! The published surface against a real data file, not a fixture.
//!
//! The unit and integration tests build a four-make file, which cannot show
//! that the decoder handles NHTSA's real patterns or that the catalog has the
//! rows a production decode reaches. This test uses the newest data file in
//! data/build and is skipped when there is none, so a fresh checkout stays
//! green; run `mise run data` to exercise it.

use std::path::PathBuf;

use wenmar_open_db::{Db, DecodeFailure};
use wenmar_vehicles::Scope;
use wenmar_vin::{DecodeError, DecodeOptions};

/// The newest data file in data/build, or `None` when there is none.
fn data_file() -> Option<PathBuf> {
    let mut files: Vec<PathBuf> =
        std::fs::read_dir(PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../data/build"))
            .ok()?
            .filter_map(|entry| entry.ok())
            .map(|entry| entry.path())
            .filter(|path| {
                path.file_name()
                    .and_then(|name| name.to_str())
                    .is_some_and(|name| {
                        name.starts_with("wenmar-open-") && name.ends_with(".sqlite3")
                    })
            })
            .collect();
    files.sort();
    files.pop()
}

#[tokio::test]
async fn a_consumer_decodes_a_real_vin_and_reads_the_real_catalog() {
    let Some(path) = data_file() else {
        eprintln!("no data file in data/build, skipping; run `mise run data`");
        return;
    };
    let db = Db::open(&path, 2).await.unwrap();

    // A 2023 Hyundai Kona: year, make, model and trim all come from the
    // data file, and its check digit is right. Each field is an Option:
    // one that could not be determined is left out of JSON.
    let decode = db
        .decode_vin("KM8K2CAB4PU001140", DecodeOptions::default())
        .await
        .unwrap();
    assert_eq!(decode.decoded.year, Some(2023));
    assert_eq!(decode.decoded.make.as_deref(), Some("Hyundai"));
    assert_eq!(decode.decoded.model.as_deref(), Some("Kona"));
    assert!(decode.decoded.valid, "{:?}", decode.decoded);

    // A wrong check digit is a decode with a warning, not an error.
    let wrong = db
        .decode_vin("KM8K2CAB0PU001140", DecodeOptions::default())
        .await
        .unwrap();
    assert!(!wrong.decoded.valid);
    assert!(!wrong.decoded.warnings.is_empty());

    // An unknown manufacturer is the caller's to report.
    let unknown = db
        .decode_vin("ZZZK2CAB4PU001140", DecodeOptions::default())
        .await
        .unwrap_err();
    assert!(
        matches!(
            unknown,
            DecodeFailure::Decode(DecodeError::UnknownManufacturer { .. })
        ),
        "{unknown}"
    );

    // The catalog answers with real rows.
    let makes = db
        .catalog(|catalog| catalog.makes(Some(2019), Scope::Light, "", 50))
        .await
        .unwrap()
        .unwrap();
    assert!(makes.len() > 10, "{} makes for 2019", makes.len());

    let years = db
        .catalog(|catalog| catalog.years(Scope::Light, ""))
        .await
        .unwrap()
        .unwrap();
    assert!(years.contains(&2023), "{years:?}");
}
