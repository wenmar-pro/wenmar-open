//! The rows one VIN needs, read before decoding.
//!
//! `wenmar-vin` has no database code. This module asks the data file for
//! everything the decoder could want for one VIN and hands it over as a
//! [`VinData`] held in memory. The statements are the ones in
//! `wenmar_vin::sqlite`, which reads the same tables with `rusqlite`.

use std::collections::{BTreeMap, BTreeSet, HashMap};

use wenmar_vehicles::{Source, SourceError, Value};
use wenmar_vin::{
    DataError, Element, EngineRow, Manufacturer, Pattern, SchemaRef, SpecRow, Vin, VinData,
    model_year, pattern,
};

/// vPIC's ids for the two elements whose rows lead to more rows.
const MODEL_ELEMENT_ID: i64 = 28;

/// Most ids put in one `IN (...)` list.
const CHUNK: usize = 200;

const MANUFACTURERS_SQL: &str = "
SELECT code, manufacturer, make, country, vehicle_type, light_vehicle, vehicle_type_id
FROM wmi WHERE code IN (?1, ?2)";

const SCHEMAS_SQL: &str = "
SELECT schema_id, year_from, year_to FROM wmi_schema WHERE wmi = ?1";

const MAKES_SQL: &str = "SELECT make_id FROM wmi_make WHERE wmi = ?1";

/// The specification schemas that list a model (`?1`), with the make and
/// vehicle type each is for.
const SPEC_SCHEMAS_SQL: &str = "
SELECT s.id, s.make_id, s.vehicle_type_id
FROM spec_schema_model m
JOIN spec_schema s ON s.id = m.schema_id
WHERE m.model_id = ?1";

#[derive(Debug, Clone)]
struct SchemaRange {
    id: i64,
    year_from: u16,
    year_to: Option<u16>,
}

impl SchemaRange {
    fn covers(&self, year: u16) -> bool {
        self.year_from <= year && self.year_to.is_none_or(|to| to >= year)
    }
}

/// Everything the decoder may ask for about one VIN.
#[derive(Debug, Clone, Default)]
pub struct VinRows {
    manufacturers: Vec<Manufacturer>,
    /// The schemas of the manufacturer code the decoder will use.
    schemas: Vec<SchemaRange>,
    patterns: Vec<Pattern>,
    /// Engine model name, trimmed and in lowercase, to its rows.
    engines: HashMap<String, Vec<EngineRow>>,
    /// Model id and model year to the specification rows.
    specs: HashMap<(i64, u16), Vec<SpecRow>>,
}

fn shape(table: &'static str) -> SourceError {
    format!("unexpected row in {table}").into()
}

fn integer(row: &[Value], column: usize, table: &'static str) -> Result<i64, SourceError> {
    row.get(column)
        .and_then(Value::integer)
        .ok_or_else(|| shape(table))
}

fn text(row: &[Value], column: usize, table: &'static str) -> Result<String, SourceError> {
    row.get(column)
        .and_then(Value::text)
        .map(str::to_owned)
        .ok_or_else(|| shape(table))
}

fn optional_text(row: &[Value], column: usize) -> Option<String> {
    row.get(column).and_then(Value::text).map(str::to_owned)
}

fn year(row: &[Value], column: usize, table: &'static str) -> Result<u16, SourceError> {
    u16::try_from(integer(row, column, table)?).map_err(|_| shape(table))
}

fn element(id: i64) -> Element {
    Element::from_vpic_id(id).unwrap_or(Element::Other(id))
}

fn placeholders(count: usize) -> String {
    vec!["?"; count].join(", ")
}

