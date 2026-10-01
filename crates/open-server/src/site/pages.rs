//! The three pages of prose: `/docs`, `/data` and `/about`.
//!
//! Each is written once, as a [`Doc`], and shown as HTML and as Markdown.

use askama::Template;
use axum::extract::State;
use axum::http::StatusCode;
use axum::response::Response;

use crate::site::markdown::{self, Doc, Format, Section};
use crate::site::{self, Page};
use crate::state::AppState;

#[derive(Template)]
#[template(path = "doc.html")]
struct DocPage {
    page: Page,
    doc: Doc,
}

fn section(heading: &str, paragraphs: &[&str]) -> Section {
    Section {
        heading: heading.to_owned(),
        paragraphs: paragraphs.iter().map(|text| (*text).to_owned()).collect(),
        code: None,
        links: Vec::new(),
    }
}

fn with_code(mut section: Section, code: String) -> Section {
    section.code = Some(code);
    section
}

fn with_links(mut section: Section, links: &[(&str, String)]) -> Section {
    section.links = links
        .iter()
        .map(|(label, address)| ((*label).to_owned(), address.clone()))
        .collect();
    section
}

/// The API reference.
pub fn docs(state: &AppState) -> Doc {
    let base = &state.config().base_url;
    let limit = state.config().requests_per_minute;
    Doc {
        title: "API".to_owned(),
        intro: "A JSON API for VIN decoding and for looking up vehicles by year, make, model, trim and engine. It needs no key and no account, and any website may call it from a browser.".to_owned(),
        sections: vec![
            with_code(
                section(
                    "Decode a VIN",
                    &["Spaces and dashes are ignored. A wrong check digit is not an error: the answer has valid set to false and a warning, because many genuine VINs from outside North America fail the check. Add ?year= to use a model year of your own."],
                ),
                format!("curl {base}/v1/vin/KM8K2CAB4PU001140"),
            ),
            with_code(
                section(
                    "Decode up to 50 VINs",
                    &["The answer is a list in the order asked. Each item is a decode, or the error a single request for that VIN would have given."],
                ),
                format!(
                    "curl -X POST {base}/v1/vin/batch \\\n  -H 'Content-Type: application/json' \\\n  -d '{{\"vins\": [\"KM8K2CAB4PU001140\", \"1HGCM82633A004352\"]}}'"
                ),
            ),
            with_code(
                section(
                    "Pick a vehicle step by step",
                    &["Each step offers only what is valid for the steps before it. A make or model may be given by name, by an alias such as chevy, or by its id. Add term= to keep only names that start with it."],
                ),
                format!(
                    "curl {base}/v1/vehicles/years\ncurl '{base}/v1/vehicles/makes?year=2019'\ncurl '{base}/v1/vehicles/models?make=honda&year=2019'\ncurl '{base}/v1/vehicles/submodels?make=honda&model=civic&year=2019'\ncurl '{base}/v1/vehicles/engines?make=honda&model=civic&year=2019&submodel=si'"
                ),
            ),
            with_code(
                section(
                    "Search and vehicle ids",
                    &["Search reads what people type. Every vehicle has an id built from its year and names, which stays the same from one data release to the next for as long as the names do."],
                ),
                format!(
                    "curl '{base}/v1/vehicles/search?q=2019+civic+si'\ncurl {base}/v1/vehicles/2019_honda_civic_si"
                ),
            ),
            section(
                "Errors",
                &[
                    "An error is a JSON object with one member, error, which has a code, a message and details. The HTTP status is the main signal.",
                    "400 invalid_vin: wrong length or illegal characters; details.suggestions lists likely corrections. 400 validation_failed: a parameter or body is missing or wrong. 404 not_found: no manufacturer is registered for the VIN's first three characters, or no vehicle has that id. 429 rate_limited: too many requests. 500 internal_error: our fault.",
                ],
            ),
            section(
                "Limits",
                &[
                    &format!("One address may make {limit} requests a minute. That is far more than a shop needs, and it exists only so that one client cannot slow the service for everyone. Over it, the answer is 429 with a Retry-After header saying how many seconds to wait."),
                    "A batch holds at most 50 VINs. A request body may be at most 16 KB.",
                ],
            ),
            section(
                "Caching and versions",
                &[
                    "Every answer carries the data version in an X-Data-Version header. Answers do not change until the data does, so successful GETs may be cached for a day and carry an ETag; send it back in If-None-Match to get a 304.",
                    "Fields and endpoints are only ever added. Nothing that has shipped is removed or given a new meaning.",
                ],
            ),
            with_links(
                section(
                    "For programs and AI agents",
                    &["The OpenAPI description is generated from the server's own code. The MCP endpoint speaks Streamable HTTP, needs no key, and has two tools: wenmar_vin and wenmar_vehicles."],
                ),
                &[
                    ("OpenAPI description", format!("{base}/v1/openapi.json")),
                    ("MCP endpoint", format!("{base}/mcp")),
                    ("llms.txt", format!("{base}/llms.txt")),
                ],
            ),
        ],
    }
}

