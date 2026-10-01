//! Turns a vPIC dump into a Wenmar Open data file.

use std::collections::HashMap;
use std::io::BufRead;
use std::path::Path;

use anyhow::{Context, Result, bail};
use rusqlite::{Connection, Transaction, params_from_iter};
use wenmar_vin::sqlite::{SCHEMA, SCHEMA_VERSION};

use crate::dump::{Table, read_tables};

/// What gets stamped into the data file.
#[derive(Debug, Clone)]
pub struct BuildInfo {
    /// This project's version of the data, such as `2026.09`.
    pub data_version: String,
    /// The NHTSA release it was built from, such as `vPICList_lite_2026_09`.
    pub vpic_release: String,
    /// Build time as `YYYY-MM-DD HH:MM:SS`. Manufacturer codes that become
    /// public after this are left out.
    pub built_at: String,
}

/// Row counts of the finished file.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Summary {
    pub manufacturers: u64,
    pub schema_links: u64,
    pub patterns: u64,
    pub spec_rows: u64,
    pub engine_rows: u64,
}

/// Used only for NHTSA's error correction, and by far the largest table.
const SKIPPED_TABLES: [&str; 1] = ["wmiyearvalidchars"];

/// Tables a data file cannot be built without. The specification and
/// engine-model tables are used when present.
const REQUIRED_TABLES: [&str; 12] = [
    "pattern",
    "element",
    "manufacturer",
    "country",
    "vehicletype",
    "make",
    "model",
    "make_model",
    "wmi",
    "wmi_make",
    "vinschema",
    "wmi_vinschema",
];

/// Elements NHTSA never takes from patterns: make, manufacturer name, model
/// year, and vehicle type.
const NEVER_FROM_PATTERNS: &str = "'26', '27', '29', '39'";

fn quoted(identifier: &str) -> String {
    format!("\"{}\"", identifier.replace('"', "\"\""))
}

fn staging(table: &str) -> String {
    quoted(&format!("raw_{table}"))
}

fn create_staging(transaction: &Transaction<'_>, table: &Table) -> Result<()> {
    let columns: Vec<String> = table.columns.iter().map(|column| quoted(column)).collect();
    transaction
        .execute(
            &format!(
                "CREATE TABLE IF NOT EXISTS {} ({})",
                staging(&table.name),
                columns.join(", ")
            ),
            [],
        )
        .with_context(|| format!("creating the staging table for {}", table.name))?;
    Ok(())
}

/// Copies every table of the dump into `raw_<table>`, all columns as text.
fn stage<R: BufRead>(transaction: &Transaction<'_>, dump: R) -> Result<Vec<Table>> {
    let mut inserts: HashMap<String, String> = HashMap::new();
    let tables = read_tables(dump, |table, row| {
        if SKIPPED_TABLES.contains(&table.name.as_str()) {
            return Ok(());
        }
        if !inserts.contains_key(&table.name) {
            create_staging(transaction, table)?;
            let placeholders = vec!["?"; table.columns.len()].join(", ");
            inserts.insert(
                table.name.clone(),
                format!(
                    "INSERT INTO {} VALUES ({placeholders})",
                    staging(&table.name)
                ),
            );
        }
        transaction
            .prepare_cached(&inserts[&table.name])?
            .execute(params_from_iter(row.iter()))
            .with_context(|| format!("staging a row of {}", table.name))?;
        Ok(())
    })?;
    // A table with no rows never reaches the closure above, but the shaping
    // queries and the clean-up still expect its staging table to exist.
    for table in &tables {
        if !SKIPPED_TABLES.contains(&table.name.as_str()) {
            create_staging(transaction, table)?;
        }
    }
    Ok(tables)
}

fn shape_manufacturers(transaction: &Transaction<'_>, built_at: &str) -> Result<()> {
    transaction.execute(
        "INSERT OR IGNORE INTO wmi
             (code, manufacturer, make, country, vehicle_type, light_vehicle, vehicle_type_id)
         SELECT
             UPPER(w.wmi),
             COALESCE(m.name, ''),
             (SELECT MIN(mk.name)
                FROM raw_wmi_make wm JOIN raw_make mk ON mk.id = wm.makeid
               WHERE wm.wmiid = w.id
              HAVING COUNT(*) = 1),
             c.name,
             vt.name,
             CASE WHEN w.vehicletypeid IN ('2', '7')
                    OR (w.vehicletypeid = '3' AND w.trucktypeid = '1')
                  THEN 1 ELSE 0 END,
             CAST(w.vehicletypeid AS INTEGER)
         FROM raw_wmi w
         LEFT JOIN raw_manufacturer m ON m.id = w.manufacturerid
         LEFT JOIN raw_country c ON c.id = w.countryid
         LEFT JOIN raw_vehicletype vt ON vt.id = w.vehicletypeid
         WHERE w.publicavailabilitydate <= ?1",
        [built_at],
    )?;
    Ok(())
}

