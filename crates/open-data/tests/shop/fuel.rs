use open_data::shop::build::{ShopInputs, build_shop, build_with};
use open_data::shop::fuel::{shape_epa, shape_nrcan};
use rusqlite::Connection;
use wenmar_shopdata::Dataset;

use crate::common::{
    Scratch, add_engines, epa_emissions_dir, epa_guide_dir, epa_vehicles_dir, info, nrcan_fuel_dir,
    rows,
};

/// The curated oil spellings the fixture builds with.
fn add_oil_file(scratch: &Scratch) {
    std::fs::write(
        scratch.curated().join("shop/oil-grades.yaml"),
        "grades:\n  - text: \"SW30\"\n    grades: [\"5W-30\"]\n",
    )
    .unwrap();
}

fn epa(scratch: &Scratch) -> Connection {
    add_engines(scratch);
    add_oil_file(scratch);
    let (vehicles, guide, emissions) = (
        epa_vehicles_dir(scratch),
        epa_guide_dir(scratch),
        epa_emissions_dir(scratch),
    );
    let summary = build_with(
        &scratch.core(),
        &scratch.out(),
        &scratch.curated(),
        &info(),
        |context| shape_epa(context, &vehicles, Some(&guide), Some(&emissions)),
    )
    .unwrap();
    let entry = |dataset| summary.datasets.get(dataset).cloned().unwrap();
    assert_eq!(entry(Dataset::FuelUs).rows, 29);
    assert_eq!(entry(Dataset::FuelUs).as_of, "2026-09-29");
    assert_eq!(entry(Dataset::FuelUs).licence, "public-domain");
    assert_eq!(entry(Dataset::Specs).rows, 29);
    assert_eq!(entry(Dataset::OilGrade).rows, 7);
    assert!(!summary.datasets.contains(Dataset::FuelCa));
    Connection::open(scratch.out()).unwrap()
}

#[test]
fn epa_rows_are_stored_matched_and_in_both_units() {
    let scratch = Scratch::new("fuel-epa");
    let shop = epa(&scratch);
    assert_eq!(
        rows(
            &shop,
            "SELECT source_id, year, make_id, model_id, \"match\", engine_id, submodel_id, confidence,
                    drive_text, body_text, displacement_l, cylinders, transmission, drive, fuel, fuel2,
                    city_l_100km, highway_l_100km, combined_l_100km, city_mpg, highway_mpg,
                    combined_mpg, kwh_100km, range_km, co2_g_km
             FROM fuel_rating WHERE source_id IN (39241, 39449, 42135, 41189, 33335, 31356)
             ORDER BY source_id"
        ),
        [
            // Scion: not a catalog make. Stored all the same.
            "31356|2012|-|-|-|-|-|-|-|-|2.5|4|Manual 6-spd|Front-Wheel Drive|Regular Gasoline|-|10.2|7.8|9.4|23|30|25|-|-|221",
            // A plug-in hybrid: gasoline figures and the electric range.
            "33335|2013|448|2209|base|120815|-|engine|-|-|1.8|4|Automatic (variable gear ratios)|Front-Wheel Drive|Regular Gasoline|Electricity|4.6|4.8|4.7|51|49|50|18|18|83",
            "39241|2018|460|1801|exact|144955|-|engine|2WD|Pickup|2.7|6|Automatic (S10)|Rear-Wheel Drive|Regular Gasoline|-|11.8|9|10.7|20|26|22|-|-|249",
            "39449|2018|460|1801|base|144957|-|engine|2WD|-|3.3|6|Automatic (S6)|Rear-Wheel Drive|Regular Gasoline|E85|11.8|9.4|10.7|20|25|22|-|-|253",
            // Electric: no litres per 100 km and no miles per gallon.
            "41189|2019|-|-|-|-|-|-|-|-|-|-|Automatic (A1)|Rear-Wheel Drive|Electricity|-|-|-|-|-|-|-|16.1|499|0",
            "42135|2020|474|1861|base|147956|90001|submodel|-|-|2|4|Automatic (variable gear ratios)|Front-Wheel Drive|Regular Gasoline|-|4.9|5|4.9|48|47|48|-|-|115",
        ]
    );
    // One row for each row of the file: none matched two models.
    assert_eq!(
        rows(
            &shop,
            "SELECT count(*), count(DISTINCT source_id) FROM fuel_rating"
        ),
        ["29|29"]
    );
    assert_eq!(
        rows(
            &shop,
            "SELECT count(*) FROM fuel_rating WHERE (model_id IS NULL) <> (confidence IS NULL) OR (model_id IS NULL) <> (\"match\" IS NULL)"
        ),
        ["0"]
    );
}

