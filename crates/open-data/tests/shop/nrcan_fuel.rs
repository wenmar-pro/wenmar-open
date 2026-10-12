use std::path::Path;

use anyhow::Result;
use open_data::shop::epa_match::{CatalogEngines, EngineFacts, match_vehicle};
use open_data::shop::matcher::{CuratedModels, Matcher};
use open_data::shop::nrcan_fuel::{CATALOGUE, NrcanCounts, NrcanKind, NrcanRow, fetch_with, parse};
use open_data::sources::{Downloaded, SourceRecord};
use rusqlite::Connection;

use crate::common::{Scratch, add_engines, nrcan_fuel_dir};

fn read(directory: &Path) -> Result<(Vec<NrcanRow>, NrcanCounts)> {
    let mut rows = Vec::new();
    let counts = parse(directory, &mut |row| {
        rows.push(row);
        Ok(())
    })?;
    Ok((rows, counts))
}

/// What a rating says of its engine. NRCan does not say whether an engine
/// has a turbocharger, and marks an ordinary hybrid only in the model name.
fn facts(row: &NrcanRow) -> EngineFacts {
    EngineFacts {
        displacement_l: row.displacement_l,
        cylinders: row.cylinders,
        diesel: row.fuel == "Diesel",
        turbo: None,
        electric: row.kind == NrcanKind::Electric,
        hybrid: open_data::shop::nrcan_fuel::named_hybrid(&row.model_text),
        plug_in: row.kind == NrcanKind::PlugIn,
    }
}

#[test]
fn the_three_layouts_read_whole() {
    let scratch = Scratch::new("nrcan-rows");
    let (rows, counts) = read(&nrcan_fuel_dir(&scratch)).unwrap();
    assert_eq!(
        counts,
        NrcanCounts {
            files: 4,
            conventional: 16,
            electric: 2,
            plug_in: 4,
        }
    );
    // Files in name order: electric, plug-in, then the two of ratings.
    assert_eq!(
        rows[0],
        NrcanRow {
            kind: NrcanKind::Electric,
            year: 2019,
            make_text: "Tesla".to_owned(),
            model_text: "Model 3 Long Range".to_owned(),
            size_class: "Mid-size".to_owned(),
            displacement_l: None,
            cylinders: None,
            transmission: "A1".to_owned(),
            fuel: "Electricity".to_owned(),
            city_l_100km: None,
            highway_l_100km: None,
            combined_l_100km: None,
            kwh_100km: Some(16.1),
            le_100km: Some(1.8),
            range_km: Some(499),
            co2_g_km: Some(0),
            co2_rating: Some(10),
            smog_rating: Some(10),
            motor_kw: Some(211),
            recharge_h: Some(10.0),
        }
    );
    // A quoted model name with a quotation mark in it.
    assert_eq!(rows[1].model_text, "EX30 Cross Country (19\" Wheels)");
    // A plug-in hybrid: gasoline-only figures, electric range and use.
    let volt = &rows[2];
    assert_eq!(volt.kind, NrcanKind::PlugIn);
    assert_eq!(volt.fuel, "Premium Gasoline");
    assert_eq!(volt.combined_l_100km, Some(6.4));
    assert_eq!((volt.le_100km, volt.kwh_100km), (Some(2.5), Some(22.3)));
    assert_eq!(volt.range_km, Some(56));
    assert_eq!(volt.co2_rating, None);
    assert_eq!(volt.motor_kw, Some(111));
    assert_eq!(rows[3].kwh_100km, Some(23.2));
    // Conventional rows.
    let truck = rows
        .iter()
        .find(|row| row.model_text == "F-150" && row.displacement_l == Some(2.7))
        .unwrap();
    assert_eq!(truck.kind, NrcanKind::Conventional);
    assert_eq!(truck.year, 2018);
    assert_eq!(truck.fuel, "Regular Gasoline");
    assert_eq!(
        (
            truck.city_l_100km,
            truck.highway_l_100km,
            truck.combined_l_100km
        ),
        (Some(11.9), Some(9.0), Some(10.6))
    );
    assert_eq!(truck.co2_g_km, Some(249));
    assert_eq!((truck.co2_rating, truck.smog_rating), (Some(4), Some(5)));
    assert_eq!(truck.transmission, "AS10");
    assert_eq!(truck.kwh_100km, None);
    // The same vehicle on E85 is a row of its own.
    assert_eq!(rows.iter().filter(|row| row.fuel == "E85").count(), 2);
    assert_eq!(rows.iter().filter(|row| row.fuel == "Diesel").count(), 1);
    // `n/a` is None.
    assert_eq!(
        rows.iter()
            .find(|row| row.model_text == "Civic Sedan")
            .unwrap()
            .smog_rating,
        None
    );
    // Latin-1: one byte for the accented letter.
    assert_eq!(
        rows.last().unwrap().model_text,
        "A5 Coup\u{e9} 45 TFSI quattro"
    );
}