/// Every make a manufacturer code builds. Specification sheets are filed
/// under a make.
fn shape_manufacturer_makes(transaction: &Transaction<'_>) -> Result<()> {
    transaction.execute(
        "INSERT INTO wmi_make (wmi, make_id)
         SELECT DISTINCT UPPER(w.wmi), CAST(wm.makeid AS INTEGER)
         FROM raw_wmi_make wm
         JOIN raw_wmi w ON w.id = wm.wmiid
         WHERE wm.makeid IS NOT NULL
           AND UPPER(w.wmi) IN (SELECT code FROM wmi)",
        [],
    )?;
    Ok(())
}

fn shape_schema_links(transaction: &Transaction<'_>) -> Result<()> {
    transaction.execute(
        "INSERT INTO wmi_schema (wmi, schema_id, year_from, year_to)
         SELECT UPPER(w.wmi), CAST(l.vinschemaid AS INTEGER), CAST(l.yearfrom AS INTEGER),
                CAST(l.yearto AS INTEGER)
         FROM raw_wmi_vinschema l
         JOIN raw_wmi w ON w.id = l.wmiid
         JOIN raw_vinschema s ON s.id = l.vinschemaid
         WHERE COALESCE(s.tobeqced, 'f') <> 't'
           AND UPPER(w.wmi) IN (SELECT code FROM wmi)",
        [],
    )?;
    Ok(())
}

/// An element whose rows are kept, with the name of the vPIC table that
/// resolves its values, if any.
struct DecodableElement {
    id: String,
    lookup: Option<String>,
}

fn decodable_elements(transaction: &Transaction<'_>) -> Result<Vec<DecodableElement>> {
    let mut statement = transaction.prepare(&format!(
        "SELECT id, lookuptable FROM raw_element
         WHERE decode IS NOT NULL
           AND COALESCE(isprivate, 'f') <> 't'
           AND COALESCE(groupname, '') <> 'Internal'
           AND id NOT IN ({NEVER_FROM_PATTERNS})"
    ))?;
    let rows = statement.query_map([], |row| {
        let lookup: Option<String> = row.get(1)?;
        Ok(DecodableElement {
            id: row.get(0)?,
            lookup: lookup.map(|name| name.to_lowercase()),
        })
    })?;
    Ok(rows.collect::<rusqlite::Result<_>>()?)
}

fn is_staged(staged: &[Table], table: &str) -> bool {
    staged.iter().any(|staged| staged.name == table)
}

/// How a row's raw attribute becomes its value, as two pieces of SQL about a
/// row aliased `p`.
struct ValueSource {
    /// The expression that gives the value.
    value: &'static str,
    /// The join that expression needs, or nothing.
    join: String,
}

/// Patterns, specification rows and engine-model rows all resolve values
/// through this. `None` means the element's rows are left out: its lookup
/// table is not in the dump (a database view, for example), so nothing can
/// be resolved.
fn value_source(element: &DecodableElement, staged: &[Table]) -> Option<ValueSource> {
    match &element.lookup {
        // The value is the attribute itself.
        None => Some(ValueSource {
            value: "p.attributeid",
            join: String::new(),
        }),
        Some(table) if !is_staged(staged, table) => None,
        // A row whose attribute has no lookup row is dropped by the join.
        Some(table) => Some(ValueSource {
            value: "l.name",
            join: format!("JOIN {} l ON l.id = p.attributeid", staging(table)),
        }),
    }
}

