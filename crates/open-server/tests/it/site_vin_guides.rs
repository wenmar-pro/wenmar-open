//! "The manufacturer's VIN guide filed with NHTSA" on a make's page and a
//! manufacturer code's page, and that a server without the list shows
//! neither page any differently.

use axum::http::StatusCode;

use crate::common::{
    self, assert_basics, assert_head, assert_no_injection, assert_targets, markdown, page,
};

const HEADING: &str = r#"<h2 id="vin-guides">The manufacturer's VIN guide filed with NHTSA</h2>"#;
const DOCUMENTS: &str = "https://vpic.nhtsa.dot.gov/mid/home/displayfile/";

/// What marks the section on a page and in its Markdown version.
const MARKS: [&str; 6] = [
    r#"id="vin-guides""#,
    "VIN guide filed with NHTSA",
    "Part 565",
    "vpic.nhtsa.dot.gov",
    "Newest filings",
    "filings are shown",
];

/// Every address on a page that leaves this site for NHTSA's vPIC.
fn document_links(html: &str) -> Vec<&str> {
    html.split("href=\"")
        .skip(1)
        .filter_map(|rest| rest.split('"').next())
        .filter(|href| href.contains("vpic.nhtsa.dot.gov"))
        .collect()
}

#[tokio::test]
async fn a_makes_page_lists_the_newest_filings_of_its_manufacturers() {
    let app = common::vin_guides_app().await;
    let (status, html) = page(&app, "/makes/hyundai").await;
    assert_eq!(status, StatusCode::OK);
    assert_basics(&html, "/makes/hyundai");
    assert_head(&html, "/makes/hyundai");
    assert_targets(&html, "/makes/hyundai");
    assert_eq!(html.matches(HEADING).count(), 1, "{html}");
    // What the documents are, said in full.
    assert!(html.contains(
        "<p>A manufacturer files letters with NHTSA under 49 CFR Part 565 that explain how its VINs are built: what each position of the VIN stands for. The letters are the manufacturers&#39; own documents. NHTSA keeps them, and each link here opens NHTSA&#39;s copy. This site keeps the list, not the letters.</p>"
    ), "{html}");
    assert!(html.contains("<caption>Newest filings</caption>"));
    // The newest ten of thirteen, newest first, each naming who filed it.
    let links = document_links(&html);
    let expected: Vec<String> = [12, 13, 11, 10, 9, 8, 7, 6, 5, 4]
        .iter()
        .map(|number| format!("{DOCUMENTS}00000000-0000-4000-8000-{number:012}"))
        .collect();
    assert_eq!(links, expected);
    assert!(html.contains(
        r#"<tr><th scope="row">2026-06-01</th><td><a href="https://vpic.nhtsa.dot.gov/mid/home/displayfile/00000000-0000-4000-8000-000000000012">HYUNDAI MOTOR CO, from model year 2027</a> <span class="note">Part 565 letter <span class="mono">ORG20012</span>, on NHTSA's site</span></td></tr>"#
    ), "{html}");
    assert!(
        html.contains(r#">HYUNDAI MOTOR MANUFACTURING ALABAMA LLC (HMMA), model year 2027</a>"#)
    );
    assert!(html.contains(r#">HYUNDAI MOTOR CO, model years 2026 to 2028</a>"#));
    assert!(html.contains(
        r#"<p class="note">The newest 10 of 13 filings are shown. The list is NHTSA's, as of 2026-10-12.</p>"#
    ), "{html}");
    // After the manufacturer codes, at the end of the page.
    let codes = html.find("<h2>Manufacturer codes</h2>").unwrap();
    assert!(codes < html.find(HEADING).unwrap());
    // The page is still the page it was.
    assert!(html.contains("<h1>Hyundai models, 2023</h1>"));
    assert!(html.contains(r#"<link rel="canonical" href="https://open.example/makes/hyundai">"#));

    // A make no filing reaches has no section.
    let (_, html) = page(&app, "/makes/honda").await;
    for mark in MARKS {
        assert!(!html.contains(mark), "{mark}");
    }
}

#[tokio::test]
async fn a_manufacturer_codes_page_lists_its_manufacturers_filings() {
    let app = common::vin_guides_app().await;
    let (status, html) = page(&app, "/wmi/KM8").await;
    assert_eq!(status, StatusCode::OK);
    assert_basics(&html, "/wmi/KM8");
    assert_head(&html, "/wmi/KM8");
    assert_targets(&html, "/wmi/KM8");
    assert_eq!(html.matches(HEADING).count(), 1, "{html}");
    // Only Hyundai Motor Co's own twelve: the code is its code.
    assert_eq!(document_links(&html).len(), 10);
    assert!(!html.contains("ALABAMA"));
    assert!(html.contains("The newest 10 of 12 filings are shown."));
    // Between the makes and the link that ends the page.
    let makes = html
        .find(r#"<a href="/makes/hyundai">Hyundai</a>"#)
        .unwrap();
    let section = html.find(HEADING).unwrap();
    let end = html
        .find(r#"<p class="more"><a href="/">Decode a VIN</a></p>"#)
        .unwrap();
    assert!(makes < section && section < end);

    // A six-character code. One of its two filings has an address that is
    // not NHTSA's copy: it is left out, and nothing of it reaches the page.
    let (_, html) = page(&app, "/wmi/1A9881").await;
    assert_no_injection(&html);
    assert_eq!(
        document_links(&html),
        [format!("{DOCUMENTS}00000000-0000-4000-8000-000000000014")]
    );
    assert!(html.contains(">RANGER TRAILER WORKS, up to model year 2020</a>"));
    assert!(!html.contains("example.com"));
    // Fewer than ten: nothing is said of how many are shown.
    assert!(html.contains(r#"<p class="note">The list is NHTSA's, as of 2026-10-12.</p>"#));

    // A code no filing reaches has no section.
    let (_, html) = page(&app, "/wmi/1A9").await;
    for mark in MARKS {
        assert!(!html.contains(mark), "{mark}");
    }
}

#[tokio::test]
async fn names_that_are_html_stay_text() {
    let app = common::vin_guides_app().await;
    let (status, html) = page(&app, "/makes/b-b-script").await;
    assert_eq!(status, StatusCode::OK);
    assert_no_injection(&html);
    assert!(html.contains(HEADING));
    assert!(
        html.contains("B&#38;B &#60;script&#62;alert(1)&#60;/script&#62;</a>"),
        "{html}"
    );
    assert!(html.contains(r#"<span class="mono">&#60;b&#62;Bold&#60;/b&#62;</span>"#));
    let text = markdown(&app, "/makes/b-b-script.md").await;
    assert!(text.contains(r"\<script\>"), "{text}");
    assert!(
        !text.contains("<script>") && !text.contains("<b>"),
        "{text}"
    );
}

#[tokio::test]
async fn the_markdown_versions_end_with_the_same_list() {
    let app = common::vin_guides_app().await;
    let text = markdown(&app, "/wmi/1A9881.md").await;
    assert!(
        text.ends_with(
            "\n## The manufacturer's VIN guide filed with NHTSA\n\nA manufacturer files letters with NHTSA under 49 CFR Part 565 that explain how its VINs are built: what each position of the VIN stands for. The letters are the manufacturers' own documents. NHTSA keeps them, and each link here opens NHTSA's copy. This site keeps the list, not the letters.\n\n- [2019-05-05, RANGER TRAILER WORKS, up to model year 2020](https://vpic.nhtsa.dot.gov/mid/home/displayfile/00000000-0000-4000-8000-000000000014). Part 565 letter ORG20014.\n\nThe list is NHTSA's, as of 2026-10-12.\n"
        ),
        "{text}"
    );
    let text = markdown(&app, "/makes/hyundai.md").await;
    assert!(text.contains("\n## The manufacturer's VIN guide filed with NHTSA\n"));
    assert_eq!(
        text.matches("](https://vpic.nhtsa.dot.gov/mid/home/displayfile/")
            .count(),
        10
    );
    assert!(text.contains(
        "- [2026-06-01, HYUNDAI MOTOR CO, from model year 2027](https://vpic.nhtsa.dot.gov/mid/home/displayfile/00000000-0000-4000-8000-000000000012). Part 565 letter ORG20012.\n"
    ), "{text}");
    assert!(
        text.ends_with(
            "\nThe newest 10 of 13 filings are shown. The list is NHTSA's, as of 2026-10-12.\n"
        ),
        "{text}"
    );
}

#[tokio::test]
async fn without_the_list_both_pages_are_as_they_were() {
    // No shop file; a shop file with no datasets; a shop file whose only
    // datasets are the recalls.
    let none = common::app().await;
    let empty = common::app_with_shop("{}", "").await;
    let other = common::recalls_app().await;
    for path in [
        "/makes/hyundai",
        "/makes/hyundai.md",
        "/makes/hyundai?year=2022",
        "/makes/ranger-trailers",
        "/wmi/KM8",
        "/wmi/KM8.md",
        "/wmi/1A9881",
        "/wmi/1A9",
    ] {
        let (status, expected) = page(&none, path).await;
        assert_eq!(status, StatusCode::OK, "{path}");
        for mark in MARKS {
            assert!(!expected.contains(mark), "{path} has {mark:?} with no list");
        }
        // The same bytes whether there is no shop file or one without the list.
        for app in [&empty, &other] {
            let (status, html) = page(app, path).await;
            assert_eq!(status, StatusCode::OK, "{path}");
            assert_eq!(html, expected, "{path}");
        }
    }
}

#[tokio::test]
async fn meta_lists_the_dataset() {
    let app = common::vin_guides_app().await;
    let (status, body) = app.json("/v1/meta").await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(
        body["datasets"]["vin_guides"],
        serde_json::json!({ "as_of": "2026-10-12", "rows": 16, "licence": "public-domain", "attribution": "" })
    );
}
