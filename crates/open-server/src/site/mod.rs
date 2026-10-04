//! The website: pages for someone at a service counter or beside a car.
//!
//! Every page is rendered on the server and works with JavaScript off.
//! Reference pages also have a Markdown version at the same address with
//! `.md` added, for AI agents.

pub mod assets;
pub mod catalog;
pub mod guides;
pub mod home;
pub mod jsonld;
pub mod markdown;
pub mod pages;
pub mod seo;
pub mod tokens;
pub mod tools;
pub mod vin;
pub mod wmi;

use askama::Template;
use axum::Router;
use axum::body::Body;
use axum::extract::{Request, State};
use axum::http::{HeaderMap, HeaderName, HeaderValue, Method, StatusCode, Uri, header};
use axum::middleware::Next;
use axum::response::{Html, IntoResponse, Response};
use axum::routing::get;

use crate::error::ApiError;
use crate::headers;
use crate::state::AppState;

/// What a browser may load on a page of this site: its own stylesheet,
/// script, fonts and images, and nothing else. An injected `<script>` would
/// not run even if one got past the escaping.
pub const CONTENT_SECURITY_POLICY: &str = "default-src 'none'; style-src 'self'; script-src 'self'; img-src 'self'; font-src 'self'; form-action 'self'; base-uri 'none'; frame-ancestors 'none'";

/// How long a page may be cached. Pages change only with the data file or
/// the server, and the `ETag` names both.
pub const CACHE_PAGE: &str = "public, max-age=3600";

/// A link to Wenmar Pro. The marker lets Wenmar Pro count visits that came
/// from here; `placement` says which link was followed. Nothing about the
/// visitor or the vehicle is ever in it.
pub fn pro_link(placement: &str) -> String {
    format!(
        "https://wenmarpro.com/?utm_source=wenmar-open&utm_medium=referral&utm_campaign={placement}"
    )
}

/// A model year in an address: exactly four digits. `2019` has one address;
/// `+2019` and `02019`, which a number parser would also accept, have none.
pub fn year_in(text: &str) -> Option<u16> {
    if text.len() == 4 && text.bytes().all(|byte| byte.is_ascii_digit()) {
        text.parse().ok()
    } else {
        None
    }
}

/// What the page layout needs from every page.
#[derive(Debug, Clone)]
pub struct Page {
    pub title: String,
    pub description: String,
    /// The page's own address, for pages that should be indexed.
    pub canonical: Option<String>,
    /// The address of the Markdown version, when there is one.
    pub markdown: Option<String>,
    /// Whether search engines may index the page.
    pub index: bool,
    pub data_version: String,
    /// Names the build in the address of the stylesheet and the script.
    pub asset_version: &'static str,
    pub pro_header: String,
    /// The site's public address, for the full addresses the head needs.
    pub base: String,
    /// The entry of the header's navigation the page belongs under:
    /// `makes`, `guides`, `docs`, `data`, `about`, or nothing.
    pub section: &'static str,
    /// The pages above this one, as name and address, outermost first.
    /// They are shown above the heading.
    pub crumbs: Vec<(String, String)>,
    /// Whether the fixed-width face is on the first screen, and so worth
    /// asking for early.
    pub mono_first: bool,
    /// `website`, or `article` for a page of prose.
    pub og_type: &'static str,
    /// Structured data, already made safe for a script element. Only an
    /// indexed page shows it.
    pub json_ld: Option<String>,
}

impl Page {
    /// A page that is not indexed. Call [`Page::indexed`] for one that is.
    pub fn new(state: &AppState, title: impl Into<String>, description: impl Into<String>) -> Page {
        Page {
            title: title.into(),
            description: description.into(),
            canonical: None,
            markdown: None,
            index: false,
            data_version: state.db().meta().data_version.clone(),
            asset_version: crate::BUILD_ID,
            pro_header: pro_link("header"),
            base: state.config().base_url.clone(),
            section: "",
            crumbs: Vec::new(),
            mono_first: false,
            og_type: "website",
            json_ld: None,
        }
    }

    /// Marks the page as one to index, at `path` on the public address.
    pub fn indexed(mut self, state: &AppState, path: &str) -> Page {
        self.canonical = Some(format!("{}{path}", state.config().base_url));
        self.index = true;
        self
    }

    /// Says the page has a Markdown version at `path`.
    pub fn with_markdown(mut self, path: &str) -> Page {
        self.markdown = Some(path.to_owned());
        self
    }

    /// Says which entry of the header's navigation the page is under.
    pub fn in_section(mut self, section: &'static str) -> Page {
        self.section = section;
        self
    }

    /// Says which pages are above this one, outermost first.
    pub fn under(mut self, crumbs: Vec<(String, String)>) -> Page {
        self.crumbs = crumbs;
        self
    }

    /// Says a VIN or a code is on the page's first screen.
    pub fn with_mono(mut self) -> Page {
        self.mono_first = true;
        self
    }

    /// Says the page is a piece of prose.
    pub fn as_article(mut self) -> Page {
        self.og_type = "article";
        self
    }

