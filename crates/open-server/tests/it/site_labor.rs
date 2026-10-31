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
    assert!(html.contains("never copied from a labour guide"));
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

const KONA_A: &str = "/makes/hyundai/kona/2023/labor/test-job-a";
const KONA_B: &str = "/makes/hyundai/kona/2023/labor/test-job-b";

#[tokio::test]
async fn a_job_on_a_model_year_says_the_hours_and_what_the_figure_is_for() {
    let app = common::labor_app().await;
    // Job A has a figure for the model: narrower than a class, so the
    // page is offered to search engines.
    let response = app.get(KONA_A).await;
    assert_eq!(response.status(), StatusCode::OK);
    assert!(response.headers().get("x-robots-tag").is_none());
    let (_, html) = page(&app, KONA_A).await;
    common::assert_basics(&html, KONA_A);
    common::assert_head(&html, KONA_A);
    assert!(html.contains("<h1>Test job A on the 2023 Hyundai Kona: labour time</h1>"));
    assert!(html.contains("<strong>87 to 89 hours, typically 88.</strong>"));
    assert!(html.contains("This figure is for the Hyundai Kona, every model year."));
    assert!(html.contains("Written by a person who does this work."));
    assert!(html.contains(NOTE));
    assert!(html.contains(common::LABOR_CREDIT));
    assert!(html.contains("<li>made-up step one</li>"));
    assert!(html.contains("<li>made-up exclusion</li>"));
    // No price until a rate is given.
    assert!(html.contains("<label for=\"rate\">"));
    assert!(html.contains("<label for=\"province\">"));
    assert!(!html.contains("<caption>At your rate</caption>"));
    assert!(!html.contains('$'));
    // The recalls of the model year, and the job's own page.
    assert!(html.contains("href=\"/makes/hyundai/kona/2023/recalls\""));
    assert!(html.contains("href=\"/jobs/test-job-a\""));
    assert!(html.contains("Report an error on this page"));

    // Job B has only a class figure: the page says so and is not indexed.
    let response = app.get(KONA_B).await;
    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(header(&response, "x-robots-tag"), "noindex");
    let (_, html) = page(&app, KONA_B).await;
    assert!(html.contains("<strong>47 to 49 hours, typically 48.</strong>"));
    assert!(html.contains(
        "This figure is typical for a Small SUV. It was not worked out on this vehicle."
    ));
    assert!(html.contains("Measured from 40 repair orders at 6 shops."));
    assert!(html.contains("<meta name=\"robots\" content=\"noindex"));
    // Job C has no figure that reaches the vehicle.
    let path = "/makes/hyundai/kona/2023/labor/test-job-c";
    let response = app.get(path).await;
    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(header(&response, "x-robots-tag"), "noindex");
    let (_, html) = page(&app, path).await;
    assert!(html.contains("No estimate covers this job on this vehicle yet."));
    assert!(!html.contains("<label for=\"rate\">"));
    // The engine is asked for in the query, and such a page is a view.
    let (_, html) = page(&app, &format!("{KONA_A}?engine=2-0l")).await;
    assert!(html.contains("<strong>67 to 69.5 hours, typically 68.</strong>"));
    assert!(html.contains("with the 2-0l engine"));
    assert!(html.contains("<meta name=\"robots\" content=\"noindex"));

    let text = common::markdown(&app, &format!("{KONA_A}.md")).await;
    assert!(text.starts_with("# Test job A on the 2023 Hyundai Kona: labour time\n"));
    assert!(text.contains("87 to 89 hours, typically 88."));
    assert!(text.contains(common::LABOR_CREDIT));
    assert!(!text.contains('$'));
}

