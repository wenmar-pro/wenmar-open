//! The fixture `clients/js` tests against: the data file as SQL, and the
//! answer to every case. Both are written from here, so the tables are the
//! libraries' own and the answers are worked out natively.
//!
//! After a change that is meant to change them:
//!
//!   UPDATE_FIXTURES=1 cargo test -p wenmar-open-wasm --test fixture

mod common;

use std::path::PathBuf;

use serde_json::{Value, json};

fn directory() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../clients/js/test/fixtures")
}

fn check(name: &str, fresh: &str) {
    let path = directory().join(name);
    if std::env::var_os("UPDATE_FIXTURES").is_some() {
        std::fs::create_dir_all(directory()).unwrap();
        std::fs::write(&path, fresh).unwrap();
        return;
    }
    let committed = std::fs::read_to_string(&path).unwrap_or_default();
    assert!(
        committed == fresh,
        "{} is out of date. Run: UPDATE_FIXTURES=1 cargo test -p wenmar-open-wasm --test fixture",
        path.display()
    );
}

#[test]
fn the_fixtures_of_the_npm_package_are_up_to_date() {
    let connection = common::data();
    let mut engine = wenmar_open_wasm::Engine::new();
    // Every statement that is run, with its rows: a database for tests that
    // have no SQLite.
    let mut recorded: Vec<Value> = Vec::new();
    let mut record = |statements: &[Value]| {
        for statement in statements {
            let answered = json!({
                "sql": statement["sql"],
                "params": statement["params"],
                "rows": common::rows(&connection, statement),
            });
            if !recorded.contains(&answered) {
                recorded.push(answered);
            }
        }
    };
    let open = common::run(&mut engine, &connection, "open", json!({}));
    record(&open.statements);
    let cases: Vec<Value> = common::cases()
        .into_iter()
        .map(|(name, op, args)| {
            let run = common::run(&mut engine, &connection, op, args.clone());
            record(&run.statements);
            json!({ "name": name, "op": op, "args": args, "steps": run.steps, "answer": run.answer })
        })
        .collect();
    let cases = json!({ "current_year": common::YEAR, "cases": cases, "recorded": recorded });
    check(
        "offline.sql",
        &common::fixture_sql(wenmar_open_wasm::SCHEMA_VERSION),
    );
    check(
        "offline-cases.json",
        &format!("{}\n", serde_json::to_string_pretty(&cases).unwrap()),
    );
}
