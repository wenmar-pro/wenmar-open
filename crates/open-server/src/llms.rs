//! `/llms.txt`: what this service is, for language models and their tools.

use axum::extract::State;
use axum::http::{HeaderValue, header};
use axum::response::{IntoResponse, Response};

use crate::state::AppState;

/// The text of `/llms.txt` for a server at `base` serving `data_version`.
pub fn text(base: &str, data_version: &str) -> String {
    format!(
        "# Wenmar Open

> Free vehicle data for auto repair shops: VIN decoding and a year, make, model, submodel and engine catalog, built from NHTSA's vPIC. No API key and no account. Read-only. Data version {data_version}.

Responses are plain JSON with no wrapper. Errors are `{{ \"error\": {{ \"code\", \"message\", \"details\" }} }}`. One address may make 600 requests a minute; over that the answer is 429 with `Retry-After`. Fields and endpoints are only ever added.

## API

- [OpenAPI description]({base}/v1/openapi.json): every endpoint, parameter and response
- [Decode a VIN]({base}/v1/vin/KM8K2CAB4PU001140): `GET /v1/vin/{{vin}}`, optional `?year=`
- Decode up to 50 VINs: `POST /v1/vin/batch` with `{{ \"vins\": [...] }}`
- [Years]({base}/v1/vehicles/years): `GET /v1/vehicles/years`
- [Makes]({base}/v1/vehicles/makes?year=2019): `GET /v1/vehicles/makes?year=`
- [Models]({base}/v1/vehicles/models?make=honda&year=2019): `GET /v1/vehicles/models?make=&year=`
- [Submodels]({base}/v1/vehicles/submodels?make=honda&model=civic&year=2019): `GET /v1/vehicles/submodels?make=&model=&year=`
- [Engines]({base}/v1/vehicles/engines?make=honda&model=civic&year=2019): `GET /v1/vehicles/engines?make=&model=&year=&submodel=`
- [Search]({base}/v1/vehicles/search?q=2019+civic+si): `GET /v1/vehicles/search?q=`
- [One vehicle by id]({base}/v1/vehicles/2019_honda_civic): `GET /v1/vehicles/{{id}}`
- [Data version]({base}/v1/meta): `GET /v1/meta`

## MCP

- Remote MCP server: `{base}/mcp` (Streamable HTTP, no key). Tools: `wenmar_vin` (actions `decode`, `batch`) and `wenmar_vehicles` (actions `years`, `makes`, `models`, `submodels`, `engines`, `search`, `entry`).

## Pages

Every reference page has a Markdown version at the same address with `.md` added.

- [API reference]({base}/docs.md)
- [About the data]({base}/data.md)
- [About]({base}/about.md)
- [Makes]({base}/makes.md): each make links to its models, and each model year to its trims and engines
- One make: `{base}/makes/{{make}}.md`, such as [Honda]({base}/makes/honda.md)
- One model year: `{base}/makes/{{make}}/{{model}}/{{year}}.md`
- One manufacturer code: `{base}/wmi/{{code}}.md`

## Notes

- A wrong check digit is not an error. The decode has `valid: false` and a warning, because many genuine VINs from outside North America fail the check.
- Vehicle ids such as `2019_honda_civic_si` are built from the year and the names, and stay the same between data releases for as long as the names do.
- The data is what manufacturers reported to NHTSA. It can be incomplete, especially for vehicles never sold in the United States.
"
    )
}

pub async fn llms_txt(State(state): State<AppState>) -> Response {
    let body = text(&state.config().base_url, &state.db().meta().data_version);
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
