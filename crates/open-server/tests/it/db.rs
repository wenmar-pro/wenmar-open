use open_server::db::{Db, DbError};
use wenmar_vehicles::{Scope, Source};

use crate::common;

#[tokio::test]
async fn opens_a_data_file_and_reads_its_meta() {
    let fixture = common::data_file();
    let db = Db::open(&fixture.path(), 2).await.unwrap();
    assert_eq!(db.meta().data_version, "2026.09");
    assert_eq!(db.meta().vpic_release, "vPICList_lite_2026_09");
    assert_eq!(db.meta().built_at, "2026-10-01 04:25:57");
}

#[tokio::test]
async fn reads_rows_and_the_catalog_on_a_blocking_thread() {
    let fixture = common::data_file();
    let db = Db::open(&fixture.path(), 2).await.unwrap();
    let (rows, makes) = db
        .run(|worker| {
            let rows = worker
                .source
                .query(
                    "SELECT code, light_vehicle FROM wmi WHERE code IN (?1, ?2) ORDER BY code",
                    &["KM8".into(), "1M8".into()],
                )
                .unwrap();
            let makes = worker.catalog.makes(None, Scope::Light, "", 10).unwrap();
            (rows, makes)
        })
        .await
        .unwrap();
    assert_eq!(rows.len(), 2);
    assert_eq!(rows[0][0].text(), Some("1M8"));
    assert_eq!(rows[1][1].integer(), Some(1));
    assert_eq!(makes[0].name, "Ford");
}

#[tokio::test]
async fn leaves_the_data_file_and_its_directory_untouched() {
    let fixture = common::data_file();
    let before = std::fs::read(fixture.path()).unwrap();
    {
        let db = Db::open(&fixture.path(), 2).await.unwrap();
        db.run(|worker| worker.catalog.years(Scope::All, "").unwrap())
            .await
            .unwrap();
        let refused = db
            .run(|worker| {
                worker
                    .source
                    .query("INSERT INTO meta VALUES ('x', 'y')", &[])
            })
            .await
            .unwrap();
        assert!(refused.is_err(), "a write must be refused");
        // No journal, WAL or lock file while the file is open.
        assert_eq!(common::files(&fixture), ["data.sqlite3"]);
    }
    assert_eq!(common::files(&fixture), ["data.sqlite3"]);
    assert_eq!(std::fs::read(fixture.path()).unwrap(), before);
}

#[tokio::test]
async fn a_second_reader_is_not_locked_out() {
    let fixture = common::data_file();
    let _db = Db::open(&fixture.path(), 2).await.unwrap();
    let other = rusqlite::Connection::open_with_flags(
        fixture.path(),
        rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY,
    )
    .unwrap();
    let count: i64 = other
        .query_row("SELECT COUNT(*) FROM wmi", [], |row| row.get(0))
        .unwrap();
    assert_eq!(count, 4);
    let again = Db::open(&fixture.path(), 1).await.unwrap();
    assert_eq!(again.meta().data_version, "2026.09");
}

#[tokio::test]
async fn more_requests_than_connections_all_finish() {
    let fixture = common::data_file();
    let db = std::sync::Arc::new(Db::open(&fixture.path(), 2).await.unwrap());
    let mut tasks = Vec::new();
    for _ in 0..16 {
        let db = std::sync::Arc::clone(&db);
        tasks.push(tokio::spawn(async move {
            db.run(|worker| worker.catalog.years(Scope::Light, "").unwrap().len())
                .await
                .unwrap()
        }));
    }
    for task in tasks {
        assert_eq!(task.await.unwrap(), 5);
    }
}

#[tokio::test]
async fn a_panic_in_one_task_does_not_break_the_next() {
    let fixture = common::data_file();
    let db = Db::open(&fixture.path(), 1).await.unwrap();
    let failed = db.run(|_| panic!("a bug in a handler")).await;
    assert!(matches!(failed, Err::<(), _>(DbError::Stopped)));
    let years = db
        .run(|worker| worker.catalog.years(Scope::Light, "").unwrap())
        .await
        .unwrap();
    assert_eq!(years.first(), Some(&2023));
}

#[tokio::test]
async fn refuses_a_missing_file_without_creating_it() {
    let fixture = common::data_file();
    let missing = fixture.directory().join("absent.sqlite3");
    let error = Db::open(&missing, 1).await.unwrap_err();
    assert!(matches!(error, DbError::Missing(_)), "{error}");
    assert!(!missing.exists());
}

#[tokio::test]
async fn refuses_a_file_without_a_meta_table() {
    let fixture = common::other_sqlite_file();
    let error = Db::open(&fixture.path(), 1).await.unwrap_err();
    assert_eq!(
        error.to_string(),
        "not a Wenmar Open data file: it has no meta table"
    );
}

#[tokio::test]
async fn refuses_a_file_of_another_schema_version() {
    let fixture = common::old_data_file();
    let error = Db::open(&fixture.path(), 1).await.unwrap_err();
    assert_eq!(
        error.to_string(),
        "data file has schema version 2, this build reads version 3; rebuild the data file with open-data build"
    );
}

#[tokio::test]
async fn refuses_a_file_that_is_not_a_database() {
    let fixture = common::data_file();
    let path = fixture.directory().join("notes.txt");
    std::fs::write(&path, "these are not the rows you are looking for").unwrap();
    assert!(Db::open(&path, 1).await.is_err());
}
