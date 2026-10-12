//! The Markdown version of the tire size page, for AI agents.
//!
//! It says what the calculator does, what each input is, the formulas and
//! one worked example. It does not compute: the example is the one the
//! page opens with, worked out by the same code and written down.

use crate::site::markdown::Doc;
use crate::site::pages::{section, with_code, with_links, with_table};
use crate::site::tools::pieces::ADVICE;
use crate::site::tools::tire_size::form::{Form, example};
use crate::site::tools::tire_size::result::{LIMITS, formulas, shown};
use crate::site::tools::tire_size::{PATH, SUMMARY, TITLE};

fn refs(paragraphs: &[String]) -> Vec<&str> {
    paragraphs.iter().map(String::as_str).collect()
}

pub fn doc(base: &str) -> Doc {
    let address = format!("{base}{PATH}");
    let enter = vec![
        "Two tire sizes: the one on the vehicle now, and the one to compare with it.".to_owned(),
        "A size is written as on the sidewall: 275/65R18, P215/65R15, LT285/70R17 or 225/70R19.5. Spaces and the case of letters do not matter. Sizes in inches, such as 35x12.50R17, are not read.".to_owned(),
    ];
    let mut sections = vec![
        section("What you enter", &refs(&enter)),
        section("How this is worked out", &refs(&formulas())),
    ];
    let (_, sizes) = Form::read(&example());
    if let Some((first, second)) = sizes {
        let shown = shown(&first, &second);
        sections.push(with_table(
            section(
                "A worked example: the two sizes",
                &["The example compares 275/65R18 with LT285/70R17."],
            ),
            shown.sizes,
        ));
        sections.push(with_table(
            section(
                "A worked example: the second size against the first",
                &[shown.speedometer.as_str()],
            ),
            shown.difference,
        ));
    }
    sections.push(section("What this does not say", &[LIMITS]));
    sections.push(with_code(
        section(
            "A link to a filled-in form",
            &["Both sizes are in the address, so a comparison can be linked to, bookmarked and printed. Nothing is stored. The fields are first and second. This address is the example above:"],
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

    #[test]
    fn it_states_the_formulas_and_the_worked_example() {
        let text = markdown::doc(&doc("https://open.example"));
        let headings: Vec<&str> = text
            .lines()
            .filter_map(|line| line.strip_prefix("## "))
            .collect();
        assert_eq!(
            headings,
            [
                "What you enter",
                "How this is worked out",
                "A worked example: the two sizes",
                "A worked example: the second size against the first",
                "What this does not say",
                "A link to a filled-in form",
                "Use it",
            ]
        );
        assert!(
            text.contains("| Overall diameter | 814.7 mm (32.07 in) | 830.8 mm (32.71 in) |\n")
        );
        assert!(
            text.contains(
                "https://open.example/tools/tire-size?first=275/65R18&second=LT285/70R17"
            )
        );
    }
}
