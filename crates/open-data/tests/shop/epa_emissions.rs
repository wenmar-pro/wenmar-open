use std::collections::BTreeMap;

use open_data::shop::epa_emissions::{EmissionCounts, EmissionRow, FILE, distinct, parse};

use crate::common::{Scratch, epa_emissions_dir};

fn read(directory: &std::path::Path) -> anyhow::Result<(Vec<EmissionRow>, EmissionCounts)> {
    let mut rows = Vec::new();
    let counts = parse(directory, &mut |row| {
        rows.push(row);
        Ok(())
    })?;
    Ok((rows, counts))
}

#[test]
fn the_fixture_reads_whole() {
    let scratch = Scratch::new("epa-emissions");
    let (rows, counts) = read(&epa_emissions_dir(&scratch)).unwrap();
    assert_eq!(counts, EmissionCounts { rows: 16 });
    // Two certifications for each of eight vehicles.
    let mut by_vehicle: BTreeMap<u32, Vec<EmissionRow>> = BTreeMap::new();
    for row in rows {
        by_vehicle.entry(row.epa_id).or_default().push(row);
    }
    assert_eq!(by_vehicle.len(), 8);
    let truck = distinct(by_vehicle[&39241].clone());
    assert_eq!(
        truck,
        [
            EmissionRow {
                epa_id: 39241,
                sales_area: 3,
                standard: "T3B70".to_owned(),
                standard_text: Some("Federal Tier 3 Bin 70".to_owned()),
                smog_score: Some(5),
                engine_family: "JFMXT02.73JK".to_owned(),
            },
            EmissionRow {
                epa_id: 39241,
                sales_area: 7,
                standard: "L3ULEV70".to_owned(),
                standard_text: Some("California LEV-III ULEV70".to_owned()),
                smog_score: Some(5),
                engine_family: "JFMXT02.73JK".to_owned(),
            },
        ]
    );
    // Whatever order the file has them in, they come out by sales area.
    let areas: Vec<u8> = distinct(by_vehicle[&39449].clone())
        .iter()
        .map(|row| row.sales_area)
        .collect();
    assert_eq!(areas, [3, 7]);
    let electric = distinct(by_vehicle[&41189].clone());
    assert_eq!(electric[1].standard, "ZEV");
    assert_eq!(electric[1].smog_score, Some(10));
}

#[test]
fn a_missing_archive_says_to_fetch() {
    let scratch = Scratch::new("epa-emissions-missing");
    let error = read(&scratch.path("sources/epa-emissions")).unwrap_err();
    let message = format!("{error:#}");
    assert!(message.contains(FILE), "{message}");
    assert!(
        message.contains("open-data fetch epa-emissions"),
        "{message}"
    );
}

/// Reads the real file. Run after `open-data fetch epa-emissions`:
/// `cargo test --release -p open-data --test shop the_fetched_emissions_file_parses -- --ignored --nocapture`
#[test]
#[ignore = "needs data/build/sources/epa-emissions"]
fn the_fetched_emissions_file_parses() {
    let directory = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../data/build/sources/epa-emissions");
    let (rows, counts) = read(&directory).unwrap();
    let vehicles: std::collections::BTreeSet<u32> = rows.iter().map(|row| row.epa_id).collect();
    let mut areas: BTreeMap<u8, u64> = BTreeMap::new();
    let mut standards: std::collections::BTreeSet<&str> = Default::default();
    for row in &rows {
        *areas.entry(row.sales_area).or_default() += 1;
        standards.insert(&row.standard);
    }
    let kept = distinct(rows.clone()).len();
    println!(
        "{counts:?}, {kept} distinct, {} vehicles, sales areas {areas:?}, {} standards, {} without a score, {} without words",
        vehicles.len(),
        standards.len(),
        rows.iter().filter(|row| row.smog_score.is_none()).count(),
        rows.iter()
            .filter(|row| row.standard_text.is_none())
            .count(),
    );
    assert!(counts.rows > 50_000);
}
