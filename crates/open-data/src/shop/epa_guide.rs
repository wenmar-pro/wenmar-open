//! EPA's yearly Fuel Economy Guide files: fetching them, reading the
//! spreadsheet in each, and joining a row to its row of `vehicles.csv`.
//!
//! The guide has what `vehicles.csv` leaves out: the engine oil viscosity
//! the maker certified with, the number of gears, cylinder deactivation,
//! valves, passenger and luggage volume, and the hybrid battery.

use std::collections::HashMap;
use std::fs::File;
use std::io::Read;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result, bail};

use crate::shop::epa_vehicles::EpaVehicleRow;
use crate::shop::xlsx;
use crate::sources::{self, Downloaded, FileRecord, SourceRecord};

/// The source id, and the directory under `sources/`.
pub const SOURCE: &str = "epa-guide";

/// The first model year read. It is the first whose file has an
/// `Oil Viscosity` column. 2010 is a spreadsheet of the same kind without
/// it; 2009 and earlier are `.xls` or `.csv` in other layouts.
pub const FIRST_YEAR: i32 = 2011;

/// How many model years past the current calendar year are asked for.
pub const YEARS_AHEAD: i32 = 2;

/// The longest cylinder deactivation description kept. Longer ones are
/// sentences, not names.
pub const LONGEST_NAME: usize = 40;

/// The file of a model year, as EPA names it: `18data.zip`.
pub fn file_name(year: i32) -> String {
    format!("{:02}data.zip", year.rem_euclid(100))
}

/// Where a model year's file is.
pub fn url(year: i32) -> String {
    format!(
        "https://www.fueleconomy.gov/feg/epadata/{}",
        file_name(year)
    )
}

/// Whether a downloaded file is a zip archive.
fn is_zip(path: &Path) -> bool {
    let mut start = [0u8; 2];
    File::open(path)
        .and_then(|mut file| file.read_exact(&mut start))
        .is_ok()
        && &start == b"PK"
}

/// Fetches every year's file through `download` and writes `source.json`.
///
/// Years [`FIRST_YEAR`] to `this_year` must be there: a failed download
/// of one of them is an error. The years after, up to [`YEARS_AHEAD`]
/// more, are asked for in turn, and the first that is missing ends the
/// fetch quietly: EPA publishes a model year's file during the year
/// before. Returns the files' paths in year order, then `source.json`.
pub fn fetch_with(
    build_dir: &Path,
    fetched_at: &str,
    this_year: i32,
    download: &mut dyn FnMut(&str, &Path) -> Result<Downloaded>,
) -> Result<Vec<PathBuf>> {
    let directory = sources::directory(build_dir, SOURCE);
    std::fs::create_dir_all(&directory)
        .with_context(|| format!("creating {}", directory.display()))?;
    let mut paths = Vec::new();
    let mut files = Vec::new();
    for year in FIRST_YEAR..=this_year + YEARS_AHEAD {
        let name = file_name(year);
        let address = url(year);
        let path = directory.join(&name);
        let part = directory.join(format!("{name}.part"));
        let downloaded = match download(&address, &part) {
            Ok(downloaded) if is_zip(&part) => downloaded,
            outcome => {
                let _ = std::fs::remove_file(&part);
                if year > this_year {
                    break;
                }
                return Err(match outcome {
                    Err(error) => error.context(format!("fetching the {year} guide file")),
                    Ok(_) => anyhow::anyhow!("{address} is not a zip archive"),
                });
            }
        };
        std::fs::rename(&part, &path).with_context(|| format!("writing {}", path.display()))?;
        files.push(FileRecord {
            name,
            url: address,
            last_modified: downloaded.last_modified,
            bytes: downloaded.bytes,
        });
        paths.push(path);
    }
    let as_of = files
        .iter()
        .filter_map(|file| file.last_modified.as_deref().and_then(sources::http_date))
        .max()
        .unwrap_or_else(|| fetched_at.chars().take(10).collect());
    let record = SourceRecord {
        source: SOURCE.to_owned(),
        fetched_at: fetched_at.to_owned(),
        as_of,
        files,
    };
    let record_path = directory.join("source.json");
    let mut text = serde_json::to_string_pretty(&record)?;
    text.push('\n');
    std::fs::write(&record_path, text)
        .with_context(|| format!("writing {}", record_path.display()))?;
    paths.push(record_path);
    Ok(paths)
}

