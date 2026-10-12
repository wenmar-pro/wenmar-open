//! The Markdown version of the wire gauge calculator, for AI agents.
//!
//! It says what the calculator takes, how the result is worked out, and the
//! resistance table with its sources and the day it was read. A table not yet
//! checked has its warning and no table. Every figure is read from the data
//! file, and the worked example is the one the page opens with.

use crate::site::markdown::{Doc, Table};
use crate::site::pages::{section, with_code, with_links, with_table};
use crate::site::tools::canada_invoice_tax::doc::source_text;
use crate::site::tools::wire_gauge::data::WIRE;
use crate::site::tools::wire_gauge::form::{Form, defaults, example};
use crate::site::tools::wire_gauge::result::{self, checked, formulas, shown};
use crate::site::tools::wire_gauge::{PATH, SUMMARY, TITLE};

fn refs(paragraphs: &[String]) -> Vec<&str> {
    paragraphs.iter().map(String::as_str).collect()
}

pub fn doc(base: &str) -> Doc {
    let address = format!("{base}{PATH}");
    let defaults = defaults();
    let mut sections = vec![section(
        "What you enter",
        &[
            "The system voltage, 12 V or 24 V; the current in amperes; the length of wire the current flows through, in metres or feet; and the voltage drop allowed, as a percent.",
            defaults.as_str(),
            result::LIMITS,
        ],
    )];
    let formulas = formulas();
    sections.push(section("How this is worked out", &refs(&formulas)));

    if WIRE.confirmed {
        let mut paragraphs = vec![checked(&WIRE)];
        paragraphs.extend(WIRE.conditions.iter().map(|line| line.to_string()));
        let texts: Vec<(String, &str)> = WIRE
            .sources
            .iter()
            .map(|source| (source_text(source), source.url))
            .collect();
        let links: Vec<(&str, String)> = texts
            .iter()
            .map(|(text, url)| (text.as_str(), (*url).to_owned()))
            .collect();
        let table = Table {
            caption: "Resistance of each gauge".to_owned(),
            head: vec!["Gauge".to_owned(), "Resistance".to_owned()],
            rows: WIRE
                .gauges
                .iter()
                .map(|gauge| {
                    vec![
                        gauge.name.to_owned(),
                        format!("{} {}", gauge.resistance, WIRE.unit.label()),
                    ]
                })
                .collect(),
            prose: false,
        };
        let wire_section = with_table(section("The wire table", &refs(&paragraphs)), table);
        sections.push(with_links(wire_section, &links));

        if let (_, Some(circuit)) = Form::read(&example()) {
            let shown = shown(&circuit, &WIRE);
            let worked = [
                "The example is 20 A through 5 m of wire at 12 V, with 3% allowed.".to_owned(),
                shown.pick.clone(),
            ];
            sections.push(with_table(
                section("A worked example", &refs(&worked)),
                shown.table,
            ));
        }
        sections.push(with_code(
            section(
                "A link to a filled-in form",
                &["Every field is in the address, so a filled-in form can be linked to, bookmarked and printed. Nothing is stored. The fields are system, current, length, unit and allowed. This address is the example above:"],
            ),
            format!("{address}?{}", example().query()),
        ));
    } else {
        let mut paragraphs = vec![result::NOT_CHECKED.to_owned()];
        paragraphs.extend(WIRE.missing.iter().map(|line| line.to_string()));
        sections.push(section("The wire table", &refs(&paragraphs)));
    }
    sections.push(with_links(
        section("Use it", &["The calculator as a web page:"]),
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
    fn the_calculator_states_its_formulas_and_its_table() {
        let text = markdown::doc(&doc("https://open.example"));
        assert!(
            text.starts_with("# Wire gauge calculator for 12 V and 24 V\n"),
            "{text}"
        );
        assert!(text.contains("\n## The wire table\n"), "{text}");
    }
}
