//! The recall routes: a model year's recalls, a VIN's, one campaign, and
//! the list.

use axum::http::StatusCode;
use serde_json::{Value, json};

use crate::common::{self, OGL_CANADA, TestApp};

const NHTSA_LOOKUP: &str = "https://www.nhtsa.gov/recalls";
const TC_LOOKUP: &str =
    "https://wwwapps.tc.gc.ca/Saf-Sec-Sur/7/VRDB-BDRV/search-recherche/menu.aspx?lang=eng";

/// The campaign numbers of a list, in order.
fn campaigns(body: &Value) -> Vec<String> {
    body["recalls"]
        .as_array()
        .unwrap_or_else(|| panic!("no recalls in {body}"))
        .iter()
        .map(|recall| recall["campaign"].as_str().unwrap().to_owned())
        .collect()
}

async fn ok(app: &TestApp, path: &str) -> Value {
    let (status, body) = app.json(path).await;
    assert_eq!(status, StatusCode::OK, "{path}: {body}");
    body
}

#[tokio::test]
async fn a_model_year_lists_the_recalls_that_may_apply_newest_first() {
    let app = common::recalls_app().await;
    let body = ok(&app, "/v1/vehicles/2023_hyundai_kona/recalls").await;
    assert_eq!(
        body["vehicle"],
        json!({ "id": "2023_hyundai_kona", "year": 2023, "make": "Hyundai", "model": "Kona" })
    );
    assert_eq!(body["may_apply"], true);
    assert_eq!(
        body["as_of"],
        json!({ "us": "2026-10-11", "ca": "2026-10-06" })
    );
    assert_eq!(body["lookup"]["nhtsa"], NHTSA_LOOKUP);
    assert_eq!(body["lookup"]["tc"], TC_LOOKUP);
    // The maker's own lookup is an address, or null when none is on file.
    let maker = &body["lookup"]["maker"];
    assert!(
        maker.is_null()
            || maker
                .as_str()
                .is_some_and(|url| url.starts_with("https://"))
    );
    assert_eq!(body["attribution"], json!([OGL_CANADA]));
    assert_eq!(
        campaigns(&body),
        ["24V456000", "2023101", "23V123000", "22V789000"]
    );
    assert_eq!(
        body["recalls"][2],
        json!({
            "source": "nhtsa",
            "country": "us",
            "campaign": "23V123000",
            "date": "2023-03-02",
            "manufacturer": "Hyundai Motor America",
            "component": "ELECTRICAL SYSTEM:WIRING; EXTERIOR LIGHTING:HEADLIGHTS",
            "summary": "A wiring harness may overheat.",
            "mfr_campaign": "240",
            "units": 5000,
            "do_not_drive": false,
            "park_outside": true,
            "notification": null,
            "match": "exact",
            "models": ["KONA"],
            "built_from": "2022-09-01",
            "built_to": "2023-01-31",
            "url": "https://open.example/recalls/nhtsa/23V123000"
        })
    );
    // The campaign that names a trim of the model says so.
    assert_eq!(body["recalls"][0]["match"], "base");
    assert_eq!(body["recalls"][0]["models"], json!(["KONA ELECTRIC"]));
    assert_eq!(body["recalls"][0]["do_not_drive"], true);
    // A Canadian campaign has no advisories.
    assert_eq!(body["recalls"][1]["source"], "tc");
    assert_eq!(body["recalls"][1]["country"], "ca");
    assert_eq!(body["recalls"][1]["do_not_drive"], Value::Null);
    assert_eq!(body["recalls"][1]["notification"], "Safety Mfr");

    // A submodel or an engine in the id is set aside.
    let trimmed = ok(&app, "/v1/vehicles/2023_hyundai_kona_se/recalls").await;
    assert_eq!(trimmed, body);
    // A model year with none says so with an empty list, and still says
    // where to check a VIN.
    let none = ok(&app, "/v1/vehicles/2019_honda_civic/recalls").await;
    assert_eq!(none["recalls"], json!([]));
    assert_eq!(none["may_apply"], true);
    assert_eq!(none["lookup"]["nhtsa"], NHTSA_LOOKUP);
    assert_eq!(none["as_of"]["us"], "2026-10-11");
}