/// Fetches the guide files from the network. `fetched_at` is UTC,
/// `YYYY-MM-DD HH:MM:SS`; its year is the current calendar year.
pub fn fetch(build_dir: &Path, fetched_at: &str) -> Result<Vec<PathBuf>> {
    let this_year: i32 = fetched_at
        .get(..4)
        .and_then(|year| year.parse().ok())
        .with_context(|| format!("{fetched_at:?} does not start with a year"))?;
    fetch_with(build_dir, fetched_at, this_year, &mut sources::download)
}

/// A column the parser reads: the names its header has had over the
/// years, and whether a file without it is refused.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Column {
    pub field: &'static str,
    pub names: &'static [&'static str],
    pub required: bool,
}

/// Every column read, by header text. Header cells are trimmed and blanks
/// inside them squeezed before comparing; letter case matters. Columns
/// are found by name, never by position: EPA has added and moved columns
/// between years.
pub const COLUMNS: [Column; 30] = [
    Column {
        field: "year",
        names: &["Model Year", "Model Yr"],
        required: true,
    },
    Column {
        field: "mfr_code",
        names: &["Verify Mfr Cd"],
        required: true,
    },
    Column {
        field: "division",
        names: &["Division"],
        required: true,
    },
    Column {
        field: "carline",
        names: &["Carline"],
        required: true,
    },
    Column {
        field: "index",
        names: &["Index (Model Type Index)"],
        required: true,
    },
    Column {
        field: "displacement",
        names: &["Eng Displ"],
        required: true,
    },
    Column {
        field: "cylinders",
        names: &["# Cyl"],
        required: true,
    },
    Column {
        field: "transmission",
        names: &[
            "Transmission",
            "Trans in FE Guide (MFR entered for data entered after May 13 2011)",
            "Trans as listed in FE Guide (derived from col AA thru AF)",
        ],
        required: true,
    },
    Column {
        field: "aspiration",
        names: &["Air Aspiration Method Desc"],
        required: false,
    },
    Column {
        field: "transmission_type",
        names: &["Trans Desc"],
        required: false,
    },
    Column {
        field: "gears",
        names: &["# Gears"],
        required: false,
    },
    Column {
        field: "drive",
        names: &["Drive Desc"],
        required: false,
    },
    Column {
        field: "fuel_usage",
        names: &["Fuel Usage Desc - Conventional Fuel"],
        required: false,
    },
    Column {
        field: "passenger_2door",
        names: &["2Dr Pass Vol"],
        required: false,
    },
    Column {
        field: "luggage_2door",
        names: &["2Dr Lugg Vol"],
        required: false,
    },
    Column {
        field: "passenger_4door",
        names: &["4Dr Pass Vol"],
        required: false,
    },
    Column {
        field: "luggage_4door",
        names: &["4Dr Lugg Vol"],
        required: false,
    },
    Column {
        field: "passenger_hatch",
        names: &["Htchbk Pass Vol"],
        required: false,
    },
    Column {
        field: "luggage_hatch",
        names: &["Htchbk Lugg Vol"],
        required: false,
    },
    Column {
        field: "intake_valves",
        names: &["Intake Valves Per Cyl"],
        required: false,
    },
    Column {
        field: "exhaust_valves",
        names: &["Exhaust Valves Per Cyl"],
        required: false,
    },
    Column {
        field: "cylinder_deactivation",
        names: &["Cyl Deact?"],
        required: false,
    },
    Column {
        field: "cylinder_deactivation_name",
        names: &["Cyl Deact Desc"],
        required: false,
    },
    Column {
        field: "variable_valve_timing",
        names: &["Var Valve Timing?"],
        required: false,
    },
    Column {
        field: "variable_valve_lift",
        names: &["Var Valve Lift?"],
        required: false,
    },
    Column {
        field: "battery_type",
        names: &["Battery Type Desc"],
        required: false,
    },
    Column {
        field: "battery_voltage",
        names: &["Total Voltage for Battery Pack(s)"],
        required: false,
    },
    Column {
        field: "motor_kw",
        names: &["Rated Motor Gen Power (kW)"],
        required: false,
    },
    Column {
        field: "oil",
        names: &["Oil Viscosity"],
        required: false,
    },
    Column {
        field: "stop_start",
        names: &[
            "Stop/Start System (Engine Management System) Code",
            "Eng Mgmt System (Stop/Start Sys) Code",
        ],
        required: false,
    },
];

