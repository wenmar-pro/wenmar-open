//! The Markdown version of the battery calculator, for AI agents.
//!
//! It says what the calculator takes, how the result is worked out, and the
//! table of each kind of battery with its source and the day it was read. A
//! battery not yet checked has its warning and no table. Every figure is
//! read from the data file.

use crate::site::markdown::Doc;
use crate::site::pages::{section, with_links, with_table};
use crate::site::tools::battery_soc::data::BATTERIES;
use crate::site::tools::battery_soc::result;
use crate::site::tools::battery_soc::{PATH, SUMMARY, TITLE};
use crate::site::tools::canada_invoice_tax::doc::source_text;

pub fn doc(base: &str) -> Doc {
    let mut sections = vec![section(
        "What you enter",
        &[
            "The kind of battery, the voltage across its posts with nothing drawing from it, and, if you have it, the battery's temperature in \u{b0}C or \u{b0}F.",
            result::LIMITS,
        ],
    )];
    let formulas = result::formulas();
    let lines: Vec<&str> = formulas.iter().map(String::as_str).collect();
    sections.push(section("How this is worked out", &lines));
    for battery in &BATTERIES {
        if battery.confirmed {
            let checked = result::checked(battery);
            let mut paragraphs = vec![checked.as_str()];
            paragraphs.extend(battery.conditions.iter().copied());
            paragraphs.extend(battery.notes.iter().copied());
            let texts: Vec<(String, &str)> = battery
                .correction_sources
                .iter()
                .chain(battery.sources)
                .map(|source| (source_text(source), source.url))
                .collect();
            let links: Vec<(&str, String)> = texts
                .iter()
                .map(|(text, url)| (text.as_str(), (*url).to_owned()))
                .collect();
            let section = with_table(section(battery.name, &paragraphs), result::chart(battery));
            sections.push(with_links(section, &links));
        } else {
            let mut paragraphs = vec!["Not yet checked."];
            paragraphs.extend(battery.missing.iter().copied());
            sections.push(section(battery.name, &paragraphs));
        }
    }
    sections.push(with_links(
        section("Use it", &["The calculator as a web page:"]),
        &[(TITLE, format!("{base}{PATH}"))],
    ));
    Doc {
        title: TITLE.to_owned(),
        intro: format!(
            "{SUMMARY} This page describes the calculator and does not compute. The calculator is a web page with a form."
        ),
        sections,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::site::markdown;

    #[test]
    fn the_calculator_names_each_kind_of_battery_under_its_own_heading() {
        let text = markdown::doc(&doc("https://open.example"));
        assert!(
            text.starts_with("# Battery state of charge from resting voltage\n"),
            "{text}"
        );
        for battery in &BATTERIES {
            assert!(text.contains(&format!("\n## {}\n", battery.name)), "{text}");
        }
    }
}
