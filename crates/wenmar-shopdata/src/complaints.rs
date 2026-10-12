//! Owner complaints to NHTSA, as counts by model year and component.
//!
//! A complaint is what an owner reported. NHTSA does not verify it, and
//! this file holds no complaint: only how many there are. Nothing here
//! says a vehicle has a defect.

use serde::Serialize;
use wenmar_vehicles::{Source, Value};

use crate::dataset::Dataset;
use crate::error::ShopError;
use crate::shop::ShopData;

/// A component with fewer complaints than this is not listed: one
/// owner's report is not what owners report.
pub const FEWEST_SHOWN: u32 = 5;

/// A median over fewer mileages than this is not given.
pub const FEWEST_FOR_MEDIAN: u32 = 5;

/// Most components listed for a model year.
pub const MOST_COMPONENTS: usize = 12;

/// What every page and answer about complaints says.
pub const NOTE: &str = "These are counts of the complaints owners filed with NHTSA about this model year. A complaint is an owner's report: it is not verified by NHTSA or by this site, and it is not a finding that anything is wrong with the vehicle. A count is not a rate: a model that sold more has more complaints. A complaint may name several components, so the components do not add up to the total.";

/// Where an owner files a complaint with NHTSA.
pub const NHTSA_COMPLAINTS: &str = "https://www.nhtsa.gov/report-a-safety-problem";

/// Miles as kilometres, to the nearest one.
pub fn km(miles: u32) -> u32 {
    ((u64::from(miles) * 1_609_344 + 500_000) / 1_000_000) as u32
}

/// The counts of one model year, or of one component of it.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
pub struct ComplaintFigures {
    pub complaints: u32,
    /// Complaints that report a crash.
    pub crashes: u32,
    /// Complaints that report a fire.
    pub fires: u32,
    /// Complaints that report at least one person injured.
    pub injuries: u32,
    /// The median of the mileages owners gave, in kilometres and in
    /// miles. `None` when fewer than [`FEWEST_FOR_MEDIAN`] gave one.
    pub median_km: Option<u32>,
    pub median_miles: Option<u32>,
}

/// The complaints of a model year that name one component.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ComponentComplaints {
    /// An id of `data/components.yaml`.
    pub id: String,
    pub name: String,
    /// The system a part belongs to. `None` for a system.
    pub parent: Option<String>,
    #[serde(flatten)]
    pub figures: ComplaintFigures,
}

/// The complaints of one model year.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct VehicleComplaintCounts {
    pub total: ComplaintFigures,
    /// Components with at least [`FEWEST_SHOWN`] complaints, most first.
    pub components: Vec<ComponentComplaints>,
}

/// Reads the six count columns that start at `first`.
fn figures(row: &[Value], first: usize) -> Result<ComplaintFigures, ShopError> {
    let number = |column: usize| {
        row.get(column)
            .and_then(Value::integer)
            .and_then(|value| u32::try_from(value).ok())
            .ok_or(ShopError::Shape("complaint_count"))
    };
    let with_miles = number(first + 4)?;
    let median_miles = row
        .get(first + 5)
        .and_then(Value::integer)
        .and_then(|value| u32::try_from(value).ok())
        .filter(|_| with_miles >= FEWEST_FOR_MEDIAN);
    Ok(ComplaintFigures {
        complaints: number(first)?,
        crashes: number(first + 1)?,
        fires: number(first + 2)?,
        injuries: number(first + 3)?,
        median_km: median_miles.map(km),
        median_miles,
    })
}

impl<S: Source> ShopData<S> {
    /// The day the complaints were current, as `2026-10-11`.
    pub fn complaints_as_of(&self) -> Option<&str> {
        self.datasets()
            .get(Dataset::Complaints)
            .map(|entry| entry.as_of.as_str())
    }

    /// What owners reported about one model year: the total, and each
    /// component with at least [`FEWEST_SHOWN`] complaints, most first.
    /// All zeros for a model year with none.
    pub fn complaints_for_vehicle(
        &self,
        model_id: i64,
        year: u16,
    ) -> Result<VehicleComplaintCounts, ShopError> {
        self.require(Dataset::Complaints)?;
        let params = [model_id.into(), i64::from(year).into()];
        let total = self.query(
            "SELECT complaints, crashes, fires, injuries, with_miles, median_miles
             FROM complaint_count WHERE model_id = ?1 AND year = ?2 AND component_id = ''",
            &params,
        )?;
        let Some(row) = total.first() else {
            return Ok(VehicleComplaintCounts::default());
        };
        let mut found = VehicleComplaintCounts {
            total: figures(row, 0)?,
            components: Vec::new(),
        };
        let rows = self.query(
            &format!(
                "SELECT k.id, k.name, k.parent, c.complaints, c.crashes, c.fires, c.injuries, c.with_miles, c.median_miles
                 FROM complaint_count c JOIN component k ON k.id = c.component_id
                 WHERE c.model_id = ?1 AND c.year = ?2 AND c.complaints >= {FEWEST_SHOWN}
                 ORDER BY c.complaints DESC, k.name LIMIT {MOST_COMPONENTS}"
            ),
            &params,
        )?;
        for row in &rows {
            let text = |column: usize| {
                row.get(column)
                    .and_then(Value::text)
                    .map(str::to_owned)
                    .ok_or(ShopError::Shape("component"))
            };
            found.components.push(ComponentComplaints {
                id: text(0)?,
                name: text(1)?,
                parent: row.get(2).and_then(Value::text).map(str::to_owned),
                figures: figures(row, 3)?,
            });
        }
        Ok(found)
    }
}
