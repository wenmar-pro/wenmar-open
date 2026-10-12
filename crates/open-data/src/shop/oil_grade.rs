//! Reads an engine oil viscosity grade out of the loose text automakers
//! give EPA: `0W20`, `SAE 0W-20`, `ow-20`, `5W/30, VW50400`.

use std::collections::HashMap;
use std::path::Path;

use anyhow::{Context, Result, bail};
use serde::Deserialize;
use wenmar_vehicles::text::squeeze;

/// The winter grades of SAE J300, as numbers before the `W`. Longest
/// first, so `10W` is not read as `0W`.
const WINTER: [&str; 6] = ["10", "15", "20", "25", "0", "5"];

/// The grades after the `W`. Longest first.
const SUMMER: [&str; 8] = ["12", "16", "20", "30", "40", "50", "60", "8"];

/// Every grade the pattern finds in `text`, in the order written, each
/// once, as `0W-20`.
///
/// A grade is: a winter number (`0`, `5`, `10`, `15`, `20`, `25`, or the
/// letter `O` standing for zero) that does not follow a digit; `W`; then
/// at most one `-` or `/`; then `8`, `12`, `16`, `20`, `30`, `40`, `50` or
/// `60` not followed by a digit. Blanks may stand between the parts, and
/// letter case does not matter. The letter `O` counts only when it does
/// not follow a letter or a digit.
pub fn grades_in(text: &str) -> Vec<String> {
    let upper: Vec<char> = text.to_ascii_uppercase().chars().collect();
    let starts_with = |at: usize, word: &str| {
        let word: Vec<char> = word.chars().collect();
        upper.len() >= at + word.len() && upper[at..at + word.len()] == word[..]
    };
    let blanks = |mut at: usize| {
        while at < upper.len() && upper[at] == ' ' {
            at += 1;
        }
        at
    };
    let mut grades: Vec<String> = Vec::new();
    let mut at = 0;
    while at < upper.len() {
        let previous = at.checked_sub(1).map(|index| upper[index]);
        let after_digit = previous.is_some_and(|character| character.is_ascii_digit());
        let after_letter = previous.is_some_and(|character| character.is_ascii_alphabetic());
        let winter = if after_digit {
            None
        } else if let Some(number) = WINTER.iter().find(|number| starts_with(at, number)) {
            Some((*number, number.len()))
        } else if upper[at] == 'O' && !after_letter {
            Some(("0", 1))
        } else {
            None
        };
        let Some((winter, length)) = winter else {
            at += 1;
            continue;
        };
        let mut next = blanks(at + length);
        if !starts_with(next, "W") {
            at += 1;
            continue;
        }
        next = blanks(next + 1);
        if starts_with(next, "-") || starts_with(next, "/") {
            next = blanks(next + 1);
        }
        let summer = SUMMER.iter().find(|number| {
            starts_with(next, number)
                && !upper
                    .get(next + number.len())
                    .is_some_and(|character| character.is_ascii_digit())
        });
        let Some(summer) = summer else {
            at += 1;
            continue;
        };
        let grade = format!("{winter}W-{summer}");
        if !grades.contains(&grade) {
            grades.push(grade);
        }
        at = next + summer.len();
    }
    grades
}

