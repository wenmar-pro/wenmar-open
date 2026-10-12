//! The bulletin routes: a model year's bulletins, a VIN's, one bulletin,
//! and the list.

use axum::http::StatusCode;
use serde_json::{Value, json};

use crate::common::{self, TestApp};

/// The NHTSA ids of a list, in order.
fn ids(body: &Value) -> Vec<i64> {
    body["bulletins"]
        .as_array()
        .unwrap_or_else(|| panic!("no bulletins in {body}"))
        .iter()
        .map(|bulletin| bulletin["nhtsa_id"].as_i64().unwrap())
        .collect()
}

async fn ok(app: &TestApp, path: &str) -> Value {
    let (status, body) = app.json(path).await;
    assert_eq!(status, StatusCode::OK, "{path}: {body}");
    body
}

const KONA_2023: [i64; 6] = [11000003, 11000002, 11000010, 11000001, 11000005, 11000004];

#[tokio::test]
async fn a_model_year_lists_its_bulletins_newest_first_with_counts() {
    let app = common::bulletins_app().await;
    let body = ok(&app, "/v1/vehicles/2023_hyundai_kona/bulletins").await;
    assert_eq!(
        body["vehicle"],
        json!({ "id": "2023_hyundai_kona", "year": 2023, "make": "Hyundai", "model": "Kona" })
    );
    assert_eq!(body["as_of"], "2026-10-11");
    assert_eq!(ids(&body), KONA_2023);
    assert_eq!(body["next_cursor"], Value::Null);
    assert_eq!(body["attribution"], json!([]));
    assert!(
        body["note"]
            .as_str()
            .unwrap()
            .contains("A bulletin is not a recall")
    );
    assert_eq!(
        body["bulletins"][1],
        json!({
            "nhtsa_id": 11000002,
            "type": "warranty_extension",
            "type_name": "Warranty extension",
            "date_added": "2025-06-02",
            "date": "2025-05-20",
            "document_id": "WX-25-003",
            "mfr_campaign": "WX003",
            "component": "POWER TRAIN:AUTOMATIC TRANSMISSION",
            "summary": "Warranty extension: the transmission control module is covered for 10 years.",
            "document": {
                "url": "https://static.nhtsa.gov/odi/tsbs/2025/MC-11000002-0001.pdf",
                "alternate_url": "https://static.nhtsa.gov/odi/tsbs/2025/MC-11000002-9999.pdf",
                "confirmed": false
            },
            "url": "https://open.example/bulletins/11000002"
        })
    );
    // No manufacturer's date: no address can be built.
    assert_eq!(body["bulletins"][4]["nhtsa_id"], 11000005);
    assert_eq!(body["bulletins"][4]["document"], Value::Null);
    // The counts are of every bulletin of the model year.
    assert_eq!(body["counts"]["count"], 6);
    assert_eq!(
        body["counts"]["by_type"],
        json!([
            { "id": "service_bulletin", "name": "Service bulletin", "count": 4 },
            { "id": "warranty_extension", "name": "Warranty extension", "count": 1 },
            { "id": "over_the_air", "name": "Over-the-air update", "count": 1 }
        ])
    );
    assert_eq!(
        body["counts"]["components"][0],
        json!({ "id": "electrical", "name": "Electrical system", "count": 2 })
    );
    assert_eq!(
        body["counts"]["codes"],
        json!([
            { "id": "P0300", "name": "P0300", "count": 1 },
            { "id": "P0420", "name": "P0420", "count": 1 }
        ])
    );
    assert_eq!(
        body["counts"]["topics"],
        json!([{ "id": "maintenance", "name": "maintenance", "count": 1 }])
    );
    // A submodel or an engine in the id is set aside.
    let other = ok(&app, "/v1/vehicles/2022_hyundai_kona/bulletins").await;
    assert_eq!(ids(&other), [11000001, 11000006]);
}