    /// Describes the page to search engines as structured data: `things`
    /// are schema.org objects. Only an indexed page shows them.
    pub fn describing(mut self, things: Vec<serde_json::Value>) -> Page {
        self.json_ld = Some(jsonld::graph(things));
        self
    }

    /// The way down to this page, as structured data: the pages above it,
    /// then `name`, which is the page itself.
    pub fn trail(&self, name: &str) -> serde_json::Value {
        jsonld::breadcrumbs(&self.base, &self.crumbs, name)
    }
}

/// Renders a template as an HTML response.
pub fn html<T: Template>(status: StatusCode, template: &T) -> Response {
    match template.render() {
        Ok(body) => (status, Html(body)).into_response(),
        Err(error) => {
            tracing::error!(%error, "a page could not be rendered");
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Html("<!doctype html><title>Something went wrong</title><p>Something went wrong on our side.</p>"),
            )
                .into_response()
        }
    }
}

#[derive(Template)]
#[template(path = "problem.html")]
pub struct Problem {
    pub page: Page,
    pub heading: String,
    pub message: String,
    /// VINs the visitor may have meant.
    pub suggestions: Vec<String>,
    /// Whether to show the VIN box again.
    pub retry: bool,
}

/// The page for an address that does not exist.
pub fn not_found(state: &AppState) -> Response {
    html(
        StatusCode::NOT_FOUND,
        &Problem {
            page: Page::new(state, "Not found - Wenmar Open", "There is no page here."),
            heading: "There is no page here".to_owned(),
            message: "The address may be mistyped, or the page may have moved.".to_owned(),
            suggestions: Vec::new(),
            retry: false,
        },
    )
}

/// The page for a failure that is not the visitor's doing.
pub fn failed(state: &AppState, error: &ApiError) -> Response {
    let (heading, message) = match error {
        ApiError::Unavailable => ("The server is busy", "Try again in a moment."),
        _ => (
            "Something went wrong",
            "It is on our side, not yours. Try again in a moment.",
        ),
    };
    html(
        error.status(),
        &Problem {
            page: Page::new(state, format!("{heading} - Wenmar Open"), message),
            heading: heading.to_owned(),
            message: message.to_owned(),
            suggestions: Vec::new(),
            retry: false,
        },
    )
}

/// Answers an address nothing else matched: JSON under `/v1`, a page
/// everywhere else.
pub async fn fallback(State(state): State<AppState>, uri: Uri) -> Response {
    if uri.path().starts_with("/v1/") || uri.path() == "/v1" {
        return ApiError::NotFound("There is nothing at this address.".to_owned()).into_response();
    }
    not_found(&state)
}

fn is_page(path: &str) -> bool {
    !(path.starts_with("/v1/")
        || path == "/v1"
        || path == "/mcp"
        || path == "/health"
        || path == "/.well-known/api-catalog")
}

/// What a browser may do with any page of this site, whatever its status.
pub fn secure(headers: &mut HeaderMap) {
    headers.insert(
        header::CONTENT_SECURITY_POLICY,
        HeaderValue::from_static(CONTENT_SECURITY_POLICY),
    );
    headers.insert(
        header::X_CONTENT_TYPE_OPTIONS,
        HeaderValue::from_static("nosniff"),
    );
    // Another site is told only that the visitor came from this one, never
    // which page: a result page's address holds a VIN.
    headers.insert(
        header::REFERRER_POLICY,
        HeaderValue::from_static("strict-origin-when-cross-origin"),
    );
}

/// Whether a handler said its answer is for this visitor alone, or for no
/// cache at all.
fn kept_private(headers: &HeaderMap) -> bool {
    headers
        .get(header::CACHE_CONTROL)
        .and_then(|value| value.to_str().ok())
        .is_some_and(|value| value.starts_with("private") || value.starts_with("no-store"))
}

/// Headers for every page: what the browser may load, how long the page
/// may be cached, and a 304 when the visitor already has it.
///
/// The handler always runs. A 304 is sent only in place of a 200 that
/// anyone may cache, so a result page, an error and a redirect are never
/// answered "unchanged", and only a public page carries an `ETag`.
pub async fn page_headers(State(state): State<AppState>, request: Request, next: Next) -> Response {
    let path = request.uri().path().to_owned();
    if !is_page(&path) {
        let mut response = next.run(request).await;
        // JSON and MCP answers are not pages to index.
        response
            .headers_mut()
            .insert("x-robots-tag", HeaderValue::from_static("noindex"));
        return response;
    }
    let cacheable = matches!(*request.method(), Method::GET | Method::HEAD);
    let has_it = cacheable
        && request
            .headers()
            .get(header::IF_NONE_MATCH)
            .is_some_and(|value| headers::names(value, state.etag_text()));
    let mut response = next.run(request).await;
    let public =
        cacheable && response.status() == StatusCode::OK && !kept_private(response.headers());
    if public && has_it {
        let mut unchanged = Response::new(Body::empty());
        *unchanged.status_mut() = StatusCode::NOT_MODIFIED;
        // What the handler said about the page still holds for the copy the
        // visitor has.
        for name in [
            header::CACHE_CONTROL,
            header::LINK,
            HeaderName::from_static("x-robots-tag"),
        ] {
            if let Some(value) = response.headers().get(&name) {
                unchanged.headers_mut().insert(name, value.clone());
            }
        }
        response = unchanged;
    }
    let headers = response.headers_mut();
    secure(headers);
    if public {
        headers.insert(header::ETAG, state.etag().clone());
        if !headers.contains_key(header::CACHE_CONTROL) {
            headers.insert(header::CACHE_CONTROL, HeaderValue::from_static(CACHE_PAGE));
        }
    } else if !headers.contains_key(header::CACHE_CONTROL) {
        headers.insert(header::CACHE_CONTROL, HeaderValue::from_static("no-store"));
    }
    response
}

