//! The fuel shape steps: EPA's files and NRCan's ratings into the tables
//! of `wenmar_shopdata::schema::FUEL`.

use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::path::Path;

use anyhow::{Context, Result, bail};
use rusqlite::{Transaction, params};
use wenmar_shopdata::{Dataset, DatasetEntry, licence, schema};

use crate::shop::build::ShapeContext;
use crate::shop::epa_emissions::{self, EmissionRow};
use crate::shop::epa_guide::{self, GuideJoin, GuideRow};
use crate::shop::epa_match::{CatalogEngines, EngineFacts, VehicleMatch, match_vehicle};
use crate::shop::epa_vehicles::{self, EpaVehicleRow};
use crate::shop::nrcan_fuel::{self, NrcanKind, NrcanRow};
use crate::shop::oil_grade::{self, CuratedOil};
use crate::shop::recalls::OGL_CANADA_ATTRIBUTION;
use crate::sources::SourceRecord;

/// `fuel_rating.source` of an EPA row and of an NRCan row.
pub const EPA: &str = "epa";
pub const NRCAN: &str = "nrcan";

/// What joins several grades in `oil_grade.grades`.
pub const GRADE_SEPARATOR: &str = ", ";

/// Litres per 100 km times miles per US gallon.
pub const L_100KM_MPG: f64 = 235.214_583;
pub const KM_PER_MILE: f64 = 1.609_344;

/// L/100 km of a figure in miles per US gallon, to one decimal.
pub fn l_100km(mpg: u16) -> Option<f64> {
    (mpg > 0).then(|| (L_100KM_MPG / f64::from(mpg) * 10.0).round() / 10.0)
}

/// Miles per US gallon of a figure in L/100 km, to a whole number.
pub fn mpg(l_100km: f64) -> Option<i64> {
    (l_100km > 0.0).then(|| (L_100KM_MPG / l_100km).round() as i64)
}

/// Whether a fuel is sold by the litre, so that L/100 km means something.
pub fn is_liquid(fuel: &str) -> bool {
    matches!(
        fuel,
        "Regular Gasoline" | "Midgrade Gasoline" | "Premium Gasoline" | "Diesel" | "E85"
    )
}

pub fn create_tables(transaction: &Transaction<'_>) -> Result<()> {
    transaction.execute_batch(schema::FUEL)?;
    Ok(())
}

/// One rating, as it is stored. The matched columns come from `matched`.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Rating<'a> {
    pub source: &'a str,
    pub source_id: Option<u32>,
    pub year: i32,
    pub make_text: &'a str,
    pub model_text: &'a str,
    pub displacement_l: Option<f64>,
    pub cylinders: Option<u8>,
    pub transmission: Option<&'a str>,
    pub drive: Option<&'a str>,
    pub fuel: &'a str,
    pub fuel2: Option<&'a str>,
    pub city_l_100km: Option<f64>,
    pub highway_l_100km: Option<f64>,
    pub combined_l_100km: Option<f64>,
    pub city_mpg: Option<i64>,
    pub highway_mpg: Option<i64>,
    pub combined_mpg: Option<i64>,
    pub kwh_100km: Option<f64>,
    pub range_km: Option<i64>,
    pub co2_g_km: Option<i64>,
}

/// Writes a rating: one row per catalog model it matched, and one row
/// with no model when it matched none.
pub fn insert_rating(
    transaction: &Transaction<'_>,
    rating: &Rating<'_>,
    matched: &VehicleMatch,
) -> Result<()> {
    let resolved = &matched.model.resolved;
    let models: Vec<Option<i64>> = if resolved.model_ids.is_empty() {
        vec![None]
    } else {
        resolved.model_ids.iter().copied().map(Some).collect()
    };
    let mut statement = transaction.prepare_cached(
        "INSERT INTO fuel_rating (source, source_id, year, make_id, model_id, make_text,
            model_text, \"match\", engine_id, submodel_id, confidence, drive_text, body_text,
            note_text, displacement_l, cylinders, transmission, drive, fuel, fuel2,
            city_l_100km, highway_l_100km, combined_l_100km, city_mpg, highway_mpg,
            combined_mpg, kwh_100km, range_km, co2_g_km)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16, ?17,
            ?18, ?19, ?20, ?21, ?22, ?23, ?24, ?25, ?26, ?27, ?28, ?29)",
    )?;
    for model_id in models {
        statement.execute(params![
            rating.source,
            rating.source_id,
            rating.year,
            resolved.make_id,
            model_id,
            rating.make_text,
            rating.model_text,
            resolved.match_text(),
            matched.engine_id,
            matched.submodel_id,
            matched.confidence.map(|confidence| confidence.as_str()),
            matched.model.name.drive,
            matched.model.name.body,
            matched.model.name.note,
            rating.displacement_l,
            rating.cylinders,
            rating.transmission,
            rating.drive,
            rating.fuel,
            rating.fuel2,
            rating.city_l_100km,
            rating.highway_l_100km,
            rating.combined_l_100km,
            rating.city_mpg,
            rating.highway_mpg,
            rating.combined_mpg,
            rating.kwh_100km,
            rating.range_km,
            rating.co2_g_km,
        ])?;
    }
    Ok(())
}

