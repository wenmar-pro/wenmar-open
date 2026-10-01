//! Derives submodels and engines for every model year and writes them.

use std::collections::{BTreeMap, BTreeSet, HashMap};

use anyhow::{Context, Result, bail};
use rusqlite::{Transaction, params};
use wenmar_vehicles::FIRST_YEAR;
use wenmar_vehicles::summary::short_drive;
use wenmar_vehicles::text::normalize;

use crate::catalog::CatalogSummary;
use crate::catalog::cells::{CATALOG_ELEMENTS, CELL_CAP, Row, cells};
use crate::catalog::curated::{Curated, Preset};
use crate::catalog::detail::{Detail, assemble};
use crate::catalog::engine::{ENGINE_ELEMENTS, ENGINE_MODEL_ELEMENT, Facts, label};
use crate::catalog::keys::Key;
use crate::catalog::names::{Speller, split_trims};
use crate::catalog::tally::{Interner, Outcome, Tally};

/// Engine facts by engine-model name (trimmed, lowercase), most recent
/// change first.
pub(crate) type EngineModels = HashMap<String, Vec<(i64, String)>>;

/// The schemas that apply together to a manufacturer code in a model year,
/// each with the first year of its link, in schema order.
type SchemaSet = Vec<(i64, u16)>;

/// One link of a schema to a manufacturer code: the schema, its first year
/// and its last year, if it has one.
type Link = (i64, i64, Option<i64>);

/// The model rows of each schema as `(keys, make, model)`.
type ModelRows = HashMap<i64, Vec<(Key, i64, i64)>>;

/// A model year: the year, the make and the model.
type Vehicle = (u16, i64, i64);

/// What one cell decodes to. `labels` remembers the label of each set of
/// engine facts already seen, because millions of cells share a few
/// thousand engines.
pub(crate) fn outcome<'a>(
    winners: &[&'a Row],
    vin8: u64,
    engine_models: &'a EngineModels,
    names: &mut Interner,
    labels: &mut HashMap<Facts<'a>, Option<u32>>,
) -> Outcome {
    let mut outcome = Outcome {
        vin8,
        ..Outcome::default()
    };
    let mut facts = Facts::default();
    let mut engine_model = None;
    for row in winners {
        match row.element {
            38 => outcome.trim = Some(names.intern(&row.value)),
            34 => outcome.series = Some(names.intern(&row.value)),
            5 => outcome.body = Some(names.intern(&row.value)),
            15 => outcome.drive = Some(names.intern(&short_drive(&row.value))),
            37 => outcome.transmission = Some(names.intern(&row.value)),
            ENGINE_MODEL_ELEMENT => engine_model = Some(row.value.as_str()),
            element => facts.fill(element, &row.value),
        }
    }
    // As in a decode, the engine model only fills in what the patterns left out.
    if let Some(rows) = engine_model.and_then(|name| engine_models.get(&name.trim().to_lowercase()))
    {
        for (element, value) in rows {
            facts.fill(*element, value);
        }
    }
    outcome.engine = *labels
        .entry(facts)
        .or_insert_with(|| label(&facts).map(|label| names.intern(&label)));
    outcome
}

/// The decoder's rule: vPIC marks attributes that do not apply with a
/// placeholder value, and an empty value says nothing.
fn usable(value: &str) -> Option<&str> {
    let value = value.trim();
    (!value.is_empty() && !value.eq_ignore_ascii_case("not applicable")).then_some(value)
}

fn id_list(ids: &[i64]) -> String {
    ids.iter()
        .map(i64::to_string)
        .collect::<Vec<_>>()
        .join(", ")
}

/// The links of every manufacturer code, in code order.
fn load_links(transaction: &Transaction<'_>) -> Result<BTreeMap<String, Vec<Link>>> {
    let mut links: BTreeMap<String, Vec<Link>> = BTreeMap::new();
    let mut statement =
        transaction.prepare("SELECT wmi, schema_id, year_from, year_to FROM wmi_schema")?;
    let mut rows = statement.query([])?;
    while let Some(row) = rows.next()? {
        let wmi: String = row.get(0)?;
        links
            .entry(wmi)
            .or_default()
            .push((row.get(1)?, row.get(2)?, row.get(3)?));
    }
    Ok(links)
}

