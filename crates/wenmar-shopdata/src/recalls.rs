//! Recall campaigns: NHTSA's for the United States and Transport Canada's
//! for Canada.
//!
//! A campaign lists vehicles by make, model and model year. Nothing here
//! says a recall applies to one vehicle: only the manufacturer can say that
//! of a VIN. Every answer is "may apply to this model year".

use std::fmt;

use serde::Serialize;
use wenmar_vehicles::{Source, Value};

use crate::dataset::Dataset;
use crate::error::ShopError;
use crate::shop::ShopData;

/// Most campaigns one page of a list holds, whatever `limit` says.
pub const MOST_RECALLS: usize = 100;

/// Longest free text matched. The rest is ignored.
pub const LONGEST_RECALL_QUERY: usize = 100;

/// Who published a campaign.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum RecallSource {
    /// NHTSA, United States.
    Nhtsa,
    /// Transport Canada.
    Tc,
}

impl RecallSource {
    pub const ALL: [RecallSource; 2] = [RecallSource::Nhtsa, RecallSource::Tc];

    /// The `source` column and the word in an address: `nhtsa` or `tc`.
    pub fn id(self) -> &'static str {
        match self {
            RecallSource::Nhtsa => "nhtsa",
            RecallSource::Tc => "tc",
        }
    }

    /// The country code a caller filters by: `us` or `ca`.
    pub fn country(self) -> &'static str {
        match self {
            RecallSource::Nhtsa => "us",
            RecallSource::Tc => "ca",
        }
    }

    /// The dataset that holds this source's campaigns.
    pub fn dataset(self) -> Dataset {
        match self {
            RecallSource::Nhtsa => Dataset::RecallsUs,
            RecallSource::Tc => Dataset::RecallsCa,
        }
    }

    /// Reads `nhtsa` or `tc`.
    pub fn parse(id: &str) -> Option<RecallSource> {
        RecallSource::ALL
            .into_iter()
            .find(|source| source.id() == id)
    }

    /// Reads `us` or `ca`.
    pub fn from_country(country: &str) -> Option<RecallSource> {
        RecallSource::ALL
            .into_iter()
            .find(|source| source.country() == country)
    }
}

impl fmt::Display for RecallSource {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.id())
    }
}

/// A campaign as a list shows it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct RecallSummary {
    /// `nhtsa` or `tc`.
    pub source: String,
    /// `us` or `ca`.
    pub country: String,
    /// The publisher's campaign number, such as `19V888000` or `2019631`.
    pub campaign: String,
    /// The day the campaign was reported, as `2019-12-12`.
    pub date: Option<String>,
    pub manufacturer: Option<String>,
    /// The component as the source words it.
    pub component: Option<String>,
    /// What is wrong, in the source's words.
    pub summary: Option<String>,
    /// The manufacturer's own campaign number.
    pub mfr_campaign: Option<String>,
    /// Units potentially affected.
    pub units: Option<u32>,
    /// NHTSA's advisories. `None` for a Canadian campaign: Transport
    /// Canada's file has no such field.
    pub do_not_drive: Option<bool>,
    pub park_outside: Option<bool>,
    /// Transport Canada's notification type, such as `Safety Mfr`.
    pub notification: Option<String>,
}

/// A campaign that may apply to one model year.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct VehicleRecall {
    #[serde(flatten)]
    pub recall: RecallSummary,
    /// How the campaign's vehicle text reached this model: `exact`, `base`
    /// (the source named a series or trim of it) or `curated`.
    #[serde(rename = "match")]
    pub matched: String,
    /// The model names the campaign lists that reached this model year.
    pub models: Vec<String>,
    /// The first and last build date the campaign gives for them.
    pub built_from: Option<String>,
    pub built_to: Option<String>,
}

/// One vehicle a campaign lists.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct RecallVehicle {
    pub year: Option<u16>,
    /// `catalog_make.id`, when the make is in the catalog.
    pub make_id: Option<i64>,
    /// `catalog_model.id`, when the model matched.
    pub model_id: Option<i64>,
    /// The make and model as the source words them.
    pub make_text: String,
    pub model_text: String,
    /// `exact`, `base` or `curated`; `None` when the model did not match.
    #[serde(rename = "match")]
    pub matched: Option<String>,
    pub built_from: Option<String>,
    pub built_to: Option<String>,
}

