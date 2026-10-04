//! The Markdown version of the Canadian invoice page, for AI agents.
//!
//! It says what the calculator does, what each input is, the formulas, the
//! rates it holds with their sources, and one worked example. It does not
//! compute: the example is the one the page opens with, worked out by the
//! same code and written down. Every rate in it is read from the rates
//! file.

use shop_math::{MAX_COUNT, Money};

use crate::site::markdown::{Doc, Table};
use crate::site::pages::{section, with_code, with_links, with_table};
use crate::site::tools::canada_invoice_tax::form::{Form, example};
use crate::site::tools::canada_invoice_tax::rates::{PROVINCES, Source};
use crate::site::tools::canada_invoice_tax::result::{checked, formulas, rate_lines, scope, table};
use crate::site::tools::canada_invoice_tax::{PATH, SUMMARY, TITLE};
use crate::site::tools::pieces::ADVICE;

fn refs(paragraphs: &[String]) -> Vec<&str> {
    paragraphs.iter().map(String::as_str).collect()
}

/// A source as the text of its link: its title, and how it was read when
/// that was from an archived copy.
pub fn source_text(source: &Source) -> String {
    match source.archived {
        Some(date) => format!("{} (read from an archived copy of {date})", source.title),
        None => source.title.to_owned(),
    }
}

/// The rates held, one row for each province the calculator covers.
fn rates() -> Table {
    Table {
        caption: "The rates held for each province and territory covered".to_owned(),
        head: ["Province or territory", "Checked", "Taxes and tire fees"]
            .map(str::to_owned)
            .to_vec(),
        rows: PROVINCES
            .iter()
            .filter(|province| province.confirmed)
            .map(|province| {
                let mut lines = rate_lines(province);
                lines.extend(province.notes.iter().map(|note| (*note).to_owned()));
                vec![
                    format!("{} ({})", province.name, province.code),
                    province.as_of.to_owned(),
                    lines.join(" "),
                ]
            })
            .collect(),
        prose: true,
    }
}

/// Every page a covered province's figures were read from, each once.
fn sources() -> Vec<(String, String)> {
    let mut links: Vec<(String, String)> = Vec::new();
    for province in PROVINCES.iter().filter(|province| province.confirmed) {
        for source in province.sources() {
            if !links.iter().any(|(_, url)| url == source.url) {
                links.push((source_text(source), source.url.to_owned()));
            }
        }
    }
    links
}

