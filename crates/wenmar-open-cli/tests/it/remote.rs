use serde_json::{Value, json};
use wenmar_open_cli::env::Env;

use crate::common::{self, KONA, UNKNOWN};
use crate::server::{self, Handler, Reply, Seen};

fn kona() -> Value {
    json!({
        "vin": KONA,
        "valid": true,
        "check_digit": { "valid": true, "expected": "4", "actual": "4" },
        "year": 2023,
        "make": "Hyundai",
        "model": "Kona",
        "manufacturer": { "wmi": "KM8", "name": "Hyundai Motor Co" },
        "plant": { "code": "U" },
        "warnings": []
    })
}

/// Answers the way the hosted API does for the handful of requests the
/// tests make.
fn api(request: &Seen) -> Reply {
    let (path, _) = request
        .target
        .split_once('?')
        .unwrap_or((&request.target, ""));
    match (request.method.as_str(), path) {
        ("GET", "/v1/meta") => Reply::json(
            200,
            &json!({ "data_version": "2026.10", "vpic_release": "vPICList_lite_2026_10", "built_at": "2026-11-01 04:00:00", "server_version": "0.1.0" }),
        ),
        ("GET", "/v1/vin/KM8K2CAB4PU001140") => Reply::json(200, &kona()),
        ("GET", "/v1/vin/ZZZK2CAB4PU001140") => Reply::json(
            404,
            &json!({ "error": { "code": "not_found", "message": "No manufacturer is registered for ZZZ.", "details": {} } }),
        ),
        ("POST", "/v1/vin/batch") => {
            let body: Value = serde_json::from_str(&request.body).unwrap();
            let items: Vec<Value> = body["vins"]
                .as_array()
                .unwrap()
                .iter()
                .map(|vin| {
                    if vin == KONA {
                        kona()
                    } else {
                        json!({ "error": { "code": "not_found", "message": "No manufacturer is registered for ZZZ.", "details": {} } })
                    }
                })
                .collect();
            Reply::json(200, &Value::Array(items))
        }
        ("GET", "/v1/vehicles/years") => Reply::json(200, &json!([2027, 2026])),
        ("GET", "/v1/vehicles/makes") => Reply::json(
            200,
            &json!([{ "id": "ford", "name": "Ford", "popular": true }]),
        ),
        ("GET", "/v1/vehicles/search") => Reply::json(
            200,
            &json!([{ "id": "2019_honda_civic_si", "summary": "2019 Honda Civic Si" }]),
        ),
        ("GET", "/v1/vehicles/2019_honda_civic_si") => Reply::json(
            200,
            &json!({ "id": "2019_honda_civic_si", "summary": "2019 Honda Civic Si" }),
        ),
        ("GET", "/v1/vehicles/models") => Reply::json(
            429,
            &json!({ "error": { "code": "rate_limited", "message": "Too many requests from this address. The limit is per minute.", "details": { "retry_after": 17 } } }),
        ),
        _ => Reply::json(
            404,
            &json!({ "error": { "code": "not_found", "message": "There is nothing at this address.", "details": {} } }),
        ),
    }
}

/// Surroundings with an empty data directory and the API at `url`.
fn online(fixture: &common::Fixture, url: &str) -> Env {
    Env {
        api: Some(url.to_owned()),
        ..common::env(fixture)
    }
}

#[test]
fn with_no_data_file_the_answer_comes_from_the_api() {
    let server = server::serve(api);
    let fixture = common::empty_dir();
    let env = online(&fixture, server.url());

    let run = common::run(&env, &["vin", "decode", "km8k2cab4pu001140"]);
    assert_eq!(run.code, 0, "{}", run.stderr);
    assert_eq!(run.json(), kona());
    let run = common::run(&env, &["vin", "decode", KONA, "--year", "2023"]);
    assert_eq!(run.code, 0, "{}", run.stderr);
    let run = common::run(&env, &["vehicles", "years"]);
    assert_eq!(run.json(), json!([2027, 2026]));
    let run = common::run(&env, &["vehicles", "entry", "2019_honda_civic_si"]);
    assert_eq!(run.json()["id"], "2019_honda_civic_si");
    assert_eq!(
        server.targets(),
        [
            "/v1/vin/KM8K2CAB4PU001140",
            "/v1/vin/KM8K2CAB4PU001140?year=2023",
            "/v1/vehicles/years",
            "/v1/vehicles/2019_honda_civic_si"
        ]
    );
    // Nothing was written while asking.
    assert!(fixture.files().is_empty());
}