/// Turns a refusal made before a page's handler ran into a page.
///
/// The ceiling on requests, the limit on requests in flight, the time
/// limit, the limit on an address's length and a caught panic all answer
/// with the API's JSON error. That is right under `/v1` and wrong for a
/// person at a browser.
pub async fn page_refusals(
    State(state): State<AppState>,
    request: Request,
    next: Next,
) -> Response {
    let page = is_page(request.uri().path());
    let response = next.run(request).await;
    let status = response.status();
    let (heading, message) = match status {
        StatusCode::TOO_MANY_REQUESTS => (
            "Too many requests",
            "More requests came from this address in one minute than the limit allows. Wait a minute and try again.",
        ),
        StatusCode::URI_TOO_LONG => (
            "That address is too long",
            "A VIN has 17 characters. Go back to the start and type it again.",
        ),
        StatusCode::SERVICE_UNAVAILABLE => ("The server is busy", "Try again in a moment."),
        StatusCode::INTERNAL_SERVER_ERROR => (
            "Something went wrong",
            "It is on our side, not yours. Try again in a moment.",
        ),
        _ => return response,
    };
    let is_json = response
        .headers()
        .get(header::CONTENT_TYPE)
        .and_then(|value| value.to_str().ok())
        .is_some_and(|value| value.starts_with("application/json"));
    if !page || !is_json {
        return response;
    }
    let mut refusal = html(
        status,
        &Problem {
            page: Page::new(&state, format!("{heading} - Wenmar Open"), message),
            heading: heading.to_owned(),
            message: message.to_owned(),
            suggestions: Vec::new(),
            retry: false,
        },
    );
    let headers = refusal.headers_mut();
    secure(headers);
    headers.insert(header::CACHE_CONTROL, HeaderValue::from_static("no-store"));
    headers.insert("x-robots-tag", HeaderValue::from_static("noindex"));
    if let Some(wait) = response.headers().get(header::RETRY_AFTER) {
        headers.insert(header::RETRY_AFTER, wait.clone());
    }
    refusal
}

/// The pages.
pub fn router() -> Router<AppState> {
    Router::new()
        .route("/", get(home::home))
        .route("/vin", get(home::vin_form))
        .route("/pick", get(home::pick))
        .route("/vin/{vin}", get(vin::result))
        .route("/makes", get(catalog::makes))
        .route("/makes.md", get(catalog::makes_md))
        .route("/makes/{make}", get(catalog::make))
        .route("/makes/{make}/{model}/{year}", get(catalog::model_year))
        .route("/wmi/{code}", get(wmi::wmi))
        .route("/guides", get(guides::index_html))
        .route("/guides.md", get(guides::index_md))
        .route("/guides/{page}", get(guides::guide))
        .route("/docs", get(pages::docs_html))
        .route("/docs.md", get(pages::docs_md))
        .route("/data", get(pages::data_html))
        .route("/data.md", get(pages::data_md))
        .route("/about", get(pages::about_html))
        .route("/about.md", get(pages::about_md))
        .route("/robots.txt", get(seo::robots))
        .route("/sitemap.xml", get(seo::index))
        .route("/sitemaps/{file}", get(seo::sitemap))
        .route("/assets/site.css", get(assets::stylesheet))
        .route("/assets/site.js", get(assets::script))
        .route(
            "/assets/fonts/dm-sans-latin-wght.woff2",
            get(assets::font_sans),
        )
        .route(
            "/assets/fonts/jetbrains-mono-latin-400.woff2",
            get(assets::font_mono),
        )
        .route("/assets/fonts/OFL-DM-Sans.txt", get(assets::licence_sans))
        .route(
            "/assets/fonts/OFL-JetBrains-Mono.txt",
            get(assets::licence_mono),
        )
        .route("/assets/favicon.svg", get(assets::favicon_svg))
        .route("/assets/favicon-96.png", get(assets::favicon_png))
        .route("/assets/apple-touch-icon.png", get(assets::touch_icon))
        .route("/assets/og.png", get(assets::share_image))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_year_in_an_address_is_exactly_four_digits() {
        assert_eq!(year_in("2019"), Some(2019));
        assert_eq!(year_in("1981"), Some(1981));
        for bad in [
            "+2019",
            "02019",
            "201",
            "",
            "20 9",
            "2019.0",
            "２０１９",
            "-201",
        ] {
            assert_eq!(year_in(bad), None, "{bad}");
        }
    }
}
