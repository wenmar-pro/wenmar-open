//! `/rules/canada`: what provincial law asks of a repair shop, one page for
//! each province and territory, and an index of them.
//!
//! No rule is written here. Each is in `data/rules/canada.toml`, with its
//! source and the day it was read, and reaches this code through the
//! server's build (`rules_build.rs`). A province that is not confirmed
//! shows no rule at all.

pub use crate::site::tools::canada_invoice_tax::rates::Source;

use crate::site::markdown::{Doc, Section};
use crate::site::tools::canada_invoice_tax::doc::source_text;

/// One topic of one province, as the law's own pages state it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Rule {
    pub topic: &'static str,
    /// False when the province is known to set no rule on the topic.
    pub applies: bool,
    pub summary: &'static [&'static str],
    pub law: Option<&'static str>,
    /// The day the rule was read from its sources.
    pub as_of: &'static str,
    pub effective: Option<&'static str>,
    pub sources: &'static [Source],
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Province {
    pub code: &'static str,
    pub name: &'static str,
    pub as_of: &'static str,
    pub confirmed: bool,
    pub missing: &'static [&'static str],
    pub notes: &'static [&'static str],
    pub rules: &'static [Rule],
}

include!(concat!(env!("OUT_DIR"), "/canada_rules.rs"));

/// Each topic's key in the rules file and its heading on a page, in the order of the page.
pub const TOPICS: [(&str, &str); 9] = [
    ("written_estimate", "Written estimates"),
    ("authorisation", "Authorisation before work starts"),
    ("estimate_overrun", "Going over the estimate"),
    ("repair_warranty", "Warranty on repairs"),
    ("invoice_contents", "What an invoice must show"),
    ("old_parts", "Return of old parts"),
    ("winter_tires", "Winter tires"),
    ("inspection", "Vehicle inspections"),
    ("environmental_fees", "Environmental fees"),
];

pub const DISCLAIMER: &str = "This page is a summary for repair shops, in our own words. It is not legal advice and it is not the law: read the act or regulation linked under each rule, or ask a lawyer, before relying on it.";
pub const NOT_CHECKED: &str = "Not yet checked.";
pub const INDEX_PATH: &str = "/rules/canada";
pub const INDEX_TITLE: &str = "Canadian rules for repair shops, by province";
pub const INDEX_DESCRIPTION: &str = "What provincial law asks of an auto repair shop in Canada: estimates, authorisation, warranty, invoices, old parts, winter tires, inspections and fees.";

impl Province {
    /// The address of this province's page.
    pub fn path(&self) -> String {
        format!("{INDEX_PATH}/{}", self.code.to_ascii_lowercase())
    }

    /// The rule on a topic. None for every topic when the province is not
    /// confirmed, whatever its entry holds.
    pub fn rule(&self, topic: &str) -> Option<&Rule> {
        if !self.confirmed {
            return None;
        }
        self.rules.iter().find(|rule| rule.topic == topic)
    }

    pub fn title(&self) -> String {
        format!("{}: rules for auto repair shops", self.name)
    }

    pub fn description(&self) -> String {
        format!(
            "What the law of {} asks of an auto repair shop: estimates, authorisation, warranty, invoices, old parts, winter tires, inspections and fees.",
            self.name
        )
    }
}

/// The province whose code, in small letters, is exactly `segment`.
pub fn find(segment: &str) -> Option<&'static Province> {
    PROVINCES
        .iter()
        .find(|province| province.code.to_ascii_lowercase() == segment)
}

fn section_of(heading: &str, paragraphs: Vec<String>, links: Vec<(String, String)>) -> Section {
    Section {
        heading: heading.to_owned(),
        paragraphs,
        links,
        ..Section::default()
    }
}