/// One campaign with everything the file holds of it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct RecallDetail {
    #[serde(flatten)]
    pub recall: RecallSummary,
    /// The day owners were notified.
    pub owner_notified: Option<String>,
    pub consequence: Option<String>,
    pub remedy: Option<String>,
    pub notes: Option<String>,
    /// The manufacturer's part numbers, separated by commas.
    pub part_number: Option<String>,
    /// Transport Canada's French text.
    pub component_fr: Option<String>,
    pub notification_fr: Option<String>,
    pub summary_fr: Option<String>,
    /// Every vehicle the campaign lists, by make, model and year.
    pub vehicles: Vec<RecallVehicle>,
}

/// How many campaigns list one model year, for each country. `None` for a
/// country the file holds no recalls for; `Some(0)` when it holds them and
/// none lists the year.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct RecallYearCount {
    pub year: u16,
    pub us: Option<u32>,
    pub ca: Option<u32>,
}

/// Where a list left off. Its text form is for handing back unchanged.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RecallCursor {
    /// The report date of the last campaign given; empty when it had none.
    date: String,
    id: i64,
}

impl RecallCursor {
    /// Reads a cursor a list gave out. `None` for anything else.
    pub fn parse(text: &str) -> Option<RecallCursor> {
        let (date, id) = text.rsplit_once('_')?;
        let dated = date.len() == 10
            && date.bytes().enumerate().all(|(index, byte)| match index {
                4 | 7 => byte == b'-',
                _ => byte.is_ascii_digit(),
            });
        if !(date.is_empty() || dated) || id.is_empty() || id.len() > 18 {
            return None;
        }
        if !id.bytes().all(|byte| byte.is_ascii_digit()) {
            return None;
        }
        Some(RecallCursor {
            date: date.to_owned(),
            id: id.parse().ok()?,
        })
    }
}

impl fmt::Display for RecallCursor {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{}_{}", self.date, self.id)
    }
}

/// What a list of campaigns is narrowed by. Everything left `None` is not
/// narrowed by.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct RecallQuery {
    /// Only this publisher's campaigns. `None`: both, where the file holds
    /// both.
    pub source: Option<RecallSource>,
    /// Only campaigns that list a vehicle of this make, model or year.
    pub make_id: Option<i64>,
    pub model_id: Option<i64>,
    pub year: Option<u16>,
    /// Only campaigns whose component text contains this.
    pub component: Option<String>,
    /// Only campaigns with this text in them. See [`RECALL_LIST`].
    pub q: Option<String>,
    /// Only campaigns reported on or after this day, as `2026-09-01`.
    pub since: Option<String>,
    /// At most this many, up to [`MOST_RECALLS`].
    pub limit: usize,
    /// Where the page before this one left off.
    pub cursor: Option<RecallCursor>,
}

/// One page of a list, newest first.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RecallPage {
    pub recalls: Vec<RecallSummary>,
    /// The cursor for the next page. `None` when this is the last.
    pub next_cursor: Option<RecallCursor>,
}

/// Campaigns that list one model year. `?1` make id, `?2` model id, `?3`
/// year, `?4` and `?5` the sources.
pub const RECALLS_FOR_VEHICLE: &str = "
SELECT r.id, r.source, r.campaign, r.report_date, r.manufacturer, r.component, COALESCE(r.summary_en, r.notes_en), r.mfr_campaign, r.units, r.do_not_drive, r.park_outside, r.notification,
       v.\"match\", v.model_text, v.built_from, v.built_to
FROM recall_vehicle v
JOIN recall r ON r.id = v.recall_id
WHERE v.make_id = ?1 AND v.model_id = ?2 AND v.year = ?3 AND r.source IN (?4, ?5)
ORDER BY r.report_date DESC, r.id DESC, v.model_text";

