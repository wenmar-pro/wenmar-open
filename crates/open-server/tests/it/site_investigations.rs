//! What the model-year page, the VIN result page and a recall campaign's
//! page say of NHTSA's investigations, and that they say nothing on a
//! server that holds none.

use axum::http::StatusCode;

use crate::common::{
    self, assert_basics, assert_head, assert_no_injection, assert_targets, markdown, page,
};

const NOTE: &str = "An investigation is NHTSA looking into a possible defect. It is not a finding that a defect exists, and it does not say that any one vehicle is affected.";

#[tokio::test]
async fn the_model_year_page_says_which_open_investigations_list_it() {
    let app = common::investigations_app().await;
    let path = "/makes/hyundai/kona/2023";
    let (status, html) = page(&app, path).await;
    assert_eq!(status, StatusCode::OK);
    assert_basics(&html, path);
    assert_head(&html, path);
    assert_targets(&html, path);
    assert_eq!(
        html.matches(r#"<h2 id="investigations">Under investigation by NHTSA</h2>"#)
            .count(),
        1,
        "{html}"
    );
    assert!(
        html.contains(&format!(
            "<p>2 open NHTSA investigations list this model year. {NOTE}</p>"
        )),
        "{html}"
    );
    assert!(
        html.contains(
            "<caption>Open NHTSA investigations that list the 2023 Hyundai Kona</caption>"
        )
    );
    // Newest first, each with its kind, the day opened, subject and component.
    let first = html
        .find(r#"<tr><th scope="row" class="mono">PE25010</th><td>Preliminary evaluation, opened 2025-09-15. Loss of motive power. ENGINE; FUEL SYSTEM, GASOLINE.</td></tr>"#)
        .unwrap_or_else(|| panic!("{html}"));
    let second = html
        .find(r#"<tr><th scope="row" class="mono">EA24003</th><td>Engineering analysis, opened 2024-08-01. Brake control unit failure. SERVICE BRAKES, HYDRAULIC. Led to recall 24V456000.</td></tr>"#)
        .unwrap_or_else(|| panic!("{html}"));
    assert!(first < second);
    // Neither the audit query nor the closed investigation.
    assert!(!html.contains("AQ22002") && !html.contains("PE22001"));
    assert!(html.contains(
        r#"<p class="note">From NHTSA's investigation file, as of 2026-10-12. <a href="/v1/vehicles/2023_hyundai_kona/investigations">The JSON</a> has NHTSA's summary of each.</p>"#
    ), "{html}");
    // The section is between the table about the vehicle and the trims.
    let vehicle = html.find("<caption>Vehicle</caption>").unwrap();
    let section = html.find(r#"<h2 id="investigations">"#).unwrap();
    let trims = html.find("<caption>Trims</caption>").unwrap();
    assert!(vehicle < section && section < trims);
    // The page is still the page it was.
    assert!(html.contains("<title>2023 Hyundai Kona trims and engines - Wenmar Open</title>"));
    assert!(!html.contains(r#"name="robots""#));
    // Nothing says the vehicle has a defect.
    let lower = html.to_lowercase();
    for claim in ["is defective", "has a defect", "your vehicle"] {
        assert!(!lower.contains(claim), "{claim:?}");
    }

    // The Markdown version ends with the same.
    let text = markdown(&app, "/makes/hyundai/kona/2023.md").await;
    assert!(
        text.ends_with(&format!(
            "\n## Under investigation by NHTSA\n\n2 open NHTSA investigations list this model year. {NOTE}\n\n- PE25010: Preliminary evaluation, opened 2025\\-09\\-15. Loss of motive power. ENGINE; FUEL SYSTEM, GASOLINE.\n- EA24003: Engineering analysis, opened 2024\\-08\\-01. Brake control unit failure. SERVICE BRAKES, HYDRAULIC. Led to recall 24V456000.\n\nFrom NHTSA's investigation file, as of 2026-10-12. JSON: [/v1/vehicles/2023_hyundai_kona/investigations](/v1/vehicles/2023_hyundai_kona/investigations)\n"
        )),
        "{text}"
    );

    // A model year whose only investigation is closed, one whose only
    // investigation was opened in 1998, and one with none: no section.
    for path in [
        "/makes/hyundai/kona/2022",
        "/makes/chevrolet/silverado/2019",
        "/makes/honda/civic/2019",
        "/makes/honda/civic/2019.md",
    ] {
        let (status, html) = page(&app, path).await;
        assert_eq!(status, StatusCode::OK, "{path}");
        assert!(!html.to_lowercase().contains("investigat"), "{path}");
    }
    // Texts that are HTML stay text.
    let (_, html) = page(&app, "/makes/b-b-script/model-img/2019").await;
    assert_no_injection(&html);
    assert!(html.contains(r#"<th scope="row" class="mono">DP19001</th>"#));
    assert!(html.contains("1 open NHTSA investigation lists this model year."));
}

#[tokio::test]
async fn the_vin_page_says_which_open_investigations_list_its_model_year() {
    let app = common::investigations_app().await;
    let path = "/vin/KM8K2CAB4PU001140";
    let (status, html) = page(&app, path).await;
    assert_eq!(status, StatusCode::OK);
    assert_basics(&html, path);
    assert_head(&html, path);
    assert_targets(&html, path);
    assert!(html.contains(r#"<h2 id="investigations">Under investigation by NHTSA</h2>"#));
    assert!(html.contains(&format!(
        "<p>2 open NHTSA investigations list this model year. {NOTE}</p>"
    )));
    assert!(html.contains(r#"<th scope="row" class="mono">PE25010</th>"#));
    assert!(html.contains(r#"<th scope="row" class="mono">EA24003</th>"#));
    // After the recalls, before the links that end the page.
    let recalls = html.find(r#"<h2 id="recalls">"#).unwrap();
    let section = html.find(r#"<h2 id="investigations">"#).unwrap();
    let end = html.find("Decode another VIN").unwrap();
    assert!(recalls < section && section < end);
    // Nothing of it is copied, shared or added to the warnings.
    let copied = html
        .split("data-copy-text=\"")
        .nth(1)
        .unwrap()
        .split('"')
        .next()
        .unwrap();
    assert!(!copied.to_lowercase().contains("investigat"), "{copied}");
    let head = html.split("</head>").next().unwrap();
    assert!(!head.to_lowercase().contains("investigat"));
    assert_eq!(html.matches(r#"<p class="warning">"#).count(), 2);
    assert_eq!(html.matches("utm_campaign=vin-result").count(), 1);

    // A VIN the catalog cannot place has no section.
    let (_, html) = page(&app, "/vin/1M8PDMPA9SP000001").await;
    assert!(!html.to_lowercase().contains("investigat"));
}

#[tokio::test]
async fn a_campaign_page_says_which_investigations_led_to_it() {
    let app = common::investigations_app().await;
    let path = "/recalls/nhtsa/19V888000";
    let (status, html) = page(&app, path).await;
    assert_eq!(status, StatusCode::OK);
    assert_basics(&html, path);
    assert_head(&html, path);
    assert_targets(&html, path);
    assert_eq!(
        html.matches(r#"<h2 id="investigations">Investigated as PE19004 and EA19009</h2>"#)
            .count(),
        1,
        "{html}"
    );
    assert!(
        html.contains(&format!(
            "<p>This recall followed 2 NHTSA investigations. {NOTE}</p>"
        )),
        "{html}"
    );
    assert!(html.contains("<caption>NHTSA investigations that led to this recall</caption>"));
    // Oldest first. The campaign is this page, so no line names it again.
    let first = html
        .find(r#"<tr><th scope="row" class="mono">PE19004</th><td>Preliminary evaluation, opened 2019-02-11, closed 2019-06-20. Battery cable connection. ELECTRICAL SYSTEM:12V/24V/48V BATTERY:CABLES.</td></tr>"#)
        .unwrap_or_else(|| panic!("{html}"));
    let second = html
        .find(r#"<th scope="row" class="mono">EA19009</th>"#)
        .unwrap();
    assert!(first < second);
    assert!(!html.contains("Led to recall"));
    assert!(html.contains(r#"<a href="/v1/recalls/nhtsa/19V888000/investigations">The JSON</a>"#));
    // After the vehicles, before the official lookups.
    let vehicles = html
        .find("<caption>Vehicles this campaign lists</caption>")
        .unwrap();
    let section = html.find(r#"<h2 id="investigations">"#).unwrap();
    let lookups = html.find("<h2>Check one vehicle officially</h2>").unwrap();
    assert!(vehicles < section && section < lookups);

    let text = markdown(&app, "/recalls/nhtsa/19V888000.md").await;
    assert!(
        text.contains(&format!(
            "\n## Investigated as PE19004 and EA19009\n\nThis recall followed 2 NHTSA investigations. {NOTE}\n\n- PE19004: Preliminary evaluation, opened 2019\\-02\\-11, closed 2019\\-06\\-20. Battery cable connection. ELECTRICAL SYSTEM:12V/24V/48V BATTERY:CABLES.\n- EA19009: Engineering analysis, opened 2019\\-06\\-20, closed 2019\\-12\\-20. Battery cable connection. ELECTRICAL SYSTEM:12V/24V/48V BATTERY:CABLES.\n\nFrom NHTSA's investigation file, as of 2026-10-12. JSON: [/v1/recalls/nhtsa/19V888000/investigations](/v1/recalls/nhtsa/19V888000/investigations)\n"
        )),
        "{text}"
    );
    assert!(
        text.find("## Investigated as").unwrap() < text.find("## Check a VIN").unwrap(),
        "{text}"
    );

    // One investigation, still open.
    let (_, html) = page(&app, "/recalls/nhtsa/24V456000").await;
    assert!(html.contains(r#"<h2 id="investigations">Investigated as EA24003</h2>"#));
    assert!(html.contains("<p>This recall followed 1 NHTSA investigation. "));
    assert!(html.contains(
        "<td>Engineering analysis, opened 2024-08-01. Brake control unit failure. SERVICE BRAKES, HYDRAULIC.</td>"
    ));
    // A campaign that followed none, and a Canadian one: no section.
    for path in ["/recalls/nhtsa/22V789000", "/recalls/tc/2019631"] {
        let (status, html) = page(&app, path).await;
        assert_eq!(status, StatusCode::OK, "{path}");
        assert!(!html.to_lowercase().contains("investigat"), "{path}");
    }
    // Texts that are HTML stay text.
    let (_, html) = page(&app, "/recalls/nhtsa/19V600000").await;
    assert_no_injection(&html);
    assert!(html.contains(r#"<h2 id="investigations">Investigated as DP19001</h2>"#));
}

#[tokio::test]
async fn without_investigation_data_the_pages_are_as_they_were() {
    // Recalls and no investigations: not one word of them on any page.
    let recalls = common::recalls_app().await;
    for path in [
        "/makes/hyundai/kona/2023",
        "/makes/hyundai/kona/2023.md",
        "/vin/KM8K2CAB4PU001140",
        "/recalls/nhtsa/19V888000",
        "/recalls/nhtsa/19V888000.md",
        "/recalls/nhtsa/24V456000",
    ] {
        let (status, html) = page(&recalls, path).await;
        assert_eq!(status, StatusCode::OK, "{path}");
        assert!(!html.to_lowercase().contains("investigat"), "{path}");
    }
    // Investigations and no recalls: a page of a model year that none
    // lists is, byte for byte, the page of a server with no shop file.
    let rows = format!(
        "{}{}",
        wenmar_shopdata::schema::INVESTIGATIONS,
        common::INVESTIGATION_INSERTS
    );
    let alone = common::app_with_shop(common::INVESTIGATION_DATASETS_ALONE, &rows).await;
    let none = common::app().await;
    for path in [
        "/makes/honda/civic/2019",
        "/makes/honda/civic/2019.md",
        "/makes/hyundai/kona/2022",
        "/vin/1M8PDMPA9SP000001",
    ] {
        let (status, expected) = page(&none, path).await;
        assert_eq!(status, StatusCode::OK, "{path}");
        let (status, html) = page(&alone, path).await;
        assert_eq!(status, StatusCode::OK, "{path}");
        assert_eq!(html, expected, "{path}");
    }
    // And with only investigations, a listed model year has its section
    // and no recall line.
    let (_, html) = page(&alone, "/makes/hyundai/kona/2023").await;
    assert!(html.contains(r#"<h2 id="investigations">Under investigation by NHTSA</h2>"#));
    assert!(!html.contains(r#"<th scope="row">Recalls</th>"#));
}
