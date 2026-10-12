//! The bulletin line on the model-year page and the section on the VIN
//! result page.

use axum::http::StatusCode;

use crate::common::{self, page};

#[tokio::test]
async fn the_model_year_page_says_how_many_bulletins_are_on_file() {
    let app = common::bulletins_app().await;
    let (status, html) = page(&app, "/makes/hyundai/kona/2023").await;
    assert_eq!(status, StatusCode::OK);
    assert!(html.contains(
        "<tr><th scope=\"row\">Bulletins</th><td><a href=\"/makes/hyundai/kona/2023/bulletins\">6 manufacturer bulletins on file</a></td></tr>"
    ));
    // The recall line is still there, before it.
    let recalls = html.find("<th scope=\"row\">Recalls</th>").unwrap();
    let bulletins = html.find("<th scope=\"row\">Bulletins</th>").unwrap();
    assert!(recalls < bulletins);
    let (_, text) = page(&app, "/makes/hyundai/kona/2023.md").await;
    assert!(text.contains(
        "\n## Bulletins\n\n[6 manufacturer bulletins on file](/makes/hyundai/kona/2023/bulletins.md)\n"
    ));
    // A model year with none says so and still links the list.
    let (_, html) = page(&app, "/makes/honda/civic/2019").await;
    assert!(html.contains("None on file from NHTSA"));
}

#[tokio::test]
async fn the_vin_page_lists_the_newest_bulletins_of_its_model_year() {
    let app = common::bulletins_app().await;
    let (status, html) = page(&app, &format!("/vin/{}", common::KONA)).await;
    assert_eq!(status, StatusCode::OK);
    let section = html
        .find("<h2 id=\"bulletins\">Manufacturer bulletins</h2>")
        .unwrap();
    let rest = &html[section..];
    assert!(rest.contains(
        "6 manufacturer bulletins on file list the 2023 Hyundai Kona. This is a list for the model year"
    ));
    assert!(rest.contains("Warranty extension: 1"));
    for id in [11000003, 11000002, 11000010] {
        assert!(rest.contains(&format!("href=\"/bulletins/{id}\"")), "{id}");
    }
    assert!(!rest.contains("href=\"/bulletins/11000004\""));
    assert!(rest.contains(
        "<a href=\"/makes/hyundai/kona/2023/bulletins\">All bulletins for the 2023 Hyundai Kona</a>"
    ));
    // After the recall section, not in place of it.
    assert!(html.find("Recalls that may apply").unwrap() < section);
}

#[tokio::test]
async fn without_bulletins_both_pages_are_as_they_were() {
    for app in [common::app().await, common::recalls_app().await] {
        let (status, html) = page(&app, "/makes/hyundai/kona/2023").await;
        assert_eq!(status, StatusCode::OK);
        assert!(!html.contains("Bulletins</th>"));
        let (status, html) = page(&app, &format!("/vin/{}", common::KONA)).await;
        assert_eq!(status, StatusCode::OK);
        assert!(!html.contains("Manufacturer bulletins"));
    }
}