#[tokio::test]
async fn a_list_is_paged_with_a_cursor() {
    let app = common::bulletins_app().await;
    let mut seen = Vec::new();
    let mut path = "/v1/vehicles/2023_hyundai_kona/bulletins?limit=4".to_owned();
    let mut pages = 0;
    loop {
        let body = ok(&app, &path).await;
        pages += 1;
        seen.extend(ids(&body));
        // The counts do not change from page to page.
        assert_eq!(body["counts"]["count"], 6);
        match body["next_cursor"].as_str() {
            Some(cursor) => {
                path = format!("/v1/vehicles/2023_hyundai_kona/bulletins?limit=4&cursor={cursor}");
            }
            None => break,
        }
    }
    assert_eq!(pages, 2);
    assert_eq!(seen, KONA_2023);
    // The whole list pages the same way.
    let first = ok(&app, "/v1/bulletins?limit=3").await;
    assert_eq!(ids(&first), [11000007, 11000003, 11000002]);
    assert_eq!(first["next_cursor"], "2025-06-02_11000002");
    let second = ok(&app, "/v1/bulletins?limit=3&cursor=2025-06-02_11000002").await;
    assert_eq!(ids(&second), [11000010, 11000001, 11000005]);
}

#[tokio::test]
async fn a_list_is_narrowed_by_type_component_code_topic_and_text() {
    let app = common::bulletins_app().await;
    let kona = "/v1/vehicles/2023_hyundai_kona/bulletins";
    for (query, expected) in [
        ("type=warranty_extension", vec![11000002]),
        ("type=OVER_THE_AIR", vec![11000003]),
        ("component=software", vec![11000003, 11000001]),
        ("component=brakes", vec![11000004]),
        ("code=p0420", vec![11000001]),
        ("code=P0300:00", vec![11000001]),
        ("topic=maintenance", vec![11000005]),
        ("q=noise", vec![11000004]),
        ("q=WX003", vec![11000002]),
        (
            "q=&type=&component=&code=&topic=&limit=&cursor=",
            KONA_2023.to_vec(),
        ),
        ("component=software&type=service_bulletin", vec![11000001]),
        ("component=no-such-part", vec![]),
        ("code=P3999", vec![]),
    ] {
        let body = ok(&app, &format!("{kona}?{query}")).await;
        assert_eq!(ids(&body), expected, "{query}");
        // Narrowing never changes the counts.
        assert_eq!(body["counts"]["count"], 6, "{query}");
    }
    // The list of everything takes the same narrowings, and a vehicle in
    // words.
    for (query, expected) in [
        (
            "",
            vec![
                11000007, 11000003, 11000002, 11000010, 11000001, 11000005, 11000004, 11000006,
                11000009, 10100001,
            ],
        ),
        ("code=P0420", vec![11000001, 11000006]),
        ("component=engine", vec![11000007, 11000001, 11000006]),
        ("make=chevrolet", vec![11000007, 11000006]),
        ("make=Chevrolet&model=Silverado&year=2020", vec![11000007]),
        (
            "make=hyundai&model=kona&year=2023&topic=maintenance",
            vec![11000005],
        ),
        ("year=2020", vec![11000007, 10100001]),
        ("q=catalytic%20converter", vec![11000007, 11000006]),
        ("make=nonesuch", vec![]),
        ("make=hyundai&model=nonesuch", vec![]),
    ] {
        let body = ok(&app, &format!("/v1/bulletins?{query}")).await;
        assert_eq!(ids(&body), expected, "{query}");
        assert_eq!(body["as_of"], "2026-10-11");
    }
}

#[tokio::test]
async fn a_wrong_query_value_is_named() {
    let app = common::bulletins_app().await;
    for (query, field) in [
        ("type=recall", "type"),
        ("code=P042", "code"),
        ("code=engine", "code"),
        ("component=Brake%20Pad", "component"),
        ("topic=%27%20OR%201", "topic"),
        ("cursor=yesterday", "cursor"),
    ] {
        for path in [
            format!("/v1/vehicles/2023_hyundai_kona/bulletins?{query}"),
            format!("/v1/vin/{}/bulletins?{query}", common::KONA),
            format!("/v1/bulletins?{query}"),
        ] {
            let (status, body) = app.json(&path).await;
            assert_eq!(status, StatusCode::BAD_REQUEST, "{path}: {body}");
            assert_eq!(body["error"]["code"], "validation_failed", "{path}");
            assert_eq!(body["error"]["details"]["field"], field, "{path}");
        }
    }
    let (status, body) = app.json("/v1/bulletins?limit=many").await;
    assert_eq!(status, StatusCode::BAD_REQUEST, "{body}");
    assert_eq!(body["error"]["code"], "validation_failed");
    let (status, body) = app.json("/v1/bulletins?model=kona").await;
    assert_eq!(status, StatusCode::BAD_REQUEST, "{body}");
    assert_eq!(body["error"]["details"]["field"], "make");
    // A limit over the most is held to the most, not refused.
    let body = ok(&app, "/v1/bulletins?limit=5000").await;
    assert_eq!(ids(&body).len(), 10);
    let (status, body) = app.json("/v1/vehicles/nonsense/bulletins").await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    assert_eq!(body["error"]["code"], "not_found");
}