/// One row of a guide file: one model, engine and transmission.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct GuideRow {
    /// `Model Year`.
    pub year: i32,
    /// `Verify Mfr Cd`: `FMX`. The same code as `vehicles.csv` `mfrCode`.
    pub mfr_code: String,
    /// `Division`: the make, in the maker's own capitals.
    pub division: String,
    /// `Carline`: the model, such as `F150 PICKUP 2WD`.
    pub carline: String,
    /// `Index (Model Type Index)`: `vehicles.csv` `engId`.
    pub index: u32,
    /// `Eng Displ`, litres to one decimal.
    pub displacement_l: Option<f64>,
    /// `# Cyl`.
    pub cylinders: Option<u8>,
    /// `Transmission`: `Auto(S10)`, `Manual(M6)`.
    pub transmission: String,
    /// `Air Aspiration Method Desc`: `Naturally Aspirated`, `Turbocharged`,
    /// `Supercharged`, `Turbocharged+Supercharged`, `Other`.
    pub aspiration: Option<String>,
    /// `Trans Desc`: `Automatic`, `Semi-Automatic`, `Manual`,
    /// `Continuously Variable`, `Automated Manual` and three longer ones.
    pub transmission_type: Option<String>,
    /// `# Gears`: 1 to 10. 1 is what a continuously variable one has.
    pub gears: Option<u8>,
    /// `Drive Desc`: `All Wheel Drive`, `2-Wheel Drive, Rear`,
    /// `2-Wheel Drive, Front`, `4-Wheel Drive`, `Part-time 4-Wheel Drive`.
    pub drive: Option<String>,
    /// `Fuel Usage Desc - Conventional Fuel`: says whether premium is
    /// recommended or required, which `vehicles.csv` does not.
    pub fuel_usage: Option<String>,
    /// Passenger and luggage volume in cubic feet, by body.
    pub passenger_ft3_2door: Option<u16>,
    pub luggage_ft3_2door: Option<u16>,
    pub passenger_ft3_4door: Option<u16>,
    pub luggage_ft3_4door: Option<u16>,
    pub passenger_ft3_hatch: Option<u16>,
    pub luggage_ft3_hatch: Option<u16>,
    /// `Intake Valves Per Cyl` and `Exhaust Valves Per Cyl`.
    pub intake_valves: Option<u8>,
    pub exhaust_valves: Option<u8>,
    /// `Cyl Deact?`: `Y` or `N`.
    pub cylinder_deactivation: Option<bool>,
    /// `Cyl Deact Desc`, kept only when deactivation is `Y` and the text
    /// is a name: at most [`LONGEST_NAME`] characters with a letter in it.
    pub cylinder_deactivation_name: Option<String>,
    /// `Var Valve Timing?` and `Var Valve Lift?`: `Y` or `N`.
    pub variable_valve_timing: Option<bool>,
    pub variable_valve_lift: Option<bool>,
    /// `Battery Type Desc`: `Lithium Ion`, `NiMH`. None for `Other`.
    pub battery_type: Option<String>,
    /// `Total Voltage for Battery Pack(s)`.
    pub battery_voltage: Option<u16>,
    /// `Rated Motor Gen Power (kW)`, as written: `135`, `8 and 30`.
    pub motor_kw: Option<String>,
    /// `Oil Viscosity`, as written: `5W30 GF-5`. `oil_grade::read` reads it.
    pub oil_text: Option<String>,
    /// The stop-start code: `Y` or `N`.
    pub stop_start: Option<bool>,
}

/// What a parse read.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct GuideCounts {
    /// Files read, one per model year.
    pub files: u64,
    pub rows: u64,
    /// Rows left out because they had no carline or no index.
    pub skipped: u64,
    /// `(model year, field)` for each optional column a file did not have.
    pub missing: Vec<(i32, &'static str)>,
}