#[test]
fn parameters_are_sent_as_the_api_names_them_and_escaped() {
    let server = server::serve(api);
    let fixture = common::empty_dir();
    let env = online(&fixture, server.url());
    common::run(
        &env,
        &[
            "vehicles", "makes", "--year", "2019", "--scope", "all", "--term", "a&b=c d",
            "--limit", "7",
        ],
    );
    common::run(&env, &["vehicles", "search", "2019", "civic", "si?", "#x"]);
    assert_eq!(
        server.targets(),
        [
            "/v1/vehicles/makes?year=2019&scope=all&term=a%26b%3Dc%20d&limit=7",
            "/v1/vehicles/search?q=2019%20civic%20si%3F%20%23x&limit=10"
        ]
    );
}

#[test]
fn a_request_says_what_tool_made_it_and_nothing_else() {
    let server = server::serve(api);
    let fixture = common::empty_dir();
    common::run(&online(&fixture, server.url()), &["vehicles", "years"]);
    let seen = server.seen();
    assert_eq!(
        seen[0].header("user-agent"),
        Some(format!("wenmar-open/{}", env!("CARGO_PKG_VERSION")).as_str())
    );
    let mut names: Vec<&str> = seen[0]
        .headers
        .iter()
        .map(|(name, _)| name.as_str())
        // Present when another crate in the workspace turns on ureq's gzip
        // feature; it names a content coding, not the person or the machine.
        .filter(|name| *name != "accept-encoding")
        .collect();
    names.sort_unstable();
    // No cookie, no key, no identifier.
    assert_eq!(names, ["accept", "host", "user-agent"]);
}

#[test]
fn an_error_from_the_api_keeps_its_code_and_details() {
    let server = server::serve(api);
    let fixture = common::empty_dir();
    let env = online(&fixture, server.url());

    let run = common::run(&env, &["vin", "decode", UNKNOWN]);
    assert_eq!(run.code, 3);
    assert_eq!(
        run.error(),
        json!({ "error": { "code": "not_found", "message": "No manufacturer is registered for ZZZ.", "details": {} } })
    );
    let run = common::run(&env, &["vehicles", "models", "--make", "honda"]);
    assert_eq!(run.code, 5);
    assert_eq!(run.error()["error"]["code"], "rate_limited");
    assert_eq!(run.error()["error"]["details"]["retry_after"], 17);
}

#[test]
fn what_the_api_would_refuse_is_refused_without_a_request() {
    let server = server::serve(api);
    let fixture = common::empty_dir();
    let env = online(&fixture, server.url());
    let long = "A".repeat(100_000);
    for (args, code) in [
        (vec!["vin", "decode", "nope"], "invalid_vin"),
        (vec!["vin", "decode", "../../etc/passwd"], "invalid_vin"),
        (vec!["vin", "decode", long.as_str()], "invalid_vin"),
        (
            vec!["vin", "decode", KONA, "--year", "1900"],
            "validation_failed",
        ),
        (vec!["vehicles", "entry", "../meta"], "not_found"),
        (
            vec!["vehicles", "entry", "2019_honda_civic?x=1"],
            "not_found",
        ),
        (vec!["vehicles", "entry", long.as_str()], "not_found"),
        (
            vec!["vehicles", "makes", "--scope", "heavy"],
            "validation_failed",
        ),
    ] {
        let run = common::run(&env, &args);
        assert_eq!(run.error()["error"]["code"], code, "{:.40?}", args);
    }
    assert!(server.seen().is_empty(), "{:?}", server.targets());
}

#[test]
fn an_api_that_is_down_is_a_network_error() {
    let fixture = common::empty_dir();
    let env = online(&fixture, &server::nothing_listening());
    let run = common::run(&env, &["vin", "decode", KONA]);
    assert_eq!(run.code, 10);
    assert_eq!(run.stdout, "");
    let error = run.error();
    assert_eq!(error["error"]["code"], "network");
    assert_eq!(
        error["error"]["details"]["hint"],
        "Check the connection, or run `wenmar-open data pull` once to work without one."
    );
}

#[test]
fn an_answer_that_is_not_the_apis_is_a_bad_response() {
    let page = "<html><body><h1>Sign in to the hotel Wi-Fi</h1></body></html>";
    let cases: Vec<Handler> = vec![
        // A captive portal, or a proxy's error page.
        Box::new(move |_| Reply::html(200, page)),
        Box::new(move |_| Reply::html(502, page)),
        Box::new(|_| Reply::html(200, "")),
        // JSON, but not this API's.
        Box::new(|_| Reply::json(200, &json!({ "login": "required" }))),
        Box::new(|_| Reply::json(500, &json!({ "message": "oops" }))),
        Box::new(|_| Reply::json(200, &json!(null))),
        // An answer far larger than any the API gives.
        Box::new(|_| Reply::html(200, &"[".repeat(5 * 1024 * 1024))),
    ];
    for (index, handler) in cases.into_iter().enumerate() {
        let server = server::serve(handler);
        let fixture = common::empty_dir();
        let env = online(&fixture, server.url());
        for args in [
            vec!["vin", "decode", KONA],
            vec!["vehicles", "makes"],
            vec!["vehicles", "entry", "2019_honda_civic_si"],
        ] {
            let run = common::run(&env, &args);
            assert_eq!(run.code, 6, "case {index} {args:?}: {}", run.stderr);
            assert_eq!(run.stdout, "", "case {index}");
            let error = run.error();
            assert_eq!(error["error"]["code"], "bad_response", "case {index}");
            // The page itself is not repeated.
            assert!(!run.stderr.contains("hotel"), "case {index}");
            assert!(run.stderr.len() < 1_000, "case {index}");
        }
    }
}