/// The page of one province.
pub fn province_doc(base: &str, province: &Province) -> Doc {
    let intro = if province.confirmed {
        format!(
            "What the law of {} asks of a shop that repairs vehicles for the public, each rule with the page it was read from. Checked on {}.",
            province.name, province.as_of
        )
    } else {
        format!(
            "Nothing on this page has been checked for {} yet. {}",
            province.name,
            province.missing.join(" ")
        )
    };
    let mut sections: Vec<Section> = TOPICS
        .iter()
        .map(|(key, heading)| match province.rule(key) {
            None => section_of(heading, vec![NOT_CHECKED.to_owned()], Vec::new()),
            Some(rule) => {
                let mut paragraphs: Vec<String> =
                    rule.summary.iter().map(|line| (*line).to_owned()).collect();
                if let Some(law) = rule.law {
                    paragraphs.push(format!("Law: {law}"));
                }
                if let Some(effective) = rule.effective {
                    paragraphs.push(format!("In effect from {effective}."));
                }
                paragraphs.push(format!("Read on {}.", rule.as_of));
                let links = rule
                    .sources
                    .iter()
                    .map(|source| (source_text(source), source.url.to_owned()))
                    .collect();
                section_of(heading, paragraphs, links)
            }
        })
        .collect();
    let mut notes = vec![DISCLAIMER.to_owned()];
    notes.extend(province.notes.iter().map(|line| (*line).to_owned()));
    sections.push(section_of("Not legal advice", notes, Vec::new()));
    sections.push(section_of(
        "Other provinces and territories",
        vec!["The rules of the other provinces and territories are listed on one page.".to_owned()],
        vec![(INDEX_TITLE.to_owned(), format!("{base}{INDEX_PATH}"))],
    ));
    Doc {
        title: province.title(),
        intro,
        sections,
    }
}