fn shape_patterns(transaction: &Transaction<'_>, staged: &[Table]) -> Result<()> {
    for element in decodable_elements(transaction)? {
        let Some(ValueSource { value, join }) = value_source(&element, staged) else {
            continue;
        };
        // The model also carries its make.
        let make = if element.lookup.as_deref() == Some("model") {
            "(SELECT MIN(mk.name)
                FROM raw_make_model mm JOIN raw_make mk ON mk.id = mm.makeid
               WHERE mm.modelid = p.attributeid)"
        } else {
            "NULL"
        };
        let sql = format!(
            "INSERT INTO pattern
                 (id, schema_id, keys, element_id, value, changed_on, make, attribute)
             SELECT CAST(p.id AS INTEGER), CAST(p.vinschemaid AS INTEGER), UPPER(p.keys),
                    CAST(p.elementid AS INTEGER), {value},
                    COALESCE(p.updatedon, p.createdon, ''), {make}, p.attributeid
             FROM raw_pattern p
             {join}
             WHERE p.elementid = ?1
               AND p.vinschemaid IN (SELECT CAST(schema_id AS TEXT) FROM wmi_schema)
               AND p.keys NOT LIKE '%#%'"
        );
        transaction
            .execute(&sql, [&element.id])
            .with_context(|| format!("shaping patterns for element {}", element.id))?;
    }
    Ok(())
}

/// NHTSA's specification sheets. A sheet is a group of rows under one
/// `vspecschemapattern`: key rows say which vehicles it describes, the rest
/// say what those vehicles have.
const SPECIFICATION_TABLES: [&str; 5] = [
    "vehiclespecschema",
    "vehiclespecschema_model",
    "vehiclespecschema_year",
    "vspecschemapattern",
    "vehiclespecpattern",
];

fn shape_specifications(transaction: &Transaction<'_>, staged: &[Table]) -> Result<()> {
    if !SPECIFICATION_TABLES
        .iter()
        .all(|table| is_staged(staged, table))
    {
        return Ok(());
    }
    transaction.execute_batch(
        "INSERT INTO spec_schema (id, make_id, vehicle_type_id)
         SELECT CAST(s.id AS INTEGER), CAST(s.makeid AS INTEGER), CAST(s.vehicletypeid AS INTEGER)
         FROM raw_vehiclespecschema s
         WHERE COALESCE(s.tobeqced, 'f') <> 't' AND s.makeid IS NOT NULL;

         INSERT INTO spec_schema_model (schema_id, model_id)
         SELECT DISTINCT CAST(m.vehiclespecschemaid AS INTEGER), CAST(m.modelid AS INTEGER)
         FROM raw_vehiclespecschema_model m
         WHERE m.modelid IS NOT NULL
           AND CAST(m.vehiclespecschemaid AS INTEGER) IN (SELECT id FROM spec_schema);

         INSERT INTO spec_schema_year (schema_id, year)
         SELECT DISTINCT CAST(y.vehiclespecschemaid AS INTEGER), CAST(y.year AS INTEGER)
         FROM raw_vehiclespecschema_year y
         WHERE y.year IS NOT NULL
           AND CAST(y.vehiclespecschemaid AS INTEGER) IN (SELECT id FROM spec_schema);",
    )?;
    for element in decodable_elements(transaction)? {
        let Some(ValueSource { value, join }) = value_source(&element, staged) else {
            continue;
        };
        let sql = format!(
            "INSERT INTO spec_row
                 (id, spec_pattern_id, schema_id, is_key, element_id, attribute, value, changed_on)
             SELECT CAST(p.id AS INTEGER), CAST(p.vspecschemapatternid AS INTEGER),
                    CAST(sp.schemaid AS INTEGER), CASE WHEN p.iskey = 't' THEN 1 ELSE 0 END,
                    CAST(p.elementid AS INTEGER), p.attributeid, {value},
                    COALESCE(p.updatedon, p.createdon, '')
             FROM raw_vehiclespecpattern p
             JOIN raw_vspecschemapattern sp ON sp.id = p.vspecschemapatternid
             {join}
             WHERE p.elementid = ?1
               AND CAST(sp.schemaid AS INTEGER) IN (SELECT id FROM spec_schema)"
        );
        transaction
            .execute(&sql, [&element.id])
            .with_context(|| format!("shaping specification rows for element {}", element.id))?;
    }
    // NHTSA applies a sheet only when every one of its keys matches. A key
    // that was not kept above can never match, and a sheet without it would
    // apply to vehicles it does not describe, so the whole sheet goes.
    transaction.execute(
        "DELETE FROM spec_row
         WHERE spec_pattern_id IN (
             SELECT CAST(p.vspecschemapatternid AS INTEGER)
             FROM raw_vehiclespecpattern p
             WHERE p.iskey = 't'
               AND CAST(p.id AS INTEGER) NOT IN (SELECT id FROM spec_row))",
        [],
    )?;
    Ok(())
}