#[test]
fn ratings_match_the_catalog_with_the_same_rules_as_epa() {
    let scratch = Scratch::new("nrcan-match");
    add_engines(&scratch);
    let catalog = Connection::open(scratch.core()).unwrap();
    let matcher =
        Matcher::load(&catalog, &CuratedModels::load(&scratch.curated()).unwrap()).unwrap();
    let engines = CatalogEngines::load(&catalog).unwrap();
    let (rows, _) = read(&nrcan_fuel_dir(&scratch)).unwrap();
    let found = |model: &str, litres: Option<f64>, fuel: &str| {
        let row = rows
            .iter()
            .find(|row| row.model_text == model && row.displacement_l == litres && row.fuel == fuel)
            .unwrap();
        // NRCan has no base model, so the model text stands in for it.
        let matched = match_vehicle(
            &matcher,
            &engines,
            row.year,
            &row.make_text,
            &row.model_text,
            &row.model_text,
            &facts(row),
        );
        (matched.model.resolved.model_ids, matched.engine_id)
    };
    let petrol = "Regular Gasoline";
    assert_eq!(
        found("F-150", Some(2.7), petrol),
        (vec![1801], Some(144955))
    );
    assert_eq!(
        found("F-150", Some(3.0), "Diesel"),
        (vec![1801], Some(144956))
    );
    // `4X4` is a drive word and `FFV` is left for the base rule.
    assert_eq!(
        found("F-150 FFV 4X4", Some(5.0), "E85"),
        (vec![1801], Some(144959))
    );
    assert_eq!(found("CR-V AWD", Some(1.5), petrol), (vec![1865], None));
    assert_eq!(
        found("1500 4X4", Some(5.7), petrol),
        (vec![13620], Some(148370))
    );
    // The model name says it is a hybrid, so the hybrid engine is taken.
    assert_eq!(
        found("Accord Hybrid", Some(2.0), petrol),
        (vec![1861], Some(147956))
    );
    // 2016 Civic 1.5L: two engines of that size are not hybrids and NRCan
    // does not say which has the turbocharger, so no engine is given.
    assert_eq!(found("Civic Sedan", Some(1.5), petrol), (vec![1863], None));
    assert_eq!(
        found("Civic Sedan", Some(2.0), petrol),
        (vec![1863], Some(138071))
    );
    // The plug-in Prius finds the PHEV engine.
    assert_eq!(
        found("Prius Plug-in Hybrid", Some(1.8), petrol),
        (vec![2209], Some(120815))
    );
    // No such make, or no such model, in the fixture catalog.
    assert_eq!(
        found("Model 3 Long Range", None, "Electricity"),
        (vec![], None)
    );
    assert_eq!(
        found("Cooper 3 Door", Some(1.5), "Premium Gasoline"),
        (vec![], None)
    );
}

#[test]
fn a_fetch_asks_the_catalogue_which_files_there_are() {
    let scratch = Scratch::new("nrcan-fetch");
    let answer = r#"{"success": true, "result": {"license_id": "ca-ogl-lgo", "resources": [
        {"format": "CSV", "language": ["en"], "url": "https://open.canada.ca/data/dataset/x/resource/b/download/my2026-fuel-consumption-ratings.csv"},
        {"format": "CSV", "language": ["fr"], "url": "https://open.canada.ca/data/dataset/x/resource/c/download/am2026-cotes.csv"},
        {"format": "CSV", "language": ["en"], "url": "https://open.canada.ca/data/dataset/x/resource/e/download/my2012-2026-battery-electric-vehicles.csv"}
    ]}}"#;
    let build = scratch.path("build");
    let directory = build.join("sources/nrcan-fuel");
    // A file of an earlier fetch that is no longer listed.
    std::fs::create_dir_all(&directory).unwrap();
    std::fs::write(directory.join("my2024-old.csv"), b"old").unwrap();
    let mut asked = Vec::new();
    let mut fake = |url: &str, path: &Path| -> Result<Downloaded> {
        asked.push(url.to_owned());
        let body = if url == CATALOGUE {
            answer
        } else {
            "Model year\n"
        };
        std::fs::write(path, body)?;
        Ok(Downloaded {
            last_modified: url
                .contains("my2026")
                .then(|| "Tue, 06 Oct 2026 16:44:28 GMT".to_owned()),
            bytes: body.len() as u64,
        })
    };
    let paths = fetch_with(&build, "2026-10-12 10:20:00", &mut fake).unwrap();
    assert_eq!(asked.len(), 3);
    assert_eq!(asked[0], CATALOGUE);
    assert_eq!(
        paths,
        [
            directory.join("package.json"),
            directory.join("my2026-fuel-consumption-ratings.csv"),
            directory.join("my2012-2026-battery-electric-vehicles.csv"),
            directory.join("source.json"),
        ]
    );
    assert!(!directory.join("my2024-old.csv").exists());
    let record = SourceRecord::read(&directory).unwrap();
    assert_eq!(record.source, "nrcan-fuel");
    assert_eq!(record.as_of, "2026-10-06");
    assert_eq!(record.files.len(), 2);

    // Another licence stops the fetch before any file is taken.
    let scratch = Scratch::new("nrcan-fetch-licence");
    let other = answer.replace("ca-ogl-lgo", "notspecified");
    let mut asked = 0;
    let mut fake = |_: &str, path: &Path| -> Result<Downloaded> {
        asked += 1;
        std::fs::write(path, &other)?;
        Ok(Downloaded {
            last_modified: None,
            bytes: 1,
        })
    };
    let error = fetch_with(&scratch.path("build"), "2026-10-12 10:20:00", &mut fake).unwrap_err();
    assert!(format!("{error:#}").contains("licence is now"), "{error:#}");
    assert_eq!(asked, 1);
}

