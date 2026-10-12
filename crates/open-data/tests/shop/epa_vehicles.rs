use open_data::shop::epa_vehicles::{EpaCounts, EpaVehicleRow, FILE, HEADER, parse};

use crate::common::{Scratch, epa_vehicles_dir, fixture};

fn read(directory: &std::path::Path) -> anyhow::Result<(Vec<EpaVehicleRow>, EpaCounts)> {
    let mut rows = Vec::new();
    let counts = parse(directory, &mut |row| {
        rows.push(row);
        Ok(())
    })?;
    Ok((rows, counts))
}

fn fixture_rows(name: &str) -> Vec<EpaVehicleRow> {
    let scratch = Scratch::new(name);
    let (rows, counts) = read(&epa_vehicles_dir(&scratch)).unwrap();
    assert_eq!(counts, EpaCounts { rows: 29 });
    rows
}

fn by_id(rows: &[EpaVehicleRow], id: u32) -> &EpaVehicleRow {
    rows.iter().find(|row| row.id == id).unwrap()
}

#[test]
fn the_fixture_header_is_the_pinned_one() {
    let text = std::fs::read_to_string(fixture("epa-vehicles").join("vehicles.csv")).unwrap();
    assert_eq!(text.lines().next().unwrap(), HEADER.join(","));
}

#[test]
fn a_petrol_truck_reads_whole() {
    let rows = fixture_rows("epa-truck");
    assert_eq!(rows.len(), 29);
    let row = by_id(&rows, 39241);
    assert_eq!(
        row,
        &EpaVehicleRow {
            id: 39241,
            year: 2018,
            make_text: "Ford".to_owned(),
            model_text: "F150 Pickup 2WD".to_owned(),
            base_model: "F150".to_owned(),
            displacement_l: Some(2.7),
            cylinders: Some(6),
            transmission: Some("Automatic (S10)".to_owned()),
            drive: Some("Rear-Wheel Drive".to_owned()),
            fuel: "Regular".to_owned(),
            fuel1: Some("Regular Gasoline".to_owned()),
            fuel2: None,
            size_class: "Standard Pickup Trucks 2WD".to_owned(),
            atv_type: None,
            turbo: true,
            supercharger: false,
            start_stop: Some(true),
            guzzler: false,
            engine_index: 25,
            mfr_code: Some("FMX".to_owned()),
            engine_text: Some("SIDI & PFI".to_owned()),
            transmission_text: None,
            ev_motor: None,
            city_mpg: 20,
            highway_mpg: 26,
            combined_mpg: 22,
            city_mpg2: None,
            highway_mpg2: None,
            combined_mpg2: None,
            co2_g_mi: 401.0,
            kwh_100mi: None,
            range_mi: None,
            range2_mi: None,
            charge_240v_h: None,
            phev_combined_mpge: None,
            passenger_ft3_2door: None,
            passenger_ft3_4door: None,
            passenger_ft3_hatch: None,
            luggage_ft3_2door: None,
            luggage_ft3_4door: None,
            luggage_ft3_hatch: None,
        }
    );
}

