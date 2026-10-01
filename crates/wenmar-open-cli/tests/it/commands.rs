use serde_json::{Value, json};

use crate::common::{self, KONA, KONA_BAD_CHECK, UNKNOWN};

fn names(body: &Value, field: &str) -> Vec<String> {
    body.as_array()
        .unwrap()
        .iter()
        .map(|item| item[field].as_str().unwrap().to_owned())
        .collect()
}

#[test]
fn decodes_a_vin_as_the_api_does() {
    let fixture = common::data_dir();
    let run = common::run(&common::env(&fixture), &["vin", "decode", KONA]);
    assert_eq!(run.code, 0, "{}", run.stderr);
    assert_eq!(run.stderr, "");
    // Not at a terminal: one line of JSON.
    assert_eq!(run.stdout.lines().count(), 1);
    assert_eq!(
        run.json(),
        json!({
            "vin": "KM8K2CAB4PU001140",
            "valid": true,
            "check_digit": { "valid": true, "expected": "4", "actual": "4" },
            "year": 2023,
            "make": "Hyundai",
            "model": "Kona",
            "trim": "SE",
            "body": "Sport Utility Vehicle (SUV)/Multi-Purpose Vehicle (MPV)",
            "transmission": "Automatic",
            "engine": {
                "label": "2.0L",
                "model": "G4NH",
                "displacement_l": 2.0,
                "displacement_cc": 2000,
                "cylinders": 4,
                "fuel": "Gasoline"
            },
            "safety": { "abs": "Standard", "tpms": "Direct" },
            "manufacturer": {
                "wmi": "KM8",
                "name": "Hyundai Motor Co",
                "country": "South Korea",
                "vehicle_type": "Multipurpose Passenger Vehicle (MPV)"
            },
            "plant": { "code": "U", "city": "Ulsan" },
            "warnings": [],
            "catalog": {
                "entry": {
                    "id": "2023_hyundai_kona_se_2-0l",
                    "year": 2023,
                    "make": "Hyundai",
                    "model": "Kona",
                    "submodel": "SE",
                    "engine": "2.0L",
                    "transmission": "Automatic",
                    "body": "SUV",
                    "vehicle_types": ["Multipurpose Passenger Vehicle (MPV)"],
                    "summary": "2023 Hyundai Kona SE, 2.0L, Automatic, SUV"
                },
                "vehicle_id": "2023_hyundai_kona",
                "submodel_id": "se",
                "engine_id": "2-0l"
            }
        })
    );
}

#[test]
fn spaces_dashes_and_lowercase_are_accepted() {
    let fixture = common::data_dir();
    let run = common::run(
        &common::env(&fixture),
        &["vin", "decode", "km8-k2cab4 pu001140"],
    );
    assert_eq!(run.json()["vin"], KONA);
}

#[test]
fn a_wrong_check_digit_is_not_an_error() {
    let fixture = common::data_dir();
    let run = common::run(&common::env(&fixture), &["vin", "decode", KONA_BAD_CHECK]);
    assert_eq!(run.code, 0);
    let body = run.json();
    assert_eq!(body["valid"], false);
    assert_eq!(body["model"], "Kona");
    assert_eq!(body["warnings"][0]["code"], "invalid_check_digit");
}

#[test]
fn the_year_can_be_overridden() {
    let fixture = common::data_dir();
    let env = common::env(&fixture);
    let run = common::run(&env, &["vin", "decode", KONA, "--year", "1993"]);
    assert_eq!(run.json()["year"], 1993);
    assert_eq!(run.json()["model"], "Old Model");
    let run = common::run(&env, &["vin", "decode", KONA, "--year", "1900"]);
    assert_eq!(run.code, 4);
    assert_eq!(run.error()["error"]["code"], "validation_failed");
    assert_eq!(run.error()["error"]["details"]["field"], "year");
}

