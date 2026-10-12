//! The refrigerant chart, `/tools/refrigerant-pt`, through HTTP.

use axum::http::StatusCode;
use open_server::site::tools::refrigerant_pt::data::REFRIGERANTS;
use open_server::site::tools::refrigerant_pt::result;

use crate::common::{self, assert_basics, assert_head, assert_targets, markdown, page};

const PAGE: &str = "/tools/refrigerant-pt";

#[tokio::test]
async fn the_chart_shows_each_refrigerant_as_the_data_file_holds_it() {
    let app = common::app().await;
    let (status, html) = page(&app, PAGE).await;
    assert_eq!(status, StatusCode::OK);
    assert_basics(&html, PAGE);
    assert_head(&html, PAGE);
    assert_targets(&html, PAGE);
    assert!(
        html.contains(
            "<title>R-134a and R-1234yf pressure-temperature chart - Wenmar Open</title>"
        )
    );
    assert!(
        html.contains(r#"<link rel="canonical" href="https://open.example/tools/refrigerant-pt">"#)
    );
    assert!(html.contains("This chart is not a charging procedure"));
    assert!(!html.contains("<form") && !html.contains("<input") && !html.contains("tax advice"));
    assert!(html.contains("utm_campaign=tool-refrigerant-pt"));
    assert!(
        html.contains(r#"<h2 id="r134a">R-134a</h2>"#)
            && html.contains(r#"<h2 id="r1234yf">R-1234yf</h2>"#)
    );
    let text = markdown(&app, "/tools/refrigerant-pt.md").await;
    for refrigerant in &REFRIGERANTS {
        if !refrigerant.confirmed {
            assert!(
                html.contains(&format!(
                    r#"<p class="warning">{}</p>"#,
                    result::not_checked(refrigerant)
                )),
                "{}",
                refrigerant.key
            );
            assert!(!html.contains(&format!("<caption>{}:", refrigerant.name)));
            assert!(text.contains(&result::not_checked(refrigerant)));
            continue;
        }
        let table = result::table(refrigerant);
        assert!(html.contains(&format!("<caption>{}</caption>", table.caption)));
        assert!(html.contains(&result::checked(refrigerant)));
        assert!(table.rows.len() >= 2);
        for row in &table.rows {
            let cells: String = row[1..]
                .iter()
                .map(|cell| format!("<td>{cell}</td>"))
                .collect();
            assert!(
                html.contains(&format!(
                    r#"<tr><th scope="row">{}</th>{cells}</tr>"#,
                    row[0]
                )),
                "{row:?}"
            );
            assert!(
                text.contains(&format!("| {} |\n", row.join(" | "))),
                "{row:?}"
            );
        }
        for source in refrigerant
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
    }
}
