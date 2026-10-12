//! Reads EPA's fuel economy file, `vehicles.csv`, as fetched.
//!
//! One row is one model year, make, model and powertrain. Nothing here
//! touches the catalog or a database: matching is `epa_match`.

use std::fs::File;
use std::io::Read;
use std::path::Path;

use anyhow::{Context, Result, bail};

/// The archive `open-data fetch epa-vehicles` saves.
pub const FILE: &str = "vehicles.csv.zip";

/// The one file inside it.
pub const INNER: &str = "vehicles.csv";

/// The column names of `vehicles.csv`, in order. The parser reads columns
/// by these names and refuses a file whose header is anything else.
pub const HEADER: [&str; 84] = [
    "barrels08",
    "barrelsA08",
    "charge120",
    "charge240",
    "city08",
    "city08U",
    "cityA08",
    "cityA08U",
    "cityCD",
    "cityE",
    "cityUF",
    "co2",
    "co2A",
    "co2TailpipeAGpm",
    "co2TailpipeGpm",
    "comb08",
    "comb08U",
    "combA08",
    "combA08U",
    "combE",
    "combinedCD",
    "combinedUF",
    "cylinders",
    "displ",
    "drive",
    "engId",
    "eng_dscr",
    "feScore",
    "fuelCost08",
    "fuelCostA08",
    "fuelType",
    "fuelType1",
    "ghgScore",
    "ghgScoreA",
    "highway08",
    "highway08U",
    "highwayA08",
    "highwayA08U",
    "highwayCD",
    "highwayE",
    "highwayUF",
    "hlv",
    "hpv",
    "id",
    "lv2",
    "lv4",
    "make",
    "model",
    "mpgData",
    "phevBlended",
    "pv2",
    "pv4",
    "range",
    "rangeCity",
    "rangeCityA",
    "rangeHwy",
    "rangeHwyA",
    "trany",
    "UCity",
    "UCityA",
    "UHighway",
    "UHighwayA",
    "VClass",
    "year",
    "youSaveSpend",
    "baseModel",
    "guzzler",
    "trans_dscr",
    "tCharger",
    "sCharger",
    "atvType",
    "fuelType2",
    "rangeA",
    "evMotor",
    "mfrCode",
    "c240Dscr",
    "charge240b",
    "c240bDscr",
    "createdOn",
    "modifiedOn",
    "startStop",
    "phevCity",
    "phevHwy",
    "phevComb",
];

/// One row of `vehicles.csv`: the columns later plans use.
#[derive(Debug, Clone, PartialEq)]
pub struct EpaVehicleRow {
    /// `id`: EPA's own number for the row. `emissions.csv` joins on it.
    pub id: u32,
    /// `year`: the model year.
    pub year: i32,
    /// `make`: the division, such as `Ford`.
    pub make_text: String,
    /// `model`: the carline, with body and drive in it: `F150 Pickup 2WD`.
    pub model_text: String,
    /// `baseModel`: EPA's own family name: `F150`, `3 Series`.
    pub base_model: String,
    /// `displ`: litres. None for an electric vehicle.
    pub displacement_l: Option<f64>,
    /// `cylinders`. None for an electric vehicle and for `NA`.
    pub cylinders: Option<u8>,
    /// `trany`: `Automatic (S10)`, `Manual 6-spd`.
    pub transmission: Option<String>,
    /// `drive`: `Rear-Wheel Drive`, `Part-time 4-Wheel Drive`.
    pub drive: Option<String>,
    /// `fuelType`: both fuels in one phrase, such as `Gasoline or E85`.
    pub fuel: String,
    /// `fuelType1`: the main fuel, which is also the recommended grade:
    /// `Regular Gasoline`, `Midgrade Gasoline`, `Premium Gasoline`,
    /// `Diesel`, `Electricity`, `Natural Gas`, `Hydrogen`.
    pub fuel1: Option<String>,
    /// `fuelType2`: `E85`, `Electricity`, `Natural Gas`, `Propane`.
    pub fuel2: Option<String>,
    /// `VClass`: EPA's size class, such as `Standard Pickup Trucks 2WD`.
    pub size_class: String,
    /// `atvType`: `Hybrid`, `Plug-in Hybrid`, `EV`, `FFV`, `Diesel`, `CNG`,
    /// `FCV`, `eFCV`, `Bifuel (CNG)`, `Bifuel (LPG)`.
    pub atv_type: Option<String>,
    /// `tCharger` is `T`.
    pub turbo: bool,
    /// `sCharger` is `S`.
    pub supercharger: bool,
    /// `startStop`: `Y` or `N`; None for older rows, where it is empty.
    pub start_stop: Option<bool>,
    /// `guzzler` is `G`, `T` or `S`: subject to the gas guzzler tax.
    pub guzzler: bool,
    /// `engId`: the model type index. With `mfr_code` and the year it
    /// joins a row of the yearly guide file.
    pub engine_index: u32,
    /// `mfrCode`: three letters, such as `FMX`. Empty before 2012.
    pub mfr_code: Option<String>,
    /// `eng_dscr`: the engine descriptor, such as `SIDI; Mild Hybrid`.
    pub engine_text: Option<String>,
    /// `trans_dscr`: the transmission descriptor, such as `CLKUP`.
    pub transmission_text: Option<String>,
    /// `evMotor`: `48V Li-Ion`, `211 kW AC 3-Phase`.
    pub ev_motor: Option<String>,
    /// `city08`, `highway08`, `comb08`: miles per US gallon on `fuel1`,
    /// rounded as on the label. For electricity it is MPGe.
    pub city_mpg: u16,
    pub highway_mpg: u16,
    pub combined_mpg: u16,
    /// `cityA08`, `highwayA08`, `combA08`: the same on `fuel2`. None for 0.
    pub city_mpg2: Option<u16>,
    pub highway_mpg2: Option<u16>,
    pub combined_mpg2: Option<u16>,
    /// `co2TailpipeGpm`: tailpipe CO2 in grams per mile on `fuel1`. 0 for
    /// an electric vehicle.
    pub co2_g_mi: f64,
    /// `combE`: kWh per 100 miles. None for 0.
    pub kwh_100mi: Option<f64>,
    /// `range`: miles on a full charge or a full tank of `fuel1`. EPA
    /// fills it for electric and hydrogen vehicles and for plug-in
    /// hybrids, where it is the range on gasoline. None for 0.
    pub range_mi: Option<u16>,
    /// `rangeA`: miles on `fuel2`. For a plug-in hybrid this is the
    /// range on a full battery. None when empty or not a whole number.
    pub range2_mi: Option<u16>,
    /// `charge240`: hours to charge at 240 V. None for 0.
    pub charge_240v_h: Option<f64>,
    /// `phevComb`: combined MPGe of a plug-in hybrid. None for 0.
    pub phev_combined_mpge: Option<u16>,
    /// `pv2`, `pv4`, `hpv`: passenger volume in cubic feet of the two-door,
    /// four-door and hatchback bodies. None for 0.
    pub passenger_ft3_2door: Option<u16>,
    pub passenger_ft3_4door: Option<u16>,
    pub passenger_ft3_hatch: Option<u16>,
    /// `lv2`, `lv4`, `hlv`: luggage volume in cubic feet. None for 0.
    pub luggage_ft3_2door: Option<u16>,
    pub luggage_ft3_4door: Option<u16>,
    pub luggage_ft3_hatch: Option<u16>,
}

