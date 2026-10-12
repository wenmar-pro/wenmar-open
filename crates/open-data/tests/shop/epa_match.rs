use std::collections::BTreeMap;

use open_data::shop::epa_match::{
    CatalogEngines, Confidence, EngineFacts, match_model, match_vehicle,
};
use open_data::shop::epa_vehicles::{self, EpaVehicleRow};
use open_data::shop::matcher::{CuratedModels, MatchKind, Matcher};
use rusqlite::Connection;

use crate::common::{Scratch, add_engines, epa_vehicles_dir};

/// What `vehicles.csv` says of a row's engine.
fn facts(row: &EpaVehicleRow) -> EngineFacts {
    let atv = row.atv_type.as_deref().unwrap_or("");
    EngineFacts {
        displacement_l: row.displacement_l,
        cylinders: row.cylinders,
        diesel: row.fuel1.as_deref() == Some("Diesel"),
        turbo: Some(row.turbo),
        electric: row.displacement_l.is_none(),
        hybrid: Some(atv == "Hybrid"),
        plug_in: atv == "Plug-in Hybrid",
    }
}

struct Loaded {
    matcher: Matcher,
    engines: CatalogEngines,
}

fn load(core: &std::path::Path, curated: &std::path::Path) -> Loaded {
    let catalog = Connection::open(core).unwrap();
    Loaded {
        matcher: Matcher::load(&catalog, &CuratedModels::load(curated).unwrap()).unwrap(),
        engines: CatalogEngines::load(&catalog).unwrap(),
    }
}

fn fixture_rows(scratch: &Scratch) -> Vec<EpaVehicleRow> {
    let mut rows = Vec::new();
    epa_vehicles::parse(&epa_vehicles_dir(scratch), &mut |row| {
        rows.push(row);
        Ok(())
    })
    .unwrap();
    rows
}

#[test]
fn each_rule_finds_its_model() {
    let scratch = Scratch::new("epa-match-model");
    let loaded = load(&scratch.core(), &scratch.curated());
    for (year, make, model, base, model_ids, kind, step) in [
        // 1: the text as written.
        (
            2019,
            "Ram",
            "1500",
            "1500",
            vec![13620],
            Some(MatchKind::Exact),
            1,
        ),
        // 2: the name without drive and body.
        (
            2018,
            "Ford",
            "F150 Pickup 2WD",
            "F150",
            vec![1801],
            Some(MatchKind::Exact),
            2,
        ),
        (
            2019,
            "Honda",
            "CR-V AWD",
            "CR-V",
            vec![1865],
            Some(MatchKind::Exact),
            2,
        ),
        (
            2025,
            "Mazda",
            "CX-50 4WD",
            "CX-50",
            vec![30046],
            Some(MatchKind::Exact),
            2,
        ),
        // 3: the base model.
        (
            2018,
            "Ford",
            "F150 Pickup 4WD FFV",
            "F150",
            vec![1801],
            Some(MatchKind::Base),
            3,
        ),
        (
            2016,
            "Honda",
            "Civic 4Dr",
            "Civic",
            vec![1863],
            Some(MatchKind::Base),
            3,
        ),
        (
            2020,
            "Chevrolet",
            "Tahoe K1500 4WD",
            "Tahoe",
            vec![1852],
            Some(MatchKind::Base),
            3,
        ),
        (
            2015,
            "GMC",
            "Sierra K15 4WD",
            "Sierra",
            vec![1857],
            Some(MatchKind::Base),
            3,
        ),
        // 4: words taken off the end; the base model is a family name.
        (
            2015,
            "BMW",
            "328i xDrive",
            "3 Series",
            vec![1709],
            Some(MatchKind::Base),
            4,
        ),
        (
            2015,
            "BMW",
            "328d xDrive Sports Wagon",
            "3 Series",
            vec![2173],
            Some(MatchKind::Base),
            4,
        ),
        (
            2005,
            "Dodge",
            "Ram 1500 Pickup 2WD",
            "Ram 1500 Pickup",
            vec![1938],
            Some(MatchKind::Base),
            4,
        ),
        (
            2016,
            "Mercedes-Benz",
            "C300 4matic",
            "C-Class",
            vec![2085],
            Some(MatchKind::Base),
            3,
        ),
        // 6: the catalog has no 2019 328i here, so the family name in
        // models.yaml answers: the 3 Series of that year.
        (
            2019,
            "BMW",
            "328i xDrive",
            "3 Series",
            vec![2777],
            Some(MatchKind::Curated),
            6,
        ),
        // A year the catalog has none of the family in.
        (2022, "BMW", "328i xDrive", "3 Series", vec![], None, 0),
        // A make the catalog does not have, and one it has under another name.
        (2012, "Scion", "tC", "tC", vec![], None, 0),
        (
            2019,
            "Tesla",
            "Model 3 Long Range",
            "Model 3",
            vec![],
            None,
            0,
        ),
    ] {
        let found = match_model(&loaded.matcher, year, make, model, base);
        assert_eq!(
            (
                found.resolved.model_ids.clone(),
                found.resolved.kind,
                found.step
            ),
            (model_ids, kind, step),
            "{year} {make} {model}"
        );
    }
    // The make is kept when only the model is unmatched.
    let found = match_model(&loaded.matcher, 2022, "BMW", "328i xDrive", "3 Series");
    assert_eq!(found.resolved.make_id, Some(452));
    assert_eq!(found.name.name, "328i xDrive");
    // The make followed by the base model: `Mazda` and `3` is `Mazda3`.
    let connection = Connection::open(scratch.core()).unwrap();
    connection
        .execute_batch("INSERT INTO catalog_vehicle VALUES (90, 2025, 473, 2074, 132, 1, NULL);")
        .unwrap();
    drop(connection);
    let loaded = load(&scratch.core(), &scratch.curated());
    let found = match_model(&loaded.matcher, 2025, "Mazda", "3 4-Door 2WD", "3");
    assert_eq!(
        (found.resolved.model_ids, found.resolved.kind, found.step),
        (vec![2074], Some(MatchKind::Base), 3)
    );
}

