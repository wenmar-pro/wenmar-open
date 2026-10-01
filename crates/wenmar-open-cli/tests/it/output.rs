use wenmar_open_cli::env::Env;

use crate::common::{self, HOSTILE, KONA};

/// The same surroundings, with a person watching.
fn terminal(fixture: &common::Fixture) -> Env {
    Env {
        stdout_terminal: true,
        ..common::env(fixture)
    }
}

#[test]
fn at_a_terminal_the_answer_is_text() {
    let fixture = common::data_dir();
    let run = common::run(&terminal(&fixture), &["vin", "decode", KONA]);
    assert_eq!(run.code, 0, "{}", run.stderr);
    assert_eq!(
        run.stdout,
        "\
KM8K2CAB4PU001140
2023 Hyundai Kona SE

Check digit   valid
Body          Sport Utility Vehicle (SUV)/Multi-Purpose Vehicle (MPV)
Transmission  Automatic
Engine        2.0L, cylinders 4, displacement cc 2000, displacement l 2.0, fuel Gasoline, model G4NH
Manufacturer  Hyundai Motor Co, South Korea, vehicle type Multipurpose Passenger Vehicle (MPV), wmi KM8
Plant         Ulsan, code U
Vehicle id    2023_hyundai_kona_se_2-0l

Safety equipment
  Abs   Standard
  Tpms  Direct
"
    );
    let run = common::run(
        &terminal(&fixture),
        &["vehicles", "search", "2019", "civic", "si"],
    );
    assert_eq!(
        run.stdout,
        "\
ID                   VEHICLE
2019_honda_civic_si  2019 Honda Civic Si, Manual, FWD, Sedan
2019_honda_civic     2019 Honda Civic, FWD
"
    );
}

#[test]
fn json_can_be_asked_for_at_a_terminal_and_is_indented_there() {
    let fixture = common::data_dir();
    let run = common::run(&terminal(&fixture), &["vehicles", "years", "--json"]);
    assert_eq!(
        run.stdout,
        "[\n  2023,\n  2022,\n  2020,\n  2019,\n  2018\n]\n"
    );
    // Before the command works as well as after it.
    let run = common::run(&terminal(&fixture), &["--json", "vehicles", "years"]);
    assert_eq!(run.json()[0], 2023);
    // Piped, it is one line.
    let run = common::run(&common::env(&fixture), &["vehicles", "years"]);
    assert_eq!(run.stdout, "[2023,2022,2020,2019,2018]\n");
}

#[test]
fn jq_filters_the_json() {
    let fixture = common::data_dir();
    let env = common::env(&fixture);
    let run = common::run(&env, &["vin", "decode", KONA, "--jq", ".make"]);
    assert_eq!(run.stdout, "Hyundai\n");
    let run = common::run(&env, &["vin", "decode", KONA, "--jq", ".catalog.entry.id"]);
    assert_eq!(run.stdout, "2023_hyundai_kona_se_2-0l\n");
    let run = common::run(&env, &["vehicles", "makes", "--jq", ".[].id"]);
    assert_eq!(run.stdout, "ford\nchevrolet\nhonda\nhyundai\n");
    let run = common::run(
        &env,
        &["vehicles", "makes", "--jq", "map(select(.id == \"kia\"))"],
    );
    assert_eq!(run.stdout, "[]\n");
    // At a terminal too: --jq means JSON.
    let run = common::run(
        &terminal(&fixture),
        &["vehicles", "years", "--jq", "length"],
    );
    assert_eq!(run.stdout, "5\n");
}

#[test]
fn a_jq_expression_that_is_wrong_is_an_error_and_prints_no_answer() {
    let fixture = common::data_dir();
    let env = common::env(&fixture);
    let run = common::run(&env, &["vehicles", "years", "--jq", ".["]);
    assert_eq!(run.code, 2);
    assert_eq!(run.stdout, "");
    assert_eq!(run.error()["error"]["code"], "usage");
    let run = common::run(&env, &["vin", "decode", KONA, "--jq", ".make.x"]);
    assert_eq!(run.code, 1);
    assert_eq!(run.stdout, "");
    assert_eq!(run.error()["error"]["code"], "jq_error");
}

