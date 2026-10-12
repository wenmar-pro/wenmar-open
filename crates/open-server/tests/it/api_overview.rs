//! The answers that sit over the per-dataset routes: the overview of a
//! model year and of a VIN, the model years of a model, the campaigns
//! reported since a day, and the recall count in a batch decode.

use axum::http::StatusCode;
use serde_json::{Value, json};

use crate::common::{self, KONA, OGL_CANADA, TestApp, body_json};

const NHTSA_LOOKUP: &str = "https://www.nhtsa.gov/recalls";
const TC_LOOKUP: &str =
    "https://wwwapps.tc.gc.ca/Saf-Sec-Sur/7/VRDB-BDRV/search-recherche/menu.aspx?lang=eng";

/// The blocks no dataset fills yet. A plan that fills one takes its name
/// out of this list and adds a test of its own.
const NOT_YET: [&str; 2] = ["tires", "labor"];

async fn ok(app: &TestApp, path: &str) -> Value {
    let (status, body) = app.json(path).await;
    assert_eq!(status, StatusCode::OK, "{path}: {body}");
    body
}

fn campaigns(list: &Value) -> Vec<&str> {
    list.as_array()
        .unwrap_or_else(|| panic!("not a list: {list}"))
        .iter()
        .map(|recall| recall["campaign"].as_str().unwrap())
        .collect()
}

/// Every key an overview has, in the order they are written.
const KEYS: [&str; 13] = [
    "vehicle",
    "decode",
    "specs",
    "fuel",
    "fluids",
    "tires",
    "recalls",
    "safety",
    "labor",
    "bulletins",
    "links",
    "attribution",
    "warnings",
];

fn assert_shape(body: &Value) {
    let object = body.as_object().unwrap();
    for key in KEYS {
        assert!(object.contains_key(key), "no {key} in {body}");
    }
    assert_eq!(object.len(), KEYS.len(), "{body}");
}

#[tokio::test]
async fn a_model_year_gets_everything_held_for_it_in_one_answer() {
    let app = common::recalls_app().await;
    let body = ok(&app, "/v1/vehicles/2023_hyundai_kona/overview").await;
    assert_shape(&body);
    assert_eq!(
        body["vehicle"],
        json!({ "id": "2023_hyundai_kona", "year": 2023, "make": "Hyundai", "model": "Kona" })
    );
    assert_eq!(body["decode"], Value::Null);
    let recalls = &body["recalls"];
    assert_eq!(recalls["may_apply"], true);
    assert_eq!(recalls["count"], json!({ "us": 3, "ca": 1 }));
    assert_eq!(
        recalls["as_of"],
        json!({ "us": "2026-10-11", "ca": "2026-10-06" })
    );
    // 24V456000 says not to drive, 23V123000 to park outside.
    assert_eq!(recalls["do_not_drive"], true);
    assert_eq!(recalls["park_outside"], true);
    // The three newest of the four, as `/v1/vehicles/{id}/recalls` gives them.
    assert_eq!(
        campaigns(&recalls["newest"]),
        ["24V456000", "2023101", "23V123000"]
    );
    let whole = ok(&app, "/v1/vehicles/2023_hyundai_kona/recalls").await;
    assert_eq!(recalls["newest"][0], whole["recalls"][0]);
    assert_eq!(recalls["lookup"], whole["lookup"]);
    assert_eq!(recalls["lookup"]["nhtsa"], NHTSA_LOOKUP);
    assert_eq!(recalls["lookup"]["tc"], TC_LOOKUP);
    assert_eq!(
        recalls["url"],
        "https://open.example/makes/hyundai/kona/2023/recalls"
    );
    assert_eq!(
        body["links"]["page"],
        "https://open.example/makes/hyundai/kona/2023"
    );
    // The maker's links are an object, or null when none are on file.
    let maker = &body["links"]["maker"];
    assert!(maker.is_null() || maker["checked"].is_string(), "{maker}");
    assert_eq!(body["attribution"], json!([OGL_CANADA]));
    assert_eq!(body["warnings"], json!([]));

    // A submodel in the id is set aside, as for the recall list.
    let trimmed = ok(&app, "/v1/vehicles/2023_hyundai_kona_se/overview").await;
    assert_eq!(trimmed, body);

    // A model year no campaign lists: counted, and none found.
    let civic = ok(&app, "/v1/vehicles/2019_honda_civic/overview").await;
    assert_eq!(civic["recalls"]["count"], json!({ "us": 0, "ca": 0 }));
    assert_eq!(civic["recalls"]["newest"], json!([]));
    assert_eq!(civic["recalls"]["do_not_drive"], false);
    assert_eq!(civic["recalls"]["may_apply"], true);
}

