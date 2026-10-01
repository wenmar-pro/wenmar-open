use axum::http::StatusCode;
use serde_json::{Value, json};

use crate::common;

fn names(body: &Value, field: &str) -> Vec<String> {
    body.as_array()
        .unwrap()
        .iter()
        .map(|item| item[field].as_str().unwrap().to_owned())
        .collect()
}

#[tokio::test]
async fn years_are_newest_first() {
    let app = common::app().await;
    let (status, body) = app.json("/v1/vehicles/years").await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body, json!([2023, 2022, 2020, 2019, 2018]));
    let (_, body) = app.json("/v1/vehicles/years?term=201").await;
    assert_eq!(body, json!([2019, 2018]));
}

#[tokio::test]
async fn makes_are_popular_first_and_can_be_narrowed() {
    let app = common::app().await;
    let (status, body) = app.json("/v1/vehicles/makes").await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(
        body[0],
        json!({ "id": "ford", "name": "Ford", "popular": true })
    );
    // Trailers are outside the default scope.
    assert!(!names(&body, "id").contains(&"ranger-trailers".to_owned()));

    let (_, body) = app.json("/v1/vehicles/makes?year=2023").await;
    assert_eq!(names(&body, "id"), ["hyundai"]);
    let (_, body) = app.json("/v1/vehicles/makes?term=chev").await;
    assert_eq!(names(&body, "name"), ["Chevrolet"]);
    let (_, body) = app.json("/v1/vehicles/makes?scope=all&term=ranger").await;
    assert_eq!(names(&body, "id"), ["ranger-trailers"]);
    let (_, body) = app.json("/v1/vehicles/makes?limit=2").await;
    assert_eq!(body.as_array().unwrap().len(), 2);
    // An empty form field is the same as leaving it out.
    let (status, _) = app.json("/v1/vehicles/makes?year=&limit=").await;
    assert_eq!(status, StatusCode::OK);
}

#[tokio::test]
async fn models_take_a_name_an_alias_or_an_id() {
    let app = common::app().await;
    let (status, body) = app.json("/v1/vehicles/models?make=honda").await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(
        body,
        json!([
            { "id": "civic", "name": "Civic", "year_from": 2018, "year_to": 2020 },
            { "id": "cr-v", "name": "CR-V", "year_from": 2019, "year_to": 2019 }
        ])
    );
    let (_, body) = app.json("/v1/vehicles/models?make=HONDA&year=2020").await;
    assert_eq!(names(&body, "id"), ["civic"]);
    let (_, body) = app.json("/v1/vehicles/models?make=chevy").await;
    assert_eq!(names(&body, "name"), ["Silverado"]);
    let (_, body) = app.json("/v1/vehicles/models?make=honda&term=crv").await;
    assert_eq!(names(&body, "name"), ["CR-V"]);
    // An unknown make has no models. It is not an error.
    let (status, body) = app.json("/v1/vehicles/models?make=nobody").await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body, json!([]));
}

#[tokio::test]
async fn submodels_and_engines_follow_the_steps_before_them() {
    let app = common::app().await;
    let (status, body) = app
        .json("/v1/vehicles/submodels?make=honda&model=civic&year=2019")
        .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(
        body,
        json!([
            { "id": "lx", "name": "LX", "kind": "trim" },
            { "id": "si", "name": "Si", "kind": "trim" },
            { "id": "touring", "name": "Touring", "kind": "trim" }
        ])
    );
    // `trims` is the same list under the name most people use.
    let (_, trims) = app
        .json("/v1/vehicles/trims?make=honda&model=civic&year=2019")
        .await;
    assert_eq!(trims, body);

    let (_, body) = app
        .json("/v1/vehicles/engines?make=ford&model=f150&year=2019")
        .await;
    assert_eq!(
        body,
        json!([
            { "id": "3-5l-turbo-v6", "label": "3.5L Turbo V6", "vin8": "4G", "preset": false },
            { "id": "5-0l-v8", "label": "5.0L V8", "vin8": "5", "preset": false }
        ])
    );
    // The Si comes only with the 1.5L Turbo.
    let (_, body) = app
        .json("/v1/vehicles/engines?make=honda&model=civic&year=2019&submodel=si")
        .await;
    assert_eq!(names(&body, "label"), ["1.5L Turbo"]);
}

