//! The battery page, `/tools/battery-state-of-charge`, through HTTP.

use axum::http::StatusCode;
use open_server::site::tools::battery_soc::data::BATTERIES;
use open_server::site::tools::battery_soc::result;
use shop_math::battery_soc::volts;

use crate::common::{
    self, assert_basics, assert_head, assert_no_injection, assert_targets, markdown, page,
};

const PAGE: &str = "/tools/battery-state-of-charge";

#[tokio::test]
async fn the_battery_page_shows_what_the_data_file_holds() {
    let app = common::app().await;
    let (status, html) = page(&app, PAGE).await;
    assert_eq!(status, StatusCode::OK);
    assert_basics(&html, PAGE);
    assert_head(&html, PAGE);
    assert_targets(&html, PAGE);
    assert!(
        html.contains("<title>Battery state of charge from resting voltage - Wenmar Open</title>")
    );
    assert!(html.contains("a worn battery can read full"));
    assert!(
        html.contains("utm_campaign=tool-battery-state-of-charge") && !html.contains("tax advice")
    );
    let text = markdown(&app, "/tools/battery-state-of-charge.md").await;
    for battery in &BATTERIES {
        assert!(
            text.contains(&format!("\n## {}\n", battery.name)),
            "{}",
            battery.key
        );
        if !battery.confirmed {
            assert!(
                html.contains(&format!(
                    "<p>{}: {}</p>",
                    battery.name,
                    battery.missing.join(" ")
                )),
                "{}",
                battery.key
            );
        }
    }
    let Some(first) = BATTERIES.iter().find(|battery| battery.confirmed) else {
        // Nothing has been read yet: no form, no result, and the page says so.
        assert!(html.contains(&format!(
            r#"<p class="warning">{}</p>"#,
            result::NONE_CHECKED
        )));
        assert!(!html.contains("<form") && !html.contains("<caption>"));
        return;
    };
    // The example is the middle row of the first checked table, so it is on a row.
    assert!(html.contains("The form is filled in with an example."));
    let (millivolts, _) = first.rows[first.rows.len() / 2];
    assert!(html.contains(&format!("<strong>{} V is on the table", volts(millivolts))));
    assert!(html.contains(&format!(
        "<caption>{}</caption>",
        result::chart(first).caption
    )));
    assert!(html.contains(&result::checked(first)));
    for source in first
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
    // A reading below every row is said to be below, and what cannot be read is reported.
    let (_, html) = page(
        &app,
        &format!(
            "{PAGE}?battery={}&volts=0.001&temperature=&unit=c",
            first.key
        ),
    )
    .await;
    assert!(html.contains("<strong>0.001 V is below the table"));
    let (_, html) = page(
        &app,
        &format!("{PAGE}?battery=%3Cscript%3E&volts=abc&temperature=%3Cb%3E&unit=k"),
    )
    .await;
    assert!(html.contains("There is no result yet."));
    assert!(html.contains("Kind of battery: Choose a kind of battery from the list."));
    assert_no_injection(&html);
}