/// The model rows of the models in the catalog.
fn load_models(transaction: &Transaction<'_>) -> Result<ModelRows> {
    let mut makes: HashMap<i64, i64> = HashMap::new();
    let mut statement = transaction.prepare("SELECT id, make_id FROM catalog_model")?;
    let mut rows = statement.query([])?;
    while let Some(row) = rows.next()? {
        makes.insert(row.get(0)?, row.get(1)?);
    }

    let mut models = ModelRows::new();
    let mut statement = transaction.prepare(
        "SELECT schema_id, keys, CAST(attribute AS INTEGER) FROM pattern WHERE element_id = 28",
    )?;
    let mut rows = statement.query([])?;
    while let Some(row) = rows.next()? {
        let (schema, keys): (i64, String) = (row.get(0)?, row.get(1)?);
        let model: Option<i64> = row.get(2)?;
        let Some(model) = model else {
            continue;
        };
        if let (Some(key), Some(make)) = (Key::parse(&keys), makes.get(&model)) {
            models.entry(schema).or_default().push((key, *make, model));
        }
    }
    Ok(models)
}

/// The rows of each schema that say something about a trim, a series, a
/// body, a drive type, a transmission or an engine.
fn load_rows(transaction: &Transaction<'_>) -> Result<HashMap<i64, Vec<Row>>> {
    let mut by_schema: HashMap<i64, Vec<Row>> = HashMap::new();
    let mut statement = transaction.prepare(&format!(
        "SELECT id, schema_id, keys, element_id, value, changed_on FROM pattern
         WHERE element_id IN ({})",
        id_list(&CATALOG_ELEMENTS)
    ))?;
    let mut rows = statement.query([])?;
    while let Some(row) = rows.next()? {
        let (id, schema): (i64, i64) = (row.get(0)?, row.get(1)?);
        let (keys, element): (String, i64) = (row.get(2)?, row.get(3)?);
        let (value, changed_on): (String, String) = (row.get(4)?, row.get(5)?);
        let Some(value) = usable(&value) else {
            continue;
        };
        if let Some(row) = Row::new(id, element, &keys, value, &changed_on) {
            by_schema.entry(schema).or_default().push(row);
        }
    }
    Ok(by_schema)
}

fn load_engine_models(transaction: &Transaction<'_>) -> Result<EngineModels> {
    let mut engine_models = EngineModels::new();
    let mut statement = transaction.prepare(&format!(
        "SELECT engine_model, element_id, value FROM engine_model_row
         WHERE element_id IN ({})
         ORDER BY engine_model, changed_on DESC, id",
        id_list(&ENGINE_ELEMENTS)
    ))?;
    let mut rows = statement.query([])?;
    while let Some(row) = rows.next()? {
        let (name, element): (String, i64) = (row.get(0)?, row.get(1)?);
        let value: String = row.get(2)?;
        if let Some(value) = usable(&value) {
            engine_models
                .entry(name)
                .or_default()
                .push((element, value.to_owned()));
        }
    }
    Ok(engine_models)
}

/// Every trim (lists split) and series value with the number of patterns
/// that carry it.
fn load_speller(transaction: &Transaction<'_>, curated: &Curated) -> Result<Speller> {
    let mut counted: Vec<(String, u64)> = Vec::new();
    for (element, split) in [(38, true), (34, false)] {
        let mut statement = transaction
            .prepare("SELECT value, COUNT(*) FROM pattern WHERE element_id = ?1 GROUP BY value")?;
        let mut rows = statement.query([element])?;
        while let Some(row) = rows.next()? {
            let (value, count): (String, i64) = (row.get(0)?, row.get(1)?);
            let count = u64::try_from(count).unwrap_or(0);
            let Some(value) = usable(&value) else {
                continue;
            };
            if split {
                counted.extend(split_trims(value).into_iter().map(|part| (part, count)));
            } else {
                counted.push((value.to_owned(), count));
            }
        }
    }
    Ok(Speller::new(&counted, &curated.submodel_names))
}

