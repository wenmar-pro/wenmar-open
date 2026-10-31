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

const KONA_LIST: &str = "/makes/hyundai/kona/2023/labor";

#[tokio::test]
async fn a_vehicle_has_a_page_of_all_its_labour_times() {
    let app = common::labor_ai_app().await;
    let response = app.get(KONA_LIST).await;
    assert_eq!(response.status(), StatusCode::OK);
    assert!(response.headers().get("x-robots-tag").is_none());
    let (_, html) = page(&app, KONA_LIST).await;
    common::assert_basics(&html, KONA_LIST);
    common::assert_head(&html, KONA_LIST);
    assert!(
        html.contains("<h1>2023 Hyundai Kona labour times</h1>"),
        "{html}"
    );
    let title = html
        .split("<title>")
        .nth(1)
        .and_then(|rest| rest.split("</title>").next())
        .unwrap_or_default();
    assert!(title.contains("2023 Hyundai Kona labour times"), "{title}");
    // The jobs with a figure of their own, each linked to its page.
    for job in ["test-job-a", "test-job-c"] {
        assert!(
            html.contains(&format!(
                r#"<a href="/makes/hyundai/kona/2023/labor/{job}">"#
            )),
            "{job}:\n{html}"
        );
    }
    assert!(html.contains("88 hours"), "{html}");
    assert!(html.contains("87 to 89 hours"), "{html}");
    assert!(html.contains("13 hours"), "{html}");
    assert!(html.contains("12 to 15 hours"), "{html}");
    // Job B is only typical of a small SUV, and says so.
    let narrow = html
        .find("/makes/hyundai/kona/2023/labor/test-job-a")
        .unwrap();
    let class = html
        .find("Typical for a Small SUV")
        .expect("a class heading");
    assert!(narrow < class, "narrow figures come first:\n{html}");
    assert!(
        html.contains(r#"<a href="/makes/hyundai/kona/2023/labor/test-job-b">"#),
        "{html}"
    );
    assert!(html.contains("47 to 49 hours"), "{html}");
    assert!(html.contains("48 hours"), "{html}");
    // The head names the API answer for the page.
    assert_eq!(
        common::json_alternate(&html).as_deref(),
        Some("/v1/vehicles/2023_hyundai_kona/labor")
    );
    let (status, _) = app.json("/v1/vehicles/2023_hyundai_kona/labor").await;
    assert_eq!(status, StatusCode::OK);

    let text = common::markdown(&app, &format!("{KONA_LIST}.md")).await;
    assert!(
        text.starts_with("# 2023 Hyundai Kona labour times\n"),
        "{text}"
    );
    assert!(text.contains("87 to 89 hours, typically 88"), "{text}");
    assert!(text.contains("Typical for a Small SUV"), "{text}");
    assert!(
        text.contains("/makes/hyundai/kona/2023/labor/test-job-a"),
        "{text}"
    );

    // A job with no figure at all is listed as such, with a way to
    // suggest one.
    let app = common::labor_app().await;
    let (_, html) = page(&app, KONA_LIST).await;
    assert!(html.contains("No estimate yet"), "{html}");
    assert!(html.contains("Test job C"), "{html}");
    assert!(
        html.contains(r#"<a href="/feedback/labor?vehicle=2023%20Hyundai%20Kona"#),
        "{html}"
    );
    assert!(html.contains("Suggest a time"), "{html}");
}

#[tokio::test]
async fn a_rate_puts_the_charge_beside_every_time_on_the_list() {
    let app = common::labor_ai_app().await;
    let response = app.get(&format!("{KONA_LIST}?rate=120")).await;
    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(header(&response, "x-robots-tag"), "noindex");
    let (_, html) = page(&app, &format!("{KONA_LIST}?rate=120")).await;
    // 87, 88 and 89 hours at $120.00; 47, 48 and 49.
    for charge in [
        "$10,440.00",
        "$10,560.00",
        "$10,680.00",
        "$5,640.00",
        "$5,760.00",
        "$5,880.00",
    ] {
        assert!(html.contains(charge), "{charge}:\n{html}");
    }
    assert!(html.contains("value=\"120\""), "{html}");
    // A page with a query is a view of the page that has none.
    assert!(
        html.contains(r#"<meta name="robots" content="noindex">"#),
        "{html}"
    );
    assert!(
        html.contains(&format!(
            r#"<link rel="canonical" href="https://open.example{KONA_LIST}">"#
        )),
        "{html}"
    );
    // A rate that cannot be read: the times, and why no charge.
    let (status, html) = page(&app, &format!("{KONA_LIST}?rate=abc")).await;
    assert_eq!(status, StatusCode::OK);
    assert!(
        html.contains("Enter your hourly rate as dollars, such as 150 or 149.50."),
        "{html}"
    );
}

#[tokio::test]
async fn the_vehicle_list_is_offered_to_search_engines_only_with_a_figure_of_its_own() {
    let app = common::labor_ai_app().await;
    let response = app.get(KONA_LIST).await;
    assert_eq!(response.status(), StatusCode::OK);
    assert!(response.headers().get("x-robots-tag").is_none());
    // The Civic has no labour estimate at all: a page, not indexed.
    let path = "/makes/honda/civic/2019/labor";
    let response = app.get(path).await;
    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(header(&response, "x-robots-tag"), "noindex");
    let (_, html) = page(&app, path).await;
    assert!(
        html.contains(r#"<meta name="robots" content="noindex">"#),
        "{html}"
    );
    assert!(html.contains("No estimate yet"), "{html}");
    // The list exists only where the estimates are held.
    let app = common::app_with_shop(common::JOB_DATASETS, &common::job_rows()).await;
    let (status, html) = page(&app, KONA_LIST).await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    assert!(html.contains("There is no page here"));
    let (status, _) = page(&app, &format!("{KONA_LIST}.md")).await;
    assert_eq!(status, StatusCode::NOT_FOUND);
}

const KONA_C: &str = "/makes/hyundai/kona/2023/labor/test-job-c";

#[tokio::test]
async fn a_vehicle_job_page_answers_in_its_first_sentence_and_says_how() {
    let app = common::labor_ai_app().await;
    let (status, html) = page(&app, KONA_C).await;
    assert_eq!(status, StatusCode::OK);
    let answer = "Test job C on a 2023 Hyundai Kona takes about 13 hours (12 to 15 hours).";
    let main = html.split("<main id=\"main\">").nth(1).expect("a main");
    let first_p = main
        .split_once("<p>")
        .and_then(|(_, rest)| rest.split("</p>").next())
        .expect("a first paragraph");
    assert_eq!(first_p, answer, "{main}");
    // The description starts with the same sentence.
    let description = html
        .split(r#"<meta name="description" content=""#)
        .nth(1)
        .and_then(|rest| rest.split('"').next())
        .expect("a description");
    assert!(description.starts_with(answer), "{description}");
    // What the AI figure assumed, and how it was made.
    assert!(
        html.contains("<h2>What the estimate assumed</h2>"),
        "{html}"
    );
    assert!(html.contains("Made-up assumption."), "{html}");
    assert!(html.contains("confidence 45%"), "{html}");
    assert!(html.contains("test-model"), "{html}");
    assert!(html.contains("2026-10-30"), "{html}");
    assert!(
        html.contains(r#"href="/jobs/how-estimates-are-made""#),
        "{html}"
    );
    // The other catalog years the generation figure covers.
    let years = "The same estimate covers the 2021 to 2024 Hyundai Kona";
    assert!(html.contains(years), "{html}");
    let after = &html[html.find(years).unwrap()..];
    assert!(
        after.contains(r#"<a href="/makes/hyundai/kona/2022/labor/test-job-c">2022</a>"#),
        "{html}"
    );
    assert!(
        !after.contains(r#"<a href="/makes/hyundai/kona/2023/labor/test-job-c">"#),
        "the year asked for is not linked as another:\n{html}"
    );
    // The title answers the question people search.
    assert!(
        html.contains("<title>2023 Hyundai Kona Test job C: 13 hours - Wenmar Open</title>"),
        "{html}"
    );
    // The Markdown twin leads with the same answer.
    let text = common::markdown(&app, &format!("{KONA_C}.md")).await;
    // The answer opens the Markdown twin, its brackets escaped as Markdown
    // punctuation, which reads the same.
    assert!(
        text.starts_with(
            "# Test job C on the 2023 Hyundai Kona: labour time\n\nTest job C on a 2023 Hyundai Kona takes about 13 hours \\(12 to 15 hours\\).\n"
        ),
        "{text}"
    );
    assert!(text.contains(r"Made\-up assumption."), "{text}");
    assert!(
        text.contains("https://open.example/jobs/how-estimates-are-made"),
        "{text}"
    );
    assert!(text.contains(years), "{text}");

    // An expert figure has no assumption to show, and says nothing of a
    // model or a confidence.
    let (_, html) = page(&app, "/makes/hyundai/kona/2023/labor/test-job-a").await;
    assert!(!html.contains("What the estimate assumed"), "{html}");
    assert!(!html.contains("confidence"), "{html}");
    assert!(!html.contains("test-model"), "{html}");
}

#[tokio::test]
async fn a_job_lists_the_vehicles_it_has_a_figure_for() {
    let app = common::labor_ai_app().await;
    let (status, html) = page(&app, "/jobs/test-job-c").await;
    assert_eq!(status, StatusCode::OK);
    assert!(
        html.contains("<h2>Vehicles with an estimate</h2>"),
        "{html}"
    );
    assert!(
        html.contains(r#"<a href="/makes/hyundai/kona/2023/labor/test-job-c">Hyundai Kona</a>"#),
        "{html}"
    );
    assert!(html.contains("2021 to 2024"), "{html}");
    assert!(html.contains("13 hours"), "{html}");
    assert!(html.contains("AI"), "{html}");
    assert!(
        !html.contains("No estimate has been written for this job yet."),
        "a job with a figure is not said to have none:\n{html}"
    );
    let text = common::markdown(&app, "/jobs/test-job-c.md").await;
    assert!(text.contains("## Vehicles with an estimate\n"), "{text}");
    assert!(
        text.contains(
            "- [Hyundai Kona](/makes/hyundai/kona/2023/labor/test-job-c.md): 2021 to 2024, 13 hours, AI, 45% confidence"
        ),
        "{text}"
    );

    // A job with only class figures has no vehicle of its own to list, and
    // is not said to have no estimate either.
    let app = common::labor_app().await;
    let (_, html) = page(&app, "/jobs/test-job-c").await;
    assert!(!html.contains("Vehicles with an estimate"), "{html}");
    assert!(
        !html.contains("No estimate has been written for this job yet."),
        "its class figure is an estimate:\n{html}"
    );
}

#[tokio::test]
async fn the_job_list_is_a_dataset_a_search_engine_can_list() {
    let app = common::labor_ai_app().await;
    let (status, html) = page(&app, "/jobs").await;
    assert_eq!(status, StatusCode::OK);
    assert!(html.contains(r#""@type":"Dataset""#), "{html}");
    assert!(
        html.contains(r#""name":"Wenmar labour estimates""#),
        "{html}"
    );
    assert!(
        html.contains(r#""license":"https://creativecommons.org/licenses/by/4.0/""#),
        "{html}"
    );
    assert!(html.contains(r#""isAccessibleForFree":true"#), "{html}");
    assert!(
        html.contains(r#""contentUrl":"https://open.example/v1/jobs""#),
        "{html}"
    );
    assert!(
        html.contains(r#""contentUrl":"https://open.example/jobs.md""#),
        "{html}"
    );
    // The newest figure, not the day the job list was built.
    assert!(html.contains(r#""dateModified":"2026-10-30""#), "{html}");
    assert!(html.contains(r#""@type":"Organization""#), "{html}");
}
