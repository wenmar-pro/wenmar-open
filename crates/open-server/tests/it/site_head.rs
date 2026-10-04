//! What search engines read: titles, descriptions, breadcrumbs and
//! structured data.

use std::collections::HashSet;

use axum::http::StatusCode;
use serde_json::{Value, json};

use crate::common::{
    self, TestApp, assert_basics, assert_head, assert_no_injection, assert_targets, json_ld, page,
};

/// The thing of one schema.org type in a page's structured data.
fn thing(html: &str, kind: &str) -> Option<Value> {
    json_ld(html)?["@graph"]
        .as_array()?
        .iter()
        .find(|thing| thing["@type"] == kind)
        .cloned()
}

fn meta(html: &str, start: &str) -> String {
    html.split(start)
        .nth(1)
        .and_then(|rest| rest.split('"').next())
        .unwrap_or_else(|| panic!("no {start}"))
        .to_owned()
}

fn title(html: &str) -> String {
    html.split("<title>")
        .nth(1)
        .and_then(|rest| rest.split("</title>").next())
        .unwrap()
        .to_owned()
}

fn description(html: &str) -> String {
    meta(html, r#"<meta name="description" content=""#)
}

/// Every address the sitemaps list.
async fn listed(app: &TestApp) -> Vec<String> {
    let mut paths = Vec::new();
    let (_, index) = page(app, "/sitemap.xml").await;
    for sitemap in index.split("<loc>https://open.example").skip(1) {
        let (_, xml) = page(app, sitemap.split("</loc>").next().unwrap()).await;
        for entry in xml.split("<loc>https://open.example").skip(1) {
            paths.push(entry.split("</loc>").next().unwrap().to_owned());
        }
    }
    paths
}

#[tokio::test]
async fn structured_data_says_what_each_page_is() {
    let app = common::app().await;
    // The home page: the site, the one thing it does, and who runs it.
    let (_, html) = page(&app, "/").await;
    let site = thing(&html, "WebSite").unwrap();
    assert_eq!(site["name"], "Wenmar Open");
    assert_eq!(site["url"], "https://open.example/");
    assert_eq!(
        site["potentialAction"],
        json!({
            "@type": "SearchAction",
            "target": {
                "@type": "EntryPoint",
                "urlTemplate": "https://open.example/vin?vin={vin}"
            },
            "query-input": "required name=vin"
        })
    );
    let organization = thing(&html, "Organization").unwrap();
    assert_eq!(organization["name"], "Wenmar Pro");
    assert_eq!(organization["@id"], site["publisher"]["@id"]);
    assert_eq!(
        organization["logo"],
        "https://open.example/assets/apple-touch-icon.png"
    );

    // A make: the way back up, said to people and to search engines alike.
    let (_, html) = page(&app, "/makes/honda").await;
    assert!(html.contains(r#"<li><a href="/makes">Makes</a></li>"#));
    assert_eq!(
        thing(&html, "BreadcrumbList").unwrap()["itemListElement"],
        json!([
            { "@type": "ListItem", "position": 1, "name": "Makes", "item": "https://open.example/makes" },
            { "@type": "ListItem", "position": 2, "name": "Honda" }
        ])
    );
    assert!(thing(&html, "Car").is_none());

    // A model year: the vehicle, and nothing a shop listing would have.
    let (_, html) = page(&app, "/makes/honda/civic/2019").await;
    assert!(html.contains(r#"<li><a href="/makes/honda">Honda</a></li>"#));
    let trail = thing(&html, "BreadcrumbList").unwrap();
    assert_eq!(trail["itemListElement"].as_array().unwrap().len(), 3);
    assert_eq!(trail["itemListElement"][2]["name"], "2019 Honda Civic");
    assert_eq!(
        thing(&html, "Car").unwrap(),
        json!({
            "@type": "Car",
            "name": "2019 Honda Civic",
            "url": "https://open.example/makes/honda/civic/2019",
            "vehicleModelDate": "2019",
            "brand": { "@type": "Brand", "name": "Honda" },
            "model": "Civic",
            "driveWheelConfiguration": "FWD",
            "vehicleEngine": [
                { "@type": "EngineSpecification", "name": "1.5L Turbo" },
                { "@type": "EngineSpecification", "name": "2.0L" }
            ]
        })
    );
    assert!(!html.contains("offers") && !html.contains("Product"));

    // A manufacturer code and a guide.
    let (_, html) = page(&app, "/wmi/KM8").await;
    let trail = thing(&html, "BreadcrumbList").unwrap();
    assert_eq!(
        trail["itemListElement"][0]["item"],
        "https://open.example/guides"
    );
    assert_eq!(trail["itemListElement"][2]["name"], "Manufacturer code KM8");
    let (_, html) = page(&app, "/guides/check-digit").await;
    let article = thing(&html, "Article").unwrap();
    assert_eq!(article["headline"], "What the VIN check digit is");
    assert_eq!(article["url"], "https://open.example/guides/check-digit");
    assert!(
        article.get("datePublished").is_none(),
        "no date to go stale"
    );
    assert!(thing(&html, "BreadcrumbList").is_some());
    let (_, html) = page(&app, "/about").await;
    assert!(thing(&html, "Organization").is_some());

    // No markup that search engines no longer use or that does not fit.
    for path in ["/", "/guides", "/guides/how-to-read-a-vin", "/docs"] {
        let (_, html) = page(&app, path).await;
        for kind in ["FAQPage", "HowTo", "QAPage", "Product"] {
            assert!(!html.contains(kind), "{path}: {kind}");
        }
    }
    // A list, a page of prose about the API, and anything not indexed carry
    // none at all.
    for path in [
        "/makes",
        "/docs",
        "/data",
        "/guides",
        "/makes/honda?year=2019",
        "/makes/ranger-trailers",
        "/makes/ranger-trailers/tilt-deck/2019",
        "/vin/KM8K2CAB4PU001140",
        "/vin/KM8K2",
        "/nothing",
    ] {
        let (_, html) = page(&app, path).await;
        assert!(json_ld(&html).is_none(), "{path}");
        assert!(!html.contains("application/ld+json"), "{path}");
    }
}

#[tokio::test]
async fn hostile_names_cannot_leave_the_structured_data() {
    let make = r#"Evil </script><script>alert(2)</script> "q" \ & <!--"#;
    let app = common::app_with_rows(
        r#"INSERT INTO catalog_make VALUES (8200, 'evil', 'Evil </script><script>alert(2)</script> "q" \ & <!--', 'evil', NULL, 4, 1);
           INSERT INTO catalog_model VALUES (9950, 8200, 'roadster', 'Roadster </SCRIPT> ''single''', 'roadster', 2019, 2019, 4, 1);
           INSERT INTO catalog_detail VALUES (8, 'Coupe </script>', NULL, NULL);
           INSERT INTO catalog_vehicle VALUES (40, 2019, 8200, 9950, 4, 1, 8);
           INSERT INTO catalog_engine VALUES (20, 8, '2.0L "</script>"', NULL, 'vpic');"#,
    )
    .await;
    for path in ["/makes/evil", "/makes/evil/roadster/2019"] {
        let (status, html) = page(&app, path).await;
        assert_eq!(status, StatusCode::OK, "{path}");
        // Two script elements begin and two end: the structured data and
        // the site's own script. Nothing from the data opened or closed one.
        assert_eq!(html.matches("<script").count(), 2, "{path}:\n{html}");
        assert_eq!(html.matches("</script>").count(), 2, "{path}:\n{html}");
        assert!(!html.to_lowercase().contains("</script><script>alert"));
        assert_no_injection(&html);
        assert_basics(&html, path);
        // The name is there, whole, once the JSON is read.
        let data = json_ld(&html).unwrap();
        let names = data.to_string();
        assert!(names.contains("alert(2)"), "{path}: {names}");
    }
    let (_, html) = page(&app, "/makes/evil").await;
    assert_eq!(
        thing(&html, "BreadcrumbList").unwrap()["itemListElement"][1]["name"],
        make
    );
    let (_, html) = page(&app, "/makes/evil/roadster/2019").await;
    let car = thing(&html, "Car").unwrap();
    assert_eq!(car["brand"]["name"], make);
    assert_eq!(car["model"], "Roadster </SCRIPT> 'single'");
    assert_eq!(car["bodyType"], "Coupe </script>");
    assert_eq!(car["vehicleEngine"][0]["name"], "2.0L \"</script>\"");
    // The make the fixture has always held.
    let (_, html) = page(&app, "/makes/b-b-script/model-img/2019").await;
    assert_eq!(
        thing(&html, "Car").unwrap()["brand"]["name"],
        "B&B <script>alert(1)</script>"
    );
    assert_eq!(html.matches("<script").count(), 2);
    assert_no_injection(&html);
}

#[tokio::test]
async fn titles_and_descriptions_are_for_what_people_search_and_are_unique() {
    let app = common::app().await;
    let paths = listed(&app).await;
    assert!(paths.len() >= 20, "{paths:?}");
    let (mut titles, mut descriptions) = (HashSet::new(), HashSet::new());
    for path in &paths {
        let (status, html) = page(&app, path).await;
        assert_eq!(status, StatusCode::OK, "{path}");
        let (title, description) = (title(&html), description(&html));
        // Measured as written in the page, where `&` is five characters:
        // generous limits, the exact ones are in the unit tests.
        assert!((10..=120).contains(&title.len()), "{path}: {title}");
        assert!(
            (40..=260).contains(&description.len()),
            "{path}: {description}"
        );
        assert_ne!(title, description, "{path}");
        assert!(
            titles.insert(title.clone()),
            "{path} repeats the title {title}"
        );
        assert!(
            descriptions.insert(description.clone()),
            "{path} repeats the description {description}"
        );
        assert_head(&html, path);
    }
    for (path, expected) in [
        ("/", "Free VIN decoder and shop calculators - Wenmar Open"),
        (
            "/tools/parts-matrix",
            "Parts markup matrix calculator - Wenmar Open",
        ),
        (
            "/tools/gross-profit",
            "Gross profit calculator for auto repair shops - Wenmar Open",
        ),
        (
            "/tools/labor-rate",
            "Labor rate calculator for auto repair shops - Wenmar Open",
        ),
        (
            "/tools/canada-invoice-tax",
            "Canadian invoice tax and tire fee calculator - Wenmar Open",
        ),
        (
            "/makes",
            "Car and truck makes, with models by year - Wenmar Open",
        ),
        (
            "/makes/honda",
            "Honda VIN decoder and models by year - Wenmar Open",
        ),
        (
            "/makes/honda/civic/2019",
            "2019 Honda Civic trims and engines - Wenmar Open",
        ),
        (
            "/wmi/KM8",
            "VINs starting with KM8: Hyundai Motor Co - Wenmar Open",
        ),
        (
            "/wmi/1A9881",
            "Manufacturer code 1A9881: Ranger Trailer Works - Wenmar Open",
        ),
        ("/docs", "Free VIN decoder API, no key - Wenmar Open"),
        (
            "/data",
            "Vehicle data download: NHTSA vPIC as SQLite - Wenmar Open",
        ),
        ("/about", "About Wenmar Open"),
        (
            "/guides/model-year",
            "VIN model year chart: the 10th character - Wenmar Open",
        ),
    ] {
        let (_, html) = page(&app, path).await;
        assert_eq!(title(&html), expected, "{path}");
    }
    let (_, html) = page(&app, "/makes/honda/civic/2019").await;
    assert_eq!(
        description(&html),
        "The 2019 Honda Civic has 3 trims (LX, Si, Touring) and 2 engines (1.5L Turbo, 2.0L), with the VIN character for each engine."
    );
    let (_, html) = page(&app, "/makes/honda").await;
    assert_eq!(
        description(&html),
        "Honda models for every model year from 2018 to 2020, with trims and engines. Decode Honda VINs free, with no account and no key."
    );
    // A six-character code is not something a VIN starts with.
    let (_, html) = page(&app, "/wmi/1A9881").await;
    let text = description(&html);
    assert!(
        text.starts_with("A VIN that starts with 1A9 and has 881 in positions 12 to 14 was built by Ranger Trailer Works"),
        "{text}"
    );
    assert!(!html.contains("start with 1A9881") && !html.contains("starts with 1A9881"));
    let (_, html) = page(&app, "/wmi/KM8").await;
    assert_eq!(
        description(&html),
        "A VIN that starts with KM8 was built by Hyundai Motor Co in South Korea. Makes: Hyundai. Model years on file: 1990 to now."
    );
}

#[tokio::test]
async fn a_page_with_nothing_on_file_says_so_and_claims_nothing() {
    let app = common::app().await;
    // A model year with no trims and no engines.
    let (status, html) = page(&app, "/makes/honda/cr-v/2019").await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(html.matches("None on file.").count(), 2, "{html}");
    let text = description(&html);
    assert_eq!(
        text,
        "The 2019 Honda CR-V, from NHTSA&#39;s data. No trims or engines are on file for this model year."
    );
    assert!(!text.contains(" 0 "), "{text}");
    let car = thing(&html, "Car").unwrap();
    assert!(car.get("vehicleEngine").is_none(), "{car}");
    assert!(car.get("bodyType").is_none(), "{car}");
    assert_eq!(car["name"], "2019 Honda CR-V");
    // It is the model's only year, so there are no other years to offer.
    assert!(!html.contains("Other years of"), "{html}");
    assert_basics(&html, "/makes/honda/cr-v/2019");

    // A manufacturer code with no makes and no model years.
    let (_, html) = page(&app, "/wmi/1A9").await;
    assert!(
        html.contains("No makes are on file under this code."),
        "{html}"
    );
    let text = description(&html);
    assert_eq!(
        text,
        "A VIN that starts with 1A9 was built by Many Small Makers in United States (USA)."
    );
    assert!(!html.contains("Model years on file"));

    // A year no make was built in.
    let (_, html) = page(&app, "/makes?year=1066").await;
    assert!(html.contains("No makes are on file for that year."));
    assert!(!html.contains("<h2>Popular</h2>"));
    assert!(html.contains(r#"<meta name="robots" content="noindex">"#));
}

#[tokio::test]
async fn reference_pages_lead_to_each_other_and_to_the_guides() {
    let app = common::app().await;
    // A make: its models, its years, a VIN box, its codes.
    let (_, html) = page(&app, "/makes/hyundai").await;
    // The heading has no "a" before the name: "a Audi" would be wrong, and
    // no rule picks the article for every make on file.
    assert!(html.contains("<h2>Hyundai VIN decoder</h2>"));
    assert!(!html.contains("Decode a Hyundai"));
    assert_eq!(html.matches(r#"class="primary""#).count(), 1);
    assert!(html.contains(r#"<li><a class="mono" href="/wmi/KM8">KM8</a></li>"#));
    assert!(html.contains(r#"<a href="/guides/wmi">"#));
    assert!(html.contains(r#"<a href="/makes" aria-current="page">Makes</a>"#));
    // A model year: its other years, and how to read the eighth character.
    let (_, html) = page(&app, "/makes/honda/civic/2019").await;
    assert!(html.contains(r#"<h2 id="years">Other years of the Civic</h2>"#));
    assert!(html.contains(r#"<li><a href="/makes/honda/civic/2020">2020</a></li>"#));
    assert!(
        html.contains(r#"<li><a href="/makes/honda/civic/2019" aria-current="page">2019</a></li>"#)
    );
    assert!(html.contains(r#"<li><a href="/makes/honda/civic/2018">2018</a></li>"#));
    assert!(html.contains(r#"<a href="/guides/how-to-read-a-vin">"#));
    // A manufacturer code: up to the guide, across to its makes.
    let (_, html) = page(&app, "/wmi/KM8").await;
    assert!(html.contains(r#"<li><a href="/guides/wmi">The first three characters</a></li>"#));
    assert!(html.contains(r#"<a href="/makes/hyundai">Hyundai</a>"#));
    // The Markdown versions say the same.
    let (_, text) = page(&app, "/makes/hyundai.md").await;
    assert!(
        text.contains("## Manufacturer codes\n\n[KM8](/wmi/KM8.md)\n"),
        "{text}"
    );
    let (_, text) = page(&app, "/makes/honda/civic/2019.md").await;
    assert!(
        text.contains("## Other years\n\n[2020](/makes/honda/civic/2020.md) [2018](/makes/honda/civic/2018.md)\n"),
        "{text}"
    );

    // Every link in these pages leads somewhere.
    let mut followed = 0;
    for path in [
        "/makes",
        "/makes/hyundai",
        "/makes/honda/civic/2019",
        "/wmi/KM8",
        "/wmi/1A9881",
    ] {
        let (_, html) = page(&app, path).await;
        assert_targets(&html, path);
        let main = html.split(r#"<main id="main">"#).nth(1).unwrap();
        let main = main.split("</main>").next().unwrap();
        for link in main.split(r#"href=""#).skip(1) {
            let address = link.split('"').next().unwrap();
            let status = app.get(address).await.status();
            assert_eq!(status, StatusCode::OK, "{path} links to {address}");
            followed += 1;
        }
    }
    assert!(followed >= 20, "{followed} links followed");
}

#[tokio::test]
async fn a_description_stops_at_the_end_of_a_sentence() {
    // Names as long as the ones on file: the fullest wording of each of
    // these descriptions is longer than a search result shows.
    let app = common::app_with_rows(
        "INSERT INTO wmi VALUES ('4C9337', 'Crary Industries Incorporated', 'Crary', 'United States (USA)', 'Trailer', 0, 6);
         INSERT INTO catalog_make VALUES
           (8300, 'crary-agricultural-solutions', 'Crary Agricultural Solutions', 'craryagriculturalsolutions', NULL, 64, 0),
           (8400, 'excalibur-automobile-corporation', 'Excalibur Automobile Corporation', 'excaliburautomobilecorporation', NULL, 4, 1);
         INSERT INTO wmi_make VALUES ('4C9337', 8300);
         INSERT INTO wmi_schema VALUES ('4C9337', 60, 2000, 2000);
         INSERT INTO catalog_model VALUES (9960, 8400, 'series-iv', 'Series IV', 'seriesiv', 1980, 1984, 4, 1);
         INSERT INTO catalog_detail VALUES (9, NULL, NULL, NULL);
         INSERT INTO catalog_vehicle VALUES
           (50, 1980, 8400, 9960, 4, 1, NULL),
           (51, 1984, 8400, 9960, 4, 1, 9);
         INSERT INTO catalog_submodel VALUES
           (20, 9, 'Phaeton', 'phaeton', 'trim', 1, NULL, NULL, NULL),
           (21, 9, 'Roadster', 'roadster', 'trim', 1, NULL, NULL, NULL),
           (22, 9, 'Royale', 'royale', 'trim', 1, NULL, NULL, NULL),
           (23, 9, 'Sedan', 'sedan', 'trim', 1, NULL, NULL, NULL);
         INSERT INTO catalog_engine VALUES
           (30, 9, '5.0L V8', NULL, 'vpic'),
           (31, 9, '5.7L V8', NULL, 'vpic'),
           (32, 9, '7.4L V8', NULL, 'vpic'),
           (33, 9, '8.2L V8', NULL, 'vpic');",
    )
    .await;
    for (path, expected) in [
        // The makes do not fit, so they are left out; the years do.
        (
            "/wmi/4C9337",
            "A VIN that starts with 4C9 and has 337 in positions 12 to 14 was built by Crary Industries Incorporated in United States (USA). Model years on file: 2000.",
        ),
        // The second sentence in its shorter wording.
        (
            "/makes/excalibur-automobile-corporation",
            "Excalibur Automobile Corporation models for every model year from 1980 to 1984, with trims and engines. Decode Excalibur Automobile Corporation VINs free.",
        ),
        // The one sentence without its last clause.
        (
            "/makes/excalibur-automobile-corporation/series-iv/1984",
            "The 1984 Excalibur Automobile Corporation Series IV has 4 trims (Phaeton, Roadster, Royale and more) and 4 engines (5.0L V8, 5.7L V8, 7.4L V8 and more).",
        ),
    ] {
        let (status, html) = page(&app, path).await;
        assert_eq!(status, StatusCode::OK, "{path}");
        let text = description(&html);
        assert!(text.ends_with('.'), "{path}: {text}");
        assert!(text.chars().count() <= 160, "{path}: {text}");
        assert_eq!(text, expected, "{path}");
        // A shared link says the same.
        assert_eq!(
            meta(&html, r#"<meta property="og:description" content=""#),
            expected,
            "{path}"
        );
    }
}