/// Model ids, engine id, submodel id and confidence of one row.
type Found = (Vec<i64>, Option<i64>, Option<i64>, Option<Confidence>);

#[test]
fn the_fixture_rows_match_as_far_as_the_catalog_goes() {
    let scratch = Scratch::new("epa-match-rows");
    add_engines(&scratch);
    let loaded = load(&scratch.core(), &scratch.curated());
    let rows = fixture_rows(&scratch);
    let found: BTreeMap<u32, Found> = rows
        .iter()
        .map(|row| {
            let matched = match_vehicle(
                &loaded.matcher,
                &loaded.engines,
                row.year,
                &row.make_text,
                &row.model_text,
                &row.base_model,
                &facts(row),
            );
            (
                row.id,
                (
                    matched.model.resolved.model_ids,
                    matched.engine_id,
                    matched.submodel_id,
                    matched.confidence,
                ),
            )
        })
        .collect();
    let engine = Some(Confidence::Engine);
    let model = Some(Confidence::Model);
    for (id, expected) in [
        // 2018 F-150: 3.5L turbo, 2.7L, and the flexible-fuel 3.3L.
        (39238, (vec![1801], Some(144958), None, engine)),
        (39241, (vec![1801], Some(144955), None, engine)),
        (39252, (vec![1801], Some(144958), None, engine)),
        (39449, (vec![1801], Some(144957), None, engine)),
        // 2020 Silverado 5.3L: the catalog's label has no layout.
        (41833, (vec![1850], Some(155277), None, engine)),
        // 2016 Civic 1.5L turbo: three 1.5L engines, the turbo decides.
        (37075, (vec![1863], Some(138069), None, engine)),
        // 2018 Civic: the fixture catalog has no engines for it.
        (39348, (vec![1863], None, None, model)),
        // 2020 Accord Hybrid: engine, and `Hybrid` is a listed submodel.
        (
            42135,
            (
                vec![1861],
                Some(147956),
                Some(90001),
                Some(Confidence::Submodel),
            ),
        ),
        // 2015 328d: diesel to diesel.
        (35568, (vec![2173], Some(130489), None, engine)),
        // 2013 Prius and Prius Plug-in: the hybrid and the PHEV engine.
        (33324, (vec![2209], Some(120814), None, engine)),
        (33335, (vec![2209], Some(120815), None, engine)),
        // 2019 Ram 1500 5.7L V8.
        (39941, (vec![13620], Some(148370), None, engine)),
        // Matched to a model only: no engines in the fixture catalog.
        (40857, (vec![1865], None, None, model)),
        (48537, (vec![2369], None, None, model)),
        // Unmatched.
        (31356, (vec![], None, None, None)),
        (41189, (vec![], None, None, None)),
    ] {
        assert_eq!(found[&id], expected, "EPA id {id}");
    }
}

