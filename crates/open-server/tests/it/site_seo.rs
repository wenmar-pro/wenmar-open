//! What search engines are told.

use axum::http::StatusCode;

use crate::common::{self, header, page};

#[tokio::test]
async fn robots_txt_allows_everything_and_names_the_sitemap() {
    let app = common::app().await;
    let (status, robots) = page(&app, "/robots.txt").await;
    assert_eq!(status, StatusCode::OK);
    // Result pages are kept out of search engines by `noindex`, which a
    // crawler can only see if it is allowed to fetch the page.
    assert_eq!(
        robots,
        "User-agent: *\nAllow: /\n\nSitemap: https://open.example/sitemap.xml\n"
    );
}

#[tokio::test]
async fn sitemaps_list_reference_pages_and_never_a_vin() {
    let app = common::app().await;
    let response = app.get("/sitemap.xml").await;
    assert_eq!(
        header(&response, "content-type"),
        "application/xml; charset=utf-8"
    );
    let index = common::body_text(response).await;
    for name in ["pages", "makes", "wmi", "models-2023", "models-2018"] {
        assert!(
            index.contains(&format!(
                "<loc>https://open.example/sitemaps/{name}.xml</loc>"
            )),
            "{name}:\n{index}"
        );
    }
    assert!(
        !index.contains("models-2021"),
        "no model was built that year"
    );

    let (_, pages) = page(&app, "/sitemaps/pages.xml").await;
    assert!(
        pages.contains("<url><loc>https://open.example/</loc><lastmod>2026-10-01</lastmod></url>")
    );
    assert!(pages.contains("<loc>https://open.example/docs</loc>"));
    let (_, makes) = page(&app, "/sitemaps/makes.xml").await;
    assert!(makes.contains("<loc>https://open.example/makes/honda</loc>"));
    assert!(!makes.contains("ranger-trailers"));
    let (_, models) = page(&app, "/sitemaps/models-2019.xml").await;
    assert!(models.contains("<loc>https://open.example/makes/honda/civic/2019</loc>"));
    assert!(models.contains("<loc>https://open.example/makes/ford/f-150/2019</loc>"));
    assert!(!models.contains("tilt-deck"), "trailers are not listed");
    let (_, codes) = page(&app, "/sitemaps/wmi.xml").await;
    assert!(codes.contains("<loc>https://open.example/wmi/1A9881</loc>"));

    for sitemap in [&index, &pages, &makes, &models, &codes] {
        assert!(sitemap.starts_with("<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n"));
        assert!(!sitemap.contains("/vin/"), "a result page is never listed");
        // Nothing from the data file that could break the XML.
        assert!(
            !sitemap.contains('&') && !sitemap.contains("<script"),
            "{sitemap}"
        );
    }
}

#[tokio::test]
async fn every_address_in_a_sitemap_is_a_page_that_may_be_indexed() {
    let app = common::app().await;
    for sitemap in ["pages", "makes", "wmi", "models-2019", "models-2023"] {
        let (_, xml) = page(&app, &format!("/sitemaps/{sitemap}.xml")).await;
        let mut checked = 0;
        for entry in xml.split("<loc>https://open.example").skip(1) {
            let path = entry.split("</loc>").next().unwrap();
            let (status, html) = page(&app, path).await;
            assert_eq!(status, StatusCode::OK, "{sitemap}: {path}");
            assert!(
                !html.contains(r#"<meta name="robots" content="noindex">"#),
                "{sitemap}: {path} is listed but not indexed"
            );
            checked += 1;
        }
        assert!(checked > 0, "{sitemap} is empty");
    }
}

#[tokio::test]
async fn a_sitemap_that_does_not_exist_is_404() {
    let app = common::app().await;
    for path in [
        "/sitemaps/models-1066.xml",
        "/sitemaps/models-x.xml",
        "/sitemaps/models-.xml",
        "/sitemaps/..%2Frobots.txt",
        "/sitemaps/pages",
        "/sitemaps/nothing.xml",
        "/sitemaps/models-2019.xml.xml",
    ] {
        let (status, _) = page(&app, path).await;
        assert_eq!(status, StatusCode::NOT_FOUND, "{path}");
    }
}
