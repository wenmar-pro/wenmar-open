//! What the model-year page says of fuel consumption and engine oil, and
//! that it says nothing on a server that holds neither.

use axum::http::StatusCode;

use crate::common::{self, assert_basics, assert_head, assert_targets, markdown, page};

/// What marks the two sections.
const MARKS: [&str; 5] = [
    r#"<h2 id="fuel">Fuel economy</h2>"#,
    r#"<h2 id="oil">Engine oil</h2>"#,
    "## Fuel economy",
    "## Engine oil",
    "L/100 km",
];

#[tokio::test]
async fn the_model_year_page_shows_fuel_by_engine_and_the_certified_oil_grade() {
    let app = common::fuel_app().await;
    let (status, html) = page(&app, "/makes/hyundai/kona/2023").await;
    assert_eq!(status, StatusCode::OK);
    assert_basics(&html, "/makes/hyundai/kona/2023");
    assert_head(&html, "/makes/hyundai/kona/2023");
    assert_targets(&html, "/makes/hyundai/kona/2023");
    // L/100 km first, MPG beside; a range over the two drives of the 2.0L.
    for line in [
        r#"<tr><th scope="row">2.0L</th><td>EPA</td><td>7.4 to 7.8 L/100 km (30 to 32 MPG)</td><td>7.8 to 8.4 L/100 km (28 to 30 MPG)</td><td>6.7 to 7.1 L/100 km (33 to 35 MPG)</td><td>Regular Gasoline</td></tr>"#,
        r#"<tr><th scope="row">1.6L Turbo</th><td>EPA</td><td>8.1 L/100 km (29 MPG)</td><td>8.7 L/100 km (27 MPG)</td><td>7.4 L/100 km (32 MPG)</td><td>Regular Gasoline</td></tr>"#,
        r#"<tr><th scope="row">2.0L</th><td>NRCan</td><td>7.3 L/100 km (32 MPG)</td><td>7.9 L/100 km (30 MPG)</td><td>6.6 L/100 km (36 MPG)</td><td>Regular Gasoline</td></tr>"#,
        // Matched to the model year only: no catalog engine is named.
        r#"<tr><th scope="row">Electric <span class="note">not matched to an engine</span></th><td>EPA</td><td>17.4 kWh/100 km</td><td></td><td></td><td>Electricity</td></tr>"#,
        r#"<caption>Engine oil viscosity grade of the 2023 Hyundai Kona, per EPA fuel economy certification</caption>"#,
        r#"<tr><th scope="row">2.0L</th><td>0W-20</td></tr>"#,
        r#"<tr><th scope="row">1.6L Turbo</th><td>5W-30 or 0W-30</td></tr>"#,
    ] {
        assert_eq!(html.matches(line).count(), 1, "{line}\n{html}");
    }
    assert!(html.contains(
        "EPA ratings as of 2026-09-29. Natural Resources Canada ratings as of 2026-10-06."
    ));
    assert!(html.contains(common::OGL_CANADA));
    assert!(html.contains("It is not a reading of the owner's manual"));
    // After the engines, before the other years.
    let engines = html.find("<caption>Engines</caption>").unwrap();
    let fuel = html.find(MARKS[0]).unwrap();
    let oil = html.find(MARKS[1]).unwrap();
    assert!(engines < fuel && fuel < oil);

    let text = markdown(&app, "/makes/hyundai/kona/2023.md").await;
    assert!(text.contains("\n## Fuel economy\n\n| Engine | Source | Combined | City | Highway | Recommended fuel |\n"), "{text}");
    assert!(text.contains("| 2.0L | EPA | 7.4 to 7.8 L/100 km (30 to 32 MPG) | 7.8 to 8.4 L/100 km (28 to 30 MPG) | 6.7 to 7.1 L/100 km (33 to 35 MPG) | Regular Gasoline |\n"), "{text}");
    assert!(
        text.contains(
            "| Electric (not matched to an engine) | EPA | 17.4 kWh/100 km |  |  | Electricity |\n"
        ),
        "{text}"
    );
    assert!(
        text.contains("\n## Engine oil\n\nViscosity grade per EPA fuel economy certification."),
        "{text}"
    );
    assert!(text.contains("- 1.6L Turbo: 5W-30 or 0W-30\n"), "{text}");

    // No engine settled: the source's own engine size, marked.
    let (_, html) = page(&app, "/makes/honda/civic/2019").await;
    assert!(html.contains(r#"<tr><th scope="row">1.8 L, 4 cylinders <span class="note">not matched to an engine</span></th><td>0W-20</td></tr>"#), "{html}");
}

#[tokio::test]
async fn without_fuel_data_the_page_is_as_it_was() {
    // A model year with nothing on file, on a server that holds the data.
    let app = common::fuel_app().await;
    let (_, html) = page(&app, "/makes/ford/f-150/2019").await;
    let text = markdown(&app, "/makes/ford/f-150/2019.md").await;
    for mark in MARKS {
        assert!(!html.contains(mark), "{mark}");
        assert!(!text.contains(mark), "{mark}");
    }
    // A server with no shop file, and one that holds only recalls.
    for app in [common::app().await, common::recalls_app().await] {
        let (status, html) = page(&app, "/makes/hyundai/kona/2023").await;
        assert_eq!(status, StatusCode::OK);
        let text = markdown(&app, "/makes/hyundai/kona/2023.md").await;
        for mark in MARKS {
            assert!(!html.contains(mark), "{mark}");
            assert!(!text.contains(mark), "{mark}");
        }
    }
    // EPA's ratings alone: the fuel table, and no oil section.
    let app = common::app_with_shop(common::FUEL_DATASETS_US, &common::fuel_rows()).await;
    let (_, html) = page(&app, "/makes/hyundai/kona/2023").await;
    assert!(html.contains(MARKS[0]));
    assert!(!html.contains(MARKS[1]));
    assert!(!html.contains("Natural Resources Canada"));
}