/// One campaign. `?1` source, `?2` campaign.
pub const RECALL: &str = "
SELECT r.id, r.source, r.campaign, r.report_date, r.manufacturer, r.component, COALESCE(r.summary_en, r.notes_en), r.mfr_campaign, r.units, r.do_not_drive, r.park_outside, r.notification,
       r.owner_notified, r.consequence_en, r.remedy_en, CASE WHEN r.summary_en IS NULL THEN NULL ELSE r.notes_en END, r.part_number, r.component_fr, r.notification_fr, r.summary_fr
FROM recall r
WHERE r.source = ?1 AND r.campaign = ?2";

/// The vehicles of one campaign. `?1` the campaign's row id.
pub const RECALL_VEHICLES: &str = "
SELECT year, make_id, model_id, make_text, model_text, \"match\", built_from, built_to
FROM recall_vehicle
WHERE recall_id = ?1
ORDER BY make_text, model_text, year, model_id";

/// A page of campaigns, newest first. `?1` and `?2` the sources; `?3` the
/// component pattern; `?4` the free-text pattern; `?5` the earliest report
/// date; `?6`, `?7`, `?8` make id, model id and year; `?9` and `?10` the
/// cursor's date and id; `?11` the limit. A `NULL` narrows nothing.
///
/// Free text is matched with `LIKE`, so letters match in either case and
/// the text may be anywhere in: the campaign number, the manufacturer's
/// campaign number, the manufacturer, the component, the summary, the
/// consequence, the remedy, the notes, and the make and model text of the
/// vehicles listed. It is a scan, not an index.
///
/// The `+` before `r.source` keeps SQLite from choosing the index on
/// `(source, campaign)`: the list is then read in the order of the index on
/// `report_date`, newest first, and stops at the limit without sorting.
pub const RECALL_LIST: &str = "
SELECT r.id, r.source, r.campaign, r.report_date, r.manufacturer, r.component, COALESCE(r.summary_en, r.notes_en), r.mfr_campaign, r.units, r.do_not_drive, r.park_outside, r.notification
FROM recall r
WHERE +r.source IN (?1, ?2)
  AND (?3 IS NULL OR r.component LIKE ?3 ESCAPE '\\')
  AND (?4 IS NULL
       OR r.campaign LIKE ?4 ESCAPE '\\'
       OR r.mfr_campaign LIKE ?4 ESCAPE '\\'
       OR r.manufacturer LIKE ?4 ESCAPE '\\'
       OR r.component LIKE ?4 ESCAPE '\\'
       OR r.summary_en LIKE ?4 ESCAPE '\\'
       OR r.consequence_en LIKE ?4 ESCAPE '\\'
       OR r.remedy_en LIKE ?4 ESCAPE '\\'
       OR r.notes_en LIKE ?4 ESCAPE '\\'
       OR EXISTS (SELECT 1 FROM recall_vehicle t
                  WHERE t.recall_id = r.id
                    AND (t.make_text LIKE ?4 ESCAPE '\\' OR t.model_text LIKE ?4 ESCAPE '\\')))
  AND (?5 IS NULL OR r.report_date >= ?5)
  AND ((?6 IS NULL AND ?7 IS NULL AND ?8 IS NULL)
       OR EXISTS (SELECT 1 FROM recall_vehicle v
                  WHERE v.recall_id = r.id
                    AND (?6 IS NULL OR v.make_id = ?6)
                    AND (?7 IS NULL OR v.model_id = ?7)
                    AND (?8 IS NULL OR v.year = ?8)))
  AND (?9 IS NULL
       OR r.report_date < ?9
       OR (r.report_date = ?9 AND r.id < ?10))
ORDER BY r.report_date DESC, r.id DESC
LIMIT ?11";

/// The model years that have a campaign matched to a catalog model.
pub const RECALL_YEARS: &str = "
SELECT DISTINCT year FROM recall_vehicle
WHERE year IS NOT NULL AND model_id IS NOT NULL
ORDER BY year DESC";

/// The catalog models with a campaign in one model year. `?1` year.
pub const RECALL_MODELS: &str = "
SELECT DISTINCT make_id, model_id FROM recall_vehicle
WHERE year = ?1 AND make_id IS NOT NULL AND model_id IS NOT NULL
ORDER BY make_id, model_id";

