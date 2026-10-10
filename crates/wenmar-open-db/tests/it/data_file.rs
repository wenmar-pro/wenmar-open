//! Opening a data file: what is read, what is refused, and that the file
//! is left exactly as it was.

use wenmar_open_db::{Db, DbError};
use wenmar_vehicles::{Scope, Source};

use crate::common;

#[tokio::test]
async fn opens_a_data_file_and_reads_its_meta_and_its_catalog() {
    let fixture = common::data_file();
    let db = Db::open(&fixture.path(), 2).await.unwrap();
    assert_eq!(db.meta().data_version, "2026.09");
    assert_eq!(db.meta().vpic_release, "vPICList_lite_2026_09");
    assert_eq!(db.meta().built_at, "2026-10-01 04:25:57");
    let years = db
        .run(|worker| worker.catalog.years(Scope::Light, "").unwrap())
        .await
        .unwrap();
    assert!(years.contains(&2023), "{years:?}");
}

#[tokio::test]
async fn the_file_is_never_written_and_nothing_is_created_beside_it() {
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
    let first = Db::open(&fixture.path(), 2).await.unwrap();
    // Another engine, as another program on the same machine would be.
    let other = rusqlite::Connection::open_with_flags(
        fixture.path(),
        rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY,
    )
    .unwrap();
    let count: i64 = other
        .query_row("SELECT COUNT(*) FROM wmi", [], |row| row.get(0))
        .unwrap();
    assert!(count > 0);
    // A second handle of this crate on the same file, while the first is
    // open and after it has been used.
    first
        .run(|worker| worker.catalog.years(Scope::Light, "").unwrap())
        .await
        .unwrap();
    let second = Db::open(&fixture.path(), 1).await.unwrap();
    assert_eq!(second.meta().data_version, "2026.09");
    let years = second
        .run(|worker| worker.catalog.years(Scope::Light, "").unwrap())
        .await
        .unwrap();
    assert!(years.contains(&2023));
    assert_eq!(first.meta(), second.meta());
}

#[tokio::test]
async fn a_missing_file_is_refused_and_not_created() {
    let fixture = common::data_file();
    let missing = fixture.directory().join("absent.sqlite3");
    let error = Db::open(&missing, 1).await.unwrap_err();
    assert!(matches!(error, DbError::Missing(_)), "{error}");
    assert!(!missing.exists());
}

#[tokio::test]
async fn a_file_of_another_schema_version_is_refused() {
    let fixture = common::old_data_file();
    let error = Db::open(&fixture.path(), 1).await.unwrap_err();
    assert!(matches!(error, DbError::NotADataFile(_)), "{error}");
    assert!(
        error
            .to_string()
            .starts_with("data file has schema version 2,"),
        "{error}"
    );
}

#[tokio::test]
async fn database_work_does_not_hold_up_the_runtime() {
    use std::sync::Arc;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::time::Duration;

    // `#[tokio::test]` runs on one thread. If `run` did its work on that
    // thread, nothing else could happen until it returned.
    let fixture = common::data_file();
    let db = Db::open(&fixture.path(), 1).await.unwrap();
    let ticks = Arc::new(AtomicUsize::new(0));
    let ticker = {
        let ticks = Arc::clone(&ticks);
        tokio::spawn(async move {
            loop {
                tokio::time::sleep(Duration::from_millis(1)).await;
                ticks.fetch_add(1, Ordering::Relaxed);
            }
        })
    };
    db.run(|_| std::thread::sleep(Duration::from_millis(200)))
        .await
        .unwrap();
    ticker.abort();
    let ticked = ticks.load(Ordering::Relaxed);
    assert!(ticked >= 10, "the runtime ran {ticked} ticks in 200 ms");
}

#[tokio::test]
async fn opening_the_data_file_does_not_hold_up_the_runtime() {
    use std::sync::Arc;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::time::Duration;

    // The same shape as `database_work_does_not_hold_up_the_runtime`, for
    // the work `Db::open` does itself: the meta read and every slot's two
    // connections. If any of it ran on the runtime thread the ticker could
    // not run while the file opened. Enough slots that the opening takes
    // long enough to tell the difference.
    let fixture = common::data_file();
    let ticks = Arc::new(AtomicUsize::new(0));
    let ticker = {
        let ticks = Arc::clone(&ticks);
        tokio::spawn(async move {
            loop {
                tokio::time::sleep(Duration::from_millis(1)).await;
                ticks.fetch_add(1, Ordering::Relaxed);
            }
        })
    };
    Db::open(&fixture.path(), 64).await.unwrap();
    ticker.abort();
    let ticked = ticks.load(Ordering::Relaxed);
    assert!(ticked >= 5, "the runtime ran {ticked} ticks while opening");
}

#[tokio::test]
async fn the_mmap_pragma_never_breaks_reading() {
    // SQLite never errors on mmap_size: it silently falls back. Setting it
    // to -1 after opening asks for no mapping at all — the shape of "this
    // platform will not map it" — and reading must go on exactly as it
    // did, because the pragma may never be the reason a data file fails.
    let fixture = common::data_file();
    let db = Db::open(&fixture.path(), 1).await.unwrap();
    assert!(
        db.run(|worker| worker.source.query("PRAGMA mmap_size = -1", &[]).is_ok())
            .await
            .unwrap(),
        "the pragma must not be able to break reading"
    );
    let years = db
        .run(|worker| worker.catalog.years(Scope::All, "").unwrap())
        .await
        .unwrap();
    assert!(years.contains(&2023), "{years:?}");
}

#[tokio::test]
async fn every_connection_comes_back_to_the_pool() {
    // rusqlite::Connection is Send but not Sync, so the pool is what makes
    // concurrency work at all. This proves liveness, not pool size: far
    // more tasks than slots all finish, because `start` puts every worker
    // back and replaces any that a panic took.
    let fixture = common::data_file();
    let size = 4;
    let db = std::sync::Arc::new(Db::open(&fixture.path(), size).await.unwrap());
    // Far more tasks than connections, all at once, several times over.
    for _ in 0..8 {
        let mut tasks = Vec::new();
        for _ in 0..32 {
            let db = std::sync::Arc::clone(&db);
            tasks.push(tokio::spawn(async move {
                db.run(|worker| worker.catalog.years(Scope::Light, "").unwrap())
                    .await
                    .unwrap()
            }));
        }
        for task in tasks {
            task.await.unwrap();
        }
    }
    // Every task was answered, and slow work still has its reserved half.
    assert_eq!(db.slow_connections(), size / 2);
    let db = std::sync::Arc::try_unwrap(db)
        .map_err(|_| "a handle was left behind")
        .unwrap();
    // After all that, one more query still answers: the connections work.
    let years = db
        .run(|worker| worker.catalog.years(Scope::Light, "").unwrap())
        .await
        .unwrap();
    assert!(years.contains(&2023), "{years:?}");
}