fn squeezed(text: &str) -> String {
    text.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn text(cell: &str) -> Option<String> {
    let cell = squeezed(cell);
    (!cell.is_empty()).then_some(cell)
}

/// A spreadsheet number: `2018`, `6.0`, `4.4000000000000004`.
fn number(cell: &str) -> Option<f64> {
    cell.trim()
        .parse::<f64>()
        .ok()
        .filter(|number| number.is_finite())
}

fn count<T: TryFrom<i64>>(cell: &str) -> Option<T> {
    number(cell)
        .filter(|number| number.fract() == 0.0 && *number > 0.0)
        .and_then(|number| T::try_from(number as i64).ok())
}

fn yes_no(cell: &str) -> Option<bool> {
    match cell.trim() {
        "Y" | "y" => Some(true),
        "N" | "n" => Some(false),
        _ => None,
    }
}

/// The rows of one sheet. `rows[0]` is the header. `year` is the model
/// year of the file, used in errors and in `missing`.
pub fn parse_sheet(
    rows: &[Vec<String>],
    year: i32,
    counts: &mut GuideCounts,
    row_fn: &mut dyn FnMut(GuideRow) -> Result<()>,
) -> Result<()> {
    let header: Vec<String> = rows
        .first()
        .context("the sheet is empty")?
        .iter()
        .map(|cell| squeezed(cell))
        .collect();
    let mut at: HashMap<&'static str, usize> = HashMap::new();
    for column in &COLUMNS {
        let found = column
            .names
            .iter()
            .find_map(|name| header.iter().position(|cell| cell == name));
        match found {
            Some(index) => {
                at.insert(column.field, index);
            }
            None if column.required => bail!(
                "the {year} guide file has no column {:?}; it is needed to tell rows apart",
                column.names[0]
            ),
            None => counts.missing.push((year, column.field)),
        }
    }
    for cells in &rows[1..] {
        let get = |field: &str| -> &str {
            at.get(field)
                .and_then(|index| cells.get(*index))
                .map_or("", String::as_str)
        };
        let (Some(carline), Some(index)) = (text(get("carline")), count::<u32>(get("index")))
        else {
            counts.skipped += 1;
            continue;
        };
        let cylinder_deactivation = yes_no(get("cylinder_deactivation"));
        counts.rows += 1;
        row_fn(GuideRow {
            year: count(get("year")).unwrap_or(year),
            mfr_code: squeezed(get("mfr_code")),
            division: squeezed(get("division")),
            carline,
            index,
            displacement_l: number(get("displacement"))
                .filter(|litres| *litres > 0.0)
                .map(|litres| (litres * 10.0).round() / 10.0),
            cylinders: count(get("cylinders")),
            transmission: squeezed(get("transmission")),
            aspiration: text(get("aspiration")),
            transmission_type: text(get("transmission_type")),
            gears: count(get("gears")),
            drive: text(get("drive")),
            fuel_usage: text(get("fuel_usage")),
            passenger_ft3_2door: count(get("passenger_2door")),
            luggage_ft3_2door: count(get("luggage_2door")),
            passenger_ft3_4door: count(get("passenger_4door")),
            luggage_ft3_4door: count(get("luggage_4door")),
            passenger_ft3_hatch: count(get("passenger_hatch")),
            luggage_ft3_hatch: count(get("luggage_hatch")),
            intake_valves: count(get("intake_valves")),
            exhaust_valves: count(get("exhaust_valves")),
            cylinder_deactivation,
            cylinder_deactivation_name: text(get("cylinder_deactivation_name")).filter(|name| {
                cylinder_deactivation == Some(true)
                    && name.chars().count() <= LONGEST_NAME
                    && name
                        .chars()
                        .any(|character| character.is_ascii_alphabetic())
            }),
            variable_valve_timing: yes_no(get("variable_valve_timing")),
            variable_valve_lift: yes_no(get("variable_valve_lift")),
            battery_type: text(get("battery_type")).filter(|kind| {
                kind != "Other"
                    && kind
                        .chars()
                        .any(|character| character.is_ascii_alphabetic())
            }),
            battery_voltage: count(get("battery_voltage")),
            motor_kw: text(get("motor_kw")),
            oil_text: text(get("oil")),
            stop_start: yes_no(get("stop_start")),
        })?;
    }
    Ok(())
}

/// The model year in a guide file's name: `18data.zip` is 2018. Two-digit
/// years are all read as 20xx; nothing before 2011 is read.
pub fn year_of(name: &str) -> Option<i32> {
    let digits = name.strip_suffix("data.zip")?;
    (digits.len() == 2 && digits.bytes().all(|byte| byte.is_ascii_digit()))
        .then(|| 2000 + digits.parse::<i32>().expect("two digits"))
}

/// Reads every guide file in a fetched `epa-guide` directory, oldest
/// first, and hands on each row of each file's first sheet.
pub fn parse(
    directory: &Path,
    row_fn: &mut dyn FnMut(GuideRow) -> Result<()>,
) -> Result<GuideCounts> {
    let mut years: Vec<(i32, PathBuf)> = std::fs::read_dir(directory)
        .with_context(|| {
            format!(
                "reading {}; run open-data fetch epa-guide",
                directory.display()
            )
        })?
        .filter_map(|entry| entry.ok())
        .filter_map(|entry| {
            let year = year_of(&entry.file_name().to_string_lossy())?;
            (year >= FIRST_YEAR).then(|| (year, entry.path()))
        })
        .collect();
    years.sort();
    if years.is_empty() {
        bail!(
            "{} holds no guide file; run open-data fetch epa-guide",
            directory.display()
        );
    }
    let mut counts = GuideCounts::default();
    for (year, path) in years {
        let file = File::open(&path).with_context(|| format!("opening {}", path.display()))?;
        let mut archive =
            zip::ZipArchive::new(file).with_context(|| format!("reading {}", path.display()))?;
        let name = archive
            .file_names()
            .find(|name| name.to_ascii_lowercase().ends_with(".xlsx"))
            .map(str::to_owned)
            .with_context(|| format!("{} holds no .xlsx file", path.display()))?;
        let mut workbook = Vec::new();
        archive
            .by_name(&name)?
            .read_to_end(&mut workbook)
            .with_context(|| format!("reading {name} in {}", path.display()))?;
        let rows = xlsx::first_sheet(&workbook)
            .with_context(|| format!("reading {name} in {}", path.display()))?;
        parse_sheet(&rows, year, &mut counts, row_fn)
            .with_context(|| format!("reading {name} in {}", path.display()))?;
        counts.files += 1;
    }
    Ok(counts)
}

/// The short code of a `vehicles.csv` transmission: `Automatic (S10)` is
/// `S10`, `Automatic 6-spd` is `A6`, `Manual 6-spd` is `M6`, and
/// `Automatic (variable gear ratios)` is `AV`.
pub fn vehicle_transmission_code(trany: &str) -> String {
    let trany = trany.trim();
    if let Some(inside) = trany
        .strip_prefix("Automatic (")
        .and_then(|rest| rest.strip_suffix(')'))
    {
        return if inside == "variable gear ratios" {
            "AV".to_owned()
        } else {
            inside.to_owned()
        };
    }
    for (prefix, letter) in [("Automatic ", "A"), ("Manual ", "M")] {
        if let Some(speeds) = trany
            .strip_prefix(prefix)
            .and_then(|rest| rest.strip_suffix("-spd"))
        {
            return format!("{letter}{speeds}");
        }
    }
    trany.to_owned()
}

/// The short code of a guide transmission: `Auto(S10)` is `S10` and
/// `Manual(M6)` is `M6`.
pub fn guide_transmission_code(transmission: &str) -> String {
    let transmission = transmission.trim();
    transmission
        .split_once('(')
        .and_then(|(_, rest)| rest.strip_suffix(')'))
        .map_or_else(|| transmission.to_owned(), |code| code.trim().to_owned())
}

/// Finds the `vehicles.csv` row a guide row describes.
pub struct GuideJoin {
    /// `(year, maker code, index)` to the positions of its rows.
    coded: HashMap<(i32, String, u32), Vec<usize>>,
    /// `(year, index)` of rows with no maker code.
    uncoded: HashMap<(i32, u32), Vec<usize>>,
    /// Litres in tenths, cylinders, transmission code, EPA id.
    rows: Vec<(Option<i64>, Option<u8>, String, u32)>,
}

fn tenths(litres: Option<f64>) -> Option<i64> {
    litres.map(|litres| (litres * 10.0).round() as i64)
}

impl GuideJoin {
    pub fn new<'a>(vehicles: impl IntoIterator<Item = &'a EpaVehicleRow>) -> GuideJoin {
        let mut join = GuideJoin {
            coded: HashMap::new(),
            uncoded: HashMap::new(),
            rows: Vec::new(),
        };
        for vehicle in vehicles {
            let position = join.rows.len();
            join.rows.push((
                tenths(vehicle.displacement_l),
                vehicle.cylinders,
                vehicle_transmission_code(vehicle.transmission.as_deref().unwrap_or("")),
                vehicle.id,
            ));
            match &vehicle.mfr_code {
                Some(code) => join
                    .coded
                    .entry((vehicle.year, code.clone(), vehicle.engine_index))
                    .or_default()
                    .push(position),
                None => join
                    .uncoded
                    .entry((vehicle.year, vehicle.engine_index))
                    .or_default()
                    .push(position),
            }
        }
        join
    }

    /// The EPA id of the `vehicles.csv` row for a guide row.
    ///
    /// 1. The rows with the same model year, maker code and index. One:
    ///    that row. Several: the one with the same litres, cylinders and
    ///    transmission code, if exactly one has.
    /// 2. With none, the rows of that model year and index that have no
    ///    maker code (model years before 2012), and the same litres,
    ///    cylinders and transmission code. Exactly one: that row.
    pub fn find(&self, row: &GuideRow) -> Option<u32> {
        let same = |position: &&usize| {
            let (litres, cylinders, transmission, _) = &self.rows[**position];
            *litres == tenths(row.displacement_l)
                && *cylinders == row.cylinders
                && *transmission == guide_transmission_code(&row.transmission)
        };
        let id = |position: &usize| self.rows[*position].3;
        if let Some(coded) = self.coded.get(&(row.year, row.mfr_code.clone(), row.index)) {
            return match coded.as_slice() {
                [only] => Some(id(only)),
                several => {
                    let mut matching = several.iter().filter(same);
                    match (matching.next(), matching.next()) {
                        (Some(only), None) => Some(id(only)),
                        _ => None,
                    }
                }
            };
        }
        let uncoded = self.uncoded.get(&(row.year, row.index))?;
        let mut matching = uncoded.iter().filter(same);
        match (matching.next(), matching.next()) {
            (Some(only), None) => Some(id(only)),
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn file_names_and_years() {
        assert_eq!(file_name(2011), "11data.zip");
        assert_eq!(file_name(2027), "27data.zip");
        assert_eq!(
            url(2018),
            "https://www.fueleconomy.gov/feg/epadata/18data.zip"
        );
        assert_eq!(year_of("18data.zip"), Some(2018));
        assert_eq!(year_of("09data.zip"), Some(2009));
        assert_eq!(year_of("source.json"), None);
        assert_eq!(year_of("118data.zip"), None);
        assert_eq!(year_of("18data.zip.part"), None);
    }

    #[test]
    fn cells_are_read_as_the_spreadsheet_stores_them() {
        assert_eq!(number("4.4000000000000004"), Some(4.4));
        assert_eq!(count::<u8>("6"), Some(6));
        assert_eq!(count::<u8>("6.0"), Some(6));
        assert_eq!(count::<u8>("0"), None);
        assert_eq!(count::<u8>("]"), None);
        assert_eq!(count::<u8>(""), None);
        assert_eq!(count::<u16>("259"), Some(259));
        assert_eq!(yes_no("Y"), Some(true));
        assert_eq!(yes_no(" N "), Some(false));
        assert_eq!(yes_no(""), None);
        assert_eq!(text("  5W30   GF-5 ").as_deref(), Some("5W30 GF-5"));
        assert_eq!(text("   "), None);
    }

    #[test]
    fn transmission_codes_agree_between_the_two_files() {
        for (vehicle, guide) in [
            ("Automatic (S10)", "Auto(S10)"),
            ("Automatic 6-spd", "Auto(A6)"),
            ("Manual 6-spd", "Manual(M6)"),
            ("Automatic (variable gear ratios)", "Auto(AV)"),
            ("Automatic (AM-S7)", "Auto(AM-S7)"),
            ("Automatic (AV-S6)", "Auto(AV-S6)"),
            ("Automatic (A1)", "Auto(A1)"),
        ] {
            assert_eq!(
                vehicle_transmission_code(vehicle),
                guide_transmission_code(guide),
                "{vehicle}"
            );
        }
        assert_eq!(vehicle_transmission_code("Automatic 10-spd"), "A10");
        assert_eq!(guide_transmission_code("CVT"), "CVT");
    }
}