/// How many campaigns list each model year of one model, by source. `?1`
/// make id, `?2` model id, `?3` and `?4` the sources. A campaign counts
/// once for a year, however many of its vehicle lines reach the model.
pub const RECALL_COUNTS_BY_YEAR: &str = "
SELECT v.year, r.source, COUNT(DISTINCT r.id)
FROM recall_vehicle v
JOIN recall r ON r.id = v.recall_id
WHERE v.make_id = ?1 AND v.model_id = ?2 AND v.year IS NOT NULL AND r.source IN (?3, ?4)
GROUP BY v.year, r.source
ORDER BY v.year DESC, r.source";

const TABLE: &str = "recall";

fn text(row: &[Value], column: usize) -> Result<String, ShopError> {
    row.get(column)
        .and_then(Value::text)
        .map(str::to_owned)
        .ok_or(ShopError::Shape(TABLE))
}

fn optional_text(row: &[Value], column: usize) -> Option<String> {
    row.get(column).and_then(Value::text).map(str::to_owned)
}

fn optional_integer(row: &[Value], column: usize) -> Option<i64> {
    row.get(column).and_then(Value::integer)
}

/// The columns [`summary`] reads, for a query over `recall r`.
pub const SUMMARY_COLUMNS: &str = "r.id, r.source, r.campaign, r.report_date, r.manufacturer, r.component, COALESCE(r.summary_en, r.notes_en), r.mfr_campaign, r.units, r.do_not_drive, r.park_outside, r.notification";

/// Reads a [`RecallSummary`] from [`SUMMARY_COLUMNS`].
pub(crate) fn summary(row: &[Value]) -> Result<RecallSummary, ShopError> {
    let source = text(row, 1)?;
    let country = RecallSource::parse(&source)
        .ok_or(ShopError::Shape(TABLE))?
        .country()
        .to_owned();
    Ok(RecallSummary {
        source,
        country,
        campaign: text(row, 2)?,
        date: optional_text(row, 3).filter(|date| !date.is_empty()),
        manufacturer: optional_text(row, 4),
        component: optional_text(row, 5),
        summary: optional_text(row, 6),
        mfr_campaign: optional_text(row, 7),
        units: optional_integer(row, 8).and_then(|units| u32::try_from(units).ok()),
        do_not_drive: optional_integer(row, 9).map(|flag| flag != 0),
        park_outside: optional_integer(row, 10).map(|flag| flag != 0),
        notification: optional_text(row, 11),
    })
}

/// The better of two ways a campaign reached a model.
fn better(left: &str, right: &str) -> String {
    let rank = |kind: &str| match kind {
        "exact" => 0,
        "base" => 1,
        _ => 2,
    };
    if rank(right) < rank(left) {
        right.to_owned()
    } else {
        left.to_owned()
    }
}

/// A `LIKE` pattern that finds `text` anywhere, with `%`, `_` and `\` in
/// it meaning themselves. `None` when the text is blank.
pub fn contains_pattern(text: &str) -> Option<String> {
    let text: String = text.trim().chars().take(LONGEST_RECALL_QUERY).collect();
    if text.is_empty() {
        return None;
    }
    let mut pattern = String::with_capacity(text.len() + 2);
    pattern.push('%');
    for character in text.chars() {
        if matches!(character, '%' | '_' | '\\') {
            pattern.push('\\');
        }
        pattern.push(character);
    }
    pattern.push('%');
    Some(pattern)
}

/// The systems a component text names. NHTSA writes a component as a path,
/// `AIR BAGS:FRONTAL:DRIVER SIDE`, and a campaign with several has them
/// separated by `; `: the system is the part before the first colon.
/// Transport Canada writes the system alone, `Airbag`.
pub fn systems(component: &str) -> Vec<String> {
    let mut found: Vec<String> = Vec::new();
    for part in component.split(';') {
        let system = part.split(':').next().unwrap_or("").trim();
        if !system.is_empty() && !found.iter().any(|known| known.eq_ignore_ascii_case(system)) {
            found.push(system.to_owned());
        }
    }
    found
}

