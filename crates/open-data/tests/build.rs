use std::path::PathBuf;

use open_data::build::{BuildInfo, build};
use rusqlite::Connection;
use wenmar_vin::sqlite::SqliteData;
use wenmar_vin::{DecodeOptions, Decoder};

/// A hand-written dump with the tables and columns the build reads.
/// `~` stands for a tab. Element 5 (Body Class) looks its value up in `bodystyle`.
fn dump() -> String {
    "\
COPY vpic.element (id, name, lookuptable, isprivate, groupname, decode) FROM stdin;
28~Model~Model~f~General~Pattern
26~Make~Make~f~General~WMI, Pattern
38~Trim~\\N~f~General~Pattern
5~Body Class~BodyStyle~f~Exterior/Body~Pattern
31~Plant City~\\N~f~General~Pattern
96~NCSA Body Type~\\N~f~Internal~Pattern
91~CAFE Model~\\N~f~\\N~\\N
77~Plant State~\\N~t~General~Pattern
\\.
COPY vpic.bodystyle (id, name) FROM stdin;
7~Sport Utility Vehicle (SUV)/Multi-Purpose Vehicle (MPV)
\\.
COPY vpic.manufacturer (id, name) FROM stdin;
500~HYUNDAI MOTOR CO
\\.
COPY vpic.country (id, name) FROM stdin;
3~SOUTH KOREA
\\.
COPY vpic.vehicletype (id, name) FROM stdin;
7~Multipurpose Passenger Vehicle (MPV)
5~Bus
\\.
COPY vpic.make (id, name) FROM stdin;
498~Hyundai
499~Genesis
\\.
COPY vpic.model (id, name) FROM stdin;
900~Kona
\\.
COPY vpic.make_model (id, makeid, modelid) FROM stdin;
1~498~900
\\.
COPY vpic.wmi (id, wmi, manufacturerid, vehicletypeid, countryid, publicavailabilitydate, trucktypeid) FROM stdin;
1~KM8~500~7~3~2015-01-01 00:00:00~\\N
2~KMH~500~7~3~2015-01-01 00:00:00~\\N
3~1M8~500~5~\\N~2015-01-01 00:00:00~\\N
4~ZZZ~500~7~3~2099-01-01 00:00:00~\\N
5~NUL~500~7~3~\\N~\\N
\\.
COPY vpic.wmi_make (wmiid, makeid) FROM stdin;
1~498
2~498
2~499
\\.
COPY vpic.vinschema (id, name, tobeqced) FROM stdin;
10~Kona 2022~\\N
11~Unreviewed~t
\\.
COPY vpic.wmi_vinschema (id, wmiid, vinschemaid, yearfrom, yearto) FROM stdin;
1~1~10~2022~\\N
2~1~11~2022~\\N
3~4~10~2022~\\N
\\.
COPY vpic.pattern (id, vinschemaid, keys, elementid, attributeid, createdon, updatedon) FROM stdin;
100~10~K2***~28~900~2022-05-01 00:00:00~\\N
101~10~K2ca*~38~SE~2022-05-01 00:00:00~2023-02-02 00:00:00
102~10~K2***~5~7~2022-05-01 00:00:00~\\N
103~10~*****|*U~31~Ulsan~2022-05-01 00:00:00~\\N
104~10~K2***~5~999~2022-05-01 00:00:00~\\N
105~10~K2***~96~Internal~2022-05-01 00:00:00~\\N
106~10~K2***~91~Not Decoded~2022-05-01 00:00:00~\\N
107~10~K2***~77~Private~2022-05-01 00:00:00~\\N
108~10~##***~38~Computed~2022-05-01 00:00:00~\\N
109~11~K2***~38~Unreviewed~2022-05-01 00:00:00~\\N
110~10~K2***~26~498~2022-05-01 00:00:00~\\N
\\.
COPY vpic.wmiyearvalidchars (id, wmi, year, position, \"char\") FROM stdin;
1~KM8~2023~4~K
\\.
"
    .replace('~', "\t")
}

struct Built {
    path: PathBuf,
}

impl Built {
    fn new(name: &str) -> Self {
        let path =
            std::env::temp_dir().join(format!("open-data-{name}-{}.sqlite3", std::process::id()));
        let _ = std::fs::remove_file(&path);
        Self { path }
    }
}

impl Drop for Built {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.path);
    }
}

fn info() -> BuildInfo {
    BuildInfo {
        data_version: "2026.09".to_owned(),
        vpic_release: "vPICList_lite_2026_09".to_owned(),
        built_at: "2026-09-30 12:00:00".to_owned(),
    }
}

fn text(connection: &Connection, sql: &str) -> Vec<String> {
    let mut statement = connection.prepare(sql).unwrap();
    statement
        .query_map([], |row| row.get(0))
        .unwrap()
        .map(Result::unwrap)
        .collect()
}

