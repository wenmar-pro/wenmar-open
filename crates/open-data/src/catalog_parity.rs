//! Compares the catalog's models with NHTSA's recorded answers.
//!
//! NHTSA's `GetModelsForMakeYear` is the list the product's old sync job
//! downloaded make by make. The catalog derives the same list offline, and
//! this check holds it to NHTSA's own answers for a sample of makes and
//! years.

use std::collections::BTreeMap;
use std::fmt::Write as _;

use serde::{Deserialize, Serialize};
use wenmar_vehicles::text::squeeze;
use wenmar_vehicles::{Catalog, CatalogError, Scope, Source};

/// NHTSA's answers, as recorded by `tools/catalog/record.py`.
#[derive(Debug, Clone, Deserialize)]
pub struct Fixtures {
    pub answers: Vec<Answer>,
}

/// The models NHTSA lists for one make and model year.
#[derive(Debug, Clone, Deserialize)]
pub struct Answer {
    pub make: String,
    pub year: u16,
    pub models: Vec<String>,
}

/// How the catalog compared. The differing models are kept by name, so a
/// change that fixes one and breaks another cannot hide behind equal totals.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Report {
    /// Make-years compared.
    pub pairs: u32,
    /// Models both list.
    pub agree: u32,
    /// Models NHTSA lists and the catalog does not, as `2019 Honda: Accord`.
    pub missing: Vec<String>,
    /// Models the catalog lists and NHTSA does not.
    pub extra: Vec<String>,
}

/// The most models one make-year is asked for.
const MOST_MODELS: usize = 500;

/// Names are compared without regard to case or stray spaces: NHTSA's live
/// API and its monthly release differ in capitalisation.
fn key(name: &str) -> String {
    squeeze(name).to_lowercase()
}

/// Adds one make-year to the report. `ours` is the catalog's models.
pub fn compare(answer: &Answer, ours: &[String], report: &mut Report) {
    let theirs: BTreeMap<String, &str> = answer
        .models
        .iter()
        .map(|name| (key(name), name.as_str()))
        .collect();
    let ours: BTreeMap<String, &str> = ours.iter().map(|name| (key(name), name.as_str())).collect();
    let label = |name: &str| format!("{} {}: {}", answer.year, answer.make, squeeze(name));
    report.pairs += 1;
    for (key, name) in &theirs {
        if ours.contains_key(key) {
            report.agree += 1;
        } else {
            report.missing.push(label(name));
        }
    }
    for (key, name) in &ours {
        if !theirs.contains_key(key) {
            report.extra.push(label(name));
        }
    }
}

/// Compares every recorded make-year with the catalog. Every vehicle type
/// counts, as in NHTSA's answer.
pub fn run<S: Source>(catalog: &Catalog<S>, fixtures: &Fixtures) -> Result<Report, CatalogError> {
    let mut report = Report::default();
    for answer in &fixtures.answers {
        let ours: Vec<String> = catalog
            .models(&answer.make, Some(answer.year), Scope::All, "", MOST_MODELS)?
            .into_iter()
            .map(|model| model.name)
            .collect();
        compare(answer, &ours, &mut report);
    }
    report.missing.sort();
    report.extra.sort();
    Ok(report)
}

fn newly(now: &[String], was: &[String]) -> Vec<String> {
    now.iter()
        .filter(|entry| !was.contains(entry))
        .cloned()
        .collect()
}

fn sample(entries: &[String]) -> String {
    const SHOWN: usize = 5;
    let mut text = entries
        .iter()
        .take(SHOWN)
        .cloned()
        .collect::<Vec<_>>()
        .join("; ");
    if entries.len() > SHOWN {
        let _ = write!(text, "; and {} more", entries.len() - SHOWN);
    }
    text
}

/// What got worse since the baseline. Empty when nothing did.
pub fn regressions(report: &Report, baseline: &Report) -> Vec<String> {
    let mut problems = Vec::new();
    if report.pairs != baseline.pairs {
        problems.push(format!(
            "{} make-years were compared and the baseline has {}; record a new baseline",
            report.pairs, baseline.pairs
        ));
    }
    let missing = newly(&report.missing, &baseline.missing);
    if !missing.is_empty() {
        problems.push(format!(
            "{} models NHTSA lists are newly missing: {}",
            missing.len(),
            sample(&missing)
        ));
    }
    let extra = newly(&report.extra, &baseline.extra);
    if !extra.is_empty() {
        problems.push(format!(
            "{} models are newly listed that NHTSA does not list: {}",
            extra.len(),
            sample(&extra)
        ));
    }
    problems
}

/// The report as text.
pub fn table(report: &Report) -> String {
    let mut text = String::new();
    let _ = writeln!(
        text,
        "catalog models against NHTSA, {} make-years",
        report.pairs
    );
    let _ = writeln!(text, "  agree    {}", report.agree);
    let _ = writeln!(
        text,
        "  missing  {}  (NHTSA lists it, the catalog does not)",
        report.missing.len()
    );
    let _ = writeln!(
        text,
        "  extra    {}  (the catalog lists it, NHTSA does not)",
        report.extra.len()
    );
    for (title, entries) in [("missing", &report.missing), ("extra", &report.extra)] {
        for entry in entries.iter().take(20) {
            let _ = writeln!(text, "  {title}: {entry}");
        }
        if entries.len() > 20 {
            let _ = writeln!(text, "  {title}: and {} more", entries.len() - 20);
        }
    }
    text
}

