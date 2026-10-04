//! How a program or an AI agent finds the service and learns to use it:
//! `llms.txt`, `llms-full.txt`, the API catalog, the docs, the registry file.

use axum::body::Body;
use axum::http::{Request, StatusCode};
use serde_json::{Value, json};

use crate::common::{self, body_json, body_text, header, page};

#[tokio::test]
async fn llms_txt_follows_the_convention() {
    let app = common::app_with(|config| config.requests_per_minute = 42).await;
    let response = app.get("/llms.txt").await;
    assert!(header(&response, "content-type").starts_with("text/plain"));
    let text = body_text(response).await;
    // One H1, then a blockquote.
    assert!(text.starts_with("# Wenmar Open\n\n> "), "{text}");
    assert_eq!(
        text.lines().filter(|line| line.starts_with("# ")).count(),
        1
    );
    // The limit is the one the server enforces.
    assert!(
        text.contains("One address may make 42 requests a minute"),
        "{text}"
    );
    // The one request an assistant needs, said before any list.
    let prose = text.split("\n## ").next().unwrap();
    assert!(prose.contains("fetch `https://open.example/v1/vin/` followed by the VIN"));
    assert!(prose.contains("`https://open.example/mcp`"));
    // H2 sections in order, the skippable one last.
    let sections: Vec<&str> = text
        .lines()
        .filter_map(|line| line.strip_prefix("## "))
        .collect();
    assert_eq!(
        sections,
        [
            "API",
            "Docs",
            "Shop calculators",
            "Reference pages",
            "Optional"
        ]
    );
    // Under an H2, every line is a list item that is a link, with optional
    // notes after a colon. Nothing else.
    let lists = text.split_once("\n## ").unwrap().1;
    let mut links = 0;
    for line in lists.lines() {
        if line.is_empty() || line.starts_with("## ") || !line.contains(' ') {
            continue;
        }
        assert!(line.starts_with("- ["), "not a link: {line}");
        let address = line.split("](").nth(1).unwrap().split(')').next().unwrap();
        assert!(address.starts_with("https://open.example/"), "{line}");
        links += 1;
    }
    assert!(links >= 20, "{links} links");
    // No page for one VIN is listed.
    assert!(!text.contains("open.example/vin/"), "{text}");
    // Every guide is listed, as Markdown.
    for slug in [
        "how-to-read-a-vin",
        "where-to-find-the-vin",
        "wmi",
        "model-year",
        "check-digit",
    ] {
        assert!(
            text.contains(&format!("](https://open.example/guides/{slug}.md)")),
            "{slug}"
        );
    }
    assert!(text.contains("- [Everything in one file](https://open.example/llms-full.txt)"));
}

#[tokio::test]
async fn llms_full_txt_is_the_pages_themselves() {
    let app = common::app().await;
    let response = app.get("/llms-full.txt").await;
    assert_eq!(response.status(), StatusCode::OK);
    assert!(header(&response, "content-type").starts_with("text/plain"));
    assert_eq!(header(&response, "x-robots-tag"), "noindex");
    let full = body_text(response).await;
    assert!(full.starts_with("# Wenmar Open\n\n> "), "{:.200}", full);
    assert_eq!(
        full.lines().filter(|line| line.starts_with("# ")).count(),
        1
    );
    assert!(!full.contains("open.example/vin/"));
    // Each page's title is a section, and its sections are one level down.
    assert!(full.contains("\n## API\n"));
    assert!(full.contains("\n### Decode a VIN\n"));
    assert!(full.contains("\n## How a VIN encodes the model year\n"));
    assert!(full.contains("\n### The chart\n"));
    // Every line of every page's Markdown is in it, word for word, so it
    // cannot say something a page does not.
    let mut checked = 0;
    for path in [
        "/docs.md",
        "/guides/how-to-read-a-vin.md",
        "/guides/where-to-find-the-vin.md",
        "/guides/wmi.md",
        "/guides/model-year.md",
        "/guides/check-digit.md",
        "/tools/parts-matrix.md",
        "/tools/canada-invoice-tax.md",
        "/tools/gross-profit.md",
        "/tools/labor-rate.md",
        "/data.md",
        "/about.md",
    ] {
        let (status, text) = page(&app, path).await;
        assert_eq!(status, StatusCode::OK, "{path}");
        for line in text.lines() {
            if line.is_empty() || line.starts_with('#') {
                continue;
            }
            assert!(full.contains(line), "{path}: missing: {line}");
            checked += 1;
        }
    }
    assert!(checked > 100, "{checked} lines checked");
}

