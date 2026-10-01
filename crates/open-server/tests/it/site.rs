//! What every page has in common, shown on the page for an address that
//! does not exist.

use axum::body::Body;
use axum::http::{Request, StatusCode};

use crate::common::{self, assert_basics, assert_no_injection, body_json, body_text, header, page};

#[tokio::test]
async fn an_address_that_does_not_exist_is_a_page_and_names_no_file() {
    let app = common::app().await;
    for path in [
        "/nothing",
        "/assets/nothing.css",
        "/assets/../Cargo.toml",
        "/assets/%2e%2e/Cargo.toml",
        "/assets/..%2F..%2FCargo.toml",
        "/assets/site.css/../../../etc/passwd",
        "/assets//etc/passwd",
        "/.env",
        "/.git/config",
        "/%3Cscript%3Ealert(1)%3C%2Fscript%3E",
    ] {
        let response = app.get(path).await;
        assert_eq!(response.status(), StatusCode::NOT_FOUND, "{path}");
        assert!(
            header(&response, "content-type").starts_with("text/html"),
            "{path}"
        );
        assert_eq!(header(&response, "cache-control"), "no-store", "{path}");
        let html = body_text(response).await;
        assert!(html.contains("<h1>There is no page here</h1>"), "{path}");
        assert!(
            !html.contains("[workspace]") && !html.contains("root:"),
            "{path}"
        );
        assert_no_injection(&html);
        assert_basics(&html, path);
        // An error page is not for a search engine.
        assert!(html.contains(r#"<meta name="robots" content="noindex">"#));
    }
    // Under /v1 the answer is still JSON.
    let (status, body) = app.json("/v1/nothing").await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    assert_eq!(body["error"]["code"], "not_found");
}

#[tokio::test]
async fn pages_load_nothing_from_anywhere_else() {
    let app = common::app().await;
    let response = app.get("/nothing").await;
    assert_eq!(
        header(&response, "content-security-policy"),
        "default-src 'none'; style-src 'self'; script-src 'self'; img-src 'self'; font-src 'self'; form-action 'self'; base-uri 'none'; frame-ancestors 'none'"
    );
    assert_eq!(header(&response, "x-content-type-options"), "nosniff");
    assert_eq!(
        header(&response, "referrer-policy"),
        "strict-origin-when-cross-origin"
    );
    assert!(response.headers().get("set-cookie").is_none());
    let html = body_text(response).await;
    // The only script is the site's own file, and nothing is inline.
    assert_eq!(html.matches("<script").count(), 1);
    assert!(html.contains(r#"<script src="/assets/site.js?v="#));
    assert!(!html.contains("style="));
    assert!(!html.contains(" onclick=") && !html.contains(" onload="));
    // The only address on another site is Wenmar Pro's, with its marker.
    let mut outside = 0;
    for link in html.split("https://").skip(1) {
        outside += 1;
        assert!(
            link.starts_with(
                "wenmarpro.com/?utm_source=wenmar-open&#38;utm_medium=referral&#38;utm_campaign=header\""
            ),
            "{:.90}",
            link
        );
    }
    assert_eq!(outside, 1, "one quiet mark in the header");
    assert!(html.contains(">by Wenmar Pro</a>"));
}

#[tokio::test]
async fn the_stylesheet_and_script_are_the_sites_own() {
    let app = common::app().await;
    let response = app.get("/assets/site.css").await;
    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(header(&response, "content-type"), "text/css; charset=utf-8");
    assert_eq!(header(&response, "cache-control"), "public, max-age=604800");
    let css = body_text(response).await;
    assert!(
        css.starts_with(":root{color-scheme:light dark;--brand:#e50914;"),
        "{:.80}",
        css
    );
    assert!(css.contains("@media (prefers-color-scheme:dark)"));
    assert!(css.contains("@media print"));
    assert!(css.contains(":focus-visible"), "keyboard focus is visible");
    assert!(css.contains("min-height:var(--touch)"), "targets are 44px");
    assert!(
        !css.contains("@import") && !css.contains("url("),
        "nothing is fetched"
    );

    let response = app.get("/assets/site.js").await;
    assert_eq!(
        header(&response, "content-type"),
        "text/javascript; charset=utf-8"
    );
    let script = body_text(response).await;
    assert!(script.contains("data-copy"));
    assert!(!script.contains("http"), "the script talks to no one");
}

#[tokio::test]
async fn something_the_visitor_already_has_is_304() {
    let app = common::app().await;
    let first = app.get("/assets/site.css").await;
    let etag = header(&first, "etag").to_owned();
    let again = app
        .send(
            Request::get("/assets/site.css")
                .header("if-none-match", &etag)
                .body(Body::empty())
                .unwrap(),
        )
        .await;
    assert_eq!(again.status(), StatusCode::NOT_MODIFIED);
    assert_eq!(body_text(again).await, "");
}

#[tokio::test]
async fn what_an_earlier_build_sent_is_not_what_the_visitor_already_has() {
    let app = common::app().await;
    // Before the tag named the build, every build of one crate version over
    // one data file had this tag, so a reworded page or a fixed stylesheet
    // was answered 304 until the next data release.
    let version = env!("CARGO_PKG_VERSION");
    let older = format!("W/\"2026.09-{version}\"");
    for path in ["/", "/about", "/assets/site.css", "/assets/site.js"] {
        let response = app
            .send(
                Request::get(path)
                    .header("if-none-match", &older)
                    .body(Body::empty())
                    .unwrap(),
            )
            .await;
        assert_eq!(response.status(), StatusCode::OK, "{path}");
        let etag = header(&response, "etag");
        assert_ne!(etag, older, "{path}");
        assert!(
            etag.starts_with(&format!("W/\"2026.09-{version}-")),
            "{path}: {etag}"
        );
    }
    // A page asks for the stylesheet and the script at an address that names
    // the build, so new markup is never paired with an old stylesheet that a
    // browser may keep for a week.
    let (_, html) = page(&app, "/").await;
    let build = open_server::BUILD_ID;
    assert_eq!(build.len(), 16, "{build}");
    assert!(
        build.bytes().all(|byte| byte.is_ascii_hexdigit()),
        "{build}"
    );
    assert!(html.contains(&format!(
        r#"<link rel="stylesheet" href="/assets/site.css?v={build}">"#
    )));
    assert!(html.contains(&format!(
        r#"<script src="/assets/site.js?v={build}" defer></script>"#
    )));
    let response = app.get("/").await;
    assert_eq!(
        header(&response, "etag"),
        format!("W/\"2026.09-{version}-{build}\"")
    );
}

#[tokio::test]
async fn json_answers_are_marked_as_not_for_indexing() {
    let app = common::app().await;
    for path in ["/v1/meta", "/v1/vin/KM8K2CAB4PU001140", "/v1/nothing"] {
        let response = app.get(path).await;
        assert_eq!(header(&response, "x-robots-tag"), "noindex", "{path}");
        assert!(response.headers().get("content-security-policy").is_none());
    }
    // The page for a missing address is unchanged by the API's own headers.
    let (status, _) = page(&app, "/nothing").await;
    assert_eq!(status, StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn only_a_public_page_is_ever_answered_304() {
    let app = common::app().await;
    let etag = header(&app.get("/").await, "etag").to_owned();
    let conditional = |path: &str| {
        Request::get(path)
            .header("if-none-match", &etag)
            .body(Body::empty())
            .unwrap()
    };
    // A public page the visitor already has.
    let again = app.send(conditional("/about")).await;
    assert_eq!(again.status(), StatusCode::NOT_MODIFIED);
    assert_eq!(header(&again, "etag"), etag);
    assert_eq!(header(&again, "cache-control"), "public, max-age=3600");
    assert_eq!(body_text(again).await, "");
    // A Markdown page keeps what makes it a copy.
    let again = app.send(conditional("/about.md")).await;
    assert_eq!(again.status(), StatusCode::NOT_MODIFIED);
    assert_eq!(header(&again, "x-robots-tag"), "noindex");
    assert!(header(&again, "link").ends_with("rel=\"canonical\""));
    // An address that is not a page is not "unchanged".
    for (path, status) in [
        ("/nothing", StatusCode::NOT_FOUND),
        ("/makes/nobody", StatusCode::NOT_FOUND),
        ("/vin/KM8K2", StatusCode::BAD_REQUEST),
        ("/vin?vin=KM8K2CAB4PU001140", StatusCode::SEE_OTHER),
    ] {
        let response = app.send(conditional(path)).await;
        assert_eq!(response.status(), status, "{path}");
        assert!(response.headers().get("etag").is_none(), "{path}");
    }
}

#[tokio::test]
async fn a_refusal_before_the_handler_is_a_page_on_a_page_address() {
    let app = common::app_with(|config| config.requests_per_minute = 1).await;
    assert_eq!(app.get("/").await.status(), StatusCode::OK);
    let refused = app.get("/").await;
    assert_eq!(refused.status(), StatusCode::TOO_MANY_REQUESTS);
    assert!(header(&refused, "content-type").starts_with("text/html"));
    assert!(refused.headers().get("retry-after").is_some());
    assert_eq!(header(&refused, "cache-control"), "no-store");
    assert_eq!(header(&refused, "x-robots-tag"), "noindex");
    assert!(refused.headers().get("content-security-policy").is_some());
    let html = body_text(refused).await;
    assert!(html.contains("<h1>Too many requests</h1>"), "{html}");
    assert_basics(&html, "/");
    // Under /v1 the refusal is still the API's JSON.
    let refused = app.get("/v1/meta").await;
    assert_eq!(refused.status(), StatusCode::TOO_MANY_REQUESTS);
    assert!(header(&refused, "content-type").starts_with("application/json"));
    assert_eq!(body_json(refused).await["error"]["code"], "rate_limited");

    // An address too long to read is a page too.
    let app = common::app().await;
    let long = format!("/vin/{}", "A".repeat(60_000));
    let refused = app.get(&long).await;
    assert_eq!(refused.status(), StatusCode::URI_TOO_LONG);
    assert!(header(&refused, "content-type").starts_with("text/html"));
    let html = body_text(refused).await;
    assert!(html.contains("<h1>That address is too long</h1>"), "{html}");
    assert!(html.len() < 10_000);
}

#[tokio::test]
async fn an_address_that_is_not_text_is_the_not_found_page() {
    let app = common::app().await;
    for path in [
        "/makes/%ff",
        "/makes/%ff.md",
        "/makes/honda/%ff/2019",
        "/makes/%ff/civic/2019",
        "/wmi/%ff",
        "/sitemaps/%ff.xml",
    ] {
        let response = app.get(path).await;
        assert_eq!(response.status(), StatusCode::NOT_FOUND, "{path}");
        assert!(
            header(&response, "content-type").starts_with("text/html"),
            "{path}"
        );
        let html = body_text(response).await;
        assert!(html.contains("<h1>There is no page here</h1>"), "{path}");
        assert!(!html.contains("Invalid"), "{path}: the framework's words");
    }
}