/// What NHTSA records about an engine model, such as its cylinder count.
const ENGINE_MODEL_TABLES: [&str; 2] = ["enginemodel", "enginemodelpattern"];

fn shape_engine_models(transaction: &Transaction<'_>, staged: &[Table]) -> Result<()> {
    if !ENGINE_MODEL_TABLES
        .iter()
        .all(|table| is_staged(staged, table))
    {
        return Ok(());
    }
    for element in decodable_elements(transaction)? {
        let Some(ValueSource { value, join }) = value_source(&element, staged) else {
            continue;
        };
        let sql = format!(
            "INSERT INTO engine_model_row
                 (id, engine_model, element_id, attribute, value, changed_on)
             SELECT CAST(p.id AS INTEGER), LOWER(TRIM(em.name)), CAST(p.elementid AS INTEGER),
                    p.attributeid, {value}, COALESCE(p.updatedon, p.createdon, '')
             FROM raw_enginemodelpattern p
             JOIN raw_enginemodel em ON em.id = p.enginemodelid
             {join}
             WHERE p.elementid = ?1 AND em.name IS NOT NULL"
        );
        transaction
            .execute(&sql, [&element.id])
            .with_context(|| format!("shaping engine-model rows for element {}", element.id))?;
    }
    Ok(())
}

fn count(connection: &Connection, table: &str) -> Result<u64> {
    let rows: i64 = connection.query_row(&format!("SELECT COUNT(*) FROM {table}"), [], |row| {
        row.get(0)
    })?;
    Ok(u64::try_from(rows).unwrap_or(0))
}

/// Builds a data file at `output` from a vPIC plain-text dump.
pub fn build<R: BufRead>(dump: R, output: &Path, info: &BuildInfo) -> Result<Summary> {
    if output.exists() {
        bail!(
            "{} already exists; remove it or choose another path",
            output.display()
        );
    }
    let result = build_into(dump, output, info);
    if result.is_err() {
        // Never leave a half-built file that could be mistaken for a good one.
        let _ = std::fs::remove_file(output);
    }
    result
}

fn build_into<R: BufRead>(dump: R, output: &Path, info: &BuildInfo) -> Result<Summary> {
    let mut connection =
        Connection::open(output).with_context(|| format!("creating {}", output.display()))?;
    connection.execute_batch("PRAGMA journal_mode = OFF; PRAGMA synchronous = OFF;")?;
    connection.execute_batch(SCHEMA)?;

    let transaction = connection.transaction()?;
    let staged = stage(&transaction, dump)?;
    for required in REQUIRED_TABLES {
        if !staged.iter().any(|table| table.name == required) {
            bail!("the dump is missing table {required}");
        }
    }
    shape_manufacturers(&transaction, &info.built_at)?;
    shape_manufacturer_makes(&transaction)?;
    shape_schema_links(&transaction)?;
    shape_patterns(&transaction, &staged)?;
    shape_specifications(&transaction, &staged)?;
    shape_engine_models(&transaction, &staged)?;
    // A change in the dump's layout must fail here, not ship a file that
    // decodes nothing.
    for (table, what) in [
        ("wmi", "manufacturer codes"),
        ("wmi_schema", "schema links"),
        ("pattern", "patterns"),
    ] {
        if count(&transaction, table)? == 0 {
            bail!("the dump produced no {what}; its layout may have changed");
        }
    }
    for (key, value) in [
        ("schema_version", SCHEMA_VERSION),
        ("data_version", info.data_version.as_str()),
        ("vpic_release", info.vpic_release.as_str()),
        ("built_at", info.built_at.as_str()),
    ] {
        transaction.execute(
            "INSERT INTO meta (key, value) VALUES (?1, ?2)",
            [key, value],
        )?;
    }
    for table in &staged {
        if !SKIPPED_TABLES.contains(&table.name.as_str()) {
            transaction.execute(&format!("DROP TABLE {}", staging(&table.name)), [])?;
        }
    }
    transaction.commit()?;
    connection.execute_batch("VACUUM;")?;

    Ok(Summary {
        manufacturers: count(&connection, "wmi")?,
        schema_links: count(&connection, "wmi_schema")?,
        patterns: count(&connection, "pattern")?,
        spec_rows: count(&connection, "spec_row")?,
        engine_rows: count(&connection, "engine_model_row")?,
    })
}
