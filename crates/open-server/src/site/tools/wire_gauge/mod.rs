//! `/tools/wire-gauge`: the voltage drop along each gauge of wire for a
//! current and a length at 12 V or 24 V, and the thinnest that stays within
//! the drop allowed.
//!
//! No resistance is written here: each gauge's is in
//! `data/reference/wire-resistance.toml` and reaches this code through
//! [`data`]. A table is worked out only when it is confirmed; until then the
//! page says "Not yet checked" and works nothing out.

pub mod data;
pub mod doc;
pub mod form;
pub mod result;

use askama::Template;
use axum::http::StatusCode;
use axum::response::Response;

use crate::site::tools::Tool;
use crate::site::tools::canada_invoice_tax::doc::source_text;
use crate::site::tools::field::Sent;
use crate::site::{self, Page};
use data::WIRE;
use form::Form;

/// The page's address, its heading and its one sentence.
pub const PATH: &str = "/tools/wire-gauge";
pub const TITLE: &str = "Wire gauge calculator for 12 V and 24 V";
pub const SUMMARY: &str = "Works out the voltage drop of each wire gauge for a current and a length at 12 V or 24 V, and names the thinnest wire that stays within the drop allowed.";

/// The calculator's entry in the list of tools.
pub static TOOL: Tool = Tool {
    slug: "wire-gauge",
    title: TITLE,
    description: "A free wire gauge calculator for 12 V and 24 V circuits: the voltage drop of each gauge for a current and a length, and the thinnest wire that stays within it.",
    summary: SUMMARY,
    pro_does: "is shop management software from the people who make this site.",
    doc: doc::doc,
    page,
};

#[derive(Template)]
#[template(path = "wire_gauge.html")]
struct View {
    page: Page,
    tool: &'static Tool,
    /// Whether the form is filled with the page's own example.
    example: bool,
    /// Whether the table is confirmed. Until it is, there is no form.
    confirmed: bool,
    form: Form,
    defaults: String,
    /// What to correct, when there is no result.
    troubles: Vec<(String, String)>,
    shown: Option<result::Shown>,
    checked: String,
    conditions: &'static [&'static str],
    notes: &'static [&'static str],
    missing: &'static [&'static str],
    /// Each source as the text of its link and its address.
    sources: Vec<(String, &'static str)>,
    not_checked: &'static str,
    limits: &'static str,
    how: Vec<String>,
}

/// The page: the example when the form was not sent, and otherwise what
/// was sent, with its result or with a message beside what could not be
/// read. It is `200` either way: a form being filled in is not a failure.
fn page(page: Page, tool: &'static Tool, sent: &Sent) -> Response {
    let example = !Form::was_sent(sent);
    let sent = if example {
        form::example()
    } else {
        sent.clone()
    };
    let (form, circuit) = Form::read(&sent);
    let confirmed = WIRE.confirmed;
    let sources = WIRE
        .sources
        .iter()
        .map(|source| (source_text(source), source.url))
        .collect();
    site::html(
        StatusCode::OK,
        &View {
            page,
            tool,
            example,
            confirmed,
            defaults: form::defaults(),
            troubles: form.troubles(),
            shown: circuit
                .as_ref()
                .filter(|_| confirmed)
                .map(|circuit| result::shown(circuit, &WIRE)),
            checked: result::checked(&WIRE),
            conditions: WIRE.conditions,
            notes: WIRE.notes,
            missing: WIRE.missing,
            sources,
            not_checked: result::NOT_CHECKED,
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
        assert!(TOOL.pro_link().ends_with("utm_campaign=tool-wire-gauge"));
    }
}
