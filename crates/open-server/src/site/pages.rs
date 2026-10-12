//! The three pages of prose: `/docs`, `/data` and `/about`.
//!
//! Each is written once, as a [`Doc`], and shown as HTML and as Markdown.

use askama::Template;
use axum::extract::State;
use axum::http::StatusCode;
use axum::response::Response;

use crate::site::jsonld;
use crate::site::licences;
use crate::site::markdown::{self, Doc, Format, Section, Table};
use crate::site::{self, Page};
use crate::state::AppState;

#[derive(Template)]
#[template(path = "doc.html")]
struct DocPage {
    page: Page,
    doc: Doc,
}

pub(crate) fn section(heading: &str, paragraphs: &[&str]) -> Section {
    Section {
        heading: heading.to_owned(),
        paragraphs: paragraphs.iter().map(|text| (*text).to_owned()).collect(),
        ..Section::default()
    }
}

pub(crate) fn with_code(mut section: Section, code: String) -> Section {
    section.code = Some(code);
    section
}

pub(crate) fn with_links(mut section: Section, links: &[(&str, String)]) -> Section {
    section.links = links
        .iter()
        .map(|(label, address)| ((*label).to_owned(), address.clone()))
        .collect();
    section
}

pub(crate) fn with_table(mut section: Section, table: Table) -> Section {
    section.table = Some(table);
    section
}

/// A page of prose as HTML.
pub fn render(page: Page, doc: Doc) -> Response {
    site::html(StatusCode::OK, &DocPage { page, doc })
}

