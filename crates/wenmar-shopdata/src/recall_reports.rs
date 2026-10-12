//! How far a recall's remedy has got: the newest report the manufacturer
//! filed with NHTSA for a campaign.
//!
//! The figures are totals for the whole campaign at the end of a period
//! that is often years past. They say nothing of one vehicle: only the
//! manufacturer can say whether a VIN has been remedied.

use serde::Serialize;
use wenmar_vehicles::{Source, Value};

use crate::dataset::Dataset;
use crate::error::ShopError;
use crate::shop::ShopData;

/// The newest report of one NHTSA campaign.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct RecallReport {
    /// NHTSA's campaign number, such as `21V983000`.
    pub campaign: String,
    /// `quarterly` or `annual`.
    pub kind: String,
    /// The report's number among those of its kind, from 1.
    pub report_number: u32,
    /// The first and the last quarter the report covers, each as `2026-1`.
    /// A quarterly report has the same quarter in both.
    pub period_from: String,
    pub period_to: String,
    /// The day the report was filed, as `2026-04-13`.
    pub submitted: Option<String>,
    /// Vehicles in the campaign.
    pub involved: u32,
    /// Of those, remedied by the end of the period.
    pub remedied: u32,
    /// NHTSA's "total unreachable" and "total removed", as the
    /// manufacturer reported them. NHTSA's description of the file says no
    /// more of either than its name.
    pub unreachable: u32,
    pub removed: u32,
}

impl RecallReport {
    /// `remedied` as a share of `involved`, in whole percent, rounded
    /// down, and never more than 100. `None` when `involved` is 0.
    pub fn remedied_percent(&self) -> Option<u8> {
        if self.involved == 0 {
            return None;
        }
        let percent = u64::from(self.remedied) * 100 / u64::from(self.involved);
        Some(u8::try_from(percent.min(100)).unwrap_or(100))
    }
}

/// The newest report of one campaign. `?1` the campaign number.
pub const RECALL_REPORT: &str = "
SELECT campaign, kind, report_number, period_from, period_to, submitted, involved, remedied, unreachable, removed
FROM recall_report
WHERE campaign = ?1";

const TABLE: &str = "recall_report";

fn text(row: &[Value], column: usize) -> Result<String, ShopError> {
    row.get(column)
        .and_then(Value::text)
        .map(str::to_owned)
        .ok_or(ShopError::Shape(TABLE))
}

fn number(row: &[Value], column: usize) -> Result<u32, ShopError> {
    row.get(column)
        .and_then(Value::integer)
        .and_then(|number| u32::try_from(number).ok())
        .ok_or(ShopError::Shape(TABLE))
}

impl<S: Source> ShopData<S> {
    /// The day the reports were current, as `2026-10-12`. `None` when the
    /// file does not hold them.
    pub fn recall_reports_as_of(&self) -> Option<&str> {
        self.datasets()
            .get(Dataset::RecallReports)
            .map(|entry| entry.as_of.as_str())
    }

    /// The newest report of one NHTSA campaign. `None` when the campaign
    /// has none. Letters in the number may be in either case.
    pub fn recall_report(&self, campaign: &str) -> Result<Option<RecallReport>, ShopError> {
        self.require(Dataset::RecallReports)?;
        let campaign = campaign.trim().to_ascii_uppercase();
        let rows = self.query(RECALL_REPORT, &[campaign.as_str().into()])?;
        let Some(row) = rows.first() else {
            return Ok(None);
        };
        Ok(Some(RecallReport {
            campaign: text(row, 0)?,
            kind: text(row, 1)?,
            report_number: number(row, 2)?,
            period_from: text(row, 3)?,
            period_to: text(row, 4)?,
            submitted: row.get(5).and_then(Value::text).map(str::to_owned),
            involved: number(row, 6)?,
            remedied: number(row, 7)?,
            unreachable: number(row, 8)?,
            removed: number(row, 9)?,
        }))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn report(involved: u32, remedied: u32) -> RecallReport {
        RecallReport {
            campaign: "21V983000".to_owned(),
            kind: "quarterly".to_owned(),
            report_number: 1,
            period_from: "2026-1".to_owned(),
            period_to: "2026-1".to_owned(),
            submitted: None,
            involved,
            remedied,
            unreachable: 0,
            removed: 0,
        }
    }

    #[test]
    fn the_share_is_whole_percent_rounded_down() {
        assert_eq!(report(57_591, 38_917).remedied_percent(), Some(67));
        assert_eq!(report(1_000, 999).remedied_percent(), Some(99));
        assert_eq!(report(1_000, 1_000).remedied_percent(), Some(100));
        assert_eq!(report(1_000, 0).remedied_percent(), Some(0));
        assert_eq!(report(3, 2).remedied_percent(), Some(66));
        // More remedied than involved is a filing error, not 110%.
        assert_eq!(report(100, 110).remedied_percent(), Some(100));
        assert_eq!(report(0, 22).remedied_percent(), None);
        assert_eq!(report(u32::MAX, u32::MAX).remedied_percent(), Some(100));
    }
}
