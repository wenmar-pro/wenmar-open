//! The Markdown version of the Ontario payroll page, for AI agents.
//!
//! It says what the calculator does, what each input is, how it is worked
//! out, the tables it uses with their addresses, how to check a result,
//! one worked example and its factors. It does not compute: the example is
//! worked out by the same code and written down, on a fixed day, so the
//! Markdown does not change from day to day.

use payroll_calc::{CHECK_URL, PDOC_URL};

use crate::site::markdown::Doc;
use crate::site::pages::{section, with_links, with_table};
use crate::site::tools::payroll_deductions::result;
use crate::site::tools::payroll_deductions::result::{
    check, claims, edition_line, factors, how, scope, table,
};
use crate::site::tools::payroll_deductions::{PATH, SUMMARY, TITLE};
use crate::site::tools::pieces::ADVICE;

fn refs(paragraphs: &[String]) -> Vec<&str> {
    paragraphs.iter().map(String::as_str).collect()
}

/// The pay date the worked example is on: the first day of the newest
/// edition, so the Markdown does not change from day to day.
fn example_date() -> chrono::NaiveDate {
    let held = payroll_calc::editions();
    held[held.len() - 1].effective
}

/// The worked example, worked out on [`example_date`].
fn example() -> payroll_calc::Worked {
    payroll_calc::deductions(
        &payroll_calc::Query {
            frequency: Some("biweekly".to_owned()),
            gross: Some("2400.00".to_owned()),
            pay_date: Some(example_date().to_string()),
            ..payroll_calc::Query::default()
        },
        example_date(),
    )
    .expect("the example's pay date is the first day of the newest edition")
}

pub fn doc(base: &str) -> Doc {
    let address = format!("{base}{PATH}");
    let enter = vec![
        "A pay frequency (weekly, biweekly, semi-monthly or monthly), the gross pay for one pay period, the federal and Ontario claim amounts, the pay date and, optionally, what was deducted and earned on earlier pays this calendar year.".to_owned(),
        result::CLAIMS_HELP.to_owned(),
        "A pay date left empty is today. Money may be written as 1234.5, 1,234.50 or $1,234.50.".to_owned(),
    ];
    let held = payroll_calc::editions();
    let tables: Vec<String> = held.iter().map(edition_line).collect();
    let table_links: Vec<(String, String)> = held
        .iter()
        .filter_map(|edition| edition.url.map(|url| (edition.title(), (*url).to_owned())))
        .collect();
    let table_links: Vec<(&str, String)> = table_links
        .iter()
        .map(|(title, url)| (title.as_str(), url.clone()))
        .collect();
    let check_lines: Vec<String> = check()
        .iter()
        .enumerate()
        .map(|(number, line)| format!("{}. {line}", number + 1))
        .collect();
    let worked = example();
    let example_lines = vec![
        format!(
            "The page opens with this example: a biweekly pay of $2,400.00, with the claims left to the basic amounts and the pay date {}.",
            example_date()
        ),
        claims(&worked).join(" "),
    ];
    let sections = vec![
        section("An estimate", &[payroll_calc::NOTE]),
        section("What you enter", &refs(&enter)),
        section("How this is worked out", &refs(&how())),
        with_links(section("The tables", &refs(&tables)), &table_links),
        with_links(
            section("Check it yourself", &refs(&check_lines)),
            &[
                (
                    "CRA's Payroll Deductions Online Calculator (PDOC)",
                    PDOC_URL.to_owned(),
                ),
                (
                    "The pays compared with PDOC, and how (payroll-ca)",
                    CHECK_URL.to_owned(),
                ),
            ],
        ),
        with_table(
            section("A worked example", &refs(&example_lines)),
            table(&worked),
        ),
        with_table(
            section("The factors of the worked example", &[]),
            factors(&worked),
        ),
        section(
            "What this covers",
            &refs(&{
                let mut lines = scope();
                lines.push(ADVICE.to_owned());
                lines
            }),
        ),
        with_links(
            section("Use it", &["The calculator as a web page:"]),
            &[(TITLE, address)],
        ),
    ];
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
    use crate::site::markdown;

    use super::*;

    fn text() -> String {
        markdown::doc(&doc("https://open.example"))
    }

    #[test]
    fn it_says_what_the_calculator_does_and_how_to_check_it() {
        let text = text();
        assert!(text.starts_with("# Ontario payroll deductions calculator\n"));
        for stated in [
            "## An estimate\n",
            "## What you enter\n",
            "## How this is worked out\n",
            "## The tables\n",
            "## Check it yourself\n",
            "## A worked example\n",
            "## The factors of the worked example\n",
            "## What this covers\n",
            "## Use it\n",
            "An estimate. The employer remains liable to CRA",
            "Leave a claim empty for the basic personal amount (claim code 1).",
            "A pay date left empty is today. Money may be written as 1234.5, 1,234.50 or $1,234.50.",
            "Tables of CRA's T4127 Payroll Deductions Formulas, 123rd edition, for pay dates from 2026-07-01 to 2026-12-31.",
            "https://www.canada.ca/content/dam/cra-arc/formspubs/pub/t4127-jul/t4127-jul-26e.pdf",
            "1. Open CRA's Payroll Deductions Online Calculator (PDOC), accept the terms and choose Salary.",
            "PDOC's federal tax, provincial tax, CPP, CPP2 and EI should match this page to the cent.",
            "The page opens with this example: a biweekly pay of $2,400.00, with the claims left to the basic amounts and the pay date 2026-07-01.",
            "Federal claim: $16,452.00, the basic personal amount of the tables (claim code 1).",
            "| Net pay | $1,880.15 |",
            "Ontario only, by T4127 Option 1, the regular method, for one regular pay.",
            "This is arithmetic, not accounting or tax advice.",
            "The calculator as a web page:",
            "- [Ontario payroll deductions calculator](https://open.example/tools/payroll-deductions)\n",
        ] {
            assert!(text.contains(stated), "missing: {stated}\n{text}");
        }
        assert!(text.contains(PDOC_URL));
        assert!(text.contains(CHECK_URL));
    }
}
