//! `/tools/canada-invoice-tax`: the sales taxes and the new-tire fees on a
//! repair invoice in a Canadian province or territory.
//!
//! No rate, rule or fee is written here: each is in `data/rates/canada.toml`
//! and reaches this code through [`rates`].

pub mod doc;
pub mod form;
pub mod rates;
pub mod result;

use askama::Template;
use axum::http::StatusCode;
use axum::response::Response;

use crate::site::markdown::Table;
use crate::site::tools::Tool;
use crate::site::tools::field::Sent;
use crate::site::{self, Page};

/// The page's address, its heading and its one sentence.
pub const PATH: &str = "/tools/canada-invoice-tax";
pub const TITLE: &str = "Canadian invoice tax and tire fee calculator";
pub const SUMMARY: &str = "Works out the GST, HST, PST or QST and the new-tire fees on a repair invoice in a Canadian province or territory, and the invoice's total.";

/// The calculator's entry in the list of tools.
pub static TOOL: Tool = Tool {
    slug: "canada-invoice-tax",
    title: TITLE,
    description: "A free calculator for Canadian auto repair invoices: GST, HST, PST or QST and the new-tire fee by province, with the source and date of every rate.",
    summary: SUMMARY,
    pro_does: "applies GST, HST, PST and QST by province automatically.",
    doc: doc::doc,
    page,
};

/// What the page says of the province chosen: its rates in words, when
/// they were checked, and where each was read.
struct ProvinceView {
    name: &'static str,
    checked: String,
    rates: Vec<String>,
    notes: &'static [&'static str],
    no_tire_fee: Option<&'static str>,
    /// Each source as the text of its link and its address.
    sources: Vec<(String, &'static str)>,
}

impl ProvinceView {
    fn of(province: &'static rates::Province) -> ProvinceView {
        ProvinceView {
            name: province.name,
            checked: result::checked(province),
            rates: result::rate_lines(province),
            notes: province.notes,
            no_tire_fee: province.no_tire_fee.map(|none| none.statement),
            sources: province
                .sources()
                .into_iter()
                .map(|source| (doc::source_text(source), source.url))
                .collect(),
        }
    }
}

#[derive(Template)]
#[template(path = "canada_invoice_tax.html")]
struct View {
    page: Page,
    tool: &'static Tool,
    currency: &'static str,
    /// Whether the form is filled with the page's own example.
    example: bool,
    form: form::Form,
    covered: Vec<form::Choice>,
    not_covered: Vec<form::Choice>,
    /// What to correct, when there is no result.
    troubles: Vec<(String, String)>,
    table: Option<Table>,
    /// The province chosen, whether or not there is a result.
    province: Option<ProvinceView>,
    /// The provinces not yet covered, each with what is missing.
    waiting: Vec<(&'static str, String)>,
    scope: Vec<String>,
    how: Vec<String>,
}

/// The page: the example when the form was not sent, and otherwise what
/// was sent, with its result or with a message beside what could not be
/// read. It is `200` either way: a form being filled in is not a failure.
fn page(page: Page, tool: &'static Tool, sent: &Sent) -> Response {
    let example = !form::Form::was_sent(sent);
    let (form, worked) = if example {
        form::Form::read(&form::example())
    } else {
        form::Form::read(sent)
    };
    // The footer says whose rates the page shows and when they were checked.
    let page = match form.chosen {
        Some(province) => page.with_rates(result::checked(province)),
        None => page,
    };
    let waiting = rates::PROVINCES
        .iter()
        .filter(|province| !province.confirmed)
        .map(|province| (province.name, province.missing.join(" ")))
        .collect();
    site::html(
        StatusCode::OK,
        &View {
            page,
            tool,
            currency: rates::CURRENCY,
            example,
            covered: form.covered(),
            not_covered: form.not_covered(),
            troubles: form.troubles(),
            table: worked.as_ref().map(result::table),
            province: form.chosen.map(ProvinceView::of),
            waiting,
            scope: result::scope(),
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
                .ends_with("utm_campaign=tool-canada-invoice-tax")
        );
        assert_eq!(
            TOOL.pro_does,
            "applies GST, HST, PST and QST by province automatically."
        );
    }

    /// A rate, a rule or a fee is written in one place, the rates file. No
    /// file of the page writes a percentage or an amount of its own.
    #[test]
    fn no_page_code_writes_a_rate_or_a_fee_of_its_own() {
        let files = [
            (
                "canada_invoice_tax.html",
                include_str!("../../../../templates/canada_invoice_tax.html"),
            ),
            ("mod.rs", include_str!("mod.rs")),
            ("form.rs", include_str!("form.rs")),
            ("result.rs", include_str!("result.rs")),
            ("doc.rs", include_str!("doc.rs")),
            ("rates.rs", include_str!("rates.rs")),
        ];
        // The one sentence that shows how money may be typed.
        let typed = "1234.5, 1,234.50 or $1,234.50";
        for (name, text) in files {
            let code = text.split("#[cfg(test)]").next().unwrap();
            let code = code.replace(typed, "");
            let bytes = code.as_bytes();
            for (index, pair) in bytes.windows(2).enumerate() {
                let rate = pair[0].is_ascii_digit() && pair[1] == b'%';
                let amount = pair[0] == b'$' && pair[1].is_ascii_digit();
                let from = index.saturating_sub(30);
                assert!(
                    !rate && !amount,
                    "{name} writes a figure: {}",
                    String::from_utf8_lossy(&bytes[from..index + 2])
                );
            }
            // Nor a rate or a fee as a bare number.
            for word in ["9.975", "6.50", "14.00"] {
                assert!(!code.contains(word), "{name} writes {word}");
            }
        }
    }
}
