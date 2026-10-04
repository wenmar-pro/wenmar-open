//! `/tools/parts-matrix`: what a part sells for under a matrix of tiers,
//! and the margin the matrix makes on a shop's mix of parts.

pub mod doc;
pub mod form;
pub mod presets;
pub mod result;

use askama::Template;
use axum::http::StatusCode;
use axum::response::Response;

use crate::site::tools::Tool;
use crate::site::tools::field::Sent;
use crate::site::{self, Page};

/// The page's address, its heading and its one sentence.
pub const PATH: &str = "/tools/parts-matrix";
pub const TITLE: &str = "Parts markup matrix calculator";
pub const SUMMARY: &str = "Works out what a part sells for under a parts matrix, and the margin the matrix makes across a shop's mix of parts.";

/// The calculator's entry in the list of tools.
pub static TOOL: Tool = Tool {
    slug: "parts-matrix",
    title: TITLE,
    description: "A free parts markup matrix calculator for auto repair shops: price a part by tiers of cost, compare markup with margin, and see the blended margin.",
    summary: SUMMARY,
    pro_does: "applies a parts matrix to every estimate.",
    doc: doc::doc,
    page,
};

#[derive(Template)]
#[template(path = "parts_matrix.html")]
struct View {
    page: Page,
    tool: &'static Tool,
    /// Whether the form is filled with the page's own example.
    example: bool,
    /// The example matrices, as the words of a link and its address.
    presets: Vec<(&'static str, String)>,
    form: form::Form,
    /// What to correct, when there is no result.
    troubles: Vec<(String, String)>,
    shown: Option<result::Shown>,
    usual_markup: String,
    how: Vec<String>,
    illustrations: &'static str,
}

/// The page: the example when the form was not sent, and otherwise what
/// was sent, with its result or with a message beside what could not be
/// read. It is `200` either way: a form being filled in is not a failure.
fn page(page: Page, tool: &'static Tool, sent: &Sent) -> Response {
    let example = !form::Form::was_sent(sent);
    let (form, matrix) = if example {
        form::Form::read(&presets::example())
    } else {
        form::Form::read(sent)
    };
    site::html(
        StatusCode::OK,
        &View {
            page,
            tool,
            example,
            presets: presets::PRESETS
                .iter()
                .map(|preset| (preset.label, preset.href(PATH)))
                .collect(),
            troubles: form.troubles(),
            form,
            shown: matrix.as_ref().map(result::shown),
            usual_markup: result::usual_markup(),
            how: result::formulas(),
            illustrations: doc::ILLUSTRATIONS,
        },
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_entry_and_the_page_agree_on_the_address() {
        assert_eq!(TOOL.path(), PATH);
        assert!(TOOL.pro_link().ends_with("utm_campaign=tool-parts-matrix"));
    }
}
