//! Search through the whole application: the catalog's own matching first,
//! then the full-text index.

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
async fn words_the_catalog_cannot_place_fall_back_to_the_full_text_index() {
    use open_server::db::Db;
    use wenmar_vehicles::Scope;

    // The catalog's own matching reads words in order and finds nothing here.
    let fixture = common::data_file();
    let db = Db::open(&fixture.path(), 1).await.unwrap();
    for text in ["the civic by honda", "a used 2019 civic"] {
        let alone = db
            .run(move |worker| worker.catalog.search(text, Scope::Light, 10).unwrap())
            .await
            .unwrap();
        assert!(alone.is_empty(), "{text}");
    }

    let app = common::app().await;
    // Without a year, the newest model year.
    let (_, body) = app.json("/v1/vehicles/search?q=the+civic+by+honda").await;
    assert_eq!(body[0]["id"], "2020_honda_civic");
    // With a year the model was made in, that year.
    let (_, body) = app.json("/v1/vehicles/search?q=a+used+2019+civic").await;
    assert_eq!(names(&body, "id"), ["2019_honda_civic"]);
    // With a year it was not made in, nothing: a wrong year is not guessed.
    let (_, body) = app.json("/v1/vehicles/search?q=used+civic+2021").await;
    assert_eq!(body, json!([]));
    // The scope applies to the index too.
    let (_, body) = app.json("/v1/vehicles/search?q=deck+tilt").await;
    assert_eq!(body, json!([]));
    let (_, body) = app.json("/v1/vehicles/search?q=deck+tilt&scope=all").await;
    assert_eq!(names(&body, "id"), ["2019_ranger-trailers_tilt-deck"]);
}
