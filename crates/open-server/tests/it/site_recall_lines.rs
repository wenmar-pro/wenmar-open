//! What the model-year page and the VIN result page say of recalls, and
//! that they say nothing on a server that holds none.

use axum::http::StatusCode;

use crate::common::{
    self, assert_basics, assert_head, assert_no_injection, assert_targets, body_text, header,
    markdown, page,
};

const NHTSA_LOOKUP: &str = "https://www.nhtsa.gov/recalls";
const TC_LOOKUP: &str =
    "https://wwwapps.tc.gc.ca/Saf-Sec-Sur/7/VRDB-BDRV/search-recherche/menu.aspx?lang=eng";

/// What marks a recall line or section on the two pages. A maker's own
/// link may end in `/recalls`, so the addresses are this site's in full.
const MARKS: [&str; 7] = [
    r#"<th scope="row">Recalls</th>"#,
    r#"href="/makes/hyundai/kona/2023/recalls"#,
    r#"href="/makes/honda/civic/2019/recalls"#,
    "/recalls.md)",
    "## Recalls",
    r#"id="recalls""#,
    "may apply",
];

#[tokio::test]
async fn the_model_year_page_says_how_many_recalls_may_apply_and_links_them() {
    let app = common::recalls_app().await;
    let (status, html) = page(&app, "/makes/hyundai/kona/2023").await;
    assert_eq!(status, StatusCode::OK);
    assert_basics(&html, "/makes/hyundai/kona/2023");
    assert_head(&html, "/makes/hyundai/kona/2023");
    assert_targets(&html, "/makes/hyundai/kona/2023");
    // One line, in the table about the vehicle, after its id.
    let line = r#"<tr><th scope="row">Recalls</th><td><a href="/makes/hyundai/kona/2023/recalls">4 recall campaigns may apply</a></td></tr>"#;
    assert_eq!(html.matches(line).count(), 1, "{html}");
    let id = html
        .find(r#"<td class="mono">2023_hyundai_kona</td>"#)
        .unwrap();
    let trims = html.find("<caption>Trims</caption>").unwrap();
    let at = html.find(line).unwrap();
    assert!(id < at && at < trims);
    assert_eq!(
        app.get("/makes/hyundai/kona/2023/recalls").await.status(),
        StatusCode::OK
    );
    // The page is still the page it was: indexed, with its own title.
    assert!(html.contains("<title>2023 Hyundai Kona trims and engines - Wenmar Open</title>"));
    assert!(!html.contains(r#"name="robots""#));

    // One campaign is one campaign, and none is said as none.
    let (_, html) = page(&app, "/makes/hyundai/kona/2022").await;
    assert!(
        html.contains(
            r#"<a href="/makes/hyundai/kona/2022/recalls">1 recall campaign may apply</a>"#
        ),
        "{html}"
    );
    let (_, html) = page(&app, "/makes/honda/civic/2019").await;
    assert!(html.contains(
        r#"<tr><th scope="row">Recalls</th><td><a href="/makes/honda/civic/2019/recalls">None found that may apply. Check the VIN to be sure.</a></td></tr>"#
    ), "{html}");
    // A trailer has the line too.
    let (_, html) = page(&app, "/makes/ranger-trailers/tilt-deck/2019").await;
    assert!(html.contains(
        r#"<a href="/makes/ranger-trailers/tilt-deck/2019/recalls">1 recall campaign may apply</a>"#
    ));
    // Names that are HTML stay text.
    let (_, html) = page(&app, "/makes/b-b-script/model-img/2019").await;
    assert_no_injection(&html);
    assert!(html.contains(
        r#"<a href="/makes/b-b-script/model-img/2019/recalls">1 recall campaign may apply</a>"#
    ));

    // The Markdown version ends with the same line.
    let text = markdown(&app, "/makes/hyundai/kona/2023.md").await;
    assert!(
        text.ends_with(
            "\n## Recalls\n\n[4 recall campaigns may apply](/makes/hyundai/kona/2023/recalls.md)\n"
        ),
        "{text}"
    );
    assert_eq!(
        app.get("/makes/hyundai/kona/2023/recalls.md")
            .await
            .status(),
        StatusCode::OK
    );
}

#[tokio::test]
async fn the_vin_page_lists_recalls_that_may_apply_to_its_model_year() {
    let app = common::recalls_app().await;
    let path = "/vin/KM8K2CAB4PU001140";
    let response = app.get(path).await;
    assert_eq!(response.status(), StatusCode::OK);
    // Still one vehicle's page: private, and not indexed.
    assert_eq!(header(&response, "x-robots-tag"), "noindex");
    assert_eq!(header(&response, "cache-control"), "private, max-age=3600");
    let html = body_text(response).await;
    assert_basics(&html, path);
    assert_head(&html, path);
    assert_targets(&html, path);
    assert!(
        html.contains(r#"<h2 id="recalls">Recalls that may apply</h2>"#),
        "{html}"
    );
    assert!(html.contains(
        "<p>4 recall campaigns may apply to the 2023 Hyundai Kona. This is a list for the model year, not a check of this VIN: only the manufacturer can say whether a recall applies to this vehicle.</p>"
    ), "{html}");
    // The three newest, of four, newest first.
    assert!(html.contains("<caption>Newest recalls for the 2023 Hyundai Kona</caption>"));
    let rows: Vec<usize> = [
        r#"<a class="mono" href="/recalls/nhtsa/24V456000">24V456000</a>"#,
        r#"<a class="mono" href="/recalls/tc/2023101">2023101</a>"#,
        r#"<a class="mono" href="/recalls/nhtsa/23V123000">23V123000</a>"#,
    ]
    .iter()
    .map(|row| {
        html.find(row)
            .unwrap_or_else(|| panic!("no {row}:\n{html}"))
    })
    .collect();
    assert!(rows[0] < rows[1] && rows[1] < rows[2]);
    assert!(
        !html.contains("22V789000"),
        "the fourth is on the full list"
    );
    assert!(html.contains(
        r#"<td>2024-05-10. SERVICE BRAKES, HYDRAULIC:ANTILOCK/TRACTION CONTROL/ELECTRONIC LIMITED SLIP. The brake control unit may fail. <strong>NHTSA advises: do not drive until the repair is done.</strong></td>"#
    ), "{html}");
    // The whole list, and where one VIN can be checked.
    for link in [
        r#"<li><a href="/makes/hyundai/kona/2023/recalls">All recalls that may apply to the 2023 Hyundai Kona</a></li>"#.to_owned(),
        format!(r#"<li><a href="{NHTSA_LOOKUP}">NHTSA recall lookup by VIN (United States)</a></li>"#),
        format!(r#"<li><a href="{TC_LOOKUP}">Transport Canada recall search (Canada)</a></li>"#),
    ] {
        assert!(html.contains(&link), "{link}\n{html}");
    }
    assert!(html.contains(
        "United States recalls are from NHTSA, as of 2026-10-11. Canadian recalls are from Transport Canada, as of 2026-10-06."
    ));
    // The section is after the sheet and before the links that end the page.
    let sheet = html.find("<caption>Where it was built</caption>").unwrap();
    let section = html.find(r#"<h2 id="recalls">"#).unwrap();
    let end = html.find("Decode another VIN").unwrap();
    assert!(sheet < section && section < end);

    // The advisories are said at the top, before the sheet, each with its
    // campaign and the lookup.
    let first = html.find(r#"<p class="warning">Recall <a href="/recalls/nhtsa/24V456000">24V456000</a> may apply to this vehicle. NHTSA advises: do not drive until the repair is done. <a href="https://www.nhtsa.gov/recalls">Check this VIN with NHTSA</a> before it is driven or parked inside.</p>"#)
        .unwrap_or_else(|| panic!("{html}"));
    let second = html.find(r#"<p class="warning">Recall <a href="/recalls/nhtsa/23V123000">23V123000</a> may apply to this vehicle. NHTSA advises: park outside, away from buildings, until the repair is done."#).unwrap();
    let actions = html.find(r#"<div class="actions">"#).unwrap();
    assert!(first < second && second < actions && actions < sheet);
    assert_eq!(html.matches(r#"<p class="warning">"#).count(), 2);
    // Nothing says the recall applies.
    let lower = html.to_lowercase();
    for claim in [
        "applies to this vin",
        "your vehicle is",
        "this vehicle is affected",
        "open recall",
    ] {
        assert!(!lower.contains(claim), "{claim:?}");
    }
    // What is copied and what is shared are as before: the sheet, and no VIN.
    assert!(html.contains(
        "data-copy-text=\"2023 Hyundai Kona SE\nVIN KM8K2CAB4PU001140\n\nVehicle\nYear: 2023\n"
    ));
    let copied = html
        .split("data-copy-text=\"")
        .nth(1)
        .unwrap()
        .split('"')
        .next()
        .unwrap();
    assert!(!copied.to_lowercase().contains("recall"), "{copied}");
    let head = html.split("</head>").next().unwrap();
    assert!(!head.contains("KM8K2CAB4PU001140") && !head.to_lowercase().contains("recall"));
    assert_eq!(html.matches("utm_campaign=vin-result").count(), 1);
    assert_eq!(html.matches(r#"class="primary""#).count(), 0);

    // A wrong check digit is warned of, and the recalls are still listed.
    let (_, html) = page(&app, "/vin/KM8K2CAB0PU001140").await;
    assert!(html.contains(r#"<h2 id="recalls">Recalls that may apply</h2>"#));
    assert_eq!(html.matches(r#"<p class="warning">"#).count(), 3);
}

#[tokio::test]
async fn a_vin_the_catalog_cannot_place_has_no_recall_section() {
    let app = common::recalls_app().await;
    for path in [
        "/vin/1M8PDMPA9SP000001",
        "/vin/KM8K2",
        "/vin/ZZZK2CAB4PU001140",
    ] {
        let (_, html) = page(&app, path).await;
        assert!(!html.contains(r#"id="recalls""#), "{path}");
        assert!(!html.contains("Recalls that may apply"), "{path}");
        assert_basics(&html, path);
    }
}

#[tokio::test]
async fn a_server_with_one_countrys_recalls_counts_what_it_holds() {
    let app = common::app_with_shop(common::RECALL_DATASETS_CA, &common::recall_rows()).await;
    let (_, html) = page(&app, "/vin/KM8K2CAB4PU001140").await;
    assert!(
        html.contains("<p>1 recall campaign may apply to the 2023 Hyundai Kona."),
        "{html}"
    );
    assert!(html.contains(
        "United States recalls are from NHTSA, not on file here. Canadian recalls are from Transport Canada, as of 2026-10-06."
    ));
    assert!(html.contains(r#"href="/recalls/tc/2023101""#));
    assert!(
        !html.contains(r#"<p class="warning">"#),
        "a Canadian campaign carries no advisory"
    );
    let (_, html) = page(&app, "/makes/hyundai/kona/2023").await;
    assert!(
        html.contains(
            r#"<a href="/makes/hyundai/kona/2023/recalls">1 recall campaign may apply</a>"#
        )
    );
}

#[tokio::test]
async fn without_recall_data_both_pages_are_as_they_were() {
    // No shop file; a shop file with no datasets; a shop file whose only
    // dataset is not recalls.
    let none = common::app().await;
    let empty = common::app_with_shop("{}", "").await;
    let other = common::app_with_shop(
        r#"{"codes":{"as_of":"2026-10-11","rows":1,"licence":"mit","attribution":""}}"#,
        "",
    )
    .await;
    for path in [
        "/makes/hyundai/kona/2023",
        "/makes/hyundai/kona/2023.md",
        "/makes/honda/civic/2019",
        "/vin/KM8K2CAB4PU001140",
        "/vin/KM8K2CAB0PU001140",
    ] {
        let (status, expected) = page(&none, path).await;
        assert_eq!(status, StatusCode::OK, "{path}");
        for mark in MARKS {
            assert!(
                !expected.contains(mark),
                "{path} has {mark:?} with no recall data"
            );
        }
        // The same bytes whether there is no shop file or one without recalls.
        for app in [&empty, &other] {
            let (status, html) = page(app, path).await;
            assert_eq!(status, StatusCode::OK, "{path}");
            assert_eq!(html, expected, "{path}");
        }
    }
    // And the pages they would have linked to do not exist.
    for app in [&none, &empty, &other] {
        let (status, _) = page(app, "/makes/hyundai/kona/2023/recalls").await;
        assert_eq!(status, StatusCode::NOT_FOUND);
    }
}