#[test]
fn errors_go_to_standard_error_with_the_api_codes_and_an_exit_code() {
    let fixture = common::data_dir();
    let env = common::env(&fixture);

    let run = common::run(&env, &["vin", "decode", "KM8K2CAB4PUO01140"]);
    assert_eq!(run.code, 4);
    assert_eq!(run.stdout, "");
    assert_eq!(
        run.error(),
        json!({
            "error": {
                "code": "invalid_vin",
                "message": "a VIN uses only digits and letters other than I, O and Q",
                "details": {
                    "invalid_characters": [{ "character": "O", "position": 12 }],
                    "suggestions": ["KM8K2CAB4PU001140"]
                }
            }
        })
    );

    let run = common::run(&env, &["vin", "decode", UNKNOWN]);
    assert_eq!(run.code, 3);
    assert_eq!(
        run.error(),
        json!({ "error": { "code": "not_found", "message": "No manufacturer is registered for ZZZ.", "details": {} } })
    );

    let run = common::run(&env, &["vehicles", "entry", "2019_honda_nothing"]);
    assert_eq!(run.code, 3);
    assert_eq!(run.error()["error"]["message"], "No vehicle has that id.");

    let run = common::run(&env, &["vehicles", "makes", "--scope", "heavy"]);
    assert_eq!(run.code, 4);
    assert_eq!(run.error()["error"]["details"]["field"], "scope");
}

#[test]
fn the_cascade_offers_what_the_api_offers() {
    let fixture = common::data_dir();
    let env = common::env(&fixture);

    let run = common::run(&env, &["vehicles", "years"]);
    assert_eq!(run.json(), json!([2023, 2022, 2020, 2019, 2018]));
    let run = common::run(&env, &["vehicles", "years", "--term", "201"]);
    assert_eq!(run.json(), json!([2019, 2018]));

    let run = common::run(&env, &["vehicles", "makes", "--year", "2019"]);
    assert_eq!(
        run.json(),
        json!([
            { "id": "ford", "name": "Ford", "popular": true },
            { "id": "chevrolet", "name": "Chevrolet", "popular": true },
            { "id": "honda", "name": "Honda", "popular": true }
        ])
    );
    // Trailers are outside the default scope.
    let run = common::run(
        &env,
        &["vehicles", "makes", "--scope", "all", "--term", "ranger"],
    );
    assert_eq!(names(&run.json(), "id"), ["ranger-trailers"]);
    let run = common::run(&env, &["vehicles", "makes", "--limit", "2"]);
    assert_eq!(run.json().as_array().unwrap().len(), 2);

    let run = common::run(&env, &["vehicles", "models", "--make", "honda"]);
    assert_eq!(
        run.json(),
        json!([
            { "id": "civic", "name": "Civic", "year_from": 2018, "year_to": 2020 },
            { "id": "cr-v", "name": "CR-V", "year_from": 2019, "year_to": 2019 }
        ])
    );
    let run = common::run(&env, &["vehicles", "models", "--make", "chevy"]);
    assert_eq!(names(&run.json(), "name"), ["Silverado"]);
    // An unknown make has no models. It is not an error.
    let run = common::run(&env, &["vehicles", "models", "--make", "nobody"]);
    assert_eq!(run.code, 0);
    assert_eq!(run.json(), json!([]));

    let submodels = [
        "vehicles",
        "submodels",
        "--make",
        "honda",
        "--model",
        "civic",
        "--year",
        "2019",
    ];
    let run = common::run(&env, &submodels);
    assert_eq!(
        run.json(),
        json!([
            { "id": "lx", "name": "LX", "kind": "trim" },
            { "id": "si", "name": "Si", "kind": "trim" },
            { "id": "touring", "name": "Touring", "kind": "trim" }
        ])
    );

    let run = common::run(
        &env,
        &[
            "vehicles", "engines", "--make", "ford", "--model", "f150", "--year", "2019",
        ],
    );
    assert_eq!(
        run.json(),
        json!([
            { "id": "3-5l-turbo-v6", "label": "3.5L Turbo V6", "vin8": "4G", "preset": false },
            { "id": "5-0l-v8", "label": "5.0L V8", "vin8": "5", "preset": false }
        ])
    );
    // The Si comes only with the 1.5L Turbo.
    let run = common::run(
        &env,
        &[
            "vehicles",
            "engines",
            "--make",
            "honda",
            "--model",
            "civic",
            "--year",
            "2019",
            "--submodel",
            "si",
        ],
    );
    assert_eq!(names(&run.json(), "label"), ["1.5L Turbo"]);
}

