//! Natural Resources Canada's fuel consumption ratings: finding the files,
//! fetching them and reading their three layouts.
//!
//! The files are listed by open.canada.ca's catalogue, and a new model
//! year adds a file, so the fetch asks the catalogue which files there
//! are instead of keeping a list.

use std::path::{Path, PathBuf};

use anyhow::{Context, Result, bail};
use serde::Deserialize;

use crate::sources::{self, Downloaded, FileRecord, SourceRecord};

/// The source id, and the directory under `sources/`.
pub const SOURCE: &str = "nrcan-fuel";

/// The dataset's record in open.canada.ca's catalogue (a CKAN API).
pub const CATALOGUE: &str =
    "https://open.canada.ca/data/api/action/package_show?id=98f1a129-f628-4ce4-b24d-6f16bf24dd64";

/// The file the catalogue's answer is saved as, beside the CSV files.
pub const CATALOGUE_FILE: &str = "package.json";

/// The licence id the catalogue must give for the dataset.
pub const LICENCE_ID: &str = "ca-ogl-lgo";

/// The header of the ratings of conventional and hybrid vehicles.
pub const CONVENTIONAL: [&str; 15] = [
    "Model year",
    "Make",
    "Model",
    "Vehicle class",
    "Engine size (L)",
    "Cylinders",
    "Transmission",
    "Fuel type",
    "City (L/100 km)",
    "Highway (L/100 km)",
    "Combined (L/100 km)",
    "Combined (mpg)",
    "CO2 emissions (g/km)",
    "CO2 rating",
    "Smog rating",
];

/// The header of the battery-electric ratings.
pub const ELECTRIC: [&str; 18] = [
    "Model year",
    "Make",
    "Model",
    "Vehicle class",
    "Motor (kW)",
    "Transmission",
    "Fuel type",
    "City (kWh/100 km)",
    "Highway (kWh/100 km)",
    "Combined (kWh/100 km)",
    "City (Le/100 km)",
    "Highway (Le/100 km)",
    "Combined (Le/100 km)",
    "Range (km)",
    "CO2 emissions (g/km)",
    "CO2 rating",
    "Smog rating",
    "Recharge time (h)",
];

/// The header of the plug-in hybrid ratings.
pub const PLUG_IN: [&str; 20] = [
    "Model year",
    "Make",
    "Model",
    "Vehicle class",
    "Motor (kW)",
    "Engine size (L)",
    "Cylinders",
    "Transmission",
    "Fuel type 1",
    "Combined Le/100 km",
    "Range 1 (km)",
    "Recharge time (h)",
    "Fuel type 2",
    "City (L/100 km)",
    "Highway (L/100 km)",
    "Combined (L/100 km)",
    "Range 2 (km)",
    "CO2 emissions (g/km)",
    "CO2 rating",
    "Smog rating",
];

/// Which of the three layouts a row came from.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NrcanKind {
    /// Petrol, diesel, flexible-fuel, natural gas and ordinary hybrids.
    Conventional,
    Electric,
    PlugIn,
}

