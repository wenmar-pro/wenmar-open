//! Manufacturer communications NHTSA publishes: service bulletins,
//! service campaigns, warranty extensions, over-the-air updates.
//!
//! The file holds NHTSA's index of each communication and its summary.
//! The document itself is the manufacturer's and is never stored here.

use std::fmt;

use serde::Serialize;
use wenmar_vehicles::{Source, Value};

use crate::dataset::Dataset;
use crate::error::ShopError;
use crate::recalls::contains_pattern;
use crate::shop::ShopData;

/// What kind of communication a bulletin is.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum BulletinType {
    ServiceBulletin,
    ServiceCampaign,
    WarrantyExtension,
    OverTheAir,
    Emissions,
    Other,
}

impl BulletinType {
    pub const ALL: [BulletinType; 6] = [
        BulletinType::ServiceBulletin,
        BulletinType::ServiceCampaign,
        BulletinType::WarrantyExtension,
        BulletinType::OverTheAir,
        BulletinType::Emissions,
        BulletinType::Other,
    ];

    /// The `type` column, and the word a caller filters by.
    pub fn id(self) -> &'static str {
        match self {
            BulletinType::ServiceBulletin => "service_bulletin",
            BulletinType::ServiceCampaign => "service_campaign",
            BulletinType::WarrantyExtension => "warranty_extension",
            BulletinType::OverTheAir => "over_the_air",
            BulletinType::Emissions => "emissions",
            BulletinType::Other => "other",
        }
    }

    /// The name a page shows.
    pub fn label(self) -> &'static str {
        match self {
            BulletinType::ServiceBulletin => "Service bulletin",
            BulletinType::ServiceCampaign => "Service campaign",
            BulletinType::WarrantyExtension => "Warranty extension",
            BulletinType::OverTheAir => "Over-the-air update",
            BulletinType::Emissions => "Emissions",
            BulletinType::Other => "Other",
        }
    }

    /// The type as NHTSA's files spell it.
    pub fn source_text(self) -> &'static str {
        match self {
            BulletinType::ServiceBulletin => "Service Bulletin/Repair Instructions",
            BulletinType::ServiceCampaign => "Service Campaign",
            BulletinType::WarrantyExtension => "Warranty Program/Extension",
            BulletinType::OverTheAir => "Over The Air",
            BulletinType::Emissions => "Emissions",
            BulletinType::Other => "Other",
        }
    }

    /// Reads the `type` column. `None` for anything else.
    pub fn parse(id: &str) -> Option<BulletinType> {
        BulletinType::ALL.into_iter().find(|kind| kind.id() == id)
    }

    /// Reads NHTSA's spelling, in either case and with or without spaces
    /// around the slash. `None` for a type this build does not know.
    pub fn from_source(text: &str) -> Option<BulletinType> {
        let squeezed = |text: &str| -> String {
            text.chars()
                .filter(|character| !character.is_whitespace())
                .flat_map(char::to_lowercase)
                .collect()
        };
        let wanted = squeezed(text);
        BulletinType::ALL
            .into_iter()
            .find(|kind| squeezed(kind.source_text()) == wanted)
    }
}

impl fmt::Display for BulletinType {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.id())
    }
}

/// Most bulletins one page of a list holds, whatever `limit` says.
pub const MOST_BULLETINS: usize = 100;

/// Most trouble codes a count of a model year's bulletins names.
pub const MOST_CODES: usize = 20;

/// Where NHTSA keeps its copies of the documents.
pub const DOCUMENT_BASE: &str = "https://static.nhtsa.gov/odi/tsbs/";

/// The lowest NHTSA id for which a document was found at an address built
/// by [`document_urls`] when the rule was checked on 2026-10-12. Every id
/// tried below it, the nearest being 10107070, answered "not found".
pub const FIRST_DOCUMENT_ID: i64 = 10_108_757;

/// Bulletins NHTSA published up to this year were all found at the
/// address ending `-9999.pdf`; later ones at either, `-0001.pdf` more
/// often.
pub const LAST_YEAR_OF_9999_FIRST: &str = "2018";