#[test]
fn an_engine_is_given_only_when_one_is_left() {
    let scratch = Scratch::new("epa-match-engine");
    add_engines(&scratch);
    let loaded = load(&scratch.core(), &scratch.curated());
    let engines = &loaded.engines;
    let petrol = |litres: f64, cylinders: u8| EngineFacts {
        displacement_l: Some(litres),
        cylinders: Some(cylinders),
        turbo: Some(false),
        hybrid: Some(false),
        ..EngineFacts::default()
    };
    // No engine of that size.
    assert_eq!(engines.engine(2018, 1801, &petrol(4.6, 8)), None);
    // The size is there but the label says V6 and the row says 8.
    assert_eq!(engines.engine(2018, 1801, &petrol(2.7, 8)), None);
    // A petrol row never takes the diesel, nor a diesel row a petrol engine.
    assert_eq!(engines.engine(2018, 1801, &petrol(3.0, 6)), None);
    let diesel = EngineFacts {
        diesel: true,
        ..petrol(3.0, 6)
    };
    assert_eq!(engines.engine(2018, 1801, &diesel), Some(144956));
    assert_eq!(
        engines.engine(
            2018,
            1801,
            &EngineFacts {
                diesel: true,
                ..petrol(2.7, 6)
            }
        ),
        None
    );
    // 2016 Civic 1.5L without a turbocharger: `1.5L`, not the hybrid.
    assert_eq!(engines.engine(2016, 1863, &petrol(1.5, 4)), Some(138067));
    // A hybrid row takes the hybrid.
    let hybrid = EngineFacts {
        hybrid: Some(true),
        ..petrol(1.5, 4)
    };
    assert_eq!(engines.engine(2016, 1863, &hybrid), Some(138068));
    // A source that does not say whether there is a turbocharger (NRCan)
    // leaves two 1.5L engines that are not hybrids: no engine.
    let unknown = EngineFacts {
        turbo: None,
        ..petrol(1.5, 4)
    };
    assert_eq!(engines.engine(2016, 1863, &unknown), None);
    // A source that does not say whether the row is a hybrid either
    // leaves all three.
    let unknown = EngineFacts {
        turbo: None,
        hybrid: None,
        ..petrol(1.5, 4)
    };
    assert_eq!(engines.engine(2016, 1863, &unknown), None);
    // 2020 Accord 2.0L: a hybrid and a turbocharged engine. Without
    // knowing which the row is, neither is given.
    let unknown = EngineFacts {
        turbo: None,
        hybrid: None,
        ..petrol(2.0, 4)
    };
    assert_eq!(engines.engine(2020, 1861, &unknown), None);
    let named = EngineFacts {
        hybrid: Some(true),
        ..unknown
    };
    assert_eq!(engines.engine(2020, 1861, &named), Some(147956));
    // An electric row needs an engine labelled Electric.
    let electric = EngineFacts {
        electric: true,
        ..EngineFacts::default()
    };
    assert_eq!(engines.engine(2018, 1801, &electric), None);
    // A model year with no detail, and one the catalog does not have.
    assert_eq!(engines.engine(2018, 1863, &petrol(1.5, 4)), None);
    assert_eq!(engines.engine(1999, 1801, &petrol(2.7, 6)), None);
    // Submodels: a listed one by a run of words; never an unlisted one,
    // never the model's own name.
    assert_eq!(engines.submodel(2018, 1801, "F150 RAPTOR"), Some(47514));
    assert_eq!(engines.submodel(2018, 1801, "F150 Pickup XL/XLT"), None);
    assert_eq!(engines.submodel(2018, 1801, "F150 XL"), Some(47516));
    assert_eq!(engines.submodel(2020, 1861, "Accord Hidden"), None);
    assert_eq!(engines.submodel(2018, 1801, "F150"), None);
}

