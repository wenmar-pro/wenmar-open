//! The recall pages: `/recalls`, a model year's recalls, one campaign,
//! their Markdown versions and their sitemaps.

use std::collections::HashSet;

use axum::http::StatusCode;
use serde_json::json;

use crate::common::{
    self, OGL_CANADA, TestApp, assert_basics, assert_head, assert_no_injection, assert_targets,
    header, json_ld, markdown, page,
};

const NHTSA_LOOKUP: &str = "https://www.nhtsa.gov/recalls";
const TC_LOOKUP: &str =
    "https://wwwapps.tc.gc.ca/Saf-Sec-Sur/7/VRDB-BDRV/search-recherche/menu.aspx?lang=eng";
const MAY_APPLY: &str = "A recall listed here may apply to a vehicle of this year, make and model. Only the manufacturer can say whether it applies to one vehicle: check the VIN with the official lookups.";

/// One page of every kind, and the same pages in other states.
const PAGES: [&str; 8] = [
    "/recalls",
    "/makes/hyundai/kona/2023/recalls",
    "/makes/hyundai/kona/2023/recalls?component=SEAT%20BELTS",
    "/makes/honda/civic/2019/recalls",
    "/makes/ranger-trailers/tilt-deck/2019/recalls",
    "/recalls/nhtsa/24V456000",
    "/recalls/tc/2023101",
    "/recalls/nhtsa/68V023000",
];

const MARKDOWN: [&str; 4] = [
    "/makes/hyundai/kona/2023/recalls.md",
    "/makes/honda/civic/2019/recalls.md",
    "/recalls/nhtsa/24V456000.md",
    "/recalls/tc/2023101.md",
];

fn title(html: &str) -> String {
    html.split("<title>")
        .nth(1)
        .and_then(|rest| rest.split("</title>").next())
        .unwrap()
        .to_owned()
}

