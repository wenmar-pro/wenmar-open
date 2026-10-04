//! `/tools/labor-rate`: the hourly rate a shop needs, and the rate it is
//! getting.

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
pub const PATH: &str = "/tools/labor-rate";
pub const TITLE: &str = "Labor rate calculator for auto repair shops";
pub const SUMMARY: &str = "Works out the hourly labor rate a repair shop needs to cover its costs and leave a profit, and the rate it is getting for an hour billed.";

/// The calculator's entry in the list of tools.
pub static TOOL: Tool = Tool {
    slug: "labor-rate",
    title: TITLE,
    description: "A free labor rate calculator for auto repair shops: the hourly rate that covers technician cost, overhead and profit, and the effective rate you are getting.",
    summary: SUMMARY,
    pro_does: "is shop management software from the people who make this site.",
    doc: doc::doc,
    page,
};

#[derive(Template)]
#[template(path = "labor_rate.html")]
struct View {
    page: Page,
    tool: &'static Tool,
    /// Whether the form is filled with the page's own example.
    example: bool,
    form: form::Form,
    /// What a field left empty takes, and what the page does not say.
    defaults: String,
    /// The rate the shop needs, or what to correct in its part of the form.
    needed: Option<result::NeededShown>,
    needed_troubles: Vec<(String, String)>,
    /// The rate the shop is getting, or what to correct in its part.
    getting: Option<result::GettingShown>,
    getting_troubles: Vec<(String, String)>,
    how: Vec<String>,
}

/// The page: the example when the form was not sent, and otherwise what
/// was sent. Each part of the form has its own result, or a message beside
/// what could not be read. It is `200` either way: a form being filled in
/// is not a failure.
fn page(page: Page, tool: &'static Tool, sent: &Sent) -> Response {
    let example = !form::Form::was_sent(sent);
    let (form, needed, getting) = if example {
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
            defaults: form::defaults(),
            needed: needed.as_ref().map(result::needed),
            needed_troubles: form.needed.troubles(),
            getting: getting.as_ref().map(result::getting),
            getting_troubles: form.getting.troubles(),
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
        assert!(TOOL.pro_link().ends_with("utm_campaign=tool-labor-rate"));
        // The line about Wenmar Pro claims no feature.
        assert_eq!(
            TOOL.pro_does,
            "is shop management software from the people who make this site."
        );
    }
}
