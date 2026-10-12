//! `/tools/battery-state-of-charge`: where a 12 V battery's open-circuit
//! voltage falls in a state-of-charge table, corrected for temperature.
//!
//! No table or correction is written here: each is in
//! `data/reference/battery-soc.toml` and reaches this code through [`data`].
//! A battery is worked out only when its entry is confirmed; until then the
//! page says "Not yet checked" and works nothing out.

pub mod data;
pub mod doc;
pub mod form;
pub mod result;

use askama::Template;
use axum::http::StatusCode;
use axum::response::Response;

use crate::site::markdown::Table;
use crate::site::tools::Tool;
use crate::site::tools::canada_invoice_tax::doc::source_text;
use crate::site::tools::canada_invoice_tax::form::Choice;
use crate::site::tools::field::Sent;
use crate::site::{self, Page};
use data::BATTERIES;
use form::Form;

/// The page's address, its heading and its one sentence.
pub const PATH: &str = "/tools/battery-state-of-charge";
pub const TITLE: &str = "Battery state of charge from resting voltage";
pub const SUMMARY: &str = "Reads a 12 V battery's state of charge from its open-circuit voltage, corrected for the battery's temperature, against a table read from a published source.";

/// The calculator's entry in the list of tools.
pub static TOOL: Tool = Tool {
    slug: "battery-state-of-charge",
    title: TITLE,
    description: "A free battery state of charge calculator: read a 12 V battery's charge from its resting voltage, corrected for temperature, with the source of the table.",
    summary: SUMMARY,
    pro_does: "is shop management software from the people who make this site.",
    doc: doc::doc,
    page,
};

/// What the page says of the battery chosen: its table, when it was
/// checked, and where each figure was read.
struct BatteryView {
    name: &'static str,
    checked: String,
    conditions: &'static [&'static str],
    notes: &'static [&'static str],
    chart: Table,
    /// Each source as the text of its link and its address, once each.
    sources: Vec<(String, &'static str)>,
}

impl BatteryView {
    fn of(battery: &'static data::Battery) -> BatteryView {
        let mut sources: Vec<(String, &'static str)> = Vec::new();
        for source in battery.correction_sources.iter().chain(battery.sources) {
            if !sources.iter().any(|(_, url)| *url == source.url) {
                sources.push((source_text(source), source.url));
            }
        }
        BatteryView {
            name: battery.name,
            checked: result::checked(battery),
            conditions: battery.conditions,
            notes: battery.notes,
            chart: result::chart(battery),
            sources,
        }
    }
}

#[derive(Template)]
#[template(path = "battery_soc.html")]
struct View {
    page: Page,
    tool: &'static Tool,
    /// Whether the form is filled with the page's own example.
    example: bool,
    form: Form,
    covered: Vec<Choice>,
    not_covered: Vec<Choice>,
    /// What to correct, when there is no result.
    troubles: Vec<(String, String)>,
    shown: Option<result::Shown>,
    /// The battery chosen, whether or not there is a result.
    battery: Option<BatteryView>,
    /// The batteries not yet confirmed, each with what is missing.
    waiting: Vec<(&'static str, String)>,
    none_checked: &'static str,
    limits: &'static str,
    how: Vec<String>,
}

/// The page: the example when the form was not sent, and otherwise what
/// was sent, with its result or with a message beside what could not be
/// read. It is `200` either way: a form being filled in is not a failure.
fn page(page: Page, tool: &'static Tool, sent: &Sent) -> Response {
    let example = !Form::was_sent(sent) && form::example(&BATTERIES).is_some();
    let sent = if Form::was_sent(sent) {
        sent.clone()
    } else {
        form::example(&BATTERIES).unwrap_or_default()
    };
    let (form, reading) = Form::read(&sent, &BATTERIES);
    let waiting = BATTERIES
        .iter()
        .filter(|battery| !battery.confirmed)
        .map(|battery| (battery.name, battery.missing.join(" ")))
        .collect();
    site::html(
        StatusCode::OK,
        &View {
            page,
            tool,
            example,
            covered: form.covered(),
            not_covered: form.not_covered(),
            troubles: form.troubles(),
            shown: reading.as_ref().map(result::shown),
            battery: form.chosen.map(BatteryView::of),
            waiting,
            none_checked: result::NONE_CHECKED,
            limits: result::LIMITS,
            how: result::formulas(),
            form,
        },
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_entry_and_the_page_agree_on_the_address() {
        assert_eq!(TOOL.path(), PATH);
        assert!(
            TOOL.pro_link()
                .ends_with("utm_campaign=tool-battery-state-of-charge")
        );
    }
}
