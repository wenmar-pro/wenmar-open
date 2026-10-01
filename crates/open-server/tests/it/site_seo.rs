//! What search engines are told.

use axum::http::StatusCode;

use crate::common::{self, header, page};

/// The groups of a robots.txt: for each, its user agents and its rules.
fn groups(robots: &str) -> Vec<(Vec<&str>, Vec<&str>)> {
    let mut groups: Vec<(Vec<&str>, Vec<&str>)> = Vec::new();
    let mut in_rules = true;
    for line in robots.lines() {
        if let Some(agent) = line.strip_prefix("User-agent: ") {
            if in_rules {
                groups.push((Vec::new(), Vec::new()));
                in_rules = false;
            }
            groups.last_mut().unwrap().0.push(agent);
        } else if line.starts_with("Allow: ") || line.starts_with("Disallow: ") {
            in_rules = true;
            groups.last_mut().unwrap().1.push(line);
        }
    }
    groups
}

#[tokio::test]
async fn robots_txt_states_a_policy_for_ai_crawlers() {
    let app = common::app().await;
    let (status, robots) = page(&app, "/robots.txt").await;
    assert_eq!(status, StatusCode::OK);
    // Every line is a comment, blank, or one of four plain directives:
    // nothing experimental that a parser might choke on.
    for line in robots.lines() {
        assert!(
            line.is_empty()
                || line.starts_with("# ")
                || line == "#"
                || line.starts_with("User-agent: ")
                || line.starts_with("Allow: ")
                || line.starts_with("Disallow: ")
                || line.starts_with("Sitemap: "),
            "{line}"
        );
    }
    assert!(robots.ends_with("\nSitemap: https://open.example/sitemap.xml\n"));

    let groups = groups(&robots);
    assert_eq!(groups.len(), 3, "{robots}");
    // Everyone: everything. A search engine must be able to fetch a result
    // page to see that it is not to be indexed.
    assert_eq!(groups[0], (vec!["*"], vec!["Allow: /"]));
    // Crawlers that build training sets or AI search indexes: everything
    // but single VINs, as pages and as API answers.
    let (crawlers, rules) = &groups[1];
    assert_eq!(
        rules,
        &["Allow: /", "Disallow: /vin/", "Disallow: /v1/vin/"]
    );
    for token in [
        "GPTBot",
        "OAI-SearchBot",
        "ClaudeBot",
        "Claude-SearchBot",
        "Google-Extended",
        "PerplexityBot",
        "Applebot-Extended",
        "Amazonbot",
        "meta-externalagent",
        "CCBot",
    ] {
        assert!(crawlers.contains(&token), "{token}");
    }
    // Fetchers acting for one person who asked: everything, a decode
    // included. That request is what the service is for.
    let (fetchers, rules) = &groups[2];
    assert_eq!(rules, &["Allow: /"]);
    for token in ["Claude-User", "ChatGPT-User", "Perplexity-User"] {
        assert!(fetchers.contains(&token), "{token}");
    }
    // No agent is in two groups, where the second would be ignored.
    let mut seen = std::collections::HashSet::new();
    for (agents, _) in &groups {
        for agent in agents {
            assert!(seen.insert(agent.to_ascii_lowercase()), "{agent} twice");
        }
    }
    // What the rules mean for an address, read the way a crawler reads
    // them: the longest rule that matches wins.
    let allowed = |rules: &[&str], path: &str| {
        rules
            .iter()
            .filter_map(|rule| rule.split_once(": "))
            .filter(|(_, prefix)| path.starts_with(prefix))
            .max_by_key(|(_, prefix)| prefix.len())
            .is_none_or(|(kind, _)| kind == "Allow")
    };
    let crawler = &groups[1].1;
    for path in [
        "/",
        "/makes/honda",
        "/makes/honda/civic/2019",
        "/wmi/KM8",
        "/guides/wmi",
        "/docs",
        "/llms.txt",
        "/llms-full.txt",
        "/v1/openapi.json",
        "/v1/vehicles/years",
    ] {
        assert!(allowed(crawler, path), "a crawler is kept from {path}");
        assert!(allowed(&groups[2].1, path), "a fetcher is kept from {path}");
    }
    for path in ["/vin/KM8K2CAB4PU001140", "/v1/vin/KM8K2CAB4PU001140"] {
        assert!(!allowed(crawler, path), "a crawler may fetch {path}");
        assert!(
            allowed(&groups[0].1, path),
            "a search engine must see noindex"
        );
        assert!(
            allowed(&groups[2].1, path),
            "a person's assistant is kept out"
        );
    }
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