#[tokio::test]
async fn blocks_with_no_data_yet_are_null() {
    let app = common::recalls_app().await;
    for path in [
        "/v1/vehicles/2023_hyundai_kona/overview".to_owned(),
        format!("/v1/vin/{KONA}/overview"),
    ] {
        let body = ok(&app, &path).await;
        for block in NOT_YET {
            assert_eq!(body[block], Value::Null, "{block} in {path}");
            assert!(body.as_object().unwrap().contains_key(block), "{block}");
        }
    }
}

#[tokio::test]
async fn the_overview_answers_without_a_shop_file_and_with_one_country() {
    let app = common::app().await;
    let body = ok(&app, "/v1/vehicles/2023_hyundai_kona/overview").await;
    assert_shape(&body);
    assert_eq!(body["recalls"], Value::Null);
    assert_eq!(body["attribution"], json!([]));
    assert_eq!(body["vehicle"]["id"], "2023_hyundai_kona");
    assert_eq!(
        body["links"]["page"],
        "https://open.example/makes/hyundai/kona/2023"
    );
    let body = ok(&app, &format!("/v1/vin/{KONA}/overview")).await;
    assert_eq!(body["recalls"], Value::Null);
    assert_eq!(body["decode"]["vin"], KONA);

    // The United States alone: Canada is not counted, and is not zero.
    let american = common::app_with_shop(common::RECALL_DATASETS_US, &common::recall_rows()).await;
    let body = ok(&american, "/v1/vehicles/2023_hyundai_kona/overview").await;
    assert_eq!(body["recalls"]["count"], json!({ "us": 3, "ca": null }));
    assert_eq!(
        body["recalls"]["as_of"],
        json!({ "us": "2026-10-11", "ca": null })
    );
    assert_eq!(body["attribution"], json!([]));
}

#[tokio::test]
async fn a_vin_gets_the_overview_of_the_model_year_it_decodes_to() {
    let app = common::recalls_app().await;
    let body = ok(&app, &format!("/v1/vin/{KONA}/overview")).await;
    assert_shape(&body);
    let decode = &body["decode"];
    assert_eq!(decode["vin"], KONA);
    assert_eq!(decode["valid"], true);
    assert_eq!(decode["year"], 2023);
    assert_eq!(decode["make"], "Hyundai");
    assert_eq!(decode["model"], "Kona");
    // What the decode itself says of the engine and the catalog entry.
    let (_, plain) = app.json(&format!("/v1/vin/{KONA}")).await;
    assert_eq!(decode["entry_id"], plain["catalog"]["entry"]["id"]);
    assert_eq!(
        decode["engine_settled"],
        plain["catalog"].get("engine_id").is_some()
    );
    assert_eq!(
        decode["engine"],
        plain["engine"].get("label").cloned().unwrap_or(Value::Null)
    );
    // The rest is the model year's own overview.
    let vehicle = ok(&app, "/v1/vehicles/2023_hyundai_kona/overview").await;
    for key in ["vehicle", "recalls", "links", "attribution"] {
        assert_eq!(body[key], vehicle[key], "{key}");
    }
    assert_eq!(body["warnings"], plain["warnings"]);

    // What is not a VIN fails as the decode does.
    let (status, body) = app.json("/v1/vin/KM8/overview").await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert_eq!(body["error"]["code"], "invalid_vin");
    let (status, _) = app
        .json(&format!("/v1/vin/{}/overview", common::UNKNOWN))
        .await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    let (status, body) = app
        .json(&format!("/v1/vin/{KONA}/overview?year=1900"))
        .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert_eq!(body["error"]["details"]["field"], "year");
}

