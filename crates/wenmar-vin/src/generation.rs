//! Which generation of a model a model year belongs to.
//!
//! vPIC has no generation: NHTSA records what a vehicle is, not where it
//! falls in its maker's redesign cycle. The table is corgi-rs's
//! `tools/generations.tsv` (MIT; see NOTICE.md), kept as published in
//! `generations.tsv` beside this crate's manifest. It is keyed by the make
//! and model names vPIC uses. A model it does not list has no generation:
//! nothing is guessed.

use std::collections::HashMap;
use std::sync::OnceLock;

use serde::Serialize;

const TABLE: &str = include_str!("../generations.tsv");

/// One generation of a model.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[non_exhaustive]
pub struct Generation {
    /// How the generation is known, such as `8th generation` or `XV40`.
    pub name: String,
    /// Its number, when it has one that is commonly used.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ordinal: Option<u8>,
    /// The maker's platform or chassis code, when there is one.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub code: Option<String>,
    /// The first model year of the generation.
    pub year_from: u16,
    /// The last model year, or `None` while it is still made.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub year_to: Option<u16>,
}

type Table = HashMap<(String, String), Vec<Generation>>;

fn key(make: &str, model: &str) -> (String, String) {
    (make.trim().to_lowercase(), model.trim().to_lowercase())
}

fn table() -> &'static Table {
    static PARSED: OnceLock<Table> = OnceLock::new();
    PARSED.get_or_init(|| parse(TABLE))
}

/// Reads the table. A row that is not well formed is left out.
fn parse(text: &str) -> Table {
    let mut parsed: Table = HashMap::new();
    for line in text.lines() {
        if line.starts_with('#') || line.trim().is_empty() {
            continue;
        }
        let columns: Vec<&str> = line.split('\t').collect();
        let [make, model, from, to, ordinal, code, name] = columns[..] else {
            continue;
        };
        let (Ok(year_from), Ok(to), Ok(ordinal)) = (
            from.parse::<u16>(),
            to.parse::<u16>(),
            ordinal.parse::<u8>(),
        ) else {
            continue;
        };
        let code = code.trim();
        parsed
            .entry(key(make, model))
            .or_default()
            .push(Generation {
                name: name.trim().to_owned(),
                ordinal: (ordinal != 0).then_some(ordinal),
                code: (!code.is_empty()).then(|| code.to_owned()),
                year_from,
                year_to: (to != 0).then_some(to),
            });
    }
    parsed
}

/// The generation a model year of a make and model belongs to, by vPIC's
/// names for them.
pub fn find(make: &str, model: &str, year: u16) -> Option<Generation> {
    table()
        .get(&key(make, model))?
        .iter()
        .find(|generation| {
            generation.year_from <= year && generation.year_to.is_none_or(|to| year <= to)
        })
        .cloned()
}

/// Every generation listed for a make and model, oldest first.
pub fn of_model(make: &str, model: &str) -> Vec<Generation> {
    let mut all = table().get(&key(make, model)).cloned().unwrap_or_default();
    all.sort_by_key(|generation| generation.year_from);
    all
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_row_of_the_table_is_read() {
        let rows = TABLE
            .lines()
            .filter(|line| !line.starts_with('#') && !line.trim().is_empty())
            .count();
        let read: usize = table().values().map(Vec::len).sum();
        assert_eq!(read, rows);
    }

    #[test]
    fn generations_of_one_model_do_not_overlap() {
        for ((make, model), list) in table() {
            let mut list = list.clone();
            list.sort_by_key(|generation| generation.year_from);
            for pair in list.windows(2) {
                let end = pair[0].year_to.unwrap_or(u16::MAX);
                assert!(end < pair[1].year_from, "{make} {model}: {pair:?}");
            }
        }
    }

    #[test]
    fn a_model_year_finds_its_generation() {
        let accord = find("Honda", "Accord", 2009).expect("the 2009 Accord is listed");
        assert_eq!(accord.ordinal, Some(8));
        assert_eq!(accord.year_from, 2008);
        assert_eq!(accord.year_to, Some(2012));
        assert_eq!(find(" honda ", "ACCORD", 2009), Some(accord));
        assert_eq!(find("Honda", "Accord", 1900), None);
        assert_eq!(find("Nobody", "Nothing", 2020), None);
    }
}