/// What `vehicles.csv` says of a row's engine.
pub fn epa_facts(row: &EpaVehicleRow) -> EngineFacts {
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

/// What an NRCan rating says of its engine. NRCan does not say whether
/// an engine has a turbocharger, and marks an ordinary hybrid only in
/// the model name.
pub fn nrcan_facts(row: &NrcanRow) -> EngineFacts {
    EngineFacts {
        displacement_l: row.displacement_l,
        cylinders: row.cylinders,
        diesel: row.fuel == "Diesel",
        turbo: None,
        electric: row.kind == NrcanKind::Electric,
        hybrid: nrcan_fuel::named_hybrid(&row.model_text),
        plug_in: row.kind == NrcanKind::PlugIn,
    }
}

/// The first value of a guide column among a vehicle's guide rows.
fn first<T: Clone>(rows: &[&GuideRow], field: impl Fn(&GuideRow) -> Option<T>) -> Option<T> {
    rows.iter().find_map(|row| field(row))
}

/// EPA: `vehicles.csv` into `fuel_rating` and `vehicle_spec`
/// (`fuel_us`, `specs`); with the guide files, the guide columns of
/// `vehicle_spec` and `oil_grade` (`oil_grade`); with the emissions file,
/// `emission_standard`.
pub fn shape_epa(
    context: &mut ShapeContext<'_>,
    vehicles_dir: &Path,
    guide_dir: Option<&Path>,
    emissions_dir: Option<&Path>,
) -> Result<()> {
    let as_of = SourceRecord::read(vehicles_dir)?.as_of;
    let mut vehicles: Vec<EpaVehicleRow> = Vec::new();
    epa_vehicles::parse(vehicles_dir, &mut |row| {
        vehicles.push(row);
        Ok(())
    })?;
    let known: BTreeSet<u32> = vehicles.iter().map(|vehicle| vehicle.id).collect();
    if known.len() != vehicles.len() {
        bail!("vehicles.csv gives the same id to two rows");
    }

    // Guide rows by the vehicle each describes.
    let mut guide: HashMap<u32, Vec<GuideRow>> = HashMap::new();
    let mut guide_as_of = None;
    if let Some(directory) = guide_dir {
        guide_as_of = Some(SourceRecord::read(directory)?.as_of);
        let join = GuideJoin::new(&vehicles);
        epa_guide::parse(directory, &mut |row| {
            if let Some(id) = join.find(&row) {
                guide.entry(id).or_default().push(row);
            }
            Ok(())
        })?;
    }

    create_tables(context.transaction)?;
    let engines = CatalogEngines::load(context.catalog)?;
    let transaction = context.transaction;
    let mut spec = transaction.prepare_cached(
        "INSERT INTO vehicle_spec VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12,
            ?13, ?14, ?15, ?16, ?17, ?18, ?19, ?20, ?21, ?22, ?23, ?24, ?25, ?26)",
    )?;
    for vehicle in &vehicles {
        let matched = match_vehicle(
            context.matcher,
            &engines,
            vehicle.year,
            &vehicle.make_text,
            &vehicle.model_text,
            &vehicle.base_model,
            &epa_facts(vehicle),
        );
        let fuel = vehicle.fuel1.as_deref().unwrap_or(&vehicle.fuel);
        let liquid = is_liquid(fuel);
        let figure = |mpg: u16| liquid.then_some(mpg).filter(|mpg| *mpg > 0);
        insert_rating(
            transaction,
            &Rating {
                source: EPA,
                source_id: Some(vehicle.id),
                year: vehicle.year,
                make_text: &vehicle.make_text,
                model_text: &vehicle.model_text,
                displacement_l: vehicle.displacement_l,
                cylinders: vehicle.cylinders,
                transmission: vehicle.transmission.as_deref(),
                drive: vehicle.drive.as_deref(),
                fuel,
                fuel2: vehicle.fuel2.as_deref(),
                city_l_100km: figure(vehicle.city_mpg).and_then(l_100km),
                highway_l_100km: figure(vehicle.highway_mpg).and_then(l_100km),
                combined_l_100km: figure(vehicle.combined_mpg).and_then(l_100km),
                city_mpg: figure(vehicle.city_mpg).map(i64::from),
                highway_mpg: figure(vehicle.highway_mpg).map(i64::from),
                combined_mpg: figure(vehicle.combined_mpg).map(i64::from),
                kwh_100km: vehicle
                    .kwh_100mi
                    .map(|kwh| (kwh / KM_PER_MILE * 10.0).round() / 10.0),
                // The range on a full battery: `range` when electricity is
                // the main fuel, `rangeA` when it is the second.
                range_km: if fuel == "Electricity" {
                    vehicle.range_mi
                } else if vehicle.fuel2.as_deref() == Some("Electricity") {
                    vehicle.range2_mi
                } else {
                    None
                }
                .map(|miles| (f64::from(miles) * KM_PER_MILE).round() as i64),
                co2_g_km: (liquid || vehicle.co2_g_mi == 0.0)
                    .then(|| (vehicle.co2_g_mi / KM_PER_MILE).round() as i64),
            },
            &matched,
        )
        .with_context(|| format!("storing EPA vehicle {}", vehicle.id))?;

        let rows: Vec<&GuideRow> = guide
            .get(&vehicle.id)
            .map(|rows| rows.iter().collect())
            .unwrap_or_default();
        spec.execute(params![
            vehicle.id,
            vehicle.size_class,
            vehicle.turbo,
            vehicle.supercharger,
            vehicle.start_stop,
            vehicle.atv_type,
            vehicle.ev_motor,
            vehicle.charge_240v_h,
            vehicle.passenger_ft3_2door,
            vehicle.passenger_ft3_4door,
            vehicle.passenger_ft3_hatch,
            vehicle.luggage_ft3_2door,
            vehicle.luggage_ft3_4door,
            vehicle.luggage_ft3_hatch,
            first(&rows, |row| row.gears),
            first(&rows, |row| row.transmission_type.clone()),
            first(&rows, |row| row.fuel_usage.clone()),
            first(&rows, |row| row.cylinder_deactivation),
            first(&rows, |row| row.cylinder_deactivation_name.clone()),
            first(&rows, |row| row.variable_valve_timing),
            first(&rows, |row| row.variable_valve_lift),
            first(&rows, |row| row.intake_valves),
            first(&rows, |row| row.exhaust_valves),
            first(&rows, |row| row.battery_type.clone()),
            first(&rows, |row| row.battery_voltage),
            first(&rows, |row| row.motor_kw.clone()),
        ])?;
    }
    drop(spec);
    context.register(
        Dataset::FuelUs,
        DatasetEntry {
            as_of: as_of.clone(),
            rows: vehicles.len() as u64,
            licence: licence::PUBLIC_DOMAIN.to_owned(),
            attribution: String::new(),
        },
    )?;
    context.register(
        Dataset::Specs,
        DatasetEntry {
            as_of: as_of.clone(),
            rows: vehicles.len() as u64,
            licence: licence::PUBLIC_DOMAIN.to_owned(),
            attribution: String::new(),
        },
    )?;

    if let Some(guide_as_of) = guide_as_of {
        let curated = CuratedOil::load(context.curated)?;
        // Each vehicle's distinct oil texts, in id order.
        let mut oils: BTreeMap<u32, Vec<(String, String)>> = BTreeMap::new();
        for (id, rows) in &guide {
            for text in rows.iter().filter_map(|row| row.oil_text.as_deref()) {
                let Some(grades) = oil_grade::read(text, &curated) else {
                    bail!(
                        "the guide files have an oil grade this build cannot read: {text:?}; add it to shop/oil-grades.yaml"
                    );
                };
                let line = (grades.join(GRADE_SEPARATOR), text.to_owned());
                let lines = oils.entry(*id).or_default();
                if !grades.is_empty() && !lines.contains(&line) {
                    lines.push(line);
                }
            }
        }
        let mut insert = context
            .transaction
            .prepare_cached("INSERT INTO oil_grade VALUES (?1, ?2, ?3)")?;
        let mut stored = 0u64;
        for (id, lines) in &oils {
            for (grades, text) in lines {
                insert.execute(params![id, grades, text])?;
                stored += 1;
            }
        }
        drop(insert);
        context.register(
            Dataset::OilGrade,
            DatasetEntry {
                as_of: guide_as_of,
                rows: stored,
                licence: licence::PUBLIC_DOMAIN.to_owned(),
                attribution: String::new(),
            },
        )?;
    }

    if let Some(directory) = emissions_dir {
        let mut rows: Vec<EmissionRow> = Vec::new();
        epa_emissions::parse(directory, &mut |row| {
            // A few rows name a vehicle that vehicles.csv no longer has.
            if known.contains(&row.epa_id) {
                rows.push(row);
            }
            Ok(())
        })?;
        let mut insert = context
            .transaction
            .prepare_cached("INSERT INTO emission_standard VALUES (?1, ?2, ?3, ?4, ?5, ?6)")?;
        for row in epa_emissions::distinct(rows) {
            insert.execute(params![
                row.epa_id,
                row.sales_area,
                row.standard,
                row.standard_text,
                row.smog_score,
                row.engine_family,
            ])?;
        }
    }
    Ok(())
}