#[tokio::test]
async fn a_vin_that_reaches_no_catalog_vehicle_gets_nulls_and_a_warning() {
    let app = common::recalls_app().await;
    let body = ok(&app, &format!("/v1/vin/{}/overview", common::COACH)).await;
    assert_shape(&body);
    assert_eq!(body["vehicle"], Value::Null);
    assert_eq!(body["recalls"], Value::Null);
    assert_eq!(body["links"]["page"], Value::Null);
    assert_eq!(body["decode"]["vin"], common::COACH);
    assert_eq!(body["decode"]["entry_id"], Value::Null);
    assert_eq!(body["decode"]["engine_settled"], false);
    let codes: Vec<&str> = body["warnings"]
        .as_array()
        .unwrap()
        .iter()
        .map(|warning| warning["code"].as_str().unwrap())
        .collect();
    assert_eq!(codes.last(), Some(&"no_catalog_vehicle"));
}

#[tokio::test]
async fn an_overview_of_a_vehicle_that_does_not_exist_is_404() {
    for app in [common::app().await, common::recalls_app().await] {
        for path in [
            "/v1/vehicles/2019_honda_nothing/overview",
            "/v1/vehicles/nonsense/overview",
            "/v1/vehicles/2031_hyundai_kona/overview",
        ] {
            let (status, body) = app.json(path).await;
            assert_eq!(status, StatusCode::NOT_FOUND, "{path}");
            assert_eq!(body["error"]["code"], "not_found", "{path}");
            assert_eq!(body["error"]["message"], "No vehicle has that id.");
        }
    }
}

#[tokio::test]
async fn a_model_lists_its_years_with_the_recalls_of_each() {
    let app = common::recalls_app().await;
    let body = ok(&app, "/v1/models/hyundai/kona").await;
    assert_eq!(body["make"], json!({ "id": "hyundai", "name": "Hyundai" }));
    assert_eq!(body["model"], json!({ "id": "kona", "name": "Kona" }));
    assert_eq!(body["may_apply"], true);
    assert_eq!(
        body["as_of"],
        json!({ "us": "2026-10-11", "ca": "2026-10-06" })
    );
    assert_eq!(body["lookup"]["nhtsa"], NHTSA_LOOKUP);
    assert_eq!(body["lookup"]["tc"], TC_LOOKUP);
    assert_eq!(body["attribution"], json!([OGL_CANADA]));
    assert_eq!(
        body["years"],
        json!([
            {
                "id": "2023_hyundai_kona", "year": 2023,
                "recalls": { "us": 3, "ca": 1 }, "bulletins": null,
                "url": "https://open.example/makes/hyundai/kona/2023"
            },
            {
                "id": "2022_hyundai_kona", "year": 2022,
                "recalls": { "us": 1, "ca": 0 }, "bulletins": null,
                "url": "https://open.example/makes/hyundai/kona/2022"
            }
        ])
    );
    // Each count is the length of that year's own list.
    for year in body["years"].as_array().unwrap() {
        let id = year["id"].as_str().unwrap();
        let list = ok(&app, &format!("/v1/vehicles/{id}/recalls")).await;
        let both =
            year["recalls"]["us"].as_u64().unwrap() + year["recalls"]["ca"].as_u64().unwrap();
        assert_eq!(
            list["recalls"].as_array().unwrap().len() as u64,
            both,
            "{id}"
        );
    }
    // A name, an alias and another spelling reach the same model.
    let named = ok(&app, "/v1/models/chevy/Silverado").await;
    assert_eq!(named["make"]["id"], "chevrolet");
    assert_eq!(named["years"][0]["id"], "2019_chevrolet_silverado");
    assert_eq!(named["years"][0]["recalls"], json!({ "us": 1, "ca": 1 }));
    let spelled = ok(&app, "/v1/models/ford/f150").await;
    assert_eq!(spelled["model"], json!({ "id": "f-150", "name": "F-150" }));
    assert_eq!(spelled["years"][0]["recalls"], json!({ "us": 0, "ca": 0 }));
    // Civic has three years and no campaign.
    let civic = ok(&app, "/v1/models/honda/civic").await;
    let years: Vec<u64> = civic["years"]
        .as_array()
        .unwrap()
        .iter()
        .map(|year| year["year"].as_u64().unwrap())
        .collect();
    assert_eq!(years, [2020, 2019, 2018]);

    for path in ["/v1/models/honda/nothing", "/v1/models/nobody/civic"] {
        let (status, body) = app.json(path).await;
        assert_eq!(status, StatusCode::NOT_FOUND, "{path}");
        assert_eq!(body["error"]["message"], "No model has that id.");
    }
}

