//! What native FTS5 must do for the server's search index, checked before
//! the index is ported to it. turso ranked with tantivy behind the same
//! `fts_match` and `fts_score` names, so the contract is the queries, not
//! the implementation.
//!
//! The bundled SQLite has none of turso's names: `CREATE INDEX ... USING
//! fts` is a syntax error and `fts_match` and `fts_score` do not exist, so
//! this builds the same normalized `text` as an FTS5 virtual table and asks
//! it the same words.
//!
//! The file separates two things. The *engine study* is what a bare FTS5
//! query answers — the query text the server builds today — including
//! where it diverges: FTS5 conjoins the words of a bare query where
//! tantivy disjoined them, and orders a negative bm25() ascending where
//! tantivy ordered a positive score descending. The *contract* follows
//! from the study: the port puts OR between the words, keeps bm25()
//! ascending, and must give the OR-joined answers the last test pins.

use rusqlite::{Connection, params};

use open_server::search_index::{Found, IndexRow, SearchIndex, words};

/// An FTS5 index shaped like the server's, built over rows.
///
/// This is the port of turso's `CREATE TABLE model ...` plus
/// `CREATE INDEX model_text ON model USING fts (text)`: SQLite puts text
/// under FTS5 only in a virtual table, and a virtual table may not share a
/// name with one of its columns, so the index is `model_text`, carrying
/// the same columns with only `text` indexed.
fn index(rows: &[IndexRow]) -> Connection {
    let connection = Connection::open_in_memory().unwrap();
    connection
        .execute(
            "CREATE VIRTUAL TABLE model_text USING fts5 (
                 make UNINDEXED, model UNINDEXED,
                 year_from UNINDEXED, year_to UNINDEXED,
                 light UNINDEXED, text
             )",
            [],
        )
        .unwrap();
    connection.execute("BEGIN", ()).unwrap();
    for row in rows {
        connection
            .execute(
                "INSERT INTO model_text (make, model, year_from, year_to, light, text)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
                params![
                    row.make,
                    row.model,
                    row.year_from,
                    row.year_to,
                    i64::from(row.light),
                    row.text
                ],
            )
            .unwrap();
    }
    connection.execute("COMMIT", ()).unwrap();
    connection
}

/// The models FTS5 returns for a query, best first.
fn ranked(connection: &Connection, query: &str, limit: i64) -> Vec<String> {
    let mut statement = connection
        .prepare(
            "SELECT make, model FROM model_text
             WHERE model_text MATCH ?1
             ORDER BY bm25(model_text)
             LIMIT ?2",
        )
        .unwrap();
    // turso ordered a tantivy score, larger first, by `fts_score ... DESC`.
    // SQLite's bm25() is negative, the better the match the further from
    // zero downwards, so best first is ascending, and the hidden `rank`
    // column orders the same way. The fixture shares a query between two
    // models on purpose, so a port that orders the other way round has
    // something to fail on.
    let rows = statement
        .query_map(params![query, limit], |row| {
            Ok(format!(
                "{} {}",
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?
            ))
        })
        .unwrap();
    rows.map(|row| row.unwrap()).collect()
}

/// The models FTS5 returns for `text` asked the way the server asks it
/// today: the words of the text, bare, in one string.
fn models(connection: &Connection, text: &str, limit: i64) -> Vec<String> {
    let words = words(text);
    if words.is_empty() {
        return Vec::new();
    }
    ranked(connection, &words.join(" "), limit)
}

/// The query the port must ask instead: the words with OR between them,
/// so any of them finds a model, as tantivy's query did.
fn or_joined(text: &str) -> String {
    words(text).join(" OR ")
}

fn row(make: &str, model: &str, light: bool, text: &str) -> IndexRow {
    IndexRow {
        make: make.to_owned(),
        model: model.to_owned(),
        year_from: 2018,
        year_to: 2020,
        light,
        text: text.to_owned(),
    }
}

fn rows() -> Vec<IndexRow> {
    vec![
        row("honda", "civic", true, "honda civic civic"),
        // The same words as the Civic, scoring lower: two models for one
        // query, so the order between them is the sort direction.
        row("honda", "civic-si", true, "honda civic si civicsi"),
        row("honda", "cr-v", true, "honda cr-v crv"),
        row("ford", "f-150", true, "ford f-150 f150"),
        row(
            "chevrolet",
            "silverado",
            true,
            "chevrolet chevy silverado silverado",
        ),
        row(
            "ranger-trailers",
            "tilt-deck",
            false,
            "ranger trailers tilt deck tiltdeck",
        ),
    ]
}

