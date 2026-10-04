//! The gross profit check, `/tools/gross-profit`, through HTTP.

use axum::http::StatusCode;
use shop_math::targets::{LABOR, OVERALL, PARTS};

use crate::common::{
    self, assert_basics, assert_head, assert_no_injection, assert_targets, body_text, header,
    json_ld, markdown, page,
};

const PAGE: &str = "/tools/gross-profit";

/// A month with labor below its range, parts above theirs, and the sublet
/// fields sent empty.
const FILLED: &str = "/tools/gross-profit?labor_sales=%2460%2C000&technician_cost=30000&parts_sales=45000&parts_cost=20000&sublet_sales=&sublet_cost=";

/// The part of a page between `<main>` and `</main>`.
fn main_of(html: &str) -> &str {
    let main = html.split(r#"<main id="main">"#).nth(1).unwrap();
    main.split("</main>").next().unwrap()
}

/// A row of the result table, as the page writes it.
fn row(cells: [&str; 5]) -> String {
    format!(
        r#"<tr><th scope="row">{}</th>{}</tr>"#,
        cells[0],
        cells[1..]
            .iter()
            .map(|cell| format!("<td>{cell}</td>"))
            .collect::<String>()
    )
}

/// A box of the form with what it holds, as the page writes it.
fn input(name: &str, value: &str) -> String {
    format!(
        r#"<input id="{name}" name="{name}" type="text" inputmode="decimal" autocomplete="off" value="{value}""#
    )
}

#[tokio::test]
async fn the_gross_profit_page_opens_with_a_labelled_example_and_its_result() {
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
        html.contains("<title>Gross profit calculator for auto repair shops - Wenmar Open</title>")
    );
    assert!(
        html.contains(r#"<link rel="canonical" href="https://open.example/tools/gross-profit">"#)
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

    // The seven parts of a calculator page, in order.
    let main = main_of(&html);
    let parts = [
        "<h1>Gross profit calculator for auto repair shops</h1>",
        "<p>Works out a repair shop&#39;s gross profit on labor, on parts, on sublet work",
        r##"<form class="calc" id="calc" action="/tools/gross-profit#result" method="get">"##,
        r#"<h2 id="result">Result</h2>"#,
        "<caption>Gross profit for the period</caption>",
        r#"<h2 id="how">How this is worked out</h2>"#,
        r#"<p class="note">This is arithmetic, not accounting or tax advice.</p>"#,
        r#"<p class="pro"><a href="https://wenmarpro.com/?utm_source=wenmar-open&#38;utm_medium=referral&#38;utm_campaign=tool-gross-profit">Wenmar Pro</a> is shop management software from the people who make this site.</p>"#,
    ];
    let places: Vec<usize> = parts
        .iter()
        .map(|part| main.find(part).unwrap_or_else(|| panic!("missing: {part}")))
        .collect();
    assert!(places.is_sorted(), "{places:?}");
    // One link to Wenmar Pro in the page's own part, with its own marker.
    assert_eq!(main.matches("wenmarpro.com").count(), 1);

    // The example is labelled, in the form and at the result.
    assert!(main.contains("The form is filled in with an example."));
    assert!(main.contains("This is the result of the example in the form."));
    for (name, value) in [
        ("labor_sales", "48000.00"),
        ("technician_cost", "18000.00"),
        ("parts_sales", "40000.00"),
        ("parts_cost", "26000.00"),
        ("sublet_sales", "2000.00"),
        ("sublet_cost", "1600.00"),
    ] {
        assert!(main.contains(&format!("{}>", input(name, value))), "{name}");
    }
    assert_eq!(main.matches("<input ").count(), 6);
    assert_eq!(main.matches(r#"class="primary""#).count(), 1);
    assert!(main.contains(r#"<button class="primary" type="submit">Work it out</button>"#));
    // No account, no email, no gate.
    assert!(!main.contains("email") && !main.contains("type=\"password\""));

    // The result is the one the Markdown version states.
    let text = markdown(&app, "/tools/gross-profit.md").await;
    assert!(
        text.starts_with("# Gross profit calculator for auto repair shops\n"),
        "{text}"
    );
    for stated in [
        "Labor gross profit: 62.5%.",
        "Parts gross profit: 35%.",
        "Parts are below the typical range. At the same parts cost of $26,000.00, sales of $52,000.00 would reach the usual target of 50%. That is $12,000.00 more than the $40,000.00 sold.",
        "Overall gross profit: 49.333%.",
        "Sublet has no target here, so its figure is shown alone.",
        "Gross profit = sales \u{2212} cost.",
        "sales needed = cost \u{f7} (1 \u{2212} usual target)",
    ] {
        assert!(main.contains(stated), "the page: {stated}");
        assert!(text.contains(stated), "the Markdown: {stated}");
    }
    let mut rows = 0;
    for line in text
        .lines()
        .filter(|line| line.starts_with("| ") && line.contains('$'))
    {
        let cells: Vec<&str> = line.trim_matches('|').split('|').map(str::trim).collect();
        assert!(
            main.contains(&row([cells[0], cells[1], cells[2], cells[3], cells[4]])),
            "{line}"
        );
        rows += 1;
    }
    assert_eq!(rows, 4, "labor, parts, sublet and all three");
    assert_eq!(main.matches(r#"<th scope="row">"#).count(), 4);
    assert!(main.contains(&row([
        "All three",
        "$90,000.00",
        "$45,600.00",
        "$44,400.00",
        "49.333%"
    ])));
    // Labor is inside its range in the example, so only parts has the
    // figure to act on.
    assert!(!main.contains("Labor is below"));
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
async fn a_filled_in_form_shows_what_was_sent_and_its_result() {
    let app = common::app().await;
    let (status, html) = page(&app, FILLED).await;
    assert_eq!(status, StatusCode::OK);
    assert_basics(&html, FILLED);
    assert_targets(&html, FILLED);
    let main = main_of(&html);
    // What was typed is shown as it was typed, and sublet left empty shows
    // the nothing that was used.
    assert!(main.contains(&format!("{}>", input("labor_sales", "$60,000"))));
    assert!(main.contains(&format!("{}>", input("sublet_sales", "0.00"))));
    assert!(main.contains(&format!("{}>", input("sublet_cost", "0.00"))));
    assert!(
        !main.contains("filled in with an example") && !main.contains("result of the example"),
        "nothing here is the example"
    );
    for cells in [
        ["Labor", "$60,000.00", "$30,000.00", "$30,000.00", "50%"],
        ["Parts", "$45,000.00", "$20,000.00", "$25,000.00", "55.556%"],
        // A line with no sales shows no percent.
        ["Sublet", "$0.00", "$0.00", "$0.00", "No sales"],
        [
            "All three",
            "$105,000.00",
            "$50,000.00",
            "$55,000.00",
            "52.381%",
        ],
    ] {
        assert!(main.contains(&row(cells)), "{cells:?}\n{main}");
    }
    assert!(
        main.contains("<strong>Labor gross profit: 50%.</strong> That is below the typical range")
    );
    // The one figure an owner can act on, for the line below its range.
    assert!(main.contains(
        "<p><strong>Labor is below the typical range. At the same technician cost of $30,000.00, sales of $100,000.00 would reach the usual target of 70%. That is $40,000.00 more than the $60,000.00 sold.</strong></p>"
    ));
    assert!(
        main.contains(
            "<strong>Parts gross profit: 55.556%.</strong> That is above the typical range"
        )
    );
    assert!(!main.contains("Parts are below"));
    assert!(main.contains(
        "<strong>Overall gross profit: 52.381%.</strong> That is inside the typical range"
    ));
    // Below and above are said in words, and nothing is marked as a failure.
    assert_eq!(main.matches(r#"class="warning""#).count(), 0);
}

#[tokio::test]
async fn a_line_with_no_sales_or_a_loss_is_shown_and_is_not_an_error() {
    let app = common::app().await;
    // A shop that sold labor only: parts typed as zero, sublet not sent.
    let path =
        "/tools/gross-profit?labor_sales=48000&technician_cost=18000&parts_sales=0&parts_cost=0";
    let (status, html) = page(&app, path).await;
    assert_eq!(status, StatusCode::OK);
    assert_basics(&html, path);
    let main = main_of(&html);
    assert!(main.contains(&row(["Parts", "$0.00", "$0.00", "$0.00", "No sales"])));
    assert!(main.contains(&row([
        "All three",
        "$48,000.00",
        "$18,000.00",
        "$30,000.00",
        "62.5%"
    ])));
    assert!(!main.contains("Parts gross profit:") && !main.contains("Parts are below"));
    assert!(main.contains("<strong>Overall gross profit: 62.5%.</strong> That is above"));
    assert_eq!(main.matches(r#"class="warning""#).count(), 0);
    assert!(!html.contains("NaN"));

    // Technicians who cost more than the labor sold: a loss, a percent
    // below zero, and still the sales the usual target would take.
    let path = "/tools/gross-profit?labor_sales=10000&technician_cost=12000&parts_sales=30000&parts_cost=12000";
    let (status, html) = page(&app, path).await;
    assert_eq!(status, StatusCode::OK);
    let main = main_of(&html);
    assert!(main.contains(&row([
        "Labor",
        "$10,000.00",
        "$12,000.00",
        "-$2,000.00",
        "-20%"
    ])));
    assert!(
        main.contains("<strong>Labor gross profit: -20%.</strong> That is below the typical range")
    );
    assert!(main.contains("sales of $40,000.00 would reach the usual target of 70%."));
    assert_eq!(main.matches(r#"class="warning""#).count(), 0);
}

#[tokio::test]
async fn a_field_that_cannot_be_read_gets_a_message_and_the_rest_keep_what_was_typed() {
    let app = common::app().await;
    let path = "/tools/gross-profit?labor_sales=48%2C0&technician_cost=18000&parts_sales=&parts_cost=26000&sublet_sales=abc&sublet_cost=";
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
            r#"{} aria-invalid="true" aria-describedby="labor_sales-error">"#,
            input("labor_sales", "48,0")
        )),
        "{main}"
    );
    assert!(main.contains(
        r#"<p class="warning" id="labor_sales-error">This is not a number. Write it like 1,234.50.</p>"#
    ));
    assert!(main.contains(r#"<p class="warning" id="parts_sales-error">Enter a number.</p>"#));
    assert!(main.contains(
        r#"<p class="warning" id="sublet_sales-error">This is not a number. Write it like 1,234.50.</p>"#
    ));
    assert_eq!(main.matches(r#"class="warning""#).count(), 3);
    // Every other field keeps what was typed, or shows its default.
    assert!(main.contains(&format!("{}>", input("technician_cost", "18000"))));
    assert!(main.contains(&format!("{}>", input("parts_cost", "26000"))));
    assert!(main.contains(&format!("{}>", input("sublet_cost", "0.00"))));
    // No result is shown, and the reader is told where to look.
    assert!(!main.contains("<table") && !main.contains("gross profit:"));
    assert!(main.contains("<p>There is no result yet. Correct what is marked above:</p>"));
    assert!(main.contains(
        r##"<li><a href="#labor_sales">Labor sales: This is not a number. Write it like 1,234.50.</a></li>"##
    ));
    assert!(main.contains(r##"<li><a href="#parts_sales">Parts sales: Enter a number.</a></li>"##));
    // The rest of the page is still there.
    assert!(main.contains(r#"<h2 id="how">How this is worked out</h2>"#));
}

#[tokio::test]
async fn no_sales_on_any_line_is_reported_at_the_form() {
    let app = common::app().await;
    for path in [
        "/tools/gross-profit?labor_sales=0&technician_cost=0&parts_sales=0&parts_cost=0",
        "/tools/gross-profit?labor_sales=0&technician_cost=18000&parts_sales=0&parts_cost=500&sublet_sales=&sublet_cost=20",
    ] {
        let (status, html) = page(&app, path).await;
        assert_eq!(status, StatusCode::OK, "{path}");
        assert_basics(&html, path);
        let main = main_of(&html);
        assert!(
            main.contains(
                r#"<p class="warning" id="calc-error">Enter sales for labor, parts or sublet.</p>"#
            ),
            "{path}"
        );
        assert_eq!(main.matches(r#"class="warning""#).count(), 1, "{path}");
        assert!(
            main.contains(
                r##"<li><a href="#calc">Enter sales for labor, parts or sublet.</a></li>"##
            )
        );
        assert!(!main.contains("<table"), "{path}");
    }
}

#[tokio::test]
async fn an_address_with_a_query_string_is_not_indexed_and_names_the_bare_address() {
    let app = common::app().await;
    let canonical = r#"<link rel="canonical" href="https://open.example/tools/gross-profit">"#;
    for path in [
        FILLED,
        "/tools/gross-profit?labor_sales=abc",
        "/tools/gross-profit?utm_source=newsletter",
        "/tools/gross-profit?",
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
        "/tools/gross-profit?utm_source=newsletter",
        "/tools/gross-profit?",
        "/tools/gross-profit?cost1=5&kind=margin",
    ] {
        let (_, html) = page(&app, path).await;
        assert!(
            html.contains("The form is filled in with an example."),
            "{path}"
        );
        assert!(html.contains("Overall gross profit: 49.333%."), "{path}");
        assert!(!html.contains("Enter sales for labor"), "{path}");
    }
}

#[tokio::test]
async fn nothing_typed_into_the_form_arrives_as_markup() {
    let app = common::app().await;
    let hostile =
        "%22%3E%3Cscript%3Ealert(1)%3C%2Fscript%3E%3Cimg%20src%3Dx%20onerror%3Dalert(1)%3E";
    let path = format!(
        "/tools/gross-profit?labor_sales={hostile}&technician_cost={hostile}&parts_sales={hostile}&parts_cost={hostile}&sublet_sales={hostile}&{hostile}={hostile}"
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
    // Figures no field accepts, text that is not UTF-8, a very long field,
    // the largest figures and the largest loss: each is a page, never an
    // error.
    let long = "9".repeat(6_000);
    for query in [
        "labor_sales=99999999999999999999&technician_cost=99999999999999999999".to_owned(),
        "labor_sales=%ff%fe&parts_sales=%00".to_owned(),
        "labor_sales=1e308&technician_cost=NaN&parts_sales=-0".to_owned(),
        format!("labor_sales={long}&technician_cost=5"),
        "labor_sales=99,999,999.99&technician_cost=0&parts_sales=99,999,999.99&parts_cost=99,999,999.99&sublet_sales=99,999,999.99&sublet_cost=0.01".to_owned(),
        "labor_sales=0.01&technician_cost=99,999,999.99&parts_sales=0&parts_cost=0".to_owned(),
        "&&&===&labor_sales".to_owned(),
    ] {
        let path = format!("/tools/gross-profit?{query}");
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
async fn each_target_is_shown_with_a_link_to_each_of_its_sources() {
    let app = common::app().await;
    let (_, html) = page(&app, PAGE).await;
    let main = main_of(&html);
    let mut links = 0;
    for (of, target) in [
        ("labor gross profit", &LABOR),
        ("parts gross profit", &PARTS),
        ("gross profit across labor, parts and sublet", &OVERALL),
    ] {
        assert!(
            main.contains(&format!(
                "the typical range of {} to {} for {of}. The usual target is {}.",
                target.range.low, target.range.high, target.usual
            )),
            "{of}"
        );
        for source in target.range_sources.iter().chain(target.usual_sources) {
            assert!(
                main.contains(&format!(r#"<a href="{}">{}</a>"#, source.url, source.name)),
                "{of}: {}",
                source.url
            );
            links += 1;
        }
    }
    // Each of the three says what its figures are, and are not.
    assert_eq!(
        main.matches("These figures are for a general repair shop.")
            .count(),
        3
    );
    assert_eq!(
        main.matches("Neither is a survey of what shops earn.")
            .count(),
        3
    );
    // Sublet has no target, and the page says so.
    assert!(main.contains(
        r#"<p class="note">Sublet has no target here, so its figure is shown alone.</p>"#
    ));
    assert!(!main.contains("Sublet gross profit:"));
    // The only addresses off the site are the sources and Wenmar Pro.
    let mut outside = 0;
    for link in main.split(r#"href="https://"#).skip(1) {
        if link.starts_with("open.example/") {
            continue;
        }
        outside += 1;
        let known = link.starts_with("wenmarpro.com/")
            || [&LABOR, &PARTS, &OVERALL].iter().any(|target| {
                target
                    .range_sources
                    .iter()
                    .chain(target.usual_sources)
                    .any(|source| link.starts_with(source.url.trim_start_matches("https://")))
            });
        assert!(known, "{:.80}", link);
    }
    assert_eq!(outside, 1 + links);
}

#[tokio::test]
async fn the_gross_profit_page_is_on_the_tools_index_with_its_sentence() {
    let app = common::app().await;
    let (_, html) = page(&app, "/tools").await;
    assert!(html.contains(
        r#"<h2 id="gross-profit-calculator-for-auto-repair-shops">Gross profit calculator for auto repair shops</h2>"#
    ));
    assert!(html.contains("<p>Works out a repair shop&#39;s gross profit on labor"));
    assert!(html.contains(
        r#"<li><a href="https://open.example/tools/gross-profit">Gross profit calculator for auto repair shops</a></li>"#
    ));
    // It comes after the parts matrix, in the order the calculators were made.
    let parts = html
        .find(r#"<h2 id="parts-markup-matrix-calculator">"#)
        .unwrap();
    let gross = html
        .find(r#"<h2 id="gross-profit-calculator-for-auto-repair-shops">"#)
        .unwrap();
    assert!(parts < gross);
    let text = markdown(&app, "/tools.md").await;
    assert!(text.contains(
        "- [Gross profit calculator for auto repair shops](https://open.example/tools/gross-profit)\n"
    ));
    // The Markdown version is served, and an address under the page is not.
    assert_eq!(
        app.get("/tools/gross-profit.md").await.status(),
        StatusCode::OK
    );
    for path in [
        "/tools/gross-profit/extra",
        "/tools/GROSS-PROFIT",
        "/tools/gross_profit",
    ] {
        assert_eq!(
            app.get(path).await.status(),
            StatusCode::NOT_FOUND,
            "{path}"
        );
    }
}
