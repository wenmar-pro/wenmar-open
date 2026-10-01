use rusqlite::Connection;
use wenmar_vehicles::schema::{SCHEMA, SCHEMA_VERSION};
use wenmar_vehicles::sqlite::SqliteSource;
use wenmar_vehicles::{Catalog, Scope};

/// A small catalog. Type bits: passenger car 4, truck 8, trailer 64, MPV
/// 128, incomplete vehicle 1024.
///
/// - Honda Civic 2018 to 2020. 2018 and 2019 have LX, Si and Touring; the
///   Si comes only with the 1.5L Turbo. 2020 has LX only.
/// - Honda CR-V 2019, with nothing below the model.
/// - Ford F-150 2019: one trim, two presets, engines told apart by the
///   eighth VIN character. Ford Ranger 2019.
/// - Chevrolet Silverado 2019: a trim, and series that are searchable but
///   not offered.
/// - Ram 2500 2019 and Pontiac 2000 1983: models that look like numbers.
/// - Ranger Trailers: a trailer maker, outside the default scope.
/// - Checker: a car maker with a model but no model year in 2019.
fn connection() -> Connection {
    let connection = Connection::open_in_memory().unwrap();
    connection
        .execute_batch("CREATE TABLE meta (key TEXT PRIMARY KEY, value TEXT NOT NULL);")
        .unwrap();
    connection.execute_batch(SCHEMA).unwrap();
    connection
        .execute_batch(&format!(
            "INSERT INTO meta VALUES ('schema_version', '{SCHEMA_VERSION}');
             INSERT INTO catalog_type VALUES
               (2, 'Passenger Car'), (3, 'Truck'), (6, 'Trailer'),
               (7, 'Multipurpose Passenger Vehicle (MPV)'), (10, 'Incomplete Vehicle');
             INSERT INTO catalog_make VALUES
               (460, 'ford', 'Ford', 'ford', 2, 1032, 1),
               (467, 'chevrolet', 'Chevrolet', 'chevrolet', 3, 8, 1),
               (474, 'honda', 'Honda', 'honda', 4, 132, 1),
               (480, 'ram', 'Ram', 'ram', 9, 8, 1),
               (490, 'pontiac', 'Pontiac', 'pontiac', 38, 4, 1),
               (5000, 'ranger-trailers', 'Ranger Trailers', 'rangertrailers', NULL, 64, 0),
               (5001, 'checker', 'Checker', 'checker', NULL, 4, 1);
             INSERT INTO catalog_alias VALUES ('chevy', 467), ('chev', 467);
             INSERT INTO catalog_model VALUES
               (1801, 460, 'f-150', 'F-150', 'f150', 2019, 2019, 1032, 1),
               (1802, 460, 'ranger', 'Ranger', 'ranger', 2019, 2019, 8, 1),
               (1850, 467, 'silverado', 'Silverado', 'silverado', 2019, 2019, 8, 1),
               (1863, 474, 'civic', 'Civic', 'civic', 2018, 2020, 4, 1),
               (1865, 474, 'cr-v', 'CR-V', 'crv', 2019, 2019, 128, 1),
               (1900, 480, '2500', '2500', '2500', 2019, 2019, 8, 1),
               (1950, 490, '2000', '2000', '2000', 1983, 1983, 4, 1),
               (9100, 5000, 'tilt-deck', 'Tilt Deck', 'tiltdeck', 2019, 2019, 64, 0),
               (9200, 5001, 'marathon', 'Marathon', 'marathon', 1982, 1982, 4, 1);
             INSERT INTO catalog_vehicle VALUES
               (1, 1982, 5001, 9200, 4, 1, NULL),
               (2, 1983, 490, 1950, 4, 1, NULL),
               (3, 2018, 474, 1863, 4, 1, 1),
               (4, 2019, 460, 1801, 1032, 1, 2),
               (5, 2019, 460, 1802, 8, 1, NULL),
               (6, 2019, 467, 1850, 8, 1, 3),
               (7, 2019, 474, 1863, 4, 1, 1),
               (8, 2019, 474, 1865, 128, 1, NULL),
               (9, 2019, 480, 1900, 8, 1, NULL),
               (10, 2019, 5000, 9100, 64, 0, NULL),
               (11, 2020, 474, 1863, 4, 1, 4);
             INSERT INTO catalog_detail VALUES
               (1, NULL, 'FWD', NULL),
               (2, 'Pickup', NULL, NULL),
               (3, NULL, NULL, NULL),
               (4, 'Sedan', 'FWD', 'CVT');
             INSERT INTO catalog_submodel VALUES
               (1, 1, 'LX', 'lx', 'trim', 1, NULL, NULL, NULL),
               (2, 1, 'Si', 'si', 'trim', 1, 'Sedan', NULL, 'Manual'),
               (3, 1, 'Touring', 'touring', 'trim', 1, NULL, NULL, 'CVT'),
               (4, 2, 'Raptor', 'raptor', 'trim', 1, NULL, '4WD', NULL),
               (5, 2, 'XLT', 'xlt', 'preset', 1, NULL, NULL, NULL),
               (6, 2, 'Lariat', 'lariat', 'preset', 1, NULL, NULL, NULL),
               (7, 3, 'LT', 'lt', 'trim', 1, NULL, NULL, NULL),
               (8, 3, '1500', '1500', 'series', 0, NULL, NULL, NULL),
               (9, 3, '2500', '2500', 'series', 0, NULL, NULL, NULL),
               (10, 4, 'LX', 'lx', 'trim', 1, NULL, NULL, NULL);
             INSERT INTO catalog_engine VALUES
               (1, 1, '1.5L Turbo', NULL, 'vpic'),
               (2, 1, '2.0L', NULL, 'vpic'),
               (3, 2, '3.5L Turbo V6', '4G', 'vpic'),
               (4, 2, '5.0L V8', '5', 'vpic'),
               (5, 3, '5.3L V8', 'CR', 'vpic'),
               (6, 3, '6.2L', 'J', 'vpic'),
               (7, 4, '2.0L', NULL, 'preset');
             INSERT INTO catalog_submodel_engine VALUES (2, 1);"
        ))
        .unwrap();
    connection
}

