//! The Ontario payroll deductions calculator, `/tools/payroll-deductions`,
//! through HTTP. The expected figures are rows of CRA's Payroll Deductions
//! Online Calculator (PDOC), captured 2026-10-14 for a pay date of
//! 2026-07-15.

use axum::http::StatusCode;
use chrono::NaiveDate;
use payroll_calc::{NOTE, PDOC_URL, Query, deductions, factors::meaning};

use crate::common::{
    self, assert_basics, assert_head, assert_no_injection, assert_targets, body_text, header,
    markdown, page,
};

const PAGE: &str = "/tools/payroll-deductions";

fn main_of(html: &str) -> &str {
    let main = html.split(r#"<main id="main">"#).nth(1).unwrap();
    main.split("</main>").next().unwrap()
}

fn row(name: &str, amount: &str) -> String {
    format!(r#"<tr><th scope="row">{name}</th><td>{amount}</td></tr>"#)
}

fn input(name: &str, value: &str) -> String {
    format!(
        r#"<input id="{name}" name="{name}" type="text" inputmode="decimal" autocomplete="off" value="{value}""#
    )
}

#[tokio::test]
async fn with_nothing_sent_the_page_shows_its_example_worked_out() {
    let app = common::app().await;
    let (status, html) = page(&app, PAGE).await;
    assert_eq!(status, StatusCode::OK);
    assert_basics(&html, PAGE);
    assert_head(&html, PAGE);
    assert_targets(&html, PAGE);
    assert!(html.contains("<title>Ontario payroll deductions calculator - Wenmar Open</title>"));
    assert!(html.contains(
        r#"<link rel="canonical" href="https://open.example/tools/payroll-deductions">"#
    ));
    let main = main_of(&html);
    assert!(main.contains("The form is filled in with an example."));
    assert!(main.contains(r#"<option value="biweekly" selected>Biweekly</option>"#));
    assert!(main.contains(&input("gross", "2400.00")));
    assert!(main.contains(&input("federal_claim", "16452.00")));
    assert!(main.contains(&input("provincial_claim", "12989.00")));
    for (name, amount) in [
        ("Federal income tax", "$223.20"),
        ("Ontario income tax", "$122.74"),
        ("CPP (base and first additional)", "$134.79"),
        ("EI", "$39.12"),
        ("Net pay", "$1,880.15"),
    ] {
        assert!(main.contains(&row(name, amount)), "{name}");
    }
    // Estimate, liable, PDOC, and the sources the footer points at.
    // The page escapes an apostrophe as askama does: &#39;.
    let escape = |text: &str| text.replace('\'', "&#39;");
    let (before, link, after) = payroll_calc::note_parts();
    assert!(
        main.contains(&format!(
            r#"<p class="note">{}<a href="{PDOC_URL}">{link}</a>{}</p>"#,
            escape(before),
            escape(after)
        )),
        "{main}"
    );
    assert!(main.contains(r#"<h2 id="sources">Tables and sources</h2>"#));
    assert!(html.contains("Sources for these rates"));
    assert!(html.contains("utm_campaign=tool-payroll-deductions"));
    assert!(html.contains(r#"<a href="/tools" aria-current="page">Tools</a>"#));
}

#[tokio::test]
async fn pdoc_rows_are_reproduced_and_a_filled_in_page_is_not_indexed() {
    let app = common::app().await;
    for (query, rows) in [
        (
            "frequency=weekly&gross=800.00&pay_date=2026-07-15",
            [
                ("Federal income tax", "$55.74"),
                ("Ontario income tax", "$33.58"),
                ("CPP (base and first additional)", "$43.60"),
                ("EI", "$13.04"),
                ("Net pay", "$654.04"),
            ],
        ),
        (
            "frequency=biweekly&gross=2400.00&pay_date=2026-07-15",
            [
                ("Federal income tax", "$223.20"),
                ("Ontario income tax", "$122.74"),
                ("CPP (base and first additional)", "$134.79"),
                ("EI", "$39.12"),
                ("Net pay", "$1,880.15"),
            ],
        ),
        (
            "frequency=weekly&gross=800.00&federal_claim=0&provincial_claim=0&pay_date=2026-07-15",
            [
                ("Federal income tax", "$100.03"),
                ("Ontario income tax", "$46.19"),
                ("CPP (base and first additional)", "$43.60"),
                ("EI", "$13.04"),
                ("Net pay", "$597.14"),
            ],
        ),
    ] {
        let address = format!("{PAGE}?{query}");
        let response = app.get(&address).await;
        assert_eq!(response.status(), StatusCode::OK);
        assert_eq!(header(&response, "x-robots-tag"), "noindex");
        assert_eq!(header(&response, "cache-control"), "private, max-age=3600");
        let html = body_text(response).await;
        let main = main_of(&html);
        assert!(!main.contains("The form is filled in with an example."));
        for (name, amount) in rows {
            assert!(main.contains(&row(name, amount)), "{address}: {name}");
        }
    }
}

#[tokio::test]
async fn every_factor_is_shown_with_its_value_and_its_meaning() {
    let app = common::app().await;
    let (_, html) = page(
        &app,
        &format!("{PAGE}?frequency=biweekly&gross=2400.00&pay_date=2026-07-15"),
    )
    .await;
    let query = Query {
        frequency: Some("biweekly".into()),
        gross: Some("2400.00".into()),
        pay_date: Some("2026-07-15".into()),
        ..Query::default()
    };
    let worked = deductions(&query, NaiveDate::from_ymd_opt(2026, 10, 14).unwrap()).unwrap();
    assert!(html.contains(r#"<h2 id="factors">The T4127 factors used</h2>"#));
    for (name, value) in &worked.deductions.factors {
        let meaning = meaning(name).unwrap().replace('\'', "&#39;");
        assert!(
            html.contains(&format!(
                r#"<tr><th scope="row">{name}</th><td>{}</td><td>{meaning}</td></tr>"#,
                value.normalize()
            )),
            "{name}"
        );
    }
}

#[tokio::test]
async fn a_pay_date_outside_the_tables_is_a_message_and_not_an_error() {
    let app = common::app().await;
    for date in ["2027-01-15", "2026-06-30"] {
        let (status, html) = page(
            &app,
            &format!("{PAGE}?frequency=weekly&gross=800.00&pay_date={date}"),
        )
        .await;
        assert_eq!(status, StatusCode::OK, "{date}");
        let main = main_of(&html);
        assert!(
            main.contains(&format!(
                r#"<p class="warning" id="pay_date-error">No tables are held for {date}. This calculator holds T4127&#39;s 123rd edition, for pay dates from 2026-07-01 to 2026-12-31.</p>"#
            )),
            "{main}"
        );
        assert!(main.contains("There is no result yet."));
        assert!(main.contains(r##"<li><a href="#pay_date">Pay date: No tables are held"##));
        assert!(!main.contains(">Net pay<"));
    }
}

#[tokio::test]
async fn a_field_that_cannot_be_read_gets_its_message_and_keeps_what_was_typed() {
    let app = common::app().await;
    let (status, html) = page(
        &app,
        &format!("{PAGE}?frequency=fortnightly&gross=abc&pay_date=15%2F07%2F2026"),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let main = main_of(&html);
    assert!(main.contains(
        r#"<p class="warning" id="frequency-error">Choose weekly, biweekly, semi-monthly or monthly.</p>"#
    ));
    assert!(main.contains(
        r#"<p class="warning" id="gross-error">This is not a number. Write it like 1,234.50.</p>"#
    ));
    assert!(
        main.contains(
            r#"<p class="warning" id="pay_date-error">Write the date like 2026-07-15.</p>"#
        )
    );
    assert!(main.contains(r#"value="abc""#));
    let (_, html) = page(
        &app,
        &format!("{PAGE}?frequency=weekly&gross=%3Cscript%3Ealert(1)%3C%2Fscript%3E"),
    )
    .await;
    assert_no_injection(&html);
}

#[tokio::test]
async fn the_markdown_version_states_the_example_the_note_and_the_sources() {
    let app = common::app().await;
    let text = markdown(&app, "/tools/payroll-deductions.md").await;
    assert!(
        text.starts_with("# Ontario payroll deductions calculator\n"),
        "{text}"
    );
    assert!(text.contains(NOTE));
    assert!(text.contains(PDOC_URL));
    assert!(text.contains(
        "https://www.canada.ca/content/dam/cra-arc/formspubs/pub/t4127-jul/t4127-jul-26e.pdf"
    ));
    assert!(text.contains("\n## A worked example\n"));
    assert!(text.contains("| Net pay | $1,880.15 |"));
}
