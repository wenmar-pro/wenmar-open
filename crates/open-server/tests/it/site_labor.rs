//! The job list and one job's definition.

use axum::http::StatusCode;

use crate::common::{self, header, page};

#[tokio::test]
async fn the_job_list_and_a_job_have_pages() {
    let app = common::jobs_app().await;
    let (status, html) = page(&app, "/jobs").await;
    assert_eq!(status, StatusCode::OK);
    common::assert_basics(&html, "/jobs");
    common::assert_head(&html, "/jobs");
    assert!(html.contains("<h1>Repair jobs</h1>"));
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
    assert!(!html.contains("labour"));
    assert!(!html.contains("hours"));
    let text = common::markdown(&app, "/jobs.md").await;
    assert!(text.starts_with("# Repair jobs\n"));
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
    assert!(html.contains("Report an error on this page"));
    assert!(!html.contains("hours"));
    assert!(!html.contains("estimate"));
    assert!(!html.contains("How estimates are made"));
    let text = common::markdown(&app, "/jobs/test-job-a.md").await;
    assert!(text.starts_with("# Test job A\n"));
    assert!(text.contains("## Includes\n\n- made\\-up step one\n"));
    assert!(!text.contains("hours"));
}

#[tokio::test]
async fn the_old_labour_pages_move_to_the_job_list() {
    let app = common::jobs_app().await;
    for (path, location) in [
        ("/jobs/how-estimates-are-made", "/jobs"),
        ("/jobs/how-estimates-are-made.md", "/jobs"),
        (
            "/makes/hyundai/kona/2023/labor/test-job-a?engine=3-5l",
            "/jobs/test-job-a",
        ),
        ("/makes/hyundai/kona/2023/labor", "/jobs"),
        ("/makes/hyundai/kona/2023/labor.md", "/jobs.md"),
        ("/feedback/labor", "/feedback"),
    ] {
        let response = app.get(path).await;
        assert_eq!(response.status(), StatusCode::MOVED_PERMANENTLY, "{path}");
        assert_eq!(header(&response, "location"), location, "{path}");
    }
    let response = app.post_json("/feedback/labor", "{}").await;
    assert_eq!(response.status(), StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn the_job_pages_exist_only_where_the_jobs_are_held() {
    let app = common::jobs_app().await;
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
    assert!(!index.contains("sitemaps/labor-"), "{index}");
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
