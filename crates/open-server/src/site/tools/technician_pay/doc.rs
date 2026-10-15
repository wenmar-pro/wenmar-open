//! The Markdown version of the technician pay page, for AI agents.
//!
//! The same sections as the page, one heading level down, so this file
//! cannot say something the page does not.

use crate::site::markdown::Doc;
use crate::site::pages::{section, with_links, with_table};
use crate::site::tools::technician_pay::result;
use crate::site::tools::technician_pay::{PATH, SUMMARY, TITLE};

const CHECK_URL: &str = "https://github.com/wenmar-pro/payroll-ca";

/// `&[String]` as `Vec<&str>`, for the page's section helpers.
fn refs(paragraphs: &[String]) -> Vec<&str> {
    paragraphs.iter().map(String::as_str).collect()
}

/// The Markdown version of the page, on the public address `base`.
pub fn doc(base: &str) -> Doc {
    let address = format!("{base}{PATH}");
    let worked = result::example();
    let example_lines = vec![format!(
        "The page opens with this example: 80 flagged hours at $32.00 and 6 hours of shop time at $25.00, paid biweekly on {}.",
        result::example_date()
    )];
    let sections = vec![
        section(
            "An estimate",
            &[
                "The deductions are the Ontario payroll calculator's, by CRA's T4127 Payroll Deductions Formulas. An estimate, not a pay slip: the employer remains liable.",
            ],
        ),
        section(
            "What you enter",
            &[
                "The flagged hours and the flat rate per flagged hour, the hourly hours and the hourly rate, and the overtime hours and the overtime rate. An empty field is zero.",
                "The pay frequency, the pay date and, optionally, the federal and Ontario claim amounts, for the Ontario deductions on the gross. Leave the frequency empty for the gross only.",
            ],
        ),
        section("How this is worked out", &refs(&result::formulas())),
        with_links(
            section(
                "Check it yourself",
                &[
                    "The deductions are checked against CRA's Payroll Deductions Online Calculator, the same way the payroll calculator is.",
                ],
            ),
            &[(
                "The pays compared with PDOC, and how (payroll-ca).",
                CHECK_URL.to_owned(),
            )],
        ),
        with_table(
            section("A worked example", &refs(&example_lines)),
            result::gross(&worked),
        ),
        with_table(
            section("The Ontario deductions of the worked example", &[]),
            result::deductions(&worked).expect("the example has a frequency"),
        ),
        section(
            "What this covers",
            &[
                "Ontario only, for one regular pay. A bonus, retroactive pay or commission is worked out another way and is not covered.",
                "A flagged hour is one the shop bills as a flat rate, not by the clock. The shop sets the flat rate, the hourly rate and the overtime rate; this page works out the pay and the Ontario deductions on it.",
            ],
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
    use super::*;

    #[test]
    fn the_doc_has_the_sections_of_the_page() {
        let doc = doc("https://open.example");
        assert_eq!(doc.title, TITLE);
        let names: Vec<&str> = doc
            .sections
            .iter()
            .map(|section| section.heading.as_str())
            .collect();
        assert!(names.contains(&"A worked example"));
        assert!(names.contains(&"How this is worked out"));
    }
}