impl<S: Source> ShopData<S> {
    /// The recall sources a reader may read: the one asked for, or every
    /// one the file holds. Fails with the dataset that is missing when the
    /// one asked for is not held, or when the file holds neither.
    pub fn require_recalls(
        &self,
        source: Option<RecallSource>,
    ) -> Result<Vec<RecallSource>, ShopError> {
        if let Some(source) = source {
            self.require(source.dataset())?;
            return Ok(vec![source]);
        }
        let held: Vec<RecallSource> = RecallSource::ALL
            .into_iter()
            .filter(|source| self.require(source.dataset()).is_ok())
            .collect();
        if held.is_empty() {
            return Err(ShopError::DatasetUnavailable(Dataset::RecallsUs));
        }
        Ok(held)
    }

    /// The day a source's campaigns were current, as `2026-10-11`. `None`
    /// when the file does not hold that source.
    pub fn recalls_as_of(&self, source: RecallSource) -> Option<&str> {
        self.datasets()
            .get(source.dataset())
            .map(|entry| entry.as_of.as_str())
    }

    /// The campaigns that list one model year, newest first. A campaign is
    /// given once, however many of its vehicle lines reach the model.
    pub fn recalls_for_vehicle(
        &self,
        make_id: i64,
        model_id: i64,
        year: u16,
        source: Option<RecallSource>,
    ) -> Result<Vec<VehicleRecall>, ShopError> {
        let sources = self.require_recalls(source)?;
        let (first, last) = (sources[0], sources[sources.len() - 1]);
        let rows = self.query(
            RECALLS_FOR_VEHICLE,
            &[
                make_id.into(),
                model_id.into(),
                i64::from(year).into(),
                first.id().into(),
                last.id().into(),
            ],
        )?;
        let mut found: Vec<(i64, VehicleRecall)> = Vec::new();
        for row in &rows {
            let id = optional_integer(row, 0).ok_or(ShopError::Shape(TABLE))?;
            let matched = text(row, 12)?;
            let model = text(row, 13)?;
            let (from, to) = (optional_text(row, 14), optional_text(row, 15));
            match found.last_mut() {
                Some((last_id, recall)) if *last_id == id => {
                    recall.matched = better(&recall.matched, &matched);
                    if !recall.models.contains(&model) {
                        recall.models.push(model);
                    }
                    recall.built_from = match (recall.built_from.take(), from) {
                        (Some(left), Some(right)) => Some(left.min(right)),
                        (left, right) => left.or(right),
                    };
                    recall.built_to = match (recall.built_to.take(), to) {
                        (Some(left), Some(right)) => Some(left.max(right)),
                        (left, right) => left.or(right),
                    };
                }
                _ => found.push((
                    id,
                    VehicleRecall {
                        recall: summary(row)?,
                        matched,
                        models: vec![model],
                        built_from: from,
                        built_to: to,
                    },
                )),
            }
        }
        Ok(found.into_iter().map(|(_, recall)| recall).collect())
    }

    /// One campaign with every vehicle it lists. `None` when the source has
    /// no campaign of that number. Letters in the number may be in either
    /// case.
    pub fn recall(
        &self,
        source: RecallSource,
        campaign: &str,
    ) -> Result<Option<RecallDetail>, ShopError> {
        self.require_recalls(Some(source))?;
        let campaign = campaign.trim().to_ascii_uppercase();
        let rows = self.query(RECALL, &[source.id().into(), campaign.as_str().into()])?;
        let Some(row) = rows.first() else {
            return Ok(None);
        };
        let id = optional_integer(row, 0).ok_or(ShopError::Shape(TABLE))?;
        let mut vehicles = Vec::new();
        for vehicle in self.query(RECALL_VEHICLES, &[id.into()])? {
            vehicles.push(RecallVehicle {
                year: optional_integer(&vehicle, 0).and_then(|year| u16::try_from(year).ok()),
                make_id: optional_integer(&vehicle, 1),
                model_id: optional_integer(&vehicle, 2),
                make_text: text(&vehicle, 3)?,
                model_text: text(&vehicle, 4)?,
                matched: optional_text(&vehicle, 5),
                built_from: optional_text(&vehicle, 6),
                built_to: optional_text(&vehicle, 7),
            });
        }
        Ok(Some(RecallDetail {
            recall: summary(row)?,
            owner_notified: optional_text(row, 12),
            consequence: optional_text(row, 13),
            remedy: optional_text(row, 14),
            notes: optional_text(row, 15),
            part_number: optional_text(row, 16),
            component_fr: optional_text(row, 17),
            notification_fr: optional_text(row, 18),
            summary_fr: optional_text(row, 19),
            vehicles,
        }))
    }

