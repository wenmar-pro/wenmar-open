use std::path::Path;

use open_data::shop::build::{ShopInputs, build_shop};
use open_data::shop::codes::{files, is_day, is_github_name, load, parse};
use open_data::shop::components::Components;
use rusqlite::Connection;

use crate::common::{COMPONENTS_YAML, Scratch, fixture, info, rows};

/// A curated directory whose `codes` holds the two fixture files.
fn with_fixtures(name: &str) -> Scratch {
    let scratch = Scratch::new(name);
    let directory = scratch.curated().join("codes");
    std::fs::create_dir_all(&directory).unwrap();
    for file in ["p0.yaml", "u0.yaml"] {
        std::fs::copy(fixture("codes").join(file), directory.join(file)).unwrap();
    }
    // Neither is built: one is not YAML by name, one starts with `_`.
    std::fs::write(directory.join("README.md"), "# not read").unwrap();
    std::fs::write(directory.join("_example.yaml"), "codes: [nonsense").unwrap();
    scratch
}

fn build(scratch: &Scratch) -> anyhow::Result<open_data::shop::build::ShopSummary> {
    build_shop(
        &scratch.core(),
        &scratch.out(),
        &scratch.curated(),
        &info(),
        &ShopInputs::default(),
    )
}

#[test]
fn the_records_are_built_into_the_tables() {
    let scratch = with_fixtures("codes-built");
    let summary = build(&scratch).unwrap();
    assert_eq!(
        summary.datasets.to_json(),
        r#"{"codes":{"as_of":"2026-10-12","rows":3,"licence":"mit","attribution":""}}"#
    );
    let connection = Connection::open(scratch.out()).unwrap();
    assert_eq!(
        rows(
            &connection,
            "SELECT code, system, title, meaning, common_causes, checks, related, author, reviewer, checked FROM code ORDER BY code"
        ),
        [
            r#"P0001|powertrain/fuel-air|Made-up title one|Made-up meaning one. This text is a test fixture and describes no trouble code.|["Made-up cause A","Made-up cause B"]|["Made-up check A"]|["P0002","U0001"]|test-author|test-reviewer|2026-10-10"#,
            r#"P0002|powertrain/emissions|Made-up title two|Made-up meaning two.|["Made-up cause C"]|["Made-up check B","Made-up check C"]|["P0001"]|test-author|test-reviewer|2026-10-12"#,
            r#"U0001|network|Made-up title four|Made-up meaning four.|["Made-up cause E"]|["Made-up check E"]|[]|test-author|other-reviewer|2026-10-11"#,
        ]
    );
    assert_eq!(
        rows(
            &connection,
            "SELECT code, component_id, position FROM code_component ORDER BY code, position"
        ),
        [
            "P0001|timing-belt|0",
            "P0001|engine|1",
            "P0002|exhaust-emissions|0",
            "U0001|electrical|0",
            "U0001|software|1",
        ]
    );
    // The components are there to name them, though the file holds no
    // recall and no bulletin.
    assert_eq!(
        rows(
            &connection,
            "SELECT name FROM component WHERE id = 'timing-belt'"
        ),
        ["Timing belt or chain"]
    );
}

#[test]
fn a_directory_with_no_record_adds_nothing() {
    for name in ["codes-none", "codes-empty"] {
        let scratch = Scratch::new(name);
        if name == "codes-empty" {
            let directory = scratch.curated().join("codes");
            std::fs::create_dir_all(&directory).unwrap();
            std::fs::write(directory.join("_example.yaml"), "codes: []").unwrap();
        }
        let summary = build(&scratch).unwrap();
        assert!(summary.datasets.is_empty());
        let connection = Connection::open(scratch.out()).unwrap();
        assert_eq!(
            rows(&connection, "SELECT name FROM sqlite_master ORDER BY name"),
            ["meta", "sqlite_autoindex_meta_1"]
        );
    }
}

#[test]
fn the_files_of_a_directory_are_read_by_name() {
    let scratch = with_fixtures("codes-files");
    let found = files(&scratch.curated().join("codes")).unwrap();
    let names: Vec<&str> = found.iter().map(|(name, _)| name.as_str()).collect();
    assert_eq!(names, ["p0", "u0"]);
    assert!(found[1].1.contains("U0001"));
    assert!(files(Path::new("/nonexistent/codes")).unwrap().is_empty());
}

#[test]
fn days_and_names_are_checked() {
    for day in ["2026-10-12", "2027-01-31"] {
        assert!(is_day(day), "{day}");
    }
    for day in [
        "2026-13-01",
        "2026-00-10",
        "2026-10-32",
        "2026-1-1",
        "26-10-12",
        "2026.10.12",
        "",
    ] {
        assert!(!is_day(day), "{day}");
    }
    for name in ["test-author", "a", "A1-b2"] {
        assert!(is_github_name(name), "{name}");
    }
    for name in ["", "-a", "a-", "a--b", "a b", "a_b", "@a", &"a".repeat(40)] {
        assert!(!is_github_name(name), "{name}");
    }
}

/// One record with one line replaced or added, as the text of `p0.yaml`.
fn record(change: impl Fn(&mut Vec<String>)) -> String {
    let mut lines: Vec<String> = [
        "codes:",
        "  - code: P0001",
        "    system: powertrain",
        "    title: Made-up title",
        "    meaning: Made-up meaning.",
        "    common_causes: [Made-up cause]",
        "    checks: [Made-up check]",
        "    author: test-author",
        "    reviewer: test-reviewer",
        "    checked: 2026-10-12",
    ]
    .iter()
    .map(|line| (*line).to_owned())
    .collect();
    change(&mut lines);
    lines.join("\n")
}

