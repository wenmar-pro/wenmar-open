//! Rows that many VINs share, kept between decodes.
//!
//! Most of the time a decode takes goes on asking the data file for rows,
//! and VINs of one manufacturer and model year ask for the same ones: the
//! manufacturer, its schemas, the patterns of each schema, the engine
//! models and the specification sheets. [`VinCache`] keeps those rows, so
//! a VIN whose rows are held is decoded without a query.
//!
//! The patterns of a schema are read whole, not narrowed to one VIN, and
//! each is compiled once ([`pattern::Compiled`]). A decode through the cache
//! gives the same answer as one through [`fetch`](crate::vin_rows::fetch).
//!
//! The data file never changes while it is open, so nothing held goes
//! stale. What is held is bounded: when a kind of row is over its limit,
//! the rows held longest are dropped first.

use std::collections::{BTreeSet, HashMap};
use std::sync::{Arc, Mutex, PoisonError};

use wenmar_vin::{Element, EngineRow, Pattern, SpecRow, Vin, pattern};

use crate::bounded::Bounded;
use crate::source::{Source, SourceError, Value};
use crate::vin_rows::{
    self, CHUNK, Direct, Lookup, MAKES_SQL, MANUFACTURERS_SQL, ManufacturerRow, PatternRow,
    SCHEMAS_SQL, SchemaRange, VinRows,
};

/// How many pattern rows a cache holds unless told otherwise: some 40 MB,
/// a sixth of the 2026.09 data file. A row takes about 190 bytes.
pub const DEFAULT_PATTERN_ROWS: usize = 200_000;

/// Specification rows held, as a share of the pattern rows held. The
/// 2026.09 data file has one specification row for six pattern rows.
const SPEC_ROWS_PER_PATTERN_ROW: usize = 3;

/// Entries held of each small kind of row: manufacturers, schema lists,
/// make ids and engine models. A key there is a code a caller typed, so it
/// is bounded too.
const SMALL_ENTRIES: usize = 20_000;

/// A pattern row with its keys compiled, its text in one allocation. The
/// schema is the one it is held under.
struct Held {
    compiled: Option<pattern::Compiled>,
    id: i64,
    element: Element,
    /// The keys, attribute, value, change time and make, one after another.
    text: Box<str>,
    /// Where each of the first four ends in `text`.
    ends: [u32; 4],
    has_make: bool,
}

impl Held {
    fn new((pattern, make): PatternRow) -> Held {
        let mut text = String::with_capacity(
            pattern.keys.len()
                + pattern.attribute.len()
                + pattern.value.len()
                + pattern.changed_on.len()
                + make.as_ref().map_or(0, String::len),
        );
        let mut ends = [0u32; 4];
        for (end, part) in ends.iter_mut().zip([
            &pattern.keys,
            &pattern.attribute,
            &pattern.value,
            &pattern.changed_on,
        ]) {
            text.push_str(part);
            *end = u32::try_from(text.len()).unwrap_or(u32::MAX);
        }
        if let Some(make) = &make {
            text.push_str(make);
        }
        Held {
            compiled: pattern::Compiled::new(&pattern.keys),
            id: pattern.id,
            element: pattern.element,
            text: text.into_boxed_str(),
            ends,
            has_make: make.is_some(),
        }
    }

    fn part(&self, index: usize) -> &str {
        let start = if index == 0 { 0 } else { self.ends[index - 1] };
        let end = self.ends.get(index).copied().unwrap_or(u32::MAX);
        let (start, end) = (start as usize, (end as usize).min(self.text.len()));
        self.text.get(start..end).unwrap_or_default()
    }

    fn row(&self, schema_id: i64) -> PatternRow {
        let pattern = Pattern {
            id: self.id,
            schema_id,
            keys: self.part(0).to_owned(),
            element: self.element,
            attribute: self.part(1).to_owned(),
            value: self.part(2).to_owned(),
            changed_on: self.part(3).to_owned(),
        };
        let make = self.has_make.then(|| self.part(4).to_owned());
        (pattern, make)
    }
}

struct Inner {
    manufacturers: Bounded<(String, String), Arc<Vec<ManufacturerRow>>>,
    schemas: Bounded<String, Arc<Vec<SchemaRange>>>,
    makes: Bounded<String, Arc<BTreeSet<i64>>>,
    patterns: Bounded<i64, Arc<Vec<Held>>>,
    engines: Bounded<String, Arc<Vec<EngineRow>>>,
    specs: Bounded<(String, i64, u16), Arc<Vec<SpecRow>>>,
}

