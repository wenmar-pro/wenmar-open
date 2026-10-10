// Does a fallback search still hold a tokio runtime thread?
//
// `rusqlite` is a synchronous wrapper around C SQLite, so the query is
// ordinary blocking work. If it ran on the runtime thread itself, a ticker
// task alongside it would stall. The batch is sized so the searching takes
// long enough for a stall to be unmistakable.
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Duration;

use open_server::search_index::IndexRow;

fn row(make: &str, model: &str, text: &str) -> IndexRow {
    IndexRow {
        make: make.to_owned(),
        model: model.to_owned(),
        year_from: 2018,
        year_to: 2020,
        light: true,
        text: text.to_owned(),
    }
}

/// Searches run: enough that they take a few hundred milliseconds together.
const SEARCHES: usize = 1_000;
/// Models: wide enough that each search is real work, not a lookup.
const MODELS: usize = 20_000;
/// Ticks the runtime must reach while that happens. A blocked runtime
/// manages about one.
const TICKS: usize = 20;

#[tokio::test(flavor = "current_thread")]
async fn a_search_does_not_hold_up_the_runtime() {
    let rows: Vec<IndexRow> = (0..MODELS)
        .map(|i| {
            row(
                "make",
                &format!("model{i}"),
                &format!("make model{i} series"),
            )
        })
        .collect();
    let index = open_server::search_index::SearchIndex::build(&rows)
        .await
        .unwrap();

    let ticks = Arc::new(AtomicUsize::new(0));
    let ticker = {
        let ticks = Arc::clone(&ticks);
        tokio::spawn(async move {
            loop {
                tokio::time::sleep(Duration::from_millis(1)).await;
                ticks.fetch_add(1, Ordering::Relaxed);
            }
        })
    };
    for round in 0..SEARCHES {
        let found = index
            .find(&format!("model{}", round % MODELS), true, 10)
            .await
            .unwrap();
        assert_eq!(found.len(), 1, "the search found {} models", found.len());
    }
    ticker.abort();
    let ticked = ticks.load(Ordering::Relaxed);
    assert!(
        ticked >= TICKS,
        "the runtime ran {ticked} ticks across {SEARCHES} searches; \
         it was held up for at least some of them"
    );
}
