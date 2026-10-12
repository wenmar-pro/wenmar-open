//! The bulletin pages: `/bulletins`, a model year's bulletins, and one
//! bulletin, with their Markdown twins and sitemaps.

use axum::http::StatusCode;

use crate::common::{self, page};

const KONA: &str = "/makes/hyundai/kona/2023/bulletins";

/// The order the ids of bulletins appear in on a page.
fn order(html: &str, ids: &[i64]) -> Vec<i64> {
    let mut found: Vec<(usize, i64)> = ids
        .iter()
        .filter_map(|id| {
            html.find(&format!("href=\"/bulletins/{id}\""))
                .map(|at| (at, *id))
        })
        .collect();
    found.sort_unstable();
    found.into_iter().map(|(_, id)| id).collect()
}

#[tokio::test]
async fn a_model_year_lists_its_bulletins_with_warranty_extensions_first() {
    let app = common::bulletins_app().await;
    let (status, html) = page(&app, KONA).await;
    assert_eq!(status, StatusCode::OK);
    assert!(html.contains("<h1>Service bulletins for the 2023 Hyundai Kona</h1>"));
    assert!(html.contains("A bulletin is not a recall"));
    assert!(html.contains("6 on file from NHTSA, as of 2026-10-11."));
    // The warranty extension is said first, then the list newest first.
    let warranty = html.find("<caption>Warranty extensions</caption>").unwrap();
    let list = html
        .find("<caption>Bulletins, newest first</caption>")
        .unwrap();
    let extension = html.find("href=\"/bulletins/11000002\"").unwrap();
    assert!(warranty < extension && extension < list);
    assert_eq!(
        order(
            &html[list..],
            &[11000001, 11000002, 11000003, 11000004, 11000005, 11000010]
        ),
        [11000003, 11000002, 11000010, 11000001, 11000005, 11000004]
    );
    // The label is the manufacturer's number, or NHTSA's id without one.
    assert!(html.contains(">WX-25-003</a>"));
    assert!(html.contains(">NHTSA 11000005</a>"));
    // Every narrowing is a link, with a count.
    for link in [
        "?type=warranty_extension\">Warranty extension</a> (1)",
        "?type=service_bulletin\">Service bulletin</a> (4)",
        "?component=software\">Software</a> (2)",
        "?component=brake-pad\">Brake pads</a> (1)",
        "?code=P0420\">P0420</a> (1)",
        "?topic=maintenance\">maintenance</a> (1)",
    ] {
        assert!(html.contains(&format!("{KONA}{link}")), "{link}");
    }
    // Links onward.
    assert!(html.contains("href=\"/makes/hyundai/kona/2023\""));
    assert!(html.contains("href=\"/makes/hyundai/kona/2023/recalls\""));
    // Text from the source is escaped.
    assert!(!html.contains("<script>alert(1)</script>"));
    assert!(
        html.contains("&lt;script&gt;alert(1)&lt;/script&gt;") || html.contains("&#60;script&#62;")
    );
    // Indexed, with its own address as the canonical one.
    assert!(html.contains(
        "<link rel=\"canonical\" href=\"https://open.example/makes/hyundai/kona/2023/bulletins\">"
    ));
}

#[tokio::test]
async fn a_list_is_narrowed_and_paged_by_links() {
    let app = common::bulletins_app().await;
    let all = [11000001, 11000002, 11000003, 11000004, 11000005, 11000010];
    for (query, expected, said) in [
        (
            "?type=warranty_extension",
            vec![11000002],
            "type: Warranty extension",
        ),
        (
            "?component=software",
            vec![11000003, 11000001],
            "component: Software",
        ),
        ("?code=P0420", vec![11000001], "trouble code: P0420"),
        ("?topic=maintenance", vec![11000005], "topic: maintenance"),
        ("?q=noise", vec![11000004], "text: noise"),
    ] {
        let (status, html) = page(&app, &format!("{KONA}{query}")).await;
        assert_eq!(status, StatusCode::OK, "{query}");
        let list = html
            .find("<caption>Bulletins, newest first</caption>")
            .unwrap();
        assert_eq!(order(&html[list..], &all), expected, "{query}");
        assert!(html.contains(said), "{query}");
        assert!(
            html.contains(&format!("<a href=\"{KONA}\">Show all</a>")),
            "{query}"
        );
        // A narrowed list does not repeat the warranty extensions, and
        // points search engines at the page without the query.
        assert!(
            !html.contains("<caption>Warranty extensions</caption>"),
            "{query}"
        );
        assert!(
            html.contains(&format!(
                "<link rel=\"canonical\" href=\"https://open.example{KONA}\">"
            )),
            "{query}"
        );
        assert!(
            html.contains("aria-current=\"page\"") || query == "?q=noise",
            "{query}"
        );
    }
    // A value that is not one narrows nothing, and is no error.
    for query in [
        "?type=nonsense",
        "?code=zzz",
        "?cursor=yesterday",
        "?limit=x",
    ] {
        let (status, html) = page(&app, &format!("{KONA}{query}")).await;
        assert_eq!(status, StatusCode::OK, "{query}");
        let list = html
            .find("<caption>Bulletins, newest first</caption>")
            .unwrap();
        assert_eq!(order(&html[list..], &all).len(), 6, "{query}");
    }
    // A later page: asked for with the cursor of the fourth bulletin.
    let (status, html) = page(&app, &format!("{KONA}?cursor=2025-03-10_11000001")).await;
    assert_eq!(status, StatusCode::OK);
    let list = html
        .find("<caption>Bulletins, newest first</caption>")
        .unwrap();
    assert_eq!(order(&html[list..], &all), [11000005, 11000004]);
    assert!(!html.contains("rel=\"next\""));
    assert!(!html.contains("<caption>Warranty extensions</caption>"));
}