/// What a cache holds, for a status page or a test.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct CacheStats {
    /// Schemas whose patterns are held.
    pub schemas: usize,
    /// Pattern rows held.
    pub pattern_rows: usize,
    /// Specification rows held.
    pub spec_rows: usize,
}

/// Rows kept between decodes. One cache may be shared by any number of
/// threads, each with its own connection to the same data file.
pub struct VinCache {
    inner: Mutex<Inner>,
}

impl std::fmt::Debug for VinCache {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("VinCache")
            .field("stats", &self.stats())
            .finish()
    }
}

impl Default for VinCache {
    fn default() -> Self {
        Self::new(DEFAULT_PATTERN_ROWS)
    }
}

impl VinCache {
    /// A cache holding at most `pattern_rows` pattern rows, and specification
    /// rows in proportion. Zero holds nothing: every decode reads the data
    /// file, as [`fetch`](crate::vin_rows::fetch) does.
    pub fn new(pattern_rows: usize) -> Self {
        let small = if pattern_rows == 0 { 0 } else { SMALL_ENTRIES };
        Self {
            inner: Mutex::new(Inner {
                manufacturers: Bounded::new(small),
                schemas: Bounded::new(small),
                makes: Bounded::new(small),
                patterns: Bounded::new(pattern_rows),
                engines: Bounded::new(small),
                specs: Bounded::new(pattern_rows / SPEC_ROWS_PER_PATTERN_ROW),
            }),
        }
    }

    fn lock(&self) -> std::sync::MutexGuard<'_, Inner> {
        self.inner.lock().unwrap_or_else(PoisonError::into_inner)
    }

    pub fn stats(&self) -> CacheStats {
        let inner = self.lock();
        CacheStats {
            schemas: inner.patterns.len(),
            pattern_rows: inner.patterns.weight(),
            spec_rows: inner.specs.weight(),
        }
    }

    /// Reads what the decoder needs for `vin`, as
    /// [`fetch`](crate::vin_rows::fetch) does, from the rows held where it
    /// can.
    pub fn fetch<S: Source>(
        &self,
        source: &S,
        vin: &Vin,
        model_year: Option<u16>,
        current_year: u16,
    ) -> Result<VinRows, SourceError> {
        vin_rows::fetch_with(source, self, vin, model_year, current_year)
    }
}

/// Every pattern of the schemas, ordered by schema and then id.
const PATTERNS_SQL: &str = "
SELECT id, schema_id, keys, element_id, value, changed_on, make, attribute
FROM pattern WHERE schema_id IN";

impl<S: Source> Lookup<S> for VinCache {
    fn manufacturers(
        &self,
        source: &S,
        first: &str,
        plain: &str,
    ) -> Result<Vec<ManufacturerRow>, SourceError> {
        let key = (first.to_owned(), plain.to_owned());
        if let Some(held) = self.lock().manufacturers.get(&key) {
            return Ok(held.as_ref().clone());
        }
        let rows: Vec<ManufacturerRow> = source
            .query(MANUFACTURERS_SQL, &[first.into(), plain.into()])?
            .iter()
            .map(|row| vin_rows::manufacturer_row(row))
            .collect::<Result<_, _>>()?;
        self.lock()
            .manufacturers
            .insert(key, Arc::new(rows.clone()), 1);
        Ok(rows)
    }

    fn schemas(&self, source: &S, wmi: &str) -> Result<Vec<SchemaRange>, SourceError> {
        if let Some(held) = self.lock().schemas.get(&wmi.to_owned()) {
            return Ok(held.as_ref().clone());
        }
        let rows: Vec<SchemaRange> = source
            .query(SCHEMAS_SQL, &[wmi.into()])?
            .iter()
            .map(|row| vin_rows::schema_row(row))
            .collect::<Result<_, _>>()?;
        self.lock()
            .schemas
            .insert(wmi.to_owned(), Arc::new(rows.clone()), 1);
        Ok(rows)
    }