#[tokio::test]
async fn country_narrows_the_list_and_the_credit() {
    let app = common::recalls_app().await;
    let us = ok(&app, "/v1/vehicles/2023_hyundai_kona/recalls?country=us").await;
    assert_eq!(campaigns(&us), ["24V456000", "23V123000", "22V789000"]);
    assert_eq!(us["attribution"], json!([]));
    // The dates of both countries are still given.
    assert_eq!(us["as_of"]["ca"], "2026-10-06");
    for path in [
        "/v1/vehicles/2023_hyundai_kona/recalls?country=ca",
        "/v1/vehicles/2023_hyundai_kona/recalls?country=CA",
    ] {
        let ca = ok(&app, path).await;
        assert_eq!(campaigns(&ca), ["2023101"], "{path}");
        assert_eq!(ca["attribution"], json!([OGL_CANADA]), "{path}");
    }
    // An empty form field is the same as leaving it out.
    let both = ok(&app, "/v1/vehicles/2023_hyundai_kona/recalls?country=").await;
    assert_eq!(campaigns(&both).len(), 4);

    for path in [
        "/v1/vehicles/2023_hyundai_kona/recalls?country=mx",
        "/v1/vin/KM8K2CAB4PU001140/recalls?country=canada",
        "/v1/recalls?country=us,ca",
    ] {
        let (status, body) = app.json(path).await;
        assert_eq!(status, StatusCode::BAD_REQUEST, "{path}");
        assert_eq!(
            body,
            json!({ "error": {
                "code": "validation_failed",
                "message": "country must be us or ca",
                "details": { "field": "country" }
            } }),
            "{path}"
        );
    }
}

#[tokio::test]
async fn a_vehicle_that_does_not_exist_is_404() {
    let app = common::recalls_app().await;
    for path in [
        "/v1/vehicles/2023_hyundai_nothing/recalls",
        "/v1/vehicles/2021_hyundai_kona/recalls",
        "/v1/vehicles/2023_hyundai_kona_nothing/recalls",
        "/v1/vehicles/nonsense/recalls",
    ] {
        let (status, body) = app.json(path).await;
        assert_eq!(status, StatusCode::NOT_FOUND, "{path}");
        assert_eq!(body["error"]["code"], "not_found", "{path}");
        assert_eq!(
            body["error"]["message"], "No vehicle has that id.",
            "{path}"
        );
    }
}

#[tokio::test]
async fn a_server_with_one_countrys_recalls_answers_for_that_country() {
    let app = common::app_with_shop(common::RECALL_DATASETS_CA, &common::recall_rows()).await;
    // Asked for both, it gives what it holds and says which it lacks.
    let body = ok(&app, "/v1/vehicles/2023_hyundai_kona/recalls").await;
    assert_eq!(campaigns(&body), ["2023101"]);
    assert_eq!(body["as_of"], json!({ "us": null, "ca": "2026-10-06" }));
    assert_eq!(body["attribution"], json!([OGL_CANADA]));
    let listed = ok(&app, "/v1/recalls").await;
    assert_eq!(campaigns(&listed), ["2023101", "2019631"]);
    ok(&app, "/v1/vehicles/2023_hyundai_kona/recalls?country=ca").await;
    ok(&app, "/v1/recalls/tc/2023101").await;
    // Asked for the other by name, it names the dataset that is missing.
    for path in [
        "/v1/vehicles/2023_hyundai_kona/recalls?country=us",
        "/v1/vin/KM8K2CAB4PU001140/recalls?country=us",
        "/v1/recalls?country=us",
        "/v1/recalls/nhtsa/24V456000",
    ] {
        let (status, body) = app.json(path).await;
        assert_eq!(status, StatusCode::NOT_FOUND, "{path}");
        assert_eq!(
            body,
            json!({ "error": {
                "code": "dataset_unavailable",
                "message": "The recalls_us dataset is not available.",
                "details": { "dataset": "recalls_us" }
            } }),
            "{path}"
        );
    }

    let app = common::app_with_shop(common::RECALL_DATASETS_US, &common::recall_rows()).await;
    let body = ok(&app, "/v1/vehicles/2023_hyundai_kona/recalls").await;
    assert_eq!(campaigns(&body), ["24V456000", "23V123000", "22V789000"]);
    assert_eq!(body["as_of"], json!({ "us": "2026-10-11", "ca": null }));
    assert_eq!(body["attribution"], json!([]));
    let (status, body) = app.json("/v1/recalls/tc/2023101").await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    assert_eq!(body["error"]["details"]["dataset"], "recalls_ca");
}