/// What a parse read.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct EpaCounts {
    pub rows: u64,
}

/// The position of each column the parser reads.
struct Columns {
    at: [usize; 84],
}

impl Columns {
    fn get<'a>(&self, record: &'a csv::StringRecord, name: &str) -> &'a str {
        let index = HEADER
            .iter()
            .position(|column| *column == name)
            .expect("a column named in this file is in HEADER");
        record.get(self.at[index]).unwrap_or_default().trim()
    }
}

/// Trimmed text, None when empty.
fn text(field: &str) -> Option<String> {
    let field = field.trim();
    (!field.is_empty()).then(|| field.to_owned())
}

/// A whole number written `19` or `19.0`. None when empty or not a number.
fn whole(field: &str) -> Option<i64> {
    let number: f64 = field.trim().parse().ok()?;
    (number.is_finite() && number.fract() == 0.0).then_some(number as i64)
}

/// A count where the file writes 0 for "none".
fn positive(field: &str) -> Option<u16> {
    whole(field)
        .filter(|number| *number > 0)
        .and_then(|number| u16::try_from(number).ok())
}

/// An amount where the file writes 0 for "none".
fn amount(field: &str) -> Option<f64> {
    field
        .trim()
        .parse::<f64>()
        .ok()
        .filter(|number| number.is_finite() && *number > 0.0)
}