/// The engine study on the fixture: what bare FTS5 — the query text the
/// server builds today — answers, and which way round it sorts. Not the
/// port's contract: the port asks its words with OR between them, as the
/// last test pins.
#[test]
fn bare_fts5_ranks_the_fixture_models_best_first() {
    let connection = index(&rows());
    // Both models have the words in any order; the Civic, with more of
    // them and less else, ranks first. Reversing the sort direction
    // reverses this list.
    assert_eq!(
        models(&connection, "civic honda", 10),
        ["honda civic", "honda civic-si"]
    );
    // The matching form of a name.
    assert_eq!(models(&connection, "f150", 10), ["ford f-150"]);
    // A make alias.
    assert_eq!(models(&connection, "chevy", 10), ["chevrolet silverado"]);
}

/// The one promise that does not carry over. turso's tantivy matched the
/// words of a query in any order, so `the civic by honda` found the Civic
/// first (`words_in_any_order_find_a_model` in `search_index.rs`). FTS5
/// conjoins the words of a bare query, and no row has `the`, so nothing is
/// found. Task 3 keeps the old promise only by putting OR between the
/// words itself; this test pins what FTS5 does with the query text as the
/// server sends it today.
#[test]
fn native_fts5_wants_every_word_where_turso_wanted_any() {
    let connection = index(&rows());
    assert_eq!(
        models(&connection, "the civic by honda", 10),
        Vec::<String>::new()
    );
    // Both words together still find the models that have both, and only
    // those.
    assert_eq!(
        models(&connection, "civic honda", 10),
        ["honda civic", "honda civic-si"]
    );
}

#[test]
fn user_text_that_is_query_syntax_finds_nothing_and_does_not_fail() {
    let connection = index(&rows());
    for text in [
        "",
        "\"unterminated",
        "'; DROP TABLE model; --",
        "+ - ( ) * : ^ ~",
        "AND OR NOT",
        "%",
    ] {
        // The server drops everything but letters and digits before
        // asking, so the text that still has words reaches FTS5 as plain
        // words and must find nothing — and must not fail its parser.
        // What is left with no words is answered empty without asking.
        assert_eq!(
            models(&connection, text, 10),
            Vec::<String>::new(),
            "{text}"
        );
    }
}

/// The port itself, asked the fixture questions the unit tests in
/// `search_index.rs` ask. Two models share `civic honda` here on purpose,
/// so an engine that ranks them the other way round — or that conjoins
/// the words and drops the stray-word query — has something to fail on.
#[test]
fn the_index_gives_the_fixture_the_answers_the_study_found() {
    let index = block_on(SearchIndex::build(&rows())).unwrap();
    // The two Civics, the plain one first: the sort direction, and the
    // OR join, in one assertion. `cr-v` shares only `honda`, so the OR
    // query reaches it too, last.
    assert_eq!(
        found(&index, "civic honda", true),
        ["honda civic", "honda civic-si", "honda cr-v"]
    );
    // The stray words cost nothing: the Civic is still first, as tantivy
    // had it, where a bare FTS5 query finds nothing at all.
    assert_eq!(found(&index, "the civic by honda", true)[0], "honda civic");
    // The matching form of a name.
    assert_eq!(found(&index, "f150", true), ["ford f-150"]);
    // A make alias.
    assert_eq!(found(&index, "chevy", true), ["chevrolet silverado"]);
    // The scope and the years come back through the port unchanged.
    assert!(found(&index, "tilt deck", true).is_empty());
    let all = block_on(index.find("tilt deck", false, 10)).unwrap();
    assert_eq!(
        format!("{} {}", all[0].make, all[0].model),
        "ranger-trailers tilt-deck"
    );
    assert_eq!((all[0].year_from, all[0].year_to), (2018, 2020));
}

/// The models `SearchIndex` returns for a text, best first, the way the
/// server asks.
fn found(index: &SearchIndex, text: &str, light_only: bool) -> Vec<String> {
    block_on(async {
        index
            .find(text, light_only, 10)
            .await
            .unwrap()
            .into_iter()
            .map(|Found { make, model, .. }| format!("{make} {model}"))
            .collect()
    })
}