/// Every set of schemas a decode would use together, with the model years
/// it is used for. A decode takes every schema linked to the manufacturer
/// code for the model year and, for a schema linked twice, the later link.
fn schema_sets(
    links: &BTreeMap<String, Vec<Link>>,
    last_year: u16,
) -> BTreeMap<SchemaSet, BTreeSet<u16>> {
    let mut sets: BTreeMap<SchemaSet, BTreeSet<u16>> = BTreeMap::new();
    for links in links.values() {
        for year in FIRST_YEAR..=last_year {
            let mut latest: BTreeMap<i64, u16> = BTreeMap::new();
            for (schema, year_from, year_to) in links {
                let applies = *year_from <= i64::from(year)
                    && i64::from(year) <= year_to.unwrap_or(i64::from(last_year));
                if !applies {
                    continue;
                }
                // Not after `year`, so only a year before zero does not fit.
                let year_from = u16::try_from(*year_from).unwrap_or(0);
                let current = latest.entry(*schema).or_insert(year_from);
                *current = (*current).max(year_from);
            }
            if !latest.is_empty() {
                sets.entry(latest.into_iter().collect())
                    .or_default()
                    .insert(year);
            }
        }
    }
    sets
}

/// The preset of each model that has one, as an index into the presets.
/// The first preset to name a model keeps it. A preset that names no model
/// is added to `unmatched`.
fn match_presets(
    transaction: &Transaction<'_>,
    presets: &[Preset],
    unmatched: &mut Vec<String>,
) -> Result<HashMap<i64, usize>> {
    let mut models: HashMap<(String, String), Vec<i64>> = HashMap::new();
    let mut statement = transaction.prepare(
        "SELECT d.id, m.norm, d.norm FROM catalog_model d
         JOIN catalog_make m ON m.id = d.make_id",
    )?;
    let mut rows = statement.query([])?;
    while let Some(row) = rows.next()? {
        models
            .entry((row.get(1)?, row.get(2)?))
            .or_default()
            .push(row.get(0)?);
    }

    let mut by_model: HashMap<i64, usize> = HashMap::new();
    for (index, preset) in presets.iter().enumerate() {
        let form = (normalize(&preset.make), normalize(&preset.model));
        let matching = models
            .get(&form)
            .filter(|_| !form.0.is_empty() && !form.1.is_empty());
        let Some(matching) = matching else {
            unmatched.push(format!("preset {} {}", preset.make, preset.model));
            continue;
        };
        for model in matching {
            by_model.entry(*model).or_insert(index);
        }
    }
    Ok(by_model)
}

/// Writes one detail with its engines and submodels, in the order they are
/// listed in.
fn write_detail(transaction: &Transaction<'_>, id: i64, detail: &Detail) -> Result<()> {
    transaction
        .prepare_cached(
            "INSERT INTO catalog_detail (id, body, drive, transmission) VALUES (?1, ?2, ?3, ?4)",
        )?
        .execute(params![id, detail.body, detail.drive, detail.transmission])?;

    let mut engine_ids: HashMap<&str, i64> = HashMap::new();
    let mut insert_engine = transaction.prepare_cached(
        "INSERT INTO catalog_engine (detail_id, label, vin8, source) VALUES (?1, ?2, ?3, ?4)",
    )?;
    for engine in &detail.engines {
        let source = if engine.preset { "preset" } else { "vpic" };
        insert_engine.execute(params![id, engine.label, engine.vin8, source])?;
        engine_ids
            .entry(engine.label.as_str())
            .or_insert(transaction.last_insert_rowid());
    }

    let mut insert_submodel = transaction.prepare_cached(
        "INSERT INTO catalog_submodel
             (detail_id, name, norm, kind, listed, body, drive, transmission)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
    )?;
    let mut insert_link = transaction.prepare_cached(
        "INSERT INTO catalog_submodel_engine (submodel_id, engine_id) VALUES (?1, ?2)",
    )?;
    for submodel in &detail.submodels {
        insert_submodel.execute(params![
            id,
            submodel.name,
            normalize(&submodel.name),
            submodel.kind.as_str(),
            submodel.listed,
            submodel.body,
            submodel.drive,
            submodel.transmission,
        ])?;
        let submodel_id = transaction.last_insert_rowid();
        for label in &submodel.engines {
            if let Some(engine_id) = engine_ids.get(label.as_str()) {
                insert_link.execute(params![submodel_id, engine_id])?;
            }
        }
    }
    Ok(())
}