#[test]
fn search_and_entry_give_catalog_entries() {
    let fixture = common::data_dir();
    let env = common::env(&fixture);
    let si = json!({
        "id": "2019_honda_civic_si",
        "year": 2019,
        "make": "Honda",
        "model": "Civic",
        "submodel": "Si",
        "transmission": "Manual",
        "drive": "FWD",
        "body": "Sedan",
        "vehicle_types": ["Passenger Car"],
        "summary": "2019 Honda Civic Si, Manual, FWD, Sedan"
    });
    let run = common::run(&env, &["vehicles", "search", "2019", "civic", "si"]);
    assert_eq!(run.json()[0], si);
    let run = common::run(&env, &["vehicles", "entry", "2019_honda_civic_si"]);
    assert_eq!(run.json(), si);
    // Text that names nothing finds nothing, and text that is syntax to
    // something else is only text.
    for text in [
        "zzzz",
        "%",
        "'; DROP TABLE catalog_make; --",
        "\"",
        "(",
        "AND OR NOT",
    ] {
        let run = common::run(&env, &["vehicles", "search", text]);
        assert_eq!(run.code, 0, "{text}: {}", run.stderr);
        assert_eq!(run.json(), json!([]), "{text}");
    }
    let run = common::run(&env, &["vehicles", "makes"]);
    assert_eq!(run.json().as_array().unwrap().len(), 4);
}

#[test]
fn very_long_arguments_are_refused_or_cut_and_never_repeated() {
    let fixture = common::data_dir();
    let env = common::env(&fixture);
    let long = "<script>".repeat(20_000);

    let run = common::run(&env, &["vin", "decode", &long]);
    assert_eq!(run.code, 4);
    assert_eq!(run.error()["error"]["code"], "invalid_vin");
    assert!(run.stderr.len() < 1_000, "{} bytes", run.stderr.len());
    assert!(!run.stderr.contains("script"));

    for args in [
        vec!["vehicles", "search", long.as_str()],
        vec!["vehicles", "models", "--make", long.as_str()],
        vec!["vehicles", "makes", "--term", long.as_str()],
    ] {
        let run = common::run(&env, &args);
        assert_eq!(run.code, 0, "{}", run.stderr);
        assert_eq!(run.json(), json!([]));
    }
    let run = common::run(&env, &["vehicles", "entry", &long]);
    assert_eq!(run.code, 3);
    assert!(!run.stderr.contains("script"));
}

#[test]
fn a_command_line_that_cannot_be_read_is_a_usage_error() {
    let fixture = common::data_dir();
    let env = common::env(&fixture);
    for args in [
        vec!["vin"],
        vec!["vin", "decode"],
        vec!["vehicles", "models"],
        vec!["vehicles", "makes", "--year", "soon"],
        vec!["vehicles", "makes", "--year", "70000"],
        vec!["vehicles", "search"],
        vec!["explode"],
        vec!["vin", "decode", KONA, "--nope"],
    ] {
        let run = common::run(&env, &args);
        assert_eq!(run.code, 2, "{args:?}");
        assert_eq!(run.stdout, "", "{args:?}");
        let error = run.error();
        assert_eq!(error["error"]["code"], "usage", "{args:?}");
        assert!(
            !error["error"]["message"].as_str().unwrap().is_empty(),
            "{args:?}"
        );
        // One line, with no terminal styling in it.
        assert_eq!(run.stderr.lines().count(), 1, "{args:?}");
        assert!(!run.stderr.contains('\u{1b}'), "{args:?}");
    }
}