fn block_on<F: std::future::Future>(future: F) -> F::Output {
    tokio::runtime::Builder::new_current_thread()
        .build()
        .unwrap()
        .block_on(future)
}

/// The data file `mise run data` builds, when it has been built. It is
/// not in the repository and CI has none, so the tests that need the real
/// catalog do nothing without it.
fn data_file() -> Option<std::path::PathBuf> {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../data/build/wenmar-open-2026.09.sqlite3");
    path.is_file().then_some(path)
}

/// The rows `state.rs` builds the server's index from, read from the real
/// data file the way the server reads it.
fn real_rows() -> Option<Vec<IndexRow>> {
    let path = data_file()?;
    let source = wenmar_vehicles::sqlite::SqliteSource::open(&path).unwrap();
    Some(open_server::search_index::rows(&source).unwrap())
}

/// The engine study on the real catalog: what a bare FTS5 query answers,
/// and where that diverges from tantivy. Diagnosis, not the contract:
/// run with `-- --nocapture` to read the whole lists.
#[tokio::test]
async fn bare_fts5_on_the_real_index_shows_where_it_diverges() {
    let Some(rows) = real_rows() else {
        eprintln!(
            "no data file beside this worktree: nothing to rank; build one with `mise run data`"
        );
        return;
    };
    assert_eq!(rows.len(), 31_470, "the 2026.09 release");
    let connection = index(&rows);
    for text in ["f150", "chevy", "2019 civic si", "civic honda"] {
        println!("{text:?} -> {:?}", models(&connection, text, 10));
    }
    // The matching form of the name: `f150` is the F-150's norm, and the
    // Dongfang DF150 motorcycles share no token with it.
    assert_eq!(models(&connection, "f150", 10), ["ford f-150"]);
    // Both words together: the three Civics, the plain one first.
    assert_eq!(models(&connection, "civic honda", 10)[0], "honda civic");
    // The alias `chevy` is in every Chevrolet model's text, all tied on
    // this one word. The order among the ties is the order `MODELS_SQL`
    // read the models in — it has no ORDER BY — so the set is pinned
    // here, not the order; the printed list shows the read order.
    let mut chevy = models(&connection, "chevy", 10);
    chevy.sort();
    assert_eq!(
        chevy,
        [
            "chevrolet aveo",
            "chevrolet camaro",
            "chevrolet corvette",
            "chevrolet cruze",
            "chevrolet impala",
            "chevrolet malibu",
            "chevrolet sonic",
            "chevrolet spark",
            "chevrolet ss",
            "chevrolet volt",
        ]
    );
    // Every word of a bare query must be present. No model's text holds
    // `2019`, so FTS5 finds nothing where tantivy, disjoining the words,
    // found the Civics.
    assert_eq!(
        models(&connection, "2019 civic si", 10),
        Vec::<String>::new()
    );
}

/// The contract for the port: the same words asked with OR between them,
/// which is tantivy's answer shape. On the real index Task 3's port must
/// give these heads, best first by bm25(), whatever it does internally.
/// Run with `-- --nocapture` to read the whole lists.
#[tokio::test]
async fn or_joined_queries_give_the_answers_the_port_must_give() {
    let Some(rows) = real_rows() else {
        eprintln!(
            "no data file beside this worktree: nothing to rank; build one with `mise run data`"
        );
        return;
    };
    let connection = index(&rows);
    for text in [
        "f150",
        "chevy",
        "2019 civic si",
        "civic honda",
        "the civic by honda",
    ] {
        println!(
            "{:?} -> {:?}",
            or_joined(text),
            ranked(&connection, &or_joined(text), 10)
        );
    }
    // turso found the Civic first even with the stray words, and still
    // does with the words joined: the tail is the models whose names hold
    // `by`, as turso's was.
    assert_eq!(
        ranked(&connection, &or_joined("the civic by honda"), 10)[0],
        "honda civic"
    );
    // The year is in no model's text; the words still find the model the
    // words were looking for, as tantivy did.
    assert_eq!(
        ranked(&connection, &or_joined("2019 civic si"), 10)[0],
        "honda civic-si"
    );
    assert_eq!(
        ranked(&connection, &or_joined("civic honda"), 10)[0],
        "honda civic"
    );
    // A single word is its own query, joined or not.
    assert_eq!(ranked(&connection, &or_joined("f150"), 10), ["ford f-150"]);
}
