//! The labor rate page, `/tools/labor-rate`, through HTTP.

use axum::http::StatusCode;
use shop_math::targets::LABOR;

use crate::common::{
    self, assert_basics, assert_head, assert_no_injection, assert_targets, body_text, header,
    json_ld, markdown, page,
};

const PAGE: &str = "/tools/labor-rate";

/// A shop of two technicians, and a month in which it got more than it
/// posts. Two fields are sent empty: one has a default, one is optional.
const FILLED: &str = "/tools/labor-rate?technicians=2&paid_hours=160&productivity=90%25&technician_cost=%2412%2C000&overhead=20000&parts_profit=&target_profit=15&target_labor=65&labor_sales=30000&hours_billed=250&posted_rate=110&period_cost=";

/// The second part alone: every field of the first part is sent empty.
const GETTING_ONLY: &str = "/tools/labor-rate?technicians=&paid_hours=&productivity=&technician_cost=&overhead=&parts_profit=&target_profit=&target_labor=&labor_sales=48000&hours_billed=520&posted_rate=&period_cost=";

/// The part of a page between `<main>` and `</main>`.
fn main_of(html: &str) -> &str {
    let main = html.split(r#"<main id="main">"#).nth(1).unwrap();
    main.split("</main>").next().unwrap()
}

/// A row of a result table, as the page writes it.
fn row(name: &str, amount: &str) -> String {
    format!(r#"<tr><th scope="row">{name}</th><td>{amount}</td></tr>"#)
}

/// A box of the form with what it holds, as the page writes it.
fn input(name: &str, value: &str) -> String {
    format!(
        r#"<input id="{name}" name="{name}" type="text" inputmode="decimal" autocomplete="off" value="{value}""#
    )
}

#[tokio::test]
async fn the_labor_rate_page_opens_with_a_labelled_example_and_both_results() {
    let app = common::app().await;
    let response = app.get(PAGE).await;
    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(header(&response, "cache-control"), "public, max-age=3600");
    assert!(response.headers().get("etag").is_some());
    assert!(response.headers().get("x-robots-tag").is_none());
    assert!(response.headers().get("set-cookie").is_none());
    let html = body_text(response).await;
    assert_basics(&html, PAGE);
    assert_head(&html, PAGE);
    assert_targets(&html, PAGE);

    // Indexed, with a title, a description, breadcrumbs and the structured
    // data kinds a guide has.
    assert!(
        html.contains("<title>Labor rate calculator for auto repair shops - Wenmar Open</title>")
    );
    assert!(
        html.contains(r#"<link rel="canonical" href="https://open.example/tools/labor-rate">"#)
    );
    assert!(
        !html.contains(r#"name="robots""#),
        "the bare address is indexed"
    );
    assert!(html.contains(r#"<li><a href="/tools">Tools</a></li>"#));
    assert!(html.contains(r#"<a href="/tools" aria-current="page">Tools</a>"#));
    let data = json_ld(&html).unwrap();
    let kinds: Vec<&str> = data["@graph"]
        .as_array()
        .unwrap()
        .iter()
        .map(|thing| thing["@type"].as_str().unwrap())
        .collect();
    assert_eq!(kinds, ["BreadcrumbList", "Article", "Organization"]);

    // The seven parts of a calculator page, in order, with the two parts
    // of the form inside the one form.
    let main = main_of(&html);
    let parts = [
        "<h1>Labor rate calculator for auto repair shops</h1>",
        "<p>Works out the hourly labor rate a repair shop needs",
        r##"<form class="calc" id="calc" action="/tools/labor-rate#result" method="get">"##,
        r#"<h2 id="needed">The rate the shop needs</h2>"#,
        r#"<h2 id="getting">The rate the shop is getting</h2>"#,
        r#"<button class="primary" type="submit">Work it out</button>"#,
        "</form>",
        r#"<h2 id="result">Result</h2>"#,
        "<caption>The rate the shop needs</caption>",
        "<caption>The rate the shop is getting</caption>",
        r#"<h2 id="how">How this is worked out</h2>"#,
        r#"<p class="note">This is arithmetic, not accounting or tax advice.</p>"#,
        r#"<p class="pro"><a href="https://wenmarpro.com/?utm_source=wenmar-open&#38;utm_medium=referral&#38;utm_campaign=tool-labor-rate">Wenmar Pro</a> is shop management software from the people who make this site.</p>"#,
    ];
    let places: Vec<usize> = parts
        .iter()
        .map(|part| main.find(part).unwrap_or_else(|| panic!("missing: {part}")))
        .collect();
    assert!(places.is_sorted(), "{places:?}");
    // One form, one button, and one link to Wenmar Pro with its own marker.
    assert_eq!(main.matches("<form").count(), 1);
    assert_eq!(main.matches("<button class=\"primary\"").count(), 1);
    assert_eq!(main.matches("wenmarpro.com").count(), 1);

    // The example is labelled, in the form and at the result, and the
    // defaults are named as examples.
    assert!(main.contains("The form is filled in with an example."));
    assert!(main.contains("This is the result of the example in the form."));
    assert!(main.contains("Left empty, paid hours are 173 (40 hours a week), productivity is 85%"));
    assert!(main.contains("These are examples to start from, not advice."));
    assert!(main.contains(
        "This page does not say what a shop should charge, and says nothing of what other shops charge."
    ));
    for (name, value) in [
        ("technicians", "3"),
        ("paid_hours", "173"),
        ("productivity", "85"),
        ("technician_cost", "18000.00"),
        ("overhead", "25000.00"),
        ("parts_profit", "12000.00"),
        ("target_profit", "10"),
        ("target_labor", &LABOR.usual.input()),
        ("labor_sales", "48000.00"),
        ("hours_billed", "520"),
        ("posted_rate", "120.00"),
        ("period_cost", "18000.00"),
    ] {
        assert!(main.contains(&format!("{}>", input(name, value))), "{name}");
    }
    assert_eq!(main.matches("<input ").count(), 12);
    // No account, no email, no gate.
    assert!(!main.contains("email") && !main.contains("type=\"password\""));

    // The results are the ones the Markdown version states.
    let text = markdown(&app, "/tools/labor-rate.md").await;
    assert!(
        text.starts_with("# Labor rate calculator for auto repair shops\n"),
        "{text}"
    );
    for stated in [
        "Labor gross profit at the rate needed: 47.742%.",
        "The rate needed is the lower of the two rates: the target labor gross profit leaves more profit than was asked for.",
        "The shop is getting less for an hour billed than it posts.",
        "discounts, warranty and internal work, and jobs sold at a menu price",
        "Labor gross profit: 62.5%.",
        "Rate needed = labor sales needed ÷ hours billed.",
        "Effective labor rate = labor sales ÷ hours billed",
    ] {
        assert!(main.contains(stated), "the page: {stated}");
        assert!(text.contains(stated), "the Markdown: {stated}");
    }
    let mut rows = 0;
    for line in text
        .lines()
        .filter(|line| line.starts_with("| ") && line.contains(|c: char| c.is_ascii_digit()))
    {
        let cells: Vec<&str> = line.trim_matches('|').split('|').map(str::trim).collect();
        assert!(main.contains(&row(cells[0], cells[1])), "{line}");
        rows += 1;
    }
    assert_eq!(
        rows, 9,
        "five figures for the rate needed, four for the rate got"
    );
    assert_eq!(main.matches(r#"<th scope="row">"#).count(), 9);
    assert!(main.contains(&row("Rate needed, per hour billed", "$78.08")));
    assert!(main.contains(&row("Effective labor rate, per hour billed", "$92.31")));
    assert!(text.contains("This page describes the calculator and does not compute."));

    // A calculator shows no vehicle data, so it names no data version.
    assert!(!html.contains("Data version"), "{PAGE}");
    // The print button is there for the script to show.
    assert!(main.contains(r#"<button type="button" data-print hidden>Print</button>"#));
    // No script was added: the page loads the site's one file and nothing inline.
    assert_eq!(
        html.matches("<script").count(),
        2,
        "structured data and site.js"
    );
    assert!(!html.contains("style=") && !html.contains(" onclick="));
}

#[tokio::test]
async fn a_filled_in_form_shows_what_was_sent_and_its_results() {
    let app = common::app().await;
    let (status, html) = page(&app, FILLED).await;
    assert_eq!(status, StatusCode::OK);
    assert_basics(&html, FILLED);
    assert_targets(&html, FILLED);
    let main = main_of(&html);
    // What was typed is shown as it was typed, and a field left empty
    // shows the default that was used.
    assert!(main.contains(&format!("{}>", input("technician_cost", "$12,000"))));
    assert!(main.contains(&format!("{}>", input("productivity", "90%"))));
    assert!(main.contains(&format!("{}>", input("parts_profit", "0.00"))));
    assert!(main.contains(&format!("{}>", input("period_cost", ""))));
    assert!(
        !main.contains("filled in with an example") && !main.contains("result of the example"),
        "nothing here is the example"
    );
    for (name, amount) in [
        ("Hours billed per month", "288"),
        ("Labor sales needed per month", "$37,647.06"),
        ("Rate needed, per hour billed", "$130.72"),
        ("Break-even rate, with no profit", "$111.11"),
        ("Rate from the target labor gross profit", "$119.05"),
        ("Effective labor rate, per hour billed", "$120.00"),
        ("Effective rate as a percent of the posted rate", "109.091%"),
        ("Posted rate less effective rate, per hour", "-$10.00"),
        ("The same over the hours billed", "-$2,500.00"),
    ] {
        assert!(
            main.contains(&row(name, amount)),
            "{name}: {amount}\n{main}"
        );
    }
    assert!(main.contains(
        "The rate needed is the higher of the two rates: the shop&#39;s overhead asks for more than the target labor gross profit gives."
    ));
    assert!(main.contains(
        "<strong>Labor gross profit at the rate needed: 68.125%.</strong> That is inside the typical range"
    ));
    // The shop gets more than it posts: the page says so in words.
    assert!(main.contains(
        "The shop is getting more for an hour billed than it posts, so the two differences are below zero."
    ));
    // No technician cost for the period, so no gross profit for it.
    assert!(!main.contains("<strong>Labor gross profit:"));

    // Gross profit on parts that covers every cost: a rate of nothing,
    // and the page says why. No percent is shown beside the target.
    let (status, html) = page(
        &app,
        "/tools/labor-rate?technicians=2&technician_cost=6000&overhead=9000&parts_profit=15000",
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let main = main_of(&html);
    assert!(main.contains(
        "<p><strong>Gross profit on parts covers the technician cost and the overhead, so labor has nothing left to cover and the rate needed is $0.00.</strong></p>"
    ));
    assert!(main.contains(&row("Rate needed, per hour billed", "$0.00")));
    assert!(!main.contains("Labor gross profit at the rate needed:"));
    assert!(!main.contains("NaN"));
}

#[tokio::test]
async fn a_part_left_empty_is_not_worked_out_and_the_other_part_still_is() {
    let app = common::app().await;
    let (status, html) = page(&app, GETTING_ONLY).await;
    assert_eq!(status, StatusCode::OK);
    assert_basics(&html, GETTING_ONLY);
    let main = main_of(&html);
    // Nothing is reported for the part left empty, and none of its
    // defaults is filled in.
    assert_eq!(main.matches(r#"class="warning""#).count(), 0, "{main}");
    assert!(!main.contains("There is no result yet."));
    assert!(main.contains(
        r#"<p class="note">Nothing was entered for the rate the shop needs, so it is not worked out.</p>"#
    ));
    assert!(main.contains(&format!("{}>", input("paid_hours", ""))));
    assert!(!main.contains("<caption>The rate the shop needs</caption>"));
    // The other part has its result.
    assert!(main.contains(&row("Effective labor rate, per hour billed", "$92.31")));
    assert_eq!(main.matches(r#"<th scope="row">"#).count(), 1);

    // The first part alone.
    let path = "/tools/labor-rate?technicians=3&technician_cost=18000&overhead=25000&parts_profit=12000&labor_sales=&hours_billed=";
    let (_, html) = page(&app, path).await;
    let main = main_of(&html);
    assert_eq!(main.matches(r#"class="warning""#).count(), 0, "{main}");
    assert!(main.contains(&row("Rate needed, per hour billed", "$78.08")));
    assert!(main.contains(
        r#"<p class="note">Nothing was entered for the rate the shop is getting, so it is not worked out.</p>"#
    ));
    assert!(!main.contains("<caption>The rate the shop is getting</caption>"));

    // The form sent with nothing in it: nothing to work out, nothing to
    // correct, and still a page.
    let (status, html) = page(&app, "/tools/labor-rate?technicians=&labor_sales=").await;
    assert_eq!(status, StatusCode::OK);
    let main = main_of(&html);
    assert_eq!(main.matches("so it is not worked out.").count(), 2);
    assert_eq!(main.matches(r#"class="warning""#).count(), 0);
    assert!(!main.contains("<table") && !main.contains("filled in with an example"));
}

#[tokio::test]
async fn a_field_that_cannot_be_read_gets_a_message_and_the_rest_keep_what_was_typed() {
    let app = common::app().await;
    let path = "/tools/labor-rate?technicians=3&paid_hours=&technician_cost=18000&overhead=25%2C0&labor_sales=48000&hours_billed=abc&posted_rate=120";
    let (status, html) = page(&app, path).await;
    assert_eq!(
        status,
        StatusCode::OK,
        "a form being filled in is not a failed request"
    );
    assert_basics(&html, path);
    assert_targets(&html, path);
    let main = main_of(&html);
    // The message is beside the field, and the field points to it.
    assert!(
        main.contains(&format!(
            r#"{} aria-invalid="true" aria-describedby="overhead-error">"#,
            input("overhead", "25,0")
        )),
        "{main}"
    );
    assert!(main.contains(
        r#"<p class="warning" id="overhead-error">This is not a number. Write it like 1,234.50.</p>"#
    ));
    assert!(main.contains(
        r#"<p class="warning" id="hours_billed-error">This is not a number. Write it like 1,234.50.</p>"#
    ));
    assert_eq!(main.matches(r#"class="warning""#).count(), 2);
    // Every other field keeps what was typed, or shows its default.
    assert!(main.contains(&format!("{}>", input("technicians", "3"))));
    assert!(main.contains(&format!("{}>", input("paid_hours", "173"))));
    assert!(main.contains(&format!("{}>", input("labor_sales", "48000"))));
    assert!(main.contains(&format!("{}>", input("posted_rate", "120"))));
    // No result is shown for either part, and the reader is told where to look.
    assert!(!main.contains("<table"));
    assert_eq!(
        main.matches("<p>There is no result yet. Correct what is marked above:</p>")
            .count(),
        2
    );
    assert!(main.contains(
        r##"<li><a href="#overhead">Overhead per month: This is not a number. Write it like 1,234.50.</a></li>"##
    ));
    assert!(main.contains(
        r##"<li><a href="#hours_billed">Hours billed in the period: This is not a number. Write it like 1,234.50.</a></li>"##
    ));
    // The rest of the page is still there.
    assert!(main.contains(r#"<h2 id="how">How this is worked out</h2>"#));

    // One part that cannot be read does not take the other's result away.
    let path = "/tools/labor-rate?technicians=3&technician_cost=18000&overhead=25000&parts_profit=12000&labor_sales=48000&hours_billed=520%2C5";
    let (_, html) = page(&app, path).await;
    let main = main_of(&html);
    assert!(main.contains(&row("Rate needed, per hour billed", "$78.08")));
    assert!(!main.contains("<caption>The rate the shop is getting</caption>"));
    assert_eq!(main.matches("There is no result yet.").count(), 1);
    assert_eq!(main.matches(r#"class="warning""#).count(), 1);
}

#[tokio::test]
async fn no_technicians_and_no_hours_are_reported_beside_their_fields() {
    let app = common::app().await;
    let path = "/tools/labor-rate?technicians=0&technician_cost=18000&overhead=25000&labor_sales=48000&hours_billed=0&posted_rate=120";
    let (status, html) = page(&app, path).await;
    assert_eq!(status, StatusCode::OK);
    assert_basics(&html, path);
    let main = main_of(&html);
    assert!(main.contains(
        r#"<p class="warning" id="technicians-error">No hours would be billed with this.</p>"#
    ));
    assert!(main.contains(
        r#"<p class="warning" id="hours_billed-error">No hours would be billed with this.</p>"#
    ));
    assert!(main.contains(
        r##"<li><a href="#technicians">Technicians: No hours would be billed with this.</a></li>"##
    ));
    assert!(!main.contains("<table"));
    // A posted rate of nothing has no percent to be taken of it.
    let (status, html) = page(
        &app,
        "/tools/labor-rate?labor_sales=48000&hours_billed=520&posted_rate=0",
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert!(
        main_of(&html)
            .contains(r#"<p class="warning" id="posted_rate-error">This cannot be zero.</p>"#)
    );
    // Each field says its own limits.
    let (_, html) = page(
        &app,
        "/tools/labor-rate?technicians=2.5&productivity=500&technician_cost=1&overhead=1&target_profit=100",
    )
    .await;
    let main = main_of(&html);
    assert!(main.contains(r#"id="technicians-error">Use a whole number.</p>"#));
    assert!(main.contains(r#"id="productivity-error">This must be from 1% to 200%.</p>"#));
    assert!(main.contains(r#"id="target_profit-error">This must be from 0% to 99%.</p>"#));
}

#[tokio::test]
async fn an_address_with_a_query_string_is_not_indexed_and_names_the_bare_address() {
    let app = common::app().await;
    let canonical = r#"<link rel="canonical" href="https://open.example/tools/labor-rate">"#;
    for path in [
        FILLED,
        "/tools/labor-rate?technicians=abc",
        "/tools/labor-rate?utm_source=newsletter",
        "/tools/labor-rate?",
    ] {
        let response = app.get(path).await;
        assert_eq!(response.status(), StatusCode::OK, "{path}");
        // A shop's own figures are for its own browser.
        assert_eq!(
            header(&response, "cache-control"),
            "private, max-age=3600",
            "{path}"
        );
        assert_eq!(header(&response, "x-robots-tag"), "noindex", "{path}");
        assert!(response.headers().get("etag").is_none(), "{path}");
        let html = body_text(response).await;
        assert_head(&html, path);
        assert!(html.contains(canonical), "{path}");
        assert!(
            html.contains(r#"<meta name="robots" content="noindex">"#),
            "{path}"
        );
        assert!(json_ld(&html).is_none(), "{path}");
    }
    // A query that is not the form's leaves the example in place: a link
    // with someone's marker on it is not a form sent empty.
    for path in [
        "/tools/labor-rate?utm_source=newsletter",
        "/tools/labor-rate?",
        "/tools/labor-rate?cost1=5&kind=margin",
    ] {
        let (_, html) = page(&app, path).await;
        assert!(
            html.contains("The form is filled in with an example."),
            "{path}"
        );
        assert!(
            html.contains(&row("Rate needed, per hour billed", "$78.08")),
            "{path}"
        );
        assert!(!html.contains("so it is not worked out."), "{path}");
    }
}

#[tokio::test]
async fn nothing_typed_into_the_form_arrives_as_markup() {
    let app = common::app().await;
    let hostile =
        "%22%3E%3Cscript%3Ealert(1)%3C%2Fscript%3E%3Cimg%20src%3Dx%20onerror%3Dalert(1)%3E";
    let path = format!(
        "/tools/labor-rate?technicians={hostile}&paid_hours={hostile}&overhead={hostile}&target_labor={hostile}&labor_sales={hostile}&posted_rate={hostile}&{hostile}={hostile}"
    );
    let (status, html) = page(&app, &path).await;
    assert_eq!(status, StatusCode::OK);
    assert_no_injection(&html);
    assert_basics(&html, &path);
    assert_eq!(
        html.matches("<script").count(),
        1,
        "only the site's own file"
    );
    assert!(html.contains("value=\"&#34;&#62;&#60;script&#62;alert(1)"));
    // Figures no field accepts, text that is not UTF-8, and a very long
    // field: each is a page with a message, never an error.
    let long = "9".repeat(6_000);
    for query in [
        "technicians=99999999999999999999&hours_billed=99999999999999999999".to_owned(),
        "technicians=%ff%fe&labor_sales=%00".to_owned(),
        "technicians=1e308&paid_hours=NaN&productivity=-0".to_owned(),
        format!("overhead={long}&hours_billed=5"),
        "technicians=1000&paid_hours=99,999,999.99&productivity=200&technician_cost=99,999,999.99&overhead=99,999,999.99&target_profit=99&target_labor=99&labor_sales=99,999,999.99&hours_billed=0.01&posted_rate=99,999,999.99&period_cost=99,999,999.99".to_owned(),
        "&&&===&technicians".to_owned(),
    ] {
        let path = format!("/tools/labor-rate?{query}");
        let (status, html) = page(&app, &path).await;
        assert_eq!(status, StatusCode::OK, "{:.80}", path);
        assert!(
            html.contains(r#"<h2 id="result">Result</h2>"#),
            "{:.80}",
            path
        );
    }
}

#[tokio::test]
async fn the_labor_target_is_shown_with_a_link_to_each_of_its_sources() {
    let app = common::app().await;
    let sources = LABOR.range_sources.len() + LABOR.usual_sources.len();
    // The example shows the target twice, once for each part; the second
    // address shows it once.
    for (path, shown) in [(PAGE, 2), (FILLED, 1)] {
        let (_, html) = page(&app, path).await;
        let main = main_of(&html);
        assert_eq!(
            main.matches(&format!(
                "the typical range of {} to {} for labor gross profit. The usual target is {}.",
                LABOR.range.low, LABOR.range.high, LABOR.usual
            ))
            .count(),
            shown,
            "{path}"
        );
        for source in LABOR.range_sources.iter().chain(LABOR.usual_sources) {
            assert_eq!(
                main.matches(&format!(r#"<a href="{}">{}</a>"#, source.url, source.name))
                    .count(),
                shown,
                "{path}: {}",
                source.url
            );
        }
        assert_eq!(
            main.matches("These figures are for a general repair shop.")
                .count(),
            shown
        );
        assert_eq!(
            main.matches("Neither is a survey of what shops earn.")
                .count(),
            shown
        );
        // The only addresses off the site are the sources and Wenmar Pro.
        let mut outside = 0;
        for link in main.split(r#"href="https://"#).skip(1) {
            if link.starts_with("open.example/") {
                continue;
            }
            outside += 1;
            let known = link.starts_with("wenmarpro.com/")
                || LABOR
                    .range_sources
                    .iter()
                    .chain(LABOR.usual_sources)
                    .any(|source| link.starts_with(source.url.trim_start_matches("https://")));
            assert!(known, "{path}: {:.80}", link);
        }
        assert_eq!(outside, 1 + shown * sources, "{path}");
    }
    // Below and above are said in words, and nothing is marked as a failure.
    let (_, html) = page(&app, PAGE).await;
    let main = main_of(&html);
    assert!(main.contains(
        "<strong>Labor gross profit at the rate needed: 47.742%.</strong> That is below the typical range"
    ));
    assert!(
        main.contains(
            "<strong>Labor gross profit: 62.5%.</strong> That is inside the typical range"
        )
    );
    assert_eq!(main.matches(r#"class="warning""#).count(), 0);
    let (_, html) = page(
        &app,
        "/tools/labor-rate?labor_sales=48000&hours_billed=520&period_cost=9000",
    )
    .await;
    assert!(
        html.contains(
            "<strong>Labor gross profit: 81.25%.</strong> That is above the typical range"
        )
    );
}

#[tokio::test]
async fn the_labor_rate_page_is_on_the_tools_index_with_its_sentence() {
    let app = common::app().await;
    let (_, html) = page(&app, "/tools").await;
    assert!(html.contains(
        r#"<h2 id="labor-rate-calculator-for-auto-repair-shops">Labor rate calculator for auto repair shops</h2>"#
    ));
    assert!(html.contains("<p>Works out the hourly labor rate a repair shop needs"));
    assert!(html.contains(
        r#"<li><a href="https://open.example/tools/labor-rate">Labor rate calculator for auto repair shops</a></li>"#
    ));
    // It comes after the parts matrix, in the order the calculators were made.
    let parts = html
        .find(r#"<h2 id="parts-markup-matrix-calculator">"#)
        .unwrap();
    let labor = html
        .find(r#"<h2 id="labor-rate-calculator-for-auto-repair-shops">"#)
        .unwrap();
    assert!(parts < labor);
    let text = markdown(&app, "/tools.md").await;
    assert!(text.contains(
        "- [Labor rate calculator for auto repair shops](https://open.example/tools/labor-rate)\n"
    ));
    // The Markdown version is served, and an address under the page is not.
    assert_eq!(
        app.get("/tools/labor-rate.md").await.status(),
        StatusCode::OK
    );
    for path in [
        "/tools/labor-rate/extra",
        "/tools/LABOR-RATE",
        "/tools/labor_rate",
    ] {
        assert_eq!(
            app.get(path).await.status(),
            StatusCode::NOT_FOUND,
            "{path}"
        );
    }
}
