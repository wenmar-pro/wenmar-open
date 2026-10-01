//! Reads the `COPY ... FROM stdin` blocks of a PostgreSQL plain-text dump.

use std::io::BufRead;

use anyhow::{Context, Result, bail};

/// A table found in the dump.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Table {
    pub name: String,
    pub columns: Vec<String>,
}

const COPY_PREFIX: &str = "COPY vpic.";
const COPY_SUFFIX: &str = " FROM stdin;";
const END_OF_DATA: &str = "\\.";

/// Parses `COPY vpic.make (id, name) FROM stdin;`.
fn parse_copy(line: &str) -> Option<Table> {
    let rest = line.strip_prefix(COPY_PREFIX)?.strip_suffix(COPY_SUFFIX)?;
    let (name, columns) = rest.split_once(" (")?;
    let columns = columns.strip_suffix(')')?;
    Some(Table {
        name: name.trim_matches('"').to_owned(),
        columns: columns
            .split(", ")
            .map(|column| column.trim_matches('"').to_owned())
            .collect(),
    })
}

/// Decodes one field of COPY text format. `\N` is null.
fn decode_field(field: &str) -> Option<String> {
    if field == "\\N" {
        return None;
    }
    if !field.contains('\\') {
        return Some(field.to_owned());
    }
    let mut decoded = String::with_capacity(field.len());
    let mut characters = field.chars();
    while let Some(character) = characters.next() {
        if character != '\\' {
            decoded.push(character);
            continue;
        }
        match characters.next() {
            Some('t') => decoded.push('\t'),
            Some('n') => decoded.push('\n'),
            Some('r') => decoded.push('\r'),
            Some('b') => decoded.push('\u{8}'),
            Some('f') => decoded.push('\u{c}'),
            Some('v') => decoded.push('\u{b}'),
            Some(other) => decoded.push(other),
            None => decoded.push('\\'),
        }
    }
    Some(decoded)
}

/// Calls `visit` for every row of every `COPY vpic.*` block, and returns the
/// tables seen in the order they appear.
pub fn read_tables<R: BufRead>(
    reader: R,
    mut visit: impl FnMut(&Table, Vec<Option<String>>) -> Result<()>,
) -> Result<Vec<Table>> {
    let mut tables = Vec::new();
    let mut current: Option<Table> = None;
    for (index, line) in reader.lines().enumerate() {
        let number = index + 1;
        let line = line.with_context(|| format!("reading line {number} of the dump"))?;
        match &current {
            None => current = parse_copy(&line),
            Some(_) if line == END_OF_DATA => tables.extend(current.take()),
            Some(table) => {
                let row: Vec<Option<String>> = line.split('\t').map(decode_field).collect();
                if row.len() != table.columns.len() {
                    bail!(
                        "table {} has {} columns but line {number} has {} fields",
                        table.name,
                        table.columns.len(),
                        row.len()
                    );
                }
                visit(table, row)?;
            }
        }
    }
    if let Some(table) = current {
        bail!("the dump ended inside the data for table {}", table.name);
    }
    if tables.is_empty() {
        bail!("the dump contains no vpic tables");
    }
    Ok(tables)
}

#[cfg(test)]
mod tests {
    use super::*;

    const DUMP: &str = "\
--
-- PostgreSQL database dump
--
SET client_encoding = 'UTF8';
CREATE TABLE vpic.make (
    id integer NOT NULL,
    name character varying(250) NOT NULL
);
COPY vpic.make (id, name, createdon) FROM stdin;
440\tAston Martin\t2015-03-04 10:05:33.893
441\tTesla\t\\N
\\.

COPY vpic.note (id, \"text\") FROM stdin;
1\tTab\\there, newline\\nhere, slash\\\\here
\\.
COPY public.other (id) FROM stdin;
9
\\.
";

    /// Table name and decoded fields of each row, in dump order.
    type Rows = Vec<(String, Vec<Option<String>>)>;

    fn read(dump: &str) -> anyhow::Result<(Vec<Table>, Rows)> {
        let mut rows = Vec::new();
        let tables = read_tables(dump.as_bytes(), |table, row| {
            rows.push((table.name.clone(), row));
            Ok(())
        })?;
        Ok((tables, rows))
    }

    #[test]
    fn reads_tables_columns_and_rows() {
        let (tables, rows) = read(DUMP).unwrap();
        assert_eq!(tables.len(), 2);
        assert_eq!(tables[0].name, "make");
        assert_eq!(tables[0].columns, vec!["id", "name", "createdon"]);
        assert_eq!(tables[1].columns, vec!["id", "text"]);
        assert_eq!(
            rows[0],
            (
                "make".to_owned(),
                vec![
                    Some("440".to_owned()),
                    Some("Aston Martin".to_owned()),
                    Some("2015-03-04 10:05:33.893".to_owned())
                ]
            )
        );
    }

    #[test]
    fn nulls_and_escapes_are_decoded() {
        let (_, rows) = read(DUMP).unwrap();
        assert_eq!(rows[1].1[2], None);
        assert_eq!(
            rows[2].1[1].as_deref(),
            Some("Tab\there, newline\nhere, slash\\here")
        );
    }

    #[test]
    fn tables_outside_the_vpic_schema_are_ignored() {
        let (tables, rows) = read(DUMP).unwrap();
        assert!(tables.iter().all(|table| table.name != "other"));
        assert_eq!(rows.len(), 3);
    }

    #[test]
    fn a_block_with_no_terminator_is_an_error() {
        let truncated = "COPY vpic.make (id, name) FROM stdin;\n440\tAston Martin\n";
        let error = read(truncated).unwrap_err().to_string();
        assert!(error.contains("make"), "{error}");
        assert!(error.contains("ended"), "{error}");
    }

    #[test]
    fn a_row_with_the_wrong_number_of_fields_is_an_error() {
        let ragged = "COPY vpic.make (id, name) FROM stdin;\n440\n\\.\n";
        let error = read(ragged).unwrap_err().to_string();
        assert!(error.contains("make"), "{error}");
        assert!(error.contains("line 2"), "{error}");
    }

    #[test]
    fn a_dump_with_no_tables_is_an_error() {
        let error = read("-- nothing here\n").unwrap_err().to_string();
        assert!(error.contains("no vpic tables"), "{error}");
    }
}