#[tokio::test]
async fn a_vin_lists_the_bulletins_of_the_model_year_it_decodes_to() {
    let app = common::bulletins_app().await;
    let body = ok(&app, &format!("/v1/vin/{}/bulletins", common::KONA)).await;
    assert_eq!(body["decode"]["vin"], common::KONA);
    assert_eq!(body["decode"]["year"], 2023);
    assert_eq!(body["vehicle"]["id"], "2023_hyundai_kona");
    assert_eq!(ids(&body), KONA_2023);
    assert_eq!(body["counts"]["count"], 6);
    assert_eq!(body["warnings"], json!([]));
    // The same as the model year's answer, in everything they share.
    let vehicle = ok(&app, "/v1/vehicles/2023_hyundai_kona/bulletins").await;
    for field in [
        "as_of",
        "counts",
        "bulletins",
        "next_cursor",
        "note",
        "attribution",
    ] {
        assert_eq!(body[field], vehicle[field], "{field}");
    }
    // Narrowed, and with another model year.
    let body = ok(
        &app,
        &format!("/v1/vin/{}/bulletins?type=warranty_extension", common::KONA),
    )
    .await;
    assert_eq!(ids(&body), [11000002]);
    let body = ok(
        &app,
        &format!("/v1/vin/{}/bulletins?year=2022", common::KONA),
    )
    .await;
    assert_eq!(body["vehicle"]["id"], "2022_hyundai_kona");
    assert_eq!(ids(&body), [11000001, 11000006]);
    // A bad VIN is the decoder's error.
    let (status, body) = app.json("/v1/vin/KM8/bulletins").await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert_eq!(body["error"]["code"], "invalid_vin");
}

