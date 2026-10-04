//! The Markdown version of the gross profit page, for AI agents.
//!
//! It says what the calculator does, what each input is, the formulas and
//! one worked example. It does not compute: the example is the one the
//! page opens with, worked out by the same code and written down.

use shop_math::Money;
use shop_math::targets::{LABOR, OVERALL, PARTS, Target};

use crate::site::markdown::Doc;
use crate::site::pages::{section, with_code, with_links, with_table};
use crate::site::tools::gross_profit::form::{Form, example};
use crate::site::tools::gross_profit::result::{
    LABOR_TARGET_OF, OVERALL_TARGET_OF, PARTS_TARGET_OF, SUBLET_ALONE, formulas, shown,
};
use crate::site::tools::gross_profit::{PATH, SUMMARY, TITLE};
use crate::site::tools::pieces::ADVICE;
use crate::site::tools::target::source_links;

fn refs(paragraphs: &[String]) -> Vec<&str> {
    paragraphs.iter().map(String::as_str).collect()
}

/// One target, as a sentence.
fn target(of: &str, target: &Target) -> String {
    format!(
        "For {of}: a typical range of {} to {}, and a usual target of {}.",
        target.range.low, target.range.high, target.usual
    )
}

pub fn doc(base: &str) -> Doc {
    let address = format!("{base}{PATH}");
    let enter = vec![
        "For one period, such as a month: labor sales and technician cost; parts sales and parts cost; and, optionally, sublet sales and sublet cost. Technician cost is what the technicians are paid, with what the shop pays on top of wages. Parts cost and sublet cost are what the shop paid for them.".to_owned(),
        format!(
            "A sublet field left empty is {}. Labor and parts are asked for: a shop that sold none of one writes a zero. At least one line must have sales.",
            Money::ZERO
        ),
        format!(
            "Money may be written as 1234.5, 1,234.50 or $1,234.50, up to {}.",
            Money::MAX_INPUT
        ),
    ];
    let targets = vec![
        "Labor, parts and the line for all three are each shown beside a target, and the page says whether the figure is below, inside or above the range.".to_owned(),
        target(LABOR_TARGET_OF, &LABOR),
        target(PARTS_TARGET_OF, &PARTS),
        target(OVERALL_TARGET_OF, &OVERALL),
        SUBLET_ALONE.to_owned(),
        "The ranges and the targets are for a general repair shop, and come from the pages below. They are what those pages call typical and what they say shops aim for, not a survey of what shops earn.".to_owned(),
    ];
    let mut links: Vec<(String, String)> = Vec::new();
    for link in [&LABOR, &PARTS, &OVERALL]
        .into_iter()
        .flat_map(source_links)
    {
        if !links.contains(&link) {
            links.push(link);
        }
    }
    let links: Vec<(&str, String)> = links
        .iter()
        .map(|(text, address)| (text.as_str(), address.clone()))
        .collect();
    let mut sections = vec![
        section("What you enter", &refs(&enter)),
        section("How this is worked out", &refs(&formulas())),
        with_links(section("The targets", &refs(&targets)), &links),
    ];
    let (form, result) = Form::read(&example());
    if let Some(result) = result {
        let shown = shown(&result);
        let entered: Vec<String> = form
            .fields()
            .iter()
            .map(|field| format!("{}: {}", field.label, field.value))
            .collect();
        let mut worked = vec![format!(
            "The page opens with this example, one month of a shop. {}.",
            entered.join("; ")
        )];
        worked.extend(shown.labor.map(|labor| labor.sentence()));
        worked.extend(shown.labor_short);
        worked.extend(shown.parts.map(|parts| parts.sentence()));
        worked.extend(shown.parts_short);
        worked.extend(shown.overall.map(|overall| overall.sentence()));
        sections.push(with_table(
            section("A worked example", &refs(&worked)),
            shown.table,
        ));
    }
    sections.push(with_code(
        section(
            "A link to a filled-in form",
            &["Every field is in the address, so a filled-in form can be linked to, bookmarked and printed. Nothing is stored. The fields are labor_sales, technician_cost, parts_sales, parts_cost, sublet_sales and sublet_cost. This address is the example above:"],
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
                "The targets",
                "A worked example",
                "A link to a filled-in form",
                "Use it"
            ]
        );
        for line in formulas() {
            assert!(text.contains(&line), "{line}");
        }
        assert!(text.contains("A sublet field left empty is $0.00."));
        assert!(text.contains("up to $99,999,999.99."));
        assert!(text.contains(SUBLET_ALONE));
        assert!(text.contains(ADVICE));
        assert!(text.contains(
            "- [Gross profit calculator for auto repair shops](https://open.example/tools/gross-profit)\n"
        ));
    }

    #[test]
    fn the_worked_example_is_the_one_the_page_opens_with() {
        let text = text();
        for stated in [
            "The page opens with this example, one month of a shop. Labor sales: 48000.00; Technician cost: 18000.00; Parts sales: 40000.00; Parts cost: 26000.00; Sublet sales (optional): 2000.00; Sublet cost (optional): 1600.00.",
            "| Labor | $48,000.00 | $18,000.00 | $30,000.00 | 62.5% |\n",
            "| Parts | $40,000.00 | $26,000.00 | $14,000.00 | 35% |\n",
            "| Sublet | $2,000.00 | $1,600.00 | $400.00 | 20% |\n",
            "| All three | $90,000.00 | $45,600.00 | $44,400.00 | 49.333% |\n",
            "Labor gross profit: 62.5%. That is inside the typical range of 60% to 75% for labor gross profit. The usual target is 70%.",
            "Parts gross profit: 35%. That is below the typical range of 40% to 50% for parts gross profit. The usual target is 50%.",
            "Parts are below the typical range. At the same parts cost of $26,000.00, sales of $52,000.00 would reach the usual target of 50%. That is $12,000.00 more than the $40,000.00 sold.",
            "Overall gross profit: 49.333%. That is below the typical range of 50% to 60% for gross profit across labor, parts and sublet. The usual target is 60%.",
            "```\nhttps://open.example/tools/gross-profit?labor_sales=48000.00&technician_cost=18000.00&parts_sales=40000.00&parts_cost=26000.00&sublet_sales=2000.00&sublet_cost=1600.00\n```",
        ] {
            assert!(text.contains(stated), "{stated}\n{text}");
        }
        // Labor is inside its range in the example: nothing to act on.
        assert!(!text.contains("Labor is below"));
    }

    #[test]
    fn each_target_is_given_with_a_link_to_each_of_its_sources_once() {
        let text = text();
        for (of, target) in [
            (LABOR_TARGET_OF, &LABOR),
            (PARTS_TARGET_OF, &PARTS),
            (OVERALL_TARGET_OF, &OVERALL),
        ] {
            assert!(
                text.contains(&format!(
                    "For {of}: a typical range of {} to {}, and a usual target of {}.",
                    target.range.low, target.range.high, target.usual
                )),
                "{of}"
            );
            for source in target.range_sources.iter().chain(target.usual_sources) {
                let link = format!("- [{}: {}]({})\n", source.name, source.title, source.url);
                assert_eq!(text.matches(&link).count(), 1, "{}", source.url);
            }
        }
        assert!(text.contains("not a survey of what shops earn"));
        assert!(text.contains("for a general repair shop"));
    }
}
