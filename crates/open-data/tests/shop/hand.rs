use open_data::shop::build::build_with;
use open_data::shop::hand::{
    self, CAPACITY, CHARGE, PRESSURE, TORQUE, both, parse_fluids, parse_tires, scaled, thousandths,
};
use rusqlite::Connection;
use wenmar_shopdata::Dataset;

use crate::common::{Scratch, fixture, info, rows};

/// A make, a model and three model years that do not exist. 2001 has two
/// engines, 2002 one, and 2003 none.
const TESTMAKE_ROWS: &str = "
INSERT INTO catalog_make VALUES (7000, 'testmake', 'Testmake', 'testmake', NULL, 4, 1);
INSERT INTO catalog_model VALUES (7001, 7000, 'sampler', 'Sampler', 'sampler', 2001, 2003, 4, 1);
INSERT INTO catalog_detail VALUES (70, NULL, NULL, NULL), (71, NULL, NULL, NULL);
INSERT INTO catalog_vehicle VALUES
  (7001, 2001, 7000, 7001, 4, 1, 70),
  (7002, 2002, 7000, 7001, 4, 1, 71),
  (7003, 2003, 7000, 7001, 4, 1, NULL);
INSERT INTO catalog_engine VALUES
  (70001, 70, '9.9L Test', NULL, 'vpic'),
  (70002, 70, '8.8L Test Diesel', NULL, 'vpic'),
  (70003, 71, '9.9L Test', NULL, 'vpic');
";

/// A scratch build directory whose catalog has the vehicle and whose
/// curated directory has the two fixture files.
fn scratch(name: &str) -> Scratch {
    let scratch = Scratch::new(name);
    let connection = Connection::open(scratch.core()).unwrap();
    connection.execute_batch(TESTMAKE_ROWS).unwrap();
    for kind in ["fluids", "tires"] {
        let directory = scratch.curated().join(kind);
        std::fs::create_dir_all(&directory).unwrap();
        std::fs::copy(
            fixture("hand").join(kind).join("testmake_sampler.yaml"),
            directory.join("testmake_sampler.yaml"),
        )
        .unwrap();
    }
    scratch
}

fn fixture_text(kind: &str) -> String {
    std::fs::read_to_string(fixture("hand").join(kind).join("testmake_sampler.yaml")).unwrap()
}

#[test]
fn a_figure_is_read_and_worked_out_in_whole_numbers() {
    assert_eq!(thousandths("5.7"), Some(5700));
    assert_eq!(thousandths("240"), Some(240_000));
    assert_eq!(thousandths("0.125"), Some(125));
    for bad in [
        "", "0", "0.0", "5.", ".5", "5,7", "-5", "5.7 L", "1.2345", "1e3", " 5",
    ] {
        assert_eq!(thousandths(bad), None, "{bad:?}");
    }
    // 1 of 3, to no, one, two and three places, half up.
    assert_eq!(scaled(1000, 1, 3, 0), 0);
    assert_eq!(scaled(1000, 1, 3, 1), 300);
    assert_eq!(scaled(1000, 1, 3, 2), 330);
    assert_eq!(scaled(1000, 1, 3, 3), 333);
    assert_eq!(scaled(1000, 1, 2, 0), 1000);
    assert_eq!(scaled(2500, 1, 1, 0), 3000);

    let litres = both(&CAPACITY, "99.9", "l").unwrap();
    assert_eq!((litres.metric_real(), litres.us_real()), (99.9, 105.6));
    let quarts = both(&CAPACITY, "88", "qt").unwrap();
    assert_eq!((quarts.metric_real(), quarts.us_real()), (83.3, 88.0));
    let kilograms = both(&CHARGE, "9.99", "kg").unwrap();
    assert_eq!((kilograms.metric, kilograms.us_real()), (9_990_000, 352.4));
    let pounds = both(&CHARGE, "99", "lb").unwrap();
    assert_eq!((pounds.metric, pounds.us), (44_906_000, 1_584_000));
    let bar = both(&PRESSURE, "9.9", "bar").unwrap();
    assert_eq!((bar.metric, bar.us), (990_000, 144_000));
    let psi = both(&PRESSURE, "144", "psi").unwrap();
    assert_eq!((psi.metric, psi.us), (993_000, 144_000));
    let torque = both(&TORQUE, "999", "nm").unwrap();
    assert_eq!((torque.metric, torque.us), (999_000, 737_000));
    let torque = both(&TORQUE, "737", "lbft").unwrap();
    assert_eq!((torque.metric, torque.us), (999_000, 737_000));

    let error = both(&CAPACITY, "5.7", "gal").unwrap_err().to_string();
    assert_eq!(error, "\"gal\" is not a unit of capacity; use one of l, qt");
    let error = both(&PRESSURE, "thirty", "psi").unwrap_err().to_string();
    assert!(
        error.starts_with("pressure \"thirty\" is not a figure"),
        "{error}"
    );
}