/// The two addresses NHTSA's copy of a bulletin's document may be at, the
/// likelier first: `<base><year>/MC-<id>-0001.pdf` and the same ending
/// `-9999.pdf`, where the year is the year of the manufacturer's own date.
/// `date_added` only decides which of the two comes first. Neither is
/// known to answer: the rule was confirmed for a sample only. `None` when
/// the bulletin has no manufacturer's date or its id is below
/// [`FIRST_DOCUMENT_ID`]; no address is known for those.
pub fn document_urls(
    nhtsa_id: i64,
    date: Option<&str>,
    date_added: &str,
) -> Option<(String, String)> {
    let year = date?.get(0..4)?;
    if nhtsa_id < FIRST_DOCUMENT_ID || !year.bytes().all(|byte| byte.is_ascii_digit()) {
        return None;
    }
    let address = |ending: &str| format!("{DOCUMENT_BASE}{year}/MC-{nhtsa_id}-{ending}.pdf");
    if date_added.get(0..4).unwrap_or("9999") <= LAST_YEAR_OF_9999_FIRST {
        Some((address("9999"), address("0001")))
    } else {
        Some((address("0001"), address("9999")))
    }
}

/// A trouble code as `bulletin_code` stores it: `P0420`. `None` for text
/// that is not a code. Letters may be given in either case, and a longer
/// form such as `P0420:00` or `P042000` is cut to its first five
/// characters.
pub fn code_form(text: &str) -> Option<String> {
    let text = text.trim().to_ascii_uppercase();
    let code = text.get(0..5)?;
    let bytes = code.as_bytes();
    let hex = |byte: &u8| byte.is_ascii_digit() || (b'A'..=b'F').contains(byte);
    (matches!(bytes[0], b'P' | b'B' | b'C' | b'U')
        && (b'0'..=b'3').contains(&bytes[1])
        && bytes[2..].iter().all(hex))
    .then(|| code.to_owned())
}

/// A bulletin as a list shows it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct BulletinSummary {
    /// NHTSA's id number for the communication.
    pub nhtsa_id: i64,
    /// One of [`BulletinType`]'s ids.
    #[serde(rename = "type")]
    pub kind: String,
    /// The day NHTSA published it, as `2019-01-07`.
    pub date_added: String,
    /// The manufacturer's own date.
    pub date: Option<String>,
    /// The manufacturer's own number for the document.
    pub document_id: Option<String>,
    /// The manufacturer's campaign id or software version.
    pub mfr_campaign: Option<String>,
    /// NHTSA's components, as it words them, separated by `; `.
    pub component: Option<String>,
    /// NHTSA's summary of the communication.
    pub summary: Option<String>,
}

/// A component a bulletin is about, from `data/components.yaml`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct BulletinComponent {
    pub id: String,
    pub name: String,
    /// The system a part belongs to. `None` for a system.
    pub parent: Option<String>,
}

/// The model years of one name a bulletin lists.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct BulletinVehicle {
    /// The make and model as the source words them.
    pub make_text: String,
    pub model_text: String,
    /// The first and last model year of a run, inclusive. Both `None`
    /// when the bulletin gave no model year.
    pub year_from: Option<u16>,
    pub year_to: Option<u16>,
    /// `catalog_make.id`, when the make is in the catalog.
    pub make_id: Option<i64>,
    /// `catalog_model.id`, when the model matched in these years.
    pub model_id: Option<i64>,
    /// `exact`, `base` or `curated`; `None` when the model did not match.
    #[serde(rename = "match")]
    pub matched: Option<String>,
}

/// One bulletin with everything the file holds of it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct BulletinDetail {
    #[serde(flatten)]
    pub bulletin: BulletinSummary,
    /// The number of the bulletin this one replaces. NHTSA no longer
    /// fills it.
    pub replaces: Option<String>,
    /// The manufacturer's own words for the system, separated by `; `.
    pub mfr_system: Option<String>,
    pub components: Vec<BulletinComponent>,
    /// The trouble codes the summary names, sorted.
    pub codes: Vec<String>,
    /// `maintenance`, or nothing.
    pub topics: Vec<String>,
    pub vehicles: Vec<BulletinVehicle>,
}

/// Where a list left off. Its text form is for handing back unchanged.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BulletinCursor {
    added: String,
    id: i64,
}

