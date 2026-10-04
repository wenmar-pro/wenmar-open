//! The Markdown version of the parts matrix page, for AI agents.
//!
//! It says what the calculator does, what each input is, the formulas and
//! one worked example. It does not compute: the example is the one the
//! page opens with, worked out by the same code and written down.

use shop_math::targets::PARTS;

use crate::site::markdown::Doc;
use crate::site::pages::{section, with_code, with_links, with_table};
use crate::site::tools::parts_matrix::form::Form;
use crate::site::tools::parts_matrix::presets::example;
use crate::site::tools::parts_matrix::result::{PARTS_TARGET_OF, formulas, shown, usual_markup};
use crate::site::tools::parts_matrix::{PATH, SUMMARY, TITLE};
use crate::site::tools::pieces::ADVICE;
use crate::site::tools::target::source_links;

/// What the page says about its example matrices, on the page and here.
pub const ILLUSTRATIONS: &str = "The example matrices are illustrations made for this page, not survey results, and no matrix here is a recommendation. The right matrix is the one that produces the margin a shop needs on its own mix of parts, which is what the share column shows.";

fn refs(paragraphs: &[String]) -> Vec<&str> {
    paragraphs.iter().map(String::as_str).collect()
}

pub fn doc(base: &str) -> Doc {
    let address = format!("{base}{PATH}");
    let mut how = formulas();
    how.push(usual_markup());
    let target = vec![
        format!(
            "When shares are given, the blended margin is shown beside a target for {PARTS_TARGET_OF}: a typical range of {} to {}, and a usual target of {}. The page says whether the figure is below, inside or above the range.",
            PARTS.range.low, PARTS.range.high, PARTS.usual
        ),
        "The range and the target are for a general repair shop, and come from the pages below. They are what those pages call typical and what they say shops aim for, not a survey of what shops earn.".to_owned(),
    ];
    let links: Vec<(String, String)> = source_links(&PARTS);
    let links: Vec<(&str, String)> = links
        .iter()
        .map(|(text, address)| (text.as_str(), address.clone()))
        .collect();
    let mut worked = vec![format!(
        "The page opens with this example: a sliding scale of six rows read as markup, a mix of parts spend, and one part that costs 42.50. {ILLUSTRATIONS}"
    )];
    let mut sections = vec![
        section(
            "What you enter",
            &[
                "Up to eight rows. Each row has a cost up to (money), a percent, and optionally a share of parts spend (percent). Rows must rise in cost up to. Empty rows are ignored, and at least one row is needed. The last row used has no upper limit, whatever its cost up to says.",
                "A choice of how the percent of every row is read: as a markup, which is the default, or as a margin.",
                "Optionally, the cost of one part to price.",
                "If any row has a share, the shares must add to 100%. A row with no share counts as none.",
                "Money may be written as 1234.5, 1,234.50 or $1,234.50, up to 99,999,999.99. A percent may be written with or without the sign, from 0 to 1,000.",
            ],
        ),
        section("How this is worked out", &refs(&how)),
        with_links(section("The target", &refs(&target)), &links),
    ];
    let (_, result) = Form::read(&example());
    if let Some(result) = result {
        let shown = shown(&result);
        worked.extend(shown.part);
        worked.extend(shown.blended.map(|blended| blended.sentence()));
        worked.push(shown.last);
        sections.push(with_table(
            section("A worked example", &refs(&worked)),
            shown.table,
        ));
    }
    sections.push(with_code(
        section(
            "A link to a filled-in form",
            &["Every field is in the address, so a filled-in form can be linked to, bookmarked and printed. Nothing is stored. The fields are kind (markup or margin), cost1, rate1 and share1 to cost8, rate8 and share8, and part. This address is the example above:"],
        ),
        format!("{address}?{}", example().query()),
    ));
    sections.push(with_links(
        section("Use it", &[ADVICE]),
        &[(TITLE, address)],
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

    fn text() -> String {
        markdown::doc(&doc("https://open.example"))
    }

    #[test]
    fn it_says_what_the_calculator_does_each_input_and_the_formulas() {
        let text = text();
        assert!(text.starts_with(&format!("# {TITLE}\n\n{SUMMARY} This page describes")));
        let headings: Vec<&str> = text
            .lines()
            .filter_map(|line| line.strip_prefix("## "))
            .collect();
        assert_eq!(
            headings,
            [
                "What you enter",
                "How this is worked out",
                "The target",
                "A worked example",
                "A link to a filled-in form",
                "Use it"
            ]
        );
        for line in formulas() {
            assert!(text.contains(&line), "{line}");
        }
        assert!(text.contains(&usual_markup()));
        assert!(text.contains(ADVICE));
        assert!(text.contains(ILLUSTRATIONS));
        assert!(text.contains(
            "- [Parts markup matrix calculator](https://open.example/tools/parts-matrix)\n"
        ));
    }

    #[test]
    fn the_worked_example_is_the_one_the_page_opens_with() {
        let text = text();
        assert!(text.contains(
            "| 3 | $25.01 to $100.00 | 80% | 44.444% | $100.00 | $180.00 | $80.00 | 35% |\n"
        ));
        assert!(text.contains(
            "| 6 | $500.01 and up | 40% | 28.571% | $1,000.00 | $1,400.00 | $400.00 | 5% |\n"
        ));
        assert!(text.contains(
            "A part that costs $42.50 is in row 3. It sells for $76.50, a profit of $34.00."
        ));
        assert!(text.contains(
            "Blended margin: 43.662%. That is inside the typical range of 40% to 50% for parts gross profit. The usual target is 50%."
        ));
        assert!(text.contains(
            "```\nhttps://open.example/tools/parts-matrix?kind=markup&cost1=5.00&rate1=150&share1=5&cost2=25.00"
        ));
        assert!(text.contains("&share6=5&part=42.50\n```"));
    }

    #[test]
    fn the_target_is_given_with_a_link_to_each_of_its_sources() {
        let text = text();
        assert!(text.contains(&format!(
            "a typical range of {} to {}, and a usual target of {}",
            PARTS.range.low, PARTS.range.high, PARTS.usual
        )));
        for source in PARTS.range_sources.iter().chain(PARTS.usual_sources) {
            assert!(
                text.contains(&format!(
                    "- [{}: {}]({})\n",
                    source.name, source.title, source.url
                )),
                "{}",
                source.url
            );
        }
        assert!(text.contains("not a survey of what shops earn"));
    }
}
