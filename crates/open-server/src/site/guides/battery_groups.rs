//! The battery group size table: what each group size is, as a page.
//!
//! The figures are compiled in from `data/reference/battery-groups.toml`;
//! see `battery_groups_build.rs` beside this crate's `Cargo.toml`. The page
//! says what a group size is and never which one fits a vehicle. Until a
//! person has read the table from a source and confirmed it, the page shows
//! no figure, and `guides::DRAFTS` holds it where nothing serves it.

use crate::site::markdown::{Doc, Table};
use crate::site::pages::{section, with_links, with_table};

/// A page a figure was read from.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Source {
    pub title: &'static str,
    pub url: &'static str,
    /// The date of the Internet Archive's copy, when that was what was read.
    pub archived: Option<&'static str>,
}

/// One dimension, in both units, as text ready to show.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Side {
    pub mm: &'static str,
    pub inches: &'static str,
}

/// One group size.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Group {
    pub name: &'static str,
    pub length: Side,
    pub width: Side,
    pub height: Side,
    /// What kind of terminals it has and where the positive one is.
    pub terminals: &'static str,
}

/// The whole table.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct GroupTable {
    pub name: &'static str,
    /// The day the table was read.
    pub as_of: &'static str,
    /// Whether every group was read from a source. Nothing is shown of a
    /// table that is not confirmed.
    pub confirmed: bool,
    pub missing: &'static [&'static str],
    pub notes: &'static [&'static str],
    /// The unit the source prints: `mm` or `in`.
    pub printed_in: &'static str,
    pub groups: &'static [Group],
    pub sources: &'static [Source],
}

include!(concat!(env!("OUT_DIR"), "/battery_groups.rs"));

pub(super) const TITLE: &str = "Battery group sizes: dimensions and terminals";
pub(super) const DESCRIPTION: &str = "A table of battery group sizes: the length, width and height of each case and where its terminals are. It does not say which group fits which vehicle.";

/// What the page says it is not, in the introduction and again in a
/// section of its own.
pub const NOT_FITMENT: &str = "This table does not say which group size fits which vehicle.";

/// The heading of the section a table that is not confirmed gets in
/// place of its figures.
pub const NOT_CHECKED: &str = "Not yet checked";

fn side(side: Side) -> String {
    format!("{} mm ({} in)", side.mm, side.inches)
}

/// The page for `table`, with links on the public address `base`.
pub fn doc_of(table: &GroupTable, base: &str) -> Doc {
    let mut sections = Vec::new();
    if table.confirmed {
        let unit = if table.printed_in == "mm" {
            "millimetres, and the inches are worked out from them"
        } else {
            "inches, and the millimetres are worked out from them"
        };
        let mut paragraphs = vec![format!(
            "Each row is one group size: the length, width and height of its case, and its terminals. The source prints the dimensions in {unit}, at 25.4 mm to the inch. Read on {}.",
            table.as_of
        )];
        paragraphs.extend(table.notes.iter().map(|note| (*note).to_owned()));
        let paragraphs: Vec<&str> = paragraphs.iter().map(String::as_str).collect();
        sections.push(with_table(
            section("The group sizes", &paragraphs),
            Table {
                caption: table.name.to_owned(),
                head: ["Group", "Length", "Width", "Height", "Terminals"]
                    .iter()
                    .map(|cell| (*cell).to_owned())
                    .collect(),
                rows: table
                    .groups
                    .iter()
                    .map(|group| {
                        vec![
                            group.name.to_owned(),
                            side(group.length),
                            side(group.width),
                            side(group.height),
                            group.terminals.to_owned(),
                        ]
                    })
                    .collect(),
                prose: true,
            },
        ));
    } else {
        let mut paragraphs =
            vec!["The table has not been read from a published source yet, so no figure is shown."];
        paragraphs.extend(table.missing.iter().copied());
        sections.push(section(NOT_CHECKED, &paragraphs));
    }
    sections.push(section(
        "What this table does not say",
        &[
            NOT_FITMENT,
            "No free source publishes which group goes in which vehicle, and the battery makers' own fitment guides may not be copied here. To choose a battery, read the group size on the battery that is in the vehicle, measure the tray and the hold-down, and check the owner's manual.",
            "A group size names a case and its terminals. The table says nothing else about a battery: not its capacity, not its cold-cranking rating and not its chemistry.",
        ],
    ));
    if !table.sources.is_empty() {
        let links: Vec<(String, String)> = table
            .sources
            .iter()
            .map(|source| {
                let title = match source.archived {
                    Some(archived) => format!(
                        "{} (the Internet Archive's copy of {archived})",
                        source.title
                    ),
                    None => source.title.to_owned(),
                };
                (title, source.url.to_owned())
            })
            .collect();
        let links: Vec<(&str, String)> = links
            .iter()
            .map(|(title, url)| (title.as_str(), url.clone()))
            .collect();
        sections.push(with_links(
            section(
                "Sources",
                &["Every figure in the table was read from these pages."],
            ),
            &links,
        ));
    }
    sections.push(with_links(
        section(
            "More for shop owners",
            &["The calculators and the other guides for shop owners are listed on one page."],
        ),
        &[("Shop calculators", format!("{base}/tools"))],
    ));
    Doc {
        title: TITLE.to_owned(),
        intro: format!(
            "A battery group size is a name for a battery case: how long, wide and tall it is, and where its terminals are. {NOT_FITMENT}"
        ),
        sections,
    }
}