#[tokio::test]
async fn a_model_lists_its_years_on_a_server_with_no_recalls() {
    let app = common::app().await;
    let body = ok(&app, "/v1/models/hyundai/kona").await;
    assert_eq!(body["as_of"], json!({ "us": null, "ca": null }));
    assert_eq!(body["attribution"], json!([]));
    assert_eq!(body["may_apply"], true);
    assert_eq!(body["lookup"]["nhtsa"], NHTSA_LOOKUP);
    for year in body["years"].as_array().unwrap() {
        assert_eq!(year["recalls"], Value::Null);
        assert_eq!(year["bulletins"], Value::Null);
    }
    assert_eq!(body["years"].as_array().unwrap().len(), 2);
}

#[tokio::test]
async fn recent_lists_the_campaigns_since_a_day_with_their_vehicles() {
    let app = common::recalls_app().await;
    // `recent` is this route, not a campaign of a source called `recent`.
    let body = ok(&app, "/v1/recalls/recent?since=2023-01-01").await;
    assert_eq!(body["may_apply"], true);
    assert_eq!(body["since"], "2023-01-01");
    assert_eq!(
        body["as_of"],
        json!({ "us": "2026-10-11", "ca": "2026-10-06" })
    );
    assert_eq!(body["lookup"]["nhtsa"], NHTSA_LOOKUP);
    assert_eq!(body["lookup"]["tc"], TC_LOOKUP);
    assert_eq!(body["attribution"], json!([OGL_CANADA]));
    assert_eq!(body["next_cursor"], Value::Null);
    let listed: Vec<&str> = body["recalls"]
        .as_array()
        .unwrap()
        .iter()
        .map(|entry| entry["recall"]["campaign"].as_str().unwrap())
        .collect();
    assert_eq!(listed, ["24V456000", "2023101", "23V123000"]);
    // Each campaign is as the list gives it, with the vehicles it lists.
    let list = ok(&app, "/v1/recalls?limit=100").await;
    let first = &body["recalls"][0];
    assert_eq!(first["recall"], list["recalls"][0]);
    assert_eq!(first["vehicles_listed"], 1);
    assert_eq!(
        first["vehicles"],
        json!([{
            "year": 2023, "make": "HYUNDAI", "model": "KONA ELECTRIC",
            "vehicle_id": "2023_hyundai_kona", "match": "base",
            "built_from": null, "built_to": null
        }])
    );
    // The day itself is included.
    let body = ok(&app, "/v1/recalls/recent?since=2024-05-10").await;
    assert_eq!(body["recalls"].as_array().unwrap().len(), 1);
    let body = ok(&app, "/v1/recalls/recent?since=2024-05-11").await;
    assert_eq!(body["recalls"], json!([]));

    // Narrowed by country and by make; a make the catalog lacks lists nothing.
    let body = ok(&app, "/v1/recalls/recent?since=2019-01-01&country=CA").await;
    let listed: Vec<&str> = body["recalls"]
        .as_array()
        .unwrap()
        .iter()
        .map(|entry| entry["recall"]["campaign"].as_str().unwrap())
        .collect();
    assert_eq!(listed, ["2023101", "2019631"]);
    assert_eq!(body["attribution"], json!([OGL_CANADA]));
    let body = ok(&app, "/v1/recalls/recent?since=2019-01-01&make=chevy").await;
    let listed: Vec<&str> = body["recalls"]
        .as_array()
        .unwrap()
        .iter()
        .map(|entry| entry["recall"]["campaign"].as_str().unwrap())
        .collect();
    assert_eq!(listed, ["2019631", "19V888000"]);
    // 19V888000 also lists a GMC that matched nothing: it has no vehicle id.
    let gm = &body["recalls"][1];
    assert_eq!(gm["vehicles_listed"], 2);
    assert_eq!(gm["vehicles"][0]["vehicle_id"], "2019_chevrolet_silverado");
    assert_eq!(gm["vehicles"][1]["vehicle_id"], Value::Null);
    let body = ok(&app, "/v1/recalls/recent?since=2019-01-01&make=nobody").await;
    assert_eq!(body["recalls"], json!([]));

    // Pages, with the cursor of the list.
    let page = ok(&app, "/v1/recalls/recent?since=1960-01-01&limit=2").await;
    assert_eq!(page["recalls"].as_array().unwrap().len(), 2);
    let cursor = page["next_cursor"].as_str().unwrap();
    let next = ok(
        &app,
        &format!("/v1/recalls/recent?since=1960-01-01&limit=2&cursor={cursor}"),
    )
    .await;
    assert_eq!(next["recalls"][0]["recall"]["campaign"], "23V123000");
}

