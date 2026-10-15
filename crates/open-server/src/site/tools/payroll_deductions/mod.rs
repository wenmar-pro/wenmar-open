//! `/tools/payroll-deductions`: the federal and Ontario income tax, CPP,
//! CPP2 and EI on one pay, the net pay, and what the employer pays on top.
//!
//! No rate, rule or amount is written here: each is in the `payroll-ca`
//! crate's tables of CRA's T4127 Payroll Deductions Formulas, and reaches
//! this code through `payroll-calc`. Ontario only.

pub mod doc;
pub mod form;
pub mod result;

use askama::Template;
use axum::http::StatusCode;
use axum::response::Response;
use chrono::NaiveDate;

use crate::site::markdown::Table;
use crate::site::tools::Tool;
use crate::site::tools::field::Sent;
use crate::site::{self, Page};

/// The page's address, its heading and its one sentence.
pub const PATH: &str = "/tools/payroll-deductions";
pub const TITLE: &str = "Ontario payroll deductions calculator";
pub const SUMMARY: &str = "Works out the federal and Ontario income tax, CPP, CPP2 and EI on one pay, the net pay and what the employer pays on top, by CRA's T4127 formulas.";

/// The calculator's entry in the list of tools.
pub static TOOL: Tool = Tool {
    slug: "payroll-deductions",
    title: TITLE,
    description: "A free Ontario payroll deductions calculator: income tax, CPP, CPP2 and EI on one pay, with net pay, employer costs and every CRA T4127 factor used.",
    summary: SUMMARY,
    pro_does: "is shop management software from the people who make this site.",
    doc: doc::doc,
    page,
};

#[derive(Template)]
#[template(path = "payroll_deductions.html")]
struct View {
    page: Page,
    tool: &'static Tool,
    /// Whether the form is filled with the page's own example.
    example: bool,
    form: form::Form,
    choices: Vec<form::Choice>,
    /// What to correct, when there is no result.
    troubles: Vec<(String, String)>,
    table: Option<Table>,
    employer: Option<Table>,
    factors: Option<Table>,
    claims: Vec<String>,
    /// The edition of the result, or of the newest edition when there is
    /// none.
    edition_line: String,
    /// T4127 of each edition (title, address), then PDOC.
    sources: Vec<(String, &'static str)>,
    note: (&'static str, &'static str, &'static str),
    pdoc: &'static str,
    claims_help: &'static str,
    year_to_date_help: &'static str,
    /// The first and last day the tables cover, for the date box.
    first_day: String,
    last_day: String,
    check: Vec<String>,
    pdoc_url: &'static str,
    check_url: &'static str,
    how: Vec<String>,
    scope: Vec<String>,
}

/// The page: the example when the form was not sent, and otherwise what
/// was sent, with its result or the messages. It is `200` either way: a
/// form being filled in is not a failure. `today` is the day the default
/// pay date is, so a test can name it.
pub fn show(page: Page, tool: &'static Tool, sent: &Sent, today: NaiveDate) -> Response {
    let example = !form::Form::was_sent(sent);
    let (form, worked) = if example {
        form::Form::read(&form::example(), today)
    } else {
        form::Form::read(sent, today)
    };
    // The footer says whose tables the page shows and for which pay dates.
    let held = payroll_calc::editions();
    let edition = worked
        .as_ref()
        .map(|worked| worked.edition)
        .unwrap_or_else(|| *held.last().expect("the engine holds an edition"));
    let page = page.with_rates(result::edition_line(&edition));
    let (first_day, last_day) = (
        held[0].effective.to_string(),
        held[held.len() - 1].valid_until.to_string(),
    );
    let mut sources: Vec<(String, &'static str)> = held
        .iter()
        .filter_map(|edition| edition.url)
        .map(|url| {
            let edition = held
                .iter()
                .find(|edition| edition.url == Some(url))
                .expect("the url is held");
            (edition.title(), url)
        })
        .collect();
    sources.push((
        "CRA's Payroll Deductions Online Calculator (PDOC)".to_owned(),
        payroll_calc::PDOC_URL,
    ));
    site::html(
        StatusCode::OK,
        &View {
            page,
            tool,
            example,
            choices: form.choices(),
            troubles: form.troubles(),
            table: worked.as_ref().map(result::table),
            employer: worked.as_ref().map(result::employer),
            factors: worked.as_ref().map(result::factors),
            claims: worked.as_ref().map(result::claims).unwrap_or_default(),
            edition_line: result::edition_line(&edition),
            sources,
            note: payroll_calc::note_parts(),
            pdoc: payroll_calc::PDOC_URL,
            claims_help: result::CLAIMS_HELP,
            year_to_date_help: result::YEAR_TO_DATE_HELP,
            first_day,
            last_day,
            check: result::check(),
            pdoc_url: payroll_calc::PDOC_URL,
            check_url: payroll_calc::CHECK_URL,
            how: result::how(),
            scope: result::scope(),
            form,
        },
    )
}

fn page(page: Page, tool: &'static Tool, sent: &Sent) -> Response {
    show(page, tool, sent, payroll_calc::today())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_entry_and_the_page_agree_on_the_address() {
        assert_eq!(TOOL.path(), PATH);
        assert!(
            TOOL.pro_link()
                .ends_with("utm_campaign=tool-payroll-deductions")
        );
        assert_eq!(
            TOOL.pro_does,
            "is shop management software from the people who make this site."
        );
    }

    /// A rate, a maximum or a basic amount is the engine's, written in its
    /// tables. No file of the page writes one of its own: the example's
    /// own figures (2400.00) are not figures the engine holds.
    #[test]
    fn no_page_code_writes_a_payroll_figure_of_its_own() {
        let files = [
            (
                "payroll_deductions.html",
                include_str!("../../../../templates/payroll_deductions.html"),
            ),
            ("mod.rs", include_str!("mod.rs")),
            ("form.rs", include_str!("form.rs")),
            ("result.rs", include_str!("result.rs")),
            ("doc.rs", include_str!("doc.rs")),
        ];
        for (name, text) in files {
            let code = text.split("#[cfg(test)]").next().unwrap();
            let bytes = code.as_bytes();
            for (index, pair) in bytes.windows(2).enumerate() {
                let rate = pair[0].is_ascii_digit() && pair[1] == b'%';
                let from = index.saturating_sub(30);
                assert!(
                    !rate,
                    "{name} writes a percentage: {}",
                    String::from_utf8_lossy(&bytes[from..index + 2])
                );
            }
            for word in [
                "16452", "12989", "16,452", "12,989", "0.0595", "0.0163", "1.4", "3500", "68900",
            ] {
                assert!(!code.contains(word), "{name} writes {word}");
            }
        }
    }
}
