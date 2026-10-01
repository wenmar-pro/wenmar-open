//! How search reads the data file through the server's engine.
//!
//! The catalog has a row for every model year: 490,000 in the 2026.09 data
//! file. A search runs a dozen statements, and each one that read those
//! rows from end to end cost the server most of a second. None of these
//! tests needs the real file: the engine is asked how it will answer each
//! statement, a search is timed against one reading of every model year on
//! a catalog large enough to tell the two apart, and the answers are
//! compared with SQLite's.

use std::time::{Duration, Instant};

use open_server::db::Db;
use wenmar_vehicles::sqlite::SqliteSource;
use wenmar_vehicles::{Catalog, Scope, Source, sql};

use crate::common;

/// The steps the server's engine takes to answer a statement.
fn plan(source: &impl Source, statement: &str) -> Vec<String> {
    source
        .query(&format!("EXPLAIN QUERY PLAN {statement}"), &[])
        .unwrap()
        .iter()
        .map(|row| row.last().and_then(|step| step.text()).unwrap().to_owned())
        .collect()
}

#[tokio::test]
async fn the_search_statements_reach_model_years_through_an_index() {
    let fixture = common::data_file();
    let db = Db::open(&fixture.path(), 1).await.unwrap();
    let plans = db
        .run(|worker| {
            [
                ("SEARCH_MODELS", sql::SEARCH_MODELS),
                ("SEARCH_MODELS_PREFIX", sql::SEARCH_MODELS_PREFIX),
                ("SEARCH_MAKE_MODELS", sql::SEARCH_MAKE_MODELS),
                ("SEARCH_SUBMODELS", sql::SEARCH_SUBMODELS),
                ("SEARCH_MAKE_SUBMODELS", sql::SEARCH_MAKE_SUBMODELS),
            ]
            .map(|(name, statement)| (name, plan(&worker.source, statement)))
        })
        .await
        .unwrap();
    for (name, plan) in plans {
        assert!(
            plan.iter()
                .any(|step| step.starts_with("SEARCH v USING INDEX catalog_vehicle_model")),
            "{name}: {plan:#?}"
        );
        assert!(
            !plan
                .iter()
                .any(|step| step.starts_with("SCAN catalog_vehicle")),
            "{name}: {plan:#?}"
        );
    }
}

/// The shortest of five runs.
fn shortest(mut work: impl FnMut()) -> Duration {
    (0..5)
        .map(|_| {
            let started = Instant::now();
            work();
            started.elapsed()
        })
        .min()
        .unwrap_or_default()
}

// Texts that make a search run as many statements as it ever does: six
// words, with and without a make among them, naming a model, a trim or
// nothing at all.
const LONG_TEXTS: [&str; 5] = [
    "maker 07 x y z w v",
    "maker 07 atlas 2 sport plus",
    "2015 maker 11 halo 4 sport",
    "zzqxv kjhgq wwpl aaa bbb ccc",
    "atlas 3 sport plus x y",
];

#[tokio::test]
async fn a_search_does_not_read_every_model_year_over_and_over() {
    let fixture = common::large_data_file();
    let db = Db::open(&fixture.path(), 1).await.unwrap();
    let (one_reading, searches) = db
        .run(|worker| {
            let one_reading = shortest(|| {
                let rows = worker
                    .source
                    .query("SELECT SUM(types), COUNT(*) FROM catalog_vehicle", &[])
                    .unwrap();
                assert!(rows[0][1].integer().unwrap() > 50_000);
            });
            let searches = LONG_TEXTS.map(|text| {
                (
                    text,
                    shortest(|| {
                        worker.catalog.search(text, Scope::All, 10).unwrap();
                    }),
                )
            });
            (one_reading, searches)
        })
        .await
        .unwrap();
    // Before the statements were rewritten these cost from 7 to 40 readings
    // each. Now the dearest, which looks up the trims of forty models called
    // `Atlas 3`, costs about one, and the rest half of one or less. Both
    // sides are measured here, on the same connection, so the machine's
    // speed does not matter.
    let slow: Vec<String> = searches
        .iter()
        .filter(|(_, search)| *search > one_reading * 5)
        .map(|(text, search)| format!("{text:?} took {search:?}"))
        .collect();
    assert!(
        slow.is_empty(),
        "one reading of every model year takes {one_reading:?}, and {slow:?}"
    );
}

#[tokio::test]
async fn both_engines_find_the_same_entries_in_the_same_order() {
    let fixture = common::large_data_file();
    let through_sqlite = Catalog::new(SqliteSource::open(fixture.path()).unwrap()).unwrap();
    let db = Db::open(&fixture.path(), 1).await.unwrap();
    let texts = [
        // A name forty models share: only the order picks the ten.
        "atlas",
        "atlas 3",
        "a",
        "c",
        "2019 c",
        "2015 atlas 1",
        "halo 4 sport plus",
        "arrow 2 sp",
        "maker 12",
        "maker 07 atlas",
        "maker 07 atlas 2",
        "maker 07 sport",
        "maker 03 1500",
        "maker 03 15",
        "1500",
        "f150",
        "chevy 1500",
        "2019 civic si",
        "ford",
    ];
    let mut found = 0;
    for text in texts.into_iter().chain(LONG_TEXTS) {
        for scope in [Scope::Light, Scope::All, Scope::Type(3), Scope::Type(6)] {
            let ids = |entries: Vec<wenmar_vehicles::Entry>| -> Vec<String> {
                entries.into_iter().map(|entry| entry.id).collect()
            };
            let expected = ids(through_sqlite.search(text, scope, 10).unwrap());
            let through_turso = ids(db
                .run(move |worker| worker.catalog.search(text, scope, 10).unwrap())
                .await
                .unwrap());
            assert_eq!(through_turso, expected, "{text:?} in {scope:?}");
            found += expected.len();
        }
    }
    // The texts are not all misses.
    assert!(found > 300, "{found} entries in all");
}