fn description(html: &str) -> String {
    html.split(r#"<meta name="description" content=""#)
        .nth(1)
        .and_then(|rest| rest.split('"').next())
        .unwrap()
        .to_owned()
}

fn indexed(html: &str) -> bool {
    !html.contains(r#"<meta name="robots" content="noindex">"#)
}

async fn ok(app: &TestApp, path: &str) -> String {
    let (status, html) = page(app, path).await;
    assert_eq!(status, StatusCode::OK, "{path}");
    html
}

#[tokio::test]
async fn every_recall_page_says_may_apply_and_links_the_official_lookups() {
    let app = common::recalls_app().await;
    for path in PAGES.into_iter().chain(MARKDOWN) {
        let text = ok(&app, path).await;
        assert!(text.contains("may apply"), "{path}");
        assert!(text.contains(NHTSA_LOOKUP), "{path}: no NHTSA lookup");
        assert!(
            text.contains(TC_LOOKUP),
            "{path}: no Transport Canada lookup"
        );
        // Nothing on a page says a recall applies to this vehicle.
        let lower = text.to_lowercase();
        for claim in [
            "applies to this vin",
            "applies to your",
            "your vehicle is",
            "this vehicle is affected",
            "open recall",
        ] {
            assert!(!lower.contains(claim), "{path} says {claim:?}");
        }
    }
    for path in PAGES {
        let html = ok(&app, path).await;
        assert!(html.contains(&format!("<p>{MAY_APPLY}</p>")), "{path}");
        assert!(
            html.contains(&format!(
                r#"<li><a href="{NHTSA_LOOKUP}">NHTSA recall lookup by VIN (United States)</a></li>"#
            )),
            "{path}"
        );
        assert!(
            html.contains(&format!(
                r#"<li><a href="{TC_LOOKUP}">Transport Canada recall search (Canada)</a></li>"#
            )),
            "{path}"
        );
        assert_basics(&html, path);
        assert_head(&html, path);
        assert_targets(&html, path);
        assert_no_injection(&html);
    }
    for path in MARKDOWN {
        let text = markdown(&app, path).await;
        assert!(text.contains(MAY_APPLY), "{path}");
    }
}

#[tokio::test]
async fn the_check_recalls_page_has_the_vin_box_and_the_picker() {
    let app = common::recalls_app().await;
    let response = app.get("/recalls").await;
    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(header(&response, "cache-control"), "public, max-age=3600");
    let html = common::body_text(response).await;
    assert!(html.contains("<h1>Check a vehicle for recalls</h1>"));
    assert!(html.contains("<title>Check a vehicle for recalls - Wenmar Open</title>"));
    assert!(html.contains(r#"<link rel="canonical" href="https://open.example/recalls">"#));
    assert!(indexed(&html));
    // The home page's two forms, as they are there.
    assert_eq!(html.matches(r#"name="vin""#).count(), 1);
    assert!(html.contains(r#"<form class="vin" action="/vin" method="get">"#));
    assert!(html.contains(
        r#"<form class="pick" action="/pick" method="get" aria-labelledby="pick-heading">"#
    ));
    assert!(html.contains(r#"<h2 id="pick-heading">No VIN? Pick the vehicle</h2>"#));
    assert!(html.contains(r#"<option value="2023">2023</option>"#));
    assert!(html.contains(r#"<option value="hyundai">Hyundai</option>"#));
    assert!(!html.contains("ranger-trailers"));
    let (_, home) = page(&app, "/").await;
    let form = |html: &str| {
        let from = html.find(r#"<form class="pick""#).unwrap();
        let to = html[from..].find("</form>").unwrap();
        html[from..from + to].to_owned()
    };
    assert_eq!(form(&html), form(&home), "one picker, in one partial");
    // The newest campaigns, each leading to its page.
    assert!(html.contains("<caption>Newest recall campaigns</caption>"));
    let newest = html.find(r#"href="/recalls/nhtsa/24V456000""#).unwrap();
    let older = html.find(r#"href="/recalls/tc/2023101""#).unwrap();
    assert!(newest < older);
    assert!(
        html.contains("<strong>NHTSA advises: do not drive until the repair is done.</strong>")
    );
    // Where the data is from, and when.
    assert!(html.contains(
        "United States recalls are from NHTSA, as of 2026-10-11. Canadian recalls are from Transport Canada, as of 2026-10-06."
    ));
    assert!(html.contains(&format!(r#"<p class="note">{OGL_CANADA}</p>"#)));
}

#[tokio::test]
async fn a_model_years_recalls_are_listed_by_country_newest_first() {
    let app = common::recalls_app().await;
    let path = "/makes/hyundai/kona/2023/recalls";
    let html = ok(&app, path).await;
    assert!(html.contains("<h1>Recalls that may apply to the 2023 Hyundai Kona</h1>"));
    assert!(html.contains("<title>2023 Hyundai Kona recalls - Wenmar Open</title>"));
    assert_eq!(
        description(&html),
        "4 recall campaigns from NHTSA and Transport Canada may apply to the 2023 Hyundai Kona. Check the VIN with the manufacturer to confirm."
    );
    assert!(html.contains(&format!(
        r#"<link rel="canonical" href="https://open.example{path}">"#
    )));
    assert!(indexed(&html));
    // The way back up.
    assert!(html.contains(r#"<li><a href="/makes/hyundai">Hyundai</a></li>"#));
    assert!(html.contains(r#"<li><a href="/makes/hyundai/kona/2023">2023 Hyundai Kona</a></li>"#));
    let trail = &json_ld(&html).unwrap()["@graph"][0];
    assert_eq!(trail["@type"], "BreadcrumbList");
    assert_eq!(
        trail["itemListElement"][3],
        json!({ "@type": "ListItem", "position": 4, "name": "Recalls" })
    );

    // Each country in its own table, with the day its data was current.
    let us = html
        .find("<caption>United States: NHTSA, as of 2026-10-11</caption>")
        .unwrap();
    let ca = html
        .find("<caption>Canada: Transport Canada, as of 2026-10-06</caption>")
        .unwrap();
    assert!(us < ca);
    let order: Vec<usize> = ["24V456000", "23V123000", "22V789000"]
        .iter()
        .map(|number| {
            html.find(&format!(
                r#"<a class="mono" href="/recalls/nhtsa/{number}">{number}</a>"#
            ))
            .unwrap()
        })
        .collect();
    assert!(us < order[0] && order[0] < order[1] && order[1] < order[2] && order[2] < ca);
    assert!(
        html.find(r#"<a class="mono" href="/recalls/tc/2023101">2023101</a>"#)
            .unwrap()
            > ca
    );
    assert!(
        html.contains(
            r#"<td>2023-03-02. ELECTRICAL SYSTEM:WIRING; EXTERIOR LIGHTING:HEADLIGHTS. A wiring harness may overheat. Built 2022-09-01 to 2023-01-31. <strong>NHTSA advises: park outside, away from buildings, until the repair is done.</strong></td>"#
        ),
        "{html}"
    );
    // The advisories come before the lists, and name their campaign.
    let warning = html.find(r#"<p class="warning">Campaign <a href="/recalls/nhtsa/24V456000">24V456000</a> may apply to this vehicle. NHTSA advises: do not drive until the repair is done.</p>"#).unwrap();
    assert!(warning < us);
    assert_eq!(html.matches(r#"<p class="warning">"#).count(), 2);

    // The component filter is links, one for each system named.
    assert!(html.contains(r#"<h2 id="components">By component</h2>"#));
    for link in [
        r#"<li><a href="/makes/hyundai/kona/2023/recalls" aria-current="page">All components</a></li>"#,
        r#"<li><a href="/makes/hyundai/kona/2023/recalls?component=Electrical">Electrical</a></li>"#,
        r#"<li><a href="/makes/hyundai/kona/2023/recalls?component=ELECTRICAL%20SYSTEM">ELECTRICAL SYSTEM</a></li>"#,
        r#"<li><a href="/makes/hyundai/kona/2023/recalls?component=EXTERIOR%20LIGHTING">EXTERIOR LIGHTING</a></li>"#,
        r#"<li><a href="/makes/hyundai/kona/2023/recalls?component=SEAT%20BELTS">SEAT BELTS</a></li>"#,
        r#"<li><a href="/makes/hyundai/kona/2023/recalls?component=SERVICE%20BRAKES%2C%20HYDRAULIC">SERVICE BRAKES, HYDRAULIC</a></li>"#,
    ] {
        assert!(html.contains(link), "{link}\n{html}");
    }
    assert!(html.contains(r#"<li><a href="/makes/hyundai/kona/2023">Trims and engines for the 2023 Hyundai Kona</a></li>"#));
    assert!(html.contains(&format!(r#"<p class="note">{OGL_CANADA}</p>"#)));
    assert!(html.contains(">Report an error on this page</a></li>"));
}

#[tokio::test]
async fn the_component_filter_narrows_the_list_and_is_not_indexed() {
    let app = common::recalls_app().await;
    let path = "/makes/hyundai/kona/2023/recalls?component=SEAT%20BELTS";
    let response = app.get(path).await;
    assert_eq!(header(&response, "x-robots-tag"), "noindex");
    let html = common::body_text(response).await;
    assert!(!indexed(&html));
    // It still names the whole list as the page to index.
    assert!(html.contains(
        r#"<link rel="canonical" href="https://open.example/makes/hyundai/kona/2023/recalls">"#
    ));
    assert!(html.contains(r#"<p class="note">Only campaigns about SEAT BELTS are shown.</p>"#));
    assert!(html.contains(r#"href="/recalls/nhtsa/22V789000""#));
    for other in ["24V456000", "23V123000", "2023101"] {
        assert!(!html.contains(other), "{other}");
    }
    assert!(
        html.contains("<tr><td>None found.</td></tr>"),
        "no Canadian campaign is about seat belts"
    );
    assert!(html.contains(
        r#"<li><a href="/makes/hyundai/kona/2023/recalls?component=SEAT%20BELTS" aria-current="page">SEAT BELTS</a></li>"#
    ));
    // No advisory of a campaign that is not shown.
    assert!(!html.contains(r#"<p class="warning">"#));
    // Lower case is the same choice. A name that is not on the page narrows nothing.
    let lower = ok(
        &app,
        "/makes/hyundai/kona/2023/recalls?component=seat%20belts",
    )
    .await;
    assert!(lower.contains("Only campaigns about SEAT BELTS are shown."));
    let unknown = ok(
        &app,
        "/makes/hyundai/kona/2023/recalls?component=%3Cscript%3E",
    )
    .await;
    assert_no_injection(&unknown);
    assert!(unknown.contains("24V456000") && unknown.contains("22V789000"));
    assert!(!unknown.contains("Only campaigns about"));
}

#[tokio::test]
async fn a_model_year_with_no_recalls_says_so_and_is_not_indexed() {
    let app = common::recalls_app().await;
    let response = app.get("/makes/honda/civic/2019/recalls").await;
    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(header(&response, "x-robots-tag"), "noindex");
    let html = common::body_text(response).await;
    assert!(!indexed(&html));
    assert_eq!(html.matches("<tr><td>None found.</td></tr>").count(), 2);
    assert!(html.contains("A list with nothing on it is not a clearance either."));
    assert!(!html.contains(r#"<h2 id="components">"#));
    assert_eq!(
        description(&html),
        "No recall campaign for the 2019 Honda Civic was found in the NHTSA or Transport Canada data. Check the VIN with the manufacturer to be sure."
    );
    // A trailer's recalls have a page for whoever asks, and no more.
    let response = app
        .get("/makes/ranger-trailers/tilt-deck/2019/recalls")
        .await;
    assert_eq!(header(&response, "x-robots-tag"), "noindex");
    let html = common::body_text(response).await;
    assert!(html.contains(r#"href="/recalls/nhtsa/19V500000""#));
    assert!(!indexed(&html));
    assert!(!html.contains(r#"rel="canonical""#));
}

#[tokio::test]
async fn a_campaign_page_shows_the_campaign_and_the_vehicles_it_lists() {
    let app = common::recalls_app().await;
    let path = "/recalls/nhtsa/19V888000";
    let html = ok(&app, path).await;
    assert!(html.contains("<h1>NHTSA recall 19V888000</h1>"));
    assert!(html.contains("<title>NHTSA recall 19V888000 - Wenmar Open</title>"));
    assert_eq!(
        description(&html),
        "NHTSA recall 19V888000, about ELECTRICAL SYSTEM:12V/24V/48V BATTERY:CABLES, may apply to 2 listed vehicles. Check the VIN with the manufacturer to confirm."
    );
    assert!(html.contains(&format!(
        r#"<link rel="canonical" href="https://open.example{path}">"#
    )));
    assert!(indexed(&html));
    assert!(html.contains(r#"<li><a href="/recalls">Recalls</a></li>"#));
    for row in [
        r#"<tr><th scope="row">Published by</th><td>NHTSA (United States)</td></tr>"#,
        r#"<tr><th scope="row">Campaign number</th><td>19V888000</td></tr>"#,
        r#"<tr><th scope="row">Manufacturer's number</th><td>N192273510</td></tr>"#,
        r#"<tr><th scope="row">Manufacturer</th><td>General Motors LLC</td></tr>"#,
        r#"<tr><th scope="row">Reported</th><td>2019-12-12</td></tr>"#,
        r#"<tr><th scope="row">Owners notified</th><td>2020-03-09</td></tr>"#,
        r#"<tr><th scope="row">Component</th><td>ELECTRICAL SYSTEM:12V/24V/48V BATTERY:CABLES</td></tr>"#,
        r#"<tr><th scope="row">Units potentially affected</th><td>350371</td></tr>"#,
    ] {
        assert!(
            html.contains(&row.replace('\'', "&#39;")) || html.contains(row),
            "{row}\n{html}"
        );
    }
    assert!(html.contains("<h2>What is wrong</h2>\n<p>The battery cable nut may be loose.</p>"));
    assert!(html.contains("<h2>What can happen</h2>\n<p>A fire may start.</p>"));
    assert!(html.contains("<h2>The remedy</h2>\n<p>Dealers will tighten the nut.</p>"));
    assert!(!html.contains("<h2>Notes</h2>"));
    // A vehicle of the catalog leads to its model year; another is named.
    assert!(html.contains(
        r#"<tr><th scope="row"><a href="/makes/chevrolet/silverado/2019">2019 CHEVROLET SILVERADO 1500</a></th><td>Built 2018-07-11 to 2019-11-22. Listed as a series or trim of this model.</td></tr>"#
    ), "{html}");
    assert!(html.contains(
        r#"<tr><th scope="row">2019 GMC SIERRA 1500</th><td>Built 2018-07-11 to 2019-11-22.</td></tr>"#
    ));
    assert!(html.contains("The campaign is from data as of 2026-10-11."));
    // An article about the vehicles, with the day it was reported.
    let graph = json_ld(&html).unwrap();
    let article = graph["@graph"]
        .as_array()
        .unwrap()
        .iter()
        .find(|thing| thing["@type"] == "Article")
        .unwrap()
        .clone();
    assert_eq!(article["headline"], "NHTSA recall 19V888000");
    assert_eq!(article["datePublished"], "2019-12-12");
    assert_eq!(
        article["url"],
        "https://open.example/recalls/nhtsa/19V888000"
    );
    assert_eq!(
        article["about"],
        json!([
            { "@type": "Car", "name": "2019 CHEVROLET SILVERADO 1500" },
            { "@type": "Car", "name": "2019 GMC SIERRA 1500" }
        ])
    );

    // A Canadian campaign: its publisher, its lines of text, its credit.
    let html = ok(&app, "/recalls/tc/2023101").await;
    assert!(html.contains("<h1>Transport Canada recall 2023101</h1>"));
    assert!(html.contains(
        r#"<tr><th scope="row">Published by</th><td>Transport Canada (Canada)</td></tr>"#
    ));
    assert!(html.contains(r#"<tr><th scope="row">Notification type</th><td>Safety Mfr</td></tr>"#));
    assert!(
        html.contains(
            "<h2>What is wrong</h2>\n<p>Issue:</p>\n<p>A wiring harness may overheat.</p>"
        )
    );
    assert!(html.contains(&format!(r#"<p class="note">{OGL_CANADA}</p>"#)));
    assert!(html.contains("The campaign is from data as of 2026-10-06."));
    // An advisory is the first thing under the heading.
    let html = ok(&app, "/recalls/nhtsa/24V456000").await;
    assert!(html.contains(
        "<h1>NHTSA recall 24V456000</h1>\n<p class=\"warning\">NHTSA advises: do not drive until the repair is done.</p>"
    ), "{html}");
    let html = ok(&app, "/recalls/nhtsa/23V123000").await;
    assert!(html.contains(
        r#"<tr><th scope="row">Part numbers</th><td>91850-J9000, 91850-J9010</td></tr>"#
    ));
    assert!(html.contains("<h2>Notes</h2>\n<p>Owners may call Hyundai.</p>"));
}

#[tokio::test]
async fn a_campaign_that_reaches_no_light_vehicle_is_not_indexed() {
    let app = common::recalls_app().await;
    // A trailer, and a vehicle that matched nothing.
    for path in ["/recalls/nhtsa/19V500000", "/recalls/nhtsa/68V023000"] {
        let response = app.get(path).await;
        assert_eq!(response.status(), StatusCode::OK, "{path}");
        assert_eq!(header(&response, "x-robots-tag"), "noindex", "{path}");
        let html = common::body_text(response).await;
        assert!(!indexed(&html), "{path}");
        assert!(!html.contains(r#"rel="canonical""#), "{path}");
        assert!(json_ld(&html).is_none(), "{path}");
    }
    let html = ok(&app, "/recalls/nhtsa/68V023000").await;
    assert!(html.contains(r#"<tr><th scope="row">JEEP WAGONEER</th><td></td></tr>"#));
    assert!(html.contains(
        "<p>POSSIBILITY THE FRONT WHEEL FLEXIBLE BRAKE HOSE WAS IMPROPERLY INSTALLED.</p>"
    ));
}

#[tokio::test]
async fn html_in_the_recall_data_is_shown_as_text() {
    let app = common::recalls_app().await;
    for path in [
        "/recalls/nhtsa/19V600000",
        "/makes/b-b-script/model-img/2019/recalls",
        "/recalls",
    ] {
        let html = ok(&app, path).await;
        assert_no_injection(&html);
        assert_basics(&html, path);
        // The structured data survives the names.
        let _ = json_ld(&html);
    }
    let html = ok(&app, "/recalls/nhtsa/19V600000").await;
    assert!(
        html.contains("B&#38;B &#60;script&#62;alert(1)&#60;/script&#62;"),
        "{html}"
    );
    let about = json_ld(&html).unwrap();
    assert!(about.to_string().contains("<script>alert(1)</script>"));
    for path in [
        "/recalls/nhtsa/19V600000.md",
        "/makes/b-b-script/model-img/2019/recalls.md",
    ] {
        let text = ok(&app, path).await;
        assert!(!text.contains("<script>"), "{path}:\n{text}");
        assert!(text.contains("\\<script\\>"), "{path}:\n{text}");
    }
}

#[tokio::test]
async fn the_markdown_versions_carry_the_same_recalls() {
    let app = common::recalls_app().await;
    let text = markdown(&app, "/makes/hyundai/kona/2023/recalls.md").await;
    assert!(
        text.starts_with("# Recalls that may apply to the 2023 Hyundai Kona\n\n"),
        "{text}"
    );
    assert!(text.contains("\n## United States (NHTSA), as of 2026-10-11\n\n- [24V456000](/recalls/nhtsa/24V456000.md), 2024-05-10: SERVICE BRAKES, HYDRAULIC:ANTILOCK/TRACTION CONTROL/ELECTRONIC LIMITED SLIP. The brake control unit may fail. **NHTSA advises: do not drive until the repair is done.**\n"), "{text}");
    assert!(text.contains("\n## Canada (Transport Canada), as of 2026-10-06\n\n- [2023101](/recalls/tc/2023101.md), 2023-03-10: Electrical. Issue: A wiring harness may overheat.\n"), "{text}");
    assert!(text.contains(&format!(
        "- [NHTSA recall lookup by VIN, United States]({NHTSA_LOOKUP})\n"
    )));
    assert!(text.contains(&format!(
        "- [Transport Canada recall search, Canada]({TC_LOOKUP})\n"
    )));
    assert!(text.contains(&format!("\n{OGL_CANADA}\n")));
    assert!(text.contains(
        "\nJSON: [/v1/vehicles/2023_hyundai_kona/recalls](/v1/vehicles/2023_hyundai_kona/recalls)\n"
    ));
    // Every link to another Markdown page leads somewhere.
    for link in text.split("](/").skip(1) {
        let address = format!("/{}", link.split(')').next().unwrap());
        assert_eq!(
            app.get(&address).await.status(),
            StatusCode::OK,
            "{address}"
        );
    }
    let none = markdown(&app, "/makes/honda/civic/2019/recalls.md").await;
    assert_eq!(none.matches("None found.\n").count(), 2);

    let text = markdown(&app, "/recalls/nhtsa/19V888000.md").await;
    assert!(text.starts_with("# NHTSA recall 19V888000\n\n"), "{text}");
    assert!(text.contains("- Manufacturer's number: N192273510\n"));
    assert!(text.contains("\n## What is wrong\n\nThe battery cable nut may be loose.\n"));
    assert!(text.contains("- [2019 CHEVROLET SILVERADO 1500](/makes/chevrolet/silverado/2019/recalls.md). Built 2018\\-07\\-11 to 2019\\-11\\-22. Listed as a series or trim of this model.\n"), "{text}");
    assert!(text.contains("- 2019 GMC SIERRA 1500. Built 2018\\-07\\-11 to 2019\\-11\\-22.\n"));
    assert!(text.contains("\nJSON: [/v1/recalls/nhtsa/19V888000](/v1/recalls/nhtsa/19V888000)\n"));
    let text = markdown(&app, "/recalls/nhtsa/24V456000.md").await;
    assert!(text.contains("**NHTSA advises: do not drive until the repair is done.**\n"));
}

#[tokio::test]
async fn recall_pages_for_things_that_do_not_exist_are_404() {
    let app = common::recalls_app().await;
    for path in [
        "/makes/honda/civic/2021/recalls",
        "/makes/honda/nothing/2019/recalls",
        "/makes/nobody/civic/2019/recalls",
        "/makes/honda/CIVIC/2019/recalls",
        "/makes/honda/civic/2019/bulletins",
        "/makes/honda/civic/2019/recalls/extra",
        "/makes/honda/civic/2019.md/recalls",
        "/makes/honda/civic/02019/recalls",
        "/makes/honda/civic/2021/recalls.md",
        "/makes/honda/civic/2019/Recalls",
        "/recalls/nhtsa/00V000000",
        "/recalls/nhtsa/19v888000",
        "/recalls/tc/19V888000",
        "/recalls/epa/19V888000",
        "/recalls/nhtsa",
        "/recalls/nhtsa/19V888000/extra",
        "/recalls/nhtsa/..%2F..%2Fabout",
        "/recalls/nhtsa/%3Cscript%3Ealert(1)%3C%2Fscript%3E",
        "/recalls/nhtsa/00V000000.md",
        "/recalls.md",
    ] {
        let (status, html) = page(&app, path).await;
        assert_eq!(status, StatusCode::NOT_FOUND, "{path}");
        assert!(html.contains("<h1>There is no page here</h1>"), "{path}");
        assert_no_injection(&html);
    }
}

#[tokio::test]
async fn a_server_with_no_recalls_has_no_recall_pages() {
    let app = common::app().await;
    for path in [
        "/recalls",
        "/makes/hyundai/kona/2023/recalls",
        "/makes/hyundai/kona/2023/recalls.md",
        "/recalls/nhtsa/24V456000",
        "/recalls/nhtsa/24V456000.md",
        "/sitemaps/recalls-2023.xml",
    ] {
        let (status, html) = page(&app, path).await;
        assert_eq!(status, StatusCode::NOT_FOUND, "{path}");
        assert!(html.contains("<h1>There is no page here</h1>"), "{path}");
    }
    let (_, index) = page(&app, "/sitemap.xml").await;
    assert!(!index.contains("recalls-"), "{index}");
    let (_, pages) = page(&app, "/sitemaps/pages.xml").await;
    assert!(!pages.contains("/recalls"), "{pages}");
}

#[tokio::test]
async fn a_server_with_one_countrys_recalls_says_which_it_lacks() {
    let app = common::app_with_shop(common::RECALL_DATASETS_CA, &common::recall_rows()).await;
    let html = ok(&app, "/makes/hyundai/kona/2023/recalls").await;
    assert!(html.contains("<caption>United States: NHTSA, not on file here</caption>"));
    assert!(html.contains("<caption>Canada: Transport Canada, as of 2026-10-06</caption>"));
    assert!(html.contains(r#"href="/recalls/tc/2023101""#));
    assert!(!html.contains("24V456000"));
    assert!(
        html.contains(NHTSA_LOOKUP),
        "the lookup is offered all the same"
    );
    let text = ok(&app, "/makes/hyundai/kona/2023/recalls.md").await;
    assert!(text.contains("## United States (NHTSA), not on file here\n\nThis server holds no recalls for this country.\n"), "{text}");
    let (status, _) = page(&app, "/recalls/nhtsa/24V456000").await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    let html = ok(&app, "/recalls").await;
    assert!(html.contains("United States recalls are from NHTSA, not on file here."));
}

#[tokio::test]
async fn recall_sitemaps_list_light_model_years_with_a_campaign() {
    let app = common::recalls_app().await;
    let (_, index) = page(&app, "/sitemap.xml").await;
    for year in [2023, 2022, 2019] {
        assert!(
            index.contains(&format!(
                "<loc>https://open.example/sitemaps/recalls-{year}.xml</loc>"
            )),
            "{year}:\n{index}"
        );
    }
    for year in [2018, 2020] {
        assert!(
            !index.contains(&format!("recalls-{year}")),
            "no campaign lists a {year}"
        );
    }
    // The model sitemaps are as before.
    assert!(index.contains("<loc>https://open.example/sitemaps/models-2018.xml</loc>"));
    let (_, pages) = page(&app, "/sitemaps/pages.xml").await;
    assert!(pages.contains("<loc>https://open.example/recalls</loc>"));

    let (status, xml) = page(&app, "/sitemaps/recalls-2019.xml").await;
    assert_eq!(status, StatusCode::OK);
    assert!(xml.starts_with("<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n"));
    assert!(xml.contains("<loc>https://open.example/makes/chevrolet/silverado/2019/recalls</loc>"));
    assert!(xml.contains("<lastmod>2026-10-01</lastmod>"));
    assert!(!xml.contains("tilt-deck"), "trailers are not listed");
    assert!(
        !xml.contains("civic"),
        "a model year with no campaign is not listed"
    );
    assert!(!xml.contains('&') && !xml.contains("<script"), "{xml}");
    let (_, xml) = page(&app, "/sitemaps/recalls-2023.xml").await;
    assert_eq!(xml.matches("<url>").count(), 1);
    assert!(xml.contains("<loc>https://open.example/makes/hyundai/kona/2023/recalls</loc>"));

    // Every address listed is a page that may be indexed, with a title and
    // a description of its own.
    let (mut titles, mut descriptions) = (HashSet::new(), HashSet::new());
    let mut checked = 0;
    for year in [2023, 2022, 2019] {
        let (_, xml) = page(&app, &format!("/sitemaps/recalls-{year}.xml")).await;
        for entry in xml.split("<loc>https://open.example").skip(1) {
            let path = entry.split("</loc>").next().unwrap();
            let html = ok(&app, path).await;
            assert!(indexed(&html), "{path} is listed but not indexed");
            let (title, description) = (title(&html), description(&html));
            assert!((10..=120).contains(&title.len()), "{path}: {title}");
            assert!(
                (40..=260).contains(&description.len()),
                "{path}: {description}"
            );
            assert!(
                titles.insert(title.clone()),
                "{path} repeats the title {title}"
            );
            assert!(
                descriptions.insert(description.clone()),
                "{path} repeats {description}"
            );
            checked += 1;
        }
    }
    assert!(checked >= 4, "{checked}");
    for path in [
        "/sitemaps/recalls-2020.xml",
        "/sitemaps/recalls-1066.xml",
        "/sitemaps/recalls-x.xml",
        "/sitemaps/recalls-02019.xml",
        "/sitemaps/recalls-2019",
    ] {
        let (status, _) = page(&app, path).await;
        assert_eq!(status, StatusCode::NOT_FOUND, "{path}");
    }
}

#[tokio::test]
async fn recall_pages_lead_to_each_other() {
    let app = common::recalls_app().await;
    let mut followed = 0;
    for path in PAGES {
        let html = ok(&app, path).await;
        let main = html.split(r#"<main id="main">"#).nth(1).unwrap();
        let main = main.split("</main>").next().unwrap();
        for link in main.split(r#"href=""#).skip(1) {
            let address = link.split('"').next().unwrap();
            // The official lookups and the report links are on other sites.
            if address.starts_with("https://") || address.starts_with("mailto:") {
                continue;
            }
            let status = app.get(address).await.status();
            assert_eq!(status, StatusCode::OK, "{path} links to {address}");
            followed += 1;
        }
    }
    assert!(followed >= 30, "{followed} links followed");
}
