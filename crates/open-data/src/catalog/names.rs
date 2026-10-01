//! One spelling for each trim and series name.

use std::collections::{BTreeMap, HashMap};

use wenmar_vehicles::text::{slug, squeeze};

/// Values that say a trim is not known, compared in lowercase with any
/// surrounding parentheses removed.
const PLACEHOLDERS: [&str; 6] = [
    "not identified",
    "n/a",
    "na",
    "unknown",
    "not applicable",
    "none",
];

/// Whether a name says nothing: a placeholder, or no letter or digit at all.
pub fn is_placeholder(name: &str) -> bool {
    let bare = name.trim().trim_matches(['(', ')']).trim().to_lowercase();
    PLACEHOLDERS.contains(&bare.as_str()) || slug(name).is_empty()
}

/// The trims in one vPIC value. Some manufacturers list several, as in
/// `S, SE, SEL`.
pub fn split_trims(raw: &str) -> Vec<String> {
    raw.split(',')
        .map(squeeze)
        .filter(|part| !part.is_empty())
        .collect()
}

/// Whether a name is written all in capitals with a word of four letters or
/// more, such as `TOURING`. Short codes such as `EX-L` and `XLT` are not.
pub fn shouting(name: &str) -> bool {
    let mut run = 0;
    let mut longest = 0;
    for character in name.chars() {
        if character.is_ascii_lowercase() {
            return false;
        }
        run = if character.is_ascii_uppercase() {
            run + 1
        } else {
            0
        };
        longest = longest.max(run);
    }
    longest >= 4
}

fn mixed_case(name: &str) -> bool {
    name.chars().any(|character| character.is_ascii_lowercase())
        && name.chars().any(|character| character.is_ascii_uppercase())
}

/// Chooses one spelling per name.
#[derive(Debug, Clone, Default)]
pub struct Speller {
    /// Lowercase spelling to the spelling to show.
    preferred: HashMap<String, String>,
    /// How many patterns carry each exact spelling.
    counts: HashMap<String, u64>,
    overrides: BTreeMap<String, String>,
}

impl Speller {
    /// `counted` is every trim (already split) and series value with the
    /// number of patterns that carry it. `overrides` maps a lowercase
    /// spelling to its replacement.
    pub fn new(counted: &[(String, u64)], overrides: &BTreeMap<String, String>) -> Speller {
        let mut counts: HashMap<String, u64> = HashMap::new();
        for (name, count) in counted {
            let name = squeeze(name);
            if !is_placeholder(&name) {
                *counts.entry(name).or_default() += count;
            }
        }
        let mut preferred: HashMap<String, String> = HashMap::new();
        for name in counts.keys() {
            let current = preferred
                .entry(name.to_lowercase())
                .or_insert_with(|| name.clone());
            if order(name, &counts) < order(current, &counts) {
                *current = name.clone();
            }
        }
        Speller {
            preferred,
            counts,
            overrides: overrides.clone(),
        }
    }

    /// The spelling to show for a raw value, or `None` when it names nothing.
    pub fn tidy(&self, raw: &str) -> Option<String> {
        let name = squeeze(raw);
        if is_placeholder(&name) {
            return None;
        }
        let lowercase = name.to_lowercase();
        if let Some(replacement) = self.overrides.get(&lowercase) {
            return Some(replacement.clone());
        }
        Some(self.preferred.get(&lowercase).cloned().unwrap_or(name))
    }

    /// Of two names with the same id form, the one to keep: the spelling
    /// more patterns carry, then the one that sorts first.
    pub fn better<'a>(&self, first: &'a str, second: &'a str) -> &'a str {
        let count = |name: &str| self.counts.get(name).copied().unwrap_or(0);
        if (std::cmp::Reverse(count(second)), second) < (std::cmp::Reverse(count(first)), first) {
            second
        } else {
            first
        }
    }
}