#[tokio::test]
async fn a_server_with_no_shop_file_answers_every_recall_route_404() {
    let app = common::app().await;
    for path in [
        "/v1/vehicles/2023_hyundai_kona/recalls",
        "/v1/vehicles/nonsense/recalls",
        "/v1/vin/KM8K2CAB4PU001140/recalls",
        "/v1/vin/KM8/recalls",
        "/v1/recalls/nhtsa/24V456000",
        "/v1/recalls",
        "/v1/recalls?q=harness",
    ] {
        let (status, body) = app.json(path).await;
        assert_eq!(status, StatusCode::NOT_FOUND, "{path}");
        assert_eq!(body["error"]["code"], "dataset_unavailable", "{path}");
        assert_eq!(
            body["error"]["message"], "The recalls_us dataset is not available.",
            "{path}"
        );
    }
    let (_, body) = app.json("/v1/recalls/tc/2023101").await;
    assert_eq!(body["error"]["details"]["dataset"], "recalls_ca");
}

#[tokio::test]
async fn a_vin_lists_the_recalls_of_the_model_year_it_decodes_to() {
    let app = common::recalls_app().await;
    let body = ok(&app, "/v1/vin/KM8K2CAB4PU001140/recalls").await;
    assert_eq!(
        body["decode"],
        json!({
            "vin": "KM8K2CAB4PU001140",
            "valid": true,
            "year": 2023,
            "make": "Hyundai",
            "model": "Kona",
            "trim": "SE"
        })
    );
    assert_eq!(body["warnings"], json!([]));
    // The rest is the model year's answer, word for word.
    let vehicle = ok(&app, "/v1/vehicles/2023_hyundai_kona/recalls").await;
    for field in [
        "vehicle",
        "may_apply",
        "as_of",
        "recalls",
        "lookup",
        "attribution",
    ] {
        assert_eq!(body[field], vehicle[field], "{field}");
    }
    assert_eq!(body["may_apply"], true);
    // Untidy typing and a country are accepted as elsewhere.
    let tidy = ok(&app, "/v1/vin/km8-k2cab4pu001140/recalls?country=ca").await;
    assert_eq!(tidy["decode"]["vin"], "KM8K2CAB4PU001140");
    assert_eq!(campaigns(&tidy), ["2023101"]);
    // A wrong check digit is a warning, and the recalls are still listed.
    let wrong = ok(&app, "/v1/vin/KM8K2CAB0PU001140/recalls").await;
    assert_eq!(wrong["decode"]["valid"], false);
    assert_eq!(wrong["warnings"][0]["code"], "invalid_check_digit");
    assert_eq!(campaigns(&wrong).len(), 4);
}

#[tokio::test]
async fn a_vin_that_reaches_no_catalog_vehicle_gets_an_empty_list_and_a_warning() {
    let app = common::recalls_app().await;
    // The bus decodes, and the catalog has no entry for it.
    let (status, decode) = app.json("/v1/vin/1M8PDMPA9SP000001").await;
    assert_eq!(status, StatusCode::OK);
    assert!(
        decode.get("catalog").is_none(),
        "the fixture changed: {decode}"
    );

    let body = ok(&app, "/v1/vin/1M8PDMPA9SP000001/recalls").await;
    assert_eq!(body["decode"]["vin"], "1M8PDMPA9SP000001");
    assert_eq!(body["vehicle"], Value::Null);
    assert_eq!(body["recalls"], json!([]));
    assert_eq!(body["may_apply"], true);
    assert_eq!(body["lookup"]["nhtsa"], NHTSA_LOOKUP);
    assert_eq!(body["lookup"]["tc"], TC_LOOKUP);
    assert_eq!(body["as_of"]["us"], "2026-10-11");
    let warnings = body["warnings"].as_array().unwrap();
    let last = warnings.last().unwrap();
    assert_eq!(last["code"], "no_catalog_vehicle");
    assert_eq!(
        last["message"],
        "This VIN does not decode to a year, make and model in the catalog, so no recalls are listed for it. Check the VIN with the official lookups."
    );

    // What is not a VIN is still an error, the same one a decode gives.
    for (path, status, code) in [
        (
            "/v1/vin/KM8/recalls",
            StatusCode::BAD_REQUEST,
            "invalid_vin",
        ),
        (
            "/v1/vin/ZZZK2CAB4PU001140/recalls",
            StatusCode::NOT_FOUND,
            "not_found",
        ),
        (
            "/v1/vin/KM8K2CAB4PU001140/recalls?year=1900",
            StatusCode::BAD_REQUEST,
            "validation_failed",
        ),
    ] {
        let (found, body) = app.json(path).await;
        assert_eq!(found, status, "{path}");
        assert_eq!(body["error"]["code"], code, "{path}");
        let (decode_status, decode) = app.json(&path.replace("/recalls", "")).await;
        assert_eq!(decode_status, status, "{path}");
        assert_eq!(decode, body, "{path}");
    }
}

