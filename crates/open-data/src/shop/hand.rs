//! The facts collected by hand: `data/fluids/*.yaml` and
//! `data/tires/*.yaml` into the tables of `wenmar_shopdata::schema::FLUIDS`
//! and `schema::TIRES`.
//!
//! Nothing is fetched. A person read every figure from the maker's own
//! document; the build checks the shape of each record and that it names
//! a vehicle and an engine the catalog has, and works out the second unit
//! of each figure. It cannot check that a figure is true.

use std::collections::HashSet;
use std::path::Path;

use anyhow::{Context, Result, bail};
use rusqlite::{Connection, OptionalExtension, params};
use serde::Deserialize;
use wenmar_shopdata::hand::FLUID_KINDS;
use wenmar_shopdata::{Dataset, DatasetEntry, licence, schema};
use wenmar_vehicles::text::{is_slug, slug, squeeze};

use crate::shop::build::ShapeContext;

/// The longest a line of text in a record may be.
pub const LONGEST: usize = 200;
/// The most model years one record may cover.
pub const MOST_YEARS: u16 = 30;

/// A unit a figure may be written in: its name in the files, whether it
/// is metric, and how many of the base unit of its side it is.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Unit {
    pub id: &'static str,
    pub metric: bool,
    pub scale: i64,
}

/// A kind of figure: the units it may be written in, the exact size of
/// the US base unit in the metric base unit as a fraction, and the
/// decimal places the worked-out figure is rounded to on each side.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Quantity {
    pub name: &'static str,
    pub units: &'static [Unit],
    pub metric_per_us: (i128, i128),
    pub metric_decimals: u32,
    pub us_decimals: u32,
}

const fn unit(id: &'static str, metric: bool, scale: i64) -> Unit {
    Unit { id, metric, scale }
}

/// Litres and US liquid quarts. A quart is 0.946352946 L exactly.
pub const CAPACITY: Quantity = Quantity {
    name: "capacity",
    units: &[unit("l", true, 1), unit("qt", false, 1)],
    metric_per_us: (946_352_946, 1_000_000_000),
    metric_decimals: 1,
    us_decimals: 1,
};

/// Grams and ounces. An ounce is 28.349523125 g exactly; a pound is 16.
pub const CHARGE: Quantity = Quantity {
    name: "charge",
    units: &[
        unit("g", true, 1),
        unit("kg", true, 1000),
        unit("oz", false, 1),
        unit("lb", false, 16),
    ],
    metric_per_us: (28_349_523_125, 1_000_000_000),
    metric_decimals: 0,
    us_decimals: 1,
};

/// Kilopascals and pounds per square inch. A pound-force is
/// 4.4482216152605 N and a square inch 645.16 mm², both exactly.
pub const PRESSURE: Quantity = Quantity {
    name: "pressure",
    units: &[
        unit("kpa", true, 1),
        unit("bar", true, 100),
        unit("psi", false, 1),
    ],
    metric_per_us: (44_482_216_152_605, 6_451_600_000_000),
    metric_decimals: 0,
    us_decimals: 0,
};

/// Newton metres and pound-feet. A pound-foot is 4.4482216152605 N times
/// 0.3048 m, exactly 1.3558179483314004 N·m.
pub const TORQUE: Quantity = Quantity {
    name: "lug_torque",
    units: &[unit("nm", true, 1), unit("lbft", false, 1)],
    metric_per_us: (13_558_179_483_314_004, 10_000_000_000_000_000),
    metric_decimals: 0,
    us_decimals: 0,
};

/// A figure in both units, each in thousandths of its base unit.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Both {
    pub metric: i64,
    pub us: i64,
}

impl Both {
    pub fn metric_real(self) -> f64 {
        self.metric as f64 / 1000.0
    }

    pub fn us_real(self) -> f64 {
        self.us as f64 / 1000.0
    }
}

/// A positive figure as written, in thousandths: digits, then optionally
/// a point and one to three more digits. `None` for anything else, zero
/// among it.
pub fn thousandths(text: &str) -> Option<i64> {
    let (whole, fraction) = text.split_once('.').unwrap_or((text, ""));
    let digits = |part: &str| part.bytes().all(|byte| byte.is_ascii_digit());
    if whole.is_empty() || whole.len() > 9 || !digits(whole) || !digits(fraction) {
        return None;
    }
    if fraction.len() > 3 || (text.contains('.') && fraction.is_empty()) {
        return None;
    }
    let mut value: i64 = whole.parse().ok()?;
    value *= 1000;
    if !fraction.is_empty() {
        let padded = format!("{fraction:0<3}");
        value += padded.parse::<i64>().ok()?;
    }
    (value > 0).then_some(value)
}