/// Reads what the decoder needs for `vin`.
///
/// `model_year` is the caller's override, if any. `current_year` must be the
/// same year the decoder is given, so both work out the same candidates.
pub fn fetch<S: Source>(
    source: &S,
    vin: &Vin,
    model_year: Option<u16>,
    current_year: u16,
) -> Result<VinRows, SourceError> {
    let mut rows = VinRows::default();
    let mut type_ids: HashMap<String, Option<i64>> = HashMap::new();

    let plain = vin.wmi().to_owned();
    let extended = vin.extended_wmi();
    let first = extended.clone().unwrap_or_else(|| plain.clone());
    for row in source.query(
        MANUFACTURERS_SQL,
        &[first.as_str().into(), plain.as_str().into()],
    )? {
        type_ids.insert(text(&row, 0, "wmi")?, row.get(6).and_then(Value::integer));
        rows.manufacturers.push(Manufacturer {
            wmi: text(&row, 0, "wmi")?,
            name: text(&row, 1, "wmi")?,
            make: optional_text(&row, 2),
            country: optional_text(&row, 3),
            vehicle_type: optional_text(&row, 4),
            light_vehicle: integer(&row, 5, "wmi")? != 0,
        });
    }
    // The decoder prefers the six-character code when it is registered.
    let find = |code: &str| rows.manufacturers.iter().find(|found| found.wmi == code);
    let Some(manufacturer) = extended.as_deref().and_then(find).or_else(|| find(&plain)) else {
        return Ok(rows);
    };
    let wmi = manufacturer.wmi.clone();
    let light = manufacturer.light_vehicle;

    for row in source.query(SCHEMAS_SQL, &[wmi.as_str().into()])? {
        rows.schemas.push(SchemaRange {
            id: integer(&row, 0, "wmi_schema")?,
            year_from: year(&row, 1, "wmi_schema")?,
            year_to: match row.get(2) {
                Some(Value::Null) | None => None,
                Some(_) => Some(year(&row, 2, "wmi_schema")?),
            },
        });
    }

    let years = match model_year {
        Some(year) => vec![year],
        None => model_year::candidates(vin, current_year, light),
    };
    let schema_ids: BTreeSet<i64> = rows
        .schemas
        .iter()
        .filter(|schema| years.iter().any(|year| schema.covers(*year)))
        .map(|schema| schema.id)
        .collect();
    let schema_ids: Vec<i64> = schema_ids.into_iter().collect();
    let key = vin.match_key();

    let mut engine_models: BTreeSet<String> = BTreeSet::new();
    let mut models: BTreeSet<(i64, i64)> = BTreeSet::new();
    for chunk in schema_ids.chunks(CHUNK) {
        // GLOB uses `?` for one character and understands `[...]` sets, so it
        // narrows the rows to those that can match. The decoder checks again.
        let sql = format!(
            "SELECT id, schema_id, keys, element_id, value, changed_on, make, attribute
             FROM pattern
             WHERE schema_id IN ({})
               AND ?{} GLOB (REPLACE(keys, '*', '?') || '*')",
            placeholders(chunk.len()),
            chunk.len() + 1
        );
        let mut params: Vec<Value> = chunk.iter().map(|id| Value::Integer(*id)).collect();
        params.push(key.as_str().into());
        for row in source.query(&sql, &params)? {
            let element_id = integer(&row, 3, "pattern")?;
            let found = Pattern {
                id: integer(&row, 0, "pattern")?,
                schema_id: integer(&row, 1, "pattern")?,
                keys: text(&row, 2, "pattern")?,
                element: element(element_id),
                attribute: text(&row, 7, "pattern")?,
                value: text(&row, 4, "pattern")?,
                changed_on: text(&row, 5, "pattern")?,
            };
            let matched = pattern::matches(&found.keys, &key);
            if matched && found.element == Element::EngineModel {
                engine_models.insert(found.value.trim().to_lowercase());
            }
            if matched
                && element_id == MODEL_ELEMENT_ID
                && let Ok(model_id) = found.attribute.trim().parse::<i64>()
            {
                models.insert((found.schema_id, model_id));
            }
            if element_id == MODEL_ELEMENT_ID
                && let Some(make) = optional_text(&row, 6)
            {
                rows.patterns.push(Pattern {
                    element: Element::Make,
                    attribute: make.clone(),
                    value: make,
                    ..found.clone()
                });
            }
            rows.patterns.push(found);
        }
    }

    let engine_models: Vec<String> = engine_models.into_iter().collect();
    for chunk in engine_models.chunks(CHUNK) {
        let sql = format!(
            "SELECT engine_model, id, element_id, attribute, value, changed_on
             FROM engine_model_row WHERE engine_model IN ({}) ORDER BY id",
            placeholders(chunk.len())
        );
        let params: Vec<Value> = chunk.iter().map(|name| name.as_str().into()).collect();
        for row in source.query(&sql, &params)? {
            rows.engines
                .entry(text(&row, 0, "engine_model_row")?)
                .or_default()
                .push(EngineRow {
                    id: integer(&row, 1, "engine_model_row")?,
                    element: element(integer(&row, 2, "engine_model_row")?),
                    attribute: text(&row, 3, "engine_model_row")?,
                    value: text(&row, 4, "engine_model_row")?,
                    changed_on: text(&row, 5, "engine_model_row")?,
                });
        }
    }

    // A specification sheet is asked for by model and year. Only the years
    // in which the model's own schema applies can be asked about.
    let mut wanted: BTreeMap<i64, BTreeSet<u16>> = BTreeMap::new();
    for (schema_id, model_id) in models {
        for candidate in &years {
            let applies = rows
                .schemas
                .iter()
                .any(|schema| schema.id == schema_id && schema.covers(*candidate));
            if applies {
                wanted.entry(model_id).or_default().insert(*candidate);
            }
        }
    }
    if wanted.is_empty() {
        return Ok(rows);
    }
    let type_id = type_ids.get(&wmi).copied().flatten();
    let mut make_ids: BTreeSet<i64> = BTreeSet::new();
    for row in source.query(MAKES_SQL, &[wmi.as_str().into()])? {
        make_ids.insert(integer(&row, 0, "wmi_make")?);
    }
    for (model_id, model_years) in wanted {
        let found = specs(source, model_id, &model_years, &make_ids, type_id)?;
        rows.specs.extend(found);
    }
    Ok(rows)
}

