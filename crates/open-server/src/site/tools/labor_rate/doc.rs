//! The Markdown version of the labor rate page, for AI agents.
//!
//! It says what the calculator does, what each input is, the formulas and
//! one worked example for each of its two parts. It does not compute: the
//! example is the one the page opens with, worked out by the same code and
//! written down.

use shop_math::labor_rate::{LEAST_PRODUCTIVITY, MOST_PRODUCTIVITY, MOST_TARGET};
use shop_math::targets::LABOR;
use shop_math::{MAX_COUNT, Money, Percent};

use crate::site::markdown::Doc;
use crate::site::pages::{section, with_code, with_links, with_table};
use crate::site::tools::field::Field;
use crate::site::tools::labor_rate::form::{Form, defaults, example};
use crate::site::tools::labor_rate::result::{LABOR_TARGET_OF, formulas, getting, needed};
use crate::site::tools::labor_rate::{PATH, SUMMARY, TITLE};
use crate::site::tools::pieces::ADVICE;
use crate::site::tools::target::source_links;

fn refs(paragraphs: &[String]) -> Vec<&str> {
    paragraphs.iter().map(String::as_str).collect()
}

/// What the example has in each field of a part, as one sentence.
fn entered(fields: &[&Field]) -> String {
    let each: Vec<String> = fields
        .iter()
        .map(|field| format!("{}: {}", field.label, field.value))
        .collect();
    format!("The example has these figures. {}.", each.join("; "))
}