#[tokio::test]
async fn one_bulletin_comes_with_its_vehicles_components_and_codes() {
    let app = common::bulletins_app().await;
    let body = ok(&app, "/v1/bulletins/11000006").await;
    assert_eq!(
        body,
        json!({
            "nhtsa_id": 11000006,
            "type": "emissions",
            "type_name": "Emissions",
            "date_added": "2023-07-07",
            "date": "2023-07-01",
            "document_id": "EM-23",
            "mfr_campaign": null,
            "replaces": null,
            "component": "ENGINE AND ENGINE COOLING:EXHAUST SYSTEM:EMISSION CONTROL:CATALYTIC CONVERTOR",
            "mfr_system": null,
            "summary": "Emissions recall: catalytic converter. DTC P0420 may set.",
            "document": {
                "url": "https://static.nhtsa.gov/odi/tsbs/2023/MC-11000006-0001.pdf",
                "alternate_url": "https://static.nhtsa.gov/odi/tsbs/2023/MC-11000006-9999.pdf",
                "confirmed": false
            },
            "url": "https://open.example/bulletins/11000006",
            "components": [
                { "id": "engine", "name": "Engine", "parent": null },
                { "id": "exhaust-emissions", "name": "Exhaust and emissions", "parent": null },
                { "id": "catalytic-converter", "name": "Catalytic converter", "parent": "exhaust-emissions" }
            ],
            "codes": ["P0420"],
            "topics": [],
            "vehicles": [
                { "make": "CHEVROLET", "model": "SILVERADO 1500", "year_from": 2019, "year_to": 2019, "match": "base", "vehicle_ids": ["2019_chevrolet_silverado"] },
                { "make": "HYUNDAI", "model": "KONA", "year_from": 2022, "year_to": 2022, "match": "exact", "vehicle_ids": ["2022_hyundai_kona"] }
            ],
            "as_of": "2026-10-11",
            "note": body["note"],
            "attribution": []
        })
    );
    // A run of years gives an id for each; an unmatched name gives none.
    let body = ok(&app, "/v1/bulletins/11000001").await;
    assert_eq!(
        body["vehicles"][0]["vehicle_ids"],
        json!(["2022_hyundai_kona", "2023_hyundai_kona"])
    );
    let body = ok(&app, "/v1/bulletins/11000007").await;
    assert_eq!(body["vehicles"][1]["make"], "GMC");
    assert_eq!(body["vehicles"][1]["match"], Value::Null);
    assert_eq!(body["vehicles"][1]["vehicle_ids"], json!([]));
    // No model year given.
    let body = ok(&app, "/v1/bulletins/11000009").await;
    assert_eq!(body["vehicles"][0]["year_from"], Value::Null);
    assert_eq!(body["vehicles"][0]["year_to"], Value::Null);
    // Received before the address rule held.
    let body = ok(&app, "/v1/bulletins/10100001").await;
    assert_eq!(body["document"], Value::Null);
    assert_eq!(body["replaces"], "95-011");
    assert_eq!(
        ok(&app, "/v1/bulletins/11000005").await["topics"],
        json!(["maintenance"])
    );
    // Text from the source is data, never markup: JSON carries it as it is.
    let body = ok(&app, "/v1/bulletins/11000010").await;
    assert_eq!(
        body["summary"],
        "A part named <script>alert(1)</script> may fail."
    );
    for missing in [
        "1",
        "99999999",
        "abc",
        "11000006x",
        "-1",
        "110000060000000000",
    ] {
        let (status, body) = app.json(&format!("/v1/bulletins/{missing}")).await;
        assert_eq!(status, StatusCode::NOT_FOUND, "{missing}");
        assert_eq!(body["error"]["code"], "not_found", "{missing}");
    }
}

#[tokio::test]
async fn a_vin_with_no_catalog_vehicle_gets_an_empty_list_and_a_warning() {
    let app = common::bulletins_app().await;
    // A year the catalog does not list the Kona in.
    let body = ok(
        &app,
        &format!("/v1/vin/{}/bulletins?year=1999", common::KONA),
    )
    .await;
    if body["vehicle"].is_null() {
        assert_eq!(ids(&body), Vec::<i64>::new());
        assert_eq!(body["counts"]["count"], 0);
        let codes: Vec<&str> = body["warnings"]
            .as_array()
            .unwrap()
            .iter()
            .map(|warning| warning["code"].as_str().unwrap())
            .collect();
        assert!(codes.contains(&"no_catalog_vehicle"), "{codes:?}");
    }
}

#[tokio::test]
async fn a_server_without_bulletins_says_the_dataset_is_unavailable() {
    // No shop file, and a shop file that holds recalls only.
    for app in [common::app().await, common::recalls_app().await] {
        for path in [
            "/v1/vehicles/2023_hyundai_kona/bulletins",
            "/v1/vehicles/nonsense/bulletins",
            "/v1/vin/KM8K2CAB4PU001140/bulletins",
            "/v1/vin/KM8/bulletins",
            "/v1/bulletins/11000006",
            "/v1/bulletins",
            "/v1/bulletins?q=noise&type=nonsense",
        ] {
            let (status, body) = app.json(path).await;
            assert_eq!(status, StatusCode::NOT_FOUND, "{path}");
            assert_eq!(
                body,
                json!({ "error": {
                    "code": "dataset_unavailable",
                    "message": "The bulletins dataset is not available.",
                    "details": { "dataset": "bulletins" }
                }}),
                "{path}"
            );
        }
    }
    // Recalls still answer on the server that has them, and bulletins do
    // not disturb them on the one that has both.
    let both = common::bulletins_app().await;
    let (status, _) = both.json("/v1/vehicles/2023_hyundai_kona/recalls").await;
    assert_eq!(status, StatusCode::OK);
    let (_, meta) = both.json("/v1/meta").await;
    assert_eq!(meta["datasets"]["bulletins"]["rows"], 10);
}
