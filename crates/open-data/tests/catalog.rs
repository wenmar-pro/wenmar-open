use std::path::PathBuf;

use open_data::build::{BuildInfo, Summary, build};
use open_data::catalog::curated::{Curated, Preset, RankedMake};
use open_data::catalog_parity;
use open_data::inspect::{self, Query};
use rusqlite::Connection;
use serde_json::json;
use wenmar_vehicles::sqlite::SqliteSource;
use wenmar_vehicles::{Catalog, Scope};

/// A hand-written dump. `~` stands for a tab.
///
/// - Ford F-150, 2015 to 2020, manufacturer code 1FT (truck). Code 1FD
///   (incomplete vehicle) uses the same schema in 2019. Its engines are in a
///   second schema that applies from 2018, keyed on position 8.
/// - Honda Civic, 2016 onwards, code 2HG (passenger car). The engine is in
///   position 6 and the trim in position 8.
/// - Honda CR-V, 1979 to 1982, code 2HK (MPV), with nothing but a model.
/// - Two trailer makes whose names differ only in punctuation.
fn dump() -> String {
    "\
COPY vpic.element (id, name, lookuptable, isprivate, groupname, decode) FROM stdin;
28~Model~Model~f~General~Pattern
26~Make~Make~f~General~WMI, Pattern
38~Trim~\\N~f~General~Pattern
34~Series~\\N~f~General~Pattern
5~Body Class~BodyStyle~f~Exterior/Body~Pattern
15~Drive Type~DriveType~f~Mechanical/Drivetrain~Pattern
37~Transmission Style~Transmission~f~Mechanical/Transmission~Pattern
13~Displacement (L)~\\N~f~Engine~Pattern
9~Engine Number of Cylinders~\\N~f~Engine~Pattern
24~Fuel Type - Primary~FuelType~f~Engine~Pattern
64~Engine Configuration~EngineConfiguration~f~Engine~Pattern
135~Turbo~Turbo~f~Engine~Pattern
\\.
COPY vpic.bodystyle (id, name) FROM stdin;
60~Pickup
13~Sedan/Saloon
61~Trailer
\\.
COPY vpic.drivetype (id, name) FROM stdin;
2~4WD/4-Wheel Drive/4x4
7~4x2
\\.
COPY vpic.transmission (id, name) FROM stdin;
3~Manual/Standard
\\.
COPY vpic.fueltype (id, name) FROM stdin;
4~Gasoline
\\.
COPY vpic.engineconfiguration (id, name) FROM stdin;
1~In-Line
2~V-Shaped
\\.
COPY vpic.turbo (id, name) FROM stdin;
1~Yes
\\.
COPY vpic.manufacturer (id, name) FROM stdin;
1~FORD MOTOR COMPANY
2~HONDA OF CANADA MFG.
3~B AND B TRAILERS
\\.
COPY vpic.country (id, name) FROM stdin;
1~UNITED STATES (USA)
2~CANADA
\\.
COPY vpic.vehicletype (id, name) FROM stdin;
2~Passenger Car
3~Truck
6~Trailer
7~Multipurpose Passenger Vehicle (MPV)
10~Incomplete Vehicle
\\.
COPY vpic.make (id, name) FROM stdin;
460~FORD
474~Honda
600~B & B Trailers
601~B+B Trailers
\\.
COPY vpic.model (id, name) FROM stdin;
1801~F-150
1863~Civic
1865~CR-V
9001~Utility
9002~Utility
\\.
COPY vpic.make_model (id, makeid, modelid) FROM stdin;
1~460~1801
2~474~1863
3~474~1865
4~600~9001
5~601~9002
\\.
COPY vpic.wmi (id, wmi, manufacturerid, vehicletypeid, countryid, publicavailabilitydate, trucktypeid) FROM stdin;
1~1FT~1~3~1~2015-01-01 00:00:00~\\N
2~2HG~2~2~2~2015-01-01 00:00:00~\\N
3~2HK~2~7~2~2015-01-01 00:00:00~\\N
4~5BB~3~6~1~2015-01-01 00:00:00~\\N
5~1FD~1~10~1~2015-01-01 00:00:00~\\N
\\.
COPY vpic.wmi_make (wmiid, makeid) FROM stdin;
1~460
2~474
3~474
4~600
4~601
5~460
\\.
COPY vpic.vinschema (id, name, tobeqced) FROM stdin;
20~F-150~\\N
21~Civic~\\N
22~CR-V~\\N
23~Trailers~\\N
24~Ford truck engines~\\N
\\.
COPY vpic.wmi_vinschema (id, wmiid, vinschemaid, yearfrom, yearto) FROM stdin;
1~1~20~2015~2020
2~2~21~2016~\\N
3~3~22~1979~1982
4~4~23~2026~2026
5~5~20~2019~2019
6~1~24~2018~2020
\\.
COPY vpic.pattern (id, vinschemaid, keys, elementid, attributeid, createdon, updatedon) FROM stdin;
200~20~*W1[CE]~28~1801~2015-01-01 00:00:00~\\N
201~20~*W1R~28~1801~2015-01-01 00:00:00~\\N
202~20~*W1R~38~Raptor~2015-01-01 00:00:00~\\N
203~20~*W1[CER]~5~60~2015-01-01 00:00:00~\\N
204~20~*W1C~15~7~2015-01-01 00:00:00~\\N
205~20~*W1[ER]~15~2~2015-01-01 00:00:00~\\N
240~24~****5~13~5.0~2018-01-01 00:00:00~\\N
241~24~****5~9~8~2018-01-01 00:00:00~\\N
242~24~****G~13~3.5~2018-01-01 00:00:00~\\N
243~24~****G~9~6~2018-01-01 00:00:00~\\N
244~24~****[5G]~64~2~2018-01-01 00:00:00~\\N
245~24~****G~135~1~2018-01-01 00:00:00~\\N
246~24~****[5G]~24~4~2018-01-01 00:00:00~\\N
210~21~FC1**~28~1863~2016-01-01 00:00:00~\\N
211~21~FC2**~28~1863~2016-01-01 00:00:00~\\N
212~21~FC1*5~38~Si~2016-01-01 00:00:00~\\N
213~21~FC2*5~38~LX~2016-01-01 00:00:00~\\N
214~21~FC2*6~38~lx~2016-01-01 00:00:00~\\N
215~21~FC1*9~38~TOURING~2016-01-01 00:00:00~\\N
216~21~FC1*8~38~Touring~2016-01-01 00:00:00~\\N
217~21~FC2*7~38~EX, EX-L~2016-01-01 00:00:00~\\N
218~21~FC1**~13~1.5~2016-01-01 00:00:00~\\N
219~21~FC2**~13~2.0~2016-01-01 00:00:00~\\N
220~21~FC1**~135~1~2016-01-01 00:00:00~\\N
221~21~FC[12]**~9~4~2016-01-01 00:00:00~\\N
222~21~FC[12]**~64~1~2016-01-01 00:00:00~\\N
223~21~FC1*5~37~3~2016-01-01 00:00:00~\\N
224~21~FC[12]**~5~13~2016-01-01 00:00:00~\\N
225~21~FC[12]**~24~4~2016-01-01 00:00:00~\\N
230~22~RD1**~28~1865~1979-01-01 00:00:00~\\N
231~23~A****~28~9001~2026-01-01 00:00:00~\\N
232~23~B****~28~9002~2026-01-01 00:00:00~\\N
233~23~[AB]****~5~61~2026-01-01 00:00:00~\\N
\\.
"
    .replace('~', "\t")
}