impl BulletinCursor {
    /// Reads a cursor a list gave out. `None` for anything else.
    pub fn parse(text: &str) -> Option<BulletinCursor> {
        let (added, id) = text.rsplit_once('_')?;
        let dated = added.len() == 10
            && added.bytes().enumerate().all(|(index, byte)| match index {
                4 | 7 => byte == b'-',
                _ => byte.is_ascii_digit(),
            });
        if !dated || id.is_empty() || id.len() > 18 || !id.bytes().all(|byte| byte.is_ascii_digit())
        {
            return None;
        }
        Some(BulletinCursor {
            added: added.to_owned(),
            id: id.parse().ok()?,
        })
    }
}

impl fmt::Display for BulletinCursor {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{}_{}", self.added, self.id)
    }
}

/// What a list of bulletins is narrowed by. Everything left `None` is not
/// narrowed by.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct BulletinQuery {
    /// Only bulletins that list a vehicle of this make, model or year.
    pub make_id: Option<i64>,
    pub model_id: Option<i64>,
    pub year: Option<u16>,
    pub kind: Option<BulletinType>,
    /// Only bulletins about this component: an id of `components.yaml`.
    pub component: Option<String>,
    /// Only bulletins whose summary names this trouble code, in
    /// [`code_form`].
    pub code: Option<String>,
    /// Only bulletins with this topic, such as `maintenance`.
    pub topic: Option<String>,
    /// Only bulletins with this text in them. See [`ShopData::bulletins`].
    pub q: Option<String>,
    /// At most this many, up to [`MOST_BULLETINS`].
    pub limit: usize,
    /// Where the page before this one left off.
    pub cursor: Option<BulletinCursor>,
}

/// One page of a list, newest first.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BulletinPage {
    pub bulletins: Vec<BulletinSummary>,
    /// The cursor for the next page. `None` when this is the last.
    pub next_cursor: Option<BulletinCursor>,
}

/// How many of something a model year's bulletins hold.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct BulletinCount {
    pub id: String,
    /// The name a page shows.
    pub name: String,
    pub count: u64,
}

/// What the bulletins of one model year are, in numbers.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
pub struct BulletinCounts {
    pub count: u64,
    /// Every type with at least one bulletin, in [`BulletinType::ALL`]'s
    /// order.
    pub by_type: Vec<BulletinCount>,
    /// Every component with at least one, commonest first.
    pub components: Vec<BulletinCount>,
    /// The commonest trouble codes named, up to [`MOST_CODES`].
    pub codes: Vec<BulletinCount>,
    pub topics: Vec<BulletinCount>,
}

/// The columns every list reads, in the order [`summary`] expects.
const COLUMNS: &str =
    "b.id, b.type, b.added, b.date, b.document_id, b.mfr_campaign, b.component, b.summary";

/// The bulletins that list one model year. `{model}` and `{year}` are
/// parameter numbers.
const OF_MODEL_YEAR: &str = "SELECT v.bulletin_id FROM bulletin_model m JOIN bulletin_vehicle v ON v.bulletin_model_id = m.id WHERE m.model_id = ?{model} AND v.year_from <= ?{year} AND v.year_to >= ?{year}";

/// One bulletin's own columns. `?1` its id.
pub const BULLETIN: &str = "
SELECT b.id, b.type, b.added, b.date, b.document_id, b.mfr_campaign, b.component, b.summary, b.replaces, b.mfr_system
FROM bulletin b WHERE b.id = ?1";

/// The vehicles of one bulletin. `?1` its id.
pub const BULLETIN_VEHICLES: &str = "
SELECT m.make_text, m.model_text, v.year_from, v.year_to, m.make_id, m.model_id, m.\"match\"
FROM bulletin_vehicle v JOIN bulletin_model m ON m.id = v.bulletin_model_id
WHERE v.bulletin_id = ?1
ORDER BY m.make_text, m.model_text, v.year_from, m.model_id";

/// The components of one bulletin, systems first. `?1` its id.
pub const BULLETIN_COMPONENTS_OF: &str = "
SELECT k.id, k.name, k.parent FROM bulletin_component c JOIN component k ON k.id = c.component_id
WHERE c.bulletin_id = ?1 ORDER BY (k.parent IS NOT NULL), k.name";

const TABLE: &str = "bulletin";