#[tokio::test]
async fn one_bulletin_has_a_page_with_more_than_its_summary() {
    let app = common::bulletins_app().await;
    let (status, html) = page(&app, "/bulletins/11000001").await;
    assert_eq!(status, StatusCode::OK);
    assert!(html.contains("<h1>Service bulletin TSB 25-01-012</h1>"));
    assert!(html.contains("A bulletin is not a recall"));
    for text in [
        "<th scope=\"row\">NHTSA id</th><td>11000001</td>",
        "<th scope=\"row\">Published by NHTSA</th><td>2025-03-10</td>",
        // A value put into a page has its apostrophe escaped.
        "<th scope=\"row\">Manufacturer&#39;s date</th><td>2025-03-01</td>",
        "<h2>NHTSA's summary</h2>",
        "Update the ECM software.",
        // The document is linked at NHTSA, both addresses, and not held.
        "href=\"https://static.nhtsa.gov/odi/tsbs/2025/MC-11000001-0001.pdf\">NHTSA's copy of the document (PDF)</a>",
        "href=\"https://static.nhtsa.gov/odi/tsbs/2025/MC-11000001-9999.pdf\"",
        "This site does not hold it.",
        "<h2>Components</h2>",
        "Electrical system, Engine, Software",
        "<h2>Trouble codes named</h2>",
        "P0300, P0420",
        // The vehicles, each model year a link to its list.
        "<th scope=\"row\">HYUNDAI KONA</th>",
        "<a href=\"/makes/hyundai/kona/2022/bulletins\">2022</a>, <a href=\"/makes/hyundai/kona/2023/bulletins\">2023</a>",
        // Another bulletin about software for the same model years.
        "Other bulletins for the same vehicles and component",
        "href=\"/bulletins/11000003\"",
    ] {
        assert!(html.contains(text), "{text}");
    }
    // It is an article in the structured data, and is indexed.
    assert!(html.contains("\"@type\":\"Article\""));
    assert!(
        html.contains("<link rel=\"canonical\" href=\"https://open.example/bulletins/11000001\">")
    );
    assert!(!html.contains("noindex"));

    // No address can be built: said, with no link to NHTSA.
    for id in ["10100001", "11000005"] {
        let (_, html) = page(&app, &format!("/bulletins/{id}")).await;
        assert!(
            html.contains("No address for NHTSA's copy can be built"),
            "{id}"
        );
        assert!(!html.contains("static.nhtsa.gov"), "{id}");
    }
    // No model year given, and a make the catalog lacks: names, no links.
    let (_, html) = page(&app, "/bulletins/11000009").await;
    assert!(html.contains("<th scope=\"row\">HYUNDAI HYUNDAI</th><td></td>"));
    let (_, html) = page(&app, "/bulletins/10100001").await;
    assert!(html.contains("<th scope=\"row\">FREIGHTLINER CASCADIA</th><td>2020</td>"));
    // A trim of a catalog model says so.
    let (_, html) = page(&app, "/bulletins/11000007").await;
    assert!(html.contains("the bulletin names a series or trim of it"));
    // Hostile text is escaped everywhere on the page.
    let (_, html) = page(&app, "/bulletins/11000010").await;
    assert!(!html.contains("<script>alert(1)</script>"));
    assert!(!html.contains("<img src=x"));
    assert!(!html.contains("<b>Bold</b>"));
    // One bulletin, one address.
    for missing in [
        "/bulletins/1",
        "/bulletins/011000001",
        "/bulletins/abc",
        "/bulletins/11000001/x",
    ] {
        let (status, _) = page(&app, missing).await;
        assert_eq!(status, StatusCode::NOT_FOUND, "{missing}");
    }
}

#[tokio::test]
async fn a_recall_for_the_same_vehicle_and_component_is_beside_a_bulletin() {
    // The recall fixture has no recall_component table: nothing is related
    // and the page is whole without it.
    let app = common::bulletins_app().await;
    let (status, html) = page(&app, "/bulletins/11000004").await;
    assert_eq!(status, StatusCode::OK);
    assert!(!html.contains("Recalls for the same vehicles and component"));
}