/// One rating.
#[derive(Debug, Clone, PartialEq)]
pub struct NrcanRow {
    pub kind: NrcanKind,
    /// `Model year`.
    pub year: i32,
    /// `Make`.
    pub make_text: String,
    /// `Model`, with drive and trim in it: `MDX SH-AWD`, `F-150 FFV 4X4`.
    pub model_text: String,
    /// `Vehicle class`: `Compact`, `Sport utility vehicle: Small`.
    pub size_class: String,
    /// `Engine size (L)`. None for a battery-electric vehicle.
    pub displacement_l: Option<f64>,
    /// `Cylinders`.
    pub cylinders: Option<u8>,
    /// `Transmission`: `A8`, `AS10`, `AM7`, `AV`, `M6`. A is automatic, AM
    /// automated manual, AS automatic with select shift, AV continuously
    /// variable, M manual; the number is the gears.
    pub transmission: String,
    /// The fuel in words, from the code in `Fuel type` (or `Fuel type 2`
    /// for a plug-in hybrid, its fuel in gasoline-only mode): X is
    /// `Regular Gasoline`, Z `Premium Gasoline`, D `Diesel`, E `E85`,
    /// N `Natural Gas`, B `Electricity`.
    pub fuel: String,
    /// City, highway and combined consumption in L/100 km. For a plug-in
    /// hybrid these are the gasoline-only figures. None for a
    /// battery-electric vehicle.
    pub city_l_100km: Option<f64>,
    pub highway_l_100km: Option<f64>,
    pub combined_l_100km: Option<f64>,
    /// Combined electricity use in kWh/100 km: the electric file's
    /// column, or the figure in parentheses in a plug-in hybrid's
    /// `Combined Le/100 km`.
    pub kwh_100km: Option<f64>,
    /// Combined gasoline litres equivalent per 100 km in electric mode.
    pub le_100km: Option<f64>,
    /// Kilometres on a full battery: `Range (km)` or `Range 1 (km)`.
    pub range_km: Option<u16>,
    /// `CO2 emissions (g/km)`, combined.
    pub co2_g_km: Option<u16>,
    /// `CO2 rating` and `Smog rating`, 1 (worst) to 10 (best). None for
    /// `n/a`.
    pub co2_rating: Option<u8>,
    pub smog_rating: Option<u8>,
    /// `Motor (kW)`: peak power of the electric motor.
    pub motor_kw: Option<u16>,
    /// `Recharge time (h)` at 240 volts.
    pub recharge_h: Option<f64>,
}

/// What a parse read.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct NrcanCounts {
    pub files: u64,
    pub conventional: u64,
    pub electric: u64,
    pub plug_in: u64,
}

/// A fuel code in words. None for a code NRCan's own table does not list.
pub fn fuel_name(code: &str) -> Option<&'static str> {
    Some(match code {
        "X" => "Regular Gasoline",
        "Z" => "Premium Gasoline",
        "D" => "Diesel",
        "E" => "E85",
        "N" => "Natural Gas",
        "B" => "Electricity",
        _ => return None,
    })
}

/// `Some(true)` when the model name has the word `Hybrid`; otherwise None,
/// because NRCan lists ordinary hybrids among the conventional vehicles
/// without marking them.
pub fn named_hybrid(model_text: &str) -> Option<bool> {
    model_text
        .split_whitespace()
        .any(|word| word.eq_ignore_ascii_case("hybrid"))
        .then_some(true)
}

fn amount(field: &str) -> Option<f64> {
    field
        .trim()
        .parse::<f64>()
        .ok()
        .filter(|number| number.is_finite())
}

fn count<T: TryFrom<i64>>(field: &str) -> Option<T> {
    amount(field)
        .filter(|number| number.fract() == 0.0)
        .and_then(|number| T::try_from(number as i64).ok())
}

/// The two figures of `2.5 (22.3 kWh/100 km)` and of
/// `2.7 ([23.2 kWh + 0.1 L]/100 km)`: Le/100 km, then kWh/100 km.
pub fn electric_mode(field: &str) -> (Option<f64>, Option<f64>) {
    let field = field.trim();
    let le = field.split([' ', '(']).next().and_then(amount);
    let kwh = field
        .split_once('(')
        .map(|(_, inside)| inside.trim_start_matches('['))
        .and_then(|inside| inside.split(' ').next())
        .and_then(amount);
    (le, kwh)
}

/// The file's bytes as text. NRCan writes Latin-1 (`Coupé` is one byte),
/// so each byte is one character.
fn latin1(bytes: &[u8]) -> String {
    bytes.iter().map(|byte| *byte as char).collect()
}