fn text(row: &[Value], column: usize) -> Result<String, ShopError> {
    row.get(column)
        .and_then(Value::text)
        .map(str::to_owned)
        .ok_or(ShopError::Shape(TABLE))
}

fn optional_text(row: &[Value], column: usize) -> Option<String> {
    row.get(column).and_then(Value::text).map(str::to_owned)
}

fn integer(row: &[Value], column: usize) -> Result<i64, ShopError> {
    row.get(column)
        .and_then(Value::integer)
        .ok_or(ShopError::Shape(TABLE))
}

fn count(row: &[Value], column: usize) -> u64 {
    row.get(column)
        .and_then(Value::integer)
        .and_then(|number| u64::try_from(number).ok())
        .unwrap_or(0)
}

/// Reads a [`BulletinSummary`] from [`COLUMNS`].
fn summary(row: &[Value]) -> Result<BulletinSummary, ShopError> {
    Ok(BulletinSummary {
        nhtsa_id: integer(row, 0)?,
        kind: text(row, 1)?,
        date_added: text(row, 2)?,
        date: optional_text(row, 3),
        document_id: optional_text(row, 4),
        mfr_campaign: optional_text(row, 5),
        component: optional_text(row, 6),
        summary: optional_text(row, 7),
    })
}

/// A model year of `bulletin_vehicle`: 0 means "none given".
fn model_year(row: &[Value], column: usize) -> Option<u16> {
    row.get(column)
        .and_then(Value::integer)
        .filter(|year| *year != crate::schema::BULLETIN_NO_YEAR)
        .and_then(|year| u16::try_from(year).ok())
}

impl<S: Source> ShopData<S> {
    /// The day the bulletins were current, as `2026-10-11`. `None` when
    /// the file does not hold them.
    pub fn bulletins_as_of(&self) -> Option<&str> {
        self.datasets()
            .get(Dataset::Bulletins)
            .map(|entry| entry.as_of.as_str())
    }

