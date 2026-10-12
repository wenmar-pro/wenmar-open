//! The tire size page, `/tools/tire-size`, through HTTP.

use axum::http::StatusCode;

use crate::common::{
    self, assert_basics, assert_head, assert_no_injection, assert_targets, body_text, header,
    markdown, page,
};

const PAGE: &str = "/tools/tire-size";

/// A row of a result table, as the page writes it.
fn row(cells: &[&str]) -> String {
    let rest: String = cells[1..]
        .iter()
        .map(|cell| format!("<td>{cell}</td>"))
        .collect();
    format!(r#"<tr><th scope="row">{}</th>{rest}</tr>"#, cells[0])
}

#[tokio::test]
async fn the_tire_size_page_opens_with_a_labelled_example_and_its_result() {
    let app = common::app().await;
    let response = app.get(PAGE).await;
    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(header(&response, "cache-control"), "public, max-age=3600");
    assert!(response.headers().get("x-robots-tag").is_none());
    let html = body_text(response).await;
    assert_basics(&html, PAGE);
    assert_head(&html, PAGE);
    assert_targets(&html, PAGE);
    assert!(html.contains("<title>Tire size comparison calculator - Wenmar Open</title>"));
    assert!(html.contains(r#"<link rel="canonical" href="https://open.example/tools/tire-size">"#));
    assert!(html.contains(r#"<li><a href="/tools">Tools</a></li>"#));
    let parts = [
        "<h1>Tire size comparison calculator</h1>",
        r##"<form class="calc" id="calc" action="/tools/tire-size#result" method="get">"##,
        "The form is filled in with an example.",
        r#"<label for="first">Size on the vehicle now</label>"#,
        r#"value="275/65R18""#,
        r#"<label for="second">Size to compare</label>"#,
        r#"value="LT285/70R17""#,
        r#"<button class="primary" type="submit">Compare</button>"#,
        r#"<h2 id="result">Result</h2>"#,
        "<caption>The two sizes</caption>",
        r#"<th scope="col">275/65R18</th><th scope="col">LT285/70R17</th>"#,
        "<caption>The second size against the first</caption>",
        "goes faster than the speedometer reads",
        "This page does not say whether a size fits a vehicle",
        r#"<h2 id="how">How this is worked out</h2>"#,
        "utm_campaign=tool-tire-size",
    ];
    let places: Vec<usize> = parts
        .iter()
        .map(|part| html.find(part).unwrap_or_else(|| panic!("missing: {part}")))
        .collect();
    assert!(places.is_sorted(), "{places:?}");
    assert_eq!(html.matches(r#"<div class="field">"#).count(), 2);
    assert!(!html.contains("style=") && !html.contains("inputmode"));
    for cells in [
        &[
            "Sidewall height",
            "178.8 mm (7.04 in)",
            "199.5 mm (7.85 in)",
        ][..],
        &[
            "Overall diameter",
            "814.7 mm (32.07 in)",
            "830.8 mm (32.71 in)",
        ],
        &["Circumference", "2559.5 mm", "2610.0 mm"],
        &["Revolutions per kilometre", "390.7", "383.1"],
        &["Difference in diameter", "16.1 mm (0.63 in)"],
        &["Difference as a percent of the first diameter", "1.976%"],
        &["Change in ride height", "8.1 mm (0.32 in)"],
        &[
            "True speed when the speedometer reads 100 km/h",
            "102.0 km/h",
        ],
    ] {
        assert!(html.contains(&row(cells)), "{cells:?}");
    }
}

#[tokio::test]
async fn a_filled_in_form_is_worked_out_kept_as_typed_and_not_indexed() {
    let app = common::app().await;
    let response = app
        .get("/tools/tire-size?first=205%2F55R16&second=225%2F45r17")
        .await;
    assert_eq!(header(&response, "cache-control"), "private, max-age=3600");
    assert_eq!(header(&response, "x-robots-tag"), "noindex");
    let html = body_text(response).await;
    assert!(!html.contains("The form is filled in with an example."));
    assert!(html.contains(r#"value="225/45r17""#));
    assert!(html.contains(r#"<th scope="col">205/55R16</th><th scope="col">225/45R17</th>"#));
    assert!(html.contains(&row(&[
        "Overall diameter",
        "631.9 mm (24.88 in)",
        "634.3 mm (24.97 in)"
    ])));
    assert!(html.contains(&row(&["Difference in diameter", "2.4 mm (0.09 in)"])));
    assert!(html.contains(&row(&[
        "Difference as a percent of the first diameter",
        "0.38%"
    ])));
    assert!(html.contains(&row(&[
        "True speed when the speedometer reads 100 km/h",
        "100.4 km/h"
    ])));
    assert!(html.contains(r#"<link rel="canonical" href="https://open.example/tools/tire-size">"#));
}

#[tokio::test]
async fn a_size_that_cannot_be_read_is_reported_beside_its_box() {
    let app = common::app().await;
    let (status, html) = page(&app, "/tools/tire-size?first=35x12.50R17&second=").await;
    assert_eq!(status, StatusCode::OK);
    assert!(html.contains("There is no result yet."));
    assert!(html.contains(r##"<a href="#first">Size on the vehicle now: This is not a tire size this page reads. Write it like 275/65R18 or LT285/70R17.</a>"##));
    assert!(html.contains(r##"<a href="#second">Size to compare: Enter a tire size.</a>"##));
    assert!(html.contains(r#"aria-invalid="true" aria-describedby="first-error""#));
    assert!(!html.contains("<caption>"));
    let hostile = "%3Cscript%3Ealert(1)%3C%2Fscript%3E";
    let (_, html) = page(
        &app,
        &format!("/tools/tire-size?first={hostile}&second={hostile}"),
    )
    .await;
    assert_no_injection(&html);
}

#[tokio::test]
async fn the_markdown_version_states_the_formulas_and_the_example() {
    let app = common::app().await;
    let text = markdown(&app, "/tools/tire-size.md").await;
    assert!(
        text.starts_with("# Tire size comparison calculator\n"),
        "{text}"
    );
    assert!(text.contains("| Overall diameter | 814.7 mm (32.07 in) | 830.8 mm (32.71 in) |\n"));
    assert!(text.contains("| True speed when the speedometer reads 100 km/h | 102.0 km/h |\n"));
    assert!(
        text.contains("https://open.example/tools/tire-size?first=275/65R18&second=LT285/70R17")
    );
}
