//! `/tools/technician-pay`: the gross pay of a technician who is paid a
//! flat rate on flagged hours, an hourly rate on the rest, and an overtime
//! rate on overtime hours, and the Ontario payroll deductions on it.
//!
//! The gross is integer arithmetic in `shop-math`. The deductions are the
//! payroll calculator's, `payroll-calc`. Ontario only.

pub mod doc;
pub mod form;
pub mod result;

use askama::Template;
use axum::http::StatusCode;
use axum::response::Response;

use crate::site::tools::Tool;
use crate::site::tools::field::Sent;
use crate::site::{self, Page};

/// The page's address, its heading and its one sentence.
pub const PATH: &str = "/tools/technician-pay";
pub const TITLE: &str = "Technician pay calculator for repair shops";
pub const SUMMARY: &str = "Works out the gross pay of a technician paid a flat rate on flagged hours, an hourly rate on the rest and an overtime rate on overtime hours, and the Ontario income tax, CPP, CPP2 and EI on it.";

/// The calculator's entry in the list of tools.
pub static TOOL: Tool = Tool {
    slug: "technician-pay",
    title: TITLE,
    description: "A free technician pay calculator for repair shops: the gross pay on flagged, hourly and overtime hours, and the Ontario income tax, CPP, CPP2 and EI on it.",
    summary: SUMMARY,
    pro_does: "is shop management software from the people who make this site.",
    doc: doc::doc,
    page,
};

#[derive(Template)]
#[template(path = "technician_pay.html")]
struct View {
    page: Page,
    tool: &'static Tool,
    /// Whether the form is filled with the page's own example.
    example: bool,
    form: form::Form,
    /// What to correct, when there is no result.
    troubles: Vec<(String, String)>,
    /// The gross pay, worked out in whole cents.
    gross: Option<crate::site::markdown::Table>,
    /// The deductions and the net, on the gross, when the pay frequency is
    /// known.
    deductions: Option<crate::site::markdown::Table>,
    how: Vec<String>,
}

/// The page: the example when the form was not sent, and otherwise what
/// was sent, with its result or the messages. It is `200` either way: a
/// form being filled in is not a failure.
fn page(page: Page, tool: &'static Tool, sent: &Sent) -> Response {
    let today = payroll_calc::today();
    let example = !form::Form::was_sent(sent);
    let (form, worked) = if example {
        form::Form::read(&form::example(), today)
    } else {
        form::Form::read(sent, today)
    };
    let gross = worked.as_ref().map(result::gross);
    let deductions = worked.as_ref().and_then(result::deductions);
    site::html(
        StatusCode::OK,
        &View {
            page,
            tool,
            example,
            troubles: form.troubles(),
            gross,
            deductions,
            form,
            how: result::formulas(),
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
                .ends_with("utm_campaign=tool-technician-pay")
        );
        // The line about Wenmar Pro claims no feature.
        assert_eq!(
            TOOL.pro_does,
            "is shop management software from the people who make this site."
        );
    }
}
