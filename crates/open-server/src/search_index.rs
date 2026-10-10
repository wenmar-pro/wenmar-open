//! The server's own search index: a second SQLite database, in memory,
//! with a full-text index over make and model names.
//!
//! The published data file is plain SQLite and stays that way. This index
//! is the server's, is rebuilt in memory from the data file at every start,
//! and is never written to disk. It is asked only when the catalog's own
//! reading of the text finds nothing, so that words in any order still find
//! a model.

use rusqlite::{Connection, params};
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};
use wenmar_vehicles::text::normalize;
use wenmar_vehicles::{Source, SourceError, Value};

/// Most words of a search that are passed to the index.
pub const MOST_WORDS: usize = 6;
/// Longest word passed to the index.
pub const LONGEST_WORD: usize = 40;

const MODELS_SQL: &str = "
SELECT mk.id, mk.slug, mk.name, md.slug, md.name, md.norm, md.year_from, md.year_to, md.light
FROM catalog_model md
JOIN catalog_make mk ON mk.id = md.make_id";

const ALIASES_SQL: &str = "SELECT make_id, norm FROM catalog_alias";

/// One model, with the text it can be found by.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IndexRow {
    pub make: String,
    pub model: String,
    pub year_from: u16,
    pub year_to: u16,
    pub light: bool,
    /// The make's name and aliases and the model's name, in lowercase, plus
    /// the model's matching form, so `f150` finds `F-150`.
    pub text: String,
}

/// A model the index found. `make` and `model` are id forms.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Found {
    pub make: String,
    pub model: String,
    pub year_from: u16,
    pub year_to: u16,
}

fn shape() -> SourceError {
    "unexpected row while building the search index".into()
}

fn text(row: &[Value], column: usize) -> Result<String, SourceError> {
    row.get(column)
        .and_then(Value::text)
        .map(str::to_owned)
        .ok_or_else(shape)
}

fn integer(row: &[Value], column: usize) -> Result<i64, SourceError> {
    row.get(column).and_then(Value::integer).ok_or_else(shape)
}

/// Reads every model of the data file. Blocks.
pub fn rows<S: Source>(source: &S) -> Result<Vec<IndexRow>, SourceError> {
    let mut aliases: std::collections::HashMap<i64, Vec<String>> = Default::default();
    for row in source.query(ALIASES_SQL, &[])? {
        aliases
            .entry(integer(&row, 0)?)
            .or_default()
            .push(text(&row, 1)?);
    }
    let mut found = Vec::new();
    for row in source.query(MODELS_SQL, &[])? {
        let make_id = integer(&row, 0)?;
        let mut words = vec![text(&row, 2)?.to_lowercase()];
        words.extend(aliases.get(&make_id).cloned().unwrap_or_default());
        words.push(text(&row, 4)?.to_lowercase());
        words.push(text(&row, 5)?);
        found.push(IndexRow {
            make: text(&row, 1)?,
            model: text(&row, 3)?,
            year_from: u16::try_from(integer(&row, 6)?).map_err(|_| shape())?,
            year_to: u16::try_from(integer(&row, 7)?).map_err(|_| shape())?,
            light: integer(&row, 8)? != 0,
            text: words.join(" "),
        });
    }
    Ok(found)
}

/// The words of a search as the index may be given them: lowercase letters
/// and digits only. The index has a query language of its own, with quotes,
/// `+`, `-`, `AND` and `OR`; nothing typed by a caller may reach it as
/// syntax, so everything but letters and digits is dropped and the words
/// are passed in lowercase, where `and` and `or` are ordinary words.
pub fn words(text: &str) -> Vec<String> {
    text.split(|character: char| !character.is_ascii_alphanumeric())
        .map(normalize)
        .filter(|word| !word.is_empty())
        .map(|word| word.chars().take(LONGEST_WORD).collect())
        .take(MOST_WORDS)
        .collect()
}