#[test]
fn at_a_terminal_an_error_is_a_line_for_a_person() {
    let fixture = common::empty_dir();
    let run = common::run(&terminal(&fixture), &["vin", "decode", KONA, "--offline"]);
    assert_eq!(run.code, 11);
    assert_eq!(run.stdout, "");
    assert_eq!(
        run.stderr,
        format!(
            "error: There is no data file at {}.\nRun `wenmar-open data pull` to download the current data file.\n",
            fixture.file().display()
        )
    );
    // With --json it is the error object, there too.
    let run = common::run(
        &terminal(&fixture),
        &["vin", "decode", KONA, "--offline", "--json"],
    );
    assert_eq!(run.error()["error"]["code"], "no_data");
    // A command line that cannot be read, with --json given.
    let run = common::run(&terminal(&fixture), &["--json", "explode"]);
    assert_eq!(run.code, 2);
    assert_eq!(run.error()["error"]["code"], "usage");
    let run = common::run(&terminal(&fixture), &["explode"]);
    assert!(run.stderr.starts_with("error: "), "{}", run.stderr);
}

#[test]
fn control_characters_in_the_data_do_not_reach_a_terminal() {
    let fixture = common::data_dir();
    let run = common::run(&terminal(&fixture), &["vin", "decode", HOSTILE]);
    assert_eq!(run.code, 0, "{}", run.stderr);
    assert!(
        run.stdout.contains("Kona\u{fffd}[31m\u{fffd} Red"),
        "{:?}",
        run.stdout
    );
    assert!(!run.stdout.contains('\u{1b}'));
    assert!(!run.stdout.contains('\u{7}'));
    // JSON carries the text exactly, escaped as JSON escapes it.
    let run = common::run(&common::env(&fixture), &["vin", "decode", HOSTILE]);
    assert_eq!(run.json()["model"], "Kona\u{1b}[31m\u{7} Red");
    assert!(!run.stdout.contains('\u{1b}'));
    // --jq writes text without its quotes: at a terminal it is cleaned as
    // any other text is, and piped it is the text exactly.
    for expression in [".model", "[.model, .make] | join(\"\\r\\n\")"] {
        let args = ["vin", "decode", HOSTILE, "--jq", expression];
        let run = common::run(&terminal(&fixture), &args);
        assert_eq!(run.code, 0, "{}", run.stderr);
        assert!(
            run.stdout.starts_with("Kona\u{fffd}[31m\u{fffd} Red"),
            "{:?}",
            run.stdout
        );
        assert!(
            !run.stdout
                .chars()
                .any(|character| character.is_control() && character != '\n'),
            "{:?}",
            run.stdout
        );
        let run = common::run(&common::env(&fixture), &args);
        assert!(
            run.stdout.starts_with("Kona\u{1b}[31m\u{7} Red"),
            "{:?}",
            run.stdout
        );
    }
}

#[test]
fn the_jq_command_filters_json_from_standard_input() {
    let fixture = common::empty_dir();
    let env = common::env(&fixture);
    let run = common::run_with_input(
        &env,
        &["jq", "--", ".[].id"],
        br#"[{"id":"ford"},{"id":7}]"#,
    );
    assert_eq!(run.code, 0, "{}", run.stderr);
    assert_eq!(run.stdout, "ford\n7\n");
    // An expression may start with a dash.
    let run = common::run_with_input(&env, &["jq", "--", "-(.a)"], br#"{"a":1}"#);
    assert_eq!(run.stdout, "-1\n");
    // What it fails with is what --jq fails with.
    let run = common::run_with_input(&env, &["jq", "--", ".["], b"1");
    assert_eq!((run.code, run.stdout.as_str()), (2, ""));
    assert_eq!(run.error()["error"]["code"], "usage");
    let run = common::run_with_input(&env, &["jq", "--", ".a"], b"1");
    assert_eq!(run.code, 1);
    assert_eq!(
        run.error()["error"]["message"],
        "the --jq expression failed: cannot index 1 with \"a\""
    );
    let run = common::run_with_input(&env, &["jq", "--", "."], b"not json");
    assert_eq!(run.code, 1);
    assert_eq!(run.error()["error"]["code"], "jq_error");
    // It never waits for a person to type.
    let typing = Env {
        stdin_terminal: true,
        ..common::env(&fixture)
    };
    let run = common::run(&typing, &["jq", "--", "."]);
    assert_eq!(run.code, 2);
    assert_eq!(run.error()["error"]["code"], "usage");
    // It is --jq's own worker, and the help does not list it.
    let run = common::run(&env, &["--help"]);
    assert!(!run.stdout.contains("  jq "), "{}", run.stdout);
}
