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
use crate::site::tools::TOOLS;
use crate::site::{markdown, pages};
use crate::state::AppState;

/// What the service is, in one paragraph. Both files open with it.
fn summary(data_version: &str, recalls: bool) -> String {
    let recalls = if recalls {
        " Recall campaigns from NHTSA and Transport Canada, listed by model year."
    } else {
        ""
    };
    format!(
        "Free vehicle data and shop calculators for auto repair shops: VIN decoding, a year, make, model, submodel and engine catalog built from NHTSA's vPIC, and calculators for a shop's prices, margins and Canadian invoice taxes.{recalls} No API key and no account. Read-only. Data version {data_version}."
    )
}

/// What `/llms.txt` says about recalls, on a server that holds them: a
/// paragraph of prose, the lines of `## API` and the lines of
/// `## Reference pages`. The words "may apply" are the point: nothing here
/// is a lookup by VIN.
fn recall_text(base: &str) -> (String, String, String) {
    let prose = format!(
        "Recall campaigns from NHTSA (United States) and Transport Canada are listed by model year: fetch `{base}/v1/vehicles/` followed by a vehicle id and `/recalls`, or `{base}/v1/vin/` followed by a VIN and `/recalls`. A campaign lists a year, a make and a model, so a recall listed here may apply to a vehicle of that model year. Only the manufacturer can say whether it applies to one vehicle: every answer has `may_apply: true` and a `lookup` object with the official lookups by VIN. `as_of` is the day each country's data was current.\n\n"
    );
    let api = format!(
        "- [Recalls that may apply to a model year]({base}/v1/vehicles/2019_honda_civic/recalls): `GET /v1/vehicles/{{id}}/recalls?country=`
- [Recalls that may apply to the model year a VIN decodes to]({base}/v1/vin/KM8K2CAB4PU001140/recalls): `GET /v1/vin/{{vin}}/recalls?country=`
- [One recall campaign]({base}/v1/recalls/nhtsa/19V888000): `GET /v1/recalls/{{source}}/{{campaign}}`
- [Recall campaigns, listed and searched]({base}/v1/recalls?make=honda&year=2019): `GET /v1/recalls?make=&model=&year=&component=&q=&country=&limit=&cursor=`
- [Recall campaigns reported since a day]({base}/v1/recalls/recent?since=2026-01-01): `GET /v1/recalls/recent?since=&country=&make=`
"
    );
    let pages = format!(
        "- [Recalls that may apply to a model year, such as the 2019 Honda Civic]({base}/makes/honda/civic/2019/recalls.md): `/makes/{{make}}/{{model}}/{{year}}/recalls.md`
- [One recall campaign, such as NHTSA 19V888000]({base}/recalls/nhtsa/19V888000.md): `/recalls/{{source}}/{{campaign}}.md`
"
    );
    (prose, api, pages)
}