/// Sorts spellings of one name, best first: not shouting, carried by more
/// patterns, mixed case, then text order.
fn order<'a>(
    name: &'a str,
    counts: &HashMap<String, u64>,
) -> (bool, std::cmp::Reverse<u64>, bool, &'a str) {
    (
        shouting(name),
        std::cmp::Reverse(counts.get(name).copied().unwrap_or(0)),
        !mixed_case(name),
        name,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn speller() -> Speller {
        let counted: Vec<(String, u64)> = [
            ("Touring", 3),
            ("TOURING", 9),
            ("EX-L", 9),
            ("Ex-L", 1),
            ("Si", 4),
            ("SI", 2),
            ("LX", 1),
            ("lx", 1),
            ("LARIAT", 2),
            ("TRAILSPORT", 29),
            ("TrailSport", 19),
            ("Trailsport", 1),
            ("  Sport   Touring ", 2),
            ("(not identified)", 50),
        ]
        .into_iter()
        .map(|(name, count)| (name.to_owned(), count))
        .collect();
        let overrides = BTreeMap::from([("si/si hpt".to_owned(), "Si".to_owned())]);
        Speller::new(&counted, &overrides)
    }

    #[test]
    fn spellings_that_differ_only_in_case_become_one() {
        let speller = speller();
        // A shouted spelling loses even when more patterns carry it.
        assert_eq!(speller.tidy("TOURING").as_deref(), Some("Touring"));
        assert_eq!(speller.tidy("touring").as_deref(), Some("Touring"));
        assert_eq!(speller.tidy("TRAILSPORT").as_deref(), Some("TrailSport"));
        // Neither shouts, so the spelling more patterns carry wins.
        assert_eq!(speller.tidy("Ex-L").as_deref(), Some("EX-L"));
        assert_eq!(speller.tidy("SI").as_deref(), Some("Si"));
        // A tie goes to text order.
        assert_eq!(speller.tidy("lx").as_deref(), Some("LX"));
    }

    #[test]
    fn a_name_with_one_spelling_is_left_alone() {
        let speller = speller();
        assert_eq!(speller.tidy("LARIAT").as_deref(), Some("LARIAT"));
        assert_eq!(speller.tidy("Never  Seen ").as_deref(), Some("Never Seen"));
        assert_eq!(
            speller.tidy("sport touring").as_deref(),
            Some("Sport Touring")
        );
    }

    #[test]
    fn an_override_wins() {
        assert_eq!(speller().tidy("Si/Si  HPT").as_deref(), Some("Si"));
        assert_eq!(speller().tidy("SI/SI HPT").as_deref(), Some("Si"));
    }

    #[test]
    fn placeholders_name_nothing() {
        let speller = speller();
        for raw in [
            "",
            "   ",
            "(not identified)",
            "Not Identified",
            "N/A",
            "NA",
            "Unknown",
            "Not Applicable",
            "none",
            "---",
            "*",
        ] {
            assert_eq!(speller.tidy(raw), None, "{raw:?}");
        }
        assert_eq!(speller.tidy("Nano").as_deref(), Some("Nano"));
    }

    #[test]
    fn a_list_of_trims_is_split_at_commas_only() {
        assert_eq!(split_trims("S, SE,SEL"), vec!["S", "SE", "SEL"]);
        assert_eq!(
            split_trims("Big Horn, Lonestar"),
            vec!["Big Horn", "Lonestar"]
        );
        assert_eq!(split_trims("Si/Si HPT"), vec!["Si/Si HPT"]);
        assert_eq!(split_trims(" , ,"), Vec::<String>::new());
    }

    #[test]
    fn shouting_needs_a_long_word_and_no_lowercase() {
        for name in ["TOURING", "SPORT TOURING", "LARIAT", "EX-L NAVI"] {
            assert!(shouting(name), "{name}");
        }
        for name in ["EX-L", "XLT", "SXT", "Touring", "TrailSport", "4X2", ""] {
            assert!(!shouting(name), "{name}");
        }
    }

    #[test]
    fn of_two_names_with_one_id_form_the_commoner_is_kept() {
        let speller = speller();
        assert_eq!(speller.better("Ex-L", "EX-L"), "EX-L");
        assert_eq!(speller.better("EX-L", "Ex-L"), "EX-L");
        // Unknown to the speller: text order.
        assert_eq!(speller.better("GT Line", "GT-Line"), "GT Line");
        assert_eq!(speller.better("GT-Line", "GT Line"), "GT Line");
    }
}