/// Reads one file's bytes and hands on each row. The header says which
/// layout it is; a header that is none of the three is an error.
pub fn parse_bytes(
    bytes: &[u8],
    origin: &str,
    counts: &mut NrcanCounts,
    row_fn: &mut dyn FnMut(NrcanRow) -> Result<()>,
) -> Result<()> {
    let text = latin1(bytes);
    let mut reader = csv::ReaderBuilder::new()
        .has_headers(true)
        .from_reader(text.as_bytes());
    let header: Vec<String> = reader
        .headers()
        .with_context(|| format!("reading the header of {origin}"))?
        .iter()
        .map(|name| name.split_whitespace().collect::<Vec<_>>().join(" "))
        .collect();
    let kind = if header == CONVENTIONAL {
        NrcanKind::Conventional
    } else if header == ELECTRIC {
        NrcanKind::Electric
    } else if header == PLUG_IN {
        NrcanKind::PlugIn
    } else {
        bail!(
            "{origin} has a header this build does not read: {}",
            header.join(",")
        );
    };
    for (index, record) in reader.records().enumerate() {
        let line = index as u64 + 2;
        let record = record.with_context(|| format!("{origin}, line {line}"))?;
        let field = |name: &str| -> &str {
            header
                .iter()
                .position(|column| column == name)
                .and_then(|index| record.get(index))
                .unwrap_or_default()
                .trim()
        };
        let year: i32 = count(field("Model year"))
            .with_context(|| format!("{origin}, line {line}: no model year"))?;
        let code = match kind {
            NrcanKind::PlugIn => field("Fuel type 2"),
            _ => field("Fuel type"),
        };
        let fuel = fuel_name(code)
            .with_context(|| format!("{origin}, line {line}: fuel type {code:?} is not known"))?;
        let (le, kwh) = match kind {
            NrcanKind::Conventional => (None, None),
            NrcanKind::Electric => (
                amount(field("Combined (Le/100 km)")),
                amount(field("Combined (kWh/100 km)")),
            ),
            NrcanKind::PlugIn => electric_mode(field("Combined Le/100 km")),
        };
        match kind {
            NrcanKind::Conventional => counts.conventional += 1,
            NrcanKind::Electric => counts.electric += 1,
            NrcanKind::PlugIn => counts.plug_in += 1,
        }
        row_fn(NrcanRow {
            kind,
            year,
            make_text: field("Make").to_owned(),
            model_text: field("Model").to_owned(),
            size_class: field("Vehicle class").to_owned(),
            displacement_l: amount(field("Engine size (L)")).filter(|litres| *litres > 0.0),
            cylinders: count(field("Cylinders")),
            transmission: field("Transmission").to_owned(),
            fuel: fuel.to_owned(),
            city_l_100km: amount(field("City (L/100 km)")),
            highway_l_100km: amount(field("Highway (L/100 km)")),
            combined_l_100km: amount(field("Combined (L/100 km)")),
            kwh_100km: kwh,
            le_100km: le,
            range_km: count(field("Range (km)")).or_else(|| count(field("Range 1 (km)"))),
            co2_g_km: count(field("CO2 emissions (g/km)")),
            co2_rating: count(field("CO2 rating")),
            smog_rating: count(field("Smog rating")),
            motor_kw: count(field("Motor (kW)")),
            recharge_h: amount(field("Recharge time (h)")),
        })?;
    }
    counts.files += 1;
    Ok(())
}

/// Reads every `.csv` file in a fetched `nrcan-fuel` directory, in name
/// order.
pub fn parse(
    directory: &Path,
    row_fn: &mut dyn FnMut(NrcanRow) -> Result<()>,
) -> Result<NrcanCounts> {
    let mut paths: Vec<PathBuf> = std::fs::read_dir(directory)
        .with_context(|| {
            format!(
                "reading {}; run open-data fetch nrcan-fuel",
                directory.display()
            )
        })?
        .filter_map(|entry| entry.ok().map(|entry| entry.path()))
        .filter(|path| path.extension().is_some_and(|extension| extension == "csv"))
        .collect();
    paths.sort();
    if paths.is_empty() {
        bail!(
            "{} holds no .csv file; run open-data fetch nrcan-fuel",
            directory.display()
        );
    }
    let mut counts = NrcanCounts::default();
    for path in paths {
        let bytes = std::fs::read(&path).with_context(|| format!("reading {}", path.display()))?;
        parse_bytes(&bytes, &path.display().to_string(), &mut counts, row_fn)?;
    }
    Ok(counts)
}

#[derive(Debug, Deserialize)]
struct Package {
    success: bool,
    result: PackageResult,
}

