//! The one page per pay frequency of the Ontario payroll calculator,
//! `/tools/payroll-deductions/ontario/<frequency>`, through HTTP.

use axum::http::StatusCode;

use crate::common::{
    self, assert_basics, assert_head, assert_no_injection, assert_targets, body_text, header,
    markdown, page,
};

const PAGE: &str = "/tools/payroll-deductions/ontario";

fn main_of(html: &str) -> &str {
    let main = html.split(r#"<main id="main">"#).nth(1).unwrap();
    main.split("</main>").next().unwrap()
}

#[tokio::test]
async fn each_frequency_page_has_its_own_title_heading_intro_and_preset() {
    let app = common::app().await;
    for (slug, heading) in [
        ("weekly", "Ontario weekly payroll deductions 2026"),
        ("biweekly", "Ontario biweekly payroll deductions 2026"),
        (
            "semi-monthly",
            "Ontario semi-monthly payroll deductions 2026",
        ),
        ("monthly", "Ontario monthly payroll deductions 2026"),
    ] {
        let path = format!("{PAGE}/{slug}");
        let (status, html) = page(&app, &path).await;
        assert_eq!(status, StatusCode::OK, "{path}");
        assert_basics(&html, &path);
        assert_head(&html, &path);
        assert_targets(&html, &path);
        assert!(
            html.contains(&format!("<title>{heading} - Wenmar Open</title>")),
            "{path}"
        );
        assert!(
            html.contains(&format!(
                r#"<link rel="canonical" href="https://open.example{path}">"#
            )),
            "{path}"
        );
        let main = main_of(&html);
        assert!(main.contains(&format!("<h1>{heading}</h1>")), "{path}");
        // The form is preset to the page's frequency.
        assert!(
            main.contains(&format!(r#"<option value="{slug}" selected>"#)),
            "{path}"
        );
        // The example gross is there.
        assert!(main.contains(r#"value="2400.00""#), "{path}");
        // The page links the other three frequencies.
        for other in ["weekly", "biweekly", "semi-monthly", "monthly"] {
            if other != slug {
                assert!(
                    main.contains(&format!(r#"<a href="{PAGE}/{other}">"#)),
                    "{path} should link {other}"
                );
            }
        }
        // The result is worked out, with a net pay.
        assert!(main.contains(">Net pay<"), "{path}");
    }
}

#[tokio::test]
async fn a_frequency_page_works_out_a_pay_of_its_frequency() {
    let app = common::app().await;
    let address = format!("{PAGE}/weekly?gross=800.00&pay_date=2026-07-15");
    let response = app.get(&address).await;
    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(header(&response, "x-robots-tag"), "noindex");
    assert_eq!(header(&response, "cache-control"), "private, max-age=3600");
    let html = body_text(response).await;
    let main = main_of(&html);
    assert!(!main.contains("The form is filled in with an example."));
    // The weekly 800.00 rows, as PDOC works them out.
    assert!(main.contains(r#"<tr><th scope="row">Federal income tax</th><td>$55.74</td></tr>"#));
    assert!(main.contains(r#"<tr><th scope="row">Net pay</th><td>$654.04</td></tr>"#));
}

#[tokio::test]
async fn a_frequency_page_that_is_not_one_of_the_four_is_404() {
    let app = common::app().await;
    let (status, _) = page(&app, &format!("{PAGE}/fortnightly")).await;
    assert_eq!(status, StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn the_markdown_versions_state_the_example_of_their_frequency() {
    let app = common::app().await;
    for slug in ["weekly", "biweekly", "semi-monthly", "monthly"] {
        let text = markdown(&app, &format!("{PAGE}/{slug}.md")).await;
        assert!(
            text.starts_with(&format!("# Ontario {slug} payroll deductions 2026\n")),
            "{slug}"
        );
        assert!(text.contains("\n## A worked example\n"), "{slug}");
        assert!(text.contains("| Net pay |"), "{slug}");
    }
}

#[tokio::test]
async fn nothing_typed_into_the_frequency_page_arrives_as_markup() {
    let app = common::app().await;
    let (status, html) = page(
        &app,
        &format!("{PAGE}/biweekly?gross=%3Cscript%3Ealert(1)%3C%2Fscript%3E"),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_no_injection(&html);
}
