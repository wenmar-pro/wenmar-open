//! The fonts, the icons and the share image: the site's own, compiled in.

use axum::http::StatusCode;
use http_body_util::BodyExt;

use crate::common::{self, TestApp, header, page};

/// `GET`s an asset: its type, how long it may be kept, and its bytes.
async fn asset(app: &TestApp, path: &str) -> (String, String, Vec<u8>) {
    let response = app.get(path).await;
    assert_eq!(response.status(), StatusCode::OK, "{path}");
    let content_type = header(&response, "content-type").to_owned();
    let cache = header(&response, "cache-control").to_owned();
    let body = response.into_body().collect().await.unwrap().to_bytes();
    (content_type, cache, body.to_vec())
}

/// The width and height a PNG file states in its header.
fn png_size(bytes: &[u8]) -> (u32, u32) {
    assert_eq!(&bytes[..8], b"\x89PNG\r\n\x1a\n", "not a PNG file");
    assert_eq!(&bytes[12..16], b"IHDR");
    let number = |at: usize| u32::from_be_bytes(bytes[at..at + 4].try_into().unwrap());
    (number(16), number(20))
}

#[tokio::test]
async fn the_fonts_are_the_sites_own_and_have_fallbacks() {
    let app = common::app().await;
    for (path, length) in [
        ("/assets/fonts/dm-sans-latin-wght.woff2", 36_932),
        ("/assets/fonts/jetbrains-mono-latin-400.woff2", 21_168),
    ] {
        let (content_type, cache, body) = asset(&app, path).await;
        assert_eq!(content_type, "font/woff2", "{path}");
        assert_eq!(cache, "public, max-age=31536000, immutable", "{path}");
        assert_eq!(&body[..4], b"wOF2", "{path}");
        // The file is kept for a year under this name. A different file
        // needs a different name.
        assert_eq!(body.len(), length, "{path}");
    }
    // Each font is served with its licence beside it.
    for (path, first_line) in [
        (
            "/assets/fonts/OFL-DM-Sans.txt",
            "Copyright 2014 The DM Sans Project Authors",
        ),
        (
            "/assets/fonts/OFL-JetBrains-Mono.txt",
            "Copyright 2020 The JetBrains Mono Project Authors",
        ),
    ] {
        let (content_type, _, body) = asset(&app, path).await;
        assert_eq!(content_type, "text/plain; charset=utf-8", "{path}");
        let text = String::from_utf8(body).unwrap();
        assert!(text.starts_with(first_line), "{path}");
        assert!(
            text.contains("SIL Open Font License, Version 1.1"),
            "{path}"
        );
    }
    // If a font does not arrive, the text is set in a face the visitor has.
    let (_, css) = page(&app, "/assets/site.css").await;
    assert!(
        css.contains("--sans:\"DM Sans\", \"DM Sans Fallback\", system-ui,"),
        "{:.400}",
        css
    );
    assert!(css.contains("--mono:\"JetBrains Mono\", ui-monospace,"));
    // And the browser is allowed to load fonts from this site only.
    let response = app.get("/").await;
    assert!(header(&response, "content-security-policy").contains("font-src 'self';"));
}

#[tokio::test]
async fn a_font_that_is_not_there_is_a_404_page() {
    let app = common::app().await;
    for path in [
        "/assets/fonts/nothing.woff2",
        "/assets/fonts/",
        "/assets/fonts/dm-sans-latin-wght.woff2.map",
        "/assets/fonts/..%2Fsite.css",
        "/assets/fonts/DM-Sans.woff2",
        "/favicon.ico",
    ] {
        let (status, html) = page(&app, path).await;
        assert_eq!(status, StatusCode::NOT_FOUND, "{path}");
        assert!(html.contains("<h1>There is no page here</h1>"), "{path}");
    }
}

#[tokio::test]
async fn the_icons_and_the_share_image_are_the_sites_own() {
    let app = common::app().await;
    let (content_type, cache, body) = asset(&app, "/assets/favicon.svg").await;
    assert_eq!(content_type, "image/svg+xml");
    assert_eq!(cache, "public, max-age=604800");
    let svg = String::from_utf8(body).unwrap();
    assert!(svg.starts_with("<svg xmlns=\"http://www.w3.org/2000/svg\""));
    assert!(svg.contains("fill=\"#e50914\""), "the brand's red");
    assert!(!svg.contains("<script") && !svg.contains("href"));

    for (path, size, most) in [
        ("/assets/favicon-96.png", (96, 96), 20_000),
        ("/assets/apple-touch-icon.png", (180, 180), 30_000),
        ("/assets/og.png", (1200, 630), 300_000),
    ] {
        let (content_type, _, body) = asset(&app, path).await;
        assert_eq!(content_type, "image/png", "{path}");
        assert_eq!(png_size(&body), size, "{path}");
        assert!(body.len() < most, "{path} is {} bytes", body.len());
    }
    // Every page names its icons, and none is a data: address any more.
    let (_, html) = page(&app, "/").await;
    assert!(html.contains(r#"<link rel="icon" href="/assets/favicon.svg" type="image/svg+xml">"#));
    assert!(html.contains(
        r#"<link rel="icon" href="/assets/favicon-96.png" type="image/png" sizes="96x96">"#
    ));
    assert!(html.contains(r#"<link rel="apple-touch-icon" href="/assets/apple-touch-icon.png">"#));
    assert!(!html.contains("data:"), "{html}");
}

#[tokio::test]
async fn the_home_page_stays_light() {
    let app = common::app().await;
    // Everything the home page loads besides its own HTML.
    let mut fixed = 0;
    for path in [
        "/assets/site.css",
        "/assets/site.js",
        "/assets/fonts/dm-sans-latin-wght.woff2",
        "/assets/fonts/jetbrains-mono-latin-400.woff2",
        "/assets/favicon.svg",
    ] {
        fixed += asset(&app, path).await.2.len();
    }
    assert!(fixed <= 110_000, "the fixed assets are {fixed} bytes");
    // The page itself. On the real data file it also holds some 400 makes,
    // which the runbook's check measures against 40 KB.
    let (_, html) = page(&app, "/").await;
    assert!(
        html.len() <= 12_000,
        "the home page is {} bytes",
        html.len()
    );
}
