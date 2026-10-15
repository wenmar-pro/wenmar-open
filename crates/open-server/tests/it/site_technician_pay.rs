//! The technician pay calculator, `/tools/technician-pay`, through HTTP.

use axum::http::StatusCode;

use crate::common::{
    self, assert_basics, assert_head, assert_no_injection, assert_targets, body_text, header,
    markdown, page,
};

const PAGE: &str = "/tools/technician-pay";

fn main_of(html: &str) -> &str {
    let main = html.split(r#"<main id="main">"#).nth(1).unwrap();
    main.split("</main>").next().unwrap()
}

#[tokio::test]
async fn the_page_opens_with_its_example_worked_out() {
    let app = common::app().await;
    let (status, html) = page(&app, PAGE).await;
    assert_eq!(status, StatusCode::OK);
    assert_basics(&html, PAGE);
    assert_head(&html, PAGE);
    assert_targets(&html, PAGE);
    assert!(
        html.contains("<title>Technician pay calculator for repair shops - Wenmar Open</title>")
    );
    assert!(
        html.contains(r#"<link rel="canonical" href="https://open.example/tools/technician-pay">"#)
    );
    let main = main_of(&html);
    assert!(main.contains("<h1>Technician pay calculator for repair shops</h1>"));
    assert!(main.contains("The form is filled in with an example."));
    // The gross pay, worked out in whole cents.
    assert!(main.contains(r#"<caption>Gross pay</caption>"#));
    assert!(main.contains(r#"<tr><th scope="row">Flagged hours</th><td>$2,560.00</td></tr>"#));
    assert!(main.contains(r#"<tr><th scope="row">Hourly hours</th><td>$150.00</td></tr>"#));
    assert!(main.contains(r#"<tr><th scope="row">Gross pay</th><td>$2,710.00</td></tr>"#));
    // The Ontario deductions on the gross.
    assert!(main.contains("Deductions from one biweekly pay"));
    assert!(main.contains(r#"<tr><th scope="row">Gross pay</th><td>$2,710.00</td></tr>"#));
    assert!(main.contains(">Net pay<"));
    // The link to Wenmar Pro.
    assert!(html.contains("utm_campaign=tool-technician-pay"));
    assert!(html.contains(r#"<a href="/tools" aria-current="page">Tools</a>"#));
}

#[tokio::test]
async fn a_filled_in_form_is_worked_out_and_not_indexed() {
    let app = common::app().await;
    // 40 flagged hours at $45 + 8 hourly hours at $30 = 1800 + 240 = 2040 gross.
    let address = format!(
        "{PAGE}?flagged_hours=40&flat_rate=45.00&hourly_hours=8&hourly_rate=30.00&frequency=weekly&pay_date=2026-07-15"
    );
    let response = app.get(&address).await;
    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(header(&response, "x-robots-tag"), "noindex");
    assert_eq!(header(&response, "cache-control"), "private, max-age=3600");
    let html = body_text(response).await;
    let main = main_of(&html);
    assert!(!main.contains("The form is filled in with an example."));
    assert!(main.contains(r#"<tr><th scope="row">Flagged hours</th><td>$1,800.00</td></tr>"#));
    assert!(main.contains(r#"<tr><th scope="row">Hourly hours</th><td>$240.00</td></tr>"#));
    assert!(main.contains(r#"<tr><th scope="row">Gross pay</th><td>$2,040.00</td></tr>"#));
    // The deductions are on the 2040 gross.
    assert!(main.contains("Deductions from one weekly pay"));
}

#[tokio::test]
async fn leaving_the_frequency_empty_shows_the_gross_only() {
    let app = common::app().await;
    let address =
        format!("{PAGE}?flagged_hours=10&flat_rate=40.00&hourly_hours=30&hourly_rate=30.00");
    let response = app.get(&address).await;
    assert_eq!(response.status(), StatusCode::OK);
    let html = body_text(response).await;
    let main = main_of(&html);
    // 10 × 40 + 30 × 30 = 400 + 900 = 1300 gross.
    assert!(main.contains(r#"<tr><th scope="row">Gross pay</th><td>$1,300.00</td></tr>"#));
    // No deductions, because no frequency.
    assert!(main.contains("The pay frequency was left empty"));
}

#[tokio::test]
async fn a_field_that_cannot_be_read_gets_its_message_and_keeps_what_was_typed() {
    let app = common::app().await;
    let (status, html) = page(
        &app,
        &format!("{PAGE}?flagged_hours=abc&flat_rate=40.00&frequency=fortnightly"),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let main = main_of(&html);
    assert!(main.contains(
        r#"<p class="warning" id="flagged_hours-error">This is not a number of hours. Write it like 8.5.</p>"#
    ));
    assert!(main.contains(r#"value="abc""#));
    assert!(main.contains("There is no result yet."));
}

#[tokio::test]
async fn nothing_typed_into_the_form_arrives_as_markup() {
    let app = common::app().await;
    let (status, html) = page(
        &app,
        &format!("{PAGE}?flagged_hours=%3Cscript%3Ealert(1)%3C%2Fscript%3E"),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_no_injection(&html);
}

#[tokio::test]
async fn the_markdown_version_states_the_formulas_and_the_example() {
    let app = common::app().await;
    let text = markdown(&app, "/tools/technician-pay.md").await;
    assert!(
        text.starts_with("# Technician pay calculator for repair shops\n"),
        "{text}"
    );
    assert!(text.contains("\n## A worked example\n"));
    assert!(text.contains("| Gross pay | $2,710.00 |"));
    assert!(text.contains("Deductions from one biweekly pay"));
}
