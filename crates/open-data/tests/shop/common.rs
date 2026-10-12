//! A small core data file and a scratch directory for shop builds.

// Shared by every test module; not every module uses every helper.
#![allow(dead_code)]

use std::path::PathBuf;

use open_data::shop::build::ShopBuildInfo;
use rusqlite::Connection;

pub const CORE_DATA_VERSION: &str = "2026.09";

/// Catalog rows copied from the 2026.09 data file: real ids, names,
/// matching forms and year ranges. Only the ids, types and details of
/// `catalog_vehicle` are made up.
///
/// THOR is the one make that is not a light make. Oshkosh has two models
/// with the same matching form. The Silverado LD exists in 2019 only, and
/// the Civic Si has no 2018 model year in vPIC.
pub const CORE_ROWS: &str = "
INSERT INTO catalog_make VALUES
  (448, 'toyota', 'Toyota', 'toyota', 1, 140, 1),
  (449, 'mercedes-benz', 'Mercedes-Benz', 'mercedesbenz', 14, 1196, 1),
  (452, 'bmw', 'BMW', 'bmw', 13, 134, 1),
  (460, 'ford', 'Ford', 'ford', 2, 1196, 1),
  (467, 'chevrolet', 'Chevrolet', 'chevrolet', 3, 1196, 1),
  (472, 'gmc', 'GMC', 'gmc', 10, 1196, 1),
  (473, 'mazda', 'Mazda', 'mazda', 15, 1196, 1),
  (474, 'honda', 'Honda', 'honda', 4, 142, 1),
  (476, 'dodge', 'Dodge', 'dodge', 16, 1196, 1),
  (496, 'ram', 'Ram', 'ram', 9, 1160, 1),
  (517, 'oshkosh', 'Oshkosh', 'oshkosh', NULL, 1096, 1),
  (12503, 'thor', 'THOR', 'thor', NULL, 2, 0);
INSERT INTO catalog_alias VALUES
  ('chevy', 467), ('chev', 467), ('mercedes', 449), ('benz', 449);
INSERT INTO catalog_model VALUES
  (2209, 448, 'prius', 'PRIUS', 'prius', 2001, 2027, 4, 1),
  (13822, 448, 'prius-v', 'Prius V', 'priusv', 2012, 2017, 4, 1),
  (2085, 449, 'c-class', 'C-Class', 'cclass', 1994, 2026, 132, 1),
  (1709, 452, '328i', '328i', '328i', 1996, 2016, 132, 1),
  (1710, 452, 'm3', 'M3', 'm3', 1988, 2027, 4, 1),
  (1711, 452, '335i', '335i', '335i', 2007, 2017, 132, 1),
  (2172, 452, '320i', '320i', '320i', 1981, 2018, 4, 1),
  (2173, 452, '328d', '328d', '328d', 2014, 2018, 4, 1),
  (2777, 452, '330i', '330i', '330i', 2001, 2026, 132, 1),
  (1801, 460, 'f-150', 'F-150', 'f150', 1981, 2027, 1192, 1),
  (1802, 460, 'flex', 'Flex', 'flex', 2009, 2019, 128, 1),
  (7331, 460, 'f-150-heritage', 'F-150 Heritage', 'f150heritage', 2004, 2004, 1032, 1),
  (1850, 467, 'silverado', 'Silverado', 'silverado', 1999, 2027, 1160, 1),
  (1852, 467, 'tahoe', 'Tahoe', 'tahoe', 1995, 2027, 1192, 1),
  (24906, 467, 'silverado-hd', 'Silverado HD', 'silveradohd', 2019, 2027, 1032, 1),
  (25279, 467, 'silverado-ld', 'Silverado LD', 'silveradold', 2019, 2019, 1032, 1),
  (1857, 472, 'sierra', 'Sierra', 'sierra', 1989, 2027, 1192, 1),
  (24909, 472, 'sierra-hd', 'Sierra HD', 'sierrahd', 2019, 2027, 1032, 1),
  (2074, 473, 'mazda3', 'Mazda3', 'mazda3', 2004, 2027, 132, 1),
  (2369, 473, 'cx-5', 'CX-5', 'cx5', 2013, 2027, 128, 1),
  (30046, 473, 'cx-50', 'CX-50', 'cx50', 2023, 2027, 128, 1),
  (1861, 474, 'accord', 'Accord', 'accord', 1981, 2027, 132, 1),
  (1863, 474, 'civic', 'Civic', 'civic', 1981, 2027, 4, 1),
  (1865, 474, 'cr-v', 'CR-V', 'crv', 1997, 2027, 128, 1),
  (29417, 474, 'civic-si', 'CIVIC SI', 'civicsi', 1990, 2027, 4, 1),
  (1938, 476, 'ram', 'Ram', 'ram', 1994, 2012, 1196, 1),
  (13620, 496, '1500', '1500', '1500', 2013, 2027, 1160, 1),
  (2191, 517, 'h-series', 'H Series', 'hseries', 1995, 2012, 8, 1),
  (21696, 517, 'h-series-2', 'H-Series', 'hseries', 2013, 2027, 8, 1);