/// The index: every province and territory, and whether it is checked.
pub fn index_doc(base: &str) -> Doc {
    let places = PROVINCES
        .iter()
        .map(|province| {
            let label = if province.confirmed {
                format!("{}: checked {}", province.name, province.as_of)
            } else {
                format!("{}: not yet checked", province.name)
            };
            (label, format!("{base}{}", province.path()))
        })
        .collect();
    let headings: Vec<&str> = TOPICS.iter().map(|(_, heading)| *heading).collect();
    Doc {
        title: INDEX_TITLE.to_owned(),
        intro: format!(
            "What provincial and territorial law asks of a shop that repairs vehicles for the public, one page for each province and territory. {DISCLAIMER}"
        ),
        sections: vec![
            section_of(
                "Provinces and territories",
                vec!["A province or territory marked not yet checked has a page, and that page shows no rule.".to_owned()],
                places,
            ),
            section_of(
                "What each page covers",
                vec![format!("Each page has these headings: {}.", headings.join("; "))],
                Vec::new(),
            ),
            section_of(
                "How the rules are kept",
                vec!["Every rule is written in one file, with the page it was read from and the day it was read. A province or territory is shown only when every topic has been read from its source. Nothing is filled in from memory or from another website.".to_owned()],
                Vec::new(),
            ),
            section_of(
                "Taxes and tire fees",
                vec!["The sales taxes and the new-tire fees on a repair invoice are worked out by the calculator.".to_owned()],
                vec![(
                    "Canadian invoice tax and tire fee calculator".to_owned(),
                    format!("{base}/tools/canada-invoice-tax"),
                )],
            ),
        ],
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::site::markdown;

    const ACT: Source = Source {
        title: "A made-up act",
        url: "https://example.org/act",
        archived: None,
    };
    const OLD: Source = Source {
        title: "A made-up page",
        url: "https://example.org/page",
        archived: Some("2025-08"),
    };
    /// Made up: no place, act or rule here is real.
    const RULES: [Rule; 2] = [
        Rule {
            topic: "written_estimate",
            applies: true,
            summary: &["A made-up rule about estimates.", "Its second sentence."],
            law: Some("Made-up Act, s. 1"),
            as_of: "2026-01-30",
            effective: Some("2020-02-29"),
            sources: &[ACT, OLD],
        },
        Rule {
            topic: "winter_tires",
            applies: false,
            summary: &["First Place sets no made-up rule on winter tires."],
            law: None,
            as_of: "2026-01-29",
            effective: None,
            sources: &[OLD],
        },
    ];
    const CHECKED: Province = Province {
        code: "XA",
        name: "First Place",
        as_of: "2026-01-31",
        confirmed: true,
        missing: &[],
        notes: &["A note the page shows."],
        rules: &RULES,
    };
    const WAITING: Province = Province {
        confirmed: false,
        missing: &["Seven topics have not been read."],
        ..CHECKED
    };

    #[test]
    fn the_topics_are_the_ones_the_build_checks_in_the_same_order() {
        let keys: Vec<&str> = TOPICS.iter().map(|(key, _)| *key).collect();
        assert_eq!(keys, crate::rules_build::TOPICS);
    }

    #[test]
    fn a_checked_province_shows_each_rule_with_its_law_its_date_and_its_sources() {
        let doc = province_doc("https://open.example", &CHECKED);
        assert_eq!(doc.title, "First Place: rules for auto repair shops");
        assert_eq!(
            doc.intro,
            "What the law of First Place asks of a shop that repairs vehicles for the public, each rule with the page it was read from. Checked on 2026-01-31."
        );
        let headings: Vec<&str> = doc
            .sections
            .iter()
            .map(|section| section.heading.as_str())
            .collect();
        assert_eq!(
            headings,
            [
                "Written estimates",
                "Authorisation before work starts",
                "Going over the estimate",
                "Warranty on repairs",
                "What an invoice must show",
                "Return of old parts",
                "Winter tires",
                "Vehicle inspections",
                "Environmental fees",
                "Not legal advice",
                "Other provinces and territories",
            ]
        );
        let estimates = &doc.sections[0];
        assert_eq!(
            estimates.paragraphs,
            [
                "A made-up rule about estimates.",
                "Its second sentence.",
                "Law: Made-up Act, s. 1",
                "In effect from 2020-02-29.",
                "Read on 2026-01-30."
            ]
        );
        assert_eq!(
            estimates.links,
            [
                (
                    "A made-up act".to_owned(),
                    "https://example.org/act".to_owned()
                ),
                (
                    "A made-up page (read from an archived copy of 2025-08)".to_owned(),
                    "https://example.org/page".to_owned()
                ),
            ]
        );
        assert_eq!(
            doc.sections[6].paragraphs,
            [
                "First Place sets no made-up rule on winter tires.",
                "Read on 2026-01-29."
            ]
        );
        // A topic with no rule says so and links nowhere.
        assert_eq!(doc.sections[1].paragraphs, [NOT_CHECKED]);
        assert!(doc.sections[1].links.is_empty());
        assert_eq!(
            doc.sections[9].paragraphs,
            [DISCLAIMER, "A note the page shows."]
        );
        assert_eq!(
            doc.sections[10].links,
            [(
                "Canadian rules for repair shops, by province".to_owned(),
                "https://open.example/rules/canada".to_owned()
            )]
        );
    }

    #[test]
    fn a_province_that_is_not_confirmed_shows_no_rule() {
        let doc = province_doc("https://open.example", &WAITING);
        assert_eq!(
            doc.intro,
            "Nothing on this page has been checked for First Place yet. Seven topics have not been read."
        );
        for section in &doc.sections[..9] {
            assert_eq!(section.paragraphs, [NOT_CHECKED], "{}", section.heading);
            assert!(section.links.is_empty(), "{}", section.heading);
        }
        let text = markdown::doc(&doc);
        assert!(
            !text.contains("made-up rule") && !text.contains("example.org"),
            "{text}"
        );
        assert!(text.contains(DISCLAIMER));
        assert!(
            WAITING.rule("written_estimate").is_none()
                && CHECKED.rule("written_estimate").is_some()
        );
        assert!(CHECKED.rule("authorisation").is_none());
        assert_eq!(CHECKED.path(), "/rules/canada/xa");
    }

    #[test]
    fn the_index_lists_every_province_and_says_which_are_checked() {
        let doc = index_doc("https://open.example");
        assert_eq!(doc.title, "Canadian rules for repair shops, by province");
        assert!(doc.intro.ends_with(DISCLAIMER));
        let places = &doc.sections[0];
        assert_eq!(places.heading, "Provinces and territories");
        assert_eq!(places.links.len(), PROVINCES.len());
        for (province, (label, address)) in PROVINCES.iter().zip(&places.links) {
            let expected = if province.confirmed {
                format!("{}: checked {}", province.name, province.as_of)
            } else {
                format!("{}: not yet checked", province.name)
            };
            assert_eq!(label, &expected);
            assert_eq!(address, &format!("https://open.example{}", province.path()));
            assert!(
                find(&province.code.to_ascii_lowercase()).is_some(),
                "{}",
                province.code
            );
            assert!(
                find(province.code).is_none(),
                "{}: only small letters are an address",
                province.code
            );
        }
        assert!(find("zz").is_none() && find("").is_none());
        for (_, heading) in TOPICS {
            assert!(doc.sections[1].paragraphs[0].contains(heading), "{heading}");
        }
    }
}
