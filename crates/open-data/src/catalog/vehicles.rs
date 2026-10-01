//! The years, makes and models of the catalog.

use std::collections::{BTreeMap, HashMap, HashSet};

use anyhow::{Context, Result};
use rusqlite::{Transaction, params};
use wenmar_vehicles::FIRST_YEAR;
use wenmar_vehicles::schema::LIGHT_TYPES;
use wenmar_vehicles::text::{normalize, slug, squeeze};

use crate::catalog::CatalogSummary;
use crate::catalog::curated::Curated;

/// vPIC's element id for the model.
const MODEL_ELEMENT: i64 = 28;

/// What the model years of one make add up to.
#[derive(Debug, Clone, Copy, Default)]
struct MakeTotals {
    types: i64,
    light: bool,
}

/// What the model years of one model add up to.
#[derive(Debug, Clone, Copy)]
struct ModelTotals {
    make_id: i64,
    types: i64,
    light: bool,
    year_from: i64,
    year_to: i64,
}

/// The rank and spelling a curated make gives a vPIC make.
struct Ranked<'a> {
    rank: i64,
    name: &'a str,
}

/// Gives each name a distinct id form. `names` is in id order: the first to
/// ask for an id form keeps it, and later ones get `-2`, `-3` and so on. A
/// name with no letters or digits gets `<fallback>-<id>`.
pub fn assign_slugs(names: &[(i64, String)], fallback: &str) -> HashMap<i64, String> {
    let mut taken: HashSet<String> = HashSet::new();
    let mut slugs = HashMap::with_capacity(names.len());
    for (id, name) in names {
        let mut base = slug(name);
        if base.is_empty() {
            base = format!("{fallback}-{id}");
        }
        let mut candidate = base.clone();
        let mut suffix = 2;
        while !taken.insert(candidate.clone()) {
            candidate = format!("{base}-{suffix}");
            suffix += 1;
        }
        slugs.insert(*id, candidate);
    }
    slugs
}

