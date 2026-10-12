//! "Maintenance updates from the manufacturer" on the model-year page.

use axum::http::StatusCode;

use crate::common::{self, page};

#[tokio::test]
async fn a_model_year_with_a_maintenance_bulletin_shows_it() {
    let app = common::bulletins_app().await;
    let (status, html) = page(&app, "/makes/hyundai/kona/2023").await;
    assert_eq!(status, StatusCode::OK);
    let section = html
        .find("<h2 id=\"maintenance\">Maintenance updates from the manufacturer</h2>")
        .unwrap();
    let rest = &html[section..];
    // The one bulletin with the topic, linked to its page, with NHTSA's
    // summary; the other five of the model year are not here.
    assert!(rest.contains("href=\"/bulletins/11000005\""));
    assert!(rest.contains("the oil change service interval is now 8,000 miles"));
    let end = rest.find("</table>").unwrap();
    assert!(!rest[..end].contains("/bulletins/11000004"));
    assert!(!rest[..end].contains("/bulletins/11000002"));
    // The page escapes the apostrophe as `&#39;`, which reads the same.
    assert!(rest.replace("&#39;", "'").contains(
        "Bulletins from the manufacturer that mention a maintenance schedule or a service interval, as NHTSA summarises them. They are not the schedule itself: read the document, and the owner's manual."
    ));
    assert!(rest.contains(
        "<a href=\"/makes/hyundai/kona/2023/bulletins?topic=maintenance\">All 1 maintenance bulletin for the 2023 Hyundai Kona</a>"
    ));
    // This bulletin has no manufacturer's date, so no address at NHTSA.
    assert!(!rest[..end].contains("static.nhtsa.gov"));

    let (_, text) = page(&app, "/makes/hyundai/kona/2023.md").await;
    assert!(text.contains("\n## Maintenance updates from the manufacturer\n"));
    assert!(text.contains("(/bulletins/11000005.md)"));
    assert!(text.contains("They are not the schedule itself"));
}

#[tokio::test]
async fn a_model_year_without_one_has_no_such_section() {
    let app = common::bulletins_app().await;
    // The Kona of 2022 has bulletins, none about maintenance.
    for path in ["/makes/hyundai/kona/2022", "/makes/honda/civic/2019"] {
        let (status, html) = page(&app, path).await;
        assert_eq!(status, StatusCode::OK, "{path}");
        assert!(!html.contains("id=\"maintenance\""), "{path}");
        let (_, text) = page(&app, &format!("{path}.md")).await;
        assert!(!text.contains("Maintenance updates"), "{path}");
    }
    // And no server without bulletins has it.
    for app in [common::app().await, common::recalls_app().await] {
        let (_, html) = page(&app, "/makes/hyundai/kona/2023").await;
        assert!(!html.contains("id=\"maintenance\""));
    }
}
