//! The stylesheet and the script, compiled into the binary. Nothing is read
//! from disk at run time, so no request can name a file.

use std::sync::OnceLock;

use axum::http::{HeaderValue, header};
use axum::response::{IntoResponse, Response};

use crate::site::tokens;

/// Assets are addressed with the server's version (`?v=`), so they can be
/// cached for a long time.
const CACHE_ASSET: &str = "public, max-age=604800";

fn asset(content_type: &'static str, body: &'static str) -> Response {
    (
        [
            (header::CONTENT_TYPE, HeaderValue::from_static(content_type)),
            (header::CACHE_CONTROL, HeaderValue::from_static(CACHE_ASSET)),
        ],
        body,
    )
        .into_response()
}

/// The brand tokens followed by the site's rules.
pub fn stylesheet_text() -> &'static str {
    static CSS: OnceLock<String> = OnceLock::new();
    CSS.get_or_init(|| format!("{}{}", tokens::css(), include_str!("../../assets/site.css")))
}

pub async fn stylesheet() -> Response {
    asset("text/css; charset=utf-8", stylesheet_text())
}

pub async fn script() -> Response {
    asset(
        "text/javascript; charset=utf-8",
        include_str!("../../assets/site.js"),
    )
}