/// Whether `grade` is written the standard way, such as `5W-30`.
pub fn is_grade(grade: &str) -> bool {
    grade
        .split_once("W-")
        .is_some_and(|(winter, summer)| WINTER.contains(&winter) && SUMMER.contains(&summer))
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct Entry {
    text: String,
    grades: Vec<String>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct File {
    grades: Vec<Entry>,
}

/// The form two spellings are compared in: single blanks, upper case.
fn form(text: &str) -> String {
    squeeze(text).to_ascii_uppercase()
}

/// `shop/oil-grades.yaml`: the spellings the pattern cannot read, each
/// with the grades it means. An entry with no grades says the text names
/// no grade.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct CuratedOil {
    entries: HashMap<String, Vec<String>>,
}

impl CuratedOil {
    /// Reads `shop/oil-grades.yaml` under `directory`.
    pub fn load(directory: &Path) -> Result<CuratedOil> {
        let path = directory.join("shop/oil-grades.yaml");
        let text = std::fs::read_to_string(&path)
            .with_context(|| format!("reading {}", path.display()))?;
        CuratedOil::parse(&text)
    }

    /// Reads the text of the file and checks it.
    pub fn parse(yaml: &str) -> Result<CuratedOil> {
        let file: File = serde_saphyr::from_str(yaml).context("reading oil-grades.yaml")?;
        let mut entries = HashMap::new();
        for entry in file.grades {
            let key = form(&entry.text);
            if key.is_empty() {
                bail!("oil-grades.yaml: an entry has no text");
            }
            if let Some(grade) = entry.grades.iter().find(|grade| !is_grade(grade)) {
                bail!(
                    "oil-grades.yaml: {grade:?} for {:?} is not a grade like 5W-30",
                    entry.text
                );
            }
            if !grades_in(&entry.text).is_empty() {
                bail!(
                    "oil-grades.yaml: the pattern already reads {:?}; remove the entry",
                    entry.text
                );
            }
            if entries.insert(key, entry.grades).is_some() {
                bail!("oil-grades.yaml lists {:?} twice", entry.text);
            }
        }
        Ok(CuratedOil { entries })
    }

    /// The grades the file gives for a spelling, if it lists it.
    pub fn get(&self, text: &str) -> Option<&[String]> {
        self.entries.get(&form(text)).map(Vec::as_slice)
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }
}

/// The grades of one spelling: what the pattern finds, else what the
/// curated file says. `None` when neither knows the spelling.
pub fn read(text: &str, curated: &CuratedOil) -> Option<Vec<String>> {
    let found = grades_in(text);
    if !found.is_empty() {
        return Some(found);
    }
    curated.get(text).map(<[String]>::to_vec)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_spellings_in_the_real_files_are_read() {
        for (text, grades) in [
            ("0W20", vec!["0W-20"]),
            ("0W-20", vec!["0W-20"]),
            ("SAE 0W-20", vec!["0W-20"]),
            ("SAE 0W20", vec!["0W-20"]),
            ("SAE5W40", vec!["5W-40"]),
            ("SAE5W-30", vec!["5W-30"]),
            ("ow-20", vec!["0W-20"]),
            ("OW-20", vec!["0W-20"]),
            ("0w-40 SN+", vec!["0W-40"]),
            ("5W/40", vec!["5W-40"]),
            ("0W/20", vec!["0W-20"]),
            ("5W20 GF4", vec!["5W-20"]),
            ("GF-5 0W-20", vec!["0W-20"]),
            ("GF-4 5W-20", vec!["5W-20"]),
            ("5W40 VW 50200", vec!["5W-40"]),
            ("0W20 VW50800", vec!["0W-20"]),
            ("0W40/VW50200/VW50500", vec!["0W-40"]),
            ("5W30 / 225.10", vec!["5W-30"]),
            ("5W30 ACEA A5/B5", vec!["5W-30"]),
            ("SAE 5W-30 / API FA-4", vec!["5W-30"]),
            ("0W-20 (GF-6A)", vec!["0W-20"]),
            ("10W-60", vec!["10W-60"]),
            ("10W40", vec!["10W-40"]),
            ("0W-16", vec!["0W-16"]),
            ("0W-8", vec!["0W-8"]),
            ("0W-12", vec!["0W-12"]),
            ("5W-30 and 0W-40", vec!["5W-30", "0W-40"]),
            ("0W-16,0W-8", vec!["0W-16", "0W-8"]),
            ("5W-30 (BASE), 5W-50 (HO)", vec!["5W-30", "5W-50"]),
            ("5W-20/0W-20", vec!["5W-20", "0W-20"]),
            ("5W-30:IS 350C,0W-20:other", vec!["5W-30", "0W-20"]),
            ("0W-8(CROWN),0W-16(OTHERS)", vec!["0W-8", "0W-16"]),
            ("0W-20, 5W-20/0W-20", vec!["0W-20", "5W-20"]),
            ("5W-20, 0W-20, GF-5", vec!["5W-20", "0W-20"]),
            ("5 W - 30", vec!["5W-30"]),
        ] {
            assert_eq!(grades_in(text), grades, "{text}");
        }
    }

    #[test]
    fn what_is_not_a_grade_is_not_read() {
        for text in [
            "",
            "10.0",
            "LV",
            "low viscosity",
            "DEXTOS 2",
            "SW30",
            "5VW40 VW50501",
            "VW50200",
            "5W-200",
            "5W-35",
            "30W-30",
            "105W-30",
            "LOW-20",
            "GF-5",
            "W-30",
        ] {
            assert!(grades_in(text).is_empty(), "{text}");
        }
        assert!(is_grade("0W-20") && is_grade("25W-60") && is_grade("0W-8"));
        assert!(!is_grade("0W20") && !is_grade("OW-20") && !is_grade("5W-35") && !is_grade(""));
    }

    #[test]
    fn the_curated_file_fills_in_and_is_checked() {
        let curated = CuratedOil::parse(
            "grades:\n  - text: \"SW30\"\n    grades: [\"5W-30\"]\n  - text: \"low  viscosity\"\n    grades: []\n",
        )
        .unwrap();
        assert_eq!(curated.len(), 2);
        assert_eq!(read("sw30", &curated), Some(vec!["5W-30".to_owned()]));
        assert_eq!(read("Low Viscosity", &curated), Some(vec![]));
        assert_eq!(read("0W20", &curated), Some(vec!["0W-20".to_owned()]));
        assert_eq!(read("thin", &curated), None);
        for (yaml, expected) in [
            ("grades:\n  - text: ''\n    grades: []\n", "has no text"),
            (
                "grades:\n  - text: SW30\n    grades: ['5W30']\n",
                "is not a grade",
            ),
            (
                "grades:\n  - text: 0W20\n    grades: ['0W-20']\n",
                "already reads",
            ),
            (
                "grades:\n  - text: SW30\n    grades: []\n  - text: sw30\n    grades: []\n",
                "twice",
            ),
            (
                "grades:\n  - txt: SW30\n    grades: []\n",
                "oil-grades.yaml",
            ),
        ] {
            let error = CuratedOil::parse(yaml).unwrap_err();
            assert!(format!("{error:#}").contains(expected), "{error:#}");
        }
    }

    #[test]
    fn the_file_in_the_repository_loads() {
        let directory = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../data");
        let curated = CuratedOil::load(&directory).unwrap();
        assert_eq!(
            read("5VW40 VW50501", &curated),
            Some(vec!["5W-40".to_owned()])
        );
        assert_eq!(read("DEXTOS 2", &curated), Some(vec![]));
    }
}