struct Built {
    path: PathBuf,
}

impl Built {
    fn new(name: &str) -> Self {
        let path = std::env::temp_dir().join(format!(
            "open-data-catalog-{name}-{}.sqlite3",
            std::process::id()
        ));
        let _ = std::fs::remove_file(&path);
        Self { path }
    }
}

impl Drop for Built {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.path);
    }
}

/// Built in 2026, so the catalog runs to model year 2027.
fn info() -> BuildInfo {
    BuildInfo {
        data_version: "2026.09".to_owned(),
        vpic_release: "vPICList_lite_2026_09".to_owned(),
        built_at: "2026-09-30 12:00:00".to_owned(),
    }
}

fn curated() -> Curated {
    Curated {
        makes: vec![
            RankedMake {
                name: "Honda".to_owned(),
                aliases: Vec::new(),
            },
            RankedMake {
                name: "Ford".to_owned(),
                aliases: vec!["blueoval".to_owned()],
            },
            RankedMake {
                name: "Chevrolet".to_owned(),
                aliases: vec!["chevy".to_owned()],
            },
        ],
        ..Curated::default()
    }
}

fn built(name: &str, curated: &Curated) -> (Built, Summary, Connection) {
    let built = Built::new(name);
    let summary = build(dump().as_bytes(), &built.path, &info(), curated).unwrap();
    let connection = Connection::open(&built.path).unwrap();
    (built, summary, connection)
}

