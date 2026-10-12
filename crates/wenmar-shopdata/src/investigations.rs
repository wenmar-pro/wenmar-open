//! NHTSA defect investigations.
//!
//! An investigation is NHTSA looking into a possible defect. It is not a
//! finding that a defect exists: most are closed with no recall. Nothing
//! here says a vehicle has a defect, and nothing here is about one vehicle:
//! an investigation lists a make, a model and a model year.

use serde::Serialize;
use wenmar_vehicles::{Source, Value};

use crate::dataset::Dataset;
use crate::error::ShopError;
use crate::shop::ShopData;

/// The earliest day opened that still counts as open when the file gives
/// no day closed. NHTSA's file leaves 39 investigations opened from 1995
/// to 2006 without a day closed; none of them is still being worked on.
/// The next one without a day closed was opened in 2015.
pub const OPEN_FROM: &str = "2010-01-01";

/// The kinds of investigation that are about a possible defect in the
/// vehicles they list, and so are said of a model year while open:
/// preliminary evaluation, engineering analysis, defect petition and
/// recall query. An audit query (`AQ`) is about what a company did, such
/// as a dealer that sold vehicles with open recalls, and lists the
/// vehicles only as the ones it happened with.
pub const VEHICLE_KINDS: [&str; 4] = ["PE", "EA", "DP", "RQ"];

/// What every page and answer about investigations says.
pub const NOT_A_FINDING: &str = "An investigation is NHTSA looking into a possible defect. It is not a finding that a defect exists, and it does not say that any one vehicle is affected.";

/// The kind of an investigation, from the letters its action number
/// starts with: `PE` for `PE24009`, `EA` for `EA78A17`, `C` for `C85007`.
pub fn kind_of(action: &str) -> String {
    action
        .chars()
        .take_while(char::is_ascii_alphabetic)
        .take(2)
        .collect::<String>()
        .to_ascii_uppercase()
}

/// A kind in words. `None` for a kind this project has no name for.
// [REVIEW] These eight names are the ones NHTSA uses for its action number prefixes. A person should confirm each against nhtsa.gov before the first release, and say what TA, ID and C stand for; until then those three have no name and are shown by their letters.
pub fn kind_name(kind: &str) -> Option<&'static str> {
    match kind {
        "PE" => Some("Preliminary evaluation"),
        "EA" => Some("Engineering analysis"),
        "DP" => Some("Defect petition"),
        "RQ" => Some("Recall query"),
        "AQ" => Some("Audit query"),
        "EQ" => Some("Equipment query"),
        "SQ" => Some("Service query"),
        "RP" => Some("Recall petition"),
        _ => None,
    }
}

/// One investigation.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Investigation {
    /// NHTSA's action number, such as `PE24009`.
    pub action: String,
    /// The letters the number starts with, such as `PE`.
    pub kind: String,
    /// The kind in words, such as `Preliminary evaluation`.
    pub kind_name: Option<String>,
    /// The day it was opened and the day it was closed, as `2024-03-27`.
    pub opened: Option<String>,
    pub closed: Option<String>,
    /// Whether it was opened on or after [`OPEN_FROM`] and is not closed.
    pub open: bool,
    /// The manufacturers it names, separated by `; `.
    pub manufacturer: Option<String>,
    /// The components it names, separated by `; `.
    pub component: Option<String>,
    /// NHTSA's one-line subject.
    pub subject: Option<String>,
    /// NHTSA's summary, which can be several thousand characters.
    pub summary: Option<String>,
    /// The recall campaigns it led to, as NHTSA numbers them.
    pub campaigns: Vec<String>,
}

/// An investigation that lists one model year.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct VehicleInvestigation {
    #[serde(flatten)]
    pub investigation: Investigation,
    /// How its vehicle text reached this model: `exact`, `base` or
    /// `curated`.
    #[serde(rename = "match")]
    pub matched: String,
    /// The model names it lists that reached this model year.
    pub models: Vec<String>,
}

/// Investigations that list one model year, newest first. `?1` make id,
/// `?2` model id, `?3` year, `?4` 1 for only the open ones of
/// [`VEHICLE_KINDS`] and 0 for every one.
pub const INVESTIGATIONS_FOR_VEHICLE: &str = "
SELECT i.id, i.action, i.kind, i.opened, i.closed, i.is_open, i.manufacturer, i.component, i.subject, i.summary,
       v.\"match\", v.model_text
FROM investigation_vehicle v
JOIN investigation i ON i.id = v.investigation_id
WHERE v.make_id = ?1 AND v.model_id = ?2 AND v.year = ?3
  AND (?4 = 0 OR (i.is_open = 1 AND i.kind IN ('PE', 'EA', 'DP', 'RQ')))
ORDER BY i.opened DESC, i.id DESC, v.model_text";

/// Investigations that led to one recall campaign, oldest first. `?1` the
/// campaign number.
pub const INVESTIGATIONS_FOR_CAMPAIGN: &str = "
SELECT i.id, i.action, i.kind, i.opened, i.closed, i.is_open, i.manufacturer, i.component, i.subject, i.summary
FROM investigation_campaign c
JOIN investigation i ON i.id = c.investigation_id
WHERE c.campaign = ?1
ORDER BY i.opened, i.id";