/// Fills `catalog_type`, `catalog_vehicle`, `catalog_make`, `catalog_alias`
/// and `catalog_model`.
pub fn build(
    transaction: &Transaction<'_>,
    curated: &Curated,
    last_year: u16,
    summary: &mut CatalogSummary,
) -> Result<()> {
    transaction
        .execute(
            "INSERT INTO catalog_type (id, name)
             SELECT CAST(id AS INTEGER), name FROM raw_vehicletype
             WHERE id IS NOT NULL AND name IS NOT NULL",
            [],
        )
        .context("listing the vehicle types")?;
    insert_vehicles(transaction, last_year)?;
    let (makes, models) = totals(transaction)?;
    let make_names = names(transaction, "raw_make")?;
    let model_names = names(transaction, "raw_model")?;

    // Only what is both in the catalog and named, in id order.
    let named_makes: Vec<(i64, String)> = makes
        .keys()
        .filter_map(|id| Some((*id, make_names.get(id)?.clone())))
        .collect();
    let make_slugs = assign_slugs(&named_makes, "make");
    let mut models_of_make: BTreeMap<i64, Vec<(i64, String)>> = BTreeMap::new();
    for (id, model) in &models {
        if let Some(name) = model_names.get(id) {
            models_of_make
                .entry(model.make_id)
                .or_default()
                .push((*id, name.clone()));
        }
    }
    let mut model_slugs: HashMap<i64, String> = HashMap::with_capacity(models.len());
    for names in models_of_make.values() {
        model_slugs.extend(assign_slugs(names, "model"));
    }

    let mut ranked: HashMap<i64, Ranked<'_>> = HashMap::new();
    let mut aliases: Vec<(&str, i64)> = Vec::new();
    for (rank, curated_make) in (1_i64..).zip(&curated.makes) {
        let form = normalize(&curated_make.name);
        // Makes are in id order, so the first of each kind has the lowest id.
        let matching = || {
            named_makes
                .iter()
                .filter(|(_, name)| !form.is_empty() && normalize(name) == form)
                .map(|(id, _)| *id)
        };
        let light = matching().find(|id| makes.get(id).is_some_and(|make| make.light));
        let Some(id) = light.or_else(|| matching().next()) else {
            summary
                .unmatched
                .push(format!("make {}", curated_make.name));
            continue;
        };
        ranked.entry(id).or_insert(Ranked {
            rank,
            name: &curated_make.name,
        });
        aliases.extend(
            curated_make
                .aliases
                .iter()
                .map(|alias| (alias.as_str(), id)),
        );
    }

    let mut insert_make = transaction.prepare(
        "INSERT INTO catalog_make (id, slug, name, norm, rank, types, light)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
    )?;
    for (id, name) in &named_makes {
        let (Some(make), Some(slug)) = (makes.get(id), make_slugs.get(id)) else {
            continue;
        };
        let ranked = ranked.get(id);
        insert_make
            .execute(params![
                id,
                slug,
                ranked.map_or(name.as_str(), |ranked| ranked.name),
                normalize(name),
                ranked.map(|ranked| ranked.rank),
                make.types,
                make.light,
            ])
            .with_context(|| format!("adding the make {name}"))?;
        summary.makes += 1;
    }

    let mut insert_alias =
        transaction.prepare("INSERT INTO catalog_alias (norm, make_id) VALUES (?1, ?2)")?;
    for (alias, make_id) in aliases {
        let form = normalize(alias);
        if !form.is_empty() {
            insert_alias.execute(params![form, make_id])?;
        }
    }

    let mut insert_model = transaction.prepare(
        "INSERT INTO catalog_model
             (id, make_id, slug, name, norm, year_from, year_to, types, light)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
    )?;
    for (id, model) in &models {
        let (Some(name), Some(slug)) = (model_names.get(id), model_slugs.get(id)) else {
            continue;
        };
        insert_model
            .execute(params![
                id,
                model.make_id,
                slug,
                name,
                normalize(name),
                model.year_from,
                model.year_to,
                model.types,
                model.light,
            ])
            .with_context(|| format!("adding the model {name}"))?;
        summary.models += 1;
    }

    let (vehicles, light): (i64, i64) = transaction.query_row(
        "SELECT COUNT(*), COALESCE(SUM(light), 0) FROM catalog_vehicle",
        [],
        |row| Ok((row.get(0)?, row.get(1)?)),
    )?;
    summary.vehicles = u64::try_from(vehicles).unwrap_or(0);
    summary.light_vehicles = u64::try_from(light).unwrap_or(0);
    Ok(())
}

/// One row per year, make and model. A model is listed for every year one of
/// its schemas is linked to a manufacturer code, which is how NHTSA's
/// `GetModelsForMakeYear` behaves. A manufacturer code with no vehicle type
/// sets bit 0 of `types`.
fn insert_vehicles(transaction: &Transaction<'_>, last_year: u16) -> Result<()> {
    let light_types = LIGHT_TYPES.map(|id| id.to_string()).join(", ");
    // SQLite has no aggregate for a bitwise OR; a sum over distinct powers
    // of two is one.
    let sql = format!(
        "WITH RECURSIVE years(year) AS (
             SELECT ?1 UNION ALL SELECT year + 1 FROM years WHERE year < ?2
         )
         INSERT INTO catalog_vehicle (year, make_id, model_id, types, light)
         SELECT y.year,
                CAST(k.id AS INTEGER),
                CAST(p.attribute AS INTEGER),
                SUM(DISTINCT 1 << COALESCE(w.vehicle_type_id, 0)),
                MAX(COALESCE(w.vehicle_type_id, 0) IN ({light_types}))
         FROM pattern p
         JOIN wmi_schema s ON s.schema_id = p.schema_id
         JOIN wmi w ON w.code = s.wmi
         JOIN raw_make k ON k.name = p.make
         JOIN raw_model d ON d.id = p.attribute
         JOIN years y ON y.year >= s.year_from AND y.year <= COALESCE(s.year_to, ?2)
         WHERE p.element_id = {MODEL_ELEMENT} AND d.name IS NOT NULL
         GROUP BY y.year, CAST(k.id AS INTEGER), CAST(p.attribute AS INTEGER)
         ORDER BY 1, 2, 3"
    );
    transaction
        .execute(&sql, params![FIRST_YEAR, last_year])
        .context("listing the model years")?;
    Ok(())
}

