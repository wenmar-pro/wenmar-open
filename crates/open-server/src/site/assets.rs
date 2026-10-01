//! The stylesheet, the script, the fonts, the icons and the share image,
//! all compiled into the binary. Nothing is read from disk at run time, so
//! no request can name a file.

use std::sync::OnceLock;

use axum::http::{HeaderValue, header};
use axum::response::{IntoResponse, Response};

use crate::site::tokens;

/// The stylesheet, the script and the icons are addressed with the build id
/// (`?v=`) or are tiny, so they may be kept for a week.
const CACHE_ASSET: &str = "public, max-age=604800";

/// A font is kept for a year and never asked about again. Its address has
/// no build id, because the stylesheet and the preload link must name the
/// same address. So a font file is never replaced in place: a new file gets
/// a new name here, in `site.css` and in the tests.
const CACHE_FONT: &str = "public, max-age=31536000, immutable";

/// Where the two fonts are, for the stylesheet and for preload links.
pub const FONT_SANS_PATH: &str = "/assets/fonts/dm-sans-latin-wght.woff2";
pub const FONT_MONO_PATH: &str = "/assets/fonts/jetbrains-mono-latin-400.woff2";

static DM_SANS: &[u8] = include_bytes!("../../assets/fonts/dm-sans-latin-wght.woff2");
static JETBRAINS_MONO: &[u8] = include_bytes!("../../assets/fonts/jetbrains-mono-latin-400.woff2");
static FAVICON_PNG: &[u8] = include_bytes!("../../assets/favicon-96.png");
static TOUCH_ICON: &[u8] = include_bytes!("../../assets/apple-touch-icon.png");
static SHARE_IMAGE: &[u8] = include_bytes!("../../assets/og.png");

fn asset(content_type: &'static str, body: &'static str) -> Response {
    bytes(content_type, CACHE_ASSET, body.as_bytes())
}

fn bytes(content_type: &'static str, cache: &'static str, body: &'static [u8]) -> Response {
    (
        [
            (header::CONTENT_TYPE, HeaderValue::from_static(content_type)),
            (header::CACHE_CONTROL, HeaderValue::from_static(cache)),
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

pub async fn font_sans() -> Response {
    bytes("font/woff2", CACHE_FONT, DM_SANS)
}

pub async fn font_mono() -> Response {
    bytes("font/woff2", CACHE_FONT, JETBRAINS_MONO)
}

/// The licence each font is distributed under, beside the font.
pub async fn licence_sans() -> Response {
    asset(
        "text/plain; charset=utf-8",
        include_str!("../../assets/fonts/OFL-DM-Sans.txt"),
    )
}

pub async fn licence_mono() -> Response {
    asset(
        "text/plain; charset=utf-8",
        include_str!("../../assets/fonts/OFL-JetBrains-Mono.txt"),
    )
}

pub async fn favicon_svg() -> Response {
    asset("image/svg+xml", include_str!("../../assets/favicon.svg"))
}

pub async fn favicon_png() -> Response {
    bytes("image/png", CACHE_ASSET, FAVICON_PNG)
}

pub async fn touch_icon() -> Response {
    bytes("image/png", CACHE_ASSET, TOUCH_ICON)
}

pub async fn share_image() -> Response {
    bytes("image/png", CACHE_ASSET, SHARE_IMAGE)
}