/// The text of `/llms.txt` for a server at `base` serving `data_version`,
/// where one address may make `limit` requests a minute. `recalls` says
/// whether the server holds recall campaigns: what is said of them is left
/// out of a server that would answer every recall route with 404.
/// `ratings` says whether it holds NHTSA's crash test ratings, and the
/// address of those is left out in the same way.
pub fn text(base: &str, data_version: &str, limit: u32, recalls: bool, ratings: bool) -> String {
    let rating_api = if ratings {
        format!(
            "- [NHTSA's crash test ratings for a model year]({base}/v1/vehicles/2019_honda_civic/safety): `GET /v1/vehicles/{{id}}/safety`\n"
        )
    } else {
        String::new()
    };
    let (recall_prose, recall_api, recall_pages) = if recalls {
        recall_text(base)
    } else {
        Default::default()
    };
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
    let tools: String = TOOLS
        .iter()
        .map(|tool| {
            format!(
                "- [{}]({base}{}.md): {}\n",
                tool.title,
                tool.path(),
                tool.summary
            )
        })
        .collect();
    format!(
        "# Wenmar Open

> {summary}

To decode a VIN, fetch `{base}/v1/vin/` followed by the VIN, and read the JSON. No key, header or sign-up is needed. Spaces and dashes are ignored. A wrong check digit is not an error: the decode has `valid: false` and a warning, because many genuine VINs from outside North America fail the check. Up to 50 VINs at once: `POST {base}/v1/vin/batch` with `{{ \"vins\": [...] }}`.

Responses are plain JSON with no wrapper. Errors are `{{ \"error\": {{ \"code\", \"message\", \"details\" }} }}`. One address may make {limit} requests a minute; over that the answer is 429 with `Retry-After`. Fields and endpoints are only ever added.

An MCP server is at `{base}/mcp` (Streamable HTTP, no key). Its tools are `wenmar_vin` (actions `decode`, `batch`, `overview`), `wenmar_vehicles` (actions `years`, `makes`, `models`, `submodels`, `engines`, `search`, `entry`, `overview`, `model_years`, `safety`) and `wenmar_recalls` (actions `vehicle`, `vin`, `campaign`, `search`, `recent`). `wenmar_recalls` lists recalls by year, make and model and never looks a VIN up: confirm by VIN with the manufacturer.

The calculators under `{base}/tools` are web pages with forms, for people. An agent does the arithmetic itself: the Markdown version of each calculator states its formula and one worked example.

{recall_prose}Vehicle ids such as `2019_honda_civic_si` are built from the year and the names, and stay the same between data releases for as long as the names do. Every reference page has a Markdown version at the same address with `.md` added. The data is what manufacturers reported to NHTSA. It can be incomplete, especially for vehicles never sold in the United States.

## API

- [OpenAPI description]({base}/v1/openapi.json): every endpoint, parameter and response
- [Decode a VIN]({base}/v1/vin/KM8K2CAB4PU001140): `GET /v1/vin/{{vin}}`, optional `?year=`
- [Years]({base}/v1/vehicles/years): `GET /v1/vehicles/years`
- [Makes]({base}/v1/vehicles/makes?year=2019): `GET /v1/vehicles/makes?year=`
- [One make, with links to the maker's own pages]({base}/v1/makes/honda): `GET /v1/makes/{{make}}`
- [Models]({base}/v1/vehicles/models?make=honda&year=2019): `GET /v1/vehicles/models?make=&year=`
- [Submodels]({base}/v1/vehicles/submodels?make=honda&model=civic&year=2019): `GET /v1/vehicles/submodels?make=&model=&year=`
- [Engines]({base}/v1/vehicles/engines?make=honda&model=civic&year=2019): `GET /v1/vehicles/engines?make=&model=&year=&submodel=`
- [Search]({base}/v1/vehicles/search?q=2019+civic+si): `GET /v1/vehicles/search?q=`
- [One vehicle by id]({base}/v1/vehicles/2019_honda_civic): `GET /v1/vehicles/{{id}}`
- [Everything held for one model year]({base}/v1/vehicles/2019_honda_civic/overview): `GET /v1/vehicles/{{id}}/overview`
- [Everything held for the vehicle a VIN decodes to]({base}/v1/vin/KM8K2CAB4PU001140/overview): `GET /v1/vin/{{vin}}/overview`
- [Every model year of one model]({base}/v1/models/honda/civic): `GET /v1/models/{{make}}/{{model}}`
{rating_api}{recall_api}- [Data version]({base}/v1/meta): `GET /v1/meta`

## Docs

- [API reference]({base}/docs.md): examples to copy, errors, limits and caching
- [Everything in one file]({base}/llms-full.txt): the API reference, the VIN guides and the notes on the data, as one Markdown document
{guides}
## Shop calculators

- [Shop calculators]({base}/tools.md): the list of calculators
{tools}
## Rules for repair shops

- [Canadian rules for repair shops, by province]({base}/rules/canada.md): what provincial law asks of a repair shop, each rule with its source and the day it was read; a province not yet checked shows no rule
- [One province, such as Ontario]({base}/rules/canada/on.md): `/rules/canada/{{code}}.md`

## Reference pages

- [Makes]({base}/makes.md): each make links to its models, and each model year to its trims and engines
- [One make, such as Honda]({base}/makes/honda.md): `/makes/{{make}}.md`
- [One model year, such as the 2019 Honda Civic]({base}/makes/honda/civic/2019.md): `/makes/{{make}}/{{model}}/{{year}}.md`
- [One manufacturer code, such as KM8]({base}/wmi/KM8.md): `/wmi/{{code}}.md`
{recall_pages}
## Optional

- [About the data]({base}/data.md): the data version, its source and how to download it
- [About]({base}/about.md): who runs the service and what it keeps
",
        summary = summary(data_version, recalls)
    )
}

/// The site's documentation as one Markdown document: the summary, then
/// the API reference, the guides, the notes on the data, and who runs the
/// service. Each is the `Doc` its page is rendered from, one heading level
/// down, so this file cannot say something a page does not.
pub fn full(state: &AppState) -> String {
    let base = &state.config().base_url;
    let mut text = format!(
        "# Wenmar Open\n\n> {}\n\nThis is the documentation of {base} as one file. The same text is on the pages {base}/docs, {base}/guides, {base}/tools, {base}/data and {base}/about.\n",
        summary(&state.db().meta().data_version, holds_recalls(state))
    );
    let mut docs = vec![pages::docs(state)];
    docs.extend(GUIDES.iter().map(|guide| guide.doc(base)));
    docs.extend(TOOLS.iter().map(|tool| tool.doc(base)));
    docs.push(crate::site::rules::index_doc(base));
    docs.extend(
        crate::site::rules::PROVINCES
            .iter()
            .filter(|province| province.confirmed)
            .map(|province| crate::site::rules::province_doc(base, province)),
    );
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

/// Whether this server holds the recalls of either country.
pub fn holds_recalls(state: &AppState) -> bool {
    crate::api::recalls::recall_shop(state, None).is_ok()
}

/// Whether this server holds NHTSA's crash test ratings.
pub fn holds_ratings(state: &AppState) -> bool {
    state.dataset(wenmar_shopdata::Dataset::Ratings).is_ok()
}

pub async fn llms_txt(State(state): State<AppState>) -> Response {
    plain(text(
        &state.config().base_url,
        &state.db().meta().data_version,
        state.config().requests_per_minute,
        holds_recalls(&state),
        holds_ratings(&state),
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