fn catalog() -> Catalog<SqliteSource> {
    Catalog::new(SqliteSource::from_connection(connection()).unwrap()).unwrap()
}

// ----- opening -----

#[test]
fn refuses_a_file_of_another_schema_version() {
    let connection = connection();
    connection
        .execute(
            "UPDATE meta SET value = '2' WHERE key = 'schema_version'",
            [],
        )
        .unwrap();
    let error = SqliteSource::from_connection(connection)
        .unwrap_err()
        .to_string();
    assert_eq!(
        error,
        "data file has schema version 2, this build reads version 3; rebuild the data file with open-data build"
    );
    let empty = Connection::open_in_memory().unwrap();
    let error = SqliteSource::from_connection(empty)
        .unwrap_err()
        .to_string();
    assert!(error.contains("not a Wenmar Open data file"), "{error}");
}

#[test]
fn an_empty_catalog_answers_with_nothing() {
    let connection = Connection::open_in_memory().unwrap();
    connection
        .execute_batch("CREATE TABLE meta (key TEXT PRIMARY KEY, value TEXT NOT NULL);")
        .unwrap();
    connection.execute_batch(SCHEMA).unwrap();
    connection
        .execute(
            "INSERT INTO meta VALUES ('schema_version', ?1)",
            [SCHEMA_VERSION],
        )
        .unwrap();
    let catalog = Catalog::new(SqliteSource::from_connection(connection).unwrap()).unwrap();
    assert!(catalog.years(Scope::All, "").unwrap().is_empty());
    assert!(catalog.makes(None, Scope::All, "", 50).unwrap().is_empty());
    assert_eq!(catalog.entry("2019_honda_civic").unwrap(), None);
}

// ----- the cascade -----

#[test]
fn years_newest_first() {
    let catalog = catalog();
    assert_eq!(
        catalog.years(Scope::Light, "").unwrap(),
        vec![2020, 2019, 2018, 1983, 1982]
    );
    assert_eq!(
        catalog.years(Scope::Light, "20").unwrap(),
        vec![2020, 2019, 2018]
    );
    assert_eq!(
        catalog.years(Scope::Light, "198").unwrap(),
        vec![1983, 1982]
    );
    assert_eq!(catalog.years(Scope::Type(6), "").unwrap(), vec![2019]);
    assert!(catalog.years(Scope::Light, "%").unwrap().is_empty());
    assert!(catalog.years(Scope::Light, "20x").unwrap().is_empty());
}

