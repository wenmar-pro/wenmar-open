//! The wire gauge page, `/tools/wire-gauge`, through HTTP.

use axum::http::StatusCode;
use open_server::site::tools::wire_gauge::data::WIRE;
use open_server::site::tools::wire_gauge::result;

use crate::common::{
    self, assert_basics, assert_head, assert_no_injection, assert_targets, markdown, page,
};

const PAGE: &str = "/tools/wire-gauge";

#[tokio::test]
async fn the_wire_gauge_page_shows_what_the_data_file_holds() {
    let app = common::app().await;
    let (status, html) = page(&app, PAGE).await;
    assert_eq!(status, StatusCode::OK);
    assert_basics(&html, PAGE);
    assert_head(&html, PAGE);
    assert_targets(&html, PAGE);
    assert!(html.contains("<title>Wire gauge calculator for 12 V and 24 V - Wenmar Open</title>"));
    assert!(html.contains("This is voltage drop only."));
    assert!(html.contains("utm_campaign=tool-wire-gauge") && !html.contains("tax advice"));
    let text = markdown(&app, "/tools/wire-gauge.md").await;
    assert!(text.contains("\n## The wire table\n"));
    if !WIRE.confirmed {
        // Nothing has been read yet: no form, no result, and the page says so.
        assert!(html.contains(&format!(
            r#"<p class="warning">{}</p>"#,
            result::NOT_CHECKED
        )));
        assert!(!html.contains("<form") && !html.contains("<caption>"));
        assert!(text.contains(result::NOT_CHECKED));
        return;
    }
    assert!(html.contains("The form is filled in with an example."));
    assert!(html.contains("<caption>Voltage drop for each gauge</caption>"));
    assert!(html.contains(r#"<th scope="col">Percent of 12 V</th><th scope="col">Within 3%</th>"#));
    assert!(html.contains(&result::checked(&WIRE)));
    for gauge in WIRE.gauges {
        assert!(
            html.contains(&format!(r#"<tr><th scope="row">{}</th>"#, gauge.name)),
            "{}",
            gauge.name
        );
    }
    for source in WIRE
        .sources
        .iter()
        .filter(|source| !source.url.contains('&'))
    {
        assert!(
            html.contains(&format!(r#"<a href="{}">"#, source.url)),
            "{}",
            source.url
        );
    }
    let (_, html) = page(
        &app,
        "/tools/wire-gauge?system=24&current=15.5&length=10&unit=ft&allowed=",
    )
    .await;
    assert!(html.contains(r#"<th scope="col">Percent of 24 V</th><th scope="col">Within 3%</th>"#));
    let (_, html) = page(
        &app,
        "/tools/wire-gauge?system=48&current=%3Cb%3E&length=&unit=x&allowed=99",
    )
    .await;
    assert!(
        html.contains("There is no result yet.")
            && html.contains("System voltage: Choose 12 V or 24 V.")
    );
    assert_no_injection(&html);
}