#[tokio::test]
async fn one_campaign_comes_with_every_vehicle_it_lists() {
    let app = common::recalls_app().await;
    // Letters in the number may be in either case.
    let body = ok(&app, "/v1/recalls/nhtsa/19v888000").await;
    assert_eq!(
        body,
        json!({
            "source": "nhtsa",
            "country": "us",
            "campaign": "19V888000",
            "date": "2019-12-12",
            "owner_notified": "2020-03-09",
            "manufacturer": "General Motors LLC",
            "component": "ELECTRICAL SYSTEM:12V/24V/48V BATTERY:CABLES",
            "summary": "The battery cable nut may be loose.",
            "consequence": "A fire may start.",
            "remedy": "Dealers will tighten the nut.",
            "notes": null,
            "mfr_campaign": "N192273510",
            "part_number": null,
            "units": 350371,
            "completion": null,
            "do_not_drive": false,
            "park_outside": false,
            "notification": null,
            "french": null,
            "vehicles": [
                {
                    "year": 2019,
                    "make": "CHEVROLET",
                    "model": "SILVERADO 1500",
                    "vehicle_id": "2019_chevrolet_silverado",
                    "match": "base",
                    "built_from": "2018-07-11",
                    "built_to": "2019-11-22"
                },
                {
                    "year": 2019,
                    "make": "GMC",
                    "model": "SIERRA 1500",
                    "vehicle_id": null,
                    "match": null,
                    "built_from": "2018-07-11",
                    "built_to": "2019-11-22"
                }
            ],
            "may_apply": true,
            "as_of": "2026-10-11",
            "lookup": { "nhtsa": NHTSA_LOOKUP, "tc": TC_LOOKUP, "maker": null },
            "attribution": [],
            "url": "https://open.example/recalls/nhtsa/19V888000"
        })
    );
    // A Canadian campaign carries its French text and its credit.
    let canadian = ok(&app, "/v1/recalls/tc/2019631").await;
    assert_eq!(canadian["country"], "ca");
    assert_eq!(canadian["as_of"], "2026-10-06");
    assert_eq!(
        canadian["french"],
        json!({
            "component": "Circuit électrique",
            "notification": "Sécurité - fabricant",
            "summary": "Problème :\nUn écrou peut être desserré."
        })
    );
    assert_eq!(canadian["summary"], "Issue:\nThe nut may be loose.");
    assert_eq!(canadian["attribution"], json!([OGL_CANADA]));
    assert_eq!(
        canadian["vehicles"][0]["vehicle_id"],
        "2019_chevrolet_silverado"
    );
    // A vehicle with no year, matched to nothing, is still listed.
    let old = ok(&app, "/v1/recalls/nhtsa/68V023000").await;
    assert_eq!(
        old["vehicles"],
        json!([{
            "year": null,
            "make": "JEEP",
            "model": "WAGONEER",
            "vehicle_id": null,
            "match": null,
            "built_from": null,
            "built_to": null
        }])
    );
    assert!(
        old["summary"]
            .as_str()
            .unwrap()
            .starts_with("POSSIBILITY THE FRONT WHEEL")
    );
    // Both part numbers and both advisories.
    let advisory = ok(&app, "/v1/recalls/nhtsa/23V123000").await;
    assert_eq!(advisory["part_number"], "91850-J9000, 91850-J9010");
    assert_eq!(advisory["park_outside"], true);
    assert_eq!(advisory["notes"], "Owners may call Hyundai.");

    for path in [
        "/v1/recalls/tc/19V888000",
        "/v1/recalls/nhtsa/2019631",
        "/v1/recalls/nhtsa/00V000000",
        "/v1/recalls/epa/19V888000",
        "/v1/recalls/NHTSA/19V888000",
        "/v1/recalls/nhtsa/19V888000%27%20OR%201=1",
        "/v1/recalls/nhtsa/..%2F..%2Fmeta",
        "/v1/recalls/nhtsa/19V888000000000000000000000",
    ] {
        let (status, body) = app.json(path).await;
        assert_eq!(status, StatusCode::NOT_FOUND, "{path}");
        assert_eq!(body["error"]["code"], "not_found", "{path}");
    }
}

