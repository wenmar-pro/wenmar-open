//! The catalog's reference pages and their Markdown versions.

use axum::http::StatusCode;

use crate::common::{self, assert_basics, assert_no_injection, header, markdown, page};

#[tokio::test]
async fn the_makes_page_lists_popular_makes_first() {
    let app = common::app().await;
    let (status, html) = page(&app, "/makes").await;
    assert_eq!(status, StatusCode::OK);
    assert_basics(&html, "/makes");
    assert!(html.contains("<h1>Makes</h1>"));
    assert!(html.contains(r#"<link rel="canonical" href="https://open.example/makes">"#));
    assert!(!html.contains(r#"name="robots""#), "the page is indexed");
    let popular = html.find("<h2>Popular</h2>").unwrap();
    let others = html.find("<h2>All others</h2>").unwrap();
    let ford = html.find(r#"<a href="/makes/ford">Ford</a>"#).unwrap();
    assert!(popular < ford && ford < others);
    assert!(!html.contains("ranger-trailers"));

    // For one year, only makes with a model that year, and not indexed.
    let (_, html) = page(&app, "/makes?year=2023").await;
    assert!(html.contains("<h1>Makes with a 2023 model</h1>"));
    assert!(html.contains(r#"<a href="/makes/hyundai?year=2023">Hyundai</a>"#));
    assert!(!html.contains("/makes/ford"));
    assert!(html.contains(r#"<meta name="robots" content="noindex">"#));
    let (status, html) = page(&app, "/makes?year=1066").await;
    assert_eq!(status, StatusCode::OK);
    assert!(html.contains("No makes are on file for that year."));
    // A year that is not a number is the same as no year.
    let (_, html) = page(&app, "/makes?year=%3Cscript%3E").await;
    assert!(html.contains("<h1>Makes</h1>"));
}

#[tokio::test]
async fn a_make_page_lists_its_models_for_a_year() {
    let app = common::app().await;
    let (status, html) = page(&app, "/makes/honda").await;
    assert_eq!(status, StatusCode::OK);
    assert_basics(&html, "/makes/honda");
    // Without a year, the newest the make has.
    assert!(html.contains("<h1>Honda models, 2020</h1>"), "{html}");
    assert!(html.contains(r#"<a href="/makes/honda/civic/2020">Civic</a>"#));
    assert!(!html.contains("CR-V"));
    assert!(html.contains(r#"<a href="/makes/honda?year=2019">2019</a>"#));
    assert!(html.contains(r#"<a href="/makes/honda?year=2020" aria-current="page">2020</a>"#));
    assert!(html.contains(r#"<link rel="canonical" href="https://open.example/makes/honda">"#));

    let (_, html) = page(&app, "/makes/honda?year=2019").await;
    assert!(html.contains("<h1>Honda models, 2019</h1>"));
    assert!(html.contains(r#"<a href="/makes/honda/cr-v/2019">CR-V</a>"#));
    assert!(html.contains(r#"<meta name="robots" content="noindex">"#));
    // A year the make was not built in falls back to the newest.
    for path in ["/makes/honda?year=1066", "/makes/honda?year=%3Cscript%3E"] {
        let (_, html) = page(&app, path).await;
        assert!(html.contains("<h1>Honda models, 2020</h1>"), "{path}");
    }
    // A make outside the default scope lists what it has.
    let (status, html) = page(&app, "/makes/ranger-trailers").await;
    assert_eq!(status, StatusCode::OK);
    assert!(html.contains(r#"<a href="/makes/ranger-trailers/tilt-deck/2019">Tilt Deck</a>"#));
}

#[tokio::test]
async fn a_make_given_by_name_or_alias_goes_to_its_one_address() {
    let app = common::app().await;
    for (path, to) in [
        ("/makes/Honda", "/makes/honda"),
        ("/makes/chevy", "/makes/chevrolet"),
        ("/makes/CHEVROLET.md", "/makes/chevrolet.md"),
    ] {
        let response = app.get(path).await;
        assert_eq!(response.status(), StatusCode::PERMANENT_REDIRECT, "{path}");
        assert_eq!(header(&response, "location"), to, "{path}");
    }
}

#[tokio::test]
async fn a_model_year_page_lists_trims_and_engines() {
    let app = common::app().await;
    let (status, html) = page(&app, "/makes/honda/civic/2019").await;
    assert_eq!(status, StatusCode::OK);
    assert_basics(&html, "/makes/honda/civic/2019");
    assert!(html.contains("<h1>2019 Honda Civic</h1>"));
    assert!(html.contains("<title>2019 Honda Civic trims and engines - Wenmar Open</title>"));
    assert!(
        html.contains(
            r#"<link rel="canonical" href="https://open.example/makes/honda/civic/2019">"#
        )
    );
    assert!(html.contains(r#"<tr><th scope="row">Drive</th><td>FWD</td></tr>"#));
    assert!(
        html.contains(r#"<tr><th scope="row">Si</th><td>Manual, FWD, Sedan</td></tr>"#),
        "{html}"
    );
    assert!(html.contains(r#"<tr><th scope="row">1.5L Turbo</th>"#));
    assert!(html.contains(r#"<td class="mono">2019_honda_civic</td>"#));

    let (_, html) = page(&app, "/makes/ford/f-150/2019").await;
    assert!(html.contains(r#"Eighth VIN character <span class="mono">5</span>"#));
    assert!(html.contains("from our own list, not NHTSA's"), "{html}");
    // A model year with nothing below it says so.
    let (_, html) = page(&app, "/makes/honda/cr-v/2019").await;
    assert!(html.contains("None on file."));
}

#[tokio::test]
async fn catalog_pages_for_things_that_do_not_exist_are_404() {
    let app = common::app().await;
    for path in [
        "/makes/nobody",
        "/makes/honda/civic/2021",
        "/makes/honda/civic/twenty",
        "/makes/honda/civic/99999",
        "/makes/honda/nothing/2019",
        "/makes/honda/civic",
        "/makes/honda/civic/2019/extra",
        "/makes/..%2F..%2Fetc%2Fpasswd",
        "/makes/honda/..%2F..%2F/2019",
        "/makes/honda/civic/..%2F2019",
        "/makes/%00",
        "/makes/honda/CIVIC/2019",
        "/makes/%3Cscript%3Ealert(1)%3C%2Fscript%3E",
        "/makes/nobody.md",
        "/makes/honda/civic/2021.md",
    ] {
        let (status, html) = page(&app, path).await;
        assert_eq!(status, StatusCode::NOT_FOUND, "{path}");
        assert!(html.contains("<h1>There is no page here</h1>"), "{path}");
        assert_no_injection(&html);
    }
}

#[tokio::test]
async fn html_in_catalog_names_is_shown_as_text() {
    let app = common::app().await;
    for path in [
        "/makes",
        "/makes/b-b-script",
        "/makes/b-b-script?year=2019",
        "/makes/b-b-script/model-img/2019",
    ] {
        let (status, html) = page(&app, path).await;
        assert_eq!(status, StatusCode::OK, "{path}");
        assert_no_injection(&html);
        assert!(html.contains("&#60;"), "{path}: the name is there, as text");
    }
    let (_, html) = page(&app, "/makes/b-b-script/model-img/2019").await;
    assert!(html.contains("&#60;b&#62;Bold&#60;/b&#62;"), "{html}");
    assert!(
        html.contains("Model &#34;&#60;img src=x onerror=alert(1)&#62;"),
        "{html}"
    );

    // The same names in Markdown cannot start markup either: every angle
    // bracket from the data has a backslash before it.
    for path in [
        "/makes.md",
        "/makes/b-b-script.md",
        "/makes/b-b-script/model-img/2019.md",
    ] {
        let text = markdown(&app, path).await;
        assert!(text.contains(r"\<"), "{path}: the name is there");
        let unescaped = text.replace(r"\<", "").replace(r"\>", "");
        assert!(
            !unescaped.contains('<') && !unescaped.contains('>'),
            "{path}:\n{text}"
        );
    }
}

#[tokio::test]
async fn every_catalog_page_has_a_markdown_version() {
    let app = common::app().await;
    let cases = [
        ("/makes.md", "# Makes\n", "- [Honda](/makes/honda.md)\n"),
        (
            "/makes/honda.md",
            "# Honda models, 2020\n",
            "- [Civic](/makes/honda/civic/2020.md)\n",
        ),
        (
            "/makes/honda/civic/2019.md",
            "# 2019 Honda Civic\n",
            "- Si: Manual, FWD, Sedan\n",
        ),
        (
            "/makes/ford/f-150/2019.md",
            "# 2019 Ford F\\-150\n",
            "- 5.0L V8 (eighth VIN character 5)\n",
        ),
    ];
    for (path, start, line) in cases {
        let text = markdown(&app, path).await;
        assert!(text.starts_with(start), "{path}:\n{text}");
        assert!(text.contains(line), "{path}:\n{text}");
    }
    // A year can be asked for in Markdown too.
    let (status, text) = page(&app, "/makes/honda.md?year=2019").await;
    assert_eq!(status, StatusCode::OK);
    assert!(text.starts_with("# Honda models, 2019\n"), "{text}");
    assert!(
        text.contains("- [CR\\-V](/makes/honda/cr-v/2019.md)\n"),
        "{text}"
    );
}