#[tokio::test]
async fn a_missing_or_malformed_parameter_is_400() {
    let app = common::app().await;
    let cases = [
        ("/v1/vehicles/models", "make"),
        ("/v1/vehicles/models?make=", "make"),
        ("/v1/vehicles/submodels?model=civic&year=2019", "make"),
        ("/v1/vehicles/submodels?make=honda&year=2019", "model"),
        ("/v1/vehicles/submodels?make=honda&model=civic", "year"),
        ("/v1/vehicles/engines?make=honda&model=civic", "year"),
        ("/v1/vehicles/search", "q"),
        ("/v1/vehicles/search?q=%20", "q"),
        ("/v1/vehicles/makes?scope=cars", "scope"),
    ];
    for (path, field) in cases {
        let (status, body) = app.json(path).await;
        assert_eq!(status, StatusCode::BAD_REQUEST, "{path}");
        assert_eq!(body["error"]["code"], "validation_failed", "{path}");
        assert_eq!(body["error"]["details"]["field"], field, "{path}");
    }
    for path in [
        "/v1/vehicles/makes?year=twenty",
        "/v1/vehicles/makes?year=99999",
        "/v1/vehicles/makes?limit=-1",
    ] {
        let (status, body) = app.json(path).await;
        assert_eq!(status, StatusCode::BAD_REQUEST, "{path}");
        assert_eq!(body["error"]["code"], "validation_failed", "{path}");
    }
}

#[tokio::test]
async fn search_reads_what_people_type() {
    let app = common::app().await;
    let (status, body) = app.json("/v1/vehicles/search?q=2019+civic+si").await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body[0]["id"], "2019_honda_civic_si");
    assert_eq!(
        body[0]["summary"],
        "2019 Honda Civic Si, Manual, FWD, Sedan"
    );
    let (_, body) = app.json("/v1/vehicles/search?q=chevy%201500").await;
    assert_eq!(body[0]["id"], "2019_chevrolet_silverado_1500");
    let (_, body) = app.json("/v1/vehicles/search?q=f150").await;
    assert_eq!(body[0]["id"], "2019_ford_f-150");
    let (_, body) = app.json("/v1/vehicles/search?q=honda&limit=1").await;
    assert_eq!(body.as_array().unwrap().len(), 1);
}

#[tokio::test]
async fn search_text_that_names_nothing_finds_nothing() {
    let app = common::app().await;
    let long = "x".repeat(50_000);
    let inputs = [
        "%25",
        "_",
        "%27%3B+DROP+TABLE+catalog_make%3B--",
        "%22unterminated",
        "AND+OR+NOT",
        "%2B-%28%29*%3A%5E~",
        "%F0%9F%9A%97",
        long.as_str(),
    ];
    for input in inputs {
        let (status, body) = app.json(&format!("/v1/vehicles/search?q={input}")).await;
        assert_eq!(status, StatusCode::OK, "{:.40}", input);
        assert_eq!(body, json!([]), "{:.40}", input);
    }
    // The catalog is still there.
    let (_, body) = app.json("/v1/vehicles/makes").await;
    assert_eq!(body[0]["id"], "ford");
}

#[tokio::test]
async fn an_entry_is_found_by_its_id() {
    let app = common::app().await;
    let (status, body) = app.json("/v1/vehicles/2019_honda_civic_si").await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(
        body,
        json!({
            "id": "2019_honda_civic_si",
            "year": 2019,
            "make": "Honda",
            "model": "Civic",
            "submodel": "Si",
            "transmission": "Manual",
            "drive": "FWD",
            "body": "Sedan",
            "vehicle_types": ["Passenger Car"],
            "summary": "2019 Honda Civic Si, Manual, FWD, Sedan"
        })
    );
    for id in [
        "2019_honda_nothing",
        "1066_honda_civic",
        "not-an-id",
        "..%2F..%2Fetc%2Fpasswd",
        "2019_honda_civic_si_x_y_z",
        "%00",
    ] {
        let (status, body) = app.json(&format!("/v1/vehicles/{id}")).await;
        assert_eq!(status, StatusCode::NOT_FOUND, "{id}");
        assert_eq!(body["error"]["code"], "not_found", "{id}");
    }
}