#[tokio::test]
async fn the_api_is_announced_where_a_program_looks() {
    let app = common::app().await;
    for request in [
        Request::get("/.well-known/api-catalog"),
        Request::head("/.well-known/api-catalog"),
    ] {
        let response = app.send(request.body(Body::empty()).unwrap()).await;
        assert_eq!(response.status(), StatusCode::OK);
        assert_eq!(
            header(&response, "content-type"),
            "application/linkset+json; profile=\"https://www.rfc-editor.org/info/rfc9727\""
        );
        assert_eq!(
            header(&response, "link"),
            "<https://open.example/.well-known/api-catalog>; rel=\"api-catalog\""
        );
    }
    let catalog = body_json(app.get("/.well-known/api-catalog").await).await;
    assert_eq!(
        catalog,
        json!({
            "linkset": [{
                "anchor": "https://open.example/v1",
                "service-desc": [
                    { "href": "https://open.example/v1/openapi.json", "type": "application/json" }
                ],
                "service-doc": [
                    { "href": "https://open.example/docs", "type": "text/html" }
                ]
            }]
        })
    );
    // What it points to is there.
    for path in ["/v1/openapi.json", "/docs"] {
        assert_eq!(app.get(path).await.status(), StatusCode::OK, "{path}");
    }
    // Nothing else is served from that directory.
    let (status, html) = page(&app, "/.well-known/nothing").await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    assert!(html.contains("<h1>There is no page here</h1>"));
}

#[tokio::test]
async fn the_docs_page_tells_an_agent_how_to_use_the_service() {
    let app = common::app().await;
    let (_, html) = page(&app, "/docs").await;
    assert!(
        html.contains(r#"<h2 id="for-ai-agents">For AI agents</h2>"#),
        "{html}"
    );
    let section = html.split(r#"<h2 id="for-ai-agents">"#).nth(1).unwrap();
    // One request to copy.
    assert!(
        section
            .contains("<pre><code>curl https://open.example/v1/vin/KM8K2CAB4PU001140</code></pre>")
    );
    // The MCP address is text to copy, not a link that answers 405.
    assert!(section.contains("<pre><code>https://open.example/mcp</code></pre>"));
    assert!(!html.contains(r#"href="https://open.example/mcp""#));
    for address in [
        "https://open.example/v1/openapi.json",
        "https://open.example/llms.txt",
        "https://open.example/llms-full.txt",
        "https://open.example/.well-known/api-catalog",
    ] {
        assert!(
            section.contains(&format!(r#"<a href="{address}">"#)),
            "{address}"
        );
        let local = address.strip_prefix("https://open.example").unwrap();
        assert_eq!(app.get(local).await.status(), StatusCode::OK, "{address}");
    }
    // Every page's footer leads there.
    let (_, home) = page(&app, "/").await;
    assert!(home.contains(r##"<a href="/docs#for-ai-agents">how to use the API</a>"##));
}

#[test]
fn the_registry_file_describes_the_hosted_server() {
    let file: Value = serde_json::from_str(include_str!("../../../../server.json")).unwrap();
    assert_eq!(
        file["$schema"],
        "https://static.modelcontextprotocol.io/schemas/2025-12-11/server.schema.json"
    );
    // A name under the owner's domain: reverse DNS, a slash, a name.
    let name = file["name"].as_str().unwrap();
    assert_eq!(name, "com.wenmarpro/wenmar-open");
    let description = file["description"].as_str().unwrap();
    assert!(
        (1..=100).contains(&description.len()),
        "{}",
        description.len()
    );
    assert_eq!(file["version"], env!("CARGO_PKG_VERSION"));
    assert_eq!(file["websiteUrl"], "https://open.wenmarpro.com");
    assert_eq!(
        file["remotes"],
        json!([{ "type": "streamable-http", "url": "https://open.wenmarpro.com/mcp" }])
    );
    // It is a remote server only: nothing to install.
    assert!(file.get("packages").is_none());
}