#[test]
fn builds_a_file_the_decoder_can_use() {
    let built = Built::new("decode");
    let summary = build(dump().as_bytes(), &built.path, &info()).unwrap();
    assert_eq!(
        (
            summary.manufacturers,
            summary.schema_links,
            summary.patterns
        ),
        (3, 1, 4)
    );

    let data = SqliteData::open(&built.path).unwrap();
    assert_eq!(
        data.meta("data_version").unwrap().as_deref(),
        Some("2026.09")
    );
    assert_eq!(
        data.meta("vpic_release").unwrap().as_deref(),
        Some("vPICList_lite_2026_09")
    );

    let options = DecodeOptions {
        model_year: None,
        current_year: Some(2026),
    };
    let decoded = Decoder::new(data)
        .decode("KM8K2CAB4PU001140", options)
        .unwrap();
    assert_eq!(decoded.year, Some(2023));
    assert_eq!(decoded.make.as_deref(), Some("Hyundai"));
    assert_eq!(decoded.model.as_deref(), Some("Kona"));
    assert_eq!(decoded.trim.as_deref(), Some("SE"));
    assert_eq!(
        decoded.body.as_deref(),
        Some("Sport Utility Vehicle (SUV)/Multi-Purpose Vehicle (MPV)")
    );
    assert_eq!(decoded.plant.city.as_deref(), Some("Ulsan"));
    assert_eq!(decoded.manufacturer.name, "HYUNDAI MOTOR CO");
    assert_eq!(decoded.manufacturer.country.as_deref(), Some("SOUTH KOREA"));
    assert!(decoded.warnings.is_empty(), "{:?}", decoded.warnings);
}

#[test]
fn keeps_only_what_nhtsa_would_decode() {
    let built = Built::new("filter");
    build(dump().as_bytes(), &built.path, &info()).unwrap();
    let connection = Connection::open(&built.path).unwrap();

    assert_eq!(
        text(
            &connection,
            "SELECT CAST(id AS TEXT) FROM pattern ORDER BY id"
        ),
        vec!["100", "101", "102", "103"]
    );
    assert_eq!(
        text(&connection, "SELECT keys FROM pattern WHERE id = 101"),
        vec!["K2CA*"]
    );
    assert_eq!(
        text(
            &connection,
            "SELECT changed_on FROM pattern WHERE id IN (100, 101) ORDER BY id"
        ),
        vec!["2022-05-01 00:00:00", "2023-02-02 00:00:00"]
    );
    assert_eq!(
        text(&connection, "SELECT make FROM pattern WHERE id = 100"),
        vec!["Hyundai"]
    );
}

#[test]
fn shapes_manufacturers() {
    let built = Built::new("wmi");
    build(dump().as_bytes(), &built.path, &info()).unwrap();
    let connection = Connection::open(&built.path).unwrap();

    assert_eq!(
        text(&connection, "SELECT code FROM wmi ORDER BY code"),
        vec!["1M8", "KM8", "KMH"]
    );
    assert_eq!(
        text(
            &connection,
            "SELECT COALESCE(make, '(none)') FROM wmi ORDER BY code"
        ),
        vec!["(none)", "Hyundai", "(none)"]
    );
    assert_eq!(
        text(
            &connection,
            "SELECT CAST(light_vehicle AS TEXT) FROM wmi ORDER BY code"
        ),
        vec!["0", "1", "1"]
    );
}

#[test]
fn leaves_no_staging_tables_behind() {
    let built = Built::new("clean");
    build(dump().as_bytes(), &built.path, &info()).unwrap();
    let connection = Connection::open(&built.path).unwrap();
    assert_eq!(
        text(
            &connection,
            "SELECT name FROM sqlite_master WHERE type = 'table' ORDER BY name"
        ),
        vec!["meta", "pattern", "wmi", "wmi_schema"]
    );
}

#[test]
fn a_dump_missing_a_needed_table_is_an_error() {
    let built = Built::new("missing");
    let partial = "COPY vpic.make (id, name) FROM stdin;\n498\tHyundai\n\\.\n";
    let error = build(partial.as_bytes(), &built.path, &info())
        .unwrap_err()
        .to_string();
    assert!(error.contains("missing table"), "{error}");
}

#[test]
fn refuses_to_overwrite_an_existing_file() {
    let built = Built::new("exists");
    std::fs::write(&built.path, b"already here").unwrap();
    let error = build(dump().as_bytes(), &built.path, &info())
        .unwrap_err()
        .to_string();
    assert!(error.contains("already exists"), "{error}");
}

#[test]
fn a_table_with_no_rows_is_staged_and_dropped_like_any_other() {
    let built = Built::new("empty-table");
    let with_empty = format!(
        "{}COPY vpic.decodingoutput (id, name) FROM stdin;\n\\.\n",
        dump()
    );
    let summary = build(with_empty.as_bytes(), &built.path, &info()).unwrap();
    assert_eq!(summary.patterns, 4);
}

#[test]
fn a_code_with_no_public_date_is_left_out() {
    // NHTSA's decoder requires the date to be on or before now, which a
    // missing date never is.
    let built = Built::new("no-date");
    build(dump().as_bytes(), &built.path, &info()).unwrap();
    let connection = Connection::open(&built.path).unwrap();
    assert_eq!(
        text(&connection, "SELECT code FROM wmi WHERE code = 'NUL'"),
        Vec::<String>::new()
    );
}

#[test]
fn a_dump_that_yields_no_patterns_is_an_error() {
    // If NHTSA changes the dump's layout, the build must fail loudly instead
    // of producing a data file that decodes nothing.
    let built = Built::new("thin");
    let full = dump();
    let start = full.find("COPY vpic.pattern ").unwrap();
    let header_end = start + full[start..].find('\n').unwrap() + 1;
    let block_end = header_end + full[header_end..].find("\\.\n").unwrap();
    let without_rows = format!("{}{}", &full[..header_end], &full[block_end..]);
    let error = build(without_rows.as_bytes(), &built.path, &info())
        .unwrap_err()
        .to_string();
    assert!(error.contains("no patterns"), "{error}");
    assert!(!built.path.exists());
}