/// Adds up the model years of each make and of each model, both in id order.
fn totals(
    transaction: &Transaction<'_>,
) -> Result<(BTreeMap<i64, MakeTotals>, BTreeMap<i64, ModelTotals>)> {
    let mut makes: BTreeMap<i64, MakeTotals> = BTreeMap::new();
    let mut models: BTreeMap<i64, ModelTotals> = BTreeMap::new();
    // The type masks must be OR-ed, which SQLite cannot do in a GROUP BY.
    let mut statement = transaction.prepare(
        "SELECT make_id, model_id, types, light, MIN(year), MAX(year)
         FROM catalog_vehicle GROUP BY make_id, model_id, types, light
         ORDER BY make_id, model_id",
    )?;
    let mut rows = statement.query([])?;
    while let Some(row) = rows.next()? {
        let (make_id, model_id): (i64, i64) = (row.get(0)?, row.get(1)?);
        let (types, light): (i64, bool) = (row.get(2)?, row.get(3)?);
        let (year_from, year_to): (i64, i64) = (row.get(4)?, row.get(5)?);
        let make = makes.entry(make_id).or_default();
        make.types |= types;
        make.light |= light;
        // A model belongs to the first make it is seen under.
        let model = models.entry(model_id).or_insert(ModelTotals {
            make_id,
            types: 0,
            light: false,
            year_from,
            year_to,
        });
        model.types |= types;
        model.light |= light;
        model.year_from = model.year_from.min(year_from);
        model.year_to = model.year_to.max(year_to);
    }
    Ok((makes, models))
}

/// The names in a staging table by id, with stray spaces removed. A row
/// without an id or a name is left out.
fn names(transaction: &Transaction<'_>, table: &str) -> Result<HashMap<i64, String>> {
    let mut statement = transaction.prepare(&format!(
        "SELECT CAST(id AS INTEGER), name FROM {table}
         WHERE id IS NOT NULL AND name IS NOT NULL"
    ))?;
    let mut names = HashMap::new();
    let mut rows = statement.query([])?;
    while let Some(row) = rows.next()? {
        let (id, name): (i64, String) = (row.get(0)?, row.get(1)?);
        names.entry(id).or_insert_with(|| squeeze(&name));
    }
    Ok(names)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_first_name_keeps_the_plain_id_form() {
        let names = vec![
            (600, "B & B Trailers".to_owned()),
            (601, "B+B Trailers".to_owned()),
            (602, "B B Trailers 2".to_owned()),
            (603, "B-B Trailers".to_owned()),
        ];
        let slugs = assign_slugs(&names, "make");
        assert_eq!(slugs[&600], "b-b-trailers");
        assert_eq!(slugs[&601], "b-b-trailers-2");
        // Its own id form is already taken by 601.
        assert_eq!(slugs[&602], "b-b-trailers-2-2");
        assert_eq!(slugs[&603], "b-b-trailers-3");
    }

    #[test]
    fn a_name_with_no_letters_or_digits_still_gets_an_id_form() {
        let names = vec![(7, "***".to_owned()), (8, "".to_owned())];
        let slugs = assign_slugs(&names, "model");
        assert_eq!(slugs[&7], "model-7");
        assert_eq!(slugs[&8], "model-8");
    }

    #[test]
    fn the_last_year_is_the_year_after_the_build() {
        assert_eq!(
            crate::catalog::last_year("2026-09-30 12:00:00").unwrap(),
            2027
        );
        for bad in ["", "soon", "26-09-30", "9999-01-01 00:00:00"] {
            assert!(crate::catalog::last_year(bad).is_err(), "{bad}");
        }
    }
}
