//! The Canadian invoice tax and tire fee calculator,
//! `/tools/canada-invoice-tax`, through HTTP.
//!
//! The rates are read from the server's own list, so no test here writes
//! one. What a test does write is an expected amount, worked by hand.

use axum::http::StatusCode;
use open_server::site::tools::canada_invoice_tax::rates::{PROVINCES, Province};

use crate::common::{
    self, assert_basics, assert_head, assert_no_injection, assert_targets, body_text, header,
    json_ld, markdown, page,
};

const PAGE: &str = "/tools/canada-invoice-tax";

/// The invoice every worked case uses: 834.37 before fees and taxes.
const INVOICE: &str = "labour=180.00&parts=640.00&supplies=14.37";

/// The part of a page between `<main>` and `</main>`.
fn main_of(html: &str) -> &str {
    let main = html.split(r#"<main id="main">"#).nth(1).unwrap();
    main.split("</main>").next().unwrap()
}

/// A row of the result table, as the page writes it.
fn row(cells: [&str; 2]) -> String {
    format!(
        r#"<tr><th scope="row">{}</th><td>{}</td></tr>"#,
        cells[0], cells[1]
    )
}

/// A box of the form with what it holds, as the page writes it.
fn input(name: &str, value: &str) -> String {
    format!(
        r#"<input id="{name}" name="{name}" type="text" inputmode="decimal" autocomplete="off" value="{value}""#
    )
}

/// The entry of the list of provinces that is chosen.
fn chosen(main: &str) -> &str {
    let option = main.split(" selected>").next().unwrap();
    option.rsplit("<option value=\"").next().unwrap()
}

fn province(code: &str) -> &'static Province {
    PROVINCES
        .iter()
        .find(|province| province.code == code)
        .unwrap()
}

/// The address of the worked invoice in a province: four tires of its
/// first class and one of its second.
fn worked_address(province: &Province) -> String {
    let mut address = format!("{PAGE}?province={}&{INVOICE}", province.code);
    for (class, count) in province.tire_classes.iter().zip(["4", "1"]) {
        address.push_str(&format!("&{}={count}", class.field()));
    }
    address
}