#[test]
fn help_and_version_go_to_standard_output() {
    let env = common::env(&common::empty_dir());
    let run = common::run(&env, &["--help"]);
    assert_eq!(run.code, 0);
    assert!(run.stdout.contains("Usage: wenmar-open"), "{}", run.stdout);
    assert_eq!(run.stderr, "");
    let run = common::run(&env, &["vehicles", "--help"]);
    assert!(run.stdout.contains("submodels"), "{}", run.stdout);
    let run = common::run(&env, &["--version"]);
    assert_eq!(
        run.stdout,
        format!("wenmar-open {}\n", env!("CARGO_PKG_VERSION"))
    );
}

#[test]
fn with_no_data_file_the_error_says_how_to_get_one() {
    let fixture = common::empty_dir();
    let run = common::run(&common::env(&fixture), &["vin", "decode", KONA]);
    assert_eq!(run.code, 11);
    let error = run.error();
    assert_eq!(error["error"]["code"], "no_data");
    assert_eq!(
        error["error"]["details"]["hint"],
        "Run `wenmar-open data pull` to download the current data file."
    );
    // Looking for the data file creates nothing.
    assert!(fixture.files().is_empty());
}

#[test]
fn a_data_file_of_another_schema_version_is_refused_and_left_alone() {
    let fixture = common::old_data_dir();
    let before = std::fs::read(fixture.file()).unwrap();
    let run = common::run(&common::env(&fixture), &["vehicles", "years"]);
    assert_eq!(run.code, 11);
    let error = run.error();
    assert_eq!(error["error"]["code"], "data_invalid");
    let message = error["error"]["message"].as_str().unwrap();
    assert!(
        message.ends_with("it has schema version 2 and this build reads version 3."),
        "{message}"
    );
    assert_eq!(fixture.files(), ["wenmar-open.sqlite3"]);
    assert_eq!(std::fs::read(fixture.file()).unwrap(), before);
}

#[test]
fn a_file_that_is_not_a_data_file_is_refused() {
    let fixture = common::empty_dir();
    // Not a database at all.
    std::fs::write(fixture.file(), "<html>Not found</html>".repeat(100)).unwrap();
    let run = common::run(&common::env(&fixture), &["vehicles", "years"]);
    assert_eq!(run.code, 11);
    assert_eq!(run.error()["error"]["code"], "data_invalid");
    // A database of something else.
    std::fs::remove_file(fixture.file()).unwrap();
    let connection = rusqlite::Connection::open(fixture.file()).unwrap();
    connection
        .execute_batch("CREATE TABLE notes (body TEXT);")
        .unwrap();
    connection.close().unwrap();
    let run = common::run(&common::env(&fixture), &["vehicles", "years"]);
    assert_eq!(run.error()["error"]["code"], "data_invalid");
    // A directory where the file should be.
    std::fs::remove_file(fixture.file()).unwrap();
    std::fs::create_dir(fixture.file()).unwrap();
    let run = common::run(&common::env(&fixture), &["vehicles", "years"]);
    assert_eq!(run.error()["error"]["code"], "data_invalid");
}

#[test]
fn reading_leaves_the_data_directory_untouched() {
    let fixture = common::data_dir();
    let before = std::fs::read(fixture.file()).unwrap();
    let env = common::env(&fixture);
    common::run(&env, &["vin", "decode", KONA]);
    common::run(&env, &["vehicles", "search", "civic"]);
    // No journal, WAL or lock file is left behind.
    assert_eq!(fixture.files(), ["wenmar-open.sqlite3"]);
    assert_eq!(std::fs::read(fixture.file()).unwrap(), before);
}
