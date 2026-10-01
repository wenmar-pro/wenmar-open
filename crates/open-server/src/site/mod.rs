//! The website: pages for someone at a service counter or beside a car.
//!
//! Every page is rendered on the server and works with JavaScript off.
//! Reference pages also have a Markdown version at the same address with
//! `.md` added, for AI agents.

pub mod assets;
pub mod catalog;
pub mod home;
pub mod markdown;
pub mod tokens;
pub mod vin;

use askama::Template;
use axum::Router;
use axum::body::Body;
use axum::extract::{Request, State};
use axum::http::{HeaderValue, Method, StatusCode, Uri, header};
use axum::middleware::Next;
use axum::response::{Html, IntoResponse, Response};
use axum::routing::get;

use crate::error::ApiError;
use crate::headers;
use crate::state::AppState;

/// What a browser may load on a page of this site: its own stylesheet and
/// script, and nothing else. An injected `<script>` would not run even if
/// one got past the escaping.
pub const CONTENT_SECURITY_POLICY: &str = "default-src 'none'; style-src 'self'; script-src 'self'; img-src 'self' data:; form-action 'self'; base-uri 'none'; frame-ancestors 'none'";

/// How long a page may be cached. Pages change only with the data file or
/// the server.
pub const CACHE_PAGE: &str = "public, max-age=3600";

/// A link to Wenmar Pro. The marker lets Wenmar Pro count visits that came
/// from here; `placement` says which link was followed. Nothing about the
/// visitor or the vehicle is ever in it.
pub fn pro_link(placement: &str) -> String {
    format!(
        "https://wenmarpro.com/?utm_source=wenmar-open&utm_medium=referral&utm_campaign={placement}"
    )
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
    pub asset_version: &'static str,
    pub pro_header: String,
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
            asset_version: env!("CARGO_PKG_VERSION"),
            pro_header: pro_link("header"),
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
    !(path.starts_with("/v1/") || path == "/v1" || path == "/mcp" || path == "/health")
}

/// Headers for every page: what the browser may load, how long the page
/// may be cached, and a 304 when the visitor already has it.
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
    let unchanged = cacheable
        && request
            .headers()
            .get(header::IF_NONE_MATCH)
            .is_some_and(|value| headers::names(value, state.etag_text()));
    let mut response = if unchanged {
        let mut response = Response::new(Body::empty());
        *response.status_mut() = StatusCode::NOT_MODIFIED;
        response
    } else {
        next.run(request).await
    };
    let status = response.status();
    let headers = response.headers_mut();
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
    if cacheable && (status == StatusCode::OK || status == StatusCode::NOT_MODIFIED) {
        headers.insert(header::ETAG, state.etag().clone());
        if !headers.contains_key(header::CACHE_CONTROL) {
            headers.insert(header::CACHE_CONTROL, HeaderValue::from_static(CACHE_PAGE));
        }
    } else if !headers.contains_key(header::CACHE_CONTROL) {
        headers.insert(header::CACHE_CONTROL, HeaderValue::from_static("no-store"));
    }
    response
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
        .route("/assets/site.css", get(assets::stylesheet))
        .route("/assets/site.js", get(assets::script))
}