fn count(transaction: &Transaction<'_>, table: &str) -> Result<u64> {
    let rows: i64 = transaction.query_row(&format!("SELECT COUNT(*) FROM {table}"), [], |row| {
        row.get(0)
    })?;
    Ok(u64::try_from(rows).unwrap_or(0))
}

/// Fills `catalog_detail`, `catalog_submodel`, `catalog_engine`,
/// `catalog_submodel_engine` and `catalog_vehicle.detail_id`. Must run
/// after `vehicles::build`.
pub fn build(
    transaction: &Transaction<'_>,
    curated: &Curated,
    last_year: u16,
    summary: &mut CatalogSummary,
) -> Result<()> {
    let links = load_links(transaction).context("reading the schema links")?;
    let models = load_models(transaction).context("reading the model patterns")?;
    let rows = load_rows(transaction).context("reading the patterns")?;
    let engine_models = load_engine_models(transaction).context("reading the engine models")?;
    let speller = load_speller(transaction, curated).context("reading the trim names")?;

    // A tally per set of schemas and model, shared by the years of the set.
    let mut names = Interner::default();
    let mut tallies: Vec<Tally> = Vec::new();
    let mut by_vehicle: HashMap<Vehicle, Vec<u32>> = HashMap::new();
    for (set, years) in &schema_sets(&links, last_year) {
        let mut others: Vec<(u16, &Row)> = Vec::new();
        let mut keys: BTreeMap<(i64, i64), Vec<Key>> = BTreeMap::new();
        for (schema, year_from) in set {
            if let Some(rows) = rows.get(schema) {
                others.extend(rows.iter().map(|row| (*year_from, row)));
            }
            for (key, make, model) in models.get(schema).into_iter().flatten() {
                keys.entry((*make, *model)).or_default().push(*key);
            }
        }
        let mut labels = HashMap::new();
        for ((make, model), model_keys) in &keys {
            let mut tally = Tally::default();
            let counted = cells(model_keys, &others, CELL_CAP, &mut |winners, vin8| {
                tally.add(&outcome(
                    winners,
                    vin8,
                    &engine_models,
                    &mut names,
                    &mut labels,
                ));
            });
            let Ok(counted) = counted else {
                // Too many combinations: the model gets nothing from this set.
                summary.capped += 1;
                continue;
            };
            summary.cells += counted;
            let Ok(index) = u32::try_from(tallies.len()) else {
                bail!("too many sets of schemas and models to number");
            };
            tallies.push(tally);
            for year in years {
                by_vehicle
                    .entry((*year, *make, *model))
                    .or_default()
                    .push(index);
            }
        }
    }

    let presets = match_presets(transaction, &curated.presets, &mut summary.unmatched)
        .context("matching the presets")?;

    // Read first: the loop below changes the table.
    let mut vehicles: Vec<Vehicle> = Vec::new();
    {
        let mut statement = transaction.prepare(
            "SELECT year, make_id, model_id FROM catalog_vehicle
             ORDER BY year, make_id, model_id",
        )?;
        let mut rows = statement.query([])?;
        while let Some(row) = rows.next()? {
            vehicles.push((row.get(0)?, row.get(1)?, row.get(2)?));
        }
    }

    // Consecutive years of a model usually share the same sets, and many
    // model years the same detail.
    let mut cache: HashMap<(i64, Vec<u32>, Option<usize>), Option<i64>> = HashMap::new();
    let mut detail_ids: HashMap<Detail, i64> = HashMap::new();
    for vehicle in vehicles {
        let (year, make, model) = vehicle;
        let mut indexes = by_vehicle.get(&vehicle).cloned().unwrap_or_default();
        indexes.sort_unstable();
        let preset = presets.get(&model).copied().filter(|index| {
            curated
                .presets
                .get(*index)
                .is_some_and(|preset| preset.from <= year && year <= preset.to.unwrap_or(u16::MAX))
        });
        let key = (model, indexes, preset);
        let detail_id = match cache.get(&key) {
            Some(detail_id) => *detail_id,
            None => {
                let mut merged = Tally::default();
                for tally in key
                    .1
                    .iter()
                    .filter_map(|index| tallies.get(usize::try_from(*index).ok()?))
                {
                    merged.merge(tally);
                }
                let preset = preset.and_then(|index| curated.presets.get(index));
                let detail_id = match assemble(&merged, &names, &speller, preset) {
                    Some(detail) => Some(match detail_ids.get(&detail) {
                        Some(id) => *id,
                        None => {
                            let id = i64::try_from(detail_ids.len())
                                .context("too many details to number")?
                                + 1;
                            write_detail(transaction, id, &detail)
                                .with_context(|| format!("writing detail {id}"))?;
                            detail_ids.insert(detail, id);
                            id
                        }
                    }),
                    None => None,
                };
                cache.insert(key, detail_id);
                detail_id
            }
        };
        if let Some(detail_id) = detail_id {
            transaction
                .prepare_cached(
                    "UPDATE catalog_vehicle SET detail_id = ?1
                     WHERE year = ?2 AND make_id = ?3 AND model_id = ?4",
                )?
                .execute(params![detail_id, year, make, model])?;
        }
    }

    summary.details = count(transaction, "catalog_detail")?;
    summary.submodels = count(transaction, "catalog_submodel")?;
    summary.engines = count(transaction, "catalog_engine")?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn row(id: i64, element: i64, value: &str) -> Row {
        Row::new(id, element, "A****", value, "").unwrap()
    }

    #[test]
    fn a_cell_becomes_names_and_an_engine_label() {
        let rows = [
            row(1, 38, "Sport"),
            row(2, 34, "1500"),
            row(3, 5, "Sedan/Saloon"),
            row(4, 15, "FWD/Front-Wheel Drive"),
            row(5, 37, "Manual/Standard"),
            row(6, 13, "2.0"),
        ];
        let winners: Vec<&Row> = rows.iter().collect();
        let engine_models = EngineModels::new();
        let (mut names, mut labels) = (Interner::default(), HashMap::new());
        let found = outcome(&winners, 0b101, &engine_models, &mut names, &mut labels);
        let text = |id: Option<u32>| id.map(|id| names.name(id).to_owned());
        assert_eq!(text(found.trim).as_deref(), Some("Sport"));
        assert_eq!(text(found.series).as_deref(), Some("1500"));
        // The body and transmission are shortened later; the drive type now.
        assert_eq!(text(found.body).as_deref(), Some("Sedan/Saloon"));
        assert_eq!(text(found.drive).as_deref(), Some("FWD"));
        assert_eq!(text(found.transmission).as_deref(), Some("Manual/Standard"));
        assert_eq!(text(found.engine).as_deref(), Some("2.0L"));
        assert_eq!(found.vin8, 0b101);
    }

    #[test]
    fn an_engine_model_fills_in_only_what_the_patterns_leave_out() {
        let engine_models = EngineModels::from([(
            "g4nh".to_owned(),
            vec![(135, "Yes".to_owned()), (13, "9.9".to_owned())],
        )]);
        let (mut names, mut labels) = (Interner::default(), HashMap::new());

        let rows = [row(1, 13, "2.0"), row(2, 18, " G4NH ")];
        let winners: Vec<&Row> = rows.iter().collect();
        let found = outcome(&winners, 1, &engine_models, &mut names, &mut labels);
        assert_eq!(names.name(found.engine.unwrap()), "2.0L Turbo");

        let rows = [row(1, 13, "2.0"), row(2, 18, "G4NH"), row(3, 135, "No")];
        let winners: Vec<&Row> = rows.iter().collect();
        let found = outcome(&winners, 1, &engine_models, &mut names, &mut labels);
        assert_eq!(names.name(found.engine.unwrap()), "2.0L");

        let rows = [row(1, 18, "Unknown Engine")];
        let winners: Vec<&Row> = rows.iter().collect();
        let found = outcome(&winners, 1, &engine_models, &mut names, &mut labels);
        assert_eq!(found.engine, None);
    }
}