    /// One page of bulletins narrowed by `query`, newest first by the day
    /// NHTSA published them. This is a model year's list, the search and
    /// the list of everything in one.
    ///
    /// Free text is matched with `LIKE`, so letters match in either case
    /// and the text may be anywhere in: the summary, the manufacturer's
    /// document number, its campaign id, NHTSA's component text and the
    /// manufacturer's own words for the system. It is a scan of whatever
    /// the other narrowings leave, not an index.
    pub fn bulletins(&self, query: &BulletinQuery) -> Result<BulletinPage, ShopError> {
        self.require(Dataset::Bulletins)?;
        let limit = query.limit.clamp(1, MOST_BULLETINS);
        let mut wheres: Vec<String> = Vec::new();
        let mut params: Vec<Value> = Vec::new();
        // Adds a parameter and gives its number.
        let mut bind = |value: Value| {
            params.push(value);
            params.len()
        };
        match (query.model_id, query.year) {
            (Some(model_id), Some(year)) => {
                let model = bind(model_id.into());
                let year = bind(i64::from(year).into());
                wheres.push(format!(
                    "b.id IN ({})",
                    OF_MODEL_YEAR
                        .replace("{model}", &model.to_string())
                        .replace("{year}", &year.to_string())
                ));
            }
            (model_id, year) if query.make_id.is_some() || model_id.is_some() || year.is_some() => {
                let mut inner: Vec<String> = Vec::new();
                if let Some(make_id) = query.make_id {
                    inner.push(format!("m.make_id = ?{}", bind(make_id.into())));
                }
                if let Some(model_id) = model_id {
                    inner.push(format!("m.model_id = ?{}", bind(model_id.into())));
                }
                if let Some(year) = year {
                    let number = bind(i64::from(year).into());
                    inner.push(format!(
                        "v.year_from <= ?{number} AND v.year_to >= ?{number}"
                    ));
                }
                wheres.push(format!(
                    "b.id IN (SELECT v.bulletin_id FROM bulletin_model m JOIN bulletin_vehicle v ON v.bulletin_model_id = m.id WHERE {})",
                    inner.join(" AND ")
                ));
            }
            _ => {}
        }
        if let Some(kind) = query.kind {
            wheres.push(format!("b.type = ?{}", bind(kind.id().into())));
        }
        if let Some(component) = query.component.as_deref() {
            wheres.push(format!(
                "b.id IN (SELECT bulletin_id FROM bulletin_component WHERE component_id = ?{})",
                bind(component.into())
            ));
        }
        if let Some(code) = query.code.as_deref() {
            wheres.push(format!(
                "b.id IN (SELECT bulletin_id FROM bulletin_code WHERE code = ?{})",
                bind(code.into())
            ));
        }
        if let Some(topic) = query.topic.as_deref() {
            wheres.push(format!(
                "b.id IN (SELECT bulletin_id FROM bulletin_topic WHERE topic = ?{})",
                bind(topic.into())
            ));
        }
        if let Some(pattern) = query.q.as_deref().and_then(contains_pattern) {
            let number = bind(pattern.as_str().into());
            wheres.push(format!(
                "(b.summary LIKE ?{number} ESCAPE '\\' OR b.document_id LIKE ?{number} ESCAPE '\\' OR b.mfr_campaign LIKE ?{number} ESCAPE '\\' OR b.component LIKE ?{number} ESCAPE '\\' OR b.mfr_system LIKE ?{number} ESCAPE '\\')"
            ));
        }
        if let Some(cursor) = &query.cursor {
            let added = bind(cursor.added.as_str().into());
            let id = bind(cursor.id.into());
            wheres.push(format!(
                "(b.added < ?{added} OR (b.added = ?{added} AND b.id < ?{id}))"
            ));
        }
        // One more than asked for, to learn whether a page follows.
        let limit_number = bind(Value::Integer(i64::try_from(limit + 1).unwrap_or(i64::MAX)));
        let filter = if wheres.is_empty() {
            String::new()
        } else {
            format!("WHERE {}", wheres.join(" AND "))
        };
        let sql = format!(
            "SELECT {COLUMNS} FROM bulletin b {filter} ORDER BY b.added DESC, b.id DESC LIMIT ?{limit_number}"
        );
        let rows = self.query(&sql, &params)?;
        let mut bulletins = Vec::with_capacity(limit);
        let mut next_cursor = None;
        for (index, row) in rows.iter().enumerate() {
            if index == limit {
                break;
            }
            let bulletin = summary(row)?;
            if index + 1 == limit && rows.len() > limit {
                next_cursor = Some(BulletinCursor {
                    added: bulletin.date_added.clone(),
                    id: bulletin.nhtsa_id,
                });
            }
            bulletins.push(bulletin);
        }
        Ok(BulletinPage {
            bulletins,
            next_cursor,
        })
    }

    /// One bulletin with its vehicles, components, codes and topics.
    /// `None` when NHTSA has no communication of that id.
    pub fn bulletin(&self, nhtsa_id: i64) -> Result<Option<BulletinDetail>, ShopError> {
        self.require(Dataset::Bulletins)?;
        let rows = self.query(BULLETIN, &[nhtsa_id.into()])?;
        let Some(row) = rows.first() else {
            return Ok(None);
        };
        let mut vehicles = Vec::new();
        for vehicle in self.query(BULLETIN_VEHICLES, &[nhtsa_id.into()])? {
            vehicles.push(BulletinVehicle {
                make_text: text(&vehicle, 0)?,
                model_text: text(&vehicle, 1)?,
                year_from: model_year(&vehicle, 2),
                year_to: model_year(&vehicle, 3),
                make_id: vehicle.get(4).and_then(Value::integer),
                model_id: vehicle.get(5).and_then(Value::integer),
                matched: optional_text(&vehicle, 6),
            });
        }
        let mut components = Vec::new();
        for component in self.query(BULLETIN_COMPONENTS_OF, &[nhtsa_id.into()])? {
            components.push(BulletinComponent {
                id: text(&component, 0)?,
                name: text(&component, 1)?,
                parent: optional_text(&component, 2),
            });
        }
        let one_column = |sql: &str| -> Result<Vec<String>, ShopError> {
            self.query(sql, &[nhtsa_id.into()])?
                .iter()
                .map(|row| text(row, 0))
                .collect()
        };
        Ok(Some(BulletinDetail {
            bulletin: summary(row)?,
            replaces: optional_text(row, 8),
            mfr_system: optional_text(row, 9),
            components,
            codes: one_column(
                "SELECT code FROM bulletin_code WHERE bulletin_id = ?1 ORDER BY code",
            )?,
            topics: one_column(
                "SELECT topic FROM bulletin_topic WHERE bulletin_id = ?1 ORDER BY topic",
            )?,
            vehicles,
        }))
    }

