//! `/tools/tire-size`: two metric tire sizes compared for diameter, sidewall,
//! circumference and speedometer error.

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
pub const PATH: &str = "/tools/tire-size";
pub const TITLE: &str = "Tire size comparison calculator";
pub const SUMMARY: &str = "Compares two metric tire sizes: the sidewall height, overall diameter, circumference and revolutions per kilometre of each, the difference in diameter, and the true speed when the speedometer reads 100 km/h.";

/// The calculator's entry in the list of tools.
pub static TOOL: Tool = Tool {
    slug: "tire-size",
    title: TITLE,
    description: "A free tire size calculator: compare two sizes such as 275/65R18 and 285/70R17 for diameter, sidewall, circumference and speedometer error.",
    summary: SUMMARY,
    pro_does: "is shop management software from the people who make this site.",
    doc: doc::doc,
    page,
};

#[derive(Template)]
#[template(path = "tire_size.html")]
struct View {
    page: Page,
    tool: &'static Tool,
    /// Whether the form is filled with the page's own example.
    example: bool,
    form: form::Form,
    /// What to correct, when there is no result.
    troubles: Vec<(String, String)>,
    shown: Option<result::Shown>,
    limits: &'static str,
    how: Vec<String>,
}

/// The page: the example when the form was not sent, and otherwise what
/// was sent, with its comparison or with a message beside each size that
/// could not be read. It is `200` either way: a form being filled in is not
/// a failure.
fn page(page: Page, tool: &'static Tool, sent: &Sent) -> Response {
    let example = !form::Form::was_sent(sent);
    let (form, sizes) = if example {
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
            shown: sizes
                .as_ref()
                .map(|(first, second)| result::shown(first, second)),
            limits: result::LIMITS,
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
        assert!(TOOL.pro_link().ends_with("utm_campaign=tool-tire-size"));
        // The line about Wenmar Pro claims no feature.
        assert_eq!(
            TOOL.pro_does,
            "is shop management software from the people who make this site."
        );
    }
}
