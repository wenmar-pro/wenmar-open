//! The trouble code pages: `/codes`, one code, and a code on a model
//! year, with their Markdown twins.

use axum::http::StatusCode;

use crate::common::{self, header, page};

#[tokio::test]
async fn the_index_lists_every_code_by_system_and_can_be_searched() {
    let app = common::codes_app().await;
    let (status, html) = page(&app, "/codes").await;
    assert_eq!(status, StatusCode::OK);
    common::assert_basics(&html, "/codes");
    common::assert_head(&html, "/codes");
    assert!(html.contains("<h1>Trouble codes</h1>"));
    assert!(html.contains("in its own words"));
    assert!(html.contains("3 codes described, as of 2026-10-12."));
    // A group for each system that has a code, in the order of the list.
    let ignition = html
        .find("<caption>Ignition and misfire</caption>")
        .unwrap();
    let emissions = html.find("<caption>Emission controls</caption>").unwrap();
    let network = html
        .find("<caption>Network and communication</caption>")
        .unwrap();
    assert!(ignition < emissions && emissions < network);
    assert!(!html.contains("<caption>Body</caption>"));
    for code in ["P0300", "P0420", "U0001"] {
        assert!(
            html.contains(&format!(
                "<a class=\"mono\" href=\"/codes/{code}\">{code}</a>"
            )),
            "{code}"
        );
    }
    assert!(html.contains("Made-up title two"));
    // The search box is a plain form.
    assert!(html.contains("<form class=\"vin\" action=\"/codes\" method=\"get\" role=\"search\">"));
    assert!(html.contains("<label for=\"code-q\">"));

    let (status, html) = page(&app, "/codes?q=title+three").await;
    assert_eq!(status, StatusCode::OK);
    assert!(html.contains("1 code found for"));
    assert!(html.contains("href=\"/codes/U0001\""));
    assert!(!html.contains("href=\"/codes/P0420\""));
    assert!(html.contains("value=\"title three\""));
    // A search is a view of the index, not a page of its own.
    assert!(
        html.contains("<meta name=\"robots\" content=\"noindex"),
        "{html}"
    );
    let (_, html) = page(&app, "/codes?q=%3Cscript%3E").await;
    common::assert_no_injection(&html);
    assert!(html.contains("No code found for"));

    let text = common::markdown(&app, "/codes.md").await;
    assert!(text.starts_with("# Trouble codes\n"));
    assert!(text.contains("## Emission controls"));
    assert!(text.contains("- [P0420](https://open.example/codes/P0420): Made\\-up title two"));
}

#[tokio::test]
async fn a_code_has_a_page_with_everything_its_record_holds() {
    let app = common::codes_app().await;
    let (status, html) = page(&app, "/codes/P0420").await;
    assert_eq!(status, StatusCode::OK);
    common::assert_basics(&html, "/codes/P0420");
    common::assert_head(&html, "/codes/P0420");
    assert!(html.contains("<h1><span class=\"mono\">P0420</span> Made-up title two</h1>"));
    assert!(html.contains("<p>Made-up meaning two.</p>"));
    assert!(html.contains("<li>Made-up cause B</li>"));
    assert!(html.contains("<li>Made-up cause C</li>"));
    assert!(html.contains("<li>Made-up check B</li>"));
    assert!(html.contains("Catalytic converter, Electrical system"));
    assert!(html.contains("<a class=\"mono\" href=\"/codes/P0300\">P0300</a>"));
    assert!(html.contains("Emission controls"));
    assert!(html.contains("Reviewed by test-reviewer on 2026-10-12."));
    assert!(html.contains("Diagnose before replacing anything."));
    assert!(html.contains("Report an error on this page"));
    // A defined term, in a set of them.
    let described = common::json_ld(&html).unwrap();
    let term = described["@graph"]
        .as_array()
        .unwrap()
        .iter()
        .find(|thing| thing["@type"] == "DefinedTerm")
        .unwrap();
    assert_eq!(term["termCode"], "P0420");
    assert_eq!(term["name"], "P0420");
    assert_eq!(term["description"], "Made-up title two");
    assert_eq!(term["url"], "https://open.example/codes/P0420");
    assert_eq!(term["inDefinedTermSet"]["name"], "OBD-II trouble codes");
    assert_eq!(
        term["inDefinedTermSet"]["url"],
        "https://open.example/codes"
    );

    let text = common::markdown(&app, "/codes/P0420.md").await;
    assert!(text.starts_with("# P0420: Made\\-up title two\n"));
    assert!(text.contains("## Common causes\n\n- Made\\-up cause B\n- Made\\-up cause C\n"));
    assert!(text.contains("## Check first\n\n- Made\\-up check B\n"));
    assert!(text.contains("[P0300](https://open.example/codes/P0300)"));
}

#[tokio::test]
async fn a_code_has_one_address_and_only_where_it_is_described() {
    let app = common::codes_app().await;
    for path in [
        "/codes/p0420",
        "/codes/P0420:00",
        "/codes/P042000",
        "/codes/P0001",
        "/codes/P1234",
        "/codes/zebra",
        "/codes/P0001.md",
    ] {
        let (status, _) = page(&app, path).await;
        assert_eq!(status, StatusCode::NOT_FOUND, "{path}");
    }
    // A server without the codes has neither page.
    let app = common::bulletins_app().await;
    for path in ["/codes", "/codes.md", "/codes/P0420", "/codes/P0420.md"] {
        let response = app.get(path).await;
        assert_eq!(response.status(), StatusCode::NOT_FOUND, "{path}");
        assert_eq!(
            header(&response, "content-type"),
            "text/html; charset=utf-8"
        );
    }
}