pub struct SearchIndex {
    // A rusqlite in-memory connection owns its own database for its
    // lifetime, so there is no handle to keep alive. The Arc is what lets
    // `find` move the lock into a blocking task.
    connection: Arc<Mutex<Connection>>,
}

impl std::fmt::Debug for SearchIndex {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.debug_struct("SearchIndex").finish()
    }
}

impl SearchIndex {
    /// Builds the index in memory, off the async runtime.
    ///
    /// About 0.1 seconds for the 31,470 models of the 2026.09 release in a
    /// release build, and seconds in a debug build, so this is not work to do
    /// on a runtime thread.
    pub async fn build(rows: &[IndexRow]) -> Result<SearchIndex, rusqlite::Error> {
        let rows = rows.to_vec();
        tokio::task::spawn_blocking(move || Self::build_blocking(&rows))
            .await
            .map_err(|_| rusqlite::Error::InvalidQuery)?
    }

    /// The build itself. Blocking; called from `build`, off the runtime.
    fn build_blocking(rows: &[IndexRow]) -> Result<SearchIndex, rusqlite::Error> {
        let connection = Connection::open_in_memory()?;
        connection.execute_batch(
            "CREATE VIRTUAL TABLE model_text USING fts5(
                 make UNINDEXED,
                 model UNINDEXED,
                 year_from UNINDEXED,
                 year_to UNINDEXED,
                 light UNINDEXED,
                 text
             );",
        )?;
        connection.execute("BEGIN", ())?;
        for row in rows {
            connection.execute(
                "INSERT INTO model_text (make, model, year_from, year_to, light, text)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
                params![
                    row.make,
                    row.model,
                    row.year_from,
                    row.year_to,
                    row.light,
                    row.text
                ],
            )?;
        }
        connection.execute("COMMIT", ())?;
        Ok(SearchIndex {
            connection: Arc::new(Mutex::new(connection)),
        })
    }

    /// Models whose text has any of the words, best first.
    ///
    /// The query is run off the async runtime. `rusqlite` is a synchronous
    /// wrapper around C SQLite, so the work is ordinary blocking I/O and
    /// computation; doing it on a runtime thread would hold one worker for the
    /// length of the query.
    pub async fn find(
        &self,
        text: &str,
        light_only: bool,
        limit: usize,
    ) -> Result<Vec<Found>, rusqlite::Error> {
        let words = words(text);
        if words.is_empty() {
            return Ok(Vec::new());
        }
        // A bare FTS5 query conjoins its words, where tantivy matched any
        // of them; OR between the words keeps the old promise. The words
        // are lowercase letters and digits, so none of them is FTS5's
        // uppercase `OR` and none can become syntax.
        let query = words.join(" OR ");
        let connection = self.connection.clone();
        // The connection is behind a std Mutex, so it has to be moved into
        // the blocking task; that is why this is not a plain `&self` capture.
        tokio::task::spawn_blocking(move || {
            // A poisoned lock would mean a previous query panicked while
            // holding it. The connection is still usable, so take it.
            let connection = connection.lock().unwrap_or_else(PoisonError::into_inner);
            let mut found = Self::query(&connection, &query, light_only, limit)?;
            found.truncate(limit);
            Ok(found)
        })
        .await
        .map_err(|_| rusqlite::Error::InvalidQuery)?
    }

    /// The search itself. Blocking; called from `find`, off the runtime.
    fn query(
        connection: &MutexGuard<'_, Connection>,
        query: &str,
        light_only: bool,
        limit: usize,
    ) -> Result<Vec<Found>, rusqlite::Error> {
        // bm25() scores in negative numbers: the better the match, the
        // further below zero it sits, so best first is ascending.
        let mut statement = connection.prepare(
            "SELECT make, model, year_from, year_to
             FROM model_text
             WHERE model_text MATCH ?1
               AND (?2 = 0 OR light = 1)
             ORDER BY bm25(model_text)
             LIMIT ?3",
        )?;
        let rows = statement.query_map(
            params![
                query,
                i64::from(light_only),
                // The real limit: what the scope allows is what counts
                // towards it, so there is nothing to make room for.
                i64::try_from(limit).unwrap_or(i64::MAX),
            ],
            |row| {
                Ok((
                    row.get::<_, String>(0).unwrap_or_default(),
                    row.get::<_, String>(1).unwrap_or_default(),
                    row.get::<_, i64>(2).unwrap_or(0),
                    row.get::<_, i64>(3).unwrap_or(0),
                ))
            },
        )?;
        // SQL applies the scope and the limit, so every row that comes
        // back is a keeper: no post-fetch filtering and no early break.
        let mut found = Vec::new();
        for row in rows {
            let (make, model, year_from, year_to) = row?;
            let year = |year: i64| u16::try_from(year).unwrap_or(0);
            found.push(Found {
                make,
                model,
                year_from: year(year_from),
                year_to: year(year_to),
            });
        }
        Ok(found)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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

    async fn index() -> SearchIndex {
        SearchIndex::build(&[
            row("honda", "civic", true, "honda civic civic"),
            // The same words as the Civic, scoring lower: two models for
            // one query, so the order between them is the sort direction.
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
        ])
        .await
        .unwrap()
    }

    async fn models(index: &SearchIndex, text: &str) -> Vec<String> {
        index
            .find(text, true, 10)
            .await
            .unwrap()
            .into_iter()
            .map(|found| format!("{} {}", found.make, found.model))
            .collect()
    }

    #[test]
    fn words_are_letters_and_digits_in_lowercase() {
        assert_eq!(words("Honda  CIVIC"), ["honda", "civic"]);
        assert_eq!(words("f-150"), ["f", "150"]);
        assert_eq!(
            words("\"civic\" AND +honda -ford"),
            ["civic", "and", "honda", "ford"]
        );
        assert_eq!(words("'; DROP TABLE model; --"), ["drop", "table", "model"]);
        assert_eq!(words("%_*()^~:"), Vec::<String>::new());
        assert_eq!(words("É 🚗"), Vec::<String>::new());
        assert_eq!(words("a b c d e f g h").len(), MOST_WORDS);
        assert_eq!(words(&"x".repeat(100_000))[0].len(), LONGEST_WORD);
    }

    #[tokio::test]
    async fn words_in_any_order_find_a_model() {
        let index = index().await;
        // Both Civics, the plain one first: the query reaches either, and
        // the order between them is the index's ranking, best first. The
        // CR-V shares only `honda`, so the words in any order reach it
        // too, after the Civics.
        assert_eq!(
            models(&index, "civic honda").await,
            ["honda civic", "honda civic-si", "honda cr-v"]
        );
        assert_eq!(models(&index, "the civic by honda").await[0], "honda civic");
        assert_eq!(models(&index, "f150").await, ["ford f-150"]);
        assert_eq!(models(&index, "chevy").await, ["chevrolet silverado"]);
    }

    #[tokio::test]
    async fn the_scope_keeps_trailers_out() {
        let index = index().await;
        assert!(models(&index, "tilt deck").await.is_empty());
        let all = index.find("tilt deck", false, 10).await.unwrap();
        assert_eq!(all[0].model, "tilt-deck");
        assert_eq!((all[0].year_from, all[0].year_to), (2018, 2020));
    }

    #[tokio::test]
    async fn text_that_is_query_syntax_finds_nothing_and_does_not_fail() {
        let index = index().await;
        for text in [
            "",
            "\"unterminated",
            "'; DROP TABLE model; --",
            "+ - ( ) * : ^ ~",
            "AND OR NOT",
            "%",
        ] {
            let found = index.find(text, false, 10).await;
            assert_eq!(found.unwrap(), Vec::new(), "{text}");
        }
        // Syntax around a real word is dropped and the word still counts;
        // both Civics hold it, the plain one first.
        assert_eq!(
            models(&index, "text:civic").await,
            ["honda civic", "honda civic-si"]
        );
        assert_eq!(models(&index, "\"civic\" -honda").await[0], "honda civic");
    }
}