#[derive(Default)]
struct Tally {
    rows: u64,
    model: u64,
    engine: u64,
    submodel: u64,
    keys: BTreeMap<(i32, String, String), bool>,
    steps: [u64; 7],
    unmatched: BTreeMap<(String, String), u64>,
}

impl Tally {
    fn line(&self, name: &str) -> String {
        let share = |part: u64, whole: u64| part as f64 * 100.0 / whole.max(1) as f64;
        let keys = self.keys.len() as u64;
        let matched_keys = self.keys.values().filter(|matched| **matched).count() as u64;
        format!(
            "{name}: rows {}, model {} ({:.1}%), engine {} ({:.1}% of rows, {:.1}% of model-matched), submodel {} ({:.1}%); year x make x model {} matched {} ({:.1}%); steps 0-6 {:?}",
            self.rows,
            self.model,
            share(self.model, self.rows),
            self.engine,
            share(self.engine, self.rows),
            share(self.engine, self.model),
            self.submodel,
            share(self.submodel, self.rows),
            keys,
            matched_keys,
            share(matched_keys, keys),
            self.steps,
        )
    }
}

/// Measures matching on the real file against the real catalog. Run after
/// `open-data fetch epa-vehicles`:
/// `cargo test --release -p open-data --test shop the_fetched_epa_file_matches -- --ignored --nocapture`
#[test]
#[ignore = "needs data/build/sources/epa-vehicles and a core data file"]
fn the_fetched_epa_file_matches() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let core = root.join("data/build/wenmar-open-2026.09.sqlite3");
    let loaded = load(&core, &root.join("data"));
    let mut all = Tally::default();
    let mut recent = Tally::default();
    epa_vehicles::parse(&root.join("data/build/sources/epa-vehicles"), &mut |row| {
        let matched = match_vehicle(
            &loaded.matcher,
            &loaded.engines,
            row.year,
            &row.make_text,
            &row.model_text,
            &row.base_model,
            &facts(&row),
        );
        for tally in [Some(&mut all), (row.year >= 2010).then_some(&mut recent)]
            .into_iter()
            .flatten()
        {
            tally.rows += 1;
            tally.steps[matched.model.step as usize] += 1;
            let is_matched = matched.confidence.is_some();
            tally.keys.insert(
                (row.year, row.make_text.clone(), row.model_text.clone()),
                is_matched,
            );
            if is_matched {
                tally.model += 1;
            } else {
                *tally
                    .unmatched
                    .entry((row.make_text.clone(), row.base_model.clone()))
                    .or_default() += 1;
            }
            tally.engine += u64::from(matched.engine_id.is_some());
            tally.submodel += u64::from(matched.submodel_id.is_some());
        }
        Ok(())
    })
    .unwrap();
    println!("{}", all.line("all years"));
    println!("{}", recent.line("2010 on"));
    let mut commonest: Vec<(&(String, String), &u64)> = recent.unmatched.iter().collect();
    commonest.sort_by(|left, right| right.1.cmp(left.1).then(left.0.cmp(right.0)));
    for ((make, base), rows) in commonest.into_iter().take(40) {
        println!("unmatched 2010 on: {rows:>4} {make} | {base}");
    }
    assert!(
        recent.model * 100 >= recent.rows * 90,
        "under 90% of rows from 2010 on matched a model"
    );
}