fn row(columns: &Columns, record: &csv::StringRecord, line: u64) -> Result<EpaVehicleRow> {
    let get = |name: &str| columns.get(record, name);
    let need = |name: &str| -> Result<i64> {
        whole(get(name)).with_context(|| format!("line {line}: {name} is {:?}", get(name)))
    };
    let mpg = |name: &str| -> Result<u16> {
        u16::try_from(need(name)?).with_context(|| format!("line {line}: {name} is out of range"))
    };
    Ok(EpaVehicleRow {
        id: u32::try_from(need("id")?).with_context(|| format!("line {line}: id"))?,
        year: i32::try_from(need("year")?).with_context(|| format!("line {line}: year"))?,
        make_text: get("make").to_owned(),
        model_text: get("model").to_owned(),
        base_model: get("baseModel").to_owned(),
        displacement_l: amount(get("displ")),
        cylinders: whole(get("cylinders")).and_then(|count| u8::try_from(count).ok()),
        transmission: text(get("trany")),
        drive: text(get("drive")),
        fuel: get("fuelType").to_owned(),
        fuel1: text(get("fuelType1")),
        fuel2: text(get("fuelType2")),
        size_class: get("VClass").to_owned(),
        atv_type: text(get("atvType")),
        turbo: get("tCharger") == "T",
        supercharger: get("sCharger") == "S",
        start_stop: match get("startStop") {
            "Y" => Some(true),
            "N" => Some(false),
            _ => None,
        },
        guzzler: !get("guzzler").is_empty(),
        engine_index: u32::try_from(need("engId")?)
            .with_context(|| format!("line {line}: engId"))?,
        mfr_code: text(get("mfrCode")),
        engine_text: text(get("eng_dscr")),
        transmission_text: text(get("trans_dscr")),
        ev_motor: text(get("evMotor")),
        city_mpg: mpg("city08")?,
        highway_mpg: mpg("highway08")?,
        combined_mpg: mpg("comb08")?,
        city_mpg2: positive(get("cityA08")),
        highway_mpg2: positive(get("highwayA08")),
        combined_mpg2: positive(get("combA08")),
        co2_g_mi: get("co2TailpipeGpm")
            .parse()
            .with_context(|| format!("line {line}: co2TailpipeGpm"))?,
        kwh_100mi: amount(get("combE")),
        range_mi: positive(get("range")),
        range2_mi: positive(get("rangeA")),
        charge_240v_h: amount(get("charge240")),
        phev_combined_mpge: positive(get("phevComb")),
        passenger_ft3_2door: positive(get("pv2")),
        passenger_ft3_4door: positive(get("pv4")),
        passenger_ft3_hatch: positive(get("hpv")),
        luggage_ft3_2door: positive(get("lv2")),
        luggage_ft3_4door: positive(get("lv4")),
        luggage_ft3_hatch: positive(get("hlv")),
    })
}

/// Reads the CSV text of `vehicles.csv` and hands on each row. `origin`
/// names the file in an error.
pub fn parse_reader(
    reader: impl Read,
    origin: &str,
    row_fn: &mut dyn FnMut(EpaVehicleRow) -> Result<()>,
) -> Result<EpaCounts> {
    let mut reader = csv::ReaderBuilder::new()
        .has_headers(true)
        .flexible(false)
        .from_reader(reader);
    let header = reader
        .headers()
        .with_context(|| format!("reading the header of {origin}"))?
        .clone();
    let found: Vec<&str> = header.iter().collect();
    if found != HEADER {
        bail!(
            "{origin} has another header than this build reads: {} columns, the first that differs is {:?}",
            found.len(),
            found
                .iter()
                .zip(HEADER.iter())
                .find(|(found, expected)| found != expected)
                .map(|(found, _)| *found)
                .unwrap_or("(a column was added or removed at the end)")
        );
    }
    let mut at = [0usize; 84];
    for (index, slot) in at.iter_mut().enumerate() {
        *slot = index;
    }
    let columns = Columns { at };
    let mut counts = EpaCounts::default();
    for (index, record) in reader.records().enumerate() {
        let line = index as u64 + 2;
        let record = record.with_context(|| format!("{origin}, line {line}"))?;
        counts.rows += 1;
        row_fn(row(&columns, &record, line).with_context(|| origin.to_owned())?)?;
    }
    Ok(counts)
}

/// Reads `vehicles.csv.zip` in a fetched `epa-vehicles` directory.
pub fn parse(
    directory: &Path,
    row_fn: &mut dyn FnMut(EpaVehicleRow) -> Result<()>,
) -> Result<EpaCounts> {
    let path = directory.join(FILE);
    let file = File::open(&path).with_context(|| {
        format!(
            "opening {}; run open-data fetch epa-vehicles",
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn numbers_are_read_as_the_file_writes_them() {
        assert_eq!(whole("19"), Some(19));
        assert_eq!(whole("19.0"), Some(19));
        assert_eq!(whole(" 2018 "), Some(2018));
        assert_eq!(whole("19.5"), None);
        assert_eq!(whole("NA"), None);
        assert_eq!(whole(""), None);
        assert_eq!(positive("0"), None);
        assert_eq!(positive("-1"), None);
        assert_eq!(positive("310"), Some(310));
        assert_eq!(amount("0.0"), None);
        assert_eq!(amount("8.5"), Some(8.5));
        assert_eq!(amount(""), None);
        assert_eq!(text("  "), None);
        assert_eq!(text(" SIDI ").as_deref(), Some("SIDI"));
    }

    #[test]
    fn the_header_has_84_columns_and_no_name_twice() {
        let mut names = HEADER.to_vec();
        names.sort_unstable();
        names.dedup();
        assert_eq!(names.len(), 84);
    }

    #[test]
    fn another_header_is_refused() {
        let mut header = HEADER.join(",");
        header = header.replace("baseModel", "base_model");
        let error = parse_reader(header.as_bytes(), "x.csv", &mut |_| Ok(())).unwrap_err();
        let message = format!("{error:#}");
        assert!(message.contains("another header"), "{message}");
        assert!(message.contains("base_model"), "{message}");
        let short = HEADER[..83].join(",");
        let error = parse_reader(short.as_bytes(), "x.csv", &mut |_| Ok(())).unwrap_err();
        assert!(format!("{error:#}").contains("83 columns"), "{error:#}");
    }
}