#[tokio::test]
async fn the_list_pages_newest_first() {
    let app = common::recalls_app().await;
    let everything = [
        "24V456000",
        "2023101",
        "23V123000",
        "22V789000",
        "2019631",
        "19V888000",
        "19V600000",
        "19V500000",
        "68V023000",
    ];
    let body = ok(&app, "/v1/recalls").await;
    assert_eq!(campaigns(&body), everything);
    assert_eq!(body["next_cursor"], Value::Null);
    assert_eq!(body["may_apply"], true);
    assert_eq!(
        body["as_of"],
        json!({ "us": "2026-10-11", "ca": "2026-10-06" })
    );
    assert_eq!(
        body["lookup"],
        json!({ "nhtsa": NHTSA_LOOKUP, "tc": TC_LOOKUP, "maker": null })
    );
    assert_eq!(body["attribution"], json!([OGL_CANADA]));
    // A campaign in a list is not about one model year.
    assert_eq!(body["recalls"][0]["match"], Value::Null);
    assert_eq!(body["recalls"][0]["models"], json!([]));
    assert_eq!(
        body["recalls"][0]["url"],
        "https://open.example/recalls/nhtsa/24V456000"
    );

    let mut seen: Vec<String> = Vec::new();
    let mut path = "/v1/recalls?limit=4".to_owned();
    let mut pages = 0;
    loop {
        let page = ok(&app, &path).await;
        pages += 1;
        seen.extend(campaigns(&page));
        match page["next_cursor"].as_str() {
            Some(cursor) => path = format!("/v1/recalls?limit=4&cursor={cursor}"),
            None => break,
        }
    }
    assert_eq!(seen, everything);
    assert_eq!(pages, 3);
    // The limit is at least 1 and at most 100.
    assert_eq!(campaigns(&ok(&app, "/v1/recalls?limit=0").await).len(), 1);
    assert_eq!(
        campaigns(&ok(&app, "/v1/recalls?limit=5000").await).len(),
        9
    );
    for path in [
        "/v1/recalls?cursor=zzz",
        "/v1/recalls?cursor=2019-12-12",
        "/v1/recalls?limit=many",
        "/v1/recalls?year=soon",
        "/v1/recalls?model=kona",
    ] {
        let (status, body) = app.json(path).await;
        assert_eq!(status, StatusCode::BAD_REQUEST, "{path}");
        assert_eq!(body["error"]["code"], "validation_failed", "{path}");
    }
    let (_, body) = app.json("/v1/recalls?cursor=zzz").await;
    assert_eq!(
        body["error"]["message"],
        "cursor is not one this API gave out"
    );
    assert_eq!(body["error"]["details"], json!({ "field": "cursor" }));
    let (_, body) = app.json("/v1/recalls?model=kona").await;
    assert_eq!(body["error"]["message"], "model needs make");
}