#[test]
fn the_kinds_of_powertrain_are_told_apart() {
    let rows = fixture_rows("epa-kinds");
    // Flexible fuel: a second fuel with its own figures.
    let ffv = by_id(&rows, 39449);
    assert_eq!(ffv.fuel, "Gasoline or E85");
    assert_eq!(ffv.fuel2.as_deref(), Some("E85"));
    assert_eq!(ffv.atv_type.as_deref(), Some("FFV"));
    assert!(ffv.combined_mpg2.is_some());
    assert!(!ffv.turbo);
    // Diesel.
    let diesel = by_id(&rows, 35568);
    assert_eq!(diesel.fuel1.as_deref(), Some("Diesel"));
    assert_eq!(diesel.atv_type.as_deref(), Some("Diesel"));
    assert_eq!(diesel.model_text, "328d xDrive Sports Wagon");
    assert_eq!(diesel.base_model, "3 Series");
    // Hybrid, with its battery in evMotor.
    let hybrid = by_id(&rows, 42135);
    assert_eq!(hybrid.atv_type.as_deref(), Some("Hybrid"));
    assert_eq!(hybrid.ev_motor.as_deref(), Some("259V Li-Ion"));
    // Plug-in hybrid.
    let plug_in = by_id(&rows, 33335);
    assert_eq!(plug_in.atv_type.as_deref(), Some("Plug-in Hybrid"));
    assert_eq!(plug_in.fuel2.as_deref(), Some("Electricity"));
    assert!(plug_in.phev_combined_mpge.is_some());
    // `range` is its range on gasoline; the battery's is `rangeA`.
    assert_eq!(plug_in.range_mi, Some(540));
    assert_eq!(plug_in.range2_mi, Some(11));
    // Electric: no displacement, no cylinders, a range.
    let electric = by_id(&rows, 41189);
    assert_eq!(electric.make_text, "Tesla");
    assert_eq!(electric.displacement_l, None);
    assert_eq!(electric.cylinders, None);
    assert_eq!(electric.fuel1.as_deref(), Some("Electricity"));
    assert_eq!(electric.range_mi, Some(310));
    assert_eq!(electric.co2_g_mi, 0.0);
    assert!(electric.kwh_100mi.is_some());
    assert_eq!(electric.combined_mpg, 130);
    // Midgrade is a recommended grade of its own.
    assert_eq!(
        by_id(&rows, 39941).fuel1.as_deref(),
        Some("Midgrade Gasoline")
    );
}

#[test]
fn old_rows_have_gaps_and_quoted_fields_are_read() {
    let rows = fixture_rows("epa-old");
    let old = by_id(&rows, 1232);
    assert_eq!(old.year, 1985);
    assert_eq!(old.mfr_code, None);
    assert_eq!(old.start_stop, None);
    assert_eq!(old.engine_index, 26005);
    assert!((old.co2_g_mi - 261.382_352_941_176_46).abs() < 1e-9);
    // `"Vans, Passenger Type"` is one field.
    let van = by_id(&rows, 39452);
    assert_eq!(van.size_class, "Vans, Passenger Type");
    assert_eq!(van.model_text, "Transit T150 Wagon");
    // Rows arrive in file order.
    let ids: Vec<u32> = rows.iter().map(|row| row.id).collect();
    assert_eq!(ids[0], 1232);
    assert_eq!(ids.len(), 29);
}

#[test]
fn a_missing_archive_says_to_fetch() {
    let scratch = Scratch::new("epa-missing");
    let error = read(&scratch.path("sources/epa-vehicles")).unwrap_err();
    let message = format!("{error:#}");
    assert!(message.contains(FILE), "{message}");
    assert!(
        message.contains("open-data fetch epa-vehicles"),
        "{message}"
    );
}

/// Reads the real file. Run after `open-data fetch epa-vehicles`:
/// `cargo test --release -p open-data --test shop the_fetched_epa_file_parses -- --ignored --nocapture`
#[test]
#[ignore = "needs data/build/sources/epa-vehicles"]
fn the_fetched_epa_file_parses() {
    let directory = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../data/build/sources/epa-vehicles");
    let (rows, counts) = read(&directory).unwrap();
    let years: std::collections::BTreeSet<i32> = rows.iter().map(|row| row.year).collect();
    let keys: std::collections::BTreeSet<(i32, &str, &str)> = rows
        .iter()
        .map(|row| (row.year, row.make_text.as_str(), row.model_text.as_str()))
        .collect();
    println!(
        "{counts:?}, model years {} to {}, {} year x make x model",
        years.first().unwrap(),
        years.last().unwrap(),
        keys.len()
    );
    assert!(counts.rows > 50_000);
}
