//! Free-text search over the catalog.

use std::cmp::Reverse;
use std::collections::HashSet;

use crate::catalog::{Catalog, CatalogError, Entry, integer, text, year};
use crate::id::VehicleId;
use crate::index::{MAX_FORM, MakeRef, Scope};
use crate::parse::{Parsed, parse};
use crate::source::{Source, Value};
use crate::sql;
use crate::text::slug;

/// The most entries a search returns, whatever limit is asked for.
pub const MOST_HITS: usize = 50;
/// How many rows each lookup of a search may bring back.
const PER_LOOKUP: i64 = 20;
/// How many submodels are tried for one model.
const SUBMODELS_PER_MODEL: i64 = 5;
/// The most words that are tried together as a model name.
const MODEL_WORDS: usize = 4;

/// Something a search found, before it is turned into an entry.
struct Hit {
    /// How many words of the text it accounts for.
    explained: usize,
    /// Whether the model, and the submodel if any, matched in full.
    exact: bool,
    rank: Option<u32>,
    year: u16,
    id: String,
}

/// Words joined into one matching form and cut to [`MAX_FORM`], as every
/// form given to a `LIKE` is. A longer one names no submodel, and some
/// engines refuse a long pattern: Cloudflare D1 allows 50 bytes.
fn joined(words: &[String]) -> String {
    let mut form = words.concat();
    // A matching form is ASCII, so any byte offset is a character boundary.
    form.truncate(MAX_FORM);
    form
}

struct Lookup<'a> {
    make: Option<&'a MakeRef>,
    /// Words the make accounts for.
    base: usize,
    year: Option<u16>,
    light: i64,
    bit: i64,
}

impl<S: Source> Catalog<S> {
    /// Catalog entries for free text such as `2019 civic si`, `chevy 1500`
    /// or `f150`, best first. Text that names nothing gives no entries.
    ///
    /// Without a year, each model is given in the newest model year that
    /// fits.
    pub fn search(
        &self,
        text: &str,
        scope: Scope,
        limit: usize,
    ) -> Result<Vec<Entry>, CatalogError> {
        let Some((light, bit)) = scope.bits() else {
            return Ok(Vec::new());
        };
        let first = parse(text, &self.index, scope, self.years, true);
        let mut hits = self.hits(&first, light, bit)?;
        if hits.is_empty() && first.year.is_some() {
            // A number in the catalog's years may be a model, as in `pontiac 2000`.
            let again = parse(text, &self.index, scope, self.years, false);
            hits = self.hits(&again, light, bit)?;
        }
        hits.sort_by(|a, b| {
            (
                Reverse(a.explained),
                Reverse(a.exact),
                a.rank.is_none(),
                a.rank,
                Reverse(a.year),
                &a.id,
            )
                .cmp(&(
                    Reverse(b.explained),
                    Reverse(b.exact),
                    b.rank.is_none(),
                    b.rank,
                    Reverse(b.year),
                    &b.id,
                ))
        });
        let mut seen = HashSet::new();
        let mut entries = Vec::new();
        for hit in hits {
            if entries.len() >= limit.min(MOST_HITS) {
                break;
            }
            if seen.insert(hit.id.clone())
                && let Some(entry) = self.entry(&hit.id)?
            {
                entries.push(entry);
            }
        }
        Ok(entries)
    }

    fn hits(&self, parsed: &Parsed, light: i64, bit: i64) -> Result<Vec<Hit>, CatalogError> {
        let mut hits = Vec::new();
        if let Some(make) = parsed.make.and_then(|id| self.index.get(id)) {
            let lookup = Lookup {
                make: Some(make),
                base: parsed.all_words.len().saturating_sub(parsed.words.len()),
                year: parsed.year,
                light,
                bit,
            };
            self.look(&lookup, &parsed.words, &mut hits)?;
        }
        // The same words may also name a model with no make given.
        let lookup = Lookup {
            make: None,
            base: 0,
            year: parsed.year,
            light,
            bit,
        };
        self.look(&lookup, &parsed.all_words, &mut hits)?;
        Ok(hits)
    }