/// The page for the table in the repository.
pub(super) fn battery_groups(base: &str) -> Doc {
    doc_of(&BATTERY_GROUPS, base)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::site::markdown;

    /// A table no source gave: made up for the tests.
    const FAKE: GroupTable = GroupTable {
        name: "Made-up battery groups",
        as_of: "2026-01-31",
        confirmed: true,
        missing: &[],
        notes: &["A note the page shows."],
        printed_in: "mm",
        groups: &[
            Group {
                name: "T1 (made up)",
                length: Side {
                    mm: "254",
                    inches: "10.00",
                },
                width: Side {
                    mm: "127",
                    inches: "5.00",
                },
                height: Side {
                    mm: "101",
                    inches: "3.96",
                },
                terminals: "Made-up posts on top, the positive one on the left.",
            },
            Group {
                name: "T2 (made up)",
                length: Side {
                    mm: "999",
                    inches: "39.33",
                },
                width: Side {
                    mm: "888",
                    inches: "34.96",
                },
                height: Side {
                    mm: "777",
                    inches: "30.59",
                },
                terminals: "Made-up terminals on the side.",
            },
        ],
        sources: &[Source {
            title: "A made-up page",
            url: "https://example.org/groups",
            archived: Some("2026-01"),
        }],
    };

    #[test]
    fn a_confirmed_table_is_shown_in_both_units_with_its_sources() {
        let text = markdown::doc(&doc_of(&FAKE, "https://open.example"));
        assert!(text.starts_with(&format!("# {TITLE}\n")), "{text}");
        for line in [
            "| Group | Length | Width | Height | Terminals |\n",
            "| T1 (made up) | 254 mm (10.00 in) | 127 mm (5.00 in) | 101 mm (3.96 in) | Made-up posts on top, the positive one on the left. |\n",
            "| T2 (made up) | 999 mm (39.33 in) | 888 mm (34.96 in) | 777 mm (30.59 in) | Made-up terminals on the side. |\n",
            "The source prints the dimensions in millimetres, and the inches are worked out from them, at 25.4 mm to the inch. Read on 2026-01-31.",
            "A note the page shows.",
            "[A made-up page (the Internet Archive's copy of 2026-01)](https://example.org/groups)",
            "[Shop calculators](https://open.example/tools)",
        ] {
            assert!(text.contains(line), "{line}\n{text}");
        }
        assert!(!text.contains(NOT_CHECKED));
        // It says twice that it is not a fitment guide.
        assert_eq!(text.matches(NOT_FITMENT).count(), 2, "{text}");
        assert!(text.is_ascii());
    }

    #[test]
    fn a_table_that_is_not_confirmed_shows_no_figure() {
        let pending = GroupTable {
            confirmed: false,
            missing: &["Nothing was read yet."],
            ..FAKE
        };
        let text = markdown::doc(&doc_of(&pending, "https://open.example"));
        assert!(text.contains(&format!("## {NOT_CHECKED}\n")), "{text}");
        assert!(text.contains("Nothing was read yet."));
        assert!(
            !text.contains("T1 (made up)") && !text.contains("254 mm"),
            "{text}"
        );
        assert_eq!(text.matches(NOT_FITMENT).count(), 2);
    }

    #[test]
    fn the_page_of_the_repository_is_a_draft_until_its_table_is_confirmed() {
        use crate::site::guides::{DRAFTS, GUIDES};
        let slug = "battery-group-sizes";
        let listed = GUIDES.iter().any(|guide| guide.slug == slug);
        let draft = DRAFTS.iter().any(|guide| guide.slug == slug);
        assert!(
            listed != draft,
            "the page is in exactly one of the two lists"
        );
        // It may be listed only once a person has confirmed the table.
        assert!(!listed || BATTERY_GROUPS.confirmed);
        let doc = battery_groups("https://open.example");
        assert_eq!(doc.title, TITLE);
        let text = markdown::doc(&doc);
        assert_eq!(text.contains(NOT_CHECKED), !BATTERY_GROUPS.confirmed);
        assert_eq!(BATTERY_GROUPS.confirmed, BATTERY_GROUPS.missing.is_empty());
    }
}