#[tokio::test]
async fn the_canadian_page_opens_with_a_labelled_example_and_its_result() {
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
        html.contains("<title>Canadian invoice tax and tire fee calculator - Wenmar Open</title>")
    );
    assert!(html.contains(
        r#"<link rel="canonical" href="https://open.example/tools/canada-invoice-tax">"#
    ));
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

    // The parts of a calculator page, in order.
    let main = main_of(&html);
    let parts = [
        "<h1>Canadian invoice tax and tire fee calculator</h1>",
        "<p>Works out the GST, HST, PST or QST and the new-tire fees on a repair invoice",
        r#"<p class="note">Amounts are in Canadian dollars (CAD).</p>"#,
        r##"<form class="calc" id="calc" action="/tools/canada-invoice-tax#result" method="get">"##,
        r#"<h2 id="result">Result</h2>"#,
        "<caption>Invoice in British Columbia, in Canadian dollars (CAD)</caption>",
        r#"<h2 id="sources">Rates and sources for British Columbia</h2>"#,
        r#"<h2 id="how">How this is worked out</h2>"#,
        r#"<h2 id="covers">What this covers</h2>"#,
        r#"<p class="note">This is arithmetic, not accounting or tax advice.</p>"#,
        r#"<p class="pro"><a href="https://wenmarpro.com/?utm_source=wenmar-open&#38;utm_medium=referral&#38;utm_campaign=tool-canada-invoice-tax">Wenmar Pro</a> applies GST, HST, PST and QST by province automatically.</p>"#,
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
    assert_eq!(chosen(main), "BC\"");
    for (name, value) in [
        ("labour", "180.00"),
        ("parts", "640.00"),
        ("supplies", "14.37"),
        ("tires_passenger_light_truck", "4"),
        ("tires_medium_truck", "0"),
    ] {
        assert!(main.contains(&format!("{}>", input(name, value))), "{name}");
    }
    assert_eq!(main.matches("<input ").count(), 5);
    assert_eq!(main.matches("<select ").count(), 1);
    assert!(main.contains(
        r#"<label for="tires_passenger_light_truck">New tires: passenger and light truck</label>"#
    ));
    assert_eq!(main.matches(r#"class="primary""#).count(), 1);
    assert!(main.contains(r#"<button class="primary" type="submit">Work it out</button>"#));
    // No account, no email, no gate.
    assert!(!main.contains("email") && !main.contains("type=\"password\""));

    // The result is the one the Markdown version states.
    let text = markdown(&app, "/tools/canada-invoice-tax.md").await;
    assert!(
        text.starts_with("# Canadian invoice tax and tire fee calculator\n"),
        "{text}"
    );
    let mut rows = 0;
    let example = text.split("\n## A worked example\n").nth(1).unwrap();
    let example = example.split("\n## ").next().unwrap();
    for line in example
        .lines()
        .filter(|line| line.starts_with("| ") && line.contains('$'))
    {
        let cells: Vec<&str> = line
            .trim_start_matches("| ")
            .trim_end_matches(" |")
            .split(" | ")
            .map(str::trim)
            .collect();
        assert!(main.contains(&row([cells[0], cells[1]])), "{line}");
        rows += 1;
    }
    assert_eq!(rows, 8, "the eight lines of the example");
    assert_eq!(main.matches(r#"<th scope="row">"#).count(), 8);
    // 834.37 and 26.00 of tire fees: 43.02 and 60.23 of tax.
    assert!(main.contains(&row(["Total (CAD)", "$963.62"])));
    for stated in [
        "Subtotal = labour + parts + shop supplies.",
        "No tax is calculated on another tax.",
        "a retail repair invoice to a consumer, in Canadian dollars (CAD)",
    ] {
        assert!(main.contains(stated), "the page: {stated}");
        assert!(text.contains(stated), "the Markdown: {stated}");
    }
    assert!(text.contains("This page describes the calculator and does not compute."));

    // The footer says whose rates these are and when they were checked,
    // in place of the data version, and links to the sources on the page.
    let footer = html.split("<footer>").nth(1).unwrap();
    assert!(
        footer.contains(&format!(
            "<p>Rates for British Columbia as of {}. <a href=\"#sources\">Sources for these rates</a>.</p>",
            province("BC").as_of
        )),
        "{footer}"
    );
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

/// A province's code, its tire fee lines, each tax's line and the total.
type Worked = (
    &'static str,
    &'static [&'static str],
    &'static [[&'static str; 2]],
    &'static str,
);

/// Labour 180.00, parts 640.00, shop supplies 14.37, four new tires of the
/// province's first class and one of its second. Each line was worked with
/// a calculator from the rates the sources give:
///
/// - BC: fees 26.00 + 14.00; 5% and 7% of 874.37 are 43.7185 and 61.2059.
/// - AB: fees 20.00 + 14.00; 5% of 868.37 is 43.4185.
/// - SK: fees 26.00 + 14.00; 5% of 874.37; 6% of 834.37 is 50.0622.
/// - MB: fees 20.00 + 14.00; 5% and 7% of 868.37 are 43.4185 and 60.7859.
/// - ON: no fee; 13% of 834.37 is 108.4681.
/// - QC: fees 18.00 + 6.00; 5% and 9.975% of 858.37 are 42.9185 and 85.6224.
/// - NB: fees 18.00 + 13.50; 15% of 865.87 is 129.8805.
/// - NS: fees 18.00 + 13.50; 14% of 865.87 is 121.2218.
/// - PE: fees 16.00 + 11.25; 15% of 861.62 is 129.243.
/// - NL: fees 12.00 + 9.00; 15% of 855.37 is 128.3055.
/// - YT: fees 28.00 + 9.00; 5% of 871.37 is 43.5685.
const WORKED: [Worked; 11] = [
    (
        "BC",
        &["$26.00", "$14.00"],
        &[
            ["GST 5% on $874.37", "$43.72"],
            ["PST 7% on $874.37", "$61.21"],
        ],
        "$979.30",
    ),
    (
        "AB",
        &["$20.00", "$14.00"],
        &[["GST 5% on $868.37", "$43.42"]],
        "$911.79",
    ),
    (
        "SK",
        &["$26.00", "$14.00"],
        &[
            ["GST 5% on $874.37", "$43.72"],
            ["PST 6% on $834.37", "$50.06"],
        ],
        "$968.15",
    ),
    (
        "MB",
        &["$20.00", "$14.00"],
        &[
            ["GST 5% on $868.37", "$43.42"],
            ["RST 7% on $868.37", "$60.79"],
        ],
        "$972.58",
    ),
    ("ON", &[], &[["HST 13% on $834.37", "$108.47"]], "$942.84"),
    (
        "QC",
        &["$18.00", "$6.00"],
        &[
            ["GST 5% on $858.37", "$42.92"],
            ["QST 9.975% on $858.37", "$85.62"],
        ],
        "$986.91",
    ),
    (
        "NB",
        &["$18.00", "$13.50"],
        &[["HST 15% on $865.87", "$129.88"]],
        "$995.75",
    ),
    (
        "NS",
        &["$18.00", "$13.50"],
        &[["HST 14% on $865.87", "$121.22"]],
        "$987.09",
    ),
    (
        "PE",
        &["$16.00", "$11.25"],
        &[["HST 15% on $861.62", "$129.24"]],
        "$990.86",
    ),
    (
        "NL",
        &["$12.00", "$9.00"],
        &[["HST 15% on $855.37", "$128.31"]],
        "$983.68",
    ),
    (
        "YT",
        &["$28.00", "$9.00"],
        &[["GST 5% on $871.37", "$43.57"]],
        "$914.94",
    ),
];

#[tokio::test]
async fn a_worked_invoice_for_each_confirmed_province_is_shown_line_by_line() {
    let app = common::app().await;
    for (code, fees, taxes, total) in WORKED {
        let province = province(code);
        let address = worked_address(province);
        let (status, html) = page(&app, &address).await;
        assert_eq!(status, StatusCode::OK, "{address}");
        assert_basics(&html, &address);
        assert_targets(&html, &address);
        let main = main_of(&html);
        assert_eq!(chosen(main), format!("{code}\""));
        assert!(
            main.contains(&format!(
                "<caption>Invoice in {}, in Canadian dollars (CAD)</caption>",
                province.name
            )),
            "{code}"
        );
        assert!(
            main.contains(&row(["Subtotal", "$834.37"])),
            "{code}\n{main}"
        );
        // One line for each tire class, and none where there is no fee.
        assert_eq!(main.matches("Tire fee, ").count() / 2, fees.len(), "{code}");
        for ((class, count), amount) in province.tire_classes.iter().zip([4, 1]).zip(fees) {
            let name = format!("Tire fee, {}: {count} at {}", class.label, class.fee);
            assert!(main.contains(&row([&name, amount])), "{code}: {name}");
        }
        // Each tax with its rate, what it was calculated on, and nothing else.
        for tax in taxes {
            assert!(main.contains(&row(*tax)), "{code}: {tax:?}\n{main}");
        }
        assert_eq!(
            main.matches("<td>$").count(),
            // Labour, parts, shop supplies, the subtotal, each fee, each
            // tax and the total.
            4 + fees.len() + taxes.len() + 1,
            "{code}"
        );
        assert!(
            main.contains(&row(["Total (CAD)", total])),
            "{code}: {total}\n{main}"
        );
        // The notes the rates file has for the province are with the result.
        for note in province.notes {
            let note = note.replace('\'', "&#39;");
            assert!(
                main.contains(&format!("<p class=\"note\">{note}</p>")),
                "{code}: {note}"
            );
        }
        assert_eq!(main.matches(r#"class="warning""#).count(), 0, "{code}");
    }
    // Every confirmed province has its worked invoice above.
    for province in PROVINCES.iter().filter(|province| province.confirmed) {
        assert!(
            WORKED.iter().any(|(code, ..)| *code == province.code),
            "{} is confirmed and has no worked invoice",
            province.code
        );
    }
    // Saskatchewan's two taxes are calculated on different amounts, and
    // the page says why.
    let (_, html) = page(&app, &worked_address(province("SK"))).await;
    assert!(
        html.contains("<p>PST 6%: on labour, parts and shop supplies. Not on the tire fee.</p>")
    );
    assert!(html.contains("when the fee is a separate line on the invoice"));
    // Ontario has no tire field, and says what a shop does with a fee of
    // its own.
    let (_, html) = page(&app, &worked_address(province("ON"))).await;
    let main = main_of(&html);
    assert_eq!(main.matches("<input ").count(), 3);
    let statement = province("ON").no_tire_fee.unwrap().statement;
    assert_eq!(
        main.matches(&format!("<p class=\"note\">{statement}</p>"))
            .count(),
        1
    );
}

#[tokio::test]
async fn a_shared_address_reproduces_the_result_exactly() {
    let app = common::app().await;
    let address = worked_address(province("QC"));
    let (_, first) = page(&app, &address).await;
    let (_, second) = page(&app, &address).await;
    assert_eq!(main_of(&first), main_of(&second));
    // What was sent is what the form holds, field for field.
    let main = main_of(&first);
    for (name, value) in [
        ("labour", "180.00"),
        ("parts", "640.00"),
        ("supplies", "14.37"),
        ("tires_diameter_83_82_cm_or_less", "4"),
        ("tires_diameter_over_83_82_cm_to_123_19_cm", "1"),
    ] {
        assert!(main.contains(&format!("{}>", input(name, value))), "{name}");
    }
    assert!(!main.contains("filled in with an example"));
    // The province in small letters, and the fields in another order.
    let turned = format!(
        "{PAGE}?tires_diameter_over_83_82_cm_to_123_19_cm=1&supplies=14.37&parts=640.00&tires_diameter_83_82_cm_or_less=4&labour=180.00&province=qc"
    );
    let (_, html) = page(&app, &turned).await;
    assert_eq!(main_of(&html), main);
}

#[tokio::test]
async fn changing_the_province_shows_its_own_tire_classes_and_says_what_was_left_out() {
    let app = common::app().await;
    // The example as the form sends it, with Quebec chosen in the list.
    let address =
        format!("{PAGE}?province=QC&{INVOICE}&tires_passenger_light_truck=4&tires_medium_truck=0");
    let (status, html) = page(&app, &address).await;
    assert_eq!(status, StatusCode::OK);
    assert_basics(&html, &address);
    let main = main_of(&html);
    assert_eq!(chosen(main), "QC\"");
    // Quebec's fields, and not British Columbia's.
    assert!(main.contains(&format!(
        "{}>",
        input("tires_diameter_83_82_cm_or_less", "0")
    )));
    assert!(main.contains(&format!(
        "{}>",
        input("tires_diameter_over_83_82_cm_to_123_19_cm", "0")
    )));
    assert!(!main.contains("tires_passenger_light_truck"));
    // The count was not moved into another class: there is no fee line,
    // and the page says so above a result that is Quebec's.
    assert!(main.contains(
        r#"<p class="warning" id="tires-left-out">A number of tires was sent for a class Quebec does not have. It was not used: enter the tires in Quebec&#39;s classes above.</p>"#
    ), "{main}");
    assert!(!main.contains("<th scope=\"row\">Tire fee, "));
    assert!(main.contains(&row(["GST 5% on $834.37", "$41.72"])));
    assert!(main.contains("<caption>Invoice in Quebec, in Canadian dollars (CAD)</caption>"));
    // A class two provinces share carries over and nothing is said.
    let address = format!("{PAGE}?province=AB&{INVOICE}&tires_passenger_light_truck=4");
    let (_, html) = page(&app, &address).await;
    let main = main_of(&html);
    assert!(!main.contains("tires-left-out"));
    assert!(main.contains(&format!("{}>", input("tires_passenger_light_truck", "4"))));
}

#[tokio::test]
async fn a_province_that_is_unknown_or_not_confirmed_computes_nothing() {
    let app = common::app().await;
    let unknown = "This is not a province or territory this page knows. Choose one from the list.";
    let mut cases = vec![
        ("XX".to_owned(), unknown.to_owned()),
        ("Ontario".to_owned(), unknown.to_owned()),
        (String::new(), "Choose a province or territory.".to_owned()),
    ];
    for province in PROVINCES.iter().filter(|province| !province.confirmed) {
        cases.push((
            province.code.to_owned(),
            format!(
                "{} is not covered yet. {}",
                province.name,
                province.missing.join(" ")
            ),
        ));
    }
    for (code, message) in cases {
        let address = format!("{PAGE}?province={code}&{INVOICE}&tires_passenger_light_truck=4");
        let (status, html) = page(&app, &address).await;
        assert_eq!(status, StatusCode::OK, "{address}");
        assert_basics(&html, &address);
        assert_targets(&html, &address);
        let main = main_of(&html);
        // No province is chosen, and the message is beside the list.
        assert_eq!(chosen(main), "\"", "{code}");
        assert!(
            main.contains(&format!(
                r#"<p class="warning" id="province-error">{message}</p>"#
            )),
            "{code}\n{main}"
        );
        assert!(main.contains(
            r#"<select id="province" name="province" aria-invalid="true" aria-describedby="province-error">"#
        ));
        // Nothing is worked out: no table, no amount, no rates, no date.
        assert!(
            !main.contains("<table") && !main.contains("<td>$"),
            "{code}"
        );
        assert!(!main.contains("Rates and sources for"), "{code}");
        assert!(!html.contains("Rates for "), "{code}");
        assert!(main.contains("<p>There is no result yet. Correct what is marked above:</p>"));
        assert!(main.contains(&format!(
            r##"<li><a href="#province">Province or territory: {message}</a></li>"##
        )));
        // The other fields keep what was typed, and there are no tire fields.
        assert!(main.contains(&format!("{}>", input("labour", "180.00"))));
        assert_eq!(main.matches("<input ").count(), 3, "{code}");
        // The rest of the page is still there.
        assert!(main.contains(r#"<h2 id="how">How this is worked out</h2>"#));
    }
    // A province that is not covered is in the list, and cannot be chosen.
    let (_, html) = page(&app, PAGE).await;
    let main = main_of(&html);
    for province in &PROVINCES {
        let option = if province.confirmed {
            format!(r#"<option value="{}""#, province.code)
        } else {
            format!(
                r#"<option value="{}" disabled>{}</option>"#,
                province.code, province.name
            )
        };
        assert_eq!(main.matches(&option).count(), 1, "{}", province.code);
    }
}

#[tokio::test]
async fn the_page_states_its_scope_and_lists_what_is_not_covered() {
    let app = common::app().await;
    let (_, html) = page(&app, PAGE).await;
    let main = main_of(&html);
    // Spec section 8.3: what it covers, and each thing it does not.
    for limit in [
        "This covers a retail repair invoice to a consumer, in Canadian dollars (CAD)",
        "It does not cover a customer who is exempt from a tax, or a vehicle that is being repaired for resale.",
        "It does not cover fees other than the fee on a new tire, such as those on oil, oil filters, antifreeze and batteries.",
        "It does not cover tire classes other than the ones listed for the province",
    ] {
        assert!(main.contains(limit), "{limit}");
    }
    assert!(
        main.contains(r#"<p class="note">This is arithmetic, not accounting or tax advice.</p>"#)
    );
    // Each province that is not confirmed is named with what is missing.
    let waiting: Vec<&Province> = PROVINCES
        .iter()
        .filter(|province| !province.confirmed)
        .collect();
    assert_eq!(
        main.contains(r#"<h2 id="not-covered">Not yet covered</h2>"#),
        !waiting.is_empty()
    );
    for province in waiting {
        assert!(
            main.contains(&format!(
                "<p>{}: {}</p>",
                province.name,
                province.missing.join(" ")
            )),
            "{}",
            province.code
        );
        assert!(main.contains(r#"<optgroup label="Not yet covered">"#));
    }
}

#[tokio::test]
async fn the_date_of_the_rates_and_every_source_are_on_the_page() {
    let app = common::app().await;
    for province in PROVINCES.iter().filter(|province| province.confirmed) {
        let address = worked_address(province);
        let (_, html) = page(&app, &address).await;
        let main = main_of(&html);
        let checked = format!("Rates for {} as of {}.", province.name, province.as_of);
        assert!(
            main.contains(&format!(
                "<p>{checked} These are the rates the result above uses:</p>"
            )),
            "{}",
            province.code
        );
        let footer = html.split("<footer>").nth(1).unwrap();
        assert!(
            footer.contains(&format!("<p>{checked} <a href=\"#sources\">")),
            "{}",
            province.code
        );
        assert_eq!(main.matches(r#"id="sources""#).count(), 1);
        // Every tax and every fee the result used has its source linked.
        let mut links = 0;
        for source in province.sources() {
            assert_eq!(
                main.matches(&format!(r#"<li><a href="{}">"#, source.url))
                    .count(),
                1,
                "{}: {}",
                province.code,
                source.url
            );
            if let Some(date) = source.archived {
                assert!(main.contains(&format!(
                    "{} (read from an archived copy of {date})</a></li>",
                    source.title
                )));
            }
            links += 1;
        }
        let used = province
            .taxes
            .iter()
            .flat_map(|tax| tax.sources)
            .chain(province.tire_classes.iter().flat_map(|class| class.sources));
        for source in used {
            assert!(main.contains(&format!(r#"<li><a href="{}">"#, source.url)));
        }
        // The only addresses off the site are the sources and Wenmar Pro.
        let outside = main
            .split(r#"href="https://"#)
            .skip(1)
            .filter(|link| !link.starts_with("open.example/"))
            .count();
        assert_eq!(outside, 1 + links, "{}", province.code);
        // A fee with a date says since when.
        for class in province.tire_classes {
            let since = class
                .effective
                .map_or_else(String::new, |day| format!(", since {day}"));
            assert!(
                main.contains(&format!(
                    "<p>Tire fee, {}: {} a tire{since}.</p>",
                    class.label, class.fee
                )),
                "{}: {}",
                province.code,
                class.key
            );
        }
    }
}

#[tokio::test]
async fn a_field_that_cannot_be_read_gets_a_message_and_the_rest_keep_what_was_typed() {
    let app = common::app().await;
    let path = "/tools/canada-invoice-tax?province=NS&labour=180%2C5&parts=640&supplies=&tires_passenger_light_truck=2.5&tires_medium_truck=";
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
            r#"{} aria-invalid="true" aria-describedby="labour-error">"#,
            input("labour", "180,5")
        )),
        "{main}"
    );
    assert!(main.contains(
        r#"<p class="warning" id="labour-error">This is not a number. Write it like 1,234.50.</p>"#
    ));
    assert!(main.contains(
        r#"<p class="warning" id="tires_passenger_light_truck-error">Use a whole number.</p>"#
    ));
    assert_eq!(main.matches(r#"class="warning""#).count(), 2);
    // Every other field keeps what was typed, or shows its default, and
    // the province stays chosen with its own tire fields.
    assert_eq!(chosen(main), "NS\"");
    assert!(main.contains(&format!("{}>", input("parts", "640"))));
    assert!(main.contains(&format!("{}>", input("supplies", "0.00"))));
    assert!(main.contains(&format!("{}>", input("tires_medium_truck", "0"))));
    // No result is shown, and the reader is told where to look.
    assert!(!main.contains("<table") && !main.contains("<td>$"));
    assert!(main.contains("<p>There is no result yet. Correct what is marked above:</p>"));
    assert!(main.contains(
        r##"<li><a href="#labour">Labour: This is not a number. Write it like 1,234.50.</a></li>"##
    ));
    assert!(main.contains(
        r##"<li><a href="#tires_passenger_light_truck">New tires: passenger and light truck: Use a whole number.</a></li>"##
    ));
    // The province's rates and sources are still shown.
    assert!(main.contains(r#"<h2 id="sources">Rates and sources for Nova Scotia</h2>"#));
    assert!(main.contains(r#"<h2 id="how">How this is worked out</h2>"#));

    // Nothing on the invoice at all is said at the form.
    let path = "/tools/canada-invoice-tax?province=NS&labour=0&parts=0";
    let (status, html) = page(&app, path).await;
    assert_eq!(status, StatusCode::OK);
    let main = main_of(&html);
    assert!(main.contains(
        r#"<p class="warning" id="calc-error">Enter labour, parts, shop supplies or a number of new tires.</p>"#
    ));
    assert!(main.contains(
        r##"<li><a href="#calc">Enter labour, parts, shop supplies or a number of new tires.</a></li>"##
    ));
    assert!(!main.contains("<table"));
}

#[tokio::test]
async fn an_address_with_a_query_string_is_not_indexed_and_names_the_bare_address() {
    let app = common::app().await;
    let canonical =
        r#"<link rel="canonical" href="https://open.example/tools/canada-invoice-tax">"#;
    let filled = worked_address(province("MB"));
    for path in [
        filled.as_str(),
        "/tools/canada-invoice-tax?labour=abc",
        "/tools/canada-invoice-tax?utm_source=newsletter",
        "/tools/canada-invoice-tax?",
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
        "/tools/canada-invoice-tax?utm_source=newsletter",
        "/tools/canada-invoice-tax?",
        "/tools/canada-invoice-tax?cost1=5&kind=margin&labor_sales=1",
    ] {
        let (_, html) = page(&app, path).await;
        assert!(
            html.contains("The form is filled in with an example."),
            "{path}"
        );
        assert!(html.contains(&row(["Total (CAD)", "$963.62"])), "{path}");
        assert!(
            !html.contains("Choose a province or territory.</p>"),
            "{path}"
        );
    }
}

#[tokio::test]
async fn nothing_typed_into_the_form_arrives_as_markup() {
    let app = common::app().await;
    let hostile =
        "%22%3E%3Cscript%3Ealert(1)%3C%2Fscript%3E%3Cimg%20src%3Dx%20onerror%3Dalert(1)%3E";
    for code in ["BC", hostile] {
        let path = format!(
            "/tools/canada-invoice-tax?province={code}&labour={hostile}&parts={hostile}&supplies={hostile}&tires_passenger_light_truck={hostile}&tires_medium_truck={hostile}&{hostile}={hostile}"
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
    }
    // Figures no field accepts, text that is not UTF-8, a very long field
    // and the largest figures: each is a page, never an error.
    let long = "9".repeat(6_000);
    for query in [
        "province=BC&labour=99999999999999999999&parts=99999999999999999999".to_owned(),
        "province=%ff%fe&labour=%00".to_owned(),
        "province=QC&labour=1e308&parts=NaN&supplies=-0".to_owned(),
        format!("province=SK&labour={long}&parts=5&tires_medium_truck=3"),
        format!("province=SK&labour=5&parts=5&tires_medium_truck={long}"),
        format!("province={long}&labour=5&parts=5"),
        "province=QC&labour=99,999,999.99&parts=99,999,999.99&supplies=99,999,999.99&tires_diameter_83_82_cm_or_less=1,000&tires_diameter_over_83_82_cm_to_123_19_cm=1000".to_owned(),
        "&&&===&province".to_owned(),
    ] {
        let path = format!("/tools/canada-invoice-tax?{query}");
        let (status, html) = page(&app, &path).await;
        assert_eq!(status, StatusCode::OK, "{:.80}", path);
        assert!(
            html.contains(r#"<h2 id="result">Result</h2>"#),
            "{:.80}",
            path
        );
    }
    // The largest invoice a form can send is worked out and shown whole:
    // 299,999,999.97 and 10,500.00 of tire fees.
    let path = "/tools/canada-invoice-tax?province=QC&labour=99,999,999.99&parts=99,999,999.99&supplies=99,999,999.99&tires_diameter_83_82_cm_or_less=1,000&tires_diameter_over_83_82_cm_to_123_19_cm=1000";
    let (_, html) = page(&app, path).await;
    assert!(
        html.contains(&row(["GST 5% on $300,010,499.97", "$15,000,525.00"])),
        "{html}"
    );
}

#[tokio::test]
async fn the_canadian_page_is_on_the_tools_index_with_its_sentence() {
    let app = common::app().await;
    let (_, html) = page(&app, "/tools").await;
    assert!(html.contains(
        r#"<h2 id="canadian-invoice-tax-and-tire-fee-calculator">Canadian invoice tax and tire fee calculator</h2>"#
    ));
    assert!(html.contains("<p>Works out the GST, HST, PST or QST and the new-tire fees"));
    assert!(html.contains(
        r#"<li><a href="https://open.example/tools/canada-invoice-tax">Canadian invoice tax and tire fee calculator</a></li>"#
    ));
    // It comes last, in the order the calculators were made.
    let parts = html
        .find(r#"<h2 id="parts-markup-matrix-calculator">"#)
        .unwrap();
    let canada = html
        .find(r#"<h2 id="canadian-invoice-tax-and-tire-fee-calculator">"#)
        .unwrap();
    assert!(parts < canada);
    let text = markdown(&app, "/tools.md").await;
    assert!(text.contains(
        "- [Canadian invoice tax and tire fee calculator](https://open.example/tools/canada-invoice-tax)\n"
    ));
    // The Markdown version is served, and an address under the page is not.
    assert_eq!(
        app.get("/tools/canada-invoice-tax.md").await.status(),
        StatusCode::OK
    );
    for path in [
        "/tools/canada-invoice-tax/extra",
        "/tools/CANADA-INVOICE-TAX",
        "/tools/canada_invoice_tax",
        "/tools/canada-invoice-tax/BC",
    ] {
        assert_eq!(
            app.get(path).await.status(),
            StatusCode::NOT_FOUND,
            "{path}"
        );
    }
}
