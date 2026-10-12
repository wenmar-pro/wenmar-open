//! What the model-year page shows of the facts read by hand, and that it
//! shows nothing for a model year without a record.

use axum::http::StatusCode;

use crate::common::{self, assert_basics, assert_head, assert_targets, markdown, page};

/// What marks the sections that only a record brings.
const MARKS: [&str; 6] = [
    r#"<h2 id="fluids">Other fluids</h2>"#,
    r#"<h2 id="tires">Tires and wheels</h2>"#,
    "Read from the maker's document",
    "## Other fluids",
    "## Tires and wheels",
    "Report an error</a></span>",
];

/// The start of every report link on the Testmake page.
const ISSUE: &str = "https://github.com/wenmar-pro/wenmar-open/issues/new?template=data_correction.yml&#38;title=Data%20correction%3A%202001%20Testmake%20Sampler&#38;page=https%3A%2F%2Fopen.example%2Fmakes%2Ftestmake%2Fsampler%2F2001&#38;vehicle=2001%20Testmake%20Sampler";

#[tokio::test]
async fn the_page_shows_each_record_with_its_source_its_day_and_a_report_link() {
    let app = common::hand_app().await;
    let path = "/makes/testmake/sampler/2001";
    let (status, html) = page(&app, path).await;
    assert_eq!(status, StatusCode::OK);
    assert_basics(&html, path);
    assert_head(&html, path);
    assert_targets(&html, path);
    for line in [
        r#"<caption>Engine oil of the 2001 Testmake Sampler, by engine</caption>"#.to_owned(),
        // No EPA grade for a vehicle that does not exist: an empty cell.
        format!(
            r#"<tr><th scope="row">9.9L Test</th><td></td><td>0W-99, TEST-SPEC-1, 99.9 L (105.6 US qt) with filter <span class="note">Source: Test manual, page 1, checked 2026-10-12. <a href="{ISSUE}&#38;field=Engine%20oil%2C%209.9L%20Test">Report an error</a></span></td></tr>"#
        ),
        format!(
            r#"<tr><th scope="row">8.8L Test Diesel</th><td></td><td>TEST-SPEC-2, 83.3 L (88.0 US qt) without filter (Canada) <span class="note">Source: Test manual, page 2, checked 2026-10-11. <a href="{ISSUE}&#38;field=Engine%20oil%2C%208.8L%20Test%20Diesel">Report an error</a></span></td></tr>"#
        ),
        format!(
            r#"<tr><th scope="row">Transmission fluid</th><td>Test automatic</td><td>TEST ATF 9</td><td>Test manual, page 3, checked 2026-10-10. <a href="{ISSUE}&#38;field=Transmission%20fluid%2C%20Test%20automatic">Report an error</a></td></tr>"#
        ),
        format!(
            r#"<tr><th scope="row">Coolant</th><td>9.9L Test</td><td>TEST COOLANT 9</td><td>Test manual, page 4, checked 2026-10-10. <a href="{ISSUE}&#38;field=Coolant%2C%209.9L%20Test">Report an error</a></td></tr>"#
        ),
        format!(
            r#"<tr><th scope="row">A/C refrigerant</th><td>All</td><td>R-TEST, 9990 g (352.4 oz)</td><td>Test under-hood label, checked 2026-10-10. <a href="{ISSUE}&#38;field=A%2FC%20refrigerant">Report an error</a></td></tr>"#
        ),
        format!(
            r#"<tr><th scope="row">Brake fluid</th><td>All</td><td>TEST BRAKE 9</td><td>Test manual, page 5, checked 2026-10-10. <a href="{ISSUE}&#38;field=Brake%20fluid">Report an error</a></td></tr>"#
        ),
        format!(
            r#"<tr><th scope="row">999/99R99</th><td>Base, Deluxe</td><td>990 kPa (144 psi) front, 980 kPa (142 psi) rear</td><td>999 N·m (737 lb·ft)</td><td>9x999</td><td>Test door placard, checked 2026-10-11. <a href="{ISSUE}&#38;field=Tires%2C%20999%2F99R99">Report an error</a></td></tr>"#
        ),
        // Every fitment is listed: a second size, front and rear.
        format!(
            r#"<tr><th scope="row">998/98R98 front, 997/97R97 rear</th><td>Not given</td><td>990 kPa (144 psi) front and rear (United States)</td><td></td><td></td><td>Test manual, page 6, checked 2026-10-10. <a href="{ISSUE}&#38;field=Tires%2C%20998%2F98R98">Report an error</a></td></tr>"#
        ),
    ] {
        assert_eq!(html.matches(line.as_str()).count(), 1, "{line}\n{html}");
    }
    // After the engines, in this order, before the closing links.
    let at = |mark: &str| {
        html.find(mark)
            .unwrap_or_else(|| panic!("no {mark} in {html}"))
    };
    let order = [
        at("<caption>Engines</caption>"),
        at(r#"<h2 id="oil">Engine oil</h2>"#),
        at(MARKS[0]),
        at(MARKS[1]),
        at("Report an error on this page"),
    ];
    assert!(order.is_sorted(), "{order:?}");

    let text = markdown(&app, &format!("{path}.md")).await;
    let issue = ISSUE.replace("&#38;", "&");
    for line in [
        "\n## Engine oil\n\nThe grade in the second column is the one the maker certified the engine with, as EPA records it.".to_owned(),
        format!("| 9.9L Test |  | 0W\\-99, TEST\\-SPEC\\-1, 99.9 L \\(105.6 US qt\\) with filter | Test manual, page 1 | 2026-10-12 | [Report an error]({issue}&field=Engine%20oil%2C%209.9L%20Test) |\n"),
        "\n## Other fluids\n\n| Fluid | For | Specification | Source | Checked | |\n".to_owned(),
        format!("| Brake fluid | All | TEST BRAKE 9 | Test manual, page 5 | 2026-10-10 | [Report an error]({issue}&field=Brake%20fluid) |\n"),
        "\n## Tires and wheels\n\nEvery original-equipment size on record, with the trims it covers.".to_owned(),
        format!("| 999/99R99 | Base, Deluxe | 990 kPa (144 psi) front, 980 kPa (142 psi) rear | 999 N·m (737 lb·ft) | 9x999 | Test door placard | 2026-10-11 | [Report an error]({issue}&field=Tires%2C%20999%2F99R99) |\n"),
    ] {
        assert_eq!(text.matches(line.as_str()).count(), 1, "{line}\n{text}");
    }
}

#[tokio::test]
async fn an_engine_with_a_record_shows_it_beside_the_epa_grade_in_one_table() {
    let app = common::hand_app().await;
    let (status, html) = page(&app, "/makes/hyundai/kona/2023").await;
    assert_eq!(status, StatusCode::OK);
    assert_targets(&html, "/makes/hyundai/kona/2023");
    // One oil section, not EPA's table and then another.
    assert_eq!(html.matches(r#"<h2 id="oil">Engine oil</h2>"#).count(), 1);
    assert!(!html.contains("Engine oil viscosity grade of the 2023 Hyundai Kona"));
    for line in [
        r#"<tr><th scope="row">2.0L</th><td>0W-20</td><td>0W-99, TEST-SPEC-K, 77.7 L (82.1 US qt) with filter <span class="note">Source: Test manual K, page 1, checked 2026-10-09. <a href=""#,
        r#"<tr><th scope="row">1.6L Turbo</th><td>5W-30 or 0W-30</td><td>TEST-SPEC-T <span class="note">Source: Test manual K, page 2, checked 2026-10-09. <a href=""#,
        r#"<tr><th scope="row">996/96R96</th><td>SE</td><td></td><td></td><td></td><td>Test door placard K, checked 2026-10-09. <a href=""#,
    ] {
        assert_eq!(html.matches(line).count(), 1, "{line}\n{html}");
    }
    // Which source each figure came from is said on the page.
    assert!(html.contains("Grade, per EPA fuel economy certification"));
    assert!(
        html.contains("The third column was read by a person from the document named beside it")
    );
    // The fuel table is untouched.
    assert!(html.contains(r#"<h2 id="fuel">Fuel economy</h2>"#));

    let text = markdown(&app, "/makes/hyundai/kona/2023.md").await;
    assert_eq!(text.matches("\n## Engine oil\n").count(), 1, "{text}");
    assert!(text.contains("| 2.0L | 0W\\-20 | 0W\\-99, TEST\\-SPEC\\-K, 77.7 L \\(82.1 US qt\\) with filter | Test manual K, page 1 | 2026-10-09 | [Report an error]("), "{text}");
}

#[tokio::test]
async fn a_model_year_without_a_record_shows_nothing_in_its_place() {
    // The 2019 Civic has an EPA grade and no record: EPA's own table,
    // as before, and none of the sections a record brings.
    let app = common::hand_app().await;
    let (status, html) = page(&app, "/makes/honda/civic/2019").await;
    assert_eq!(status, StatusCode::OK);
    let text = markdown(&app, "/makes/honda/civic/2019.md").await;
    for mark in MARKS {
        assert!(!html.contains(mark), "{mark}");
        assert!(!text.contains(mark), "{mark}");
    }
    assert!(html.contains(
        "Engine oil viscosity grade of the 2019 Honda Civic, per EPA fuel economy certification"
    ));
    // A server with no records: the page is what it was.
    let with = page(&common::fuel_app().await, "/makes/hyundai/kona/2023").await;
    assert_eq!(with.0, StatusCode::OK);
    for mark in MARKS {
        assert!(!with.1.contains(mark), "{mark}");
    }
    // And a server with no shop file at all.
    let (_, plain) = page(&common::app().await, "/makes/honda/civic/2019").await;
    for mark in MARKS {
        assert!(!plain.contains(mark), "{mark}");
    }
    assert!(!plain.contains(r#"<h2 id="oil">"#));
}