#[tokio::test]
async fn recent_refuses_what_it_cannot_read() {
    let app = common::recalls_app().await;
    for (path, field) in [
        ("/v1/recalls/recent", "since"),
        ("/v1/recalls/recent?since=", "since"),
        ("/v1/recalls/recent?since=yesterday", "since"),
        ("/v1/recalls/recent?since=2026-9-1", "since"),
        ("/v1/recalls/recent?since=2026-09-01&country=mx", "country"),
        ("/v1/recalls/recent?since=2026-09-01&cursor=nope", "cursor"),
    ] {
        let (status, body) = app.json(path).await;
        assert_eq!(status, StatusCode::BAD_REQUEST, "{path}");
        assert_eq!(body["error"]["code"], "validation_failed", "{path}");
        assert_eq!(body["error"]["details"]["field"], field, "{path}");
    }
    // One campaign is still reached by its source and number.
    let campaign = ok(&app, "/v1/recalls/nhtsa/19V888000").await;
    assert_eq!(campaign["campaign"], "19V888000");

    // No recalls on the server: the dataset is named, whatever else is wrong.
    let bare = common::app().await;
    for path in ["/v1/recalls/recent?since=2026-09-01", "/v1/recalls/recent"] {
        let (status, body) = bare.json(path).await;
        assert_eq!(status, StatusCode::NOT_FOUND, "{path}");
        assert_eq!(body["error"]["code"], "dataset_unavailable", "{path}");
        assert_eq!(body["error"]["details"]["dataset"], "recalls_us");
    }
    let american = common::app_with_shop(common::RECALL_DATASETS_US, &common::recall_rows()).await;
    let (status, body) = american
        .json("/v1/recalls/recent?since=2019-01-01&country=ca")
        .await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    assert_eq!(body["error"]["details"]["dataset"], "recalls_ca");
}

#[tokio::test]
async fn a_batch_adds_the_recall_count_only_when_asked() {
    let app = common::recalls_app().await;
    let vins = json!({ "vins": [KONA, "nope", common::COACH] }).to_string();
    let plain = body_json(app.post_json("/v1/vin/batch", &vins).await).await;
    assert!(plain[0].get("recalls").is_none(), "{}", plain[0]);

    let response = app.post_json("/v1/vin/batch?include=recalls", &vins).await;
    assert_eq!(response.status(), StatusCode::OK);
    let with = body_json(response).await;
    assert_eq!(
        with[0]["recalls"],
        json!({
            "may_apply": true,
            "count": { "us": 3, "ca": 1 },
            "do_not_drive": true,
            "park_outside": true,
            "lookup": { "nhtsa": NHTSA_LOOKUP, "tc": TC_LOOKUP, "maker": with[0]["recalls"]["lookup"]["maker"] }
        })
    );
    // Nothing else of the decode changes, and the other items are as before.
    let mut without = with[0].clone();
    without.as_object_mut().unwrap().remove("recalls");
    assert_eq!(without, plain[0]);
    assert_eq!(with[1], plain[1]);
    // A VIN that reaches no model year has nothing to count.
    assert_eq!(with[2], plain[2]);
    assert!(with[2].get("recalls").is_none());

    // Blank is the same as left out; anything else is refused.
    let blank = body_json(app.post_json("/v1/vin/batch?include=", &vins).await).await;
    assert_eq!(blank, plain);
    let response = app
        .post_json("/v1/vin/batch?include=bulletins", &vins)
        .await;
    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    let body = body_json(response).await;
    assert_eq!(body["error"]["details"]["field"], "include");

    // A server with no recalls says so when they are asked for by name.
    let bare = common::app().await;
    let response = bare.post_json("/v1/vin/batch?include=recalls", &vins).await;
    assert_eq!(response.status(), StatusCode::NOT_FOUND);
    let body = body_json(response).await;
    assert_eq!(body["error"]["code"], "dataset_unavailable");
    let response = bare.post_json("/v1/vin/batch", &vins).await;
    assert_eq!(response.status(), StatusCode::OK);
}

