//! `/open-source-vin-decoder`: the page for someone who wants to call the
//! API, run the decoder themselves or connect an AI agent to it.

use axum::extract::State;
use axum::response::Response;

use crate::site::markdown::{self, Doc, Format};
use crate::site::pages::{render, section, with_code, with_links};
use crate::site::{Page, jsonld};
use crate::state::AppState;

/// The page's address. It says what a search for a self-hosted decoder
/// asks for, and it does not change.
pub const PATH: &str = "/open-source-vin-decoder";

pub const TITLE: &str = "Open source VIN decoder API and MCP server - Wenmar Open";

pub const DESCRIPTION: &str = "Free, self-hostable VIN decoder API and open source VIN lookup. Decode VINs, recalls and vehicle data over HTTP or MCP. MIT licensed, no key.";

/// The page's text. The words a developer searches for are in the opening
/// paragraph and in the headings, and each section says how to do the
/// thing it names.
pub fn doc(base: &str) -> Doc {
    Doc {
        title: "Free, self-hostable VIN decoder API".to_owned(),
        intro: "Wenmar Open is a free self-hostable VIN decoder API and open source VIN lookup. It decodes a 17-character VIN to its year, make, model, trim, engine and safety equipment, and answers for a vehicle's recalls, service bulletins, trouble codes and labour times. Every answer is JSON, with no key and no account. The same lookups are an MCP server for VIN decoding.".to_owned(),
        sections: vec![
            with_code(
                with_links(
                    section(
                        "A free VIN decoder API",
                        &[
                            "One request decodes a VIN. The answer is plain JSON with no wrapper, the data version in an X-Data-Version header, and no key, account or origin restriction. A wrong check digit is a warning, not an error, because many genuine VINs from outside North America fail the check.",
                            "The same server answers for a year, make, model, trim and engine, for recalls and service bulletins by model year, and for OBD-II trouble codes. Fields and endpoints are only ever added.",
                        ],
                    ),
                    &[
                        (
                            "OpenAPI description",
                            format!("{base}/v1/openapi.json"),
                        ),
                        ("API reference", format!("{base}/docs")),
                    ],
                ),
                format!("curl {base}/v1/vin/KM8K2CAB4PU001140"),
            ),
            with_code(
                with_links(
                    section(
                        "Self-host the open source VIN lookup",
                        &[
                            "The server, the decoder and the data tooling are one MIT-licensed Rust workspace. The server opens a SQLite file read-only, needs no database server and writes nothing. With mise installed, run it from the checkout:",
                            "bin/setup installs the pinned toolchain and builds once; mise run data downloads NHTSA's vPIC release and builds the data file; mise run serve starts the site and the API at http://localhost:3000. The Dockerfile builds the same binary with the data and shop files baked in, for a container host.",
                        ],
                    ),
                    &[
                        (
                            "Source code",
                            "https://github.com/wenmar-pro/wenmar-open".to_owned(),
                        ),
                        (
                            "Data releases",
                            "https://github.com/wenmar-pro/wenmar-open/releases".to_owned(),
                        ),
                        ("About the data", format!("{base}/data")),
                    ],
                ),
                "git clone https://github.com/wenmar-pro/wenmar-open\ncd wenmar-open\nbin/setup\nmise run data\nmise run serve".to_owned(),
            ),
            with_code(
                with_links(
                    section(
                        "MCP server for VIN decoding",
                        &[
                            "The hosted service is also an MCP server for VIN decoding. It speaks Streamable HTTP, needs no key, and offers the tools wenmar_vin, wenmar_vehicles, wenmar_recalls, wenmar_bulletins, wenmar_codes, wenmar_labor and wenmar_payroll_deductions. A connector given the bare domain works too, because the site's own address answers MCP messages.",
                            "Add the address as a remote server or a custom connector:",
                        ],
                    ),
                    &[
                        (
                            "MCP server card",
                            format!("{base}/mcp/server-card"),
                        ),
                        ("llms.txt", format!("{base}/llms.txt")),
                        ("How to use the API", format!("{base}/docs#for-ai-agents")),
                    ],
                ),
                format!(
                    "{{\n  \"mcpServers\": {{\n    \"wenmar-open\": {{\n      \"type\": \"http\",\n      \"url\": \"{base}/mcp\"\n    }}\n  }}\n}}"
                ),
            ),
            with_links(
                section(
                    "Licence and data",
                    &[
                        "The code is MIT licensed, so a copy may be run, changed and sold. The vehicle catalog and the decodes come from NHTSA's vPIC, a public-domain US government dataset. Every dataset, its source and the credit its licence asks for are listed on the data page, and the whole data set downloads as one SQLite file.",
                    ],
                ),
                &[
                    ("Licences and download", format!("{base}/data")),
                    (
                        "MIT licence",
                        "https://github.com/wenmar-pro/wenmar-open/blob/main/LICENSE".to_owned(),
                    ),
                ],
            ),
            with_links(
                section(
                    "Start here",
                    &["The reference has an example for every endpoint."],
                ),
                &[
                    ("API reference", format!("{base}/docs")),
                    ("Vehicle data download", format!("{base}/data")),
                    ("About Wenmar Open", format!("{base}/about")),
                    (
                        "Source code",
                        "https://github.com/wenmar-pro/wenmar-open".to_owned(),
                    ),
                ],
            ),
        ],
    }
}

fn show(state: &AppState, format: Format) -> Response {
    let base = &state.config().base_url;
    if format == Format::Markdown {
        let canonical = format!("{base}{PATH}");
        return markdown::response(markdown::doc(&doc(base)), &canonical);
    }
    let page = Page::new(state, TITLE, DESCRIPTION)
        .indexed(state, PATH)
        .with_markdown(&format!("{PATH}.md"))
        .in_section("docs")
        .under(vec![("API".to_owned(), "/docs".to_owned())]);
    let trail = page.trail("Free, self-hostable VIN decoder API");
    let page = page.describing(vec![
        trail,
        jsonld::web_api(base),
        jsonld::software_application(base),
        jsonld::organization(base),
    ]);
    render(page, doc(base))
}

pub async fn html(State(state): State<AppState>) -> Response {
    show(&state, Format::Html)
}

pub async fn markdown_copy(State(state): State<AppState>) -> Response {
    show(&state, Format::Markdown)
}
