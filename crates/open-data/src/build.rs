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
}

/// Used only for NHTSA's error correction, and by far the largest table.
const SKIPPED_TABLES: [&str; 1] = ["wmiyearvalidchars"];

/// Tables the shaping queries read.
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
        "INSERT OR IGNORE INTO wmi (code, manufacturer, make, country, vehicle_type, light_vehicle)
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
                  THEN 1 ELSE 0 END
         FROM raw_wmi w
         LEFT JOIN raw_manufacturer m ON m.id = w.manufacturerid
         LEFT JOIN raw_country c ON c.id = w.countryid
         LEFT JOIN raw_vehicletype vt ON vt.id = w.vehicletypeid
         WHERE w.publicavailabilitydate IS NULL OR w.publicavailabilitydate <= ?1",
        [built_at],
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

/// Elements whose patterns are kept, with the staging table that resolves
/// their values, if any.
fn decodable_elements(transaction: &Transaction<'_>) -> Result<Vec<(String, Option<String>)>> {
    let mut statement = transaction.prepare(&format!(
        "SELECT id, lookuptable FROM raw_element
         WHERE decode IS NOT NULL
           AND COALESCE(isprivate, 'f') <> 't'
           AND COALESCE(groupname, '') <> 'Internal'
           AND id NOT IN ({NEVER_FROM_PATTERNS})"
    ))?;
    let rows = statement.query_map([], |row| {
        let lookup: Option<String> = row.get(1)?;
        Ok((
            row.get::<_, String>(0)?,
            lookup.map(|name| name.to_lowercase()),
        ))
    })?;
    Ok(rows.collect::<rusqlite::Result<_>>()?)
}

fn shape_patterns(transaction: &Transaction<'_>, staged: &[Table]) -> Result<()> {
    const KEPT: &str = "p.vinschemaid IN (SELECT CAST(schema_id AS TEXT) FROM wmi_schema)
           AND p.keys NOT LIKE '%#%'";
    for (element_id, lookup) in decodable_elements(transaction)? {
        let sql = match lookup {
            // The value is the attribute itself.
            None => format!(
                "INSERT INTO pattern (id, schema_id, keys, element_id, value, changed_on, make)
                 SELECT CAST(p.id AS INTEGER), CAST(p.vinschemaid AS INTEGER), UPPER(p.keys),
                        CAST(p.elementid AS INTEGER), p.attributeid,
                        COALESCE(p.updatedon, p.createdon, ''), NULL
                 FROM raw_pattern p
                 WHERE p.elementid = ?1 AND {KEPT}"
            ),
            // A lookup table that is not in the dump (a database view, for
            // example) resolves nothing, so the element is left out.
            Some(table) if !staged.iter().any(|staged| staged.name == table) => continue,
            // The model also carries its make.
            Some(table) if table == "model" => format!(
                "INSERT INTO pattern (id, schema_id, keys, element_id, value, changed_on, make)
                 SELECT CAST(p.id AS INTEGER), CAST(p.vinschemaid AS INTEGER), UPPER(p.keys),
                        CAST(p.elementid AS INTEGER), l.name,
                        COALESCE(p.updatedon, p.createdon, ''),
                        (SELECT MIN(mk.name)
                           FROM raw_make_model mm JOIN raw_make mk ON mk.id = mm.makeid
                          WHERE mm.modelid = p.attributeid)
                 FROM raw_pattern p
                 JOIN raw_model l ON l.id = p.attributeid
                 WHERE p.elementid = ?1 AND {KEPT}"
            ),
            Some(table) => format!(
                "INSERT INTO pattern (id, schema_id, keys, element_id, value, changed_on, make)
                 SELECT CAST(p.id AS INTEGER), CAST(p.vinschemaid AS INTEGER), UPPER(p.keys),
                        CAST(p.elementid AS INTEGER), l.name,
                        COALESCE(p.updatedon, p.createdon, ''), NULL
                 FROM raw_pattern p
                 JOIN {} l ON l.id = p.attributeid
                 WHERE p.elementid = ?1 AND {KEPT}",
                staging(&table)
            ),
        };
        transaction
            .execute(&sql, [&element_id])
            .with_context(|| format!("shaping patterns for element {element_id}"))?;
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
    shape_schema_links(&transaction)?;
    shape_patterns(&transaction, &staged)?;
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
    })
}
