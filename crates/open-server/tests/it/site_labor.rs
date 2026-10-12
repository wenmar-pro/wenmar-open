//! The labour pages: `/jobs`, one job, and a job on a model year, with
//! their Markdown twins and the price form.

use axum::http::StatusCode;

use crate::common::{self, header, page};

const NOTE: &str =
    "An estimate, not a quote. Rust, seized fasteners and added equipment change the time.";

#[tokio::test]
async fn the_job_list_and_a_job_have_pages() {
    let app = common::labor_app().await;
    let (status, html) = page(&app, "/jobs").await;
    assert_eq!(status, StatusCode::OK);
    common::assert_basics(&html, "/jobs");
    common::assert_head(&html, "/jobs");
    assert!(html.contains("<h1>Repair jobs and labour times</h1>"));
    for (id, name) in [
        ("test-job-a", "Test job A"),
        ("test-job-b", "Test job B"),
        ("test-job-c", "Test job C"),
    ] {
        assert!(
            html.contains(&format!("<a href=\"/jobs/{id}\">{name}</a>")),
            "{id}"
        );
    }
    assert!(html.contains("never taken from a labour guide"));
    let text = common::markdown(&app, "/jobs.md").await;
    assert!(text.starts_with("# Repair jobs and labour times\n"));
    assert!(text.contains("- [Test job A](https://open.example/jobs/test-job-a)"));

    let (status, html) = page(&app, "/jobs/test-job-a").await;
    assert_eq!(status, StatusCode::OK);
    common::assert_basics(&html, "/jobs/test-job-a");
    common::assert_head(&html, "/jobs/test-job-a");
    assert!(html.contains("<h1>Test job A</h1>"));
    assert!(html.contains("<li>made-up step one</li>"));
    assert!(html.contains("<li>made-up exclusion</li>"));
    assert!(html.contains("Timing belt or chain, Engine"));
    assert!(html.contains("href=\"/jobs/test-job-b\""));
    // Its class figure, the caution and the credit.
    assert!(html.contains("<caption>Typical hours by class of vehicle</caption>"));
    assert!(html.contains("<th scope=\"row\">Small SUV</th>"));
    assert!(html.contains("97.5 to 99.25 hours, typically 98"));
    assert!(html.contains(NOTE));
    assert!(html.contains(common::LABOR_CREDIT));
    assert!(html.contains("Report an error on this page"));
    // A measured figure says what stands behind it.
    let (_, html) = page(&app, "/jobs/test-job-b").await;
    assert!(html.contains("47 to 49 hours, typically 48"));
    assert!(html.contains("measured from 40 repair orders at 6 shops"));
    let text = common::markdown(&app, "/jobs/test-job-a.md").await;
    assert!(text.starts_with("# Test job A\n"));
    assert!(text.contains("## Includes\n\n- made\\-up step one\n"));
    assert!(text.contains(common::LABOR_CREDIT));
}

#[tokio::test]
async fn a_job_page_sends_a_visitor_to_their_vehicle() {
    let app = common::labor_app().await;
    let response = app
        .get("/jobs/test-job-a?year=2023&make=Hyundai&model=Kona")
        .await;
    assert_eq!(response.status(), StatusCode::SEE_OTHER);
    assert_eq!(
        header(&response, "location"),
        "/makes/hyundai/kona/2023/labor/test-job-a"
    );
    // A vehicle the catalog does not have: the page, saying so.
    let (status, html) = page(
        &app,
        "/jobs/test-job-a?year=2023&make=Hyundai&model=Nothing",
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert!(html.contains("The catalog has no 2023 Hyundai Nothing."));
    assert!(html.contains("<meta name=\"robots\" content=\"noindex"));
    common::assert_no_injection(
        &page(&app, "/jobs/test-job-a?year=2023&make=%3Cscript%3E&model=x")
            .await
            .1,
    );
}

#[tokio::test]
async fn the_job_pages_exist_only_where_the_jobs_are_held() {
    let app = common::labor_app().await;
    for path in [
        "/jobs/test-job-z",
        "/jobs/Test-Job-A",
        "/jobs/test-job-z.md",
    ] {
        let (status, _) = page(&app, path).await;
        assert_eq!(status, StatusCode::NOT_FOUND, "{path}");
    }
    let (_, xml) = page(&app, "/sitemaps/jobs.xml").await;
    assert!(xml.contains("<loc>https://open.example/jobs/test-job-a</loc>"));
    assert_eq!(xml.matches("<loc>").count(), 3);
    let (_, index) = page(&app, "/sitemap.xml").await;
    assert!(index.contains("https://open.example/sitemaps/jobs.xml"));
    let (_, pages) = page(&app, "/sitemaps/pages.xml").await;
    assert!(pages.contains("<loc>https://open.example/jobs</loc>"));
    let app = common::recalls_app().await;
    for path in [
        "/jobs",
        "/jobs.md",
        "/jobs/test-job-a",
        "/sitemaps/jobs.xml",
    ] {
        let (status, _) = page(&app, path).await;
        assert_eq!(status, StatusCode::NOT_FOUND, "{path}");
    }
    let (_, index) = page(&app, "/sitemap.xml").await;
    assert!(!index.contains("sitemaps/jobs.xml"));
}
