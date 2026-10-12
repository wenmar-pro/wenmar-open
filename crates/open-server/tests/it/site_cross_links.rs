//! Recalls and bulletins that point at each other, and sibling vehicles.

use axum::http::StatusCode;

use crate::common::{self, page};

#[tokio::test]
async fn a_campaign_lists_the_bulletins_for_the_same_vehicles_and_component() {
    let app = common::cross_links_app().await;
    // 24V456000 is about brakes for the 2023 Kona; so is bulletin 11000004.
    let (status, html) = page(&app, "/recalls/nhtsa/24V456000").await;
    assert_eq!(status, StatusCode::OK);
    let table = html
        .find("<caption>Bulletins for the same vehicles and component</caption>")
        .unwrap();
    assert!(html[table..].contains("href=\"/bulletins/11000004\""));
    assert!(!html[table..].contains("href=\"/bulletins/11000002\""));
    assert!(html.contains(
        "A bulletin is the manufacturer's note to its dealers. It is not part of this recall."
    ));
    // 23V123000 is about the electrical system: the two software bulletins.
    let (_, html) = page(&app, "/recalls/nhtsa/23V123000").await;
    assert!(html.contains("href=\"/bulletins/11000003\""));
    assert!(html.contains("href=\"/bulletins/11000001\""));
    let (_, text) = page(&app, "/recalls/nhtsa/24V456000.md").await;
    assert!(text.contains("## Bulletins for the same vehicles and component"));
    assert!(text.contains("(/bulletins/11000004.md)"));
    // A campaign with nothing beside it has no such table.
    let (_, html) = page(&app, "/recalls/nhtsa/19V500000").await;
    assert!(!html.contains("Bulletins for the same vehicles and component"));
    // And on a server without the component tables, no campaign has one.
    let (_, html) = page(&common::bulletins_app().await, "/recalls/nhtsa/24V456000").await;
    assert!(!html.contains("Bulletins for the same vehicles and component"));
    let (status, _) = page(&common::recalls_app().await, "/recalls/nhtsa/24V456000").await;
    assert_eq!(status, StatusCode::OK);
}

#[tokio::test]
async fn the_two_lists_of_a_model_year_link_each_other() {
    let app = common::cross_links_app().await;
    let (_, html) = page(&app, "/makes/hyundai/kona/2023/recalls").await;
    assert!(html.contains(
        "<a href=\"/makes/hyundai/kona/2023/bulletins\">Service bulletins for the 2023 Hyundai Kona</a>"
    ));
    let (_, html) = page(&app, "/makes/hyundai/kona/2023/bulletins").await;
    assert!(html.contains("href=\"/makes/hyundai/kona/2023/recalls\""));
    // Without bulletins the recall list has no such link.
    let (_, html) = page(
        &common::recalls_app().await,
        "/makes/hyundai/kona/2023/recalls",
    )
    .await;
    assert!(!html.contains("/bulletins"));
}

#[tokio::test]
async fn the_model_year_page_names_the_models_it_shares_documents_with() {
    let app = common::cross_links_app().await;
    let (status, html) = page(&app, "/makes/hyundai/kona/2023").await;
    assert_eq!(status, StatusCode::OK);
    assert!(html.contains("<h2 id=\"siblings\">Shares campaigns and bulletins with</h2>"));
    assert!(html.contains("2023 Chevrolet Silverado"));
    assert!(html.contains("(7 of 20)"));
    assert!(html.contains("Worked out from the recall campaigns and bulletins that list both. It does not mean a part or a repair for one fits the other."));
    let (_, text) = page(&app, "/makes/hyundai/kona/2023.md").await;
    assert!(text.contains("## Shares campaigns and bulletins with"));
    assert!(text.contains("It does not mean a part or a repair for one fits the other."));
    // No row, no heading.
    let (_, html) = page(&app, "/makes/hyundai/kona/2022").await;
    assert!(!html.contains("id=\"siblings\""));
    let (_, html) = page(&common::app().await, "/makes/hyundai/kona/2023").await;
    assert!(!html.contains("id=\"siblings\""));
}
