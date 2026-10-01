//! The server's own search index: a second turso database with a full-text
//! index over make and model names.
//!
//! The published data file is plain SQLite and stays that way. This index
//! is the server's, is rebuilt in memory from the data file at every start,
//! and is never written to disk. It is asked only when the catalog's own
//! reading of the text finds nothing, so that words in any order still find
//! a model.

use tokio::sync::Mutex;
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
    connection: Mutex<turso::Connection>,
    // Keeps the in-memory database alive.
    _database: turso::Database,
}

impl std::fmt::Debug for SearchIndex {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.debug_struct("SearchIndex").finish()
    }
}

impl SearchIndex {
    /// Builds the index in memory. About 0.4 seconds for the 31,470 models
    /// of the 2026.09 release.
    pub async fn build(rows: &[IndexRow]) -> Result<SearchIndex, turso::Error> {
        let database = turso::Builder::new_local(":memory:")
            .experimental_index_method(true)
            .build()
            .await?;
        let connection = database.connect()?;
        connection
            .execute_batch(
                "CREATE TABLE model (
                     id        INTEGER PRIMARY KEY,
                     make      TEXT NOT NULL,
                     model     TEXT NOT NULL,
                     year_from INTEGER NOT NULL,
                     year_to   INTEGER NOT NULL,
                     light     INTEGER NOT NULL,
                     text      TEXT NOT NULL
                 );",
            )
            .await?;
        connection.execute("BEGIN", ()).await?;
        for row in rows {
            connection
                .execute(
                    "INSERT INTO model (make, model, year_from, year_to, light, text)
                     VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
                    turso::params_from_iter(vec![
                        turso::Value::Text(row.make.clone()),
                        turso::Value::Text(row.model.clone()),
                        turso::Value::Integer(i64::from(row.year_from)),
                        turso::Value::Integer(i64::from(row.year_to)),
                        turso::Value::Integer(i64::from(row.light)),
                        turso::Value::Text(row.text.clone()),
                    ]),
                )
                .await?;
        }
        connection.execute("COMMIT", ()).await?;
        connection
            .execute("CREATE INDEX model_text ON model USING fts (text)", ())
            .await?;
        Ok(SearchIndex {
            connection: Mutex::new(connection),
            _database: database,
        })
    }

    /// Models whose text has any of the words, best first.
    pub async fn find(
        &self,
        text: &str,
        light_only: bool,
        limit: usize,
    ) -> Result<Vec<Found>, turso::Error> {
        let words = words(text);
        if words.is_empty() {
            return Ok(Vec::new());
        }
        let query = words.join(" ");
        let connection = self.connection.lock().await;
        let mut rows = connection
            .query(
                "SELECT make, model, year_from, year_to, light
                 FROM model
                 WHERE fts_match(text, ?1)
                 ORDER BY fts_score(text, ?1) DESC
                 LIMIT ?2",
                turso::params_from_iter(vec![
                    turso::Value::Text(query),
                    // Room for the rows the scope will remove.
                    turso::Value::Integer(i64::try_from(limit.saturating_mul(4)).unwrap_or(200)),
                ]),
            )
            .await?;
        let mut found = Vec::new();
        while let Some(row) = rows.next().await? {
            let light = row.get_value(4)?.as_integer().copied().unwrap_or(0) != 0;
            if light_only && !light {
                continue;
            }
            let year = |index: usize| -> Result<u16, turso::Error> {
                Ok(row
                    .get_value(index)?
                    .as_integer()
                    .and_then(|year| u16::try_from(*year).ok())
                    .unwrap_or(0))
            };
            found.push(Found {
                make: row.get_value(0)?.as_text().cloned().unwrap_or_default(),
                model: row.get_value(1)?.as_text().cloned().unwrap_or_default(),
                year_from: year(2)?,
                year_to: year(3)?,
            });
            if found.len() >= limit {
                break;
            }
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
        assert_eq!(models(&index, "civic honda").await[0], "honda civic");
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
        // Syntax around a real word is dropped and the word still counts.
        assert_eq!(models(&index, "text:civic").await, ["honda civic"]);
        assert_eq!(models(&index, "\"civic\" -honda").await[0], "honda civic");
    }
}
