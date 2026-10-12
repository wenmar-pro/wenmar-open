use open_data::shop::build::{ShopInputs, build_shop, build_with, is_data_version, open_catalog};
use rusqlite::Connection;
use wenmar_shopdata::sqlite::ShopSource;
use wenmar_shopdata::{Dataset, DatasetEntry, ShopData, licence};

use crate::common::{CORE_DATA_VERSION, Scratch, info, rows};

fn entry(rows: u64) -> DatasetEntry {
    DatasetEntry {
        as_of: "2026-10-11".to_owned(),
        rows,
        licence: licence::PUBLIC_DOMAIN.to_owned(),
        attribution: String::new(),
    }
}

#[test]
fn builds_a_shop_file_with_meta_and_no_datasets() {
    let scratch = Scratch::new("empty");
    let before = std::fs::read(scratch.core()).unwrap();
    let summary = build_shop(
        &scratch.core(),
        &scratch.out(),
        &scratch.curated(),
        &info(),
        &ShopInputs::default(),
    )
    .unwrap();
    assert_eq!(summary.data_version, "2026.10.12");
    assert_eq!(summary.catalog_data_version, CORE_DATA_VERSION);
    assert!(summary.datasets.is_empty());

    let connection = Connection::open(scratch.out()).unwrap();
    assert_eq!(
        rows(&connection, "SELECT key, value FROM meta ORDER BY key"),
        [
            "built_at|2026-10-12 06:00:00",
            "catalog_data_version|2026.09",
            "data_version|2026.10.12",
            "datasets|{}",
            "schema_version|1",
        ]
    );
    assert_eq!(
        rows(&connection, "SELECT name FROM sqlite_master ORDER BY name"),
        ["meta", "sqlite_autoindex_meta_1"]
    );
    drop(connection);

    // The reader accepts it, and pairs it with the core file it came from.
    let shop = ShopData::new(ShopSource::open(scratch.out()).unwrap()).unwrap();
    shop.check_catalog(CORE_DATA_VERSION).unwrap();
    assert!(shop.require(Dataset::RecallsUs).is_err());

    // The core file was only read.
    assert_eq!(std::fs::read(scratch.core()).unwrap(), before);
    let mut left: Vec<String> = std::fs::read_dir(scratch.path(""))
        .unwrap()
        .map(|entry| entry.unwrap().file_name().to_string_lossy().into_owned())
        .collect();
    left.sort();
    assert_eq!(left, ["core.sqlite3", "curated", "shop.sqlite3"]);
}

#[test]
fn a_shape_step_adds_its_table_and_its_datasets_entry() {
    let scratch = Scratch::new("step");
    let summary = build_with(
        &scratch.core(),
        &scratch.out(),
        &scratch.curated(),
        &info(),
        |context| {
            // A step may read the catalog it is matching against.
            let makes: i64 =
                context
                    .catalog
                    .query_row("SELECT COUNT(*) FROM catalog_make", [], |row| row.get(0))?;
            assert_eq!(makes, 12);
            assert_eq!(context.info.data_version, "2026.10.12");
            // A step matches the source's names through the context.
            let resolved = context
                .matcher
                .resolve(Some(2019), "CHEVROLET", "SILVERADO 1500");
            assert_eq!(resolved.model_ids, [1850]);
            assert_eq!(resolved.match_text(), Some("base"));
            context.transaction.execute_batch(
                "CREATE TABLE thing (name TEXT); INSERT INTO thing VALUES ('a'), ('b');",
            )?;
            context.register(Dataset::RecallsUs, entry(2))?;
            assert!(context.datasets().contains(Dataset::RecallsUs));
            Ok(())
        },
    )
    .unwrap();
    assert_eq!(summary.datasets.len(), 1);
    let shop = ShopData::new(ShopSource::open(scratch.out()).unwrap()).unwrap();
    shop.require(Dataset::RecallsUs).unwrap();
    assert_eq!(shop.datasets().get(Dataset::RecallsUs).unwrap().rows, 2);
    assert_eq!(shop.query("SELECT name FROM thing", &[]).unwrap().len(), 2);
}

