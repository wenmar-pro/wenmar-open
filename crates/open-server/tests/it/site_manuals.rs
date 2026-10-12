//! Links to a model year's manual and maintenance schedule on the
//! model-year page, and that the page says nothing where there is none.

use axum::http::StatusCode;

use crate::common::{self, assert_targets, markdown, page};

const DATASETS: &str =
    r#"{"manuals":{"as_of":"2026-10-12","rows":2,"licence":"mit","attribution":""}}"#;

/// Links for the vehicle that does not exist. `example.org` is a name
/// reserved for examples. The columns are those of
/// `wenmar_shopdata::schema::MANUALS`, in order.
const INSERTS: &str = "
INSERT INTO manual_link VALUES
  (1, 7000, 7001, 2001, 'maintenance_schedule', 'https://example.org/testmake/sampler/2001/schedule?a=1&b=2', 'ca', '2026-10-11', 'test-reviewer'),
  (2, 7000, 7001, 2001, 'owner_manual', 'https://example.org/testmake/sampler/2001/manual.pdf', NULL, '2026-10-12', 'test-reviewer');
";

const MARKS: [&str; 2] = [
    r#"<h2 id="manuals">"#,
    "## Owner's manual and maintenance schedule",
];

#[tokio::test]
async fn the_page_links_to_the_documents_a_person_found_with_the_day_each_was_checked() {
    let rows = format!("{}{INSERTS}", wenmar_shopdata::schema::MANUALS);
    let app = common::app_with_rows_and_shop(common::TESTMAKE_ROWS, DATASETS, &rows).await;
    let path = "/makes/testmake/sampler/2001";
    let (status, html) = page(&app, path).await;
    assert_eq!(status, StatusCode::OK);
    assert_targets(&html, path);
    // The owner's manual first; an address is escaped, never rewritten.
    let list = concat!(
        "<h2 id=\"manuals\">Owner's manual and maintenance schedule</h2>\n",
        "<ul class=\"plain links\">\n",
        "<li><a href=\"https://example.org/testmake/sampler/2001/manual.pdf\">Owner&#39;s manual for the 2001 Testmake Sampler</a> <span class=\"note\">Checked 2026-10-12.</span></li>\n",
        "<li><a href=\"https://example.org/testmake/sampler/2001/schedule?a=1&#38;b=2\">Maintenance schedule for the 2001 Testmake Sampler (Canada)</a> <span class=\"note\">Checked 2026-10-11.</span></li>\n",
        "</ul>\n",
        "<p class=\"note\">These lead to the maker's own site, which this site does not run. No manual is copied or kept here.</p>\n",
    );
    assert_eq!(html.matches(list).count(), 1, "{html}");
    // After the engines, before the closing links.
    let at = |mark: &str| html.find(mark).unwrap();
    assert!(at("<caption>Engines</caption>") < at(MARKS[0]));
    assert!(at(MARKS[0]) < at("Report an error on this page"));

    let text = markdown(&app, &format!("{path}.md")).await;
    assert!(text.contains("\n## Owner's manual and maintenance schedule\n\n- [Owner's manual for the 2001 Testmake Sampler](https://example.org/testmake/sampler/2001/manual.pdf) Checked 2026\\-10\\-12.\n- [Maintenance schedule for the 2001 Testmake Sampler \\(Canada\\)](https://example.org/testmake/sampler/2001/schedule?a=1&b=2) Checked 2026\\-10\\-11.\n\nThese lead to the maker's own site, which this site does not run. No manual is copied or kept here.\n"), "{text}");

    // A model year with no link, on the same server.
    let (_, html) = page(&app, "/makes/honda/civic/2019").await;
    let text = markdown(&app, "/makes/honda/civic/2019.md").await;
    for mark in MARKS {
        assert!(!html.contains(mark) && !text.contains(mark), "{mark}");
    }
}

#[tokio::test]
async fn a_server_without_links_shows_the_page_as_it_was() {
    for app in [common::app().await, common::hand_app().await] {
        let (status, html) = page(&app, "/makes/hyundai/kona/2023").await;
        assert_eq!(status, StatusCode::OK);
        let text = markdown(&app, "/makes/hyundai/kona/2023.md").await;
        for mark in MARKS {
            assert!(!html.contains(mark) && !text.contains(mark), "{mark}");
        }
    }
}
