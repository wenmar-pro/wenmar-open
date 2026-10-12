//! The guides to reading a scan tool: fuel trims, and OBD-II readiness
//! monitors.
//!
//! Each page has a fixed list of headings. The text under them is written
//! and approved by a person; until then a page is a draft, and
//! `guides::DRAFTS` holds it where nothing serves it.

use crate::site::guides::NOT_WRITTEN;
use crate::site::markdown::Doc;
use crate::site::pages::{section, with_links};

/// The heading of the section that lists where the page's facts were read.
pub(super) const SOURCES: &str = "Sources";
/// The heading of the last section, which leads back to the calculators.
pub(super) const MORE: &str = "More for shop owners";

pub(super) const FUEL_TRIMS: &[&str] = &[
    "What fuel trim is",
    "Short-term and long-term trim",
    "Reading the numbers",
    "Trim at idle and under load",
    "What a lean correction points to",
    "What a rich correction points to",
    "Bank 1 and bank 2",
    "What fuel trims do not tell you",
];
pub(super) const READINESS_MONITORS: &[&str] = &[
    "What a readiness monitor is",
    "Monitors that run all the time",
    "Monitors that wait for the right conditions",
    "What each monitor needs before it runs",
    "Drive cycles",
    "After codes are cleared or the battery is disconnected",
    "Readiness and emissions inspections",
];

/// A page with its headings and no text: every section, and the list of
/// sources, says it is not written.
fn draft(title: &str, intro: &str, headings: &[&str], base: &str) -> Doc {
    let mut sections: Vec<_> = headings
        .iter()
        .map(|heading| section(heading, &[NOT_WRITTEN]))
        .collect();
    sections.push(section(SOURCES, &[NOT_WRITTEN]));
    sections.push(with_links(
        section(
            MORE,
            &["The calculators and the other guides for shop owners are listed on one page."],
        ),
        &[("Shop calculators", format!("{base}/tools"))],
    ));
    Doc {
        title: title.to_owned(),
        intro: intro.to_owned(),
        sections,
    }
}

pub(super) fn fuel_trims(base: &str) -> Doc {
    draft(
        "How to read fuel trims",
        "What the fuel trim figures on a scan tool mean and how to reason from them. This page is general: the vehicle maker's service information has the figures for a given engine.",
        FUEL_TRIMS,
        base,
    )
}

pub(super) fn readiness_monitors(base: &str) -> Doc {
    draft(
        "OBD-II readiness monitors explained",
        "What a vehicle's readiness monitors are and what each needs before it will run. This page is general: the vehicle maker's service information has the drive cycle for a given vehicle.",
        READINESS_MONITORS,
        base,
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::site::guides::NOT_WRITTEN;
    use crate::site::markdown;

    fn pages() -> [(Doc, &'static [&'static str]); 2] {
        let base = "https://open.example";
        [
            (fuel_trims(base), FUEL_TRIMS),
            (readiness_monitors(base), READINESS_MONITORS),
        ]
    }

    #[test]
    fn each_page_has_its_headings_in_order_then_sources_and_the_way_back() {
        for (doc, headings) in pages() {
            let found: Vec<&str> = doc
                .sections
                .iter()
                .map(|section| section.heading.as_str())
                .collect();
            let mut expected = headings.to_vec();
            expected.extend([SOURCES, MORE]);
            assert_eq!(found, expected, "{}", doc.title);
            assert!(!doc.intro.is_empty());
            let last = doc.sections.last().unwrap();
            assert_eq!(
                last.links,
                [(
                    "Shop calculators".to_owned(),
                    "https://open.example/tools".to_owned()
                )]
            );
        }
    }

    #[test]
    fn a_page_that_is_written_has_text_in_every_section_and_cites_its_sources() {
        for (doc, _) in pages() {
            if markdown::doc(&doc).contains(NOT_WRITTEN) {
                continue;
            }
            for section in &doc.sections {
                assert!(
                    !section.paragraphs.is_empty(),
                    "{}: {}",
                    doc.title,
                    section.heading
                );
            }
            let sources = doc
                .sections
                .iter()
                .find(|section| section.heading == SOURCES)
                .unwrap();
            assert!(
                sources.links.len() >= 2,
                "{}: at least two sources",
                doc.title
            );
            for (label, address) in &sources.links {
                assert!(
                    address.starts_with("https://") && !label.is_empty(),
                    "{}: {address}",
                    doc.title
                );
            }
        }
    }

    #[test]
    fn the_written_readiness_page_has_its_table_of_monitors() {
        let doc = readiness_monitors("https://open.example");
        if markdown::doc(&doc).contains(NOT_WRITTEN) {
            return;
        }
        let needs = doc
            .sections
            .iter()
            .find(|section| section.heading == "What each monitor needs before it runs")
            .unwrap();
        let table = needs.table.as_ref().expect("a table of monitors");
        assert_eq!(
            table.head,
            ["Monitor", "What it checks", "What it generally needs"]
        );
        assert!(table.prose && table.rows.len() >= 3);
    }
}