#[test]
fn the_data_file_is_used_when_there_is_one_and_the_flags_force_either() {
    let server = server::serve(api);
    let fixture = common::data_dir();
    let env = online(&fixture, server.url());

    // The data file answers, and the API is not asked.
    let run = common::run(&env, &["vehicles", "years"]);
    assert_eq!(run.json(), json!([2023, 2022, 2020, 2019, 2018]));
    assert!(server.seen().is_empty());

    let run = common::run(&env, &["vehicles", "years", "--online"]);
    assert_eq!(run.json(), json!([2027, 2026]));
    assert_eq!(server.targets(), ["/v1/vehicles/years"]);

    let run = common::run(&env, &["--offline", "vehicles", "years"]);
    assert_eq!(run.json()[0], 2023);
    assert_eq!(server.seen().len(), 1);

    // --offline with no data file is an error, and asks nobody.
    let empty = common::empty_dir();
    let run = common::run(
        &online(&empty, server.url()),
        &["vehicles", "years", "--offline"],
    );
    assert_eq!(run.code, 11);
    assert_eq!(run.error()["error"]["code"], "no_data");
    assert_eq!(server.seen().len(), 1);

    let run = common::run(&env, &["vehicles", "years", "--offline", "--online"]);
    assert_eq!(run.code, 2);
    assert_eq!(run.error()["error"]["code"], "usage");
}

#[test]
fn a_data_file_that_cannot_be_used_gives_way_to_the_api_and_says_so() {
    let server = server::serve(api);
    let fixture = common::old_data_dir();
    let env = online(&fixture, server.url());
    let run = common::run(&env, &["vehicles", "years"]);
    assert_eq!(run.code, 0);
    assert_eq!(run.json(), json!([2027, 2026]));
    // Piped: standard error carries JSON only, and only on failure.
    assert_eq!(run.stderr, "");

    let terminal = Env {
        stdout_terminal: true,
        ..env
    };
    let run = common::run(&terminal, &["vehicles", "years"]);
    assert_eq!(run.stdout, "2027\n2026\n");
    assert_eq!(
        run.stderr,
        format!(
            "note: The data file at {} cannot be used: it has schema version 2 and this build reads version 3. Using the API at {}. Run `wenmar-open data pull` to replace it.\n",
            fixture.file().display(),
            server.url()
        )
    );
    // Offline, the same file is an error.
    let run = common::run(&terminal, &["vehicles", "years", "--offline", "--json"]);
    assert_eq!(run.error()["error"]["code"], "data_invalid");
}

#[test]
fn with_no_home_directory_the_api_is_used() {
    let server = server::serve(api);
    let env = Env {
        api: Some(server.url().to_owned()),
        ..Env::default()
    };
    let run = common::run(&env, &["vehicles", "years"]);
    assert_eq!(run.json(), json!([2027, 2026]));
    let run = common::run(&env, &["vehicles", "years", "--offline"]);
    assert_eq!(run.code, 11);
    assert_eq!(
        run.error()["error"]["details"]["hint"],
        "Set WENMAR_OPEN_DATA_DIR to a directory, or pass --data-dir."
    );
}

#[test]
fn the_api_address_is_a_flag_or_a_variable_and_must_be_http() {
    let server = server::serve(api);
    let fixture = common::empty_dir();
    // The flag wins over the variable.
    let env = online(&fixture, &server::nothing_listening());
    let run = common::run(&env, &["vehicles", "years", "--api", server.url()]);
    assert_eq!(run.code, 0, "{}", run.stderr);
    let trailing = format!("{}/", server.url());
    let run = common::run(&env, &["--api", &trailing, "vehicles", "years"]);
    assert_eq!(run.code, 0, "{}", run.stderr);

    for address in ["open.wenmarpro.com", "file:///etc/passwd", "ftp://x.test"] {
        let run = common::run(&env, &["vehicles", "years", "--api", address]);
        assert_eq!(run.code, 2, "{address}");
        assert_eq!(run.error()["error"]["code"], "usage", "{address}");
    }
}