INSERT INTO catalog_vehicle VALUES
  (1, 2013, 448, 2209, 4, 1, NULL),
  (2, 2016, 449, 2085, 132, 1, NULL),
  (3, 2015, 452, 1709, 132, 1, NULL),
  (4, 2015, 452, 1711, 132, 1, NULL),
  (5, 2015, 452, 2172, 4, 1, NULL),
  (6, 2015, 452, 2173, 4, 1, NULL),
  (7, 2019, 452, 2777, 132, 1, NULL),
  (8, 2018, 460, 1801, 1192, 1, NULL),
  (9, 2019, 460, 1802, 128, 1, NULL),
  (10, 2019, 467, 1850, 1160, 1, NULL),
  (11, 2020, 467, 1850, 1160, 1, NULL),
  (12, 2019, 467, 25279, 1032, 1, NULL),
  (13, 2020, 467, 1852, 1192, 1, NULL),
  (14, 2015, 472, 1857, 1192, 1, NULL),
  (15, 2025, 473, 2369, 128, 1, NULL),
  (16, 2025, 473, 30046, 128, 1, NULL),
  (17, 1985, 474, 1863, 4, 1, NULL),
  (18, 2016, 474, 1863, 4, 1, NULL),
  (19, 2018, 474, 1863, 4, 1, NULL),
  (20, 1990, 474, 29417, 4, 1, NULL),
  (21, 2020, 474, 1861, 132, 1, NULL),
  (22, 2019, 474, 1865, 128, 1, NULL),
  (23, 2005, 476, 1938, 1196, 1, NULL),
  (24, 2019, 496, 13620, 1160, 1, NULL),
  (25, 2010, 517, 2191, 8, 1, NULL),
  (26, 2015, 517, 21696, 8, 1, NULL);
";

/// The curated model names the fixture builds with.
pub const MODELS_YAML: &str = "
models:
  - make: BMW
    model: 3 SERIES
    to: [320i, 328i, 328d, 330i, 335i]
  - make: Mercedes-Benz
    model: C300 4MATIC
    to: [C-Class]
";

/// A temporary directory holding a fixture core file, removed when dropped.
pub struct Scratch {
    directory: PathBuf,
}

impl Scratch {
    pub fn new(name: &str) -> Scratch {
        let directory =
            std::env::temp_dir().join(format!("open-data-shop-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&directory);
        std::fs::create_dir_all(directory.join("curated/shop")).unwrap();
        std::fs::write(directory.join("curated/shop/models.yaml"), MODELS_YAML).unwrap();
        let scratch = Scratch { directory };
        let connection = Connection::open(scratch.core()).unwrap();
        connection
            .execute_batch(wenmar_vin::sqlite::SCHEMA)
            .unwrap();
        connection
            .execute_batch(wenmar_vehicles::schema::SCHEMA)
            .unwrap();
        connection
            .execute_batch(&format!(
                "INSERT INTO meta VALUES
                   ('schema_version', '{}'),
                   ('data_version', '{CORE_DATA_VERSION}'),
                   ('vpic_release', 'vPICList_lite_2026_09'),
                   ('built_at', '2026-10-02 02:30:39');",
                wenmar_vehicles::schema::SCHEMA_VERSION
            ))
            .unwrap();
        connection.execute_batch(CORE_ROWS).unwrap();
        connection.close().unwrap();
        scratch
    }

    pub fn path(&self, name: &str) -> PathBuf {
        self.directory.join(name)
    }

    pub fn core(&self) -> PathBuf {
        self.path("core.sqlite3")
    }

    pub fn out(&self) -> PathBuf {
        self.path("shop.sqlite3")
    }

    pub fn curated(&self) -> PathBuf {
        self.path("curated")
    }
}

impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.directory);
    }
}

pub fn info() -> ShopBuildInfo {
    ShopBuildInfo {
        data_version: "2026.10.12".to_owned(),
        built_at: "2026-10-12 06:00:00".to_owned(),
    }
}

/// Each row of a query as its columns joined by `|`, with NULL as `-`.
pub fn rows(connection: &Connection, sql: &str) -> Vec<String> {
    let mut statement = connection.prepare(sql).unwrap();
    let columns = statement.column_count();
    statement
        .query_map([], |row| {
            let mut cells = Vec::with_capacity(columns);
            for index in 0..columns {
                cells.push(match row.get_ref(index)? {
                    rusqlite::types::ValueRef::Null => "-".to_owned(),
                    rusqlite::types::ValueRef::Integer(number) => number.to_string(),
                    rusqlite::types::ValueRef::Real(number) => number.to_string(),
                    rusqlite::types::ValueRef::Text(text) => {
                        String::from_utf8_lossy(text).into_owned()
                    }
                    rusqlite::types::ValueRef::Blob(_) => "blob".to_owned(),
                });
            }
            Ok(cells.join("|"))
        })
        .unwrap()
        .map(Result::unwrap)
        .collect()
}

/// The committed fixture of a source: `tests/fixtures/shop/<source>`.
pub fn fixture(source: &str) -> PathBuf {
    std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/shop")
        .join(source)
}

