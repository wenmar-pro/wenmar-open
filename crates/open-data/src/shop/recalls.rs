//! What the two recall shape steps share: the tables, and how a campaign
//! and a vehicle it lists are written.

use anyhow::Result;
use rusqlite::{Transaction, params};
use wenmar_shopdata::schema;

use crate::shop::matcher::Matcher;

/// The credit the Open Government Licence - Canada asks for when the
/// provider names no statement of its own.
// [REVIEW] Check this sentence, its dash included, against the licence text at https://open.canada.ca/en/open-government-licence-canada and against the dataset page of the Vehicle Recalls Database before the first release.
pub const OGL_CANADA_ATTRIBUTION: &str =
    "Contains information licensed under the Open Government Licence \u{2013} Canada.";

/// One campaign, as it is stored.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct RecallRecord<'a> {
    /// `nhtsa` or `tc`.
    pub source: &'a str,
    pub campaign: &'a str,
    pub manufacturer: Option<&'a str>,
    pub report_date: Option<&'a str>,
    pub owner_notified: Option<&'a str>,
    pub component: Option<&'a str>,
    pub component_fr: Option<&'a str>,
    pub notification: Option<&'a str>,
    pub notification_fr: Option<&'a str>,
    pub summary_en: Option<&'a str>,
    pub consequence_en: Option<&'a str>,
    pub remedy_en: Option<&'a str>,
    pub notes_en: Option<&'a str>,
    pub summary_fr: Option<&'a str>,
    pub mfr_campaign: Option<&'a str>,
    pub part_number: Option<&'a str>,
    pub units: Option<u32>,
    pub do_not_drive: Option<bool>,
    pub park_outside: Option<bool>,
}

/// One vehicle a campaign lists, in the source's words.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct VehicleLine<'a> {
    pub year: Option<i32>,
    pub make_text: &'a str,
    pub model_text: &'a str,
    pub built_from: Option<&'a str>,
    pub built_to: Option<&'a str>,
}

const INSERT_RECALL: &str = "
INSERT INTO recall (
    source, campaign, manufacturer, report_date, owner_notified, component, component_fr,
    notification, notification_fr, summary_en, consequence_en, remedy_en, notes_en, summary_fr,
    mfr_campaign, part_number, units, do_not_drive, park_outside
) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16, ?17, ?18, ?19)";

const INSERT_VEHICLE: &str = "
INSERT INTO recall_vehicle (
    recall_id, year, make_id, model_id, make_text, model_text, \"match\", built_from, built_to
) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)";

/// Creates the recall tables, unless the other recall step already has.
pub fn create_tables(transaction: &Transaction<'_>) -> Result<()> {
    transaction.execute_batch(schema::RECALLS)?;
    Ok(())
}

/// Stores one campaign and returns its row id.
pub fn insert_recall(transaction: &Transaction<'_>, record: &RecallRecord<'_>) -> Result<i64> {
    transaction.prepare_cached(INSERT_RECALL)?.execute(params![
        record.source,
        record.campaign,
        record.manufacturer,
        record.report_date.unwrap_or(""),
        record.owner_notified,
        record.component,
        record.component_fr,
        record.notification,
        record.notification_fr,
        record.summary_en,
        record.consequence_en,
        record.remedy_en,
        record.notes_en,
        record.summary_fr,
        record.mfr_campaign,
        record.part_number,
        record.units,
        record.do_not_drive,
        record.park_outside,
    ])?;
    Ok(transaction.last_insert_rowid())
}

/// Matches one vehicle to the catalog and stores it: one row for each
/// catalog model it matched, or one row with no model when it matched
/// none. The make id is stored whenever the make is in the catalog.
pub fn insert_vehicle(
    transaction: &Transaction<'_>,
    matcher: &Matcher,
    recall_id: i64,
    line: &VehicleLine<'_>,
) -> Result<()> {
    let resolved = matcher.resolve(line.year, line.make_text, line.model_text);
    let models: Vec<Option<i64>> = if resolved.model_ids.is_empty() {
        vec![None]
    } else {
        resolved.model_ids.iter().copied().map(Some).collect()
    };
    let mut statement = transaction.prepare_cached(INSERT_VEHICLE)?;
    for model_id in models {
        statement.execute(params![
            recall_id,
            line.year,
            resolved.make_id,
            model_id,
            line.make_text,
            line.model_text,
            model_id.and(resolved.match_text()),
            line.built_from,
            line.built_to,
        ])?;
    }
    Ok(())
}