/// Each row of a query as its columns joined by `|`, with NULL as `-`.
fn rows(connection: &Connection, sql: &str) -> Vec<String> {
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

#[test]
fn both_crates_agree_on_the_schema_version() {
    assert_eq!(
        wenmar_vin::sqlite::SCHEMA_VERSION,
        wenmar_vehicles::schema::SCHEMA_VERSION
    );
    assert_eq!(wenmar_vehicles::schema::SCHEMA_VERSION, "3");
}

#[test]
fn lists_each_model_for_the_years_its_schemas_cover() {
    let (_built, summary, connection) = built("years", &curated());
    assert_eq!(
        rows(
            &connection,
            "SELECT model_id, MIN(year), MAX(year), COUNT(*) FROM catalog_vehicle
             GROUP BY model_id ORDER BY model_id"
        ),
        vec![
            // 2015 to 2020, as linked.
            "1801|2015|2020|6",
            // An open-ended link runs to the year after the build.
            "1863|2016|2027|12",
            // Nothing before 1981.
            "1865|1981|1982|2",
            "9001|2026|2026|1",
            "9002|2026|2026|1",
        ]
    );
    assert_eq!(
        (
            summary.catalog.vehicles,
            summary.catalog.light_vehicles,
            summary.catalog.models,
            summary.catalog.makes
        ),
        (22, 20, 5, 4)
    );
}

#[test]
fn records_every_vehicle_type_a_model_year_is_built_as() {
    let (_built, _summary, connection) = built("types", &curated());
    assert_eq!(
        rows(
            &connection,
            "SELECT year, types, light FROM catalog_vehicle WHERE model_id = 1801 AND year IN (2018, 2019)
             ORDER BY year"
        ),
        // Truck is 1 << 3. In 2019 the incomplete-vehicle code (1 << 10) builds it too.
        vec!["2018|8|1", "2019|1032|1"]
    );
    assert_eq!(
        rows(
            &connection,
            "SELECT types, light FROM catalog_vehicle WHERE model_id = 9001"
        ),
        vec!["64|0"]
    );
    assert_eq!(
        rows(&connection, "SELECT id, name FROM catalog_type ORDER BY id"),
        vec![
            "2|Passenger Car",
            "3|Truck",
            "6|Trailer",
            "7|Multipurpose Passenger Vehicle (MPV)",
            "10|Incomplete Vehicle"
        ]
    );
}

#[test]
fn makes_get_ranks_display_names_aliases_and_distinct_ids() {
    let (_built, summary, connection) = built("makes", &curated());
    assert_eq!(
        rows(
            &connection,
            "SELECT id, slug, name, norm, rank, types, light FROM catalog_make ORDER BY id"
        ),
        vec![
            // vPIC spells it FORD; the curated list's spelling is shown.
            "460|ford|Ford|ford|2|1032|1",
            "474|honda|Honda|honda|1|132|1",
            // The older vPIC id keeps the plain id form.
            "600|b-b-trailers|B & B Trailers|bbtrailers|-|64|0",
            "601|b-b-trailers-2|B+B Trailers|bbtrailers|-|64|0",
        ]
    );
    assert_eq!(
        rows(
            &connection,
            "SELECT norm, make_id FROM catalog_alias ORDER BY norm"
        ),
        vec!["blueoval|460"]
    );
    assert_eq!(summary.catalog.unmatched, vec!["make Chevrolet"]);
}

#[test]
fn models_get_ids_year_ranges_and_types() {
    let (_built, _summary, connection) = built("models", &curated());
    assert_eq!(
        rows(
            &connection,
            "SELECT id, make_id, slug, name, norm, year_from, year_to, types, light
             FROM catalog_model ORDER BY id"
        ),
        vec![
            "1801|460|f-150|F-150|f150|2015|2020|1032|1",
            "1863|474|civic|Civic|civic|2016|2027|4|1",
            "1865|474|cr-v|CR-V|crv|1981|1982|128|1",
            // The same model name under two makes is not a collision.
            "9001|600|utility|Utility|utility|2026|2026|64|0",
            "9002|601|utility|Utility|utility|2026|2026|64|0",
        ]
    );
}

#[test]
fn with_no_curated_lists_nothing_is_ranked() {
    let (_built, summary, connection) = built("unranked", &Curated::default());
    assert_eq!(
        rows(
            &connection,
            "SELECT name, rank FROM catalog_make ORDER BY id"
        ),
        vec!["FORD|-", "Honda|-", "B & B Trailers|-", "B+B Trailers|-"]
    );
    assert!(summary.catalog.unmatched.is_empty());
}

fn preset(make: &str, model: &str, from: u16, submodels: &[&str], engines: &[&str]) -> Preset {
    Preset {
        make: make.to_owned(),
        model: model.to_owned(),
        from,
        to: None,
        submodels: submodels.iter().map(|name| (*name).to_owned()).collect(),
        engines: engines.iter().map(|name| (*name).to_owned()).collect(),
    }
}

#[test]
fn derives_submodels_and_engines() {
    let (_built, summary, connection) = built("details", &Curated::default());

    // Details are numbered in the order model years first need them.
    assert_eq!(
        rows(
            &connection,
            "SELECT id, body, drive, transmission FROM catalog_detail ORDER BY id"
        ),
        vec![
            "1|Pickup|-|-",
            "2|Sedan|-|-",
            "3|Pickup|-|-",
            "4|Trailer|-|-"
        ]
    );
    // The F-150's engines are in a schema that starts in 2018.
    assert_eq!(
        rows(
            &connection,
            "SELECT year, detail_id FROM catalog_vehicle WHERE model_id = 1801 ORDER BY year"
        ),
        vec!["2015|1", "2016|1", "2017|1", "2018|3", "2019|3", "2020|3"]
    );
    // The two trailer models share a detail; the CR-V has none.
    assert_eq!(
        rows(
            &connection,
            "SELECT model_id, COUNT(*), MIN(COALESCE(detail_id, 0)), MAX(COALESCE(detail_id, 0))
             FROM catalog_vehicle WHERE model_id IN (1863, 1865, 9001, 9002)
             GROUP BY model_id ORDER BY model_id"
        ),
        vec!["1863|12|2|2", "1865|2|0|0", "9001|1|4|4", "9002|1|4|4"]
    );
    assert_eq!(
        rows(
            &connection,
            "SELECT detail_id, name, norm, kind, listed, body, drive, transmission
             FROM catalog_submodel ORDER BY id"
        ),
        vec![
            "1|Raptor|raptor|trim|1|Pickup|4WD|-",
            // `EX, EX-L` is two trims; `LX` and `lx` are one; `TOURING` is `Touring`.
            "2|EX|ex|trim|1|Sedan|-|-",
            "2|EX-L|exl|trim|1|Sedan|-|-",
            "2|LX|lx|trim|1|Sedan|-|-",
            "2|Si|si|trim|1|Sedan|-|Manual",
            "2|Touring|touring|trim|1|Sedan|-|-",
            "3|Raptor|raptor|trim|1|Pickup|4WD|-",
        ]
    );
    assert_eq!(
        rows(
            &connection,
            "SELECT detail_id, label, vin8, source FROM catalog_engine ORDER BY id"
        ),
        vec![
            // The Civic's engine is in position 6, so position 8 claims nothing.
            "2|1.5L Turbo|-|vpic",
            "2|2.0L|-|vpic",
            "3|3.5L Turbo V6|G|vpic",
            "3|5.0L V8|5|vpic",
        ]
    );
    // Each Civic trim comes with one of the two engines; the Raptor with both.
    assert_eq!(
        rows(
            &connection,
            "SELECT s.name, e.label FROM catalog_submodel_engine x
             JOIN catalog_submodel s ON s.id = x.submodel_id
             JOIN catalog_engine e ON e.id = x.engine_id
             ORDER BY s.name, e.label"
        ),
        vec![
            "EX|2.0L",
            "EX-L|2.0L",
            "LX|2.0L",
            "Si|1.5L Turbo",
            "Touring|1.5L Turbo",
        ]
    );
    assert_eq!(
        (
            summary.catalog.details,
            summary.catalog.submodels,
            summary.catalog.engines,
            summary.catalog.capped
        ),
        (4, 7, 4, 0)
    );
    // F-150: 3 cells in 2015-2017, 9 with engines, 3 for the 2019 incomplete
    // vehicle. Civic: 12. CR-V: 1. Trailers: 1 each.
    assert_eq!(summary.catalog.cells, 30);
}

#[test]
fn presets_and_overrides_shape_the_details() {
    let mut curated = curated();
    curated
        .submodel_names
        .insert("touring".to_owned(), "Grand Touring".to_owned());
    curated.presets = vec![
        preset("Ford", "F-150", 2019, &["XLT", "RAPTOR"], &["9.9L"]),
        preset("honda", "crv", 1981, &["LX"], &["2.0L"]),
        preset("Ford", "Fiesta", 2012, &["S"], &[]),
    ];
    let (_built, summary, connection) = built("presets", &curated);

    let submodels = |model: i64, year: u16| {
        rows(
            &connection,
            &format!(
                "SELECT s.name, s.kind, s.listed FROM catalog_vehicle v
                 JOIN catalog_submodel s ON s.detail_id = v.detail_id
                 WHERE v.model_id = {model} AND v.year = {year} ORDER BY s.id"
            ),
        )
    };
    let engines = |model: i64, year: u16| {
        rows(
            &connection,
            &format!(
                "SELECT e.label, e.vin8, e.source FROM catalog_vehicle v
                 JOIN catalog_engine e ON e.detail_id = v.detail_id
                 WHERE v.model_id = {model} AND v.year = {year} ORDER BY e.id"
            ),
        )
    };
    // Before the preset starts.
    assert_eq!(submodels(1801, 2018), vec!["Raptor|trim|1"]);
    // The trim takes the preset's spelling; the new name is added.
    assert_eq!(submodels(1801, 2019), vec!["RAPTOR|trim|1", "XLT|preset|1"]);
    // vPIC lists engines, so the preset's is not used.
    assert_eq!(
        engines(1801, 2019),
        vec!["3.5L Turbo V6|G|vpic", "5.0L V8|5|vpic"]
    );
    // vPIC has nothing for the CR-V, so the preset supplies both.
    assert_eq!(submodels(1865, 1981), vec!["LX|preset|1"]);
    assert_eq!(engines(1865, 1981), vec!["2.0L|-|preset"]);
    // The override renames both spellings of Touring.
    assert_eq!(
        rows(
            &connection,
            "SELECT name, norm FROM catalog_submodel WHERE norm LIKE '%touring'"
        ),
        vec!["Grand Touring|grandtouring"]
    );
    assert_eq!(
        summary.catalog.unmatched,
        vec!["make Chevrolet", "preset Ford Fiesta"]
    );
}

fn reader(name: &str) -> (Built, Catalog<SqliteSource>) {
    let (built, _summary, connection) = built(name, &curated());
    drop(connection);
    let catalog = Catalog::new(SqliteSource::open(&built.path).unwrap()).unwrap();
    (built, catalog)
}

#[test]
fn the_built_catalog_answers_through_the_reader() {
    let (_built, catalog) = reader("reader");
    assert_eq!(catalog.year_range(), (1981, 2027));

    let names = |year: u16, scope: Scope| -> Vec<String> {
        catalog
            .makes(Some(year), scope, "", 50)
            .unwrap()
            .into_iter()
            .map(|make| make.name)
            .collect()
    };
    // Ranked makes first, in the curated order.
    assert_eq!(names(2019, Scope::Light), vec!["Honda", "Ford"]);
    assert_eq!(
        names(2026, Scope::All),
        vec!["Honda", "B & B Trailers", "B+B Trailers"]
    );
    assert_eq!(names(2026, Scope::Light), vec!["Honda"]);

    let submodels: Vec<String> = catalog
        .submodels("honda", "civic", 2019, "")
        .unwrap()
        .into_iter()
        .map(|submodel| submodel.name)
        .collect();
    assert_eq!(submodels, vec!["EX", "EX-L", "LX", "Si", "Touring"]);

    let engines = |make: &str, model: &str, submodel: Option<&str>| -> Vec<String> {
        catalog
            .engines(make, model, 2019, submodel, "")
            .unwrap()
            .into_iter()
            .map(|engine| format!("{} @{}", engine.label, engine.vin8.unwrap_or_default()))
            .collect()
    };
    assert_eq!(
        engines("ford", "F-150", None),
        vec!["3.5L Turbo V6 @G", "5.0L V8 @5"]
    );
    assert_eq!(engines("honda", "civic", Some("si")), vec!["1.5L Turbo @"]);

    let entry = catalog
        .entry("2019_honda_civic_si_1-5l-turbo")
        .unwrap()
        .unwrap();
    assert_eq!(
        entry.summary,
        "2019 Honda Civic Si, 1.5L Turbo, Manual, Sedan"
    );
    assert_eq!(entry.vehicle_types, vec!["Passenger Car"]);

    let found: Vec<String> = catalog
        .search("2019 civic si", Scope::Light, 10)
        .unwrap()
        .into_iter()
        .map(|entry| entry.id)
        .collect();
    assert_eq!(found, vec!["2019_honda_civic_si", "2019_honda_civic"]);
    // An alias from the curated list, and the newest year of the model.
    let found = catalog.search("blueoval", Scope::Light, 10).unwrap();
    assert_eq!(found[0].id, "2020_ford_f-150");
}

#[test]
fn the_catalog_command_answers_in_json() {
    let (built, _catalog) = reader("inspect");
    let ask = |query: Query| inspect::run(&built.path, Scope::Light, &query).unwrap();

    assert_eq!(
        ask(Query::Years {
            term: "202".to_owned()
        }),
        json!([2027, 2026, 2025, 2024, 2023, 2022, 2021, 2020])
    );
    assert_eq!(
        ask(Query::Makes {
            year: Some(2019),
            term: String::new(),
            limit: 50
        }),
        json!([
            {"id": "honda", "name": "Honda", "popular": true},
            {"id": "ford", "name": "Ford", "popular": true}
        ])
    );
    assert_eq!(
        ask(Query::Models {
            make: "honda".to_owned(),
            year: Some(2019),
            term: String::new(),
            limit: 50
        }),
        json!([{"id": "civic", "name": "Civic", "year_from": 2016, "year_to": 2027}])
    );
    let found = ask(Query::Search {
        text: vec!["2019".to_owned(), "civic".to_owned(), "si".to_owned()],
        limit: 10,
    });
    assert_eq!(found[0]["id"], "2019_honda_civic_si");
    assert_eq!(
        ask(Query::Entry {
            id: "2019_honda_nothing".to_owned()
        }),
        serde_json::Value::Null
    );

    // A 2019 Civic Si: FC1 is the 1.5L Turbo, and 5 in position 8 the Si.
    let selection = ask(Query::Vin {
        vin: "2HGFC1E50KH000001".to_owned(),
    });
    assert_eq!(selection["vehicle_id"], "2019_honda_civic");
    assert_eq!(selection["submodel_id"], "si");
    assert_eq!(selection["engine_id"], "1-5l-turbo");
    assert_eq!(selection["entry"]["id"], "2019_honda_civic_si_1-5l-turbo");

    // Text that is not a VIN is an error, not a panic.
    let not_a_vin = Query::Vin {
        vin: "nope".to_owned(),
    };
    assert!(inspect::run(&built.path, Scope::Light, &not_a_vin).is_err());
}

#[test]
fn compares_the_catalog_with_recorded_answers() {
    let (_built, catalog) = reader("parity");
    let fixtures: catalog_parity::Fixtures = serde_json::from_value(json!({
        "answers": [
            {"make": "Honda", "year": 2019, "models": ["CIVIC", "Accord"]},
            {"make": "ford", "year": 2019, "models": [" F-150 "]},
            {"make": "Ford", "year": 2016, "models": []},
            {"make": "B & B Trailers", "year": 2026, "models": ["Utility"]},
            {"make": "Nobody", "year": 2019, "models": ["X"]}
        ]
    }))
    .unwrap();
    let report = catalog_parity::run(&catalog, &fixtures).unwrap();
    assert_eq!(
        report,
        catalog_parity::Report {
            pairs: 5,
            agree: 3,
            missing: vec!["2019 Honda: Accord".to_owned(), "2019 Nobody: X".to_owned()],
            extra: vec!["2016 Ford: F-150".to_owned()],
        }
    );
}
