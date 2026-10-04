//! `/tools`: free calculators for the owner of a repair shop.
//!
//! Every formula, every target figure and every source is in the crate
//! `shop-math`. The code here reads a form, calls that crate and fills a
//! page.

pub mod field;
pub mod parts_matrix;
pub mod pieces;
pub mod target;

use axum::response::Response;

use crate::site::Page;
use crate::site::markdown::Doc;
use crate::site::tools::field::Sent;

/// One calculator: everything the site needs to list it, title it and link
/// to it.
pub struct Tool {
    /// The last part of the page's address: `/tools/<slug>`.
    pub slug: &'static str,
    /// The page's heading, its title in a search result and its name in a
    /// list. It is what people search for.
    pub title: &'static str,
    /// The description a search result shows.
    pub description: &'static str,
    /// One sentence: what it works out and for whom.
    pub summary: &'static str,
    /// What Wenmar Pro does with this, as the rest of a sentence that
    /// starts with the linked words "Wenmar Pro".
    pub pro_does: &'static str,
    /// The Markdown version: what it does, each input, the formula and one
    /// worked example.
    pub doc: fn(&str) -> Doc,
    /// The page itself: reads what the form sent and fills the page. It is
    /// given the layout's part, already titled, and this entry.
    pub page: fn(Page, &'static Tool, &Sent) -> Response,
}

impl Tool {
    pub fn path(&self) -> String {
        format!("/tools/{}", self.slug)
    }

    /// The one link to Wenmar Pro on the calculator's page, with the
    /// calculator's own marker.
    pub fn pro_link(&self) -> String {
        crate::site::pro_link(&format!("tool-{}", self.slug))
    }

    /// The Markdown version, with links on the public address `base`.
    pub fn doc(&self, base: &str) -> Doc {
        (self.doc)(base)
    }
}