#[tokio::test]
async fn the_list_is_narrowed_by_make_model_year_component_text_and_country() {
    let app = common::recalls_app().await;
    let found = |path: &'static str| {
        let app = &app;
        async move { campaigns(&ok(app, path).await) }
    };
    // A make by its id, its name or an alias.
    assert_eq!(
        found("/v1/recalls?make=hyundai").await,
        ["24V456000", "2023101", "23V123000", "22V789000"]
    );
    assert_eq!(
        found("/v1/recalls?make=chevy").await,
        ["2019631", "19V888000"]
    );
    assert_eq!(
        found("/v1/recalls?make=Chevrolet&country=us").await,
        ["19V888000"]
    );
    // A model by its id or its name, and a year.
    assert_eq!(
        found("/v1/recalls?make=hyundai&model=kona&year=2022").await,
        ["22V789000"]
    );
    assert_eq!(
        found("/v1/recalls?make=hyundai&model=KONA&year=2023")
            .await
            .len(),
        4
    );
    assert_eq!(found("/v1/recalls?year=2019&country=ca").await, ["2019631"]);
    // A make or a model the catalog does not have lists nothing.
    assert!(found("/v1/recalls?make=nobody").await.is_empty());
    assert!(
        found("/v1/recalls?make=hyundai&model=nothing")
            .await
            .is_empty()
    );
    // A component, in either case.
    assert_eq!(
        found("/v1/recalls?component=brakes").await,
        ["24V456000", "68V023000"]
    );
    assert_eq!(
        found("/v1/recalls?component=Electrical").await,
        ["2023101", "23V123000", "2019631", "19V888000"]
    );
    // Free text: the summary, the manufacturer's own number, a vehicle.
    assert_eq!(
        found("/v1/recalls?q=harness").await,
        ["2023101", "23V123000"]
    );
    assert_eq!(
        found("/v1/recalls?q=n192273510").await,
        ["2019631", "19V888000"]
    );
    assert_eq!(found("/v1/recalls?q=wagoneer").await, ["68V023000"]);
    assert_eq!(found("/v1/recalls?q=sierra").await, ["19V888000"]);
    assert_eq!(
        found("/v1/recalls?q=harness&country=us&make=hyundai&year=2023").await,
        ["23V123000"]
    );
    assert!(found("/v1/recalls?q=%25").await.is_empty());
    // Blank fields narrow nothing.
    assert_eq!(
        found("/v1/recalls?make=&model=&year=&component=&q=&country=&limit=&cursor=")
            .await
            .len(),
        9
    );
}

#[tokio::test]
async fn every_recall_answer_says_may_apply_and_gives_the_official_lookups() {
    let app = common::recalls_app().await;
    for path in [
        "/v1/vehicles/2023_hyundai_kona/recalls",
        "/v1/vehicles/2019_honda_civic/recalls",
        "/v1/vehicles/2023_hyundai_kona/recalls?country=ca",
        "/v1/vin/KM8K2CAB4PU001140/recalls",
        "/v1/vin/1M8PDMPA9SP000001/recalls",
        "/v1/recalls/nhtsa/24V456000",
        "/v1/recalls/tc/2023101",
        "/v1/recalls",
        "/v1/recalls?make=nobody",
        "/v1/recalls?q=harness",
    ] {
        let response = app.get(path).await;
        assert_eq!(response.status(), StatusCode::OK, "{path}");
        let text = common::body_text(response).await;
        let body: Value = serde_json::from_str(&text).unwrap();
        assert_eq!(body["may_apply"], true, "{path}");
        assert_eq!(body["lookup"]["nhtsa"], NHTSA_LOOKUP, "{path}");
        assert_eq!(body["lookup"]["tc"], TC_LOOKUP, "{path}");
        assert!(body["lookup"].get("maker").is_some(), "{path}");
        // Nothing this server words itself says a recall applies.
        let lower = text.to_lowercase();
        for claim in [
            "applies to this vin",
            "applies to your",
            "is affected",
            "open recall",
        ] {
            assert!(!lower.contains(claim), "{path} says {claim:?}");
        }
    }
}

#[tokio::test]
async fn text_from_the_source_is_given_as_it_is() {
    let app = common::recalls_app().await;
    // JSON carries the source's characters as data; pages escape them.
    let body = ok(&app, "/v1/recalls/nhtsa/19V600000").await;
    assert_eq!(body["manufacturer"], "B&B <script>alert(1)</script>");
    assert_eq!(
        body["vehicles"][0]["model"],
        "Model \"<img src=x onerror=alert(1)>"
    );
    assert_eq!(
        body["vehicles"][0]["vehicle_id"],
        "2019_b-b-script_model-img"
    );
    let response = app.get("/v1/recalls/nhtsa/19V600000").await;
    assert_eq!(
        common::header(&response, "content-type"),
        "application/json"
    );
    assert_eq!(common::header(&response, "x-robots-tag"), "noindex");
}