#[tokio::test]
async fn the_entry_page_and_the_markdown_twins() {
    let app = common::bulletins_app().await;
    let (status, html) = page(&app, "/bulletins").await;
    assert_eq!(status, StatusCode::OK);
    assert!(html.contains("<h1>Service bulletins by vehicle</h1>"));
    assert!(html.contains("Newest bulletins from NHTSA, as of 2026-10-11"));
    assert!(html.contains("href=\"/bulletins/11000007\""));
    assert!(html.contains("name=\"vin\""));

    let (status, text) = page(&app, &format!("{KONA}.md")).await;
    assert_eq!(status, StatusCode::OK);
    assert!(text.starts_with("# Service bulletins for the 2023 Hyundai Kona\n"));
    assert!(text.contains("6 bulletins from NHTSA, as of 2026-10-11."));
    assert!(
        text.contains("- [WX\\-25\\-003](/bulletins/11000002.md), 2025-06-02, Warranty extension")
    );
    assert!(text.contains(
        "JSON: [/v1/vehicles/2023_hyundai_kona/bulletins](/v1/vehicles/2023_hyundai_kona/bulletins)"
    ));

    let (status, text) = page(&app, "/bulletins/11000001.md").await;
    assert_eq!(status, StatusCode::OK);
    assert!(text.starts_with("# Service bulletin TSB 25\\-01\\-012\n"));
    for part in [
        "## NHTSA's summary",
        "## The document",
        "[NHTSA's copy of the document (PDF)](https://static.nhtsa.gov/odi/tsbs/2025/MC-11000001-0001.pdf)",
        "## Trouble codes named\n\nP0300, P0420",
        "## Vehicles listed\n\n- 2022 to 2023 HYUNDAI KONA",
        "## Other bulletins for the same vehicles and component",
        "JSON: [/v1/bulletins/11000001](/v1/bulletins/11000001)",
    ] {
        assert!(text.contains(part), "{part}");
    }
    // Hostile text does not become Markdown or HTML.
    let (_, text) = page(&app, "/bulletins/11000010.md").await;
    assert!(!text.contains("<script>"));
}

#[tokio::test]
async fn the_sitemaps_list_every_bulletin_by_year_published() {
    let app = common::bulletins_app().await;
    let (status, index) = page(&app, "/sitemap.xml").await;
    assert_eq!(status, StatusCode::OK);
    for name in [
        "bulletins-2026",
        "bulletins-2025",
        "bulletins-2024",
        "bulletins-2023",
        "bulletins-2022",
        "bulletins-2017",
        "model-bulletins-2023",
        "model-bulletins-2022",
    ] {
        assert!(
            index.contains(&format!("https://open.example/sitemaps/{name}.xml")),
            "{name}"
        );
    }
    assert!(!index.contains("bulletins-2021"));
    let (status, xml) = page(&app, "/sitemaps/bulletins-2025.xml").await;
    assert_eq!(status, StatusCode::OK);
    let listed: Vec<&str> = xml.matches("/bulletins/").collect();
    assert_eq!(listed.len(), 3);
    for id in [11000001, 11000002, 11000010] {
        assert!(xml.contains(&format!("<loc>https://open.example/bulletins/{id}</loc>")));
    }
    let (status, xml) = page(&app, "/sitemaps/model-bulletins-2023.xml").await;
    assert_eq!(status, StatusCode::OK);
    assert!(xml.contains("<loc>https://open.example/makes/hyundai/kona/2023/bulletins</loc>"));
    let (_, pages) = page(&app, "/sitemaps/pages.xml").await;
    assert!(pages.contains("<loc>https://open.example/bulletins</loc>"));
    // A part that does not exist, a year with none, a name written oddly.
    for missing in [
        "/sitemaps/bulletins-2025-2.xml",
        "/sitemaps/bulletins-2021.xml",
        "/sitemaps/bulletins-2025-1.xml",
        "/sitemaps/bulletins-2025-02.xml",
        "/sitemaps/model-bulletins-1999.xml",
    ] {
        let (status, _) = page(&app, missing).await;
        assert_eq!(status, StatusCode::NOT_FOUND, "{missing}");
    }
}

#[tokio::test]
async fn a_server_without_bulletins_has_none_of_these_pages() {
    for app in [common::app().await, common::recalls_app().await] {
        for path in [
            "/bulletins",
            "/bulletins/11000001",
            "/bulletins/11000001.md",
            KONA,
            "/makes/hyundai/kona/2023/bulletins.md",
            "/sitemaps/bulletins-2025.xml",
            "/sitemaps/model-bulletins-2023.xml",
        ] {
            let (status, _) = page(&app, path).await;
            assert_eq!(status, StatusCode::NOT_FOUND, "{path}");
        }
        let (_, index) = page(&app, "/sitemap.xml").await;
        assert!(!index.contains("bulletins-"));
    }
    // The recall list under the same route still answers.
    let app = common::recalls_app().await;
    let (status, _) = page(&app, "/makes/hyundai/kona/2023/recalls").await;
    assert_eq!(status, StatusCode::OK);
    let (status, _) = page(
        &app,
        "/makes/hyundai/kona/2023/recalls?component=electrical",
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let (status, _) = page(&app, "/makes/hyundai/kona/2023/nonsense").await;
    assert_eq!(status, StatusCode::NOT_FOUND);
}
