//! `/tools/gross-profit`: what labor, parts and sublet made in a period,
//! each beside its target.

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
pub const PATH: &str = "/tools/gross-profit";
pub const TITLE: &str = "Gross profit calculator for auto repair shops";
pub const SUMMARY: &str = "Works out a repair shop's gross profit on labor, on parts, on sublet work and on all three together, and shows labor, parts and the overall figure beside a target.";

/// The calculator's entry in the list of tools.
pub static TOOL: Tool = Tool {
    slug: "gross-profit",
    title: TITLE,
    description: "A free gross profit calculator for auto repair shops: gross profit on labor, parts and sublet for a period, each beside the target shops aim for.",
    summary: SUMMARY,
    pro_does: "is shop management software from the people who make this site.",
    doc: doc::doc,
    page,
};

#[derive(Template)]
#[template(path = "gross_profit.html")]
struct View {
    page: Page,
    tool: &'static Tool,
    /// Whether the form is filled with the page's own example.
    example: bool,
    form: form::Form,
    /// What to correct, when there is no result.
    troubles: Vec<(String, String)>,
    shown: Option<result::Shown>,
    sublet_alone: &'static str,
    how: Vec<String>,
}

/// The page: the example when the form was not sent, and otherwise what
/// was sent, with its result or with a message beside what could not be
/// read. It is `200` either way: a form being filled in is not a failure.
fn page(page: Page, tool: &'static Tool, sent: &Sent) -> Response {
    let example = !form::Form::was_sent(sent);
    let (form, profit) = if example {
        form::Form::read(&form::example())
    } else {
        form::Form::read(sent)
    };
    site::html(
        StatusCode::OK,
        &View {
            page,
            tool,
            example,
            troubles: form.troubles(),
            form,
            shown: profit.as_ref().map(result::shown),
            sublet_alone: result::SUBLET_ALONE,
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
        assert!(TOOL.pro_link().ends_with("utm_campaign=tool-gross-profit"));
        // The line about Wenmar Pro claims no feature.
        assert_eq!(
            TOOL.pro_does,
            "is shop management software from the people who make this site."
        );
    }
}