#[test]
fn the_fixture_files_build_into_both_tables() {
    let scratch = scratch("hand");
    let summary = build_with(
        &scratch.core(),
        &scratch.out(),
        &scratch.curated(),
        &info(),
        hand::shape,
    )
    .unwrap();
    let fluids = summary.datasets.get(Dataset::Fluids).unwrap();
    assert_eq!((fluids.rows, fluids.as_of.as_str()), (11, "2026-10-12"));
    assert_eq!(
        (fluids.licence.as_str(), fluids.attribution.as_str()),
        ("mit", "")
    );
    let tires = summary.datasets.get(Dataset::Tires).unwrap();
    assert_eq!((tires.rows, tires.as_of.as_str()), (3, "2026-10-12"));
    assert_eq!(tires.licence, "mit");

    let shop = Connection::open(scratch.out()).unwrap();
    assert_eq!(
        rows(
            &shop,
            "SELECT year, kind, engine_id, applies_to, grade, specification, capacity_l, capacity_qt,
                    capacity_unit, with_filter, charge_g, charge_oz, charge_unit, market, source,
                    checked, reviewer
             FROM fluid WHERE make_id = 7000 AND model_id = 7001 ORDER BY id"
        ),
        [
            "2001|engine_oil|70001|-|0W-99|TEST-SPEC-1|99.9|105.6|l|1|-|-|-|-|Test manual, page 1|2026-10-12|test-reviewer",
            "2002|engine_oil|70003|-|0W-99|TEST-SPEC-1|99.9|105.6|l|1|-|-|-|-|Test manual, page 1|2026-10-12|test-reviewer",
            "2001|engine_oil|70002|-|-|TEST-SPEC-2|83.3|88|qt|0|-|-|-|ca|Test manual, page 2|2026-10-11|test-reviewer",
            "2001|transmission|-|Test automatic|-|TEST ATF 9|-|-|-|-|-|-|-|-|Test manual, page 3|2026-10-10|test-reviewer",
            "2002|transmission|-|Test automatic|-|TEST ATF 9|-|-|-|-|-|-|-|-|Test manual, page 3|2026-10-10|test-reviewer",
            "2003|transmission|-|Test automatic|-|TEST ATF 9|-|-|-|-|-|-|-|-|Test manual, page 3|2026-10-10|test-reviewer",
            "2002|coolant|-|-|-|TEST COOLANT 9|-|-|-|-|-|-|-|-|Test manual, page 4|2026-10-10|test-reviewer",
            "2001|refrigerant|-|-|-|R-TEST|-|-|-|-|9990|352.4|kg|-|Test under-hood label|2026-10-10|test-reviewer",
            "2001|brake|-|-|-|TEST BRAKE 9|-|-|-|-|-|-|-|-|Test manual, page 5|2026-10-10|test-reviewer",
            "2002|brake|-|-|-|TEST BRAKE 9|-|-|-|-|-|-|-|-|Test manual, page 5|2026-10-10|test-reviewer",
            "2003|brake|-|-|-|TEST BRAKE 9|-|-|-|-|-|-|-|-|Test manual, page 5|2026-10-10|test-reviewer",
        ]
    );
    assert_eq!(
        rows(
            &shop,
            "SELECT year, size, size_rear, trims, pressure_front_kpa, pressure_rear_kpa,
                    pressure_front_psi, pressure_rear_psi, pressure_unit, lug_torque_nm,
                    lug_torque_lbft, lug_torque_unit, bolt_pattern, market, source, checked, reviewer
             FROM tire_fitment WHERE make_id = 7000 AND model_id = 7001 ORDER BY id"
        ),
        [
            "2001|999/99R99|-|[\"Base\",\"Deluxe\"]|990|980|144|142|kpa|999|737|nm|9x999|-|Test door placard|2026-10-12|test-reviewer",
            "2002|999/99R99|-|[\"Base\",\"Deluxe\"]|990|980|144|142|kpa|999|737|nm|9x999|-|Test door placard|2026-10-12|test-reviewer",
            "2001|998/98R98|997/97R97|[]|990|980|144|142|bar|999|737|lbft|-|us|Test manual, page 6|2026-10-11|test-reviewer",
        ]
    );
}