#[test]
fn specs_oil_and_emissions_hang_on_the_epa_id() {
    let scratch = Scratch::new("fuel-specs");
    let shop = epa(&scratch);
    assert_eq!(
        rows(
            &shop,
            "SELECT * FROM vehicle_spec WHERE epa_id IN (39241, 41833, 42135, 41189) ORDER BY epa_id"
        ),
        [
            "39241|Standard Pickup Trucks 2WD|1|0|1|-|-|-|-|-|-|-|-|-|10|Semi-Automatic|Gasoline (Regular Unleaded Recommended)|0|-|1|0|2|2|-|-|-",
            // Electric: volumes from vehicles.csv, nothing from the guide's first sheet.
            "41189|Midsize Cars|0|0|0|EV|211 kW AC 3-Phase|10|-|97|-|-|15|-|-|-|-|-|-|-|-|-|-|-|-|-",
            "41833|Standard Pickup Trucks 4WD|0|0|1|-|-|-|-|-|-|-|-|-|8|Automatic|Gasoline (Regular Unleaded Recommended)|1|Active Fuel Management|1|0|1|1|-|-|-",
            "42135|Large Cars|0|0|1|Hybrid|259V Li-Ion|-|-|106|-|-|17|-|1|Continuously Variable|Gasoline (Regular Unleaded Recommended)|0|-|1|1|2|2|Lithium Ion|259|135",
        ]
    );
    assert_eq!(
        rows(&shop, "SELECT * FROM oil_grade ORDER BY epa_id"),
        [
            "39238|5W-30|5W30 GF-5",
            "39241|5W-30|5W-30 GF-5",
            "39252|5W-30|5W30 GF-5",
            "39348|0W-20|0W-20",
            "39449|5W-20|5W-20 GF-5",
            "41833|0W-20|0W20",
            "42135|0W-20|0W-20",
        ]
    );
    assert_eq!(
        rows(
            &shop,
            "SELECT * FROM emission_standard WHERE epa_id IN (39241, 41189) ORDER BY epa_id, sales_area"
        ),
        [
            "39241|3|T3B70|Federal Tier 3 Bin 70|5|JFMXT02.73JK",
            "39241|7|L3ULEV70|California LEV-III ULEV70|5|JFMXT02.73JK",
            "41189|3|T3B0|Federal Tier 3 Bin 0|10|KTSLV00.0L13",
            "41189|7|ZEV|California ZEV|10|KTSLV00.0L13",
        ]
    );
    assert_eq!(
        rows(&shop, "SELECT count(*) FROM emission_standard"),
        ["16"]
    );
}

#[test]
fn nrcan_rows_are_stored_with_the_credit_line() {
    let scratch = Scratch::new("fuel-nrcan");
    add_engines(&scratch);
    let directory = nrcan_fuel_dir(&scratch);
    let summary = build_with(
        &scratch.core(),
        &scratch.out(),
        &scratch.curated(),
        &info(),
        |context| shape_nrcan(context, &directory),
    )
    .unwrap();
    let entry = summary.datasets.get(Dataset::FuelCa).unwrap();
    assert_eq!(entry.rows, 22);
    assert_eq!(entry.as_of, "2026-10-06");
    assert_eq!(entry.licence, "ogl-canada");
    assert_eq!(
        entry.attribution,
        "Contains information licensed under the Open Government Licence \u{2013} Canada."
    );
    let shop = Connection::open(scratch.out()).unwrap();
    assert_eq!(
        rows(
            &shop,
            "SELECT source, source_id, year, model_id, \"match\", engine_id, confidence, drive_text,
                    displacement_l, transmission, drive, fuel, city_l_100km, highway_l_100km,
                    combined_l_100km, combined_mpg, kwh_100km, range_km, co2_g_km
             FROM fuel_rating
             WHERE model_text IN ('F-150', 'Model 3 Long Range', 'Prius Plug-in Hybrid', '1500 4X4')
               AND (displacement_l IS NULL OR displacement_l IN (2.7, 1.8, 5.7))
             ORDER BY year, model_text"
        ),
        [
            "nrcan|-|2013|2209|base|120815|engine|-|1.8|AV|-|Regular Gasoline|4.7|4.8|4.7|50|18|18|101",
            "nrcan|-|2018|1801|exact|144955|engine|-|2.7|AS10|-|Regular Gasoline|11.9|9|10.6|22|-|-|249",
            "nrcan|-|2019|13620|exact|148370|engine|4WD|5.7|A8|-|Regular Gasoline|16.1|11|13.8|17|-|-|325",
            "nrcan|-|2019|-|-|-|-|-|-|A1|-|Electricity|-|-|-|-|16.1|499|0",
        ]
    );
}