pub fn doc(base: &str) -> Doc {
    let address = format!("{base}{PATH}");
    let enter = vec![
        "A province or territory; the invoice's labour; its parts; optionally, its shop supplies charge; and, optionally, the number of new tires sold in each tire class the province has. The classes differ by province: some go by the kind of vehicle, some by rim size, and Quebec by the tire's overall diameter.".to_owned(),
        format!(
            "Shop supplies left empty is {}, and a tire count left empty is no tires. Labour and parts are asked for: an invoice with none of one writes a zero. Something must be on the invoice.",
            Money::ZERO
        ),
        format!(
            "Money may be written as 1234.5, 1,234.50 or $1,234.50, up to {}. A number of tires is a whole number up to {MAX_COUNT}.",
            Money::MAX_INPUT
        ),
    ];
    let held = vec![
        "Every rate, rule and fee is held in one file, each with the page it was read from and the day it was checked. The page shows that day and links to each source for the province chosen. The sources are the tax authorities and the tire stewardship bodies themselves.".to_owned(),
        "A province or territory is worked out only when every rule the calculator uses for it has a source.".to_owned(),
    ];
    let links = sources();
    let links: Vec<(&str, String)> = links
        .iter()
        .map(|(text, url)| (text.as_str(), url.clone()))
        .collect();
    let mut sections = vec![
        section("What you enter", &refs(&enter)),
        section("How this is worked out", &refs(&formulas())),
        with_links(
            with_table(section("The rates", &refs(&held)), rates()),
            &links,
        ),
    ];
    let waiting: Vec<String> = PROVINCES
        .iter()
        .filter(|province| !province.confirmed)
        .map(|province| format!("{}: {}", province.name, province.missing.join(" ")))
        .collect();
    if !waiting.is_empty() {
        let mut lines =
            vec!["Nothing is worked out for these, because a rule has no source yet:".to_owned()];
        lines.extend(waiting);
        sections.push(section("Not yet covered", &refs(&lines)));
    }
    sections.push(section("What this covers", &refs(&scope())));
    let (form, worked) = Form::read(&example());
    if let Some(worked) = worked {
        let mut entered: Vec<String> = vec![format!("Province: {}", worked.province.name)];
        let fields = [&form.labour, &form.parts, &form.supplies];
        entered.extend(
            fields
                .into_iter()
                .chain(&form.tires)
                .map(|field| format!("{}: {}", field.label, field.value)),
        );
        let lines = vec![
            format!(
                "The page opens with this example, a repair with four new tires. {}.",
                entered.join("; ")
            ),
            checked(worked.province),
        ];
        sections.push(with_table(
            section("A worked example", &refs(&lines)),
            table(&worked),
        ));
    }
    let classes: Vec<String> = PROVINCES
        .iter()
        .filter(|province| province.confirmed && !province.tire_classes.is_empty())
        .map(|province| {
            let fields: Vec<String> = province
                .tire_classes
                .iter()
                .map(|class| class.field())
                .collect();
            format!("{}: {}", province.code, fields.join(", "))
        })
        .collect();
    let link = vec![
        "Every field is in the address, so a filled-in form can be linked to, bookmarked and printed. Nothing is stored. The fields are province (the two-letter code), labour, parts, supplies, and one field for each tire class of the province.".to_owned(),
        format!("The tire fields, by province. {}.", classes.join(". ")),
        "This address is the example above:".to_owned(),
    ];
    sections.push(with_code(
        section("A link to a filled-in form", &refs(&link)),
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
                "The rates",
                "Not yet covered",
                "What this covers",
                "A worked example",
                "A link to a filled-in form",
                "Use it"
            ]
        );
        for stated in [
            "This page describes the calculator and does not compute.",
            "Shop supplies left empty is $0.00, and a tire count left empty is no tires.",
            "up to $99,999,999.99. A number of tires is a whole number up to 1000.",
            "Subtotal = labour + parts + shop supplies.",
            "No tax is calculated on another tax.",
            "a retail repair invoice to a consumer, in Canadian dollars (CAD)",
            "oil, oil filters, antifreeze and batteries",
            "This is arithmetic, not accounting or tax advice.",
            "- [Canadian invoice tax and tire fee calculator](https://open.example/tools/canada-invoice-tax)\n",
        ] {
            assert!(text.contains(stated), "missing: {stated}\n{text}");
        }
    }

    #[test]
    fn it_states_the_rates_of_each_province_covered_with_every_source() {
        let text = text();
        for province in &PROVINCES {
            let row = format!(
                "| {} ({}) | {} | ",
                province.name, province.code, province.as_of
            );
            assert_eq!(text.contains(&row), province.confirmed, "{}", province.code);
            if province.confirmed {
                for line in rate_lines(province) {
                    assert!(text.contains(&line), "{}: {line}", province.code);
                }
                for note in province.notes {
                    assert!(text.contains(note), "{}: {note}", province.code);
                }
                for source in province.sources() {
                    assert!(
                        text.contains(&format!("]({})\n", source.url)),
                        "{}: {}",
                        province.code,
                        source.url
                    );
                }
            } else {
                // Not covered: named with what is missing, and no rate.
                let line = format!("{}: {}", province.name, province.missing.join(" "));
                assert!(text.contains(&line), "{line}");
            }
        }
        // A source that many provinces use is linked once.
        let first = PROVINCES[0].sources()[0];
        assert_eq!(text.matches(&format!("]({})\n", first.url)).count(), 1);
        // A page read from an archived copy says so.
        assert!(text.contains(" (read from an archived copy of "));
        let archived = Source {
            title: "A page",
            url: "https://example.org/",
            archived: Some("2025-08-09"),
        };
        assert_eq!(
            source_text(&archived),
            "A page (read from an archived copy of 2025-08-09)"
        );
    }

    #[test]
    fn its_worked_example_is_the_one_the_page_opens_with() {
        let text = text();
        for stated in [
            "The page opens with this example, a repair with four new tires. Province: British Columbia; Labour: 180.00; Parts: 640.00; Shop supplies (optional): 14.37; New tires: passenger and light truck: 4; New tires: medium truck: 0.",
            "Invoice in British Columbia, in Canadian dollars (CAD):",
            "| Line | Amount |",
            "| Subtotal | $834.37 |",
            "| Tire fee, passenger and light truck: 4 at $6.50 | $26.00 |",
            "| GST 5% on $860.37 | $43.02 |",
            "| PST 7% on $860.37 | $60.23 |",
            "| Total (CAD) | $963.62 |",
            "https://open.example/tools/canada-invoice-tax?province=BC&labour=180.00&parts=640.00&supplies=14.37&tires_passenger_light_truck=4",
            "BC: tires_passenger_light_truck, tires_medium_truck. ",
            "QC: tires_diameter_83_82_cm_or_less, tires_diameter_over_83_82_cm_to_123_19_cm. ",
        ] {
            assert!(text.contains(stated), "missing: {stated}\n{text}");
        }
        let british_columbia = crate::site::tools::canada_invoice_tax::rates::province("BC");
        assert!(text.contains(&checked(british_columbia.unwrap())));
    }
}