#[derive(Debug, Deserialize)]
struct PackageResult {
    license_id: String,
    resources: Vec<Resource>,
}

#[derive(Debug, Deserialize)]
struct Resource {
    #[serde(default)]
    format: String,
    #[serde(default)]
    language: Vec<String>,
    url: String,
}

/// One file the catalogue lists.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Listed {
    pub url: String,
    /// The last part of the address: the name it is saved under.
    pub name: String,
}

/// The files to fetch, out of the catalogue's answer: every resource
/// whose format is `CSV` and whose only language is `en`, but for the
/// one whose file name starts with `original-` (the ratings of 1995 to
/// 2014 as first published, which the adjusted file replaces). Fails
/// unless the answer says `success` and the licence is [`LICENCE_ID`].
pub fn listed(package_json: &str) -> Result<Vec<Listed>> {
    let package: Package =
        serde_json::from_str(package_json).context("reading the catalogue's answer")?;
    if !package.success {
        bail!("the catalogue did not answer with success");
    }
    if package.result.license_id != LICENCE_ID {
        bail!(
            "the dataset's licence is now {:?}, not {LICENCE_ID:?}; a person must read the new licence before this is built",
            package.result.license_id
        );
    }
    let mut files = Vec::new();
    for resource in package.result.resources {
        if !resource.format.eq_ignore_ascii_case("CSV") || resource.language != ["en"] {
            continue;
        }
        let name = resource
            .url
            .rsplit('/')
            .next()
            .unwrap_or_default()
            .to_owned();
        let plain = !name.is_empty()
            && name.ends_with(".csv")
            && name
                .chars()
                .all(|character| character.is_ascii_alphanumeric() || "-_.".contains(character));
        if !plain || !resource.url.starts_with("https://") {
            bail!(
                "the catalogue lists a file with an unexpected address: {}",
                resource.url
            );
        }
        if name.starts_with("original-") {
            continue;
        }
        files.push(Listed {
            url: resource.url,
            name,
        });
    }
    if files.is_empty() {
        bail!("the catalogue lists no English CSV file");
    }
    Ok(files)
}

/// Fetches the catalogue's answer, then every file it lists, through
/// `download`, and writes `source.json`. Returns `package.json`, the CSV
/// files in the catalogue's order, then `source.json`.
pub fn fetch_with(
    build_dir: &Path,
    fetched_at: &str,
    download: &mut dyn FnMut(&str, &Path) -> Result<Downloaded>,
) -> Result<Vec<PathBuf>> {
    let directory = sources::directory(build_dir, SOURCE);
    std::fs::create_dir_all(&directory)
        .with_context(|| format!("creating {}", directory.display()))?;
    let package_path = directory.join(CATALOGUE_FILE);
    download(CATALOGUE, &package_path)?;
    let package = std::fs::read_to_string(&package_path)
        .with_context(|| format!("reading {}", package_path.display()))?;
    let files = listed(&package)?;
    // Files of an earlier fetch that the catalogue no longer lists would
    // be read as ratings, so they are removed first.
    for entry in std::fs::read_dir(&directory)?.filter_map(|entry| entry.ok()) {
        let path = entry.path();
        if path.extension().is_some_and(|extension| extension == "csv") {
            std::fs::remove_file(&path)?;
        }
    }
    let mut paths = vec![package_path];
    let mut records = Vec::new();
    for file in &files {
        let path = directory.join(&file.name);
        let part = directory.join(format!("{}.part", file.name));
        let downloaded = download(&file.url, &part)?;
        std::fs::rename(&part, &path).with_context(|| format!("writing {}", path.display()))?;
        records.push(FileRecord {
            name: file.name.clone(),
            url: file.url.clone(),
            last_modified: downloaded.last_modified,
            bytes: downloaded.bytes,
        });
        paths.push(path);
    }
    let as_of = records
        .iter()
        .filter_map(|file| file.last_modified.as_deref().and_then(sources::http_date))
        .max()
        .unwrap_or_else(|| fetched_at.chars().take(10).collect());
    let record = SourceRecord {
        source: SOURCE.to_owned(),
        fetched_at: fetched_at.to_owned(),
        as_of,
        files: records,
    };
    let record_path = directory.join("source.json");
    let mut text = serde_json::to_string_pretty(&record)?;
    text.push('\n');
    std::fs::write(&record_path, text)
        .with_context(|| format!("writing {}", record_path.display()))?;
    paths.push(record_path);
    Ok(paths)
}