/// `value` (in thousandths) times `numerator` over `denominator`, rounded
/// half up to `decimals` places, in thousandths again. Whole-number
/// arithmetic throughout.
pub fn scaled(value: i64, numerator: i128, denominator: i128, decimals: u32) -> i64 {
    // One step of the result, in thousandths: 100 for one decimal place.
    let step = 10_i128.pow(3 - decimals.min(3));
    let top = i128::from(value) * numerator;
    let bottom = denominator * step;
    let steps = (2 * top + bottom) / (2 * bottom);
    (steps * step) as i64
}

/// The figure `text` written in `unit`, in both units. The side the
/// source printed is kept as written; the other is worked out from it
/// and rounded.
pub fn both(quantity: &Quantity, text: &str, unit: &str) -> Result<Both> {
    let Some(found) = quantity.units.iter().find(|known| known.id == unit) else {
        let known: Vec<&str> = quantity.units.iter().map(|known| known.id).collect();
        bail!(
            "{unit:?} is not a unit of {}; use one of {}",
            quantity.name,
            known.join(", ")
        );
    };
    let Some(written) = thousandths(text) else {
        bail!(
            "{} {text:?} is not a figure like \"5.7\": digits, and at most three after the point, in double quotes",
            quantity.name
        );
    };
    let written = written * found.scale;
    let (numerator, denominator) = quantity.metric_per_us;
    Ok(if found.metric {
        Both {
            metric: written,
            us: scaled(written, denominator, numerator, quantity.us_decimals),
        }
    } else {
        Both {
            metric: scaled(written, numerator, denominator, quantity.metric_decimals),
            us: written,
        }
    })
}

/// A figure stored as a whole number: thousandths that must be whole.
fn whole(thousandths: i64, what: &str) -> Result<i64> {
    if thousandths % 1000 != 0 {
        bail!("{what} must be a whole number");
    }
    Ok(thousandths / 1000)
}

/// Whether `text` is a day written `2026-10-12`.
pub fn is_day(text: &str) -> bool {
    let bytes = text.as_bytes();
    let shaped = bytes.len() == 10
        && bytes.iter().enumerate().all(|(index, byte)| match index {
            4 | 7 => *byte == b'-',
            _ => byte.is_ascii_digit(),
        });
    if !shaped {
        return false;
    }
    let (month, day) = (&text[5..7], &text[8..10]);
    ("01"..="12").contains(&month) && ("01"..="31").contains(&day)
}

/// Whether `text` could be a GitHub user name.
pub fn is_reviewer(text: &str) -> bool {
    !text.is_empty()
        && text.len() <= 39
        && !text.starts_with('-')
        && !text.ends_with('-')
        && text
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || byte == b'-')
}

/// What every record carries, whatever it is about.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Provenance {
    pub year_from: u16,
    pub year_to: u16,
    pub market: Option<String>,
    pub source: String,
    pub checked: String,
    pub reviewer: String,
}

/// One checked record of a fluids file.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FluidRecord {
    pub kind: String,
    /// The engine's id form, as in a vehicle id: `2-7l-v6`.
    pub engine: Option<String>,
    pub applies_to: Option<String>,
    pub grade: Option<String>,
    pub specification: Option<String>,
    pub capacity: Option<Both>,
    pub capacity_unit: Option<String>,
    pub with_filter: Option<bool>,
    pub charge: Option<Both>,
    pub charge_unit: Option<String>,
    pub provenance: Provenance,
}

/// One checked record of a tires file.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TireRecord {
    pub size: String,
    pub size_rear: Option<String>,
    pub trims: Vec<String>,
    pub pressure_front: Option<Both>,
    pub pressure_rear: Option<Both>,
    pub pressure_unit: Option<String>,
    pub lug_torque: Option<Both>,
    pub lug_torque_unit: Option<String>,
    pub bolt_pattern: Option<String>,
    pub provenance: Provenance,
}

