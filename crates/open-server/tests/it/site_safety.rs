//! The "Crash test ratings" section of the model-year page, and that the
//! page says nothing of ratings where there is nothing to say.

use axum::http::StatusCode;

use crate::common::{
    self, assert_basics, assert_head, assert_no_injection, assert_targets, markdown, page,
};

/// What marks the section on the page and in its Markdown.
const MARKS: [&str; 5] = [
    r#"id="safety""#,
    "Crash test ratings",
    "crash test ratings",
    "/safety",
    "NHTSA's ratings",
];

#[tokio::test]
async fn the_model_year_page_has_every_rating_with_nhtsas_description() {
    let app = common::ratings_app().await;
    let path = "/makes/hyundai/kona/2023";
    let (status, html) = page(&app, path).await;
    assert_eq!(status, StatusCode::OK);
    assert_basics(&html, path);
    assert_head(&html, path);
    assert_targets(&html, path);
    assert_eq!(
        html.matches(r#"<h2 id="safety">Crash test ratings</h2>"#)
            .count(),
        1,
        "{html}"
    );
    assert!(
        html.contains("<caption>NHTSA crash test ratings for the 2023 Hyundai Kona</caption>"),
        "{html}"
    );
    // One row for each rating, none picked, in NHTSA's words.
    let rows = [
        r#"<tr><th scope="row">KONA, SUV, AWD</th><td>Overall 5 of 5. Frontal 4. Side 5. Rollover 4. Rollover risk 14.8%. Tested at 1463 kg (3225 lb).</td></tr>"#,
        r#"<tr><th scope="row">KONA, SUV, FWD</th><td>Overall 4 of 5. Frontal 4. Side 5. Rollover 4. Rollover risk 15.5%. Tested at 1363 kg (3005 lb). <span class="note">These ratings do not apply to the N Line.</span> <span class="note">The rear door opened in the side test.</span></td></tr>"#,
        r#"<tr><th scope="row">KONA ELECTRIC, SUV, FWD</th><td>Not rated by NHTSA.</td></tr>"#,
    ];
    let places: Vec<usize> = rows
        .iter()
        .map(|row| {
            html.find(row)
                .unwrap_or_else(|| panic!("no {row}:\n{html}"))
        })
        .collect();
    assert!(places[0] < places[1] && places[1] < places[2]);
    assert!(html.contains(
        r#"<p class="note">Stars are NHTSA's, from 1 to 5, each for the vehicle NHTSA names in its row: a model year can have several ratings, and they can differ. From NHTSA's file as of 2026-08-07. See <a href="https://www.nhtsa.gov/ratings">NHTSA's ratings</a>, or <a href="/v1/vehicles/2023_hyundai_kona/safety">the JSON</a>, which also lists safety features.</p>"#
    ), "{html}");
    // After the engines, before the other years and the links.
    let engines = html.find("<caption>Engines</caption>").unwrap();
    let section = html.find(r#"<h2 id="safety">"#).unwrap();
    let years = html.find(r#"<h2 id="years">"#).unwrap();
    assert!(engines < section && section < years);
    // The page is still the page it was.
    assert!(html.contains("<title>2023 Hyundai Kona trims and engines - Wenmar Open</title>"));
    assert!(!html.contains(r#"name="robots""#));
    assert!(!html.contains(r#"<th scope="row">Recalls</th>"#));
    // The link to the JSON leads somewhere.
    assert_eq!(
        app.get("/v1/vehicles/2023_hyundai_kona/safety")
            .await
            .status(),
        StatusCode::OK
    );

    // A truck, with its cab in the description.
    let (_, html) = page(&app, "/makes/ford/f-150/2019").await;
    assert!(html.contains(
        r#"<tr><th scope="row">F-150 SUPER CREW, PU/CC, 4x4</th><td>Overall 5 of 5. Frontal 5. Side 5. Rollover 4. Rollover risk 19.1%. Tested at 2295 kg (5060 lb).</td></tr>"#
    ), "{html}");
    // A later release.
    let (_, html) = page(&app, "/makes/hyundai/kona/2022").await;
    assert!(
        html.contains(r#"<th scope="row">KONA, SUV, FWD, later release</th>"#),
        "{html}"
    );
}

#[tokio::test]
async fn the_markdown_version_has_the_same_ratings() {
    let app = common::ratings_app().await;
    let text = markdown(&app, "/makes/hyundai/kona/2023.md").await;
    // Markdown escapes the brackets around the pounds.
    let section = "\n## Crash test ratings\n\n- KONA, SUV, AWD: Overall 5 of 5. Frontal 4. Side 5. Rollover 4. Rollover risk 14.8%. Tested at 1463 kg \\(3225 lb\\).\n- KONA, SUV, FWD: Overall 4 of 5. Frontal 4. Side 5. Rollover 4. Rollover risk 15.5%. Tested at 1363 kg \\(3005 lb\\). These ratings do not apply to the N Line. The rear door opened in the side test.\n- KONA ELECTRIC, SUV, FWD: Not rated by NHTSA.\n\nStars are NHTSA's, from 1 to 5, each for the vehicle NHTSA names. From NHTSA's file as of 2026-08-07. [NHTSA's ratings](https://www.nhtsa.gov/ratings). JSON, with safety features: [/v1/vehicles/2023_hyundai_kona/safety](/v1/vehicles/2023_hyundai_kona/safety)\n";
    assert_eq!(text.matches(section).count(), 1, "{text}");
    // After what the page always had.
    let json = text
        .find("\nJSON: [/v1/vehicles/2023_hyundai_kona]")
        .unwrap();
    assert!(json < text.find("## Crash test ratings").unwrap());
}

#[tokio::test]
async fn ratings_and_recalls_are_both_on_the_page() {
    let app = common::ratings_and_recalls_app().await;
    let (_, html) = page(&app, "/makes/hyundai/kona/2023").await;
    assert!(html.contains(r#"<h2 id="safety">Crash test ratings</h2>"#));
    assert!(html.contains(
        r#"<tr><th scope="row">Recalls</th><td><a href="/makes/hyundai/kona/2023/recalls">4 recall campaigns may apply</a></td></tr>"#
    ), "{html}");
    // The Markdown still ends with the recalls; the ratings come before.
    let text = markdown(&app, "/makes/hyundai/kona/2023.md").await;
    assert!(
        text.ends_with(
            "\n## Recalls\n\n[4 recall campaigns may apply](/makes/hyundai/kona/2023/recalls.md)\n"
        ),
        "{text}"
    );
    assert!(text.find("## Crash test ratings").unwrap() < text.find("## Recalls").unwrap());
}

#[tokio::test]
async fn words_from_the_file_are_never_markup() {
    let app = common::ratings_app().await;
    let (status, html) = page(&app, "/makes/b-b-script/model-img/2019").await;
    assert_eq!(status, StatusCode::OK);
    assert!(html.contains(r#"<h2 id="safety">Crash test ratings</h2>"#));
    assert_no_injection(&html);
    assert!(
        html.contains("Frontal: driver 4, passenger 4. Side: driver 4, rear passenger not rated. Rollover 3. Rollover risk 20.0%."),
        "{html}"
    );
    let text = markdown(&app, "/makes/b-b-script/model-img/2019.md").await;
    assert!(!text.contains("<script>alert"), "{text}");
}

#[tokio::test]
async fn without_ratings_the_page_is_as_it_was() {
    // No shop file; a shop file with no datasets; one that holds recalls
    // and no ratings; and one that holds ratings, for a model year NHTSA
    // lists nothing for.
    let none = common::app().await;
    let empty = common::app_with_shop("{}", "").await;
    let rated = common::ratings_app().await;
    for path in [
        "/makes/hyundai/kona/2023",
        "/makes/hyundai/kona/2023.md",
        "/makes/honda/civic/2019",
        "/makes/honda/civic/2019.md",
        "/makes/ranger-trailers/tilt-deck/2019",
    ] {
        let (status, expected) = page(&none, path).await;
        assert_eq!(status, StatusCode::OK, "{path}");
        for mark in MARKS {
            assert!(
                !expected.contains(mark),
                "{path} has {mark:?} with no ratings"
            );
        }
        let (_, html) = page(&empty, path).await;
        assert_eq!(html, expected, "{path}");
    }
    // A model year with no rating, on a server that holds ratings.
    for path in [
        "/makes/honda/civic/2019",
        "/makes/honda/civic/2019.md",
        "/makes/ranger-trailers/tilt-deck/2019",
    ] {
        let (_, expected) = page(&none, path).await;
        let (_, html) = page(&rated, path).await;
        assert_eq!(html, expected, "{path}");
    }
    // Recalls alone add the recall line and nothing of ratings.
    let recalls = common::recalls_app().await;
    for path in ["/makes/hyundai/kona/2023", "/makes/hyundai/kona/2023.md"] {
        let (_, html) = page(&recalls, path).await;
        for mark in MARKS {
            assert!(
                !html.contains(mark),
                "{path} has {mark:?} with recalls only"
            );
        }
    }
    // The VIN result page is not changed by ratings.
    let (_, expected) = page(&none, "/vin/KM8K2CAB4PU001140").await;
    let (_, html) = page(&rated, "/vin/KM8K2CAB4PU001140").await;
    assert_eq!(html, expected);
}