/// The campaigns one investigation led to. `?1` the investigation's row id.
pub const INVESTIGATION_CAMPAIGNS: &str = "
SELECT campaign FROM investigation_campaign
WHERE investigation_id = ?1
ORDER BY campaign";

const TABLE: &str = "investigation";

fn text(row: &[Value], column: usize) -> Result<String, ShopError> {
    row.get(column)
        .and_then(Value::text)
        .map(str::to_owned)
        .ok_or(ShopError::Shape(TABLE))
}

fn optional_text(row: &[Value], column: usize) -> Option<String> {
    row.get(column).and_then(Value::text).map(str::to_owned)
}

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

impl<S: Source> ShopData<S> {
    /// The day the investigations were current, as `2026-10-12`. `None`
    /// when the file does not hold them.
    pub fn investigations_as_of(&self) -> Option<&str> {
        self.datasets()
            .get(Dataset::Investigations)
            .map(|entry| entry.as_of.as_str())
    }

    /// Reads an [`Investigation`] from the first ten columns of a row, and
    /// its campaigns with one more query.
    fn investigation(&self, row: &[Value]) -> Result<(i64, Investigation), ShopError> {
        let id = row
            .first()
            .and_then(Value::integer)
            .ok_or(ShopError::Shape(TABLE))?;
        let kind = text(row, 2)?;
        let campaigns = self
            .query(INVESTIGATION_CAMPAIGNS, &[id.into()])?
            .iter()
            .filter_map(|found| optional_text(found, 0))
            .collect();
        Ok((
            id,
            Investigation {
                action: text(row, 1)?,
                kind_name: kind_name(&kind).map(str::to_owned),
                kind,
                opened: optional_text(row, 3),
                closed: optional_text(row, 4),
                open: row.get(5).and_then(Value::integer) == Some(1),
                manufacturer: optional_text(row, 6),
                component: optional_text(row, 7),
                subject: optional_text(row, 8),
                summary: optional_text(row, 9),
                campaigns,
            },
        ))
    }

    /// The investigations that list one model year, newest first. With
    /// `open_only`, only those that are open and of [`VEHICLE_KINDS`]: the
    /// ones a page may call "under investigation". An investigation is
    /// given once, however many of its vehicle lines reach the model.
    pub fn investigations_for_vehicle(
        &self,
        make_id: i64,
        model_id: i64,
        year: u16,
        open_only: bool,
    ) -> Result<Vec<VehicleInvestigation>, ShopError> {
        self.require(Dataset::Investigations)?;
        let rows = self.query(
            INVESTIGATIONS_FOR_VEHICLE,
            &[
                make_id.into(),
                model_id.into(),
                i64::from(year).into(),
                i64::from(open_only).into(),
            ],
        )?;
        let mut found: Vec<(i64, VehicleInvestigation)> = Vec::new();
        for row in &rows {
            let id = row
                .first()
                .and_then(Value::integer)
                .ok_or(ShopError::Shape(TABLE))?;
            let matched = text(row, 10)?;
            let model = text(row, 11)?;
            match found.last_mut() {
                Some((last_id, known)) if *last_id == id => {
                    known.matched = better(&known.matched, &matched);
                    if !known.models.contains(&model) {
                        known.models.push(model);
                    }
                }
                _ => {
                    let (_, investigation) = self.investigation(row)?;
                    found.push((
                        id,
                        VehicleInvestigation {
                            investigation,
                            matched,
                            models: vec![model],
                        },
                    ));
                }
            }
        }
        Ok(found.into_iter().map(|(_, known)| known).collect())
    }

    /// The investigations that led to one NHTSA recall campaign, oldest
    /// first, open or closed. Letters in the number may be in either case.
    pub fn investigations_for_campaign(
        &self,
        campaign: &str,
    ) -> Result<Vec<Investigation>, ShopError> {
        self.require(Dataset::Investigations)?;
        let campaign = campaign.trim().to_ascii_uppercase();
        let rows = self.query(INVESTIGATIONS_FOR_CAMPAIGN, &[campaign.as_str().into()])?;
        let mut found = Vec::with_capacity(rows.len());
        for row in &rows {
            found.push(self.investigation(row)?.1);
        }
        Ok(found)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_kind_is_the_letters_an_action_number_starts_with() {
        assert_eq!(kind_of("PE24009"), "PE");
        assert_eq!(kind_of("EA78A17"), "EA");
        assert_eq!(kind_of("C85007"), "C");
        assert_eq!(kind_of("pe24009"), "PE");
        assert_eq!(kind_of("12345"), "");
        assert_eq!(kind_name("PE"), Some("Preliminary evaluation"));
        assert_eq!(kind_name("EA"), Some("Engineering analysis"));
        assert_eq!(kind_name("TA"), None);
        assert_eq!(kind_name(""), None);
        // Every kind said of a vehicle has a name.
        for kind in VEHICLE_KINDS {
            assert!(kind_name(kind).is_some(), "{kind}");
        }
    }

    #[test]
    fn the_better_match_wins() {
        assert_eq!(better("base", "exact"), "exact");
        assert_eq!(better("exact", "curated"), "exact");
        assert_eq!(better("curated", "base"), "base");
    }

    #[test]
    fn the_sentence_says_an_investigation_is_not_a_finding() {
        assert!(NOT_A_FINDING.contains("not a finding that a defect exists"));
    }
}
