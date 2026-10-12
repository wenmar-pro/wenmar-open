//! The shop file beside the data file: opening it, refusing a wrong one,
//! and the reader handlers are given.

use axum::http::StatusCode;
use open_server::error::ApiError;
use open_server::state::AppState;
use wenmar_shopdata::Dataset;
use wenmar_shopdata::schema::SCHEMA_VERSION;

use crate::common::{self, SHOP_DATASETS};

/// Why the server would not start over `fixture` and its shop file.
async fn refusal(fixture: &common::Fixture) -> String {
    match AppState::open(common::shop_config(fixture)).await {
        Ok(_) => panic!("the server started"),
        Err(error) => error.to_string(),
    }
}

#[tokio::test]
async fn with_no_shop_file_there_is_no_reader_and_every_dataset_is_a_404() {
    let app = common::app().await;
    assert!(app.state.shop().is_none());
    let error = app.state.dataset(Dataset::RecallsUs).err().unwrap();
    assert_eq!(error, ApiError::DatasetUnavailable(Dataset::RecallsUs));
    assert_eq!(error.status(), StatusCode::NOT_FOUND);
    assert_eq!(
        error.body().error.message,
        "The recalls_us dataset is not available."
    );
}

#[tokio::test]
async fn a_shop_file_gives_a_reader_for_the_datasets_it_holds() {
    let app = common::app_with_shop(
        SHOP_DATASETS,
        "CREATE TABLE thing (name TEXT); INSERT INTO thing VALUES ('a'), ('b');",
    )
    .await;
    let shop = app.state.shop().unwrap();
    assert_eq!(shop.meta().data_version, "2026.10.12");
    assert_eq!(shop.meta().catalog_data_version, "2026.09");
    assert_eq!(shop.datasets().len(), 2);

    let held = app.state.dataset(Dataset::RecallsUs).unwrap();
    let names = held
        .run(|reader| {
            reader.require(Dataset::RecallsUs)?;
            reader.query("SELECT name FROM thing ORDER BY name", &[])
        })
        .await
        .unwrap()
        .unwrap();
    assert_eq!(names.len(), 2);
    assert_eq!(names[0][0].text(), Some("a"));

    let error = app.state.dataset(Dataset::Bulletins).err().unwrap();
    assert_eq!(error, ApiError::DatasetUnavailable(Dataset::Bulletins));
}

#[tokio::test]
async fn more_work_than_connections_all_finishes() {
    // The fixture has two connection slots.
    let app = common::app_with_shop(SHOP_DATASETS, "").await;
    let shop = app.state.shop().unwrap();
    let mut answers = Vec::new();
    for _ in 0..8 {
        answers.push(shop.run(|reader| reader.datasets().len()));
    }
    for answer in answers {
        assert_eq!(answer.await.unwrap(), 2);
    }
    // A panic in one piece of work does not take a connection with it.
    let failed = shop.run(|_| panic!("a bug in a handler")).await;
    assert!(failed.is_err());
    assert_eq!(shop.run(|reader| reader.datasets().len()).await.unwrap(), 2);
    assert_eq!(shop.run(|reader| reader.datasets().len()).await.unwrap(), 2);
}

#[tokio::test]
async fn the_shop_file_is_only_read() {
    let fixture = common::data_file();
    common::add_shop_file(&fixture, SCHEMA_VERSION, "2026.09", SHOP_DATASETS, "");
    let before = std::fs::read(fixture.shop_path()).unwrap();
    {
        let state = AppState::open(common::shop_config(&fixture)).await.unwrap();
        let refused = state
            .shop()
            .unwrap()
            .run(|reader| reader.query("INSERT INTO meta VALUES ('x', 'y')", &[]))
            .await
            .unwrap();
        assert!(refused.is_err(), "a write must be refused");
        assert_eq!(common::files(&fixture), ["data.sqlite3", "shop.sqlite3"]);
    }
    assert_eq!(common::files(&fixture), ["data.sqlite3", "shop.sqlite3"]);
    assert_eq!(std::fs::read(fixture.shop_path()).unwrap(), before);
}

#[tokio::test]
async fn a_shop_file_that_cannot_be_used_stops_the_server_and_says_why() {
    let path = |fixture: &common::Fixture| fixture.shop_path().display().to_string();

    let missing = common::data_file();
    assert_eq!(
        refusal(&missing).await,
        format!("the shop file {} does not exist", path(&missing))
    );
    assert!(!missing.shop_path().exists(), "it must not be created");

    let newer = common::data_file();
    common::add_shop_file(&newer, "2", "2026.09", SHOP_DATASETS, "");
    assert_eq!(
        refusal(&newer).await,
        format!(
            "the shop file {} cannot be used: shop file has schema version 2, this build reads version 1; rebuild it with open-data build-shop",
            path(&newer)
        )
    );

    let other_month = common::data_file();
    common::add_shop_file(&other_month, SCHEMA_VERSION, "2026.08", SHOP_DATASETS, "");
    assert_eq!(
        refusal(&other_month).await,
        format!(
            "the shop file {} cannot be used: shop file was matched against data version 2026.08, but the data file is version 2026.09; rebuild it with open-data build-shop --catalog <the data file>",
            path(&other_month)
        )
    );

    // The core data file named as the shop file.
    let core = common::data_file();
    std::fs::copy(core.path(), core.shop_path()).unwrap();
    let message = refusal(&core).await;
    assert!(
        message.contains("shop file has schema version 3"),
        "{message}"
    );

    let not_sqlite = common::data_file();
    std::fs::write(not_sqlite.shop_path(), b"not a database").unwrap();
    let message = refusal(&not_sqlite).await;
    assert!(message.contains("cannot be used"), "{message}");
}

#[tokio::test]
async fn the_etag_changes_with_the_shop_file() {
    let without = common::app().await;
    let with = common::app_with_shop(SHOP_DATASETS, "").await;
    assert!(!without.state.etag_text().contains('+'));
    assert!(
        with.state
            .etag_text()
            .starts_with("W/\"2026.09+2026.10.12-"),
        "{}",
        with.state.etag_text()
    );
    // The data version header still names the core data file alone.
    let response = with.get("/v1/meta").await;
    assert_eq!(common::header(&response, "x-data-version"), "2026.09");
}