#[test]
fn a_build_with_no_records_holds_neither_dataset() {
    // No directory at all.
    let scratch = Scratch::new("hand-none");
    let summary = build_with(
        &scratch.core(),
        &scratch.out(),
        &scratch.curated(),
        &info(),
        hand::shape,
    )
    .unwrap();
    assert!(summary.datasets.is_empty());
    // A directory with an example and a file with no record.
    let scratch = Scratch::new("hand-empty");
    let directory = scratch.curated().join("fluids");
    std::fs::create_dir_all(&directory).unwrap();
    std::fs::write(directory.join("_example.yaml"), "not: [read").unwrap();
    std::fs::write(directory.join("README.md"), "# not YAML").unwrap();
    std::fs::write(
        directory.join("ford_f-150.yaml"),
        "make: ford\nmodel: f-150\n",
    )
    .unwrap();
    let summary = build_with(
        &scratch.core(),
        &scratch.out(),
        &scratch.curated(),
        &info(),
        hand::shape,
    )
    .unwrap();
    assert!(summary.datasets.is_empty());
    let shop = Connection::open(scratch.out()).unwrap();
    assert_eq!(
        rows(&shop, "SELECT name FROM sqlite_master WHERE type = 'table'"),
        ["meta"]
    );
}

/// The fixture with one line replaced, and what the build must say.
fn refused(kind: &str, from: &str, to: &str) -> String {
    let text = fixture_text(kind);
    assert!(text.contains(from), "the fixture has no {from:?}");
    let changed = text.replacen(from, to, 1);
    let error = match kind {
        "fluids" => parse_fluids("testmake_sampler", &changed).map(|_| ()),
        _ => parse_tires("testmake_sampler", &changed).map(|_| ()),
    }
    .unwrap_err();
    format!("{error:#}")
}

