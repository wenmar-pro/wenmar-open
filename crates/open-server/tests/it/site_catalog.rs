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

/// A make of light trucks that went on to build only buses, under one model
/// name. Its light model years are 2008 and 2010: nothing in 2009, and the
/// years after 2010 are not light. Type bits: truck 8, bus 32.
const MIXED_MAKE: &str = "
INSERT INTO catalog_make VALUES
  (8000, 'mixed-works', 'Mixed Works', 'mixedworks', NULL, 40, 1);
INSERT INTO catalog_model VALUES
  (9800, 8000, 'hauler', 'Hauler', 'hauler', 2008, 2015, 40, 1);
INSERT INTO catalog_vehicle VALUES
  (20, 2008, 8000, 9800, 8, 1, NULL),
  (21, 2010, 8000, 9800, 8, 1, 7),
  (22, 2013, 8000, 9800, 32, 0, NULL),
  (23, 2015, 8000, 9800, 32, 0, NULL);
INSERT INTO catalog_detail VALUES (7, NULL, NULL, NULL);
INSERT INTO catalog_engine VALUES (10, 7, '6.0L V8', '349', 'vpic');
";

#[tokio::test]
async fn a_make_page_shows_the_newest_year_it_has_models_in_and_links_only_such_years() {
    let app = common::app_with_rows(MIXED_MAKE).await;
    // The page without a year is the one in the sitemap. It must not be
    // empty because the model's range runs on into years that are not light.
    let (status, html) = page(&app, "/makes/mixed-works").await;
    assert_eq!(status, StatusCode::OK);
    assert!(html.contains("<h1>Mixed Works models, 2010</h1>"), "{html}");
    assert!(html.contains(r#"<a href="/makes/mixed-works/hauler/2010">Hauler</a>"#));
    assert!(!html.contains("are on file for"), "{html}");
    assert!(!html.contains(r#"name="robots""#), "the page is indexed");
    // Only years with a model are linked: not the gap, not the bus years.
    assert!(
        html.contains(r#"<a href="/makes/mixed-works?year=2010" aria-current="page">2010</a>"#)
    );
    assert!(html.contains(r#"<a href="/makes/mixed-works?year=2008">2008</a>"#));
    assert_eq!(
        html.matches("/makes/mixed-works?year=").count(),
        2,
        "{html}"
    );

    // A year between or after them falls back to the newest, as any year
    // the make has nothing in does.
    for year in [2009, 2013, 2015, 2027] {
        let (_, html) = page(&app, &format!("/makes/mixed-works?year={year}")).await;
        assert!(html.contains("<h1>Mixed Works models, 2010</h1>"), "{year}");
        assert!(html.contains("/makes/mixed-works/hauler/2010"), "{year}");
    }
    let (_, html) = page(&app, "/makes/mixed-works?year=2008").await;
    assert!(html.contains("<h1>Mixed Works models, 2008</h1>"));
    assert!(html.contains(r#"<a href="/makes/mixed-works/hauler/2008">Hauler</a>"#));

    let text = markdown(&app, "/makes/mixed-works.md").await;
    assert!(text.starts_with("# Mixed Works models, 2010\n"), "{text}");
    assert!(text.contains("- [Hauler](/makes/mixed-works/hauler/2010.md)\n"));
    assert!(
        text.ends_with(
            "[2010](/makes/mixed-works.md?year=2010) [2008](/makes/mixed-works.md?year=2008)\n"
        ),
        "{text}"
    );

    // Every make in the sitemap has something to show.
    let (_, xml) = page(&app, "/sitemaps/makes.xml").await;
    assert!(xml.contains("/makes/mixed-works</loc>"));
    for entry in xml.split("<loc>https://open.example").skip(1) {
        let path = entry.split("</loc>").next().unwrap();
        let (_, html) = page(&app, path).await;
        assert!(!html.contains("are on file for"), "{path}");
    }
}

#[tokio::test]
async fn the_eighth_character_is_shown_as_the_alternatives_it_is() {
    let app = common::app_with_rows(MIXED_MAKE).await;
    // One character.
    let (_, html) = page(&app, "/makes/ford/f-150/2019").await;
    assert!(
        html.contains(
            r#"<tr><th scope="row">5.0L V8</th><td>Eighth VIN character <span class="mono">5</span></td></tr>"#
        ),
        "{html}"
    );
    // Two: either one means this engine.
    assert!(
        html.contains(
            r#"<tr><th scope="row">3.5L Turbo V6</th><td>Eighth VIN character <span class="mono">4</span> or <span class="mono">G</span></td></tr>"#
        ),
        "{html}"
    );
    // Three or more.
    let (_, html) = page(&app, "/makes/mixed-works/hauler/2010").await;
    assert!(
        html.contains(
            r#"<tr><th scope="row">6.0L V8</th><td>Eighth VIN character <span class="mono">3</span>, <span class="mono">4</span> or <span class="mono">9</span></td></tr>"#
        ),
        "{html}"
    );
    // An engine the eighth character does not settle says nothing of it.
    let (_, html) = page(&app, "/makes/honda/civic/2019").await;
    assert!(html.contains(r#"<tr><th scope="row">1.5L Turbo</th><td></td></tr>"#));

    let text = markdown(&app, "/makes/ford/f-150/2019.md").await;
    assert!(
        text.contains("- 5.0L V8 (eighth VIN character 5)\n"),
        "{text}"
    );
    assert!(
        text.contains("- 3.5L Turbo V6 (eighth VIN character 4 or G)\n"),
        "{text}"
    );
    let text = markdown(&app, "/makes/mixed-works/hauler/2010.md").await;
    assert!(
        text.contains("- 6.0L V8 (eighth VIN character 3, 4 or 9)\n"),
        "{text}"
    );
}

#[tokio::test]
async fn pages_for_trailers_and_buses_exist_but_are_not_offered_to_search_engines() {
    let app = common::app_with_rows(MIXED_MAKE).await;
    for path in [
        // A trailer maker and its one model year.
        "/makes/ranger-trailers",
        "/makes/ranger-trailers/tilt-deck/2019",
        // The bus years of a make that also built light trucks.
        "/makes/mixed-works/hauler/2013",
        "/makes/mixed-works/hauler/2015",
    ] {
        let (status, html) = page(&app, path).await;
        assert_eq!(status, StatusCode::OK, "{path}");
        assert_basics(&html, path);
        assert!(
            html.contains(r#"<meta name="robots" content="noindex">"#),
            "{path} may be indexed"
        );
        assert!(!html.contains(r#"rel="canonical""#), "{path}");
        // An agent can still ask for it as Markdown.
        assert!(
            html.contains(&format!(
                r#"<link rel="alternate" type="text/markdown" href="{path}.md">"#
            )),
            "{path}"
        );
    }
    // Light vehicles are indexed as before, the light years of a mixed make
    // among them.
    for path in [
        "/makes/honda",
        "/makes/honda/civic/2019",
        "/makes/mixed-works",
        "/makes/mixed-works/hauler/2010",
    ] {
        let (_, html) = page(&app, path).await;
        assert!(!html.contains(r#"name="robots""#), "{path}");
        assert!(
            html.contains(&format!(
                r#"<link rel="canonical" href="https://open.example{path}">"#
            )),
            "{path}"
        );
    }
    // The manufacturer code that leads to the trailer maker is still listed:
    // it is the page for the code, not for a trailer.
    let (_, html) = page(&app, "/wmi/1A9881").await;
    assert!(html.contains(r#"<a href="/makes/ranger-trailers">Ranger Trailers</a>"#));
    assert!(!html.contains(r#"name="robots""#));
}

#[tokio::test]
async fn a_make_reached_by_alias_keeps_the_year_asked_for() {
    let app = common::app().await;
    for (path, to) in [
        ("/makes/chevy?year=2019", "/makes/chevrolet?year=2019"),
        (
            "/makes/CHEVROLET.md?year=2019",
            "/makes/chevrolet.md?year=2019",
        ),
        ("/makes/chevy?year=soon", "/makes/chevrolet"),
        ("/makes/chevy", "/makes/chevrolet"),
    ] {
        let response = app.get(path).await;
        assert_eq!(response.status(), StatusCode::PERMANENT_REDIRECT, "{path}");
        assert_eq!(header(&response, "location"), to, "{path}");
    }
}

#[tokio::test]
async fn a_year_in_an_address_is_four_digits_and_nothing_else() {
    let app = common::app().await;
    for path in [
        "/makes/honda/civic/+2019",
        "/makes/honda/civic/%2B2019",
        "/makes/honda/civic/02019",
        "/makes/honda/civic/2019.0",
        "/makes/honda/civic/+2019.md",
        "/sitemaps/models-02019.xml",
        "/sitemaps/models-+2019.xml",
    ] {
        let (status, _) = page(&app, path).await;
        assert_eq!(status, StatusCode::NOT_FOUND, "{path}");
    }
    let (status, _) = page(&app, "/makes/honda/civic/2019").await;
    assert_eq!(status, StatusCode::OK);
}

#[tokio::test]
async fn the_two_forms_of_a_page_say_the_same_things() {
    let app = common::app().await;
    // A trim and an engine from this project's own list are marked in both.
    let (_, html) = page(&app, "/makes/honda/civic/2020").await;
    assert!(
        html.contains(
            r#"<tr><th scope="row">2.0L</th><td> <span class="note">from our own list, not NHTSA's</span></td></tr>"#
        ),
        "{html}"
    );
    let text = markdown(&app, "/makes/honda/civic/2020.md").await;
    assert!(
        text.contains("- 2.0L (from our own list, not NHTSA's)\n"),
        "{text}"
    );
    let text = markdown(&app, "/makes/ford/f-150/2019.md").await;
    // Whatever else the line says of the trim, it ends with the mark.
    assert!(
        text.lines()
            .any(|line| line.starts_with("- XLT")
                && line.ends_with("(from our own list, not NHTSA's)")),
        "{text}"
    );
    assert!(
        !text
            .lines()
            .any(|line| line.starts_with("- Raptor") && line.contains("our own list")),
        "{text}"
    );
    // A list of makes for one year links on with the year, as the page does.
    let (_, text) = page(&app, "/makes.md?year=2023").await;
    assert!(
        text.contains("- [Hyundai](/makes/hyundai.md?year=2023)\n"),
        "{text}"
    );
}

#[tokio::test]
async fn a_list_that_is_cut_says_so() {
    // 61 more trims on the 2018 and 2019 Civic, which already has three.
    let mut rows = String::new();
    for index in 0..61 {
        rows.push_str(&format!(
            "INSERT INTO catalog_submodel VALUES ({}, 1, 'Edition {index:02}', 'edition{index:02}', 'trim', 1, NULL, NULL, NULL);\n",
            100 + index
        ));
    }
    let app = common::app_with_rows(&rows).await;
    let (status, html) = page(&app, "/makes/honda/civic/2019").await;
    assert_eq!(status, StatusCode::OK);
    assert!(
        html.contains("The first 60 of 64 trims are shown."),
        "{html}"
    );
    let text = markdown(&app, "/makes/honda/civic/2019.md").await;
    assert!(
        text.contains("The first 60 of 64 trims are shown.\n"),
        "{text}"
    );
    // A list that fits says nothing of the kind.
    let (_, html) = page(&app, "/makes/ford/f-150/2019").await;
    assert!(!html.contains("are shown."), "{html}");
}