#[tokio::test]
async fn every_answer_with_recalls_says_may_apply_and_never_more() {
    let app = common::recalls_app().await;
    let vins = json!({ "vins": [KONA] }).to_string();
    let batch = body_json(app.post_json("/v1/vin/batch?include=recalls", &vins).await).await;
    let answers = [
        ok(&app, "/v1/vehicles/2023_hyundai_kona/overview").await["recalls"].clone(),
        ok(&app, &format!("/v1/vin/{KONA}/overview")).await["recalls"].clone(),
        ok(&app, "/v1/models/hyundai/kona").await,
        ok(&app, "/v1/recalls/recent?since=2019-01-01").await,
        batch[0]["recalls"].clone(),
    ];
    for answer in &answers {
        assert_eq!(answer["may_apply"], true, "{answer}");
        assert_eq!(answer["lookup"]["nhtsa"], NHTSA_LOOKUP, "{answer}");
        assert_eq!(answer["lookup"]["tc"], TC_LOOKUP, "{answer}");
    }
    // Nothing this server writes says a recall applies to a vehicle. The
    // sources' own text is set aside: it is theirs.
    let mut ours = json!(answers);
    fn drop_source_text(value: &mut Value) {
        match value {
            Value::Object(object) => {
                for key in ["summary", "component", "manufacturer"] {
                    object.remove(key);
                }
                object.values_mut().for_each(drop_source_text);
            }
            Value::Array(items) => items.iter_mut().for_each(drop_source_text),
            _ => {}
        }
    }
    drop_source_text(&mut ours);
    let text = ours.to_string().to_lowercase();
    for claim in ["applies to", "is affected", "open recall", "your vehicle"] {
        assert!(!text.contains(claim), "{claim}");
    }
}

#[tokio::test]
async fn the_overview_has_the_crash_test_ratings_when_the_server_holds_them() {
    let app = common::ratings_and_recalls_app().await;
    let body = ok(&app, "/v1/vehicles/2023_hyundai_kona/overview").await;
    let (_, safety) = app.json("/v1/vehicles/2023_hyundai_kona/safety").await;
    assert_eq!(body["safety"], safety);
    assert_eq!(body["safety"]["ratings"].as_array().unwrap().len(), 3);
    // Without ratings the block is null, and it is still there.
    let app = common::recalls_app().await;
    let body = ok(&app, "/v1/vehicles/2023_hyundai_kona/overview").await;
    assert_eq!(body["safety"], Value::Null);
    assert!(body.as_object().unwrap().contains_key("safety"));
}