    fn look(
        &self,
        lookup: &Lookup<'_>,
        words: &[String],
        hits: &mut Vec<Hit>,
    ) -> Result<(), CatalogError> {
        const TABLE: &str = "catalog_model";
        let year_value: Value = lookup.year.map(i64::from).into();
        let make_value: Value = lookup.make.map(|make| make.id).into();

        if words.is_empty() {
            // A make and nothing else: its models.
            let Some(make) = lookup.make else {
                return Ok(());
            };
            let rows = self.query(
                sql::SEARCH_MAKE_MODELS,
                &[
                    make.id.into(),
                    year_value,
                    lookup.light.into(),
                    lookup.bit.into(),
                    PER_LOOKUP.into(),
                ],
            )?;
            for row in &rows {
                hits.push(Hit {
                    explained: lookup.base,
                    exact: false,
                    rank: make.rank,
                    year: year(row, 2, TABLE)?,
                    id: VehicleId {
                        year: year(row, 2, TABLE)?,
                        make: make.slug.clone(),
                        model: text(row, 1, TABLE)?,
                        submodel: None,
                        engine: None,
                    }
                    .to_string(),
                });
            }
            return Ok(());
        }

        // The model is the longest run of leading words that names one.
        // Whatever follows it is tried as a submodel.
        let mut found_model = false;
        for count in (1..=words.len().min(MODEL_WORDS)).rev() {
            let (head, rest) = words.split_at(count);
            let form = head.concat();
            // Only the whole text may match the start of a name.
            let statement = if rest.is_empty() {
                sql::SEARCH_MODELS_PREFIX
            } else {
                sql::SEARCH_MODELS
            };
            let rows = self.query(
                statement,
                &[
                    form.as_str().into(),
                    make_value.clone(),
                    year_value.clone(),
                    lookup.light.into(),
                    lookup.bit.into(),
                    PER_LOOKUP.into(),
                ],
            )?;
            for row in &rows {
                let Some(make) = self.index.get(integer(row, 0, TABLE)?) else {
                    continue;
                };
                found_model = true;
                let exact = integer(row, 3, TABLE)? != 0;
                let model = VehicleId {
                    year: year(row, 2, TABLE)?,
                    make: make.slug.clone(),
                    model: text(row, 1, TABLE)?,
                    submodel: None,
                    engine: None,
                };
                hits.push(Hit {
                    explained: lookup.base + count,
                    exact,
                    rank: make.rank,
                    year: model.year,
                    id: model.to_string(),
                });
                if rest.is_empty() {
                    continue;
                }
                let submodels = self.query(
                    sql::SEARCH_SUBMODELS,
                    &[
                        integer(row, 4, TABLE)?.into(),
                        joined(rest).as_str().into(),
                        year_value.clone(),
                        lookup.light.into(),
                        lookup.bit.into(),
                        SUBMODELS_PER_MODEL.into(),
                    ],
                )?;
                for submodel in &submodels {
                    const TABLE: &str = "catalog_submodel";
                    let name = slug(&text(submodel, 0, TABLE)?);
                    if name.is_empty() {
                        continue;
                    }
                    hits.push(Hit {
                        explained: lookup.base + words.len(),
                        exact: exact && integer(submodel, 2, TABLE)? != 0,
                        rank: make.rank,
                        year: year(submodel, 1, TABLE)?,
                        id: VehicleId {
                            year: year(submodel, 1, TABLE)?,
                            submodel: Some(name),
                            ..model.clone()
                        }
                        .to_string(),
                    });
                }
            }
        }

        // `chevy 1500`: no Chevrolet model is called 1500, but a series is.
        if let (false, Some(make)) = (found_model, lookup.make) {
            const TABLE: &str = "catalog_submodel";
            let rows = self.query(
                sql::SEARCH_MAKE_SUBMODELS,
                &[
                    make.id.into(),
                    joined(words).as_str().into(),
                    year_value,
                    lookup.light.into(),
                    lookup.bit.into(),
                    PER_LOOKUP.into(),
                ],
            )?;
            for row in &rows {
                let name = slug(&text(row, 1, TABLE)?);
                if name.is_empty() {
                    continue;
                }
                hits.push(Hit {
                    explained: lookup.base + words.len(),
                    exact: integer(row, 3, TABLE)? != 0,
                    rank: make.rank,
                    year: year(row, 2, TABLE)?,
                    id: VehicleId {
                        year: year(row, 2, TABLE)?,
                        make: make.slug.clone(),
                        model: text(row, 0, TABLE)?,
                        submodel: Some(name),
                        engine: None,
                    }
                    .to_string(),
                });
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn words_joined_for_a_like_are_cut_to_the_longest_form() {
        assert_eq!(joined(&["si".to_owned()]), "si");
        assert_eq!(joined(&["crew".to_owned(), "cab".to_owned()]), "crewcab");
        let long = vec!["a".repeat(40); 5];
        assert_eq!(joined(&long).len(), MAX_FORM);
        assert_eq!(joined(&[]), "");
    }
}