fn make_names(
    catalog: &Catalog<SqliteSource>,
    year: Option<u16>,
    scope: Scope,
    term: &str,
) -> Vec<String> {
    catalog
        .makes(year, scope, term, 50)
        .unwrap()
        .into_iter()
        .map(|make| make.name)
        .collect()
}

#[test]
fn makes_popular_first_and_only_those_with_a_model_that_year() {
    let catalog = catalog();
    assert_eq!(
        make_names(&catalog, Some(2019), Scope::Light, ""),
        vec!["Ford", "Chevrolet", "Honda", "Ram"]
    );
    assert_eq!(
        make_names(&catalog, None, Scope::Light, ""),
        vec!["Ford", "Chevrolet", "Honda", "Ram", "Pontiac", "Checker"]
    );
    assert_eq!(
        make_names(&catalog, Some(2019), Scope::All, ""),
        vec!["Ford", "Chevrolet", "Honda", "Ram", "Ranger Trailers"]
    );
    assert_eq!(
        make_names(&catalog, Some(2019), Scope::Type(6), ""),
        vec!["Ranger Trailers"]
    );
    assert_eq!(
        make_names(&catalog, Some(1999), Scope::All, ""),
        Vec::<String>::new()
    );
}

#[test]
fn makes_by_prefix_or_alias() {
    let catalog = catalog();
    assert_eq!(
        make_names(&catalog, None, Scope::Light, "che"),
        vec!["Chevrolet", "Checker"]
    );
    assert_eq!(
        make_names(&catalog, None, Scope::Light, "Chevy"),
        vec!["Chevrolet"]
    );
    // `ra` is Ram, not the trailer maker, and never a make that merely contains it.
    assert_eq!(make_names(&catalog, None, Scope::Light, "ra"), vec!["Ram"]);
    assert_eq!(
        make_names(&catalog, None, Scope::All, "ra"),
        vec!["Ram", "Ranger Trailers"]
    );
    assert_eq!(
        make_names(&catalog, None, Scope::All, "%"),
        Vec::<String>::new()
    );
    let makes = catalog.makes(None, Scope::Light, "", 2).unwrap();
    assert_eq!(makes.len(), 2);
    assert_eq!((makes[0].id.as_str(), makes[0].popular), ("ford", true));
}

fn model_names(
    catalog: &Catalog<SqliteSource>,
    make: &str,
    year: Option<u16>,
    term: &str,
) -> Vec<String> {
    catalog
        .models(make, year, Scope::Light, term, 50)
        .unwrap()
        .into_iter()
        .map(|model| model.name)
        .collect()
}

#[test]
fn models_of_a_make_named_any_way() {
    let catalog = catalog();
    assert_eq!(
        model_names(&catalog, "ford", Some(2019), ""),
        vec!["F-150", "Ranger"]
    );
    assert_eq!(
        model_names(&catalog, "FORD", Some(2019), ""),
        vec!["F-150", "Ranger"]
    );
    assert_eq!(
        model_names(&catalog, "chevy", Some(2019), ""),
        vec!["Silverado"]
    );
    assert_eq!(
        model_names(&catalog, "honda", None, ""),
        vec!["Civic", "CR-V"]
    );
    assert_eq!(
        model_names(&catalog, "honda", Some(2020), ""),
        vec!["Civic"]
    );
    assert_eq!(
        model_names(&catalog, "nobody", Some(2019), ""),
        Vec::<String>::new()
    );
    assert_eq!(
        model_names(&catalog, "", Some(2019), ""),
        Vec::<String>::new()
    );
    // A trailer maker is outside the default scope.
    assert_eq!(
        model_names(&catalog, "ranger-trailers", Some(2019), ""),
        Vec::<String>::new()
    );
    let models = catalog.models("honda", None, Scope::Light, "", 50).unwrap();
    assert_eq!(
        (
            models[0].id.as_str(),
            models[0].year_from,
            models[0].year_to
        ),
        ("civic", 2018, 2020)
    );
}

