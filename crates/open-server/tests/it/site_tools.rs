//! The calculators under `/tools`: the index, and the parts matrix page.

use axum::http::StatusCode;
use shop_math::targets::PARTS;

use crate::common::{
    self, assert_basics, assert_head, assert_no_injection, assert_targets, body_text, header,
    json_ld, markdown, page,
};

const PAGE: &str = "/tools/parts-matrix";

/// A matrix of two rows with shares, and one part to price.
const FILLED: &str = "/tools/parts-matrix?kind=markup&cost1=25.00&rate1=100&share1=40&cost2=%241%2C000&rate2=50%25&share2=60&part=42.50";

/// The part of a page between `<main>` and `</main>`.
fn main_of(html: &str) -> &str {
    let main = html.split(r#"<main id="main">"#).nth(1).unwrap();
    main.split("</main>").next().unwrap()
}

#[tokio::test]
async fn the_tools_index_lists_each_calculator_and_has_a_markdown_version() {
    let app = common::app().await;
    let (status, html) = page(&app, "/tools").await;
    assert_eq!(status, StatusCode::OK);
    assert_basics(&html, "/tools");
    assert_head(&html, "/tools");
    assert_targets(&html, "/tools");
    assert!(html.contains("<h1>Shop calculators</h1>"));
    assert!(html.contains("<title>Free calculators for auto repair shops - Wenmar Open</title>"));
    assert!(html.contains(r#"<link rel="canonical" href="https://open.example/tools">"#));
    assert!(!html.contains(r#"name="robots""#), "the index is indexed");
    assert!(html.contains(r#"<a href="/tools" aria-current="page">Tools</a>"#));
    // The calculator's name, its one sentence, and the way to it.
    assert!(html.contains(
        r#"<h2 id="parts-markup-matrix-calculator">Parts markup matrix calculator</h2>"#
    ));
    assert!(html.contains("<p>Works out what a part sells for under a parts matrix"));
    assert!(html.contains(
        r#"<li><a href="https://open.example/tools/parts-matrix">Parts markup matrix calculator</a></li>"#
    ));
    // Only calculators that exist are listed.
    for missing in ["gross-profit", "canada-invoice-tax"] {
        assert!(!html.contains(missing), "{missing}");
    }
    // No account, no email, no pop-up: the page has no field at all.
    assert!(!html.contains("<input") && !html.contains("<form"));
    let text = markdown(&app, "/tools.md").await;
    assert!(text.starts_with("# Shop calculators\n"), "{text}");
    assert!(
        text.contains(
            "- [Parts markup matrix calculator](https://open.example/tools/parts-matrix)\n"
        )
    );
}

#[tokio::test]
async fn the_header_has_six_entries_and_tools_is_the_second() {
    let app = common::app().await;
    for path in [
        "/",
        "/makes",
        "/tools",
        PAGE,
        "/docs",
        "/vin/KM8K2CAB4PU001140",
        "/nothing",
    ] {
        let (_, html) = page(&app, path).await;
        let nav = html.split(r#"<nav aria-label="Site">"#).nth(1).unwrap();
        let nav = nav.split("</nav>").next().unwrap();
        let names: Vec<&str> = nav
            .split("</a>")
            .filter_map(|link| link.rsplit('>').next())
            .filter(|name| !name.trim().is_empty())
            .collect();
        assert_eq!(
            names,
            ["Makes", "Tools", "VIN guide", "API", "Data", "About"],
            "{path}"
        );
        assert!(nav.contains(r#"<a href="/tools""#), "{path}");
    }
}

#[tokio::test]
async fn the_parts_matrix_page_opens_with_a_labelled_example_and_its_result() {
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
    assert!(html.contains("<title>Parts markup matrix calculator - Wenmar Open</title>"));
    assert!(
        html.contains(r#"<link rel="canonical" href="https://open.example/tools/parts-matrix">"#)
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
    assert_eq!(
        data["@graph"][0]["itemListElement"][0]["item"],
        "https://open.example/tools"
    );

    // The seven parts of a calculator page, in order.
    let main = main_of(&html);
    let parts = [
        "<h1>Parts markup matrix calculator</h1>",
        "<p>Works out what a part sells for under a parts matrix",
        r##"<form class="calc" id="calc" action="/tools/parts-matrix#result" method="get">"##,
        r#"<h2 id="result">Result</h2>"#,
        r#"<h2 id="how">How this is worked out</h2>"#,
        r#"<p class="note">This is arithmetic, not accounting or tax advice.</p>"#,
        r#"<p class="pro"><a href="https://wenmarpro.com/?utm_source=wenmar-open&#38;utm_medium=referral&#38;utm_campaign=tool-parts-matrix">Wenmar Pro</a> applies a parts matrix to every estimate.</p>"#,
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
    assert!(main.contains(r#"<input id="cost1" name="cost1" type="text" inputmode="decimal" autocomplete="off" value="5.00">"#));
    assert!(main.contains(
        r#"id="share6" name="share6" type="text" inputmode="decimal" autocomplete="off" value="5">"#
    ));
    assert!(main.contains(
        r#"id="cost7" name="cost7" type="text" inputmode="decimal" autocomplete="off" value="">"#
    ));
    assert!(main.contains(
        r#"id="part" name="part" type="text" inputmode="decimal" autocomplete="off" value="42.50">"#
    ));
    assert!(main.contains(r#"<option value="markup" selected>"#));
    assert_eq!(
        main.matches("<input ").count(),
        25,
        "eight rows of three, and the part"
    );
    assert_eq!(main.matches(r#"class="primary""#).count(), 1);
    assert!(main.contains(r#"<button class="primary" type="submit">Work it out</button>"#));
    // No account, no email, no gate.
    assert!(!main.contains("email") && !main.contains("type=\"password\""));

    // The result is the one the Markdown version states.
    let text = markdown(&app, "/tools/parts-matrix.md").await;
    assert!(
        text.starts_with("# Parts markup matrix calculator\n"),
        "{text}"
    );
    for stated in [
        "A part that costs $42.50 is in row 3. It sells for $76.50, a profit of $34.00.",
        "Blended margin: 43.662%.",
        "That is inside the typical range of 40% to 50% for parts gross profit. The usual target is 50%.",
        "A margin of 50% is a markup of 100%",
        "price = cost \u{d7} (1 + markup)",
    ] {
        assert!(main.contains(stated), "the page: {stated}");
        assert!(text.contains(stated), "the Markdown: {stated}");
    }
    for row in text
        .lines()
        .filter(|line| line.starts_with("| ") && line.contains('$'))
    {
        let cells: Vec<&str> = row.trim_matches('|').split('|').map(str::trim).collect();
        let html_row = format!(
            r#"<tr><th scope="row">{}</th>{}</tr>"#,
            cells[0],
            cells[1..]
                .iter()
                .map(|cell| format!("<td>{cell}</td>"))
                .collect::<String>()
        );
        assert!(main.contains(&html_row), "{html_row}");
    }
    assert_eq!(main.matches(r#"<th scope="row">"#).count(), 6);
    // The Markdown version says it does not compute.
    assert!(text.contains("This page describes the calculator and does not compute."));

    // A calculator shows no vehicle data, so it names no data version.
    assert!(!html.contains("Data version"), "{PAGE}");
    let (_, home) = page(&app, "/").await;
    assert!(home.contains("<p>Data version 2026.09, from NHTSA vPIC.</p>"));
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
    // What was typed is shown as it was typed.
    assert!(main.contains(r#"id="cost2" name="cost2" type="text" inputmode="decimal" autocomplete="off" value="$1,000">"#));
    assert!(main.contains(
        r#"id="rate2" name="rate2" type="text" inputmode="decimal" autocomplete="off" value="50%">"#
    ));
    assert!(
        !main.contains("filled in with an example") && !main.contains("result of the example"),
        "nothing here is the example"
    );
    assert!(main.contains(
        r#"<tr><th scope="row">2</th><td>$25.01 and up</td><td>50%</td><td>33.333%</td><td>$1,000.00</td><td>$1,500.00</td><td>$500.00</td><td>60%</td></tr>"#
    ));
    assert!(main.contains(
        "A part that costs $42.50 is in row 2. It sells for $63.75, a profit of $21.25."
    ));
    assert!(
        main.contains("<strong>Blended margin: 41.176%.</strong> That is inside the typical range")
    );
    assert!(main.contains("Row 2 is the last row used, so it has no upper limit"));

    // The same figures read as margins.
    let (_, html) = page(
        &app,
        &FILLED
            .replace("kind=markup", "kind=margin")
            .replace("rate1=100", "rate1=50"),
    )
    .await;
    let main = main_of(&html);
    assert!(main.contains(r#"<option value="margin" selected>"#));
    assert!(main.contains(
        r#"<tr><th scope="row">1</th><td>$0.00 to $25.00</td><td>100%</td><td>50%</td><td>$25.00</td><td>$50.00</td><td>$25.00</td><td>40%</td></tr>"#
    ));
    assert!(main.contains("It sells for $85.00, a profit of $42.50."));

    // With no shares there is no blended margin, and the page says how to
    // get one. With no part, none is priced.
    let (_, html) = page(&app, "/tools/parts-matrix?cost3=100&rate3=60").await;
    let main = main_of(&html);
    assert!(main.contains("Row 3 is the only row used"));
    assert!(main.contains("Fill in the share column to see the blended margin"));
    assert!(!main.contains("Blended margin:") && !main.contains("It sells for"));
    assert!(!main.contains("Share of spend"));
}

#[tokio::test]
async fn a_field_that_cannot_be_read_gets_a_message_and_the_rest_keep_what_was_typed() {
    let app = common::app().await;
    let path = "/tools/parts-matrix?kind=margin&cost1=1%2C5&rate1=40&share1=abc&cost2=100&rate2=30&part=-4";
    let (status, html) = page(&app, path).await;
    assert_eq!(
        status,
        StatusCode::OK,
        "a form being filled in is not a failed request"
    );
    assert_basics(&html, path);
    assert_targets(&html, path);
    let main = main_of(&html);
    // The message is beside the row, and the field points to it.
    assert!(
        main.contains(r#"value="1,5" aria-invalid="true" aria-describedby="cost1-error">"#),
        "{main}"
    );
    assert!(main.contains(
        r#"<p class="warning" id="cost1-error">Row 1, cost up to: This is not a number. Write it like 1,234.50.</p>"#
    ));
    assert!(main.contains(
        r#"<p class="warning" id="share1-error">Row 1, share: This is not a number. Write it like 1,234.50.</p>"#
    ));
    assert!(main.contains(r#"<p class="warning" id="part-error">This cannot be below zero.</p>"#));
    assert_eq!(main.matches(r#"class="warning""#).count(), 3);
    // Every other field keeps what was typed.
    assert!(main.contains(
        r#"id="rate1" name="rate1" type="text" inputmode="decimal" autocomplete="off" value="40">"#
    ));
    assert!(main.contains(
        r#"id="cost2" name="cost2" type="text" inputmode="decimal" autocomplete="off" value="100">"#
    ));
    assert!(main.contains(r#"<option value="margin" selected>"#));
    // No result is shown, and the reader is told where to look.
    assert!(!main.contains("<table") && !main.contains("Blended margin:"));
    assert!(main.contains("<p>There is no result yet. Correct what is marked above:</p>"));
    assert!(main.contains(
        r##"<li><a href="#cost1">Row 1, cost up to: This is not a number. Write it like 1,234.50.</a></li>"##
    ));
    assert!(main.contains(r##"<li><a href="#part">One part&#39;s cost (optional): This cannot be below zero.</a></li>"##));
    // The rest of the page is still there.
    assert!(main.contains(r#"<h2 id="how">How this is worked out</h2>"#));
}

#[tokio::test]
async fn a_matrix_that_is_refused_says_why_and_names_the_row() {
    let app = common::app().await;
    let cases = [
        (
            "/tools/parts-matrix?cost2=100&rate2=50&cost5=100&rate5=40",
            r#"<p class="warning" id="cost5-error">Row 5: &#34;cost up to&#34; must be higher than in the row above.</p>"#,
            "#cost5",
        ),
        (
            "/tools/parts-matrix?kind=margin&cost1=100&rate1=100",
            r#"<p class="warning" id="rate1-error">Row 1: a margin must be below 100%.</p>"#,
            "#rate1",
        ),
        (
            "/tools/parts-matrix?cost1=10&rate1=100&share1=33.333&cost2=20&rate2=80&share2=33.333&cost3=30&rate3=60&share3=33.333",
            r#"<p class="warning" id="calc-error">The shares of parts spend add to 99.999%. They must add to 100%.</p>"#,
            "#calc",
        ),
        (
            "/tools/parts-matrix?kind=markup&cost1=&rate1=&part=",
            r#"<p class="warning" id="calc-error">Enter at least one tier: a cost and a percent.</p>"#,
            "#calc",
        ),
    ];
    for (path, message, place) in cases {
        let (status, html) = page(&app, path).await;
        assert_eq!(status, StatusCode::OK, "{path}");
        assert_basics(&html, path);
        let main = main_of(&html);
        assert!(main.contains(message), "{path}:\n{main}");
        assert!(
            main.contains(&format!(r#"<li><a href="{place}">"#)),
            "{path}"
        );
        assert!(!main.contains("<table"), "{path}");
    }
}

#[tokio::test]
async fn an_address_with_a_query_string_is_not_indexed_and_names_the_bare_address() {
    let app = common::app().await;
    let canonical = r#"<link rel="canonical" href="https://open.example/tools/parts-matrix">"#;
    for path in [
        FILLED,
        "/tools/parts-matrix?cost1=abc",
        "/tools/parts-matrix?utm_source=newsletter",
        "/tools/parts-matrix?",
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
        "/tools/parts-matrix?utm_source=newsletter",
        "/tools/parts-matrix?",
        "/tools/parts-matrix?cost9=5",
    ] {
        let (_, html) = page(&app, path).await;
        assert!(
            html.contains("The form is filled in with an example."),
            "{path}"
        );
        assert!(html.contains("Blended margin: 43.662%."), "{path}");
        assert!(!html.contains("Enter at least one tier"), "{path}");
    }
    // A page that has not changed is not sent twice, when it is the bare one.
    let response = app.get(PAGE).await;
    let etag = header(&response, "etag").to_owned();
    let request = axum::http::Request::get(PAGE)
        .header("if-none-match", etag)
        .body(axum::body::Body::empty())
        .unwrap();
    assert_eq!(app.send(request).await.status(), StatusCode::NOT_MODIFIED);
}

#[tokio::test]
async fn nothing_typed_into_the_form_arrives_as_markup() {
    let app = common::app().await;
    let hostile =
        "%22%3E%3Cscript%3Ealert(1)%3C%2Fscript%3E%3Cimg%20src%3Dx%20onerror%3Dalert(1)%3E";
    let path = format!(
        "/tools/parts-matrix?kind={hostile}&cost1={hostile}&rate1={hostile}&share1={hostile}&part={hostile}&{hostile}={hostile}"
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
        "cost1=99999999999999999999&rate1=99999999999999999999".to_owned(),
        "cost1=%ff%fe&rate1=%00".to_owned(),
        "cost1=1e308&rate1=NaN&share1=-0".to_owned(),
        format!("cost1={long}&rate1=5"),
        "kind=margin&cost1=99,999,999.99&rate1=99.999&part=99,999,999.99".to_owned(),
        "&&&===&cost1".to_owned(),
    ] {
        let path = format!("/tools/parts-matrix?{query}");
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
async fn each_preset_is_a_link_that_fills_the_form() {
    let app = common::app().await;
    let (_, html) = page(&app, PAGE).await;
    let main = main_of(&html);
    // Three links above the form, each labelled an example.
    let list = main.split("<form").next().unwrap();
    let list = list
        .split(r#"<ul class="plain links screen">"#)
        .nth(1)
        .unwrap();
    let links: Vec<(&str, &str)> = list
        .split(r#"<li><a href=""#)
        .skip(1)
        .map(|link| {
            let (address, rest) = link.split_once("\">").unwrap();
            (address, rest.split("</a>").next().unwrap())
        })
        .collect();
    assert_eq!(links.len(), 3, "{list}");
    // The page says what they are.
    assert!(list.contains("illustrations made for this page, not survey results"));
    assert!(list.contains("no matrix here is a recommendation"));
    let landed = ["43.662%", "49.418%", "50%"];
    for ((address, label), blended) in links.iter().zip(landed) {
        assert!(label.starts_with("Example: "), "{label}");
        // A preset is the page's own address with a query string.
        let address = address.replace("&#38;", "&");
        assert!(
            address.starts_with("/tools/parts-matrix?kind=markup&cost1="),
            "{address}"
        );
        let (status, html) = page(&app, &address).await;
        assert_eq!(status, StatusCode::OK, "{address}");
        let main = main_of(&html);
        assert!(
            main.contains(&format!(
                "<strong>Blended margin: {blended}.</strong> That is inside the typical range"
            )),
            "{label}"
        );
        // It is an ordinary filled-in form, not the page's example.
        assert!(
            !main.contains("The form is filled in with an example."),
            "{label}"
        );
        assert!(html.contains(r#"<meta name="robots" content="noindex">"#));
    }
    // The flat one is a single row.
    let (_, flat) = page(&app, &links[2].0.replace("&#38;", "&")).await;
    assert!(flat.contains(r#"<td>$0.00 and up</td><td>100%</td><td>50%</td>"#));
}

#[tokio::test]
async fn the_parts_target_is_shown_with_a_link_to_each_of_its_sources() {
    let app = common::app().await;
    for path in [PAGE, FILLED] {
        let (_, html) = page(&app, path).await;
        let main = main_of(&html);
        assert!(main.contains(&format!(
            "the typical range of {} to {} for parts gross profit. The usual target is {}.",
            PARTS.range.low, PARTS.range.high, PARTS.usual
        )));
        for source in PARTS.range_sources.iter().chain(PARTS.usual_sources) {
            assert!(
                main.contains(&format!(r#"<a href="{}">{}</a>"#, source.url, source.name)),
                "{path}: {}",
                source.url
            );
        }
        assert!(main.contains("These figures are for a general repair shop."));
        assert!(main.contains("Neither is a survey of what shops earn."));
        // The only addresses off the site are the sources and Wenmar Pro.
        let mut outside = 0;
        for link in main.split(r#"href="https://"#).skip(1) {
            if link.starts_with("open.example/") {
                continue;
            }
            outside += 1;
            let known = link.starts_with("wenmarpro.com/")
                || PARTS
                    .range_sources
                    .iter()
                    .chain(PARTS.usual_sources)
                    .any(|source| link.starts_with(source.url.trim_start_matches("https://")));
            assert!(known, "{path}: {:.80}", link);
        }
        assert_eq!(
            outside,
            1 + PARTS.range_sources.len() + PARTS.usual_sources.len(),
            "{path}"
        );
    }
    // Below and above are said in words, and nothing is marked as a failure.
    let (_, html) = page(&app, "/tools/parts-matrix?cost1=100&rate1=20&share1=100").await;
    let main = main_of(&html);
    assert!(
        main.contains("<strong>Blended margin: 16.667%.</strong> That is below the typical range")
    );
    assert_eq!(main.matches(r#"class="warning""#).count(), 0);
    let (_, html) = page(&app, "/tools/parts-matrix?cost1=100&rate1=300&share1=100").await;
    assert!(html.contains("<strong>Blended margin: 75%.</strong> That is above the typical range"));
}

#[tokio::test]
async fn a_calculator_that_does_not_exist_is_404() {
    let app = common::app().await;
    for path in [
        "/tools/nothing",
        "/tools/nothing.md",
        "/tools/PARTS-MATRIX",
        "/tools/parts-matrix/extra",
        "/tools/%ff",
        "/tools/..%2Fdocs",
        "/tools/",
        "/tools/nothing?cost1=5",
    ] {
        let (status, html) = page(&app, path).await;
        assert_eq!(status, StatusCode::NOT_FOUND, "{path}");
        assert!(html.contains("<h1>There is no page here</h1>"), "{path}");
    }
}

#[tokio::test]
async fn the_calculators_are_reached_from_the_home_page_the_sitemap_and_llms_txt() {
    let app = common::app().await;
    // The home page: a list under the vehicle picker, one link each.
    let (_, home) = page(&app, "/").await;
    assert_targets(&home, "/");
    let picker = home.find(r#"action="/pick""#).unwrap();
    let list = home.find("<h2>Shop calculators</h2>").unwrap();
    let guides = home.find("<h2>About VINs</h2>").unwrap();
    assert!(picker < list && list < guides);
    assert!(
        home.contains(
            r#"<li><a href="/tools/parts-matrix">Parts markup matrix calculator</a></li>"#
        )
    );
    // The site says in one line that it is both things.
    assert!(home.contains("<title>Free VIN decoder and shop calculators - Wenmar Open</title>"));
    assert!(home.contains("and use free calculators for a repair shop"));
    let (_, about) = page(&app, "/about").await;
    assert!(about.contains("with free shop calculators"));

    // The sitemap lists the index and each calculator, as it lists the guides.
    let (_, sitemap) = page(&app, "/sitemaps/pages.xml").await;
    assert!(sitemap.contains("<loc>https://open.example/tools</loc>"));
    assert!(sitemap.contains("<loc>https://open.example/tools/parts-matrix</loc>"));
    assert!(
        !sitemap.contains("parts-matrix?"),
        "no filled-in form is listed"
    );

    // llms.txt lists them as Markdown, and each of those exists.
    let (_, text) = page(&app, "/llms.txt").await;
    assert!(
        text.contains(
            "\n## Shop calculators\n\n- [Shop calculators](https://open.example/tools.md)"
        )
    );
    assert!(text.contains(
        "- [Parts markup matrix calculator](https://open.example/tools/parts-matrix.md): Works out what a part sells for"
    ));
    assert!(
        text.lines().nth(2).unwrap().contains("shop calculators"),
        "{text}"
    );
    for path in ["/tools.md", "/tools/parts-matrix.md"] {
        assert_eq!(app.get(path).await.status(), StatusCode::OK, "{path}");
    }
    // The labor rate calculator is in each of the same places.
    assert!(home.contains(
        r#"<li><a href="/tools/labor-rate">Labor rate calculator for auto repair shops</a></li>"#
    ));
    assert!(sitemap.contains("<loc>https://open.example/tools/labor-rate</loc>"));
    assert!(!sitemap.contains("labor-rate?"));
    assert!(text.contains(
        "- [Labor rate calculator for auto repair shops](https://open.example/tools/labor-rate.md): Works out the hourly labor rate"
    ));
    let (_, labor_rate) = page(&app, "/tools/labor-rate.md").await;
    let (_, all) = page(&app, "/llms-full.txt").await;
    assert!(all.contains("\n## Labor rate calculator for auto repair shops\n"));
    for line in labor_rate
        .lines()
        .filter(|line| !line.is_empty() && !line.starts_with('#'))
    {
        assert!(all.contains(line), "missing: {line}");
    }
    // llms-full.txt holds the calculator's Markdown, line for line.
    let (_, full) = page(&app, "/llms-full.txt").await;
    assert!(full.contains("\n## Parts markup matrix calculator\n"));
    let (_, own) = page(&app, "/tools/parts-matrix.md").await;
    for line in own
        .lines()
        .filter(|line| !line.is_empty() && !line.starts_with('#'))
    {
        assert!(full.contains(line), "missing: {line}");
    }
}

#[tokio::test]
async fn the_parts_matrix_guide_is_a_guide_under_tools_and_links_both_ways() {
    let app = common::app().await;
    let path = "/guides/parts-matrix";
    let (status, html) = page(&app, path).await;
    assert_eq!(status, StatusCode::OK);
    assert_basics(&html, path);
    assert_head(&html, path);
    assert_targets(&html, path);
    assert!(html.contains("<h1>How to build a parts matrix</h1>"));
    assert!(
        html.contains(r#"<link rel="canonical" href="https://open.example/guides/parts-matrix">"#)
    );
    assert!(!html.contains(r#"name="robots""#));
    // It belongs with the calculators, not with the VIN guides.
    assert!(html.contains(r#"<li><a href="/tools">Tools</a></li>"#));
    assert!(html.contains(r#"<a href="/tools" aria-current="page">Tools</a>"#));
    assert!(!html.contains("Data version"));
    for heading in [
        "Markup and margin are not the same number",
        "Why cheap parts carry a higher markup",
        "Check the price against the list price",
        "Test the matrix on last month's invoices",
        "What to aim for",
        "Check your own rules",
    ] {
        assert!(
            html.contains(&format!(">{}</h2>", heading.replace('\'', "&#39;"))),
            "{heading}"
        );
    }
    // The target is the one shop-math holds, with a link to each source.
    let main = main_of(&html);
    assert!(main.contains(&format!("{} to {}", PARTS.range.low, PARTS.range.high)));
    for source in PARTS.range_sources.iter().chain(PARTS.usual_sources) {
        assert!(
            main.contains(&format!(r#"<a href="{}">"#, source.url)),
            "{}",
            source.url
        );
    }
    assert!(main.contains("not a survey"));
    // The guide leads to the calculator, and the calculator to the guide.
    assert!(main.contains(r#"<a href="https://open.example/tools/parts-matrix">"#));
    let (_, calculator) = page(&app, PAGE).await;
    assert!(calculator.contains(
        r#"<p class="more"><a href="/guides/parts-matrix">How to build a parts matrix</a></p>"#
    ));
    let text = markdown(&app, "/guides/parts-matrix.md").await;
    assert!(text.starts_with("# How to build a parts matrix\n"));
    let calculator_text = markdown(&app, "/tools/parts-matrix.md").await;
    assert!(calculator_text.contains("](https://open.example/guides/parts-matrix)"));
    // It is listed where guides and tools are listed, and not among the
    // VIN guides of the home page.
    let (_, guides) = page(&app, "/guides").await;
    assert!(guides.contains("https://open.example/guides/parts-matrix\""));
    let (_, tools) = page(&app, "/tools").await;
    assert!(tools.contains("https://open.example/guides/parts-matrix\""));
    let (_, sitemap) = page(&app, "/sitemaps/pages.xml").await;
    assert!(sitemap.contains("<loc>https://open.example/guides/parts-matrix</loc>"));
    let (_, llms) = page(&app, "/llms.txt").await;
    assert!(llms.contains("](https://open.example/guides/parts-matrix.md)"));
    let (_, home) = page(&app, "/").await;
    assert!(!home.contains("/guides/parts-matrix"));
    assert!(home.contains(r#"<a href="/guides/check-digit">"#));
}
