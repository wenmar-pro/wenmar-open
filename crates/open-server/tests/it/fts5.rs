//! What native FTS5 must do for the server's search index, checked before
//! the index is ported to it. turso ranked with tantivy behind the same
//! `fts_match` and `fts_score` names, so the contract is the queries, not
//! the implementation.
//!
//! The bundled SQLite has none of turso's names: `CREATE INDEX ... USING
//! fts` is a syntax error and `fts_match` and `fts_score` do not exist, so
//! this builds the same normalized `text` as an FTS5 virtual table and asks
//! it the same query text. Two things turned out not to carry over, and
//! each has a test of its own below: SQLite's bm25() is negative where
//! tantivy's score was positive, and a bare FTS5 query conjoins its words
//! where tantivy disjoined them.

use rusqlite::{Connection, params};

use open_server::search_index::{IndexRow, words};

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

/// The models FTS5 returns for `text`, best first.
fn models(connection: &Connection, text: &str, limit: i64) -> Vec<String> {
    let words = words(text);
    if words.is_empty() {
        return Vec::new();
    }
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
    // column orders the same way.
    let rows = statement
        .query_map(params![words.join(" "), limit], |row| {
            Ok(format!(
                "{} {}",
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?
            ))
        })
        .unwrap();
    rows.map(|row| row.unwrap()).collect()
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

#[test]
fn native_fts5_finds_models_the_way_the_server_promises() {
    let connection = index(&rows());
    // Words in any order, and the matching form of a name.
    assert_eq!(models(&connection, "civic honda", 10)[0], "honda civic");
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
    // Both words together still find the model that has both.
    assert_eq!(models(&connection, "civic honda", 10), ["honda civic"]);
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
        // The server drops everything but letters and digits before asking,
        // so only the empty case reaches FTS5 as an empty query.
        if words(text).is_empty() {
            assert_eq!(
                models(&connection, text, 10),
                Vec::<String>::new(),
                "{text}"
            );
        }
    }
}

/// The data file `mise run data` builds, when it has been built. It is not
/// in the repository, so the tests that need the real catalog do nothing
/// without it.
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

/// The queries the port must answer the same way on the real index, and
/// the first ten models each returns. Run with `-- --nocapture` to read
/// them; Task 3's port has to reproduce these orderings.
#[tokio::test]
async fn the_real_index_ranks_the_way_the_port_must_reproduce() {
    let Some(rows) = real_rows() else {
        eprintln!("no real data file beside this worktree: nothing to rank");
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
    // this one word, and FTS5 breaks the tie by nothing better than the
    // order the models were added in.
    assert_eq!(
        models(&connection, "chevy", 10),
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