#[tokio::test]
async fn fuel_specs_and_fluids_are_filled_when_held_and_narrowed_by_a_settled_engine() {
    // A server that holds recalls and no fuel data: the three are null.
    let app = common::recalls_app().await;
    let body = ok(&app, "/v1/vehicles/2023_hyundai_kona/overview").await;
    for block in ["specs", "fuel", "fluids"] {
        assert_eq!(body[block], Value::Null, "{block}");
    }
    // A server that holds them: lists for the model year.
    let app = common::fuel_app().await;
    let body = ok(&app, "/v1/vehicles/2023_hyundai_kona/overview").await;
    assert_eq!(body["fuel"].as_array().unwrap().len(), 5);
    assert_eq!(body["specs"].as_array().unwrap().len(), 4);
    assert_eq!(body["fluids"]["engine_oil"].as_array().unwrap().len(), 2);
    assert_eq!(body["attribution"], json!([common::OGL_CANADA]));
    assert_eq!(body["recalls"], Value::Null);
    // The VIN settles the 2.0L: only what was matched to it.
    let body = ok(&app, &format!("/v1/vin/{KONA}/overview")).await;
    assert_eq!(body["decode"]["engine_settled"], true);
    let fuel = body["fuel"].as_array().unwrap();
    assert_eq!(fuel.len(), 3);
    assert!(
        fuel.iter()
            .all(|rating| rating["engine"]["label"] == "2.0L")
    );
    assert_eq!(body["specs"].as_array().unwrap().len(), 2);
    assert_eq!(
        body["fluids"]["engine_oil"],
        json!([{
            "confidence": "engine",
            "engine": { "id": "2-0l", "label": "2.0L", "vin8": "A" },
            "displacement_l": 2.0, "cylinders": 4,
            "grades": ["0W-20"], "as_written": ["0W-20", "SAE 0W-20"],
            "source": "EPA fuel economy certification"
        }])
    );
}

#[tokio::test]
async fn the_overview_has_a_bulletins_block_on_a_server_that_holds_them() {
    let app = common::bulletins_app().await;
    let (status, body) = app.json("/v1/vehicles/2023_hyundai_kona/overview").await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let block = &body["bulletins"];
    assert_eq!(block["count"], 6);
    assert_eq!(block["as_of"], "2026-10-11");
    assert_eq!(
        block["by_type"],
        json!([
            { "id": "service_bulletin", "name": "Service bulletin", "count": 4 },
            { "id": "warranty_extension", "name": "Warranty extension", "count": 1 },
            { "id": "over_the_air", "name": "Over-the-air update", "count": 1 }
        ])
    );
    assert_eq!(
        block["url"],
        "https://open.example/makes/hyundai/kona/2023/bulletins"
    );
    // The three newest, exactly as the bulletin route lists them.
    let (_, list) = app
        .json("/v1/vehicles/2023_hyundai_kona/bulletins?limit=3")
        .await;
    assert_eq!(block["newest"], list["bulletins"]);
    assert_eq!(block["newest"].as_array().unwrap().len(), 3);
    // The same block for the VIN that decodes to that model year.
    let (_, by_vin) = app
        .json(&format!("/v1/vin/{}/overview", common::KONA))
        .await;
    assert_eq!(by_vin["bulletins"], body["bulletins"]);
    // The recalls block is still there beside it.
    assert_eq!(body["recalls"]["may_apply"], true);
    // A model year with none: a block with zero, not null.
    let (_, none) = app.json("/v1/vehicles/2019_honda_civic/overview").await;
    assert_eq!(none["bulletins"]["count"], 0);
    assert_eq!(none["bulletins"]["newest"], json!([]));
}

#[tokio::test]
async fn the_overview_has_no_bulletins_block_without_the_dataset() {
    for app in [common::app().await, common::recalls_app().await] {
        let (status, body) = app.json("/v1/vehicles/2023_hyundai_kona/overview").await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(body["bulletins"], Value::Null);
    }
}

#[tokio::test]
async fn each_model_year_of_a_model_has_its_bulletin_count() {
    let app = common::bulletins_app().await;
    let (status, body) = app.json("/v1/models/hyundai/kona").await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let count = |year: u64| {
        body["years"]
            .as_array()
            .unwrap()
            .iter()
            .find(|entry| entry["year"] == year)
            .map(|entry| entry["bulletins"].clone())
    };
    assert_eq!(count(2023), Some(json!(6)));
    assert_eq!(count(2022), Some(json!(2)));
    // A server without bulletins says null, not zero.
    let (_, body) = common::recalls_app()
        .await
        .json("/v1/models/hyundai/kona")
        .await;
    assert!(
        body["years"]
            .as_array()
            .unwrap()
            .iter()
            .all(|entry| entry["bulletins"].is_null())
    );
}
