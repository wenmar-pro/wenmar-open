use std::path::Path;

use open_data::shop::build::{ShopInputs, build_shop};
use open_data::shop::components::Components;
use open_data::shop::jobs::{Jobs, Status};
use rusqlite::Connection;

use crate::common::{COMPONENTS_YAML, Scratch, info, rows};

/// A reviewed list of three made-up jobs.
const REVIEWED: &str = "
status: reviewed
updated: 2026-10-11
jobs:
  - id: test-job-a
    name: ' Test  job A '
    system: engine
    includes: ['made-up  step']
  - id: test-job-b
    name: Test job B
    system: engine
    components: [timing-belt, engine]
    includes: [made-up step one, made-up step two]
    excludes: [made-up exclusion]
    related: [test-job-a]
  - id: test-job-c
    name: Test job C
    system: electrical
    components: [software]
    includes: [made-up step]
    related: [test-job-b, test-job-a]
";

fn components() -> Components {
    Components::parse(COMPONENTS_YAML).unwrap()
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
fn a_reviewed_list_is_built_into_the_tables() {
    let scratch = Scratch::new("jobs-built");
    std::fs::write(scratch.curated().join("jobs.yaml"), REVIEWED).unwrap();
    let summary = build(&scratch).unwrap();
    assert_eq!(
        summary.datasets.to_json(),
        r#"{"jobs":{"as_of":"2026-10-11","rows":3,"licence":"mit","attribution":""}}"#
    );
    let connection = Connection::open(scratch.out()).unwrap();
    assert_eq!(
        rows(
            &connection,
            "SELECT id, name, system, includes, excludes, related, position FROM job ORDER BY position"
        ),
        [
            r#"test-job-a|Test job A|engine|["made-up step"]|[]|[]|0"#,
            r#"test-job-b|Test job B|engine|["made-up step one","made-up step two"]|["made-up exclusion"]|["test-job-a"]|1"#,
            r#"test-job-c|Test job C|electrical|["made-up step"]|[]|["test-job-b","test-job-a"]|2"#,
        ]
    );
    assert_eq!(
        rows(
            &connection,
            "SELECT job_id, component_id, position FROM job_component ORDER BY job_id, position"
        ),
        [
            "test-job-b|timing-belt|0",
            "test-job-b|engine|1",
            "test-job-c|software|0",
        ]
    );
    // The components are there to name them.
    assert_eq!(
        rows(
            &connection,
            "SELECT name FROM component WHERE id = 'engine'"
        ),
        ["Engine"]
    );
}

#[test]
fn a_draft_is_checked_and_not_built() {
    let scratch = Scratch::new("jobs-draft");
    let draft = REVIEWED
        .replace("status: reviewed", "status: draft")
        .replace("    includes: ['made-up  step']\n", "");
    std::fs::write(scratch.curated().join("jobs.yaml"), &draft).unwrap();
    let jobs = Jobs::parse(&draft, &components()).unwrap();
    assert_eq!(jobs.status, Status::Draft);
    assert!(jobs.list[0].includes.is_empty());
    let summary = build(&scratch).unwrap();
    assert!(summary.datasets.is_empty());
    let connection = Connection::open(scratch.out()).unwrap();
    assert_eq!(
        rows(&connection, "SELECT name FROM sqlite_master ORDER BY name"),
        ["meta", "sqlite_autoindex_meta_1"]
    );
    // A mistake in a draft still stops the build.
    let scratch = Scratch::new("jobs-draft-bad");
    std::fs::write(
        scratch.curated().join("jobs.yaml"),
        draft.replace("system: electrical", "system: nonsense"),
    )
    .unwrap();
    assert!(build(&scratch).is_err());
    assert!(!scratch.out().exists());
}

#[test]
fn no_file_adds_nothing() {
    let scratch = Scratch::new("jobs-none");
    assert!(build(&scratch).unwrap().datasets.is_empty());
    assert!(
        Jobs::load(&scratch.curated(), &components())
            .unwrap()
            .is_none()
    );
}

#[test]
fn a_mistake_in_the_file_is_named() {
    let cases = [
        (
            "status: reviewed",
            "status: finished",
            "status must be draft or reviewed",
        ),
        (
            "updated: 2026-10-11",
            "updated: yesterday",
            "updated must be a day",
        ),
        (
            "id: test-job-a",
            "id: Test Job",
            "\"Test Job\" is not an id",
        ),
        ("id: test-job-b", "id: test-job-a", "lists test-job-a twice"),
        ("name: Test job B", "name: ' '", "test-job-b has no name"),
        (
            "name: Test job C",
            "name: xxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxx",
            "test-job-c: the name is longer than 80",
        ),
        (
            "components: [software]",
            "components: [flux-capacitor]",
            "test-job-c: flux-capacitor is not a component",
        ),
        (
            "components: [software]",
            "components: [software, software]",
            "test-job-c names the component software twice",
        ),
        (
            "excludes: [made-up exclusion]",
            "excludes: ['']",
            "test-job-b has an empty entry in excludes",
        ),
        (
            "related: [test-job-a]\n",
            "related: [test-job-z]\n",
            "test-job-b: the related job test-job-z is not in the list",
        ),
        (
            "related: [test-job-a]\n",
            "related: [test-job-b]\n",
            "test-job-b is related to itself",
        ),
        (
            "related: [test-job-b, test-job-a]",
            "related: [test-job-a, test-job-a]",
            "test-job-c lists the related job test-job-a twice",
        ),
        (
            "    includes: ['made-up  step']\n",
            "",
            "test-job-a has no includes; a reviewed list says what every job covers",
        ),
        (
            "    excludes: [made-up exclusion]",
            "    colour: red",
            "jobs.yaml",
        ),
        ("jobs:", "tasks:", "jobs.yaml"),
    ];
    for (from, to, expected) in cases {
        assert!(REVIEWED.contains(from), "{from}");
        let error = Jobs::parse(&REVIEWED.replacen(from, to, 1), &components()).unwrap_err();
        let message = format!("{error:#}");
        assert!(message.contains(expected), "{expected}: {message}");
    }
}

#[test]
fn the_file_in_the_repository_loads() {
    let data = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../data");
    let components = Components::load(&data).unwrap();
    let jobs = Jobs::load(&data, &components).unwrap().unwrap();
    assert!(jobs.list.len() >= 20);
    assert!(jobs.list.iter().any(|job| job.id == "brake-pads-front"));
    // While it is a draft, no job says what it covers; once it is
    // reviewed, every job does. `parse` holds the second half.
    if jobs.status == Status::Draft {
        assert!(jobs.list.iter().all(|job| job.includes.is_empty()));
    }
}