/// The API reference.
pub fn docs(state: &AppState) -> Doc {
    let base = &state.config().base_url;
    let limit = state.config().requests_per_minute;
    let mut doc = Doc {
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
            with_code(
                section(
                    "Everything for one vehicle",
                    &["One answer with everything this server holds for a model year, asked for by vehicle id or by VIN, with links to its pages. A block the server has no data for is null, so the answer has the same members on every server. The recalls in it are a count of the campaigns that list the year, make and model, with the three newest; the whole list has its own address. The third address lists every model year of one model, newest first."],
                ),
                format!(
                    "curl {base}/v1/vehicles/2019_honda_civic/overview\ncurl {base}/v1/vin/KM8K2CAB4PU001140/overview\ncurl {base}/v1/models/honda/civic"
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
            with_code(
                section(
                    "For AI agents",
                    &[
                        "An assistant that can fetch a web address can decode a VIN with one request. There is no key to ask for and nothing to sign up to.",
                        "The answer is JSON: the year, make, model, trim, engine and safety equipment, and a list of warnings. A wrong check digit is a warning, not an error.",
                    ],
                ),
                format!("curl {base}/v1/vin/KM8K2CAB4PU001140"),
            ),
            with_links(
                with_code(
                    section(
                        "MCP server",
                        &["The same lookups are tools on a Model Context Protocol server: wenmar_vin, wenmar_vehicles, wenmar_recalls and wenmar_bulletins. A recall that wenmar_recalls lists may apply to a vehicle of that year, make and model; only the manufacturer can confirm it for one VIN. A bulletin that wenmar_bulletins lists is for a model year and is not a recall. The server speaks Streamable HTTP and needs no key. Add this address as a remote server or a custom connector:"],
                    ),
                    format!("{base}/mcp"),
                ),
                &[
                    ("OpenAPI description", format!("{base}/v1/openapi.json")),
                    ("llms.txt", format!("{base}/llms.txt")),
                    (
                        "The documentation as one file: llms-full.txt",
                        format!("{base}/llms-full.txt"),
                    ),
                    (
                        "API catalog",
                        format!("{base}/.well-known/api-catalog"),
                    ),
                    (
                        "MCP server card, for a client that is finding the server",
                        format!("{base}/mcp/server-card"),
                    ),
                    (
                        "AI catalog, for a client that has only this domain",
                        format!("{base}/.well-known/ai-catalog.json"),
                    ),
                ],
            ),
        ],
    };
    // Said only by a server that holds recalls: without them every address
    // below answers 404.
    if crate::llms::holds_recalls(state) {
        let recalls = with_code(
            section(
                "Recalls",
                &[
                    "Recall campaigns from NHTSA, for the United States, and from Transport Canada, newest first. A campaign lists a year, a make and a model, so a recall listed here may apply to a vehicle of that model year. Only the manufacturer can say whether it applies to one vehicle: every answer has may_apply set to true and a lookup object with the official lookups by VIN.",
                    "The last address lists the campaigns reported on or after a day, each with the vehicles it lists. Add country=us or country=ca for one country. as_of is the day each country's data was current, and attribution holds the credit the data's licence asks for. A server run without recall data answers these addresses with 404 and the code dataset_unavailable.",
                ],
            ),
            format!(
                "curl {base}/v1/vehicles/2019_honda_civic/recalls\ncurl {base}/v1/vin/KM8K2CAB4PU001140/recalls\ncurl {base}/v1/recalls/nhtsa/19V888000\ncurl '{base}/v1/recalls?make=honda&year=2019'\ncurl '{base}/v1/recalls/recent?since=2026-01-01'"
            ),
        );
        let before_errors = doc
            .sections
            .iter()
            .position(|section| section.heading == "Errors")
            .unwrap_or(doc.sections.len());
        doc.sections.insert(before_errors, recalls);
    }
    // Said only by a server that holds the ratings: without them the
    // address below answers 404.
    if crate::llms::holds_ratings(state) {
        let ratings = with_code(
            section(
                "Crash test ratings",
                &[
                    "NHTSA's crash test ratings for a model year, from its New Car Assessment Program. NHTSA rates a vehicle by body style and drive, so the answer often has several ratings and their stars can differ: each says which vehicle it is for, in NHTSA's words, and none is picked for you. A rating with no stars is a vehicle NHTSA lists and has not tested.",
                    "as_of is the day NHTSA's file was current. The same answer is the safety member of the overview of a vehicle. A server run without the ratings answers this address with 404 and the code dataset_unavailable.",
                ],
            ),
            format!("curl {base}/v1/vehicles/2019_honda_civic/safety"),
        );
        let before_errors = doc
            .sections
            .iter()
            .position(|section| section.heading == "Errors")
            .unwrap_or(doc.sections.len());
        doc.sections.insert(before_errors, ratings);
    }
    // The same for fuel, specifications, fluids and tire fitments: said
    // only of the ones this server holds.
    let held = crate::llms::held_facts(state);
    if let Some(first) = held.first() {
        let listed: Vec<String> = held
            .iter()
            .map(|topic| format!("/v1/vehicles/{{id}}/{topic}"))
            .collect();
        let facts = with_code(
            section(
                "Fuel, specifications, fluids and tires",
                &[
                    "What is held about a model year beside its recalls: fuel consumption ratings from EPA and Natural Resources Canada, specifications from EPA's files, the engine oil grade EPA records with the fluids a person read from the maker's own documents, and the original-equipment tire fitments. Each has an address for a vehicle id and one for a VIN, such as /v1/vin/{vin}/fuel.",
                    "A figure is never given more exactly than it was matched: confidence says model, engine or submodel, and an engine is named only where the match settled one. By VIN, fuel, specs and fluids hold only what is for the decoded engine when the decode settles it, and engine_settled says so. Tire fitments are never narrowed. A record read by hand carries its source and the day it was checked, and an empty list means nobody has recorded it: nothing is estimated.",
                    &format!(
                        "This server holds: {}. An address it does not hold answers 404 with the code dataset_unavailable.",
                        listed.join(", ")
                    ),
                ],
            ),
            format!("curl {base}/v1/vehicles/2019_honda_civic/{first}"),
        );
        let before_errors = doc
            .sections
            .iter()
            .position(|section| section.heading == "Errors")
            .unwrap_or(doc.sections.len());
        doc.sections.insert(before_errors, facts);
    }
    doc
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
            with_table(
                section(
                    "Licences",
                    &[
                        "Every dataset this site serves, where it comes from, and what its licence asks of anyone who republishes it. The same table is in NOTICE.md in the repository.",
                    ],
                ),
                licences::table(),
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
        intro: "Wenmar Open is a free VIN decoder and vehicle catalog for auto repair shops, with free shop calculators. There is no account, no key and nothing to install.".to_owned(),
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
                &["There are no cookies, no accounts and no analytics scripts. The service's own log records the first 11 characters of a decoded VIN, which name the maker, model and plant, and not the serial number. The hosting proxy in front of the service keeps a request log of its own, with the address of each request and where it came from. The address of a decode, as a page or through the API, holds the whole VIN. A report sent through the form at /feedback is posted as a public issue on GitHub, with the text typed and nothing else: not the sender's address."],
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

fn show(
    state: &AppState,
    path: &'static str,
    title: &str,
    description: &str,
    doc: Doc,
    format: Format,
) -> Response {
    if format == Format::Markdown {
        let canonical = format!("{}{path}", state.config().base_url);
        return markdown::response(markdown::doc(&doc), &canonical);
    }
    let page = Page::new(state, title, description)
        .indexed(state, path)
        .with_markdown(&format!("{path}.md"))
        .in_section(path.trim_start_matches('/'));
    let page = if path == "/about" {
        page.describing(vec![jsonld::organization(&state.config().base_url)])
    } else {
        page
    };
    render(page, doc)
}

const DOCS_TITLE: &str = "Free VIN decoder API, no key - Wenmar Open";
const DOCS: &str = "A free JSON API for VIN decoding and a year, make, model, trim and engine catalog. No key, no account, open to any website. Examples to copy.";
const DATA_TITLE: &str = "Vehicle data download: NHTSA vPIC as SQLite - Wenmar Open";
const DATA: &str = "Vehicle data built each month from NHTSA's vPIC: the version this site serves, where it comes from, and how to download it as one SQLite file.";
const ABOUT_TITLE: &str = "About Wenmar Open";
const ABOUT: &str = "What Wenmar Open is, who runs it, and what it keeps.";

pub async fn docs_html(State(state): State<AppState>) -> Response {
    show(
        &state,
        "/docs",
        DOCS_TITLE,
        DOCS,
        docs(&state),
        Format::Html,
    )
}

pub async fn docs_md(State(state): State<AppState>) -> Response {
    show(
        &state,
        "/docs",
        DOCS_TITLE,
        DOCS,
        docs(&state),
        Format::Markdown,
    )
}

pub async fn data_html(State(state): State<AppState>) -> Response {
    show(
        &state,
        "/data",
        DATA_TITLE,
        DATA,
        data(&state),
        Format::Html,
    )
}

pub async fn data_md(State(state): State<AppState>) -> Response {
    show(
        &state,
        "/data",
        DATA_TITLE,
        DATA,
        data(&state),
        Format::Markdown,
    )
}

pub async fn about_html(State(state): State<AppState>) -> Response {
    show(
        &state,
        "/about",
        ABOUT_TITLE,
        ABOUT,
        about(&state),
        Format::Html,
    )
}

pub async fn about_md(State(state): State<AppState>) -> Response {
    show(
        &state,
        "/about",
        ABOUT_TITLE,
        ABOUT,
        about(&state),
        Format::Markdown,
    )
}
