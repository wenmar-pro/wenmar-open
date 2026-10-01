//! `/docs`, `/data` and `/about`.

use axum::http::StatusCode;

use crate::common::{self, assert_basics, markdown, page};

#[tokio::test]
async fn the_docs_page_states_the_limit_and_shows_examples_to_copy() {
    let app = common::app().await;
    let (status, html) = page(&app, "/docs").await;
    assert_eq!(status, StatusCode::OK);
    assert_basics(&html, "/docs");
    assert!(html.contains("<h1>API</h1>"));
    assert!(html.contains(r#"<link rel="canonical" href="https://open.example/docs">"#));
    // The abuse ceiling is stated plainly, so "no limits" is not overstated.
    assert!(html.contains("One address may make 600 requests a minute."));
    assert!(html.contains("429"));
    assert!(
        html.contains("<pre><code>curl https://open.example/v1/vin/KM8K2CAB4PU001140</code></pre>")
    );
    assert!(
        html.contains(r#"<a href="https://open.example/v1/openapi.json">OpenAPI description</a>"#)
    );
    assert!(html.contains(r#"<a href="https://open.example/mcp">MCP endpoint</a>"#));
}

#[tokio::test]
async fn the_limit_on_the_docs_page_is_the_one_the_server_enforces() {
    let app = common::app_with(|config| config.requests_per_minute = 42).await;
    let (_, html) = page(&app, "/docs").await;
    assert!(html.contains("One address may make 42 requests a minute."));
}

#[tokio::test]
async fn the_data_page_says_what_is_being_served() {
    let app = common::app().await;
    let (status, html) = page(&app, "/data").await;
    assert_eq!(status, StatusCode::OK);
    assert_basics(&html, "/data");
    assert!(
        html.contains(
            "data version 2026.09, built on 2026-10-01 04:25:57 from NHTSA&#39;s release vPICList_lite_2026_09."
        ),
        "{html}"
    );
    assert!(html.contains(
        r#"<a href="https://github.com/wenmar-pro/wenmar-open/releases">Data releases</a>"#
    ));
    assert!(html.contains("Changelog</a>"));
}

#[tokio::test]
async fn the_about_page_says_who_runs_it_and_what_it_keeps() {
    let app = common::app().await;
    let (status, html) = page(&app, "/about").await;
    assert_eq!(status, StatusCode::OK);
    assert_basics(&html, "/about");
    assert!(html.contains("built and hosted by Wenmar Pro"));
    assert!(html.contains("utm_campaign=about"));
    assert!(html.contains("There are no cookies"));
}

#[tokio::test]
async fn the_prose_pages_have_markdown_versions() {
    let app = common::app().await;
    let cases = [
        ("/docs.md", "# API\n", "## Decode a VIN\n"),
        (
            "/data.md",
            "# Data\n",
            "This site is serving data version 2026.09",
        ),
        ("/about.md", "# About\n", "## Who runs it\n"),
    ];
    for (path, start, line) in cases {
        let text = markdown(&app, path).await;
        assert!(text.starts_with(start), "{path}:\n{text}");
        assert!(text.contains(line), "{path}:\n{text}");
    }
    let docs = markdown(&app, "/docs.md").await;
    assert!(docs.contains("```\ncurl https://open.example/v1/vin/KM8K2CAB4PU001140\n```"));
    assert!(docs.contains("- [OpenAPI description](https://open.example/v1/openapi.json)"));
}

#[tokio::test]
async fn llms_txt_lists_the_markdown_pages_and_each_one_exists() {
    let app = common::app().await;
    let (_, text) = page(&app, "/llms.txt").await;
    assert!(text.contains("- [API reference](https://open.example/docs.md)"));
    assert!(text.contains("- [Makes](https://open.example/makes.md)"));
    let mut checked = 0;
    for link in text.split("](https://open.example").skip(1) {
        let path = link.split(')').next().unwrap();
        if path.ends_with(".md") {
            assert_eq!(app.get(path).await.status(), StatusCode::OK, "{path}");
            checked += 1;
        }
    }
    assert!(checked >= 5, "{checked} Markdown links checked");
}