fn set(lines: &mut [String], field: &str, value: &str) {
    let prefix = format!("    {field}:");
    let prefix_first = format!("  - {field}:");
    for line in lines.iter_mut() {
        if line.starts_with(&prefix) {
            *line = format!("{prefix} {value}");
        } else if line.starts_with(&prefix_first) {
            *line = format!("{prefix_first} {value}");
        }
    }
}

#[test]
fn a_good_record_is_read() {
    let records = parse("p0", &record(|_| {})).unwrap();
    assert_eq!(records.len(), 1);
    assert_eq!(records[0].code, "P0001");
    assert!(records[0].components.is_empty() && records[0].related.is_empty());
}

#[test]
fn a_mistake_in_a_record_is_named() {
    let long = "x".repeat(121);
    let cases: Vec<(String, &str)> = vec![
        (
            record(|lines| set(lines, "code", "P1000")),
            "P1000 is not a generic code",
        ),
        (
            record(|lines| set(lines, "code", "B3000")),
            "B3000 is not a generic code",
        ),
        (
            record(|lines| set(lines, "code", "p0001")),
            "\"p0001\" is not a code",
        ),
        (
            record(|lines| set(lines, "code", "P2000")),
            "P2000 belongs in p2.yaml",
        ),
        (
            record(|lines| set(lines, "system", "network")),
            "P0001: the system network",
        ),
        (
            record(|lines| set(lines, "system", "powertrain/made-up")),
            "P0001: the system powertrain/made-up",
        ),
        (
            record(|lines| set(lines, "title", "' '")),
            "P0001 has no title",
        ),
        (
            record(|lines| set(lines, "title", &long)),
            "P0001: the title is longer than 120",
        ),
        (
            record(|lines| set(lines, "meaning", "''")),
            "P0001 has no meaning",
        ),
        (
            record(|lines| set(lines, "common_causes", "[]")),
            "P0001 needs 1 to 10 common_causes",
        ),
        (
            record(|lines| set(lines, "checks", "['']")),
            "P0001 has an empty entry in checks",
        ),
        (
            record(|lines| set(lines, "author", "'two words'")),
            "P0001: the author",
        ),
        (
            record(|lines| set(lines, "reviewer", "test-author")),
            "P0001: the reviewer is the author",
        ),
        (
            record(|lines| set(lines, "checked", "yesterday")),
            "P0001: checked must be a day",
        ),
        (
            record(|lines| lines.push("    related: [P0001]".to_owned())),
            "P0001 is related to itself",
        ),
        (
            record(|lines| lines.push("    related: [P1000]".to_owned())),
            "P0001: the related code P1000 is not a generic code",
        ),
        (
            record(|lines| lines.push("    colour: red".to_owned())),
            "p0.yaml",
        ),
        (
            record(|lines| lines.retain(|line| !line.contains("meaning"))),
            "p0.yaml",
        ),
        (
            format!(
                "{}\n{}",
                record(|_| {}),
                &record(|_| {})["codes:\n".len()..]
            ),
            "p0.yaml lists P0001 twice or out of order",
        ),
    ];
    for (yaml, expected) in cases {
        let error = parse("p0", &yaml).unwrap_err();
        let message = format!("{error:#}");
        assert!(message.contains(expected), "{expected}: {message}");
    }
    let error = parse("p1", &record(|_| {})).unwrap_err();
    assert!(
        format!("{error:#}").contains("p1.yaml is not a file of data/codes"),
        "{error:#}"
    );
}

#[test]
fn a_mistake_between_records_is_named() {
    let components = Components::parse(COMPONENTS_YAML).unwrap();
    let cases: [(&str, &str, &str); 2] = [
        (
            "components: [flux-capacitor]",
            "",
            "P0001: flux-capacitor is not a component of components.yaml",
        ),
        (
            "related: [P0002]",
            "",
            "P0001: the related code P0002 has no record",
        ),
    ];
    for (line, _, expected) in cases {
        let scratch = Scratch::new(&format!("codes-bad-{}", line.len()));
        let directory = scratch.curated().join("codes");
        std::fs::create_dir_all(&directory).unwrap();
        let yaml = record(|lines| lines.push(format!("    {line}")));
        std::fs::write(directory.join("p0.yaml"), yaml).unwrap();
        let error = load(&scratch.curated(), &components).unwrap_err();
        let message = format!("{error:#}");
        assert!(message.contains(expected), "{expected}: {message}");
        // The build fails the same way, and leaves no file.
        assert!(build(&scratch).is_err());
        assert!(!scratch.out().exists());
    }
}

#[test]
fn the_directory_in_the_repository_loads_and_its_example_is_a_good_file() {
    let data = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../data");
    let components = Components::load(&data).unwrap();
    // Whatever records exist are sound.
    load(&data, &components).unwrap();
    let example = std::fs::read_to_string(data.join("codes/_example.yaml")).unwrap();
    let records = parse("p0", &example).unwrap();
    assert_eq!(records.len(), 2);
    assert!(
        records
            .iter()
            .all(|record| record.title.starts_with("Made-up"))
    );
}