/// NRCan: the ratings into `fuel_rating` (`fuel_ca`).
pub fn shape_nrcan(context: &mut ShapeContext<'_>, directory: &Path) -> Result<()> {
    let as_of = SourceRecord::read(directory)?.as_of;
    create_tables(context.transaction)?;
    let engines = CatalogEngines::load(context.catalog)?;
    let (transaction, matcher) = (context.transaction, context.matcher);
    let mut stored = 0u64;
    nrcan_fuel::parse(directory, &mut |row| {
        // NRCan has no base model, so the model text stands in for it.
        let matched = match_vehicle(
            matcher,
            &engines,
            row.year,
            &row.make_text,
            &row.model_text,
            &row.model_text,
            &nrcan_facts(&row),
        );
        stored += 1;
        insert_rating(
            transaction,
            &Rating {
                source: NRCAN,
                source_id: None,
                year: row.year,
                make_text: &row.make_text,
                model_text: &row.model_text,
                displacement_l: row.displacement_l,
                cylinders: row.cylinders,
                transmission: Some(row.transmission.as_str()).filter(|code| !code.is_empty()),
                drive: None,
                fuel: &row.fuel,
                fuel2: None,
                city_l_100km: row.city_l_100km,
                highway_l_100km: row.highway_l_100km,
                combined_l_100km: row.combined_l_100km,
                city_mpg: row.city_l_100km.and_then(mpg),
                highway_mpg: row.highway_l_100km.and_then(mpg),
                combined_mpg: row.combined_l_100km.and_then(mpg),
                kwh_100km: row.kwh_100km,
                range_km: row.range_km.map(i64::from),
                co2_g_km: row.co2_g_km.map(i64::from),
            },
            &matched,
        )
    })?;
    context.register(
        Dataset::FuelCa,
        DatasetEntry {
            as_of,
            rows: stored,
            licence: licence::OGL_CANADA.to_owned(),
            attribution: OGL_CANADA_ATTRIBUTION.to_owned(),
        },
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn units_are_converted_once_at_build() {
        assert_eq!(l_100km(22), Some(10.7));
        assert_eq!(l_100km(48), Some(4.9));
        assert_eq!(l_100km(0), None);
        assert_eq!(mpg(10.6), Some(22));
        assert_eq!(mpg(5.0), Some(47));
        assert_eq!(mpg(0.0), None);
        assert!(is_liquid("Premium Gasoline") && is_liquid("Diesel") && is_liquid("E85"));
        assert!(!is_liquid("Electricity") && !is_liquid("Hydrogen") && !is_liquid("Natural Gas"));
    }
}
