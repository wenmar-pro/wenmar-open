//! `/tools/refrigerant-pt`: the pressure of each refrigerant at each
//! temperature, in Celsius and Fahrenheit and in kPa and psi.
//!
//! No figure is written here: each table is in `data/reference/refrigerant-pt.toml`
//! and reaches this code through [`data`]. A table is shown only when its
//! entry is confirmed; until then the page says "Not yet checked".

pub mod data;
pub mod doc;
pub mod result;

use askama::Template;
use axum::http::StatusCode;
use axum::response::Response;

use crate::site::markdown::Table;
use crate::site::tools::Tool;
use crate::site::tools::canada_invoice_tax::doc::source_text;
use crate::site::tools::field::Sent;
use crate::site::{self, Page};
use data::{REFRIGERANTS, Refrigerant};

/// The page's address, its heading and its one sentence.
pub const PATH: &str = "/tools/refrigerant-pt";
pub const TITLE: &str = "R-134a and R-1234yf pressure-temperature chart";
pub const SUMMARY: &str = "Shows the pressure of the refrigerants R-134a and R-1234yf at each temperature, in Celsius and Fahrenheit and in kPa and psi, as read from a published source.";

/// The calculator's entry in the list of tools.
pub static TOOL: Tool = Tool {
    slug: "refrigerant-pt",
    title: TITLE,
    description: "A pressure-temperature chart for the refrigerants R-134a and R-1234yf, in Celsius, Fahrenheit, kPa and psi, with the source and date of every figure.",
    summary: SUMMARY,
    pro_does: "is shop management software from the people who make this site.",
    doc: doc::doc,
    page,
};

/// One refrigerant as the page shows it.
pub struct Chart {
    pub key: &'static str,
    pub name: &'static str,
    /// Some only when the entry is confirmed.
    pub table: Option<Table>,
    pub checked: String,
    pub units: String,
    pub not_checked: String,
    pub notes: &'static [&'static str],
    pub missing: &'static [&'static str],
    /// Each source as the text of its link, and its address.
    pub sources: Vec<(String, &'static str)>,
}

impl Chart {
    pub fn of(refrigerant: &'static Refrigerant) -> Chart {
        Chart {
            key: refrigerant.key,
            name: refrigerant.name,
            table: refrigerant.confirmed.then(|| result::table(refrigerant)),
            checked: result::checked(refrigerant),
            units: result::units(refrigerant),
            not_checked: result::not_checked(refrigerant),
            notes: refrigerant.notes,
            missing: refrigerant.missing,
            sources: refrigerant
                .sources
                .iter()
                .map(|source| (source_text(source), source.url))
                .collect(),
        }
    }
}

#[derive(Template)]
#[template(path = "refrigerant_pt.html")]
struct View {
    page: Page,
    tool: &'static Tool,
    charts: Vec<Chart>,
    limits: &'static str,
    how: Vec<String>,
}

/// The page: one section for each refrigerant. The form is not read: a
/// chart has none.
fn page(page: Page, tool: &'static Tool, _sent: &Sent) -> Response {
    let charts = REFRIGERANTS.iter().map(Chart::of).collect();
    site::html(
        StatusCode::OK,
        &View {
            page,
            tool,
            charts,
            limits: result::LIMITS,
            how: result::conversions(),
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
                .ends_with("utm_campaign=tool-refrigerant-pt")
        );
    }
}
