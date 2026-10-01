//! The result page, `/vin/{vin}`.

use axum::body::Body;
use axum::http::{Request, StatusCode};

use crate::common::{
    self, assert_basics, assert_head, assert_no_injection, assert_targets, body_text, header, page,
    redirect,
};

#[tokio::test]
async fn a_result_page_shows_the_decode_and_is_not_indexed() {
    let app = common::app().await;
    let response = app.get("/vin/KM8K2CAB4PU001140").await;
    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(header(&response, "x-robots-tag"), "noindex");
    assert_eq!(header(&response, "cache-control"), "private, max-age=3600");
    let html = body_text(response).await;
    assert_basics(&html, "/vin/KM8K2CAB4PU001140");
    assert!(html.contains(r#"<meta name="robots" content="noindex">"#));
    assert!(!html.contains(r#"rel="canonical""#));
    assert!(
        html.contains(r#"<h1 id="summary">2023 Hyundai Kona SE</h1>"#),
        "{html}"
    );
    assert!(html.contains("<title>2023 Hyundai Kona SE - Wenmar Open</title>"));
    assert!(html.contains(r#"<tr><th scope="row">Engine</th><td>2.0L</td></tr>"#));
    assert!(html.contains(r#"<tr><th scope="row">ABS</th><td>Standard</td></tr>"#));
    assert!(
        html.contains(r#"<tr><th scope="row">Tire pressure monitoring</th><td>Direct</td></tr>"#)
    );
    assert!(html.contains(r#"<tr><th scope="row">Plant city</th><td>Ulsan</td></tr>"#));
    // The vehicle first, then the engine, then safety equipment.
    let vehicle = html.find("<caption>Vehicle</caption>").unwrap();
    let engine = html.find("<caption>Engine</caption>").unwrap();
    let safety = html.find("<caption>Safety equipment</caption>").unwrap();
    assert!(vehicle < engine && engine < safety);
    // Unknown fields have no row.
    assert!(!html.contains("Doors"));
    assert!(html.contains(
        r#"<a href="/makes/hyundai/kona/2023">All trims and engines for the 2023 Hyundai Kona</a>"#
    ));
    // Copy and print are conveniences that need the script; they start
    // hidden. Copy carries the sheet as text.
    assert!(
        html.contains(
            "<button type=\"button\" data-copy-text=\"2023 Hyundai Kona SE\nVIN KM8K2CAB4PU001140\n\nVehicle\nYear: 2023\n"
        ),
        "{html}"
    );
    assert!(html.contains(r#"<button type="button" data-print hidden>Print</button>"#));
    assert!(html.contains(r#"<span class="status" role="status"></span>"#));
    // One line about Wenmar Pro, and its link says nothing about the vehicle.
    assert_eq!(html.matches("utm_campaign=vin-result").count(), 1);
    for link in html.split("https://wenmarpro.com").skip(1) {
        let address = link.split('"').next().unwrap();
        assert!(!address.contains("KM8"), "{address}");
    }
}

#[tokio::test]
async fn a_vin_typed_untidily_is_sent_to_its_one_address() {
    let app = common::app().await;
    let (status, to) = redirect(&app, "/vin/km8k2cab4pu001140").await;
    assert_eq!(status, StatusCode::PERMANENT_REDIRECT);
    assert_eq!(to, "/vin/KM8K2CAB4PU001140");
    let (_, to) = redirect(&app, "/vin/KM8-K2CAB4-PU001140").await;
    assert_eq!(to, "/vin/KM8K2CAB4PU001140");
}

#[tokio::test]
async fn a_wrong_check_digit_shows_the_decode_with_a_warning() {
    let app = common::app().await;
    let (status, html) = page(&app, "/vin/KM8K2CAB0PU001140").await;
    assert_eq!(status, StatusCode::OK);
    assert!(
        html.contains(r#"<p class="warning">Position 9 should be 4 for this VIN, but it is 0."#),
        "{html}"
    );
    assert!(html.contains("2023 Hyundai Kona SE"));
}

#[tokio::test]
async fn html_in_the_data_is_shown_as_text() {
    let app = common::app().await;
    let (status, html) = page(&app, &format!("/vin/{}", common::HOSTILE)).await;
    assert_eq!(status, StatusCode::OK);
    assert_no_injection(&html);
    // It is there, as text.
    assert!(
        html.contains("&#60;script&#62;alert(1)&#60;/script&#62;"),
        "{html}"
    );
    assert!(
        html.contains("&#34;&#62;&#60;img src=x onerror=alert(1)&#62;"),
        "{html}"
    );
}

#[tokio::test]
async fn what_is_typed_in_the_address_is_never_shown_back_as_markup() {
    let app = common::app().await;
    for path in [
        "/vin/%3Cscript%3Ealert(1)%3C%2Fscript%3E",
        "/vin/%22%3E%3Cimg%20src=x%20onerror=alert(1)%3E",
        "/vin/KM8K2CAB4PU00114%3Cb%3EBold",
        "/vin/%FF%FE",
    ] {
        let (status, html) = page(&app, path).await;
        assert_eq!(status, StatusCode::BAD_REQUEST, "{path}");
        assert_no_injection(&html);
        assert_basics(&html, path);
    }
    let long = format!("/vin/{}", "A".repeat(60_000));
    // The server refuses an address over 8 KB before routing, so this never
    // reaches the page; what matters is that nothing large comes back.
    let (status, html) = page(&app, &long).await;
    assert_eq!(status, StatusCode::URI_TOO_LONG);
    assert!(html.len() < 10_000, "{} bytes", html.len());
}

#[tokio::test]
async fn a_malformed_vin_gets_a_page_that_helps() {
    let app = common::app().await;
    let response = app.get("/vin/KM8K2CAB4PUO01140").await;
    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    assert_eq!(header(&response, "x-robots-tag"), "noindex");
    let html = body_text(response).await;
    assert!(html.contains("<h1>That is not a VIN</h1>"));
    assert!(
        html.contains("A VIN uses only digits and letters other than I, O and Q."),
        "{html}"
    );
    assert!(
        html.contains(r#"<a class="mono" href="/vin/KM8K2CAB4PU001140">KM8K2CAB4PU001140</a>"#)
    );
    // The box is offered again.
    assert!(html.contains(r#"<form class="vin" action="/vin" method="get">"#));

    let (status, html) = page(&app, "/vin/KM8K2").await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert!(
        html.contains("A VIN has 17 characters, this has 5."),
        "{html}"
    );
}

#[tokio::test]
async fn an_unknown_manufacturer_gets_a_page_that_says_so() {
    let app = common::app().await;
    let (status, html) = page(&app, "/vin/ZZZK2CAB4PU001140").await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    assert!(html.contains("<h1>We do not know this manufacturer</h1>"));
    assert!(html.contains("No manufacturer is registered for ZZZ."));
    assert_basics(&html, "/vin/ZZZK2CAB4PU001140");
}

#[tokio::test]
async fn a_result_page_stays_private_when_revalidated() {
    let app = common::app().await;
    let etag = header(&app.get("/").await, "etag").to_owned();
    // A result page has no tag of its own to send back.
    let first = app.get("/vin/KM8K2CAB4PU001140").await;
    assert!(first.headers().get("etag").is_none());
    // The tag of a public page, or `*`, must not turn it into a public 304.
    for tag in [etag.as_str(), "*"] {
        let response = app
            .send(
                Request::get("/vin/KM8K2CAB4PU001140")
                    .header("if-none-match", tag)
                    .body(Body::empty())
                    .unwrap(),
            )
            .await;
        assert_eq!(response.status(), StatusCode::OK, "{tag}");
        assert_eq!(header(&response, "cache-control"), "private, max-age=3600");
        assert_eq!(header(&response, "x-robots-tag"), "noindex");
        assert!(response.headers().get("etag").is_none());
    }
}

#[tokio::test]
async fn a_failure_on_a_result_page_is_not_kept() {
    let app = common::app().await;
    for (path, status) in [
        ("/vin/KM8K2", StatusCode::BAD_REQUEST),
        ("/vin/ZZZK2CAB4PU001140", StatusCode::NOT_FOUND),
        ("/vin/%FF%FE", StatusCode::BAD_REQUEST),
    ] {
        let response = app.get(path).await;
        assert_eq!(response.status(), status, "{path}");
        assert_eq!(header(&response, "cache-control"), "no-store", "{path}");
        assert_eq!(header(&response, "x-robots-tag"), "noindex", "{path}");
    }
}

#[tokio::test]
async fn the_result_page_is_a_spec_sheet_in_short_groups() {
    let app = common::app().await;
    let (_, html) = page(&app, "/vin/KM8K2CAB4PU001140").await;
    assert_head(&html, "/vin/KM8K2CAB4PU001140");
    assert_targets(&html, "/vin/KM8K2CAB4PU001140");
    // The VIN, in the fixed-width face, under the heading.
    assert!(html.contains(r#"<p class="vinline mono">KM8K2CAB4PU001140</p>"#));
    // The groups the fixture's Kona has, in reading order.
    let mut last = 0;
    for name in [
        "Vehicle",
        "Engine",
        "Transmission and drive",
        "Safety equipment",
        "Where it was built",
    ] {
        let at = html
            .find(&format!("<caption>{name}</caption>"))
            .unwrap_or_else(|| panic!("no group {name}:\n{html}"));
        assert!(at > last, "{name} is out of order");
        last = at;
    }
    // Groups it has nothing for are left out, not shown empty.
    assert!(!html.contains("<caption>Driver assistance</caption>"));
    assert!(!html.contains("<caption>Wheels and seats</caption>"));
    // The transmission is with the drive, not in the first group.
    let driveline = html
        .find("<caption>Transmission and drive</caption>")
        .unwrap();
    let transmission = html
        .find(r#"<tr><th scope="row">Transmission</th><td>Automatic</td></tr>"#)
        .unwrap();
    let safety = html.find("<caption>Safety equipment</caption>").unwrap();
    assert!(driveline < transmission && transmission < safety);
    // The manufacturer's code leads to its page.
    assert!(html.contains(
        r#"<tr><th scope="row">Manufacturer code</th><td><a href="/wmi/KM8">KM8</a></td></tr>"#
    ));
    // On from here: the model year, how to read a VIN, another VIN.
    assert!(html.contains(r#"<li><a href="/guides/how-to-read-a-vin">How to read a VIN</a></li>"#));
    assert!(html.contains(r#"<li><a href="/">Decode another VIN</a></li>"#));
    // Nothing on a result page is the page's one red action.
    assert_eq!(html.matches(r#"class="primary""#).count(), 0);

    // A wrong check digit is explained where it is reported.
    let (_, html) = page(&app, "/vin/KM8K2CAB0PU001140").await;
    let warning = html.split(r#"<p class="warning">"#).nth(1).unwrap();
    let warning = warning.split("</p>").next().unwrap();
    assert!(warning.contains(r#"<a href="/guides/check-digit">What the check digit is</a>"#));
    // The decoder never offers a different check digit as the correction:
    // any VIN can be made to pass that way. It offers a look-alike swap
    // elsewhere, here an 8 typed for the B in position 8.
    assert!(!warning.contains("Try "), "{warning}");
    let (status, html) = page(&app, "/vin/KM8K2CA84PU001140").await;
    assert_eq!(status, StatusCode::OK);
    let warning = html.split(r#"<p class="warning">"#).nth(1).unwrap();
    let warning = warning.split("</p>").next().unwrap();
    assert!(
        warning.contains(r#"<a href="/vin/KM8K2CAB4PU001140">Try KM8K2CAB4PU001140</a>"#),
        "{warning}"
    );
    assert!(warning.contains(r#"<a href="/guides/check-digit">What the check digit is</a>"#));
}

#[tokio::test]
async fn a_crawler_that_fetches_a_result_page_learns_nothing_to_index() {
    let app = common::app().await;
    let mut bodies = Vec::new();
    for agent in [
        "Mozilla/5.0 (compatible; GPTBot/1.2; +https://openai.com/gptbot)",
        "Mozilla/5.0 (compatible; ClaudeBot/1.0; +claudebot@anthropic.com)",
        "Mozilla/5.0 (compatible; Googlebot/2.1; +http://www.google.com/bot.html)",
        "Claude-User/1.0",
        "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) Safari/605.1.15",
    ] {
        let response = app
            .send(
                Request::get("/vin/KM8K2CAB4PU001140")
                    .header("user-agent", agent)
                    .body(Body::empty())
                    .unwrap(),
            )
            .await;
        assert_eq!(response.status(), StatusCode::OK, "{agent}");
        assert_eq!(header(&response, "x-robots-tag"), "noindex", "{agent}");
        assert_eq!(header(&response, "cache-control"), "private, max-age=3600");
        assert!(response.headers().get("etag").is_none(), "{agent}");
        assert!(response.headers().get("link").is_none(), "{agent}");
        let html = body_text(response).await;
        assert!(html.contains(r#"<meta name="robots" content="noindex">"#));
        for absent in [
            r#"rel="canonical""#,
            "og:url",
            "application/ld+json",
            r#"rel="alternate""#,
        ] {
            assert!(!html.contains(absent), "{agent}: {absent}");
        }
        bodies.push(html);
    }
    // Everyone is shown the same page: a crawler is not told one thing and a
    // person another.
    assert!(bodies.windows(2).all(|pair| pair[0] == pair[1]));
    // No sitemap, and neither file for language models, names a result page.
    for path in [
        "/sitemap.xml",
        "/sitemaps/pages.xml",
        "/sitemaps/makes.xml",
        "/sitemaps/wmi.xml",
        "/llms.txt",
    ] {
        let (_, text) = page(&app, path).await;
        assert!(!text.contains("open.example/vin/"), "{path}");
    }
}