#[test]
fn a_malformed_record_is_refused_with_its_file_and_its_number() {
    for (kind, from, to, expected) in [
        (
            "fluids",
            "    source: \"Test manual, page 1\"\n",
            "",
            "missing field `source`",
        ),
        (
            "fluids",
            "source: \"Test manual, page 1\"",
            "source: \"  \"",
            "fluids/testmake_sampler.yaml, record 1: source is empty",
        ),
        (
            "fluids",
            "    checked: 2026-10-12\n",
            "",
            "missing field `checked`",
        ),
        (
            "fluids",
            "checked: 2026-10-12",
            "checked: 12 October 2026",
            "record 1: checked \"12 October 2026\" is not a day like 2026-10-12",
        ),
        (
            "fluids",
            "checked: 2026-10-12",
            "checked: 2026-13-01",
            "is not a day like 2026-10-12",
        ),
        (
            "fluids",
            "    reviewer: test-reviewer\n",
            "",
            "missing field `reviewer`",
        ),
        (
            "fluids",
            "reviewer: test-reviewer",
            "reviewer: \"Test Reviewer\"",
            "record 1: reviewer \"Test Reviewer\" is not a GitHub user name",
        ),
        (
            "fluids",
            "capacity_unit: l",
            "capacity_unit: gal",
            "record 1: \"gal\" is not a unit of capacity; use one of l, qt",
        ),
        (
            "fluids",
            "capacity: \"99.9\"",
            "capacity: \"about 100\"",
            "record 1: capacity \"about 100\" is not a figure",
        ),
        (
            "fluids",
            "    capacity_unit: l\n",
            "",
            "record 1: capacity and capacity_unit go together",
        ),
        (
            "fluids",
            "    with_filter: true\n",
            "",
            "record 1: capacity and with_filter go together",
        ),
        (
            "fluids",
            "    engine: 9-9l-test\n",
            "",
            "record 1: an engine_oil record must name its engine",
        ),
        (
            "fluids",
            "engine: 9-9l-test",
            "engine: \"9.9L Test\"",
            "record 1: engine \"9.9L Test\" must be an engine's id form",
        ),
        (
            "fluids",
            "kind: coolant",
            "kind: washer",
            "record 4: kind \"washer\" is not one of engine_oil, transmission, coolant, refrigerant, brake",
        ),
        (
            "fluids",
            "    specification: \"TEST COOLANT 9\"\n",
            "",
            "record 4: a coolant record needs a specification",
        ),
        (
            "fluids",
            "charge_unit: kg",
            "charge_unit: ml",
            "record 5: \"ml\" is not a unit of charge; use one of g, kg, oz, lb",
        ),
        (
            "fluids",
            "charge: \"9.99\"\n    charge_unit: kg",
            "charge: \"9.5\"\n    charge_unit: g",
            "record 5: a charge in grams must be a whole number",
        ),
        (
            "fluids",
            "kind: brake",
            "kind: brake\n    grade: \"0W-99\"",
            "record 6: grade, capacity and with_filter belong to an engine_oil record only",
        ),
        (
            "fluids",
            "year_to: 2002",
            "year_to: 2000",
            "record 1: year_from 2001 and year_to 2000 are not a range of model years",
        ),
        (
            "fluids",
            "market: ca",
            "market: mx",
            "record 2: market \"mx\" must be us or ca, or left out",
        ),
        (
            "fluids",
            "kind: brake",
            "kind: brake\n    notes: \"x\"",
            "unknown field `notes`",
        ),
        (
            "fluids",
            "make: testmake",
            "make: Testmake",
            "make and model must be the catalog's id forms",
        ),
        (
            "fluids",
            "model: sampler",
            "model: other",
            "the file must be named testmake_other.yaml after its make and model",
        ),
        (
            "tires",
            "    source: \"Test door placard\"\n",
            "",
            "missing field `source`",
        ),
        (
            "tires",
            "pressure_unit: kpa",
            "pressure_unit: atm",
            "tires/testmake_sampler.yaml, record 1: \"atm\" is not a unit of pressure; use one of kpa, bar, psi",
        ),
        (
            "tires",
            "    pressure_rear: \"980\"\n",
            "    pressure_rear: \"980.5\"\n",
            "record 1: a pressure in kPa and in psi must be a whole number",
        ),
        (
            "tires",
            "    pressure_unit: kpa\n",
            "",
            "record 1: pressure and pressure_unit go together",
        ),
        (
            "tires",
            "lug_torque_unit: nm",
            "lug_torque_unit: kgm",
            "record 1: \"kgm\" is not a unit of lug_torque; use one of nm, lbft",
        ),
        (
            "tires",
            "size: \"999/99R99\"",
            "size: \"\"",
            "record 1: size is empty",
        ),
        (
            "tires",
            "size_rear: \"997/97R97\"",
            "size_rear: \"998/98R98\"",
            "record 2: size_rear is for a rear size that differs",
        ),
        (
            "tires",
            "trims: [\"Base\", \"Deluxe\"]",
            "trims: [\"Base\", \"Base\"]",
            "record 1: trim \"Base\" is listed twice",
        ),
    ] {
        let error = refused(kind, from, to);
        assert!(error.contains(expected), "{from:?} -> {to:?}\n{error}");
    }
}

/// Builds with one line of a fixture replaced, and gives the error.
fn build_refused(kind: &str, from: &str, to: &str) -> String {
    let scratch = scratch(&format!("hand-bad-{}", from.len() * 100 + to.len()));
    let path = scratch.curated().join(kind).join("testmake_sampler.yaml");
    let text = std::fs::read_to_string(&path).unwrap();
    assert!(text.contains(from), "the fixture has no {from:?}");
    std::fs::write(&path, text.replacen(from, to, 1)).unwrap();
    let Err(error) = build_with(
        &scratch.core(),
        &scratch.out(),
        &scratch.curated(),
        &info(),
        hand::shape,
    ) else {
        panic!("{from:?} -> {to:?} was built");
    };
    assert!(!scratch.out().exists(), "a refused build left a file");
    format!("{error:#}")
}