#[tokio::test]
async fn a_rate_makes_a_price_and_a_province_adds_its_tax() {
    let app = common::labor_app().await;
    // 87, 88 and 89 hours at $100.00.
    let (status, html) = page(&app, &format!("{KONA_A}?rate=100")).await;
    assert_eq!(status, StatusCode::OK);
    assert!(html.contains("<caption>At your rate</caption>"));
    for charge in ["$8,700.00", "$8,800.00", "$8,900.00"] {
        assert!(html.contains(charge), "{charge}");
    }
    assert!(html.contains("value=\"100\""));
    assert!(html.contains("No province was chosen, so no tax is added."));
    assert!(html.contains("<meta name=\"robots\" content=\"noindex"));
    assert!(html.contains(NOTE));
    // Alberta charges 5% on labour: $440.00 on $8,800.00.
    let (_, html) = page(&app, &format!("{KONA_A}?rate=100.00&province=AB")).await;
    assert!(html.contains("Alberta"));
    for amount in ["$8,800.00", "$440.00", "$9,240.00"] {
        assert!(html.contains(amount), "{amount}");
    }
    assert!(html.contains("<option value=\"AB\" selected>"));
    // A rate that cannot be read: hours only, and why.
    for rate in ["abc", "-5", "1.234"] {
        let (status, html) = page(&app, &format!("{KONA_A}?rate={rate}&province=AB")).await;
        assert_eq!(status, StatusCode::OK, "{rate}");
        assert!(
            html.contains("Enter your hourly rate as dollars, such as 150 or 149.50."),
            "{rate}"
        );
        assert!(!html.contains("<caption>At your rate</caption>"), "{rate}");
    }
    common::assert_no_injection(
        &page(
            &app,
            &format!("{KONA_A}?rate=%3Cscript%3E&province=%22%3E%3Cb%3E"),
        )
        .await
        .1,
    );
}

#[tokio::test]
async fn the_page_exists_for_jobs_of_the_list_and_vehicles_of_the_catalog() {
    let app = common::labor_app().await;
    for path in [
        "/makes/hyundai/kona/2023/labor/test-job-z",
        "/makes/hyundai/kona/2023/labor/Test-Job-A",
        "/makes/hyundai/kona/2023/labor",
        "/makes/hyundai/nothing/2023/labor/test-job-a",
        "/makes/hyundai/kona/1899/labor/test-job-a",
        "/makes/hyundai/kona/2023/labor/test-job-a/more",
    ] {
        let (status, _) = page(&app, path).await;
        assert_eq!(status, StatusCode::NOT_FOUND, "{path}");
    }
    // The job list alone: no estimates, so no such page.
    let app = common::app_with_shop(common::JOB_DATASETS, &common::job_rows()).await;
    let (status, _) = page(&app, KONA_A).await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    let (status, html) = page(&app, "/jobs/test-job-a").await;
    assert_eq!(status, StatusCode::OK);
    assert!(!html.contains("id=\"vehicle\""));
}

#[tokio::test]
async fn every_labour_page_links_the_form_that_suggests_a_time() {
    let app = common::labor_app().await;
    let (_, html) = page(&app, "/jobs/test-job-a").await;
    assert!(
        html.contains("<a href=\"/feedback/labor?job=test-job-a&"),
        "{html}"
    );
    assert!(html.contains("page=%2Fjobs%2Ftest-job-a\">Suggest a time for this job</a>"));
    for job in ["test-job-a", "test-job-b", "test-job-c"] {
        let (_, html) = page(&app, &format!("/makes/hyundai/kona/2023/labor/{job}")).await;
        assert!(
            html.contains(&format!("/feedback/labor?job={job}")),
            "{job}"
        );
        assert!(html.contains("vehicle=2023%20Hyundai%20Kona"), "{job}");
        assert!(html.contains("Suggest a time for this job"), "{job}");
    }
}

const METHOD: &str = "/jobs/how-estimates-are-made";

const METHOD_HEADINGS: [&str; 3] = [
    "Written by people who do the work",
    "Measured from repair orders",
    "Worked out by a model from the repair steps",
];

