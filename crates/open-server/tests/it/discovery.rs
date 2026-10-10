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
    assert!(
        text.contains("and calculators for a shop's prices, margins and Canadian invoice taxes.")
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

/// The addresses the server card is served from: the one SEP-2127 reserves
/// under the streamable-HTTP URL, and the two `.well-known` aliases.
const CARD_PATHS: [&str; 3] = [
    "/mcp/server-card",
    "/.well-known/mcp/server-card.json",
    "/.well-known/mcp",
];

/// Checks a card against the constraints of the Server Card schema itself,
/// so a change that made it invalid cannot pass for a discovery fix.
fn assert_card_is_valid(card: &Value) {
    let text = card.to_string();
    let get = |key: &str| {
        card.get(key)
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_owned()
    };
    // The schema address is pinned by a pattern in the schema.
    assert_eq!(
        get("$schema"),
        "https://static.modelcontextprotocol.io/schemas/v1/server-card.schema.json"
    );
    // `name` is reverse-DNS: one slash, and both halves are short.
    let name = get("name");
    assert_eq!(name, "com.wenmarpro/wenmar-open");
    assert_eq!(name.matches('/').count(), 1);
    // `description` and `title` are each at most 100 characters.
    let description = get("description");
    assert!(
        (1..=100).contains(&description.len()),
        "{}",
        description.len()
    );
    assert!(description.contains("VIN"));
    assert!(card.get("version").is_some());
    assert!(card.get("remotes").is_some());
    // A card carries no primitives and no capabilities: SEP-2127 excludes
    // them, and a static manifest cannot be trusted for either.
    for excluded in [
        "tools",
        "resources",
        "prompts",
        "capabilities",
        "protocolVersion",
    ] {
        assert!(!text.contains(&format!("\"{excluded}\"")), "{excluded}");
    }
}

#[tokio::test]
async fn the_server_card_is_served_where_a_client_looks_for_it() {
    let app = common::app().await;
    let mut card = Value::Null;
    for path in CARD_PATHS {
        let response = app.get(path).await;
        assert_eq!(response.status(), StatusCode::OK, "{path}");
        // The media type is the card's own, not plain JSON: a client that
        // asked for one and got the other has been answered wrongly.
        assert_eq!(
            header(&response, "content-type"),
            "application/mcp-server-card+json",
            "{path}"
        );
        // Public metadata, and cacheable for an hour as the SEP asks.
        assert_eq!(
            header(&response, "cache-control"),
            "public, max-age=3600",
            "{path}"
        );
        // Not a page for a search engine to index.
        assert_eq!(header(&response, "x-robots-tag"), "noindex", "{path}");
        // Every address answers with the same document.
        let body = body_json(response).await;
        if card.is_null() {
            card = body;
        } else {
            assert_eq!(body, card, "{path} differs");
        }
    }
    assert_card_is_valid(&card);
    // It names where to connect, and the address is this server's.
    let remote = &card["remotes"][0];
    assert_eq!(remote["type"], "streamable-http");
    assert_eq!(remote["url"], "https://open.example/mcp");
    assert_eq!(card["websiteUrl"], "https://open.example");
    // A HEAD carries the headers without the body.
    let head = app
        .send(
            Request::head("/mcp/server-card")
                .body(Body::empty())
                .unwrap(),
        )
        .await;
    assert_eq!(head.status(), StatusCode::OK);
    assert!(header(&head, "content-type").starts_with("application/mcp-server-card"));
}

#[tokio::test]
async fn the_server_card_does_not_contradict_the_running_server() {
    // SEP-2127: a card SHOULD NOT disagree with the `serverInfo` and
    // `supportedVersions` a client sees once connected.
    let app = common::app().await;
    let card = body_json(app.get("/mcp/server-card").await).await;
    let message = json!({
        "jsonrpc": "2.0", "id": 1, "method": "server/discover",
        "params": {
            "_meta": {
                "io.modelcontextprotocol/protocolVersion": "2026-07-28",
                "io.modelcontextprotocol/clientCapabilities": {}
            }
        }
    });
    let response = app
        .send(
            Request::post("/mcp")
                .header("content-type", "application/json")
                .header("accept", "application/json, text/event-stream")
                .header("mcp-protocol-version", "2026-07-28")
                .header("mcp-method", "server/discover")
                .body(Body::from(message.to_string()))
                .unwrap(),
        )
        .await;
    let status = response.status();
    let answer = body_json(response).await;
    assert_eq!(status, StatusCode::OK, "{answer}");
    let supported: Vec<&str> = card["remotes"][0]["supportedProtocolVersions"]
        .as_array()
        .unwrap()
        .iter()
        .map(|version| version.as_str().unwrap())
        .collect();
    // The card lists every version the endpoint really speaks, newest first,
    // and nothing it does not.
    let live: Vec<&str> = answer["result"]["supportedVersions"]
        .as_array()
        .unwrap()
        .iter()
        .map(|version| version.as_str().unwrap())
        .collect();
    assert_eq!(supported, live);
    assert!(supported.contains(&"2026-07-28"));
    assert!(supported.contains(&"2025-11-25"));
}

#[tokio::test]
async fn a_server_card_that_has_not_changed_is_answered_304() {
    // The SEP asks a host to send an ETag and to honour If-None-Match, so a
    // client that polls does not re-transfer an unchanged document.
    let app = common::app().await;
    let first = app.get("/mcp/server-card").await;
    let etag = header(&first, "etag").to_owned();
    assert!(!etag.is_empty());
    let response = app
        .send(
            Request::get("/mcp/server-card")
                .header("if-none-match", etag.clone())
                .body(Body::empty())
                .unwrap(),
        )
        .await;
    assert_eq!(response.status(), StatusCode::NOT_MODIFIED);
    // And an ETag a visitor does not hold gets the whole document.
    let response = app
        .send(
            Request::get("/mcp/server-card")
                .header("if-none-match", "W/\"other\"")
                .body(Body::empty())
                .unwrap(),
        )
        .await;
    assert_eq!(response.status(), StatusCode::OK);
}

#[tokio::test]
async fn the_card_is_not_invalidated_by_a_new_month_of_data() {
    // The card is static metadata: it does not depend on the data file. Its
    // validator must therefore track the card alone, or every client that
    // polls would re-fetch the whole document each month for nothing.
    let this_month = common::app_with_rows("").await;
    let next_month = common::app_with_rows(
        "UPDATE meta SET value = '2026.10' WHERE key = 'data_version';
         UPDATE meta SET value = '2026-11-01 04:00:00' WHERE key = 'built_at';",
    )
    .await;
    let one = this_month.get("/mcp/server-card").await;
    let etag = header(&one, "etag").to_owned();
    let body = body_json(one).await;
    let two = next_month.get("/mcp/server-card").await;
    let other_etag = header(&two, "etag").to_owned();
    let other_body = body_json(two).await;
    assert_eq!(body, other_body, "the card changed with the data");
    assert_eq!(other_etag, etag, "the validator tracks the data");
    // And the ETag a client holds is still honoured after the data moves on.
    let response = next_month
        .send(
            Request::get("/mcp/server-card")
                .header("if-none-match", etag)
                .body(Body::empty())
                .unwrap(),
        )
        .await;
    assert_eq!(response.status(), StatusCode::NOT_MODIFIED);
}

#[tokio::test]
async fn the_ai_catalog_points_at_the_server_card() {
    // The catalog is how a client that has only a domain finds the card.
    let app = common::app().await;
    let response = app.get("/.well-known/ai-catalog.json").await;
    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(
        header(&response, "content-type"),
        "application/ai-catalog+json"
    );
    let catalog = body_json(response).await;
    assert_eq!(catalog["specVersion"], "1.0");
    let entry = &catalog["entries"][0];
    assert_eq!(entry["type"], "application/mcp-server-card+json");
    assert_eq!(entry["url"], "https://open.example/mcp/server-card");
    // The identifier is domain-anchored, as the AI Catalog convention says.
    assert_eq!(
        entry["identifier"],
        "urn:air:open.wenmarpro.com:mcp:wenmar-open"
    );
    // What it names is really served, and is the card.
    let named = entry["url"].as_str().unwrap();
    let local = named.strip_prefix("https://open.example").unwrap();
    let served = body_json(app.get(local).await).await;
    assert_card_is_valid(&served);
    assert_eq!(served["name"], "com.wenmarpro/wenmar-open");
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
        // The card, at the address SEP-2127 reserves, and the catalog that
        // points at it: how a client finds the MCP server from the domain.
        "https://open.example/mcp/server-card",
        "https://open.example/.well-known/ai-catalog.json",
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
