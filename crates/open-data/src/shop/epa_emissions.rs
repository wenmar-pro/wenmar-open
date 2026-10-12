//! Reads EPA's `emissions.csv`: the emission standard each vehicle of
//! `vehicles.csv` was certified to, by the area it may be sold in.

use std::fs::File;
use std::io::Read;
use std::path::Path;

use anyhow::{Context, Result, bail};

/// The archive `open-data fetch epa-emissions` saves.
pub const FILE: &str = "emissions.csv.zip";

/// The one file inside it.
pub const INNER: &str = "emissions.csv";

/// The column names of `emissions.csv`, in order.
pub const HEADER: [&str; 8] = [
    "efid",
    "id",
    "salesArea",
    "score",
    "scoreAlt",
    "smartwayScore",
    "standard",
    "stdText",
];

/// One row: one certification of one vehicle.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct EmissionRow {
    /// `id`: the `vehicles.csv` id.
    pub epa_id: u32,
    /// `salesArea`: EPA's sales area code, a number: 1, 3, 5, 7, 8 or 9.
    /// EPA's column list does not say what each number means, so it is
    /// kept as a number and never given a name here.
    pub sales_area: u8,
    /// `standard`: the standard's code, such as `T3B70` or `L3ULEV70`.
    pub standard: String,
    /// `stdText`: the standard in words, such as `Federal Tier 3 Bin 70`.
    /// None when empty or the word `null`.
    pub standard_text: Option<String>,
    /// `score`: the smog rating, 1 (worst) to 10 (best). None when the
    /// file has a number outside that range.
    pub smog_score: Option<u8>,
    /// `efid`: the engine family (test group), such as `JFMXT03.54DX`.
    pub engine_family: String,
}

/// What a parse read.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct EmissionCounts {
    pub rows: u64,
}

/// A score written `5.0`: kept when it is 1 to 10.
fn score(field: &str) -> Option<u8> {
    let number: f64 = field.trim().parse().ok()?;
    (number.fract() == 0.0 && (1.0..=10.0).contains(&number)).then_some(number as u8)
}

/// Reads the CSV text of `emissions.csv` and hands on each row.
pub fn parse_reader(
    reader: impl Read,
    origin: &str,
    row_fn: &mut dyn FnMut(EmissionRow) -> Result<()>,
) -> Result<EmissionCounts> {
    let mut reader = csv::ReaderBuilder::new()
        .has_headers(true)
        .from_reader(reader);
    let header = reader
        .headers()
        .with_context(|| format!("reading the header of {origin}"))?
        .clone();
    if header.iter().collect::<Vec<_>>() != HEADER {
        bail!(
            "{origin} has another header than this build reads: {}",
            header.iter().collect::<Vec<_>>().join(",")
        );
    }
    let mut counts = EmissionCounts::default();
    for (index, record) in reader.records().enumerate() {
        let line = index as u64 + 2;
        let record = record.with_context(|| format!("{origin}, line {line}"))?;
        let field = |index: usize| record.get(index).unwrap_or_default().trim();
        counts.rows += 1;
        row_fn(EmissionRow {
            epa_id: field(1)
                .parse()
                .with_context(|| format!("{origin}, line {line}: id is {:?}", field(1)))?,
            sales_area: field(2)
                .parse()
                .with_context(|| format!("{origin}, line {line}: salesArea is {:?}", field(2)))?,
            standard: field(6).to_owned(),
            standard_text: Some(field(7))
                .filter(|text| !text.is_empty() && *text != "null")
                .map(str::to_owned),
            smog_score: score(field(3)),
            engine_family: field(0).to_owned(),
        })?;
    }
    Ok(counts)
}

/// Reads `emissions.csv.zip` in a fetched `epa-emissions` directory.
pub fn parse(
    directory: &Path,
    row_fn: &mut dyn FnMut(EmissionRow) -> Result<()>,
) -> Result<EmissionCounts> {
    let path = directory.join(FILE);
    let file = File::open(&path).with_context(|| {
        format!(
            "opening {}; run open-data fetch epa-emissions",
            path.display()
        )
    })?;
    let mut archive =
        zip::ZipArchive::new(file).with_context(|| format!("reading {}", path.display()))?;
    let entry = archive
        .by_name(INNER)
        .with_context(|| format!("{} holds no {INNER}", path.display()))?;
    parse_reader(entry, &path.display().to_string(), row_fn)
}

/// The rows of one vehicle, each certification once: the file repeats
/// some lines. Sorted by sales area, then standard, then engine family.
pub fn distinct(mut rows: Vec<EmissionRow>) -> Vec<EmissionRow> {
    rows.sort();
    rows.dedup();
    rows
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn scores_outside_one_to_ten_are_dropped() {
        assert_eq!(score("5.0"), Some(5));
        assert_eq!(score("10.0"), Some(10));
        assert_eq!(score("1"), Some(1));
        assert_eq!(score("-1.0"), None);
        assert_eq!(score("-12.0"), None);
        assert_eq!(score("0.0"), None);
        assert_eq!(score(""), None);
    }

    #[test]
    fn rows_are_read_and_another_header_is_refused() {
        let text = "efid,id,salesArea,score,scoreAlt,smartwayScore,standard,stdText\n\
                    JFMXT03.54DX,39241,3,5.0,-1.0,-1,T3B70,Federal Tier 3 Bin 70\n\
                    JFMXT03.54DX,39241,7,5.0,-1.0,-1,L3ULEV70,California LEV-III ULEV70\n\
                    6VWXV02.0227,22000,7,-1.0,-1.0,-1,L2OP,null\n";
        let mut rows = Vec::new();
        let counts = parse_reader(text.as_bytes(), "x.csv", &mut |row| {
            rows.push(row);
            Ok(())
        })
        .unwrap();
        assert_eq!(counts.rows, 3);
        assert_eq!(
            rows[0],
            EmissionRow {
                epa_id: 39241,
                sales_area: 3,
                standard: "T3B70".to_owned(),
                standard_text: Some("Federal Tier 3 Bin 70".to_owned()),
                smog_score: Some(5),
                engine_family: "JFMXT03.54DX".to_owned(),
            }
        );
        assert_eq!(rows[2].standard_text, None);
        assert_eq!(rows[2].smog_score, None);
        let mut twice = rows.clone();
        twice.extend(rows.clone());
        assert_eq!(distinct(twice), {
            let mut sorted = rows.clone();
            sorted.sort();
            sorted
        });
        let error = parse_reader("id,efid\n".as_bytes(), "x.csv", &mut |_| Ok(())).unwrap_err();
        assert!(format!("{error:#}").contains("another header"), "{error:#}");
    }
}
