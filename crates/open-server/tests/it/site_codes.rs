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

const KONA_CODE: &str = "/makes/hyundai/kona/2023/codes/P0420";

#[tokio::test]
async fn a_code_on_a_model_year_shows_its_bulletins_and_recalls() {
    let app = common::codes_app().await;
    let response = app.get(KONA_CODE).await;
    assert_eq!(response.status(), StatusCode::OK);
    // It has a bulletin to show, so it is offered to search engines.
    assert!(response.headers().get("x-robots-tag").is_none());
    let (_, html) = page(&app, KONA_CODE).await;
    common::assert_basics(&html, KONA_CODE);
    common::assert_head(&html, KONA_CODE);
    assert!(html.contains("<h1><span class=\"mono\">P0420</span> on the 2023 Hyundai Kona</h1>"));
    assert!(html.contains(
        "<link rel=\"canonical\" href=\"https://open.example/makes/hyundai/kona/2023/codes/P0420\">"
    ));
    assert!(!html.contains("<meta name=\"robots\" content=\"noindex"));
    // The description, and where the whole of it is.
    assert!(html.contains("<h2>Made-up title two</h2>"));
    assert!(html.contains("<p>Made-up meaning two.</p>"));
    assert!(html.contains("<li>Made-up check B</li>"));
    assert!(html.contains("href=\"/codes/P0420\""));
    // The bulletin that names it, then one about the same components,
    // then the recalls about them.
    let naming = html
        .find("<caption>Bulletins for this model year that name P0420</caption>")
        .unwrap();
    let about = html
        .find("<caption>Other bulletins about the same components</caption>")
        .unwrap();
    let recalls = html
        .find("<caption>Recalls about the same components</caption>")
        .unwrap();
    assert!(naming < about && about < recalls);
    let first = html.find("href=\"/bulletins/11000001\"").unwrap();
    let second = html.find("href=\"/bulletins/11000003\"").unwrap();
    assert!(naming < first && first < about && about < second && second < recalls);
    assert!(html[recalls..].contains("href=\"/recalls/tc/2023101\""));
    assert!(html[recalls..].contains("href=\"/recalls/nhtsa/23V123000\""));
    assert!(html.contains("href=\"/makes/hyundai/kona/2023/bulletins?code=P0420\""));
    assert!(html.contains("may apply to the model year"));
    assert!(html.contains("Only the manufacturer can say whether it applies to one vehicle."));
    assert!(html.contains("Report an error on this page"));
    common::assert_no_injection(&html);

    let text = common::markdown(&app, &format!("{KONA_CODE}.md")).await;
    assert!(text.starts_with("# P0420 on the 2023 Hyundai Kona\n"));
    assert!(text.contains("## P0420: Made\\-up title two"));
    assert!(text.contains("## Bulletins for this model year that name P0420"));
    assert!(text.contains("(https://open.example/bulletins/11000001)"));
    assert!(text.contains("## Recalls about the same components"));
}

#[tokio::test]
async fn a_page_with_nothing_to_show_answers_and_is_not_indexed() {
    let app = common::codes_app().await;
    for (path, words) in [
        // Described, but nothing of this vehicle is about its component.
        (
            "/makes/chevrolet/silverado/2019/codes/U0001",
            "Made-up title three",
        ),
        // Generic and not described; no bulletin names it.
        (
            "/makes/hyundai/kona/2023/codes/P0001",
            "Nobody has written a description of this code yet.",
        ),
    ] {
        let response = app.get(path).await;
        assert_eq!(response.status(), StatusCode::OK, "{path}");
        assert_eq!(header(&response, "x-robots-tag"), "noindex", "{path}");
        let (_, html) = page(&app, path).await;
        assert!(html.contains(words), "{path}");
        assert!(
            html.contains("<meta name=\"robots\" content=\"noindex"),
            "{path}"
        );
        assert!(
            html.contains("No bulletin for this model year names"),
            "{path}"
        );
        assert!(!html.contains("<caption>Recalls about the same components</caption>"));
    }
    // A manufacturer-specific code a bulletin names: no description, one
    // bulletin, and so a page worth showing.
    let path = "/makes/hyundai/kona/2023/codes/C1234";
    let response = app.get(path).await;
    assert_eq!(response.status(), StatusCode::OK);
    assert!(response.headers().get("x-robots-tag").is_none());
    let (_, html) = page(&app, path).await;
    assert!(html.contains("This code is specific to a manufacturer and is not described here."));
    assert!(html.contains("href=\"/bulletins/11000004\""));
    assert!(!html.contains("href=\"/codes/C1234\""));

    for path in [
        "/makes/hyundai/kona/2023/codes/p0420",
        "/makes/hyundai/kona/2023/codes/zebra",
        "/makes/hyundai/kona/2023/codes",
        "/makes/hyundai/kona/1899/codes/P0420",
        "/makes/hyundai/nothing/2023/codes/P0420",
        "/makes/hyundai/kona/2023/codes/P0420/more",
    ] {
        let (status, _) = page(&app, path).await;
        assert_eq!(status, StatusCode::NOT_FOUND, "{path}");
    }
    // Recalls alone: nothing can be said about a code.
    let app = common::recalls_app().await;
    let (status, _) = page(&app, KONA_CODE).await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    // Bulletins and no codes: the page is there, without a description.
    let app = common::bulletins_app().await;
    let (status, html) = page(&app, KONA_CODE).await;
    assert_eq!(status, StatusCode::OK);
    assert!(html.contains("href=\"/bulletins/11000001\""));
    assert!(html.contains("Nobody has written a description of this code yet."));
    assert!(!html.contains("href=\"/codes/P0420\""));
}
