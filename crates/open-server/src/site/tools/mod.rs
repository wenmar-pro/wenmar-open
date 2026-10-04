//! `/tools`: free calculators for the owner of a repair shop, and a list
//! of them.
//!
//! Every formula, every target figure and every source is in the crate
//! `shop-math`. The code here reads a form, calls that crate and fills a
//! page. [`TOOLS`] is the one list of calculators: the index, the home
//! page, the sitemap and `llms.txt` all read it.

pub mod field;
pub mod labor_rate;
pub mod parts_matrix;
pub mod pieces;
pub mod target;

use axum::extract::rejection::{PathRejection, QueryRejection};
use axum::extract::{Path, Query, State};
use axum::http::{HeaderValue, Uri, header};
use axum::response::Response;

use crate::site::markdown::{self, Doc, Format};
use crate::site::pages::{render, section, with_links};
use crate::site::tools::field::Sent;
use crate::site::{self, Page, jsonld, seo};
use crate::state::AppState;

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

/// Every calculator that exists, in the order they are listed. A new one
/// is added here and nowhere else.
pub static TOOLS: [&Tool; 1] = [&parts_matrix::TOOL];

/// How long a browser may keep a page worked out from a query string. It
/// holds a shop's own figures, so no shared cache keeps it.
const CACHE_RESULT: &str = "private, max-age=3600";

const INDEX_TITLE: &str = "Free calculators for auto repair shops";
const INDEX_DESCRIPTION: &str = "Free calculators for the owner of an auto repair shop, starting with a parts markup matrix. No account, no email, and nothing typed is stored.";

/// The list of the calculators.
pub fn index(base: &str) -> Doc {
    let mut sections: Vec<_> = TOOLS
        .iter()
        .map(|tool| {
            with_links(
                section(tool.title, &[tool.summary]),
                &[(tool.title, format!("{base}{}", tool.path()))],
            )
        })
        .collect();
    let guides: Vec<(&str, String)> = crate::site::guides::GUIDES
        .iter()
        .filter(|guide| !guide.about_vins())
        .map(|guide| (guide.title, format!("{base}{}", guide.path())))
        .collect();
    sections.push(with_links(
        section("Guides", &["Longer answers that go with the calculators."]),
        &guides,
    ));
    sections.push(section(
        "How they work",
        &[
            "Each calculator is a form. What is typed goes into the page's address and nowhere else, so a result can be bookmarked, shared and printed, and nothing is stored.",
            "Every calculator has a Markdown version at the same address with .md added. It states the formula and one worked example, and does not compute.",
        ],
    ));
    Doc {
        title: "Shop calculators".to_owned(),
        intro: "Free calculators for the owner of a repair shop. There is no account, no email to give and no pop-up.".to_owned(),
        sections,
    }
}

fn show_index(state: &AppState, format: Format) -> Response {
    let base = &state.config().base_url;
    let doc = index(base);
    if format == Format::Markdown {
        return markdown::response(markdown::doc(&doc), &format!("{base}/tools"));
    }
    let page = Page::new(state, seo::title(INDEX_TITLE), INDEX_DESCRIPTION)
        .indexed(state, "/tools")
        .with_markdown("/tools.md")
        .in_section("tools")
        .without_vehicle_data();
    render(page, doc)
}

pub async fn index_html(State(state): State<AppState>) -> Response {
    show_index(&state, Format::Html)
}

pub async fn index_md(State(state): State<AppState>) -> Response {
    show_index(&state, Format::Markdown)
}

/// What the layout needs for a calculator's page. `asked` says the address
/// had a query string: the page then names the bare address as the one to
/// index, and is not indexed itself.
fn page_of(state: &AppState, tool: &'static Tool, asked: bool) -> Page {
    let base = &state.config().base_url;
    let path = tool.path();
    let page = Page::new(state, seo::title(tool.title), tool.description)
        .indexed(state, &path)
        .with_markdown(&format!("{path}.md"))
        .in_section("tools")
        .under(vec![("Tools".to_owned(), "/tools".to_owned())])
        .without_vehicle_data();
    let things = vec![
        page.trail(tool.title),
        jsonld::article(base, &path, tool.title, tool.description),
        jsonld::organization(base),
    ];
    let page = page.describing(things);
    if asked { page.with_query() } else { page }
}

/// `/tools/{page}`: one calculator, or its Markdown version.
pub async fn tool(
    State(state): State<AppState>,
    path: Result<Path<String>, PathRejection>,
    query: Result<Query<Vec<(String, String)>>, QueryRejection>,
    uri: Uri,
) -> Response {
    let Ok(Path(segment)) = path else {
        return site::not_found(&state);
    };
    let (slug, format) = markdown::split(&segment);
    let Some(tool) = TOOLS.iter().copied().find(|tool| tool.slug == slug) else {
        return site::not_found(&state);
    };
    if format == Format::Markdown {
        let base = &state.config().base_url;
        let canonical = format!("{base}{}", tool.path());
        return markdown::response(markdown::doc(&tool.doc(base)), &canonical);
    }
    // A query string that cannot be read is no query at all: the page
    // shows its example.
    let sent = query
        .map(|Query(pairs)| Sent::new(pairs))
        .unwrap_or_default();
    let asked = uri.query().is_some();
    let mut response = (tool.page)(page_of(&state, tool, asked), tool, &sent);
    if asked {
        let headers = response.headers_mut();
        headers.insert(
            header::CACHE_CONTROL,
            HeaderValue::from_static(CACHE_RESULT),
        );
        headers.insert("x-robots-tag", HeaderValue::from_static("noindex"));
    }
    response
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_tool_has_its_own_address_title_description_and_summary() {
        for (index, tool) in TOOLS.iter().enumerate() {
            assert!(wenmar_vehicles::text::is_slug(tool.slug), "{}", tool.slug);
            // Short enough for the site's name to follow it in a title.
            assert!(tool.title.len() <= 46, "{}", tool.title);
            assert!(
                (100..=160).contains(&tool.description.len()),
                "{}: {}",
                tool.slug,
                tool.description.len()
            );
            // One sentence each.
            assert!(tool.summary.ends_with('.') && tool.pro_does.ends_with('.'));
            assert_eq!(tool.summary.matches(". ").count(), 0, "{}", tool.summary);
            assert_eq!(tool.doc("https://open.example").title, tool.title);
            for other in &TOOLS[index + 1..] {
                assert_ne!(tool.slug, other.slug);
                assert_ne!(tool.title, other.title);
                assert_ne!(tool.description, other.description);
            }
        }
    }

    #[test]
    fn the_index_names_each_tool_with_its_sentence_and_its_address() {
        let text = markdown::doc(&index("https://open.example"));
        assert!(text.starts_with("# Shop calculators\n"), "{text}");
        for tool in TOOLS {
            assert!(
                text.contains(&format!("\n## {}\n\n{}\n", tool.title, tool.summary)),
                "{text}"
            );
            assert!(text.contains(&format!(
                "- [{}](https://open.example{})\n",
                tool.title,
                tool.path()
            )));
        }
    }
}
