//! The Markdown version of the refrigerant chart, for AI agents.
//!
//! It says what the chart shows, the table of each refrigerant with its
//! source and the day it was read, and how the columns are worked out. A
//! refrigerant not yet checked has its warning and no table. Every figure is
//! read from the data file.

use crate::site::markdown::Doc;
use crate::site::pages::{section, with_links, with_table};
use crate::site::tools::canada_invoice_tax::doc::source_text;
use crate::site::tools::refrigerant_pt::data::REFRIGERANTS;
use crate::site::tools::refrigerant_pt::result;
use crate::site::tools::refrigerant_pt::{PATH, SUMMARY, TITLE};

pub fn doc(base: &str) -> Doc {
    let mut sections = vec![section("What the chart shows", &[result::LIMITS])];
    for refrigerant in &REFRIGERANTS {
        let checked = result::checked(refrigerant);
        let units = result::units(refrigerant);
        let notes: Vec<&str> = refrigerant.notes.to_vec();
        if refrigerant.confirmed {
            let mut paragraphs = vec![checked.as_str(), units.as_str()];
            paragraphs.extend(notes);
            let texts: Vec<(String, &str)> = refrigerant
                .sources
                .iter()
                .map(|source| (source_text(source), source.url))
                .collect();
            let links: Vec<(&str, String)> = texts
                .iter()
                .map(|(text, url)| (text.as_str(), (*url).to_owned()))
                .collect();
            let section = with_table(
                section(refrigerant.name, &paragraphs),
                result::table(refrigerant),
            );
            sections.push(with_links(section, &links));
        } else {
            let not_checked = result::not_checked(refrigerant);
            let mut paragraphs = vec![not_checked.as_str()];
            paragraphs.extend(refrigerant.missing.iter().copied());
            sections.push(section(refrigerant.name, &paragraphs));
        }
    }
    let conversions = result::conversions();
    let lines: Vec<&str> = conversions.iter().map(String::as_str).collect();
    sections.push(section("How the columns are worked out", &lines));
    sections.push(with_links(
        section("Use it", &["The same chart as a web page:"]),
        &[(TITLE, format!("{base}{PATH}"))],
    ));
    Doc {
        title: TITLE.to_owned(),
        intro: SUMMARY.to_owned(),
        sections,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::site::markdown;

    #[test]
    fn the_chart_names_each_refrigerant_under_its_own_heading() {
        let text = markdown::doc(&doc("https://open.example"));
        assert!(
            text.starts_with("# R-134a and R-1234yf pressure-temperature chart\n"),
            "{text}"
        );
        assert!(text.contains("\n## R-134a\n"), "{text}");
        assert!(text.contains("\n## R-1234yf\n"), "{text}");
    }
}