/// A fetched `nhtsa-recalls` directory made from the fixture, under
/// `scratch`: the two text files zipped under the names `open-data fetch`
/// saves, with the CR LF line ends NHTSA writes, and `RCL.txt` and
/// `source.json` beside them.
pub fn nhtsa_recalls_dir(scratch: &Scratch) -> PathBuf {
    use std::io::Write;

    let directory = scratch.path("sources/nhtsa-recalls");
    std::fs::create_dir_all(&directory).unwrap();
    for stem in ["FLAT_RCL_PRE_2010", "FLAT_RCL_POST_2010"] {
        let text = std::fs::read_to_string(fixture("nhtsa-recalls").join(format!("{stem}.txt")))
            .unwrap()
            .replace('\n', "\r\n");
        let file = std::fs::File::create(directory.join(format!("{stem}.zip"))).unwrap();
        let mut archive = zip::ZipWriter::new(file);
        archive
            .start_file(
                format!("{stem}.txt"),
                zip::write::SimpleFileOptions::default(),
            )
            .unwrap();
        archive.write_all(text.as_bytes()).unwrap();
        archive.finish().unwrap();
    }
    for name in ["RCL.txt", "source.json"] {
        std::fs::copy(fixture("nhtsa-recalls").join(name), directory.join(name)).unwrap();
    }
    directory
}

/// A fetched `tc-recalls` directory made from the fixture, under `scratch`:
/// the CSV under the name `open-data fetch` saves, and `source.json`.
pub fn tc_recalls_dir(scratch: &Scratch) -> PathBuf {
    let directory = scratch.path("sources/tc-recalls");
    std::fs::create_dir_all(&directory).unwrap();
    for name in ["vrdb_full_monthly.csv", "source.json"] {
        std::fs::copy(fixture("tc-recalls").join(name), directory.join(name)).unwrap();
    }
    directory
}

/// A fetched `nhtsa-investigations` directory made from the fixture, under
/// `scratch`: the text file zipped under the name `open-data fetch` saves,
/// with `INV.txt` and `source.json` beside it. As in NHTSA's file, lines
/// end with LF and the last line has no line end.
pub fn nhtsa_investigations_dir(scratch: &Scratch) -> PathBuf {
    use std::io::Write;

    let directory = scratch.path("sources/nhtsa-investigations");
    std::fs::create_dir_all(&directory).unwrap();
    let text =
        std::fs::read_to_string(fixture("nhtsa-investigations").join("FLAT_INV.txt")).unwrap();
    let file = std::fs::File::create(directory.join("FLAT_INV.zip")).unwrap();
    let mut archive = zip::ZipWriter::new(file);
    archive
        .start_file("FLAT_INV.txt", zip::write::SimpleFileOptions::default())
        .unwrap();
    archive.write_all(text.trim_end().as_bytes()).unwrap();
    archive.finish().unwrap();
    for name in ["INV.txt", "source.json"] {
        std::fs::copy(
            fixture("nhtsa-investigations").join(name),
            directory.join(name),
        )
        .unwrap();
    }
    directory
}

/// A fetched `nhtsa-recall-reports` directory made from the fixture, under
/// `scratch`: the two text files zipped under the names `open-data fetch`
/// saves, with the CR LF line ends NHTSA writes, and the two layout files
/// and `source.json` beside them.
pub fn nhtsa_recall_reports_dir(scratch: &Scratch) -> PathBuf {
    use std::io::Write;

    let directory = scratch.path("sources/nhtsa-recall-reports");
    std::fs::create_dir_all(&directory).unwrap();
    for stem in ["FLAT_RCL_Qrtly_Rpts", "FLAT_RCL_Annual_Rpts"] {
        let text =
            std::fs::read_to_string(fixture("nhtsa-recall-reports").join(format!("{stem}.txt")))
                .unwrap()
                .replace('\n', "\r\n");
        let file = std::fs::File::create(directory.join(format!("{stem}.zip"))).unwrap();
        let mut archive = zip::ZipWriter::new(file);
        archive
            .start_file(
                format!("{stem}.txt"),
                zip::write::SimpleFileOptions::default(),
            )
            .unwrap();
        archive.write_all(text.as_bytes()).unwrap();
        archive.finish().unwrap();
    }
    for name in ["RCL_Qtrly_Rpts.txt", "RCL_Annual_Rpts.txt", "source.json"] {
        std::fs::copy(
            fixture("nhtsa-recall-reports").join(name),
            directory.join(name),
        )
        .unwrap();
    }
    directory
}

/// A fetched `nhtsa-ratings` directory made from the fixture, under
/// `scratch`: the CSV under the name `open-data fetch` saves, and
/// `source.json`.
pub fn nhtsa_ratings_dir(scratch: &Scratch) -> PathBuf {
    let directory = scratch.path("sources/nhtsa-ratings");
    std::fs::create_dir_all(&directory).unwrap();
    for name in ["Safercar_data.csv", "source.json"] {
        std::fs::copy(fixture("nhtsa-ratings").join(name), directory.join(name)).unwrap();
    }
    directory
}
