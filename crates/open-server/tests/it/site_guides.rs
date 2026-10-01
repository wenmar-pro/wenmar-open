//! The answer pages under `/guides`.

use axum::http::StatusCode;

use crate::common::{self, assert_basics, assert_head, assert_targets, markdown, page};

const GUIDES: [(&str, &str); 5] = [
    ("how-to-read-a-vin", "How to read a VIN"),
    (
        "where-to-find-the-vin",
        "Where to find the VIN on a vehicle",
    ),
    ("wmi", "What the first three characters of a VIN mean"),
    ("model-year", "How a VIN encodes the model year"),
    ("check-digit", "What the VIN check digit is"),
];

#[tokio::test]
async fn every_guide_is_a_page_with_a_markdown_version() {
    let app = common::app().await;
    let (status, html) = page(&app, "/guides").await;
    assert_eq!(status, StatusCode::OK);
    assert_basics(&html, "/guides");
    assert_head(&html, "/guides");
    assert_targets(&html, "/guides");
    assert!(html.contains("<h1>VIN guide</h1>"));
    assert!(html.contains(r#"<a href="/guides" aria-current="page">VIN guide</a>"#));
    assert!(html.contains(r#"<link rel="canonical" href="https://open.example/guides">"#));
    let text = markdown(&app, "/guides.md").await;
    assert!(text.starts_with("# VIN guide\n"), "{text}");

    for (slug, heading) in GUIDES {
        let path = format!("/guides/{slug}");
        let (status, html) = page(&app, &path).await;
        assert_eq!(status, StatusCode::OK, "{path}");
        assert_basics(&html, &path);
        assert_head(&html, &path);
        assert_targets(&html, &path);
        assert!(html.contains(&format!("<h1>{heading}</h1>")), "{path}");
        assert!(html.contains(&format!(
            r#"<link rel="canonical" href="https://open.example{path}">"#
        )));
        assert!(!html.contains(r#"name="robots""#), "{path} is indexed");
        assert!(html.contains(r#"<meta property="og:type" content="article">"#));
        // The way back up, and where the page is in the site.
        assert!(
            html.contains(r#"<ol class="crumbs">"#)
                && html.contains(r#"<li><a href="/guides">VIN guide</a></li>"#),
            "{path}"
        );
        assert!(html.contains(r#"<a href="/guides" aria-current="page">VIN guide</a>"#));
        // The index lists it.
        let (_, index) = page(&app, "/guides").await;
        assert!(
            index.contains(&format!("https://open.example{path}\"")),
            "{path}"
        );
        // And it has a Markdown version, which is this text and no other.
        let text = markdown(&app, &format!("{path}.md")).await;
        assert!(text.starts_with(&format!("# {heading}\n")), "{path}");
        assert!(text.contains("\n## "), "{path}: sections");
    }
}

#[tokio::test]
async fn the_tables_of_a_guide_are_tables_in_both_forms() {
    let app = common::app().await;
    let (_, html) = page(&app, "/guides/model-year").await;
    assert!(html.contains("<caption>Model year by tenth character</caption>"));
    assert!(html.contains(
        r#"<thead><tr><th scope="col">Character</th><th scope="col">1980 to 2009</th><th scope="col">2010 to 2039</th></tr></thead>"#
    ));
    assert!(html.contains(r#"<tr><th scope="row">P</th><td>1993</td><td>2023</td></tr>"#));
    assert!(html.contains(r#"<tr><th scope="row">9</th><td>2009</td><td>2039</td></tr>"#));
    assert_eq!(html.matches(r#"<th scope="row">"#).count(), 30);
    let text = markdown(&app, "/guides/model-year.md").await;
    assert!(text.contains(
        "| Character | 1980 to 2009 | 2010 to 2039 |\n| --- | --- | --- |\n| A | 1980 | 2010 |\n"
    ));
    assert!(text.contains("| P | 1993 | 2023 |\n"));

    let (_, html) = page(&app, "/guides/check-digit").await;
    assert!(
        html.contains(r#"<tr><th scope="row">8</th><td>10</td></tr>"#),
        "position 8 weighs 10"
    );
    assert!(html.contains(r#"<tr><th scope="row">7</th><td>G, P, X</td></tr>"#));
    // The worked example is computed, and it is the fixture's Kona.
    assert!(html.contains("which add up to 257"), "{html}");
    assert!(html.contains("257 divided by 11 is 23 with 4 left over"));
}

#[tokio::test]
async fn every_link_in_a_guide_leads_somewhere() {
    let app = common::app().await;
    let mut paths = vec!["/guides".to_owned()];
    paths.extend(GUIDES.iter().map(|(slug, _)| format!("/guides/{slug}")));
    let mut followed = 0;
    for path in &paths {
        let (_, html) = page(&app, path).await;
        let main = html.split(r#"<main id="main">"#).nth(1).unwrap();
        let main = main.split("</main>").next().unwrap();
        for link in main.split(r#"href=""#).skip(1) {
            let address = link.split('"').next().unwrap();
            let local = address
                .strip_prefix("https://open.example")
                .unwrap_or(address);
            assert!(
                local.starts_with('/'),
                "{path}: a link off the site: {address}"
            );
            let status = app.get(local).await.status();
            assert_eq!(status, StatusCode::OK, "{path} links to {address}");
            followed += 1;
        }
    }
    assert!(followed >= 15, "{followed} links followed");
}

#[tokio::test]
async fn the_guides_are_reached_from_the_home_page_and_the_sitemap() {
    let app = common::app().await;
    let (_, home) = page(&app, "/").await;
    let (_, sitemap) = page(&app, "/sitemaps/pages.xml").await;
    assert!(sitemap.contains("<loc>https://open.example/guides</loc>"));
    for (slug, _) in GUIDES {
        assert!(
            home.contains(&format!(r#"<a href="/guides/{slug}">"#)),
            "{slug}"
        );
        assert!(
            sitemap.contains(&format!("<loc>https://open.example/guides/{slug}</loc>")),
            "{slug}"
        );
    }
    assert_targets(&home, "/");
    // Every page's header leads to the guides.
    for path in ["/", "/makes", "/docs", "/vin/KM8K2CAB4PU001140", "/nothing"] {
        let (_, html) = page(&app, path).await;
        assert!(
            html.contains(r#"<a href="/guides">VIN guide</a>"#),
            "{path}"
        );
    }
}

#[tokio::test]
async fn a_guide_that_does_not_exist_is_404() {
    let app = common::app().await;
    for path in [
        "/guides/nothing",
        "/guides/nothing.md",
        "/guides/CHECK-DIGIT",
        "/guides/check-digit/extra",
        "/guides/%ff",
        "/guides/..%2Fdocs",
        "/guides/",
    ] {
        let (status, html) = page(&app, path).await;
        assert_eq!(status, StatusCode::NOT_FOUND, "{path}");
        assert!(html.contains("<h1>There is no page here</h1>"), "{path}");
    }
}

#[tokio::test]
async fn the_prose_pages_keep_their_place_in_the_header() {
    let app = common::app().await;
    for (path, name) in [("/docs", "API"), ("/data", "Data"), ("/about", "About")] {
        let (_, html) = page(&app, path).await;
        assert!(
            html.contains(&format!(
                r#"<a href="{path}" aria-current="page">{name}</a>"#
            )),
            "{path}"
        );
        assert_targets(&html, path);
    }
}
