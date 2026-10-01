//! `/llms.txt` and `/llms-full.txt`: what this service is, for language
//! models and their tools.
//!
//! `llms.txt` follows the llms.txt convention: a title, a summary in a
//! blockquote, prose, then sections that are lists of links. `llms-full.txt`
//! is not part of that convention. It is the site's documentation as one
//! Markdown file, put together from the same text the pages are rendered
//! from.

use axum::extract::State;
use axum::http::{HeaderValue, header};
use axum::response::{IntoResponse, Response};

use crate::site::guides::GUIDES;
use crate::site::{markdown, pages};
use crate::state::AppState;

/// What the service is, in one paragraph. Both files open with it.
fn summary(data_version: &str) -> String {
    format!(
        "Free vehicle data for auto repair shops: VIN decoding and a year, make, model, submodel and engine catalog, built from NHTSA's vPIC. No API key and no account. Read-only. Data version {data_version}."
    )
}

/// The text of `/llms.txt` for a server at `base` serving `data_version`,
/// where one address may make `limit` requests a minute.
pub fn text(base: &str, data_version: &str, limit: u32) -> String {
    let guides: String = GUIDES
        .iter()
        .map(|guide| {
            format!(
                "- [{}]({base}{}.md): {}\n",
                guide.title,
                guide.path(),
                guide.description
            )
        })
        .collect();
    format!(
        "# Wenmar Open

> {summary}

To decode a VIN, fetch `{base}/v1/vin/` followed by the VIN, and read the JSON. No key, header or sign-up is needed. Spaces and dashes are ignored. A wrong check digit is not an error: the decode has `valid: false` and a warning, because many genuine VINs from outside North America fail the check. Up to 50 VINs at once: `POST {base}/v1/vin/batch` with `{{ \"vins\": [...] }}`.

Responses are plain JSON with no wrapper. Errors are `{{ \"error\": {{ \"code\", \"message\", \"details\" }} }}`. One address may make {limit} requests a minute; over that the answer is 429 with `Retry-After`. Fields and endpoints are only ever added.

An MCP server is at `{base}/mcp` (Streamable HTTP, no key). Its tools are `wenmar_vin` (actions `decode`, `batch`) and `wenmar_vehicles` (actions `years`, `makes`, `models`, `submodels`, `engines`, `search`, `entry`).

Vehicle ids such as `2019_honda_civic_si` are built from the year and the names, and stay the same between data releases for as long as the names do. Every reference page has a Markdown version at the same address with `.md` added. The data is what manufacturers reported to NHTSA. It can be incomplete, especially for vehicles never sold in the United States.

## API

- [OpenAPI description]({base}/v1/openapi.json): every endpoint, parameter and response
- [Decode a VIN]({base}/v1/vin/KM8K2CAB4PU001140): `GET /v1/vin/{{vin}}`, optional `?year=`
- [Years]({base}/v1/vehicles/years): `GET /v1/vehicles/years`
- [Makes]({base}/v1/vehicles/makes?year=2019): `GET /v1/vehicles/makes?year=`
- [Models]({base}/v1/vehicles/models?make=honda&year=2019): `GET /v1/vehicles/models?make=&year=`
- [Submodels]({base}/v1/vehicles/submodels?make=honda&model=civic&year=2019): `GET /v1/vehicles/submodels?make=&model=&year=`
- [Engines]({base}/v1/vehicles/engines?make=honda&model=civic&year=2019): `GET /v1/vehicles/engines?make=&model=&year=&submodel=`
- [Search]({base}/v1/vehicles/search?q=2019+civic+si): `GET /v1/vehicles/search?q=`
- [One vehicle by id]({base}/v1/vehicles/2019_honda_civic): `GET /v1/vehicles/{{id}}`
- [Data version]({base}/v1/meta): `GET /v1/meta`

## Docs

- [API reference]({base}/docs.md): examples to copy, errors, limits and caching
- [Everything in one file]({base}/llms-full.txt): the API reference, the VIN guides and the notes on the data, as one Markdown document
{guides}
## Reference pages

- [Makes]({base}/makes.md): each make links to its models, and each model year to its trims and engines
- [One make, such as Honda]({base}/makes/honda.md): `/makes/{{make}}.md`
- [One model year, such as the 2019 Honda Civic]({base}/makes/honda/civic/2019.md): `/makes/{{make}}/{{model}}/{{year}}.md`
- [One manufacturer code, such as KM8]({base}/wmi/KM8.md): `/wmi/{{code}}.md`

## Optional

- [About the data]({base}/data.md): the data version, its source and how to download it
- [About]({base}/about.md): who runs the service and what it keeps
",
        summary = summary(data_version)
    )
}

/// The site's documentation as one Markdown document: the summary, then
/// the API reference, the guides, the notes on the data, and who runs the
/// service. Each is the `Doc` its page is rendered from, one heading level
/// down, so this file cannot say something a page does not.
pub fn full(state: &AppState) -> String {
    let base = &state.config().base_url;
    let mut text = format!(
        "# Wenmar Open\n\n> {}\n\nThis is the documentation of {base} as one file. The same text is on the pages {base}/docs, {base}/guides, {base}/data and {base}/about.\n",
        summary(&state.db().meta().data_version)
    );
    let mut docs = vec![pages::docs(state)];
    docs.extend(GUIDES.iter().map(|guide| guide.doc(base)));
    docs.push(pages::data(state));
    docs.push(pages::about(state));
    for doc in &docs {
        text.push('\n');
        text.push_str(&markdown::doc_at(doc, 2));
    }
    text
}

fn plain(body: String) -> Response {
    (
        [
            (
                header::CONTENT_TYPE,
                HeaderValue::from_static("text/plain; charset=utf-8"),
            ),
            (
                header::CACHE_CONTROL,
                HeaderValue::from_static("public, max-age=3600"),
            ),
        ],
        body,
    )
        .into_response()
}

pub async fn llms_txt(State(state): State<AppState>) -> Response {
    plain(text(
        &state.config().base_url,
        &state.db().meta().data_version,
        state.config().requests_per_minute,
    ))
}

pub async fn llms_full_txt(State(state): State<AppState>) -> Response {
    let mut response = plain(full(&state));
    // It repeats the pages. The pages are what a search engine should show.
    response
        .headers_mut()
        .insert("x-robots-tag", HeaderValue::from_static("noindex"));
    response
}