/// The specification rows for one model in each of `years`.
///
/// This gives the rows `wenmar_vin::sqlite` reads with one statement. It is
/// done in three small steps because turso plans that statement badly: 25 ms
/// against the real data file, where these steps take under 1 ms together.
fn specs<S: Source>(
    source: &S,
    model_id: i64,
    years: &BTreeSet<u16>,
    make_ids: &BTreeSet<i64>,
    type_id: Option<i64>,
) -> Result<HashMap<(i64, u16), Vec<SpecRow>>, SourceError> {
    let mut found: HashMap<(i64, u16), Vec<SpecRow>> = years
        .iter()
        .map(|year| ((model_id, *year), Vec::new()))
        .collect();

    // Schemas for this model, of the manufacturer's makes and vehicle type.
    let mut schema_ids: BTreeSet<i64> = BTreeSet::new();
    for row in source.query(SPEC_SCHEMAS_SQL, &[model_id.into()])? {
        let make_id = integer(&row, 1, "spec_schema")?;
        let schema_type = row.get(2).and_then(Value::integer);
        if make_ids.contains(&make_id) && type_id.is_some() && schema_type == type_id {
            schema_ids.insert(integer(&row, 0, "spec_schema")?);
        }
    }
    let schema_ids: Vec<i64> = schema_ids.into_iter().collect();

    // The years each schema is for. A schema with no years is for every year.
    let mut schema_years: HashMap<i64, Vec<u16>> = HashMap::new();
    for chunk in schema_ids.chunks(CHUNK) {
        let sql = format!(
            "SELECT schema_id, year FROM spec_schema_year WHERE schema_id IN ({})",
            placeholders(chunk.len())
        );
        let params: Vec<Value> = chunk.iter().map(|id| Value::Integer(*id)).collect();
        for row in source.query(&sql, &params)? {
            // A year outside u16 can never be asked for.
            if let Ok(year) = u16::try_from(integer(&row, 1, "spec_schema_year")?) {
                schema_years
                    .entry(integer(&row, 0, "spec_schema_year")?)
                    .or_default()
                    .push(year);
            } else {
                schema_years
                    .entry(integer(&row, 0, "spec_schema_year")?)
                    .or_default();
            }
        }
    }
    let applies = |schema_id: i64, year: u16| {
        schema_years
            .get(&schema_id)
            .is_none_or(|listed| listed.contains(&year))
    };
    let needed: Vec<i64> = schema_ids
        .iter()
        .copied()
        .filter(|id| years.iter().any(|year| applies(*id, *year)))
        .collect();

    for chunk in needed.chunks(CHUNK) {
        let sql = format!(
            "SELECT id, spec_pattern_id, is_key, element_id, attribute, value, changed_on, schema_id
             FROM spec_row WHERE schema_id IN ({})",
            placeholders(chunk.len())
        );
        let params: Vec<Value> = chunk.iter().map(|id| Value::Integer(*id)).collect();
        for row in source.query(&sql, &params)? {
            let schema_id = integer(&row, 7, "spec_row")?;
            let spec = SpecRow {
                id: integer(&row, 0, "spec_row")?,
                spec_pattern_id: integer(&row, 1, "spec_row")?,
                is_key: integer(&row, 2, "spec_row")? != 0,
                element: element(integer(&row, 3, "spec_row")?),
                attribute: text(&row, 4, "spec_row")?,
                value: text(&row, 5, "spec_row")?,
                changed_on: text(&row, 6, "spec_row")?,
            };
            for year in years {
                if applies(schema_id, *year)
                    && let Some(list) = found.get_mut(&(model_id, *year))
                {
                    list.push(spec.clone());
                }
            }
        }
    }
    for list in found.values_mut() {
        list.sort_by_key(|row| row.id);
    }
    Ok(found)
}

impl VinData for VinRows {
    fn manufacturer(&self, wmi: &str) -> Result<Option<Manufacturer>, DataError> {
        Ok(self
            .manufacturers
            .iter()
            .find(|manufacturer| manufacturer.wmi == wmi)
            .cloned())
    }

    fn schemas(&self, _wmi: &str, year: u16) -> Result<Vec<SchemaRef>, DataError> {
        Ok(self
            .schemas
            .iter()
            .filter(|schema| schema.covers(year))
            .map(|schema| SchemaRef {
                id: schema.id,
                year_from: schema.year_from,
            })
            .collect())
    }

    fn patterns(&self, schema_ids: &[i64], _match_key: &str) -> Result<Vec<Pattern>, DataError> {
        Ok(self
            .patterns
            .iter()
            .filter(|pattern| schema_ids.contains(&pattern.schema_id))
            .cloned()
            .collect())
    }

    fn engine_model(&self, name: &str) -> Result<Vec<EngineRow>, DataError> {
        Ok(self
            .engines
            .get(&name.trim().to_lowercase())
            .cloned()
            .unwrap_or_default())
    }

    fn specs(
        &self,
        _wmi: &str,
        model_attribute: &str,
        year: u16,
    ) -> Result<Vec<SpecRow>, DataError> {
        let Ok(model_id) = model_attribute.trim().parse::<i64>() else {
            return Ok(Vec::new());
        };
        Ok(self
            .specs
            .get(&(model_id, year))
            .cloned()
            .unwrap_or_default())
    }
}
