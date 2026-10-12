//! The rules for Canadian repair shops, `/rules/canada`, through HTTP.

use axum::http::StatusCode;
use open_server::site::rules::{DISCLAIMER, NOT_CHECKED, PROVINCES, TOPICS};

use crate::common::{self, assert_basics, assert_head, assert_targets, markdown, page};

#[tokio::test]
async fn the_index_lists_every_province_and_is_everywhere_a_page_is_listed() {
    let app = common::app().await;
    let (status, html) = page(&app, "/rules/canada").await;
    assert_eq!(status, StatusCode::OK);
    assert_basics(&html, "/rules/canada");
    assert_head(&html, "/rules/canada");
    assert_targets(&html, "/rules/canada");
    assert!(html.contains("<h1>Canadian rules for repair shops, by province</h1>"));
    assert!(html.contains(r#"<link rel="canonical" href="https://open.example/rules/canada">"#));
    assert!(html.contains(r#"<li><a href="/tools">Tools</a></li>"#));
    assert!(html.contains(r#"<a href="/tools" aria-current="page">Tools</a>"#));
    assert!(html.contains("It is not legal advice") && !html.contains("Data version"));
    assert_eq!(PROVINCES.len(), 13);
    let text = markdown(&app, "/rules/canada.md").await;
    assert!(text.starts_with("# Canadian rules for repair shops, by province\n"));
    let (_, sitemap) = page(&app, "/sitemaps/pages.xml").await;
    assert!(sitemap.contains("<loc>https://open.example/rules/canada</loc>"));
    for province in &PROVINCES {
        let path = province.path();
        assert!(
            html.contains(&format!(
                r#"<a href="https://open.example{path}">{}: "#,
                province.name
            )),
            "{path}"
        );
        // Only a province that has been checked is given to a search engine.
        assert_eq!(
            sitemap.contains(&format!("<loc>https://open.example{path}</loc>")),
            province.confirmed,
            "{path}"
        );
    }
    let (_, tools) = page(&app, "/tools").await;
    assert!(tools.contains(r#"<a href="https://open.example/rules/canada">Canadian rules for repair shops, by province</a>"#));
    let (_, llms) = page(&app, "/llms.txt").await;
    assert!(llms.contains("\n## Rules for repair shops\n\n- [Canadian rules for repair shops, by province](https://open.example/rules/canada.md): "));
    assert!(llms.contains("- [One province, such as Ontario](https://open.example/rules/canada/on.md): `/rules/canada/{code}.md`"));
    let (_, full) = page(&app, "/llms-full.txt").await;
    assert!(full.contains("\n## Canadian rules for repair shops, by province\n"));
}

#[tokio::test]
async fn each_province_has_a_page_that_shows_only_what_was_checked() {
    let app = common::app().await;
    for province in &PROVINCES {
        let path = province.path();
        let (status, html) = page(&app, &path).await;
        assert_eq!(status, StatusCode::OK, "{path}");
        assert_basics(&html, &path);
        assert_head(&html, &path);
        assert_targets(&html, &path);
        assert!(
            html.contains(&format!(
                "<h1>{}: rules for auto repair shops</h1>",
                province.name
            )),
            "{path}"
        );
        assert!(
            html.contains(r#"<li><a href="/rules/canada">Rules in Canada</a></li>"#),
            "{path}"
        );
        assert!(html.contains("It is not legal advice"), "{path}");
        for (_, heading) in TOPICS {
            assert!(
                html.contains(&format!(">{heading}</h2>")),
                "{path}: {heading}"
            );
        }
        let text = markdown(&app, &format!("{path}.md")).await;
        assert!(text.contains(DISCLAIMER), "{path}");
        // Indexed exactly when it is confirmed.
        assert_eq!(
            html.contains(r#"<meta name="robots" content="noindex">"#),
            !province.confirmed,
            "{path}"
        );
        if !province.confirmed {
            assert_eq!(
                html.matches(&format!("<p>{NOT_CHECKED}</p>")).count(),
                9,
                "{path}"
            );
            assert!(
                html.contains("Nothing on this page has been checked for"),
                "{path}"
            );
            continue;
        }
        for rule in province.rules {
            assert!(
                html.contains(&format!("Read on {}.", rule.as_of)),
                "{path}: {}",
                rule.topic
            );
            for source in rule
                .sources
                .iter()
                .filter(|source| !source.url.contains('&'))
            {
                assert!(
                    html.contains(&format!(r#"<a href="{}">"#, source.url)),
                    "{path}: {}",
                    source.url
                );
            }
        }
        assert!(!html.contains(&format!("<p>{NOT_CHECKED}</p>")), "{path}");
    }
}

#[tokio::test]
async fn an_address_that_names_no_province_is_404() {
    let app = common::app().await;
    for path in [
        "/rules",
        "/rules/canada/",
        "/rules/canada/ON",
        "/rules/canada/zz",
        "/rules/canada/on/extra",
        "/rules/canada/zz.md",
        "/rules/usa",
    ] {
        let (status, html) = page(&app, path).await;
        assert_eq!(status, StatusCode::NOT_FOUND, "{path}");
        assert!(html.contains("<h1>There is no page here</h1>"), "{path}");
    }
}