#[test]
fn the_flags_build_every_fuel_dataset_together() {
    let scratch = Scratch::new("fuel-flags");
    add_engines(&scratch);
    add_oil_file(&scratch);
    let inputs = ShopInputs {
        epa_vehicles: Some(epa_vehicles_dir(&scratch)),
        epa_guide: Some(epa_guide_dir(&scratch)),
        epa_emissions: Some(epa_emissions_dir(&scratch)),
        nrcan_fuel: Some(nrcan_fuel_dir(&scratch)),
        ..ShopInputs::default()
    };
    let summary = build_shop(
        &scratch.core(),
        &scratch.out(),
        &scratch.curated(),
        &info(),
        &inputs,
    )
    .unwrap();
    let ids: Vec<&str> = summary
        .datasets
        .iter()
        .map(|(dataset, _)| dataset.id())
        .collect();
    assert_eq!(ids, ["fuel_us", "fuel_ca", "specs", "oil_grade"]);
    let shop = Connection::open(scratch.out()).unwrap();
    assert_eq!(
        rows(
            &shop,
            "SELECT source, count(*) FROM fuel_rating GROUP BY source ORDER BY source"
        ),
        ["epa|29", "nrcan|22"]
    );

    // The guide and the emissions file hang on vehicles.csv.
    std::fs::remove_file(scratch.out()).unwrap();
    let alone = ShopInputs {
        epa_guide: Some(epa_guide_dir(&scratch)),
        ..ShopInputs::default()
    };
    let error = build_shop(
        &scratch.core(),
        &scratch.out(),
        &scratch.curated(),
        &info(),
        &alone,
    )
    .unwrap_err();
    assert!(
        format!("{error:#}").contains("need --epa-vehicles"),
        "{error:#}"
    );
    assert!(!scratch.out().exists());

    // Without the guide there is no oil dataset, and the guide columns are empty.
    let plain = ShopInputs {
        epa_vehicles: Some(epa_vehicles_dir(&scratch)),
        ..ShopInputs::default()
    };
    let summary = build_shop(
        &scratch.core(),
        &scratch.out(),
        &scratch.curated(),
        &info(),
        &plain,
    )
    .unwrap();
    assert!(!summary.datasets.contains(Dataset::OilGrade));
    let shop = Connection::open(scratch.out()).unwrap();
    assert_eq!(
        rows(&shop, "SELECT count(*), count(gears) FROM vehicle_spec"),
        ["29|0"]
    );
    assert_eq!(rows(&shop, "SELECT count(*) FROM oil_grade"), ["0"]);
}

#[test]
fn an_oil_spelling_nobody_has_read_stops_the_build() {
    let scratch = Scratch::new("fuel-oil-unread");
    add_engines(&scratch);
    add_oil_file(&scratch);
    let (vehicles, guide) = (epa_vehicles_dir(&scratch), epa_guide_dir(&scratch));
    // Rewrite one year's file with a spelling the pattern cannot read.
    let text = std::fs::read_to_string(crate::common::fixture("epa-guide").join("2020.txt"))
        .unwrap()
        .replace("|0W20\n", "|thin oil\n");
    let sheet: Vec<Vec<String>> = text
        .lines()
        .map(|line| line.split('|').map(str::to_owned).collect())
        .collect();
    crate::common::zip_bytes(
        &guide.join("20data.zip"),
        "2020.xlsx",
        &crate::common::xlsx_bytes(&sheet),
    );
    let error = build_with(
        &scratch.core(),
        &scratch.out(),
        &scratch.curated(),
        &info(),
        |context| shape_epa(context, &vehicles, Some(&guide), None),
    )
    .unwrap_err();
    let message = format!("{error:#}");
    assert!(message.contains("\"thin oil\""), "{message}");
    assert!(message.contains("oil-grades.yaml"), "{message}");
}