    /// One page of campaigns narrowed by `query`, newest first. This is
    /// the list, the search and "reported since a day" in one.
    pub fn recalls(&self, query: &RecallQuery) -> Result<RecallPage, ShopError> {
        let sources = self.require_recalls(query.source)?;
        let (first, last) = (sources[0], sources[sources.len() - 1]);
        let limit = query.limit.clamp(1, MOST_RECALLS);
        let component = query.component.as_deref().and_then(contains_pattern);
        let text = query.q.as_deref().and_then(contains_pattern);
        let since = query
            .since
            .as_deref()
            .map(str::trim)
            .filter(|since| !since.is_empty());
        let (cursor_date, cursor_id) = match &query.cursor {
            Some(cursor) => (Value::Text(cursor.date.clone()), Value::Integer(cursor.id)),
            None => (Value::Null, Value::Null),
        };
        let rows = self.query(
            RECALL_LIST,
            &[
                first.id().into(),
                last.id().into(),
                component.as_deref().into(),
                text.as_deref().into(),
                since.into(),
                query.make_id.into(),
                query.model_id.into(),
                query.year.map(i64::from).into(),
                cursor_date,
                cursor_id,
                // One more than asked for, to learn whether a page follows.
                Value::Integer(i64::try_from(limit + 1).unwrap_or(i64::MAX)),
            ],
        )?;
        let mut recalls = Vec::with_capacity(limit);
        let mut next_cursor = None;
        for (index, row) in rows.iter().enumerate() {
            if index == limit {
                break;
            }
            recalls.push(summary(row)?);
            if index + 1 == limit && rows.len() > limit {
                next_cursor = Some(RecallCursor {
                    date: optional_text(row, 3).unwrap_or_default(),
                    id: optional_integer(row, 0).ok_or(ShopError::Shape(TABLE))?,
                });
            }
        }
        Ok(RecallPage {
            recalls,
            next_cursor,
        })
    }

    /// The model years with a campaign matched to a catalog model, newest
    /// first.
    pub fn recall_years(&self) -> Result<Vec<u16>, ShopError> {
        self.require_recalls(None)?;
        Ok(self
            .query(RECALL_YEARS, &[])?
            .iter()
            .filter_map(|row| optional_integer(row, 0))
            .filter_map(|year| u16::try_from(year).ok())
            .collect())
    }

    /// The catalog make and model ids with a campaign in one model year.
    pub fn recall_models(&self, year: u16) -> Result<Vec<(i64, i64)>, ShopError> {
        self.require_recalls(None)?;
        Ok(self
            .query(RECALL_MODELS, &[i64::from(year).into()])?
            .iter()
            .filter_map(|row| Some((optional_integer(row, 0)?, optional_integer(row, 1)?)))
            .collect())
    }