/// About the data.
pub fn data(state: &AppState) -> Doc {
    let meta = state.db().meta();
    Doc {
        title: "Data".to_owned(),
        intro: format!(
            "This site is serving data version {}, built on {} from NHTSA's release {}.",
            meta.data_version, meta.built_at, meta.vpic_release
        ),
        sections: vec![
            with_links(
                section(
                    "Where it comes from",
                    &[
                        "All of it comes from vPIC, the Product Information Catalog and Vehicle Listing published each month by the US National Highway Traffic Safety Administration. vPIC is public-domain US government data.",
                        "vPIC describes vehicles as their manufacturers reported them to NHTSA. It can be incomplete or wrong, especially for vehicles never sold in the United States. Trims are thin for some popular models, and where one engine code is shared by several models the engine list can be longer than it should be.",
                    ],
                ),
                &[("NHTSA vPIC", "https://vpic.nhtsa.dot.gov/".to_owned())],
            ),
            with_links(
                section(
                    "Download",
                    &[
                        "The whole data set is one SQLite file, rebuilt each month. Anything that reads SQLite can use it, and the wenmar-vin Rust crate decodes VINs from it with no network.",
                    ],
                ),
                &[
                    (
                        "Data releases",
                        "https://github.com/wenmar-pro/wenmar-open/releases".to_owned(),
                    ),
                    (
                        "Changelog",
                        "https://github.com/wenmar-pro/wenmar-open/blob/main/CHANGELOG.md"
                            .to_owned(),
                    ),
                ],
            ),
            with_links(
                section(
                    "Found a wrong decode?",
                    &[
                        "Reports of wrong decodes are the most useful thing anyone can send. Each fix adds the VIN to the set every build is checked against.",
                    ],
                ),
                &[(
                    "Report a wrong decode",
                    "https://github.com/wenmar-pro/wenmar-open/issues/new/choose".to_owned(),
                )],
            ),
        ],
    }
}

/// What this is and who runs it.
pub fn about(_state: &AppState) -> Doc {
    Doc {
        title: "About".to_owned(),
        intro: "Wenmar Open is a free VIN decoder and vehicle catalog for auto repair shops. There is no account, no key and nothing to install.".to_owned(),
        sections: vec![
            with_links(
                section(
                    "Who runs it",
                    // Say that Wenmar Pro decodes VINs through this service
                    // only once it does.
                    &["Wenmar Open is built and hosted by Wenmar Pro, shop management software for independent auto repair shops."],
                ),
                &[("Wenmar Pro", site::pro_link("about"))],
            ),
            section(
                "What it keeps",
                // Two logs are kept, and this says what is in each. The
                // second sentence about logs is to be reworded only after
                // the proxy's log has been looked at: docs/deploy.md,
                // "Check it".
                &["There are no cookies, no accounts and no analytics scripts. The service's own log records the first 11 characters of a decoded VIN, which name the maker, model and plant, and not the serial number. The hosting proxy in front of the service keeps a request log of its own, with the address of each request and where it came from. The address of a decode, as a page or through the API, holds the whole VIN."],
            ),
            with_links(
                section(
                    "Open source",
                    &["The code is MIT licensed and the data is public domain. Corrections are welcome."],
                ),
                &[(
                    "Source code",
                    "https://github.com/wenmar-pro/wenmar-open".to_owned(),
                )],
            ),
        ],
    }
}

fn show(state: &AppState, path: &str, description: &str, doc: Doc, format: Format) -> Response {
    if format == Format::Markdown {
        let canonical = format!("{}{path}", state.config().base_url);
        return markdown::response(markdown::doc(&doc), &canonical);
    }
    let page = Page::new(state, format!("{} - Wenmar Open", doc.title), description)
        .indexed(state, path)
        .with_markdown(&format!("{path}.md"));
    site::html(StatusCode::OK, &DocPage { page, doc })
}

const DOCS: &str =
    "How to call the Wenmar Open API: VIN decoding and the vehicle catalog, with examples to copy.";
const DATA: &str =
    "The data version this site serves, where the data comes from, and how to download it.";
const ABOUT: &str = "What Wenmar Open is, who runs it, and what it keeps.";

pub async fn docs_html(State(state): State<AppState>) -> Response {
    show(&state, "/docs", DOCS, docs(&state), Format::Html)
}

pub async fn docs_md(State(state): State<AppState>) -> Response {
    show(&state, "/docs", DOCS, docs(&state), Format::Markdown)
}

pub async fn data_html(State(state): State<AppState>) -> Response {
    show(&state, "/data", DATA, data(&state), Format::Html)
}

pub async fn data_md(State(state): State<AppState>) -> Response {
    show(&state, "/data", DATA, data(&state), Format::Markdown)
}

pub async fn about_html(State(state): State<AppState>) -> Response {
    show(&state, "/about", ABOUT, about(&state), Format::Html)
}

pub async fn about_md(State(state): State<AppState>) -> Response {
    show(&state, "/about", ABOUT, about(&state), Format::Markdown)
}