#[cfg(test)]
mod tests {
    use super::*;

    fn answer(make: &str, year: u16, models: &[&str]) -> Answer {
        Answer {
            make: make.to_owned(),
            year,
            models: models.iter().map(|name| (*name).to_owned()).collect(),
        }
    }

    fn list(names: &[&str]) -> Vec<String> {
        names.iter().map(|name| (*name).to_owned()).collect()
    }

    #[test]
    fn models_are_compared_without_regard_to_case_or_spaces() {
        let mut report = Report::default();
        compare(
            &answer("Honda", 2019, &["Civic", "AFRICA TWIN", " CR-V ", "Accord"]),
            &list(&["Civic", "Africa Twin", "CR-V", "Fit"]),
            &mut report,
        );
        assert_eq!(
            report,
            Report {
                pairs: 1,
                agree: 3,
                missing: list(&["2019 Honda: Accord"]),
                extra: list(&["2019 Honda: Fit"]),
            }
        );
    }

    #[test]
    fn punctuation_still_counts() {
        let mut report = Report::default();
        compare(
            &answer("Honda", 2019, &["CR-V"]),
            &list(&["CRV"]),
            &mut report,
        );
        assert_eq!(
            (report.agree, report.missing.len(), report.extra.len()),
            (0, 1, 1)
        );
    }

    #[test]
    fn a_make_year_with_no_models_on_either_side_agrees() {
        let mut report = Report::default();
        compare(&answer("Pontiac", 2020, &[]), &[], &mut report);
        assert_eq!(
            report,
            Report {
                pairs: 1,
                ..Report::default()
            }
        );
    }

    fn report(pairs: u32, missing: &[&str], extra: &[&str]) -> Report {
        Report {
            pairs,
            agree: 10,
            missing: list(missing),
            extra: list(extra),
        }
    }

    #[test]
    fn a_new_difference_is_a_regression_and_a_known_one_is_not() {
        let baseline = report(2, &["2019 Honda: Accord"], &[]);
        assert!(regressions(&baseline, &baseline).is_empty());
        // One fewer difference is an improvement.
        assert!(regressions(&report(2, &[], &[]), &baseline).is_empty());

        let worse = report(
            2,
            &["2019 Honda: Accord", "2019 Honda: Fit"],
            &["2019 Ford: Pinto"],
        );
        assert_eq!(
            regressions(&worse, &baseline),
            vec![
                "1 models NHTSA lists are newly missing: 2019 Honda: Fit",
                "1 models are newly listed that NHTSA does not list: 2019 Ford: Pinto",
            ]
        );
    }

    #[test]
    fn swapping_one_difference_for_another_is_still_a_regression() {
        let baseline = report(2, &["2019 Honda: Accord"], &[]);
        let swapped = report(2, &["2019 Honda: Fit"], &[]);
        assert_eq!(regressions(&swapped, &baseline).len(), 1);
    }

    #[test]
    fn a_fixture_of_another_size_needs_a_new_baseline() {
        let problems = regressions(&report(3, &[], &[]), &report(2, &[], &[]));
        assert_eq!(
            problems,
            vec!["3 make-years were compared and the baseline has 2; record a new baseline"]
        );
    }

    #[test]
    fn long_lists_are_cut_short_in_messages() {
        let many: Vec<String> = (0..8)
            .map(|index| format!("2019 Honda: M{index}"))
            .collect();
        let now = Report {
            pairs: 2,
            agree: 0,
            missing: many,
            extra: Vec::new(),
        };
        let problems = regressions(&now, &report(2, &[], &[]));
        assert!(
            problems[0].starts_with("8 models NHTSA lists are newly missing: 2019 Honda: M0; ")
        );
        assert!(problems[0].ends_with("; and 3 more"), "{}", problems[0]);
    }

    #[test]
    fn the_table_shows_counts_and_differences() {
        let text = table(&report(2, &["2019 Honda: Accord"], &[]));
        assert!(text.contains("2 make-years"), "{text}");
        assert!(text.contains("agree    10"), "{text}");
        assert!(text.contains("missing  1"), "{text}");
        assert!(text.contains("missing: 2019 Honda: Accord"), "{text}");
    }

    #[test]
    fn reads_the_recorded_file() {
        let fixtures: Fixtures = serde_json::from_str(
            r#"{"recorded": "2026-10-01", "source": "https://example.invalid",
                "answers": [{"make": "Honda", "year": 2019, "models": ["Civic"]}]}"#,
        )
        .unwrap();
        assert_eq!(fixtures.answers.len(), 1);
        assert_eq!(fixtures.answers[0].models, vec!["Civic"]);
        let baseline: Report =
            serde_json::from_str(&serde_json::to_string(&report(1, &["x"], &[])).unwrap()).unwrap();
        assert_eq!(baseline, report(1, &["x"], &[]));
    }
}