#[test]
fn models_match_without_punctuation() {
    let catalog = catalog();
    assert_eq!(
        model_names(&catalog, "ford", Some(2019), "f150"),
        vec!["F-150"]
    );
    assert_eq!(
        model_names(&catalog, "ford", Some(2019), "F-1"),
        vec!["F-150"]
    );
    assert_eq!(
        model_names(&catalog, "honda", Some(2019), "crv"),
        vec!["CR-V"]
    );
    assert_eq!(
        model_names(&catalog, "honda", None, "c"),
        vec!["Civic", "CR-V"]
    );
    assert_eq!(
        model_names(&catalog, "honda", Some(2019), "%"),
        Vec::<String>::new()
    );
    assert_eq!(
        model_names(&catalog, "honda", Some(2019), "_"),
        Vec::<String>::new()
    );
}

fn submodel_names(
    catalog: &Catalog<SqliteSource>,
    make: &str,
    model: &str,
    year: u16,
    term: &str,
) -> Vec<String> {
    catalog
        .submodels(make, model, year, term)
        .unwrap()
        .into_iter()
        .map(|submodel| format!("{} ({}, {})", submodel.name, submodel.id, submodel.kind))
        .collect()
}

#[test]
fn submodels_offered_for_a_model_year() {
    let catalog = catalog();
    assert_eq!(
        submodel_names(&catalog, "honda", "civic", 2019, ""),
        vec!["LX (lx, trim)", "Si (si, trim)", "Touring (touring, trim)"]
    );
    assert_eq!(
        submodel_names(&catalog, "Honda", "Civic", 2020, ""),
        vec!["LX (lx, trim)"]
    );
    assert_eq!(
        submodel_names(&catalog, "ford", "F150", 2019, ""),
        vec![
            "Lariat (lariat, preset)",
            "Raptor (raptor, trim)",
            "XLT (xlt, preset)"
        ]
    );
    // Series are kept for search but not offered beside trims.
    assert_eq!(
        submodel_names(&catalog, "chevy", "silverado", 2019, ""),
        vec!["LT (lt, trim)"]
    );
    assert_eq!(
        submodel_names(&catalog, "honda", "civic", 2019, "s"),
        vec!["Si (si, trim)"]
    );
    assert!(submodel_names(&catalog, "honda", "cr-v", 2019, "").is_empty());
    assert!(submodel_names(&catalog, "honda", "civic", 1999, "").is_empty());
    assert!(submodel_names(&catalog, "honda", "prelude", 2019, "").is_empty());
}

fn engine_names(
    catalog: &Catalog<SqliteSource>,
    make: &str,
    model: &str,
    year: u16,
    submodel: Option<&str>,
    term: &str,
) -> Vec<String> {
    catalog
        .engines(make, model, year, submodel, term)
        .unwrap()
        .into_iter()
        .map(|engine| {
            format!(
                "{} ({}, {})",
                engine.label,
                engine.id,
                engine.vin8.as_deref().unwrap_or("-")
            )
        })
        .collect()
}

#[test]
fn engines_narrowed_by_the_submodel() {
    let catalog = catalog();
    assert_eq!(
        engine_names(&catalog, "honda", "civic", 2019, None, ""),
        vec!["1.5L Turbo (1-5l-turbo, -)", "2.0L (2-0l, -)"]
    );
    assert_eq!(
        engine_names(&catalog, "honda", "civic", 2019, Some("si"), ""),
        vec!["1.5L Turbo (1-5l-turbo, -)"]
    );
    assert_eq!(
        engine_names(&catalog, "honda", "civic", 2019, Some("Si"), ""),
        vec!["1.5L Turbo (1-5l-turbo, -)"]
    );
    // The LX does not narrow the list.
    assert_eq!(
        engine_names(&catalog, "honda", "civic", 2019, Some("lx"), "").len(),
        2
    );
    // A submodel the model year does not have.
    assert!(engine_names(&catalog, "honda", "civic", 2019, Some("type-r"), "").is_empty());
}