    /// The campaigns that list each model year of one model, counted by
    /// country, newest year first. Only years with a campaign are given.
    pub fn recall_counts_by_year(
        &self,
        make_id: i64,
        model_id: i64,
        source: Option<RecallSource>,
    ) -> Result<Vec<RecallYearCount>, ShopError> {
        let sources = self.require_recalls(source)?;
        let (first, last) = (sources[0], sources[sources.len() - 1]);
        let held = |source: RecallSource| sources.contains(&source).then_some(0u32);
        let rows = self.query(
            RECALL_COUNTS_BY_YEAR,
            &[
                make_id.into(),
                model_id.into(),
                first.id().into(),
                last.id().into(),
            ],
        )?;
        let mut found: Vec<RecallYearCount> = Vec::new();
        for row in &rows {
            let year = optional_integer(row, 0)
                .and_then(|year| u16::try_from(year).ok())
                .ok_or(ShopError::Shape(TABLE))?;
            let source = RecallSource::parse(&text(row, 1)?).ok_or(ShopError::Shape(TABLE))?;
            let count = optional_integer(row, 2)
                .and_then(|count| u32::try_from(count).ok())
                .ok_or(ShopError::Shape(TABLE))?;
            if found.last().is_none_or(|last| last.year != year) {
                found.push(RecallYearCount {
                    year,
                    us: held(RecallSource::Nhtsa),
                    ca: held(RecallSource::Tc),
                });
            }
            if let Some(entry) = found.last_mut() {
                match source {
                    RecallSource::Nhtsa => entry.us = Some(count),
                    RecallSource::Tc => entry.ca = Some(count),
                }
            }
        }
        Ok(found)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_source_has_an_id_a_country_and_a_dataset() {
        assert_eq!(RecallSource::Nhtsa.id(), "nhtsa");
        assert_eq!(RecallSource::Tc.id(), "tc");
        assert_eq!(RecallSource::Nhtsa.country(), "us");
        assert_eq!(RecallSource::Tc.country(), "ca");
        assert_eq!(RecallSource::Nhtsa.dataset(), Dataset::RecallsUs);
        assert_eq!(RecallSource::Tc.dataset(), Dataset::RecallsCa);
        assert_eq!(RecallSource::Tc.to_string(), "tc");
        for source in RecallSource::ALL {
            assert_eq!(RecallSource::parse(source.id()), Some(source));
            assert_eq!(RecallSource::from_country(source.country()), Some(source));
        }
        for bad in ["", "NHTSA", "us", "transport-canada"] {
            assert_eq!(RecallSource::parse(bad), None, "{bad}");
        }
        for bad in ["", "US", "nhtsa", "mx"] {
            assert_eq!(RecallSource::from_country(bad), None, "{bad}");
        }
    }

    #[test]
    fn a_cursor_reads_back_what_it_wrote_and_nothing_else() {
        for text in ["2019-12-12_41", "_7", "1968-03-19_1"] {
            let cursor = RecallCursor::parse(text).unwrap();
            assert_eq!(cursor.to_string(), text);
        }
        for bad in [
            "",
            "41",
            "2019-12-12",
            "2019-12-12_",
            "2019-12-12_x",
            "2019-12-12_-4",
            "2019/12/12_41",
            "12-12-2019_41",
            "' OR 1=1 --_1",
            "2019-12-12_9999999999999999999",
        ] {
            assert_eq!(RecallCursor::parse(bad), None, "{bad}");
        }
    }

    #[test]
    fn a_pattern_finds_the_text_anywhere_and_means_it_literally() {
        assert_eq!(contains_pattern("air bag").as_deref(), Some("%air bag%"));
        assert_eq!(contains_pattern("  100%  ").as_deref(), Some("%100\\%%"));
        assert_eq!(contains_pattern("a_b\\c").as_deref(), Some("%a\\_b\\\\c%"));
        assert_eq!(contains_pattern("   "), None);
        let long = "x".repeat(300);
        assert_eq!(
            contains_pattern(&long).unwrap().len(),
            LONGEST_RECALL_QUERY + 2
        );
    }

    #[test]
    fn the_system_is_the_part_of_a_component_before_the_colon() {
        assert_eq!(
            systems("AIR BAGS:FRONTAL:DRIVER SIDE:INFLATOR MODULE"),
            ["AIR BAGS"]
        );
        assert_eq!(
            systems(
                "POWER TRAIN:AUTOMATIC TRANSMISSION:LEVER; POWER TRAIN:AUTOMATIC TRANSMISSION:CONTROL MODULE; SEATS"
            ),
            ["POWER TRAIN", "SEATS"]
        );
        assert_eq!(systems("Electrical"), ["Electrical"]);
        assert_eq!(
            systems("FUEL SYSTEM, GASOLINE:DELIVERY"),
            ["FUEL SYSTEM, GASOLINE"]
        );
        assert!(systems("").is_empty());
        assert!(systems(" ; :x").is_empty());
    }

    #[test]
    fn the_better_match_wins() {
        assert_eq!(better("base", "exact"), "exact");
        assert_eq!(better("exact", "curated"), "exact");
        assert_eq!(better("curated", "base"), "base");
        assert_eq!(better("curated", "curated"), "curated");
    }
}
