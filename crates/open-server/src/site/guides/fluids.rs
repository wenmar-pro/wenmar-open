//! The fluid and specification guides: engine oil, transmission fluid,
//! coolant, brake fluid and refrigerants.
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

pub(super) const ENGINE_OIL: &[&str] = &[
    "What a specification is",
    "Viscosity grades",
    "API service categories",
    "ILSAC standards",
    "Vehicle maker approvals",
    "European ACEA sequences",
    "Which replaces which",
    "What not to substitute",
    "Where to find the right one",
];
pub(super) const TRANSMISSION_FLUIDS: &[&str] = &[
    "Why there are families",
    "Automatic transmission fluids",
    "CVT fluids",
    "Dual-clutch fluids",
    "Manual gearbox and axle oils",
    "Which replaces which",
    "What not to substitute",
    "Where to find the right one",
];
pub(super) const COOLANT: &[&str] = &[
    "What coolant does",
    "The inhibitor chemistries",
    "Color is not a specification",
    "Mixing types",
    "Water and concentration",
    "Service life",
    "Where to find the right one",
];
pub(super) const BRAKE_FLUID: &[&str] = &[
    "What the grades are",
    "Boiling points",
    "Glycol and silicone fluids",
    "Mixing grades",
    "Moisture and service life",
    "Where to find the right one",
];
pub(super) const REFRIGERANTS: &[&str] = &[
    "The refrigerants in use",
    "Which vehicles use which",
    "Fittings and equipment",
    "Compressor oils",
    "Rules on handling",
    "Where to find the right one",
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

pub(super) fn engine_oil(base: &str) -> Doc {
    draft(
        "Engine oil specifications explained",
        "What the letters and numbers on a bottle of engine oil mean. This page names no oil as the right one for a vehicle: the owner's manual does that.",
        ENGINE_OIL,
        base,
    )
}

pub(super) fn transmission_fluids(base: &str) -> Doc {
    draft(
        "Transmission fluid types explained",
        "The families of transmission fluid, and why one does not stand in for another. This page names no fluid as the right one for a vehicle: the owner's manual does that.",
        TRANSMISSION_FLUIDS,
        base,
    )
}

pub(super) fn coolant(base: &str) -> Doc {
    draft(
        "Coolant types and chemistries explained",
        "The kinds of engine coolant and what sets them apart. This page names no coolant as the right one for a vehicle: the owner's manual does that.",
        COOLANT,
        base,
    )
}

pub(super) fn brake_fluid(base: &str) -> Doc {
    draft(
        "Brake fluid grades explained",
        "What the grades of brake fluid are and how they differ. This page names no fluid as the right one for a vehicle: the owner's manual and the cap of the reservoir do that.",
        BRAKE_FLUID,
        base,
    )
}

pub(super) fn refrigerants(base: &str) -> Doc {
    draft(
        "Automotive refrigerants explained",
        "The refrigerants used in vehicle air conditioning and what goes with each. This page is not a service procedure.",
        REFRIGERANTS,
        base,
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::site::markdown;

    fn pages() -> [(Doc, &'static [&'static str]); 5] {
        let base = "https://open.example";
        [
            (engine_oil(base), ENGINE_OIL),
            (transmission_fluids(base), TRANSMISSION_FLUIDS),
            (coolant(base), COOLANT),
            (brake_fluid(base), BRAKE_FLUID),
            (refrigerants(base), REFRIGERANTS),
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
}