    fn patterns(
        &self,
        source: &S,
        schema_ids: &[i64],
        key: &str,
    ) -> Result<Vec<PatternRow>, SourceError> {
        let mut held: HashMap<i64, Arc<Vec<Held>>> = HashMap::new();
        let mut missing: Vec<i64> = Vec::new();
        {
            let inner = self.lock();
            for id in schema_ids {
                match inner.patterns.get(id) {
                    Some(rows) => {
                        held.insert(*id, rows);
                    }
                    None => missing.push(*id),
                }
            }
        }
        for chunk in missing.chunks(CHUNK) {
            let sql = format!(
                "{PATTERNS_SQL} ({}) ORDER BY schema_id, id",
                vec!["?"; chunk.len()].join(", ")
            );
            let params: Vec<Value> = chunk.iter().map(|id| Value::Integer(*id)).collect();
            let mut loaded: HashMap<i64, Vec<Held>> =
                chunk.iter().map(|id| (*id, Vec::new())).collect();
            for row in source.query(&sql, &params)? {
                let row = vin_rows::pattern_row(&row)?;
                if let Some(list) = loaded.get_mut(&row.0.schema_id) {
                    list.push(Held::new(row));
                }
            }
            let mut inner = self.lock();
            for (id, mut rows) in loaded {
                rows.shrink_to_fit();
                let rows = Arc::new(rows);
                // A schema with no patterns still counts, so it is held.
                let weight = rows.len().max(1);
                inner.patterns.insert(id, Arc::clone(&rows), weight);
                held.insert(id, rows);
            }
        }

        let mut found = Vec::new();
        for id in schema_ids {
            let Some(rows) = held.get(id) else {
                continue;
            };
            for row in rows.iter() {
                if row
                    .compiled
                    .as_ref()
                    .is_some_and(|compiled| compiled.matches(key))
                {
                    found.push(row.row(*id));
                }
            }
        }
        Ok(found)
    }

    fn engines(
        &self,
        source: &S,
        names: &[String],
    ) -> Result<Vec<(String, Vec<EngineRow>)>, SourceError> {
        let mut found: Vec<(String, Vec<EngineRow>)> = Vec::new();
        let mut missing: Vec<String> = Vec::new();
        {
            let inner = self.lock();
            for name in names {
                match inner.engines.get(name) {
                    Some(rows) => found.push((name.clone(), rows.as_ref().clone())),
                    None => missing.push(name.clone()),
                }
            }
        }
        if missing.is_empty() {
            return Ok(found);
        }
        let loaded = Lookup::<S>::engines(&Direct, source, &missing)?;
        let mut inner = self.lock();
        for name in &missing {
            let rows = loaded
                .iter()
                .find(|(known, _)| known == name)
                .map(|(_, rows)| rows.clone())
                .unwrap_or_default();
            inner.engines.insert(name.clone(), Arc::new(rows), 1);
        }
        drop(inner);
        found.extend(loaded);
        Ok(found)
    }

    fn makes(&self, source: &S, wmi: &str) -> Result<BTreeSet<i64>, SourceError> {
        if let Some(held) = self.lock().makes.get(&wmi.to_owned()) {
            return Ok(held.as_ref().clone());
        }
        let mut found = BTreeSet::new();
        for row in source.query(MAKES_SQL, &[wmi.into()])? {
            if let Some(id) = row.first().and_then(Value::integer) {
                found.insert(id);
            }
        }
        self.lock()
            .makes
            .insert(wmi.to_owned(), Arc::new(found.clone()), 1);
        Ok(found)
    }

    fn specs(
        &self,
        source: &S,
        wmi: &str,
        model_id: i64,
        years: &BTreeSet<u16>,
        make_ids: &BTreeSet<i64>,
        type_id: Option<i64>,
    ) -> Result<HashMap<(i64, u16), Vec<SpecRow>>, SourceError> {
        let mut found: HashMap<(i64, u16), Vec<SpecRow>> = HashMap::new();
        let mut missing: BTreeSet<u16> = BTreeSet::new();
        {
            let inner = self.lock();
            for year in years {
                match inner.specs.get(&(wmi.to_owned(), model_id, *year)) {
                    Some(rows) => {
                        found.insert((model_id, *year), rows.as_ref().clone());
                    }
                    None => {
                        missing.insert(*year);
                    }
                }
            }
        }
        if missing.is_empty() {
            return Ok(found);
        }
        let loaded = vin_rows::specs(source, model_id, &missing, make_ids, type_id)?;
        let mut inner = self.lock();
        for ((model, year), rows) in loaded {
            let weight = rows.len().max(1);
            inner.specs.insert(
                (wmi.to_owned(), model, year),
                Arc::new(rows.clone()),
                weight,
            );
            found.insert((model, year), rows);
        }
        Ok(found)
    }
}