/// Fetches the ratings from the network.
pub fn fetch(build_dir: &Path, fetched_at: &str) -> Result<Vec<PathBuf>> {
    fetch_with(build_dir, fetched_at, &mut sources::download)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fuel_codes_and_numbers() {
        assert_eq!(fuel_name("X"), Some("Regular Gasoline"));
        assert_eq!(fuel_name("Z"), Some("Premium Gasoline"));
        assert_eq!(fuel_name("B"), Some("Electricity"));
        assert_eq!(fuel_name("B/Z*"), None);
        assert_eq!(fuel_name(""), None);
        assert_eq!(amount("n/a"), None);
        assert_eq!(amount(" 8.5 "), Some(8.5));
        assert_eq!(count::<u8>("n/a"), None);
        assert_eq!(count::<u8>("10"), Some(10));
        assert_eq!(count::<u16>("2.5"), None);
        assert_eq!(
            electric_mode("2.5 (22.3 kWh/100 km)"),
            (Some(2.5), Some(22.3))
        );
        assert_eq!(
            electric_mode("2.7 ([23.2 kWh + 0.1 L]/100 km)"),
            (Some(2.7), Some(23.2))
        );
        assert_eq!(electric_mode("n/a"), (None, None));
        assert_eq!(latin1(b"Coup\xe9"), "Coup\u{e9}");
        assert_eq!(named_hybrid("Accord Hybrid"), Some(true));
        assert_eq!(named_hybrid("RAV4 hybrid AWD"), Some(true));
        assert_eq!(named_hybrid("Accord"), None);
    }

    #[test]
    fn the_catalogue_answer_gives_the_english_csv_files() {
        let answer = r#"{"success": true, "result": {"license_id": "ca-ogl-lgo", "resources": [
            {"format": "XLSX", "language": ["en"], "url": "https://open.canada.ca/data/dataset/x/resource/a/download/understanding-the-tables.xlsx"},
            {"format": "CSV", "language": ["en"], "url": "https://open.canada.ca/data/dataset/x/resource/b/download/my2026-fuel-consumption-ratings.csv"},
            {"format": "CSV", "language": ["fr"], "url": "https://open.canada.ca/data/dataset/x/resource/c/download/am2026-cotes-de-consommation-de-carburant.csv"},
            {"format": "CSV", "language": ["en"], "url": "https://open.canada.ca/data/dataset/x/resource/d/download/original-my1995-2014-fuel-consumption-ratings-2-cycle.csv"},
            {"format": "CSV", "language": ["en"], "url": "https://open.canada.ca/data/dataset/x/resource/e/download/my2012-2026-battery-electric-vehicles.csv"}
        ]}}"#;
        let files = listed(answer).unwrap();
        let names: Vec<&str> = files.iter().map(|file| file.name.as_str()).collect();
        assert_eq!(
            names,
            [
                "my2026-fuel-consumption-ratings.csv",
                "my2012-2026-battery-electric-vehicles.csv"
            ]
        );
        assert!(
            files[0]
                .url
                .ends_with("/b/download/my2026-fuel-consumption-ratings.csv")
        );
        for (answer, expected) in [
            (answer.replace("ca-ogl-lgo", "other"), "licence is now"),
            (
                answer.replace("true", "false"),
                "did not answer with success",
            ),
            (answer.replace("\"CSV\"", "\"TXT\""), "lists no English CSV"),
            (
                answer.replace("my2026-fuel-consumption-ratings.csv", "../x.csv?y=1"),
                "unexpected address",
            ),
            ("{}".to_owned(), "catalogue's answer"),
        ] {
            let error = listed(&answer).unwrap_err();
            assert!(format!("{error:#}").contains(expected), "{error:#}");
        }
    }
}