/// One file: the records of one make and model.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HandFile<R> {
    /// The file's name without `.yaml`: `ford_f-150`.
    pub name: String,
    /// The catalog's id form of the make and of the model.
    pub make: String,
    pub model: String,
    pub records: Vec<R>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawFluids {
    make: String,
    model: String,
    #[serde(default)]
    fluids: Vec<RawFluid>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawFluid {
    year_from: u16,
    year_to: Option<u16>,
    kind: String,
    engine: Option<String>,
    applies_to: Option<String>,
    grade: Option<String>,
    specification: Option<String>,
    capacity: Option<String>,
    capacity_unit: Option<String>,
    with_filter: Option<bool>,
    charge: Option<String>,
    charge_unit: Option<String>,
    market: Option<String>,
    source: String,
    checked: String,
    reviewer: String,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawTires {
    make: String,
    model: String,
    #[serde(default)]
    fitments: Vec<RawTire>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawTire {
    year_from: u16,
    year_to: Option<u16>,
    size: String,
    size_rear: Option<String>,
    #[serde(default)]
    trims: Vec<String>,
    pressure_front: Option<String>,
    pressure_rear: Option<String>,
    pressure_unit: Option<String>,
    lug_torque: Option<String>,
    lug_torque_unit: Option<String>,
    bolt_pattern: Option<String>,
    market: Option<String>,
    source: String,
    checked: String,
    reviewer: String,
}

/// One line of text, tidied. Refuses an empty one and one too long.
fn line(text: &str, field: &str) -> Result<String> {
    let tidy = squeeze(text);
    if tidy.is_empty() {
        bail!("{field} is empty");
    }
    if tidy.chars().count() > LONGEST || tidy.chars().any(char::is_control) {
        bail!("{field} must be one line of at most {LONGEST} characters");
    }
    Ok(tidy)
}

fn optional_line(text: Option<&str>, field: &str) -> Result<Option<String>> {
    text.map(|text| line(text, field)).transpose()
}

fn provenance(
    year_from: u16,
    year_to: Option<u16>,
    market: Option<&str>,
    source: &str,
    checked: &str,
    reviewer: &str,
) -> Result<Provenance> {
    let year_to = year_to.unwrap_or(year_from);
    if !(1900..=2100).contains(&year_from) || year_to < year_from {
        bail!("year_from {year_from} and year_to {year_to} are not a range of model years");
    }
    if year_to - year_from >= MOST_YEARS {
        bail!("a record covers at most {MOST_YEARS} model years");
    }
    let market = optional_line(market, "market")?;
    if let Some(market) = &market
        && !matches!(market.as_str(), "us" | "ca")
    {
        bail!("market {market:?} must be us or ca, or left out");
    }
    let checked = checked.trim();
    if !is_day(checked) {
        bail!("checked {checked:?} is not a day like 2026-10-12");
    }
    let reviewer = reviewer.trim();
    if !is_reviewer(reviewer) {
        bail!("reviewer {reviewer:?} is not a GitHub user name");
    }
    Ok(Provenance {
        year_from,
        year_to,
        market,
        source: line(source, "source")?,
        checked: checked.to_owned(),
        reviewer: reviewer.to_owned(),
    })
}

/// A figure and its unit: both, or neither.
fn figure(
    quantity: &Quantity,
    text: Option<&str>,
    unit: Option<&str>,
) -> Result<(Option<Both>, Option<String>)> {
    match (text, unit) {
        (None, None) => Ok((None, None)),
        (Some(text), Some(unit)) => {
            let unit = unit.trim();
            Ok((
                Some(both(quantity, text.trim(), unit)?),
                Some(unit.to_owned()),
            ))
        }
        _ => bail!(
            "{name} and {name}_unit go together: give both or neither",
            name = quantity.name
        ),
    }
}

fn head(name: &str, make: &str, model: &str) -> Result<()> {
    if !is_slug(make) || !is_slug(model) {
        bail!("make and model must be the catalog's id forms, such as ford and f-150");
    }
    if name != format!("{make}_{model}") {
        bail!("the file must be named {make}_{model}.yaml after its make and model");
    }
    Ok(())
}

fn fluid(raw: &RawFluid) -> Result<FluidRecord> {
    let kind = raw.kind.trim();
    if !FLUID_KINDS.contains(&kind) {
        bail!("kind {kind:?} is not one of {}", FLUID_KINDS.join(", "));
    }
    let engine = optional_line(raw.engine.as_deref(), "engine")?;
    if let Some(engine) = &engine
        && !is_slug(engine)
    {
        bail!("engine {engine:?} must be an engine's id form, such as 2-7l-v6");
    }
    let oil = kind == "engine_oil";
    if oil && engine.is_none() {
        bail!("an engine_oil record must name its engine");
    }
    let grade = optional_line(raw.grade.as_deref(), "grade")?;
    let specification = optional_line(raw.specification.as_deref(), "specification")?;
    let (capacity, capacity_unit) = figure(
        &CAPACITY,
        raw.capacity.as_deref(),
        raw.capacity_unit.as_deref(),
    )?;
    let (charge, charge_unit) = figure(&CHARGE, raw.charge.as_deref(), raw.charge_unit.as_deref())?;
    if !oil && (grade.is_some() || capacity.is_some() || raw.with_filter.is_some()) {
        bail!("grade, capacity and with_filter belong to an engine_oil record only");
    }
    if kind != "refrigerant" && charge.is_some() {
        bail!("charge belongs to a refrigerant record only");
    }
    if capacity.is_some() != raw.with_filter.is_some() {
        bail!("capacity and with_filter go together: give both or neither");
    }
    if oil && grade.is_none() && specification.is_none() && capacity.is_none() {
        bail!("an engine_oil record needs a grade, a specification or a capacity");
    }
    if !oil && specification.is_none() {
        bail!("a {kind} record needs a specification");
    }
    if let Some(charge) = charge {
        whole(charge.metric, "a charge in grams")?;
    }
    Ok(FluidRecord {
        kind: kind.to_owned(),
        engine,
        applies_to: optional_line(raw.applies_to.as_deref(), "applies_to")?,
        grade,
        specification,
        capacity,
        capacity_unit,
        with_filter: raw.with_filter,
        charge,
        charge_unit,
        provenance: provenance(
            raw.year_from,
            raw.year_to,
            raw.market.as_deref(),
            &raw.source,
            &raw.checked,
            &raw.reviewer,
        )?,
    })
}

fn tire(raw: &RawTire) -> Result<TireRecord> {
    let size = line(&raw.size, "size")?;
    let size_rear = optional_line(raw.size_rear.as_deref(), "size_rear")?;
    if size_rear.as_deref() == Some(size.as_str()) {
        bail!("size_rear is for a rear size that differs from size; leave it out");
    }
    let mut trims = Vec::with_capacity(raw.trims.len());
    for trim in &raw.trims {
        let trim = line(trim, "a trim")?;
        if trims.contains(&trim) {
            bail!("trim {trim:?} is listed twice");
        }
        trims.push(trim);
    }
    let unit = raw.pressure_unit.as_deref();
    let (pressure_front, pressure_unit) = figure(&PRESSURE, raw.pressure_front.as_deref(), unit)?;
    let (pressure_rear, _) = figure(&PRESSURE, raw.pressure_rear.as_deref(), unit)?;
    let (lug_torque, lug_torque_unit) = figure(
        &TORQUE,
        raw.lug_torque.as_deref(),
        raw.lug_torque_unit.as_deref(),
    )?;
    for (found, what) in [
        (pressure_front, "a pressure in kPa and in psi"),
        (pressure_rear, "a pressure in kPa and in psi"),
        (lug_torque, "a torque in N·m and in lb·ft"),
    ] {
        if let Some(found) = found {
            whole(found.metric, what)?;
            whole(found.us, what)?;
        }
    }
    Ok(TireRecord {
        size,
        size_rear,
        trims,
        pressure_front,
        pressure_rear,
        pressure_unit,
        lug_torque,
        lug_torque_unit,
        bolt_pattern: optional_line(raw.bolt_pattern.as_deref(), "bolt_pattern")?,
        provenance: provenance(
            raw.year_from,
            raw.year_to,
            raw.market.as_deref(),
            &raw.source,
            &raw.checked,
            &raw.reviewer,
        )?,
    })
}

/// Reads the text of one fluids file and checks every record's shape.
/// `name` is the file's name without `.yaml`.
pub fn parse_fluids(name: &str, yaml: &str) -> Result<HandFile<FluidRecord>> {
    let raw: RawFluids =
        serde_saphyr::from_str(yaml).with_context(|| format!("fluids/{name}.yaml"))?;
    let (make, model) = (raw.make.trim(), raw.model.trim());
    head(name, make, model).with_context(|| format!("fluids/{name}.yaml"))?;
    let mut records = Vec::with_capacity(raw.fluids.len());
    for (index, record) in raw.fluids.iter().enumerate() {
        records.push(
            fluid(record).with_context(|| format!("fluids/{name}.yaml, record {}", index + 1))?,
        );
    }
    Ok(HandFile {
        name: name.to_owned(),
        make: make.to_owned(),
        model: model.to_owned(),
        records,
    })
}

/// Reads the text of one tires file and checks every record's shape.
pub fn parse_tires(name: &str, yaml: &str) -> Result<HandFile<TireRecord>> {
    let raw: RawTires =
        serde_saphyr::from_str(yaml).with_context(|| format!("tires/{name}.yaml"))?;
    let (make, model) = (raw.make.trim(), raw.model.trim());
    head(name, make, model).with_context(|| format!("tires/{name}.yaml"))?;
    let mut records = Vec::with_capacity(raw.fitments.len());
    for (index, record) in raw.fitments.iter().enumerate() {
        records.push(
            tire(record).with_context(|| format!("tires/{name}.yaml, record {}", index + 1))?,
        );
    }
    Ok(HandFile {
        name: name.to_owned(),
        make: make.to_owned(),
        model: model.to_owned(),
        records,
    })
}

/// The `.yaml` files of `directory`, by name, as (name without `.yaml`,
/// text). A file whose name starts with `_` is an example and is left
/// out. A directory that does not exist has none.
pub fn files(directory: &Path) -> Result<Vec<(String, String)>> {
    if !directory.is_dir() {
        return Ok(Vec::new());
    }
    let mut found = Vec::new();
    for entry in
        std::fs::read_dir(directory).with_context(|| format!("reading {}", directory.display()))?
    {
        let path = entry?.path();
        let Some(name) = path.file_name().and_then(|name| name.to_str()) else {
            continue;
        };
        let Some(stem) = name.strip_suffix(".yaml") else {
            continue;
        };
        if stem.starts_with('_') {
            continue;
        }
        let text = std::fs::read_to_string(&path)
            .with_context(|| format!("reading {}", path.display()))?;
        found.push((stem.to_owned(), text));
    }
    found.sort();
    Ok(found)
}

/// Every fluids file under `curated/fluids`, checked.
pub fn load_fluids(curated: &Path) -> Result<Vec<HandFile<FluidRecord>>> {
    files(&curated.join("fluids"))?
        .iter()
        .map(|(name, text)| parse_fluids(name, text))
        .collect()
}

/// Every tires file under `curated/tires`, checked.
pub fn load_tires(curated: &Path) -> Result<Vec<HandFile<TireRecord>>> {
    files(&curated.join("tires"))?
        .iter()
        .map(|(name, text)| parse_tires(name, text))
        .collect()
}

/// The catalog ids of a file's make and model.
fn catalog_model(catalog: &Connection, make: &str, model: &str) -> Result<(i64, i64)> {
    let make_id: Option<i64> = catalog
        .query_row(
            "SELECT id FROM catalog_make WHERE slug = ?1",
            [make],
            |row| row.get(0),
        )
        .optional()?;
    let Some(make_id) = make_id else {
        bail!("the catalog has no make {make:?}");
    };
    let model_id: Option<i64> = catalog
        .query_row(
            "SELECT id FROM catalog_model WHERE make_id = ?1 AND slug = ?2",
            params![make_id, model],
            |row| row.get(0),
        )
        .optional()?;
    let Some(model_id) = model_id else {
        bail!("the catalog has no model {model:?} of {make:?}");
    };
    Ok((make_id, model_id))
}

/// The catalog engines of one model year, as (id form, id). Fails when
/// the catalog has no such model year.
fn catalog_engines(
    catalog: &Connection,
    make_id: i64,
    model_id: i64,
    year: u16,
) -> Result<Vec<(String, i64)>> {
    let detail: Option<Option<i64>> = catalog
        .query_row(
            "SELECT detail_id FROM catalog_vehicle
             WHERE make_id = ?1 AND model_id = ?2 AND year = ?3",
            params![make_id, model_id, year],
            |row| row.get(0),
        )
        .optional()?;
    let Some(detail) = detail else {
        bail!("the catalog has no model year {year} of this model");
    };
    let Some(detail) = detail else {
        return Ok(Vec::new());
    };
    let mut statement = catalog
        .prepare_cached("SELECT label, id FROM catalog_engine WHERE detail_id = ?1 ORDER BY id")?;
    let rows = statement.query_map([detail], |row| {
        Ok((row.get::<_, String>(0)?, row.get::<_, i64>(1)?))
    })?;
    let mut engines = Vec::new();
    for row in rows {
        let (label, id) = row?;
        engines.push((slug(&label), id));
    }
    Ok(engines)
}

/// The id of the engine a record names, in one model year.
fn engine_id(engines: &[(String, i64)], wanted: &str, year: u16) -> Result<i64> {
    let found: Vec<i64> = engines
        .iter()
        .filter(|(id_form, _)| id_form == wanted)
        .map(|(_, id)| *id)
        .collect();
    match found.as_slice() {
        [id] => Ok(*id),
        [] => {
            let known: Vec<&str> = engines
                .iter()
                .map(|(id_form, _)| id_form.as_str())
                .collect();
            bail!(
                "the catalog has no engine {wanted:?} for model year {year}; it has: {}",
                if known.is_empty() {
                    "none".to_owned()
                } else {
                    known.join(", ")
                }
            )
        }
        _ => bail!("the catalog has more than one engine {wanted:?} for model year {year}"),
    }
}

/// A `checked` day may not be after the day of the build.
fn not_ahead(checked: &str, context: &ShapeContext<'_>) -> Result<()> {
    let today: String = context.info.built_at.chars().take(10).collect();
    if checked > today.as_str() {
        bail!("checked {checked} is after the day of the build, {today}");
    }
    Ok(())
}

fn entry(rows: u64, as_of: String) -> DatasetEntry {
    DatasetEntry {
        as_of,
        rows,
        licence: licence::MIT.to_owned(),
        attribution: String::new(),
    }
}

/// What makes a fluid record of a model year one of a kind: the year, the
/// kind, the engine, `applies_to` and the market.
type FluidKey = (u16, String, Option<i64>, Option<String>, Option<String>);
/// The same for a fitment: the year, the size, the rear size, the trims
/// and the market.
type TireKey = (u16, String, Option<String>, Vec<String>, Option<String>);

/// Writes the `fluid` table and registers `fluids`. With no record in
/// `curated/fluids` it does nothing: the file then does not hold the
/// dataset.
pub fn shape_fluids(context: &mut ShapeContext<'_>) -> Result<()> {
    let files = load_fluids(context.curated)?;
    if files.iter().all(|file| file.records.is_empty()) {
        return Ok(());
    }
    context.transaction.execute_batch(schema::FLUIDS)?;
    let (mut rows, mut as_of) = (0_u64, String::new());
    for file in &files {
        let place = format!("fluids/{}.yaml", file.name);
        let (make_id, model_id) =
            catalog_model(context.catalog, &file.make, &file.model).context(place.clone())?;
        let mut taken: HashSet<FluidKey> = HashSet::new();
        for (index, record) in file.records.iter().enumerate() {
            let place = format!("{place}, record {}", index + 1);
            let provenance = &record.provenance;
            not_ahead(&provenance.checked, context).context(place.clone())?;
            for year in provenance.year_from..=provenance.year_to {
                let engines = catalog_engines(context.catalog, make_id, model_id, year)
                    .context(place.clone())?;
                let engine = match &record.engine {
                    Some(wanted) => Some(engine_id(&engines, wanted, year).context(place.clone())?),
                    None => None,
                };
                let key = (
                    year,
                    record.kind.clone(),
                    engine,
                    record.applies_to.clone(),
                    provenance.market.clone(),
                );
                if !taken.insert(key) {
                    bail!(
                        "{place}: model year {year} already has a {} record for the same engine, applies_to and market",
                        record.kind
                    );
                }
                context.transaction.execute(
                    "INSERT INTO fluid (make_id, model_id, year, kind, engine_id, applies_to, grade,
                        specification, capacity_l, capacity_qt, capacity_unit, with_filter, charge_g,
                        charge_oz, charge_unit, market, source, checked, reviewer)
                     VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16,
                        ?17, ?18, ?19)",
                    params![
                        make_id,
                        model_id,
                        year,
                        record.kind,
                        engine,
                        record.applies_to,
                        record.grade,
                        record.specification,
                        record.capacity.map(Both::metric_real),
                        record.capacity.map(Both::us_real),
                        record.capacity_unit,
                        record.with_filter,
                        record.charge.map(|charge| charge.metric / 1000),
                        record.charge.map(Both::us_real),
                        record.charge_unit,
                        provenance.market,
                        provenance.source,
                        provenance.checked,
                        provenance.reviewer,
                    ],
                )?;
                rows += 1;
            }
            if provenance.checked > as_of {
                as_of.clone_from(&provenance.checked);
            }
        }
    }
    context.register(Dataset::Fluids, entry(rows, as_of))
}

/// Writes the `tire_fitment` table and registers `tires`. With no record
/// in `curated/tires` it does nothing.
pub fn shape_tires(context: &mut ShapeContext<'_>) -> Result<()> {
    let files = load_tires(context.curated)?;
    if files.iter().all(|file| file.records.is_empty()) {
        return Ok(());
    }
    context.transaction.execute_batch(schema::TIRES)?;
    let (mut rows, mut as_of) = (0_u64, String::new());
    for file in &files {
        let place = format!("tires/{}.yaml", file.name);
        let (make_id, model_id) =
            catalog_model(context.catalog, &file.make, &file.model).context(place.clone())?;
        let mut taken: HashSet<TireKey> = HashSet::new();
        for (index, record) in file.records.iter().enumerate() {
            let place = format!("{place}, record {}", index + 1);
            let provenance = &record.provenance;
            not_ahead(&provenance.checked, context).context(place.clone())?;
            let trims = serde_json::to_string(&record.trims)?;
            let thousand =
                |found: Option<Both>, pick: fn(Both) -> i64| found.map(|both| pick(both) / 1000);
            for year in provenance.year_from..=provenance.year_to {
                // Only to learn that the catalog has the model year.
                catalog_engines(context.catalog, make_id, model_id, year).context(place.clone())?;
                let key = (
                    year,
                    record.size.clone(),
                    record.size_rear.clone(),
                    record.trims.clone(),
                    provenance.market.clone(),
                );
                if !taken.insert(key) {
                    bail!(
                        "{place}: model year {year} already has the fitment {} for the same trims and market",
                        record.size
                    );
                }
                context.transaction.execute(
                    "INSERT INTO tire_fitment (make_id, model_id, year, size, size_rear, trims,
                        pressure_front_kpa, pressure_rear_kpa, pressure_front_psi, pressure_rear_psi,
                        pressure_unit, lug_torque_nm, lug_torque_lbft, lug_torque_unit, bolt_pattern,
                        market, source, checked, reviewer)
                     VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16,
                        ?17, ?18, ?19)",
                    params![
                        make_id,
                        model_id,
                        year,
                        record.size,
                        record.size_rear,
                        trims,
                        thousand(record.pressure_front, |both| both.metric),
                        thousand(record.pressure_rear, |both| both.metric),
                        thousand(record.pressure_front, |both| both.us),
                        thousand(record.pressure_rear, |both| both.us),
                        record.pressure_unit,
                        thousand(record.lug_torque, |both| both.metric),
                        thousand(record.lug_torque, |both| both.us),
                        record.lug_torque_unit,
                        record.bolt_pattern,
                        provenance.market,
                        provenance.source,
                        provenance.checked,
                        provenance.reviewer,
                    ],
                )?;
                rows += 1;
            }
            if provenance.checked > as_of {
                as_of.clone_from(&provenance.checked);
            }
        }
    }
    context.register(Dataset::Tires, entry(rows, as_of))
}

/// Both shape steps. `build-shop` always runs it: the records are in the
/// repository, so there is no directory to pass.
pub fn shape(context: &mut ShapeContext<'_>) -> Result<()> {
    shape_fluids(context)?;
    shape_tires(context)
}