#[tokio::test]
async fn the_method_page_says_how_each_figure_is_made() {
    let app = common::labor_ai_app().await;
    let (status, html) = page(&app, METHOD).await;
    assert_eq!(status, StatusCode::OK);
    common::assert_basics(&html, METHOD);
    common::assert_head(&html, METHOD);
    assert!(html.contains("<h1>How Wenmar's labour estimates are made</h1>"));
    for heading in METHOD_HEADINGS {
        assert!(html.contains(&format!(">{heading}</h2>")), "{heading}");
    }
    // The counts are the file's: one AI figure, and how it was made.
    assert!(html.contains("1 figure"), "{html}");
    assert!(html.contains("test-model"), "{html}");
    assert!(html.contains("open-labour-ai-1"), "{html}");
    assert!(html.contains(common::LABOR_CREDIT));
    // A page worth citing: indexed, with a canonical address.
    assert!(
        html.contains(&format!(
            r#"<link rel="canonical" href="https://open.example{METHOD}">"#
        )),
        "{html}"
    );
    assert!(!html.contains("noindex"), "{html}");
    // The structured data is an article dated by the newest figure.
    assert!(html.contains(r#""@type":"Article""#), "{html}");
    assert!(html.contains(r#""dateModified":"2026-10-30""#), "{html}");
}

#[tokio::test]
async fn the_method_page_is_not_a_job() {
    let app = common::labor_ai_app().await;
    let (status, html) = page(&app, METHOD).await;
    assert_eq!(status, StatusCode::OK);
    // The job page of a job with no figure says this; the method page does
    // not. The static route won over `/jobs/{job}`.
    assert!(!html.contains("What the job includes"), "{html}");
    assert!(!html.contains("For one vehicle"), "{html}");
}

#[tokio::test]
async fn the_method_page_has_a_markdown_twin() {
    let app = common::labor_ai_app().await;
    let text = common::markdown(&app, "/jobs/how-estimates-are-made.md").await;
    assert!(text.starts_with("# How Wenmar's labour estimates are made\n"));
    for heading in METHOD_HEADINGS {
        assert!(text.contains(&format!("## {heading}\n")), "{heading}");
    }
    assert!(text.contains("1 figure"), "{text}");
    assert!(
        text.contains("The AI figures worked out with the repair steps: 1 figure."),
        "{text}"
    );
    assert!(text.contains(r"Updated 2026\-10\-30."), "{text}");
    assert!(text.contains(common::LABOR_CREDIT));
}

#[tokio::test]
async fn without_labour_the_method_page_is_absent() {
    let app = common::app_with_shop(common::JOB_DATASETS, &common::job_rows()).await;
    for path in [METHOD, "/jobs/how-estimates-are-made.md"] {
        let (status, html) = page(&app, path).await;
        assert_eq!(status, StatusCode::NOT_FOUND, "{path}");
        assert!(html.contains("There is no page here"), "{path}");
    }
}

#[tokio::test]
async fn every_labour_page_links_the_page_that_says_how_the_figures_are_made() {
    let app = common::labor_app().await;
    for path in ["/jobs", "/jobs/test-job-a", KONA_A] {
        let (status, html) = page(&app, path).await;
        assert_eq!(status, StatusCode::OK, "{path}");
        assert!(
            html.contains(r#"href="/jobs/how-estimates-are-made""#),
            "{path}"
        );
        let text = common::markdown(&app, &format!("{path}.md")).await;
        assert!(
            text.contains("https://open.example/jobs/how-estimates-are-made"),
            "{path}.md"
        );
    }
}

#[tokio::test]
async fn the_job_list_says_where_the_figures_come_from() {
    let app = common::labor_app().await;
    let sentence = "written by people who do the work, measured from repair orders, or worked out by AI from the repair steps; never copied from a labour guide";
    let old = "written by people who do the work, and never taken from a labour guide";
    let (status, html) = page(&app, "/jobs").await;
    assert_eq!(status, StatusCode::OK);
    assert!(html.contains(sentence), "{html}");
    assert!(!html.contains(old), "{html}");
    let text = common::markdown(&app, "/jobs.md").await;
    assert!(text.contains(sentence), "{text}");
    assert!(!text.contains(old), "{text}");
}