    /// What the bulletins of one model year are, in numbers: how many, by
    /// type, by component, by topic, and the codes they name most.
    pub fn bulletin_counts(
        &self,
        make_id: i64,
        model_id: i64,
        year: u16,
    ) -> Result<BulletinCounts, ShopError> {
        self.require(Dataset::Bulletins)?;
        // The make is implied by the model; it is taken so that every
        // reader of a model year has the same arguments.
        let _ = make_id;
        let of_model_year = OF_MODEL_YEAR.replace("{model}", "1").replace("{year}", "2");
        let params: [Value; 2] = [model_id.into(), i64::from(year).into()];
        let mut counts = BulletinCounts::default();
        let by_type = self.query(
            &format!(
                "SELECT b.type, COUNT(*) FROM bulletin b WHERE b.id IN ({of_model_year}) GROUP BY b.type"
            ),
            &params,
        )?;
        for kind in BulletinType::ALL {
            let found = by_type
                .iter()
                .find(|row| row.first().and_then(Value::text) == Some(kind.id()))
                .map_or(0, |row| count(row, 1));
            counts.count += found;
            if found != 0 {
                counts.by_type.push(BulletinCount {
                    id: kind.id().to_owned(),
                    name: kind.label().to_owned(),
                    count: found,
                });
            }
        }
        for row in self.query(
            &format!(
                "SELECT k.id, k.name, COUNT(*) FROM bulletin_component c JOIN component k ON k.id = c.component_id WHERE c.bulletin_id IN ({of_model_year}) GROUP BY k.id ORDER BY 3 DESC, k.name"
            ),
            &params,
        )? {
            counts.components.push(BulletinCount {
                id: text(&row, 0)?,
                name: text(&row, 1)?,
                count: count(&row, 2),
            });
        }
        for row in self.query(
            &format!(
                "SELECT code, COUNT(*) FROM bulletin_code WHERE bulletin_id IN ({of_model_year}) GROUP BY code ORDER BY 2 DESC, code LIMIT {MOST_CODES}"
            ),
            &params,
        )? {
            let code = text(&row, 0)?;
            counts.codes.push(BulletinCount {
                name: code.clone(),
                id: code,
                count: count(&row, 1),
            });
        }
        for row in self.query(
            &format!(
                "SELECT topic, COUNT(*) FROM bulletin_topic WHERE bulletin_id IN ({of_model_year}) GROUP BY topic ORDER BY topic"
            ),
            &params,
        )? {
            let topic = text(&row, 0)?;
            counts.topics.push(BulletinCount {
                name: topic.clone(),
                id: topic,
                count: count(&row, 1),
            });
        }
        Ok(counts)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_type_has_an_id_a_label_and_nhtsas_spelling() {
        let ids: Vec<&str> = BulletinType::ALL.iter().map(|kind| kind.id()).collect();
        assert_eq!(
            ids,
            [
                "service_bulletin",
                "service_campaign",
                "warranty_extension",
                "over_the_air",
                "emissions",
                "other"
            ]
        );
        for kind in BulletinType::ALL {
            assert_eq!(BulletinType::parse(kind.id()), Some(kind));
            assert_eq!(BulletinType::from_source(kind.source_text()), Some(kind));
            assert_eq!(kind.to_string(), kind.id());
            assert!(!kind.label().is_empty());
        }
        // TSBS.txt writes two of them with spaces around the slash.
        assert_eq!(
            BulletinType::from_source("Service Bulletin / Repair Instructions"),
            Some(BulletinType::ServiceBulletin)
        );
        assert_eq!(
            BulletinType::from_source("WARRANTY PROGRAM / EXTENSION"),
            Some(BulletinType::WarrantyExtension)
        );
        for bad in ["", "Recall", "service_bulletin"] {
            assert_eq!(BulletinType::from_source(bad), None, "{bad}");
        }
        for bad in ["", "Service Campaign", "ota"] {
            assert_eq!(BulletinType::parse(bad), None, "{bad}");
        }
    }
}