pub fn doc(base: &str) -> Doc {
    let address = format!("{base}{PATH}");
    let enter = vec![
        "The form has two parts that share one button. A part with nothing typed in it is not worked out, so either can be used alone.".to_owned(),
        format!(
            "For the rate the shop needs, for one month: the number of technicians; paid hours per technician; productivity, which is hours billed as a percent of hours paid, from {LEAST_PRODUCTIVITY} to {MOST_PRODUCTIVITY}; technician cost for all technicians together, which is wages and what the shop pays on top of wages; overhead, which is everything else the shop pays (rent, service advisors, insurance, software, equipment); optionally, gross profit on parts, which is what parts sales leave after their cost; target profit as a percent of labor sales, from {} to {MOST_TARGET}; and target labor gross profit, in the same range.",
            Percent::ZERO
        ),
        defaults(),
        "For the rate the shop is getting, for one period: labor sales; hours billed; optionally, the posted rate; and optionally, technician cost for the period.".to_owned(),
        format!(
            "Money may be written as 1234.5, 1,234.50 or $1,234.50, up to {}. A percent may be written with or without the sign. The number of technicians is a whole number, up to {MAX_COUNT}.",
            Money::MAX_INPUT
        ),
    ];
    let target = vec![
        format!(
            "Labor gross profit is shown beside a target for {LABOR_TARGET_OF}: a typical range of {} to {}, and a usual target of {}. The page says whether the figure is below, inside or above the range. The usual target is also what the target labor gross profit field takes when it is left empty.",
            LABOR.range.low, LABOR.range.high, LABOR.usual
        ),
        "The range and the target are for a general repair shop, and come from the pages below. They are what those pages call typical and what they say shops aim for, not a survey of what shops earn.".to_owned(),
    ];
    let links: Vec<(String, String)> = source_links(&LABOR);
    let links: Vec<(&str, String)> = links
        .iter()
        .map(|(text, address)| (text.as_str(), address.clone()))
        .collect();
    let mut sections = vec![
        section("What you enter", &refs(&enter)),
        section("How this is worked out", &refs(&formulas())),
        with_links(section("The target", &refs(&target)), &links),
    ];
    let (form, rate_needed, effective) = Form::read(&example());
    if let Some(result) = rate_needed {
        let shown = needed(&result);
        let mut worked = vec![entered(&form.needed.fields())];
        worked.extend(shown.covered);
        worked.extend(shown.gross_profit.map(|profit| profit.sentence()));
        worked.push(shown.compared);
        sections.push(with_table(
            section("A worked example: the rate the shop needs", &refs(&worked)),
            shown.table,
        ));
    }
    if let Some(result) = effective {
        let shown = getting(&result);
        let mut worked = vec![entered(&form.getting.fields())];
        worked.extend(shown.posted);
        worked.extend(shown.gross_profit.map(|profit| profit.sentence()));
        sections.push(with_table(
            section(
                "A worked example: the rate the shop is getting",
                &refs(&worked),
            ),
            shown.table,
        ));
    }
    sections.push(with_code(
        section(
            "A link to a filled-in form",
            &["Every field is in the address, so a filled-in form can be linked to, bookmarked and printed. Nothing is stored. The fields of the first part are technicians, paid_hours, productivity, technician_cost, overhead, parts_profit, target_profit and target_labor. The fields of the second part are labor_sales, hours_billed, posted_rate and period_cost. This address is the example above:"],
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
                "A worked example: the rate the shop needs",
                "A worked example: the rate the shop is getting",
                "A link to a filled-in form",
                "Use it"
            ]
        );
        for line in formulas() {
            assert!(text.contains(&line), "{line}");
        }
        assert!(text.contains(&defaults()));
        assert!(text.contains(
            "productivity, which is hours billed as a percent of hours paid, from 1% to 200%;"
        ));
        assert!(text.contains("target profit as a percent of labor sales, from 0% to 99%;"));
        assert!(text.contains("up to $99,999,999.99."));
        assert!(text.contains("a whole number, up to 1000."));
        assert!(text.contains(ADVICE));
        assert!(text.contains(
            "- [Labor rate calculator for auto repair shops](https://open.example/tools/labor-rate)\n"
        ));
    }

    #[test]
    fn the_worked_example_is_the_one_the_page_opens_with() {
        let text = text();
        assert!(text.contains(
            "The example has these figures. Technicians: 3; Paid hours per technician, per month: 173; Productivity, percent: 85; Technician cost per month: 18000.00; Overhead per month: 25000.00; Gross profit on parts per month (optional): 12000.00; Target profit, percent of labor sales: 10; Target labor gross profit, percent: 70."
        ), "{text}");
        for stated in [
            "| Hours billed per month | 441.15 |\n",
            "| Labor sales needed per month | $34,444.44 |\n",
            "| Rate needed, per hour billed | $78.08 |\n",
            "| Break-even rate, with no profit | $70.27 |\n",
            "| Rate from the target labor gross profit | $136.01 |\n",
            "Labor gross profit at the rate needed: 47.742%. That is below the typical range of 60% to 75% for labor gross profit. The usual target is 70%.",
            "The rate needed is the lower of the two rates: the target labor gross profit leaves more profit than was asked for.",
            "Labor sales in the period: 48000.00; Hours billed in the period: 520; Posted rate, per hour (optional): 120.00; Technician cost in the period (optional): 18000.00.",
            "| Effective labor rate, per hour billed | $92.31 |\n",
            "| Effective rate as a percent of the posted rate | 76.925% |\n",
            "| Posted rate less effective rate, per hour | $27.69 |\n",
            "| Posted rate × hours billed, less labor sales | $14,400.00 |\n",
            "The shop is getting less for an hour billed than it posts.",
            "Labor gross profit: 62.5%. That is inside the typical range of 60% to 75% for labor gross profit. The usual target is 70%.",
            "```\nhttps://open.example/tools/labor-rate?technicians=3&paid_hours=173&productivity=85&technician_cost=18000.00&overhead=25000.00&parts_profit=12000.00&target_profit=10&target_labor=70&labor_sales=48000.00&hours_billed=520&posted_rate=120.00&period_cost=18000.00\n```",
        ] {
            assert!(text.contains(stated), "{stated}\n{text}");
        }
    }

    #[test]
    fn the_target_is_given_with_a_link_to_each_of_its_sources() {
        let text = text();
        assert!(text.contains(&format!(
            "a typical range of {} to {}, and a usual target of {}",
            LABOR.range.low, LABOR.range.high, LABOR.usual
        )));
        for source in LABOR.range_sources.iter().chain(LABOR.usual_sources) {
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
        assert!(text.contains("for a general repair shop"));
    }
}
