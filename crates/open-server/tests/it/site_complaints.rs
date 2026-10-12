//! "What owners report" on the model-year page.

use axum::http::StatusCode;

use crate::common::{self, page};

#[tokio::test]
async fn the_model_year_page_says_what_owners_reported_and_what_that_is_not() {
    let app = common::complaints_app().await;
    let (status, html) = page(&app, "/makes/hyundai/kona/2023").await;
    assert_eq!(status, StatusCode::OK);
    let section = html
        .find("<h2 id=\"owners-report\">What owners report</h2>")
        .unwrap();
    let rest = &html[section..];
    assert!(rest.contains("60 complaints filed with NHTSA, as of 2026-10-11."));
    assert!(rest.contains("it is not verified"));
    assert!(rest.contains("A count is not a rate"));
    // Kilometres first, miles beside.
    assert!(rest.contains("48,280 km (30,000 miles)"));
    // The components, most reported first; a part under its system's name.
    let electrical = rest
        .find("<th scope=\"row\">Electrical system</th>")
        .unwrap();
    let steering = rest.find("<th scope=\"row\">Steering</th>").unwrap();
    let engine = rest.find("<th scope=\"row\">Engine</th>").unwrap();
    assert!(electrical < steering && steering < engine);
    assert!(rest.contains("<th scope=\"row\">Power steering</th>"));
    // Fewer than five: not listed.
    assert!(!rest.contains("<th scope=\"row\">Brakes</th>"));
    // Crashes, fires and injuries are said as reports.
    assert!(rest.contains("3 report a crash, 1 a fire and 2 an injury"));
    for banned in [
        "defect",
        "Known problem",
        "known problem",
        "common failure",
        "fails at",
        "unreliable",
    ] {
        assert!(!rest.contains(banned), "{banned}");
    }
    assert!(rest.contains("href=\"https://www.nhtsa.gov/report-a-safety-problem\""));

    let (_, text) = page(&app, "/makes/hyundai/kona/2023.md").await;
    assert!(text.contains("\n## What owners report\n"));
    assert!(text.contains("60 complaints filed with NHTSA"));
    assert!(text.contains("A count is not a rate"));
    assert!(text.contains("- Electrical system: 25"));
}

#[tokio::test]
async fn a_model_year_with_few_complaints_has_no_such_section() {
    let app = common::complaints_app().await;
    // Three complaints: under five, so nothing is said.
    for path in ["/makes/hyundai/kona/2022", "/makes/honda/civic/2019"] {
        let (status, html) = page(&app, path).await;
        assert_eq!(status, StatusCode::OK, "{path}");
        assert!(!html.contains("id=\"owners-report\""), "{path}");
        let (_, text) = page(&app, &format!("{path}.md")).await;
        assert!(!text.contains("What owners report"), "{path}");
    }
    for app in [common::app().await, common::recalls_app().await] {
        let (_, html) = page(&app, "/makes/hyundai/kona/2023").await;
        assert!(!html.contains("id=\"owners-report\""));
    }
}