#[test]
fn a_record_the_catalog_does_not_know_stops_the_build() {
    for (kind, from, to, expected) in [
        (
            "fluids",
            "engine: 9-9l-test",
            "engine: 7-7l-test",
            "fluids/testmake_sampler.yaml, record 1: the catalog has no engine \"7-7l-test\" for model year 2001; it has: 9-9l-test, 8-8l-test-diesel",
        ),
        // The diesel exists in 2001 only.
        (
            "fluids",
            "kind: engine_oil\n    engine: 8-8l-test-diesel",
            "year_to: 2002\n    kind: engine_oil\n    engine: 8-8l-test-diesel",
            "record 2: the catalog has no engine \"8-8l-test-diesel\" for model year 2002; it has: 9-9l-test",
        ),
        (
            "fluids",
            "year_to: 2003\n    kind: brake",
            "year_to: 2004\n    kind: brake",
            "record 6: the catalog has no model year 2004 of this model",
        ),
        (
            "fluids",
            "checked: 2026-10-12",
            "checked: 2026-10-13",
            "record 1: checked 2026-10-13 is after the day of the build, 2026-10-12",
        ),
        (
            "fluids",
            "year_from: 2002\n    kind: coolant",
            "year_from: 2001\n    year_to: 2003\n    kind: brake",
            "record 6: model year 2001 already has a brake record for the same engine, applies_to and market",
        ),
        (
            "tires",
            "year_to: 2002",
            "year_to: 2005",
            "tires/testmake_sampler.yaml, record 1: the catalog has no model year 2004 of this model",
        ),
        (
            "tires",
            "trims: [\"Base\", \"Deluxe\"]\n    size: \"999/99R99\"",
            "size: \"998/98R98\"\n    size_rear: \"997/97R97\"\n    market: us",
            "record 2: model year 2001 already has the fitment 998/98R98 for the same trims and market",
        ),
    ] {
        let error = build_refused(kind, from, to);
        assert!(error.contains(expected), "{from:?} -> {to:?}\n{error}");
    }
    // A make and a model the catalog does not have.
    for (name, text, expected) in [
        (
            "nomake_sampler",
            "make: nomake\nmodel: sampler",
            "fluids/nomake_sampler.yaml: the catalog has no make \"nomake\"",
        ),
        (
            "testmake_nomodel",
            "make: testmake\nmodel: nomodel",
            "fluids/testmake_nomodel.yaml: the catalog has no model \"nomodel\" of \"testmake\"",
        ),
    ] {
        let scratch = scratch(&format!("hand-{name}"));
        let record = "fluids:\n  - year_from: 2001\n    kind: brake\n    specification: \"TEST BRAKE 9\"\n    source: \"Test manual, page 5\"\n    checked: 2026-10-10\n    reviewer: test-reviewer\n";
        std::fs::write(
            scratch
                .curated()
                .join("fluids")
                .join(format!("{name}.yaml")),
            format!("{text}\n{record}"),
        )
        .unwrap();
        let error = build_with(
            &scratch.core(),
            &scratch.out(),
            &scratch.curated(),
            &info(),
            hand::shape,
        )
        .unwrap_err();
        assert!(format!("{error:#}").contains(expected), "{error:#}");
    }
}

#[test]
fn the_files_in_the_repository_are_well_formed() {
    let data = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../data");
    // Every record that will be built.
    hand::load_fluids(&data).unwrap();
    hand::load_tires(&data).unwrap();
    // And the examples, which the build leaves out.
    let example =
        |kind: &str| std::fs::read_to_string(data.join(kind).join("_example.yaml")).unwrap();
    let fluids = parse_fluids("testmake_sampler", &example("fluids")).unwrap();
    assert!(!fluids.records.is_empty());
    let tires = parse_tires("testmake_sampler", &example("tires")).unwrap();
    assert!(!tires.records.is_empty());
    for kind in ["fluids", "tires"] {
        assert!(data.join(kind).join("README.md").is_file(), "{kind}");
    }
}