#[test]
fn engines_carry_the_eighth_vin_character() {
    let catalog = catalog();
    assert_eq!(
        engine_names(&catalog, "ford", "f-150", 2019, None, ""),
        vec!["3.5L Turbo V6 (3-5l-turbo-v6, 4G)", "5.0L V8 (5-0l-v8, 5)"]
    );
    assert_eq!(
        engine_names(&catalog, "ford", "f-150", 2019, None, "5"),
        vec!["5.0L V8 (5-0l-v8, 5)"]
    );
    assert_eq!(
        engine_names(&catalog, "ford", "f-150", 2019, None, "3.5l t"),
        vec!["3.5L Turbo V6 (3-5l-turbo-v6, 4G)"]
    );
    assert!(engine_names(&catalog, "ford", "f-150", 2019, None, "%").is_empty());
    let preset = catalog.engines("honda", "civic", 2020, None, "").unwrap();
    assert!(preset[0].preset);
}

// ----- entries by id -----

fn summary(catalog: &Catalog<SqliteSource>, id: &str) -> Option<String> {
    catalog.entry(id).unwrap().map(|entry| {
        assert_eq!(entry.id, id);
        entry.summary
    })
}

#[test]
fn an_id_names_an_entry_at_each_level() {
    let catalog = catalog();
    assert_eq!(
        summary(&catalog, "2019_honda_civic").as_deref(),
        Some("2019 Honda Civic, FWD")
    );
    assert_eq!(
        summary(&catalog, "2019_honda_civic_si").as_deref(),
        Some("2019 Honda Civic Si, Manual, FWD, Sedan")
    );
    assert_eq!(
        summary(&catalog, "2019_honda_civic_si_1-5l-turbo").as_deref(),
        Some("2019 Honda Civic Si, 1.5L Turbo, Manual, FWD, Sedan")
    );
    assert_eq!(
        summary(&catalog, "2019_ford_f-150__5-0l-v8").as_deref(),
        Some("2019 Ford F-150, 5.0L V8, Pickup")
    );
    assert_eq!(
        summary(&catalog, "2019_ford_f-150_raptor").as_deref(),
        Some("2019 Ford F-150 Raptor, 4WD, Pickup")
    );
    // A series has an id even though the submodel step does not offer it.
    assert_eq!(
        summary(&catalog, "2019_chevrolet_silverado_1500").as_deref(),
        Some("2019 Chevrolet Silverado 1500")
    );
    assert_eq!(
        summary(&catalog, "2019_honda_cr-v").as_deref(),
        Some("2019 Honda CR-V")
    );
    assert_eq!(
        summary(&catalog, "2020_honda_civic").as_deref(),
        Some("2020 Honda Civic, CVT, FWD, Sedan")
    );
}

#[test]
fn an_entry_lists_its_fields_and_vehicle_types() {
    let entry = catalog()
        .entry("2019_ford_f-150_raptor_3-5l-turbo-v6")
        .unwrap()
        .unwrap();
    assert_eq!(entry.year, 2019);
    assert_eq!(
        (entry.make.as_str(), entry.model.as_str()),
        ("Ford", "F-150")
    );
    assert_eq!(entry.submodel.as_deref(), Some("Raptor"));
    assert_eq!(entry.engine.as_deref(), Some("3.5L Turbo V6"));
    assert_eq!(entry.drive.as_deref(), Some("4WD"));
    assert_eq!(entry.body.as_deref(), Some("Pickup"));
    assert_eq!(entry.transmission, None);
    assert_eq!(entry.vehicle_types, vec!["Truck", "Incomplete Vehicle"]);

    let json = serde_json::to_value(&entry).unwrap();
    assert_eq!(json["id"], "2019_ford_f-150_raptor_3-5l-turbo-v6");
    assert_eq!(
        json["summary"],
        "2019 Ford F-150 Raptor, 3.5L Turbo V6, 4WD, Pickup"
    );
    assert!(json.get("transmission").is_none());
}

#[test]
fn what_is_not_in_the_catalog_has_no_entry() {
    let catalog = catalog();
    for id in [
        "",
        "garbage",
        "2019_honda",
        "2017_honda_civic",
        "2019_acura_civic",
        "2019_honda_prelude",
        "2019_honda_civic_type-r",
        "2019_honda_civic_si_6-2l",
        // The engine exists for the Civic, but the Si does not come with it.
        "2019_honda_civic_si_2-0l",
        "2019_Honda_Civic",
        "2019_honda_civic_%",
    ] {
        assert_eq!(catalog.entry(id).unwrap(), None, "{id:?}");
    }
}