#[test]
fn an_unknown_layout_or_fuel_is_refused() {
    let scratch = Scratch::new("nrcan-bad");
    let directory = scratch.path("sources/nrcan-fuel");
    let error = read(&directory).unwrap_err();
    assert!(
        format!("{error:#}").contains("open-data fetch nrcan-fuel"),
        "{error:#}"
    );
    std::fs::create_dir_all(&directory).unwrap();
    std::fs::write(directory.join("a.csv"), "Model year,Make\n2026,Ford\n").unwrap();
    let error = read(&directory).unwrap_err();
    assert!(
        format!("{error:#}").contains("a header this build does not read"),
        "{error:#}"
    );
    let header = open_data::shop::nrcan_fuel::CONVENTIONAL.join(",");
    std::fs::write(
        directory.join("a.csv"),
        format!("{header}\n2026,Ford,F-150,Pickup truck: Standard,2.7,6,AS10,Q,11.9,9.0,10.6,27,249,4,5\n"),
    )
    .unwrap();
    let error = read(&directory).unwrap_err();
    assert!(
        format!("{error:#}").contains("fuel type \"Q\" is not known"),
        "{error:#}"
    );
}

/// Reads the real files and measures matching. Run after
/// `open-data fetch nrcan-fuel`:
/// `cargo test --release -p open-data --test shop the_fetched_nrcan_files_parse -- --ignored --nocapture`
#[test]
#[ignore = "needs data/build/sources/nrcan-fuel and a core data file"]
fn the_fetched_nrcan_files_parse() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let catalog = Connection::open(root.join("data/build/wenmar-open-2026.09.sqlite3")).unwrap();
    let matcher =
        Matcher::load(&catalog, &CuratedModels::load(&root.join("data")).unwrap()).unwrap();
    let engines = CatalogEngines::load(&catalog).unwrap();
    let (rows, counts) = read(&root.join("data/build/sources/nrcan-fuel")).unwrap();
    println!("{counts:?}");
    for (name, from) in [("all years", 0), ("2010 on", 2010)] {
        let (mut total, mut model, mut engine) = (0u64, 0u64, 0u64);
        let mut unmatched: std::collections::BTreeMap<(String, String), u64> = Default::default();
        for row in rows.iter().filter(|row| row.year >= from) {
            let matched = match_vehicle(
                &matcher,
                &engines,
                row.year,
                &row.make_text,
                &row.model_text,
                &row.model_text,
                &facts(row),
            );
            total += 1;
            if matched.confidence.is_some() {
                model += 1;
            } else {
                *unmatched
                    .entry((row.make_text.clone(), row.model_text.clone()))
                    .or_default() += 1;
            }
            engine += u64::from(matched.engine_id.is_some());
        }
        println!(
            "{name}: rows {total}, model {model} ({:.1}%), engine {engine} ({:.1}% of rows, {:.1}% of model-matched)",
            model as f64 * 100.0 / total as f64,
            engine as f64 * 100.0 / total as f64,
            engine as f64 * 100.0 / model as f64,
        );
        if from > 0 {
            let mut commonest: Vec<_> = unmatched.iter().collect();
            commonest.sort_by(|left, right| right.1.cmp(left.1).then(left.0.cmp(right.0)));
            for ((make, model), rows) in commonest.into_iter().take(25) {
                println!("unmatched 2010 on: {rows:>3} {make} | {model}");
            }
        }
    }
    assert!(counts.conventional > 25_000);
}
