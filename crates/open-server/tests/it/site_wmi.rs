//! `/wmi/{code}`.

use axum::http::StatusCode;

use crate::common::{self, assert_basics, assert_no_injection, header, markdown, page};

#[tokio::test]
async fn a_manufacturer_code_page_says_who_it_is_and_what_it_builds() {
    let app = common::app().await;
    let (status, html) = page(&app, "/wmi/KM8").await;
    assert_eq!(status, StatusCode::OK);
    assert_basics(&html, "/wmi/KM8");
    assert!(html.contains(r#"<h1>Manufacturer code <span class="mono">KM8</span></h1>"#));
    assert!(html.contains("was built by Hyundai Motor Co."));
    assert!(html.contains(r#"<tr><th scope="row">Country</th><td>South Korea</td></tr>"#));
    assert!(
        html.contains(r#"<tr><th scope="row">Model years on file</th><td>1990 to now</td></tr>"#)
    );
    assert!(html.contains(r#"<a href="/makes/hyundai">Hyundai</a>"#));
    assert!(html.contains(r#"<link rel="canonical" href="https://open.example/wmi/KM8">"#));

    // A six-character code of a low-volume maker.
    let (_, html) = page(&app, "/wmi/1A9881").await;
    assert!(html.contains(r#"<span class="mono">881</span> in positions 12 to 14"#));
    assert!(html.contains(r#"<a href="/makes/ranger-trailers">Ranger Trailers</a>"#));
    // A code with no make and no schemas.
    let (status, html) = page(&app, "/wmi/1A9").await;
    assert_eq!(status, StatusCode::OK);
    assert!(!html.contains("Model years on file"));
    assert!(!html.contains("<h2>Makes</h2>"));
}

#[tokio::test]
async fn a_code_in_lowercase_goes_to_its_one_address() {
    let app = common::app().await;
    for (path, to) in [("/wmi/km8", "/wmi/KM8"), ("/wmi/km8.md", "/wmi/KM8.md")] {
        let response = app.get(path).await;
        assert_eq!(response.status(), StatusCode::PERMANENT_REDIRECT, "{path}");
        assert_eq!(header(&response, "location"), to, "{path}");
    }
}

#[tokio::test]
async fn a_code_that_is_unknown_or_is_not_a_code_is_404() {
    let app = common::app().await;
    for path in [
        "/wmi/ZZZ",
        "/wmi/KM",
        "/wmi/KM8K",
        "/wmi/..%2F..",
        "/wmi/KM8K2CAB4PU001140",
        "/wmi/%3Cb%3E",
        "/wmi/K%00M",
        "/wmi/ZZZ.md",
        "/wmi",
    ] {
        let (status, html) = page(&app, path).await;
        assert_eq!(status, StatusCode::NOT_FOUND, "{path}");
        assert!(html.contains("<h1>There is no page here</h1>"), "{path}");
    }
}

#[tokio::test]
async fn a_manufacturer_code_page_has_a_markdown_version() {
    let app = common::app().await;
    let text = markdown(&app, "/wmi/KM8.md").await;
    assert!(text.starts_with("# Manufacturer code KM8\n"), "{text}");
    assert!(text.contains("- Manufacturer: Hyundai Motor Co\n"));
    assert!(text.contains("- Model years on file: 1990 to now\n"));
    assert!(text.contains("- [Hyundai](/makes/hyundai.md)\n"));
}

#[tokio::test]
async fn html_in_a_manufacturers_name_is_shown_as_text() {
    let app = common::app_with_rows(
        "INSERT INTO wmi VALUES ('ZX1', '<script>alert(1)</script> & \"Co\"', '<b>Bold</b>', '<img src=x onerror=alert(1)>', 'Trailer', 0, 6);
         INSERT INTO wmi_make VALUES ('ZX1', 6000);",
    )
    .await;
    let (status, html) = page(&app, "/wmi/ZX1").await;
    assert_eq!(status, StatusCode::OK);
    assert_no_injection(&html);
    assert!(
        html.contains("&#60;script&#62;alert(1)&#60;/script&#62; &#38; &#34;Co&#34;"),
        "{html}"
    );
    let text = markdown(&app, "/wmi/ZX1.md").await;
    assert!(text.contains(r"\<script\>"), "{text}");
    let unescaped = text.replace(r"\<", "").replace(r"\>", "");
    assert!(
        !unescaped.contains('<') && !unescaped.contains('>'),
        "{text}"
    );
}