#[test]
fn register_refuses_an_entry_that_is_wrong() {
    let scratch = Scratch::new("register");
    for (change, expected) in [
        (
            (|entry: &mut DatasetEntry| entry.licence = "cc0".to_owned()) as fn(&mut DatasetEntry),
            "recalls_us: cc0 is not a licence; use one of public-domain, ogl-canada, cc-by-4.0, mit",
        ),
        (
            |entry: &mut DatasetEntry| entry.as_of = "11 Oct 2026".to_owned(),
            "recalls_us: as_of must be a date like 2026-10-11, not \"11 Oct 2026\"",
        ),
    ] {
        let error = build_with(
            &scratch.core(),
            &scratch.out(),
            &scratch.curated(),
            &info(),
            |context| {
                let mut wrong = entry(1);
                change(&mut wrong);
                context.register(Dataset::RecallsUs, wrong)
            },
        )
        .unwrap_err();
        assert_eq!(format!("{error:#}"), expected);
        assert!(!scratch.out().exists(), "a failed build left a file");
    }
    let error = build_with(
        &scratch.core(),
        &scratch.out(),
        &scratch.curated(),
        &info(),
        |context| {
            context.register(Dataset::Codes, entry(1))?;
            context.register(Dataset::Codes, entry(1))
        },
    )
    .unwrap_err();
    assert_eq!(format!("{error:#}"), "codes was registered twice");
}

#[test]
fn refuses_what_it_cannot_build_from_and_leaves_no_file() {
    let scratch = Scratch::new("refuse");
    let build = |catalog: &std::path::Path, version: &str| {
        let mut info = info();
        info.data_version = version.to_owned();
        build_shop(
            catalog,
            &scratch.out(),
            &scratch.curated(),
            &info,
            &ShopInputs::default(),
        )
        .map(|_| ())
        .map_err(|error| format!("{error:#}"))
    };
    let missing = scratch.path("absent.sqlite3");
    assert_eq!(
        build(&missing, "2026.10.12").unwrap_err(),
        format!("the data file {} does not exist", missing.display())
    );
    assert!(!missing.exists());
    for version in ["2026.10", "2026-10-12", "2026.10.12.1", ""] {
        assert_eq!(
            build(&scratch.core(), version).unwrap_err(),
            format!("{version} is not a shop data version like 2026.10.12")
        );
    }
    // A SQLite file that is not a data file.
    let other = scratch.path("other.sqlite3");
    Connection::open(&other)
        .unwrap()
        .execute_batch("CREATE TABLE notes (body TEXT);")
        .unwrap();
    assert!(
        build(&other, "2026.10.12")
            .unwrap_err()
            .contains("is not a Wenmar Open data file")
    );
    // A core file of another schema version.
    let old = scratch.path("old.sqlite3");
    std::fs::copy(scratch.core(), &old).unwrap();
    Connection::open(&old)
        .unwrap()
        .execute(
            "UPDATE meta SET value = '2' WHERE key = 'schema_version'",
            [],
        )
        .unwrap();
    assert!(
        build(&old, "2026.10.12")
            .unwrap_err()
            .ends_with("has schema version 2, this build reads version 3")
    );
    assert!(!scratch.out().exists());

    // An output that exists is never overwritten.
    std::fs::write(scratch.out(), b"keep").unwrap();
    assert!(
        build(&scratch.core(), "2026.10.12")
            .unwrap_err()
            .contains("already exists")
    );
    assert_eq!(std::fs::read(scratch.out()).unwrap(), b"keep");
}

#[test]
fn reads_the_core_files_data_version() {
    let scratch = Scratch::new("catalog");
    let (connection, version) = open_catalog(&scratch.core()).unwrap();
    assert_eq!(version, CORE_DATA_VERSION);
    assert!(
        connection.execute("DELETE FROM catalog_make", []).is_err(),
        "the core file must be read-only"
    );
    assert!(is_data_version("2026.10.12"));
    assert!(!is_data_version("2026.09"));
}

#[test]
fn a_build_without_the_curated_models_is_refused() {
    let scratch = Scratch::new("no-curated");
    let error = build_shop(
        &scratch.core(),
        &scratch.out(),
        &scratch.path("nowhere"),
        &info(),
        &ShopInputs::default(),
    )
    .unwrap_err();
    assert!(
        format!("{error:#}").contains("shop/models.yaml"),
        "{error:#}"
    );
    assert!(!scratch.out().exists());
}
