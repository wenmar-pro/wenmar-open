//! The index of the letters manufacturers file with NHTSA under 49 CFR
//! Part 565 to explain how their VINs are built.
//!
//! The letters are the manufacturers' own documents. NHTSA keeps them, and
//! a shop file holds only NHTSA's index of them: who filed, when, for which
//! model years, and the address of NHTSA's copy. No letter is ever stored.

use serde::Serialize;
use wenmar_vehicles::{Source, Value};

use crate::dataset::Dataset;
use crate::error::ShopError;
use crate::shop::ShopData;

/// Most filings one list holds, whatever `limit` says.
pub const MOST_VIN_GUIDES: usize = 100;

/// What every stored address starts with: NHTSA's copy of a filing. The
/// rest of the address is the document's id, 36 characters.
pub const DOCUMENTS: &str = "https://vpic.nhtsa.dot.gov/mid/home/displayfile/";

/// Whether `url` is the address of NHTSA's copy of a filing: [`DOCUMENTS`]
/// then 36 lowercase hexadecimal digits and hyphens.
pub fn is_document_url(url: &str) -> bool {
    url.strip_prefix(DOCUMENTS).is_some_and(|id| {
        id.len() == 36
            && id
                .bytes()
                .all(|byte| matches!(byte, b'0'..=b'9' | b'a'..=b'f' | b'-'))
    })
}

/// One filing.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct VinGuide {
    /// NHTSA's name for the document, such as `ORG26610`.
    pub name: String,
    /// The manufacturer that filed it, as NHTSA writes the name.
    pub manufacturer: String,
    /// The kind of filing: `565`.
    pub kind: String,
    /// The date on the letter, as `2026-06-01`.
    pub letter_date: String,
    /// The model years the letter covers, where NHTSA's index gives them.
    pub year_from: Option<u16>,
    pub year_to: Option<u16>,
    /// NHTSA's copy of the document.
    pub url: String,
}

/// The newest filings that reach a make or a manufacturer code.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
pub struct VinGuideList {
    /// How many filings reach it; `guides` holds the newest of them.
    pub total: u32,
    pub guides: Vec<VinGuide>,
}

/// The newest filings of the manufacturers that build a make. `?1` make
/// id, `?2` the limit.
pub const VIN_GUIDES_FOR_MAKE: &str = "
SELECT g.name, g.manufacturer, g.kind, g.letter_date, g.year_from, g.year_to, g.url
FROM vin_guide_make m
JOIN vin_guide g ON g.manufacturer_id = m.manufacturer_id
WHERE m.make_id = ?1
ORDER BY g.letter_date DESC, g.id DESC
LIMIT ?2";

/// How many filings reach a make. `?1` make id.
pub const VIN_GUIDE_COUNT_FOR_MAKE: &str = "
SELECT COUNT(*)
FROM vin_guide_make m
JOIN vin_guide g ON g.manufacturer_id = m.manufacturer_id
WHERE m.make_id = ?1";

/// The newest filings of the manufacturer a code belongs to. `?1` the
/// code, `?2` the limit.
pub const VIN_GUIDES_FOR_WMI: &str = "
SELECT g.name, g.manufacturer, g.kind, g.letter_date, g.year_from, g.year_to, g.url
FROM vin_guide_wmi w
JOIN vin_guide g ON g.manufacturer_id = w.manufacturer_id
WHERE w.wmi = ?1
ORDER BY g.letter_date DESC, g.id DESC
LIMIT ?2";

/// How many filings reach a manufacturer code. `?1` the code.
pub const VIN_GUIDE_COUNT_FOR_WMI: &str = "
SELECT COUNT(*)
FROM vin_guide_wmi w
JOIN vin_guide g ON g.manufacturer_id = w.manufacturer_id
WHERE w.wmi = ?1";

const TABLE: &str = "vin_guide";

fn text(row: &[Value], column: usize) -> Result<String, ShopError> {
    row.get(column)
        .and_then(Value::text)
        .map(str::to_owned)
        .ok_or(ShopError::Shape(TABLE))
}

fn year(row: &[Value], column: usize) -> Option<u16> {
    row.get(column)
        .and_then(Value::integer)
        .and_then(|year| u16::try_from(year).ok())
}

impl<S: Source> ShopData<S> {
    fn vin_guide_list(
        &self,
        list: &str,
        count: &str,
        key: Value,
        limit: usize,
    ) -> Result<VinGuideList, ShopError> {
        self.require(Dataset::VinGuides)?;
        let limit = limit.clamp(1, MOST_VIN_GUIDES);
        let total = self
            .query(count, std::slice::from_ref(&key))?
            .first()
            .and_then(|row| row.first())
            .and_then(Value::integer)
            .and_then(|total| u32::try_from(total).ok())
            .ok_or(ShopError::Shape(TABLE))?;
        let rows = self.query(
            list,
            &[
                key,
                Value::Integer(i64::try_from(limit).unwrap_or(i64::MAX)),
            ],
        )?;
        let mut guides = Vec::with_capacity(rows.len());
        for row in &rows {
            guides.push(VinGuide {
                name: text(row, 0)?,
                manufacturer: text(row, 1)?,
                kind: text(row, 2)?,
                letter_date: text(row, 3)?,
                year_from: year(row, 4),
                year_to: year(row, 5),
                url: text(row, 6)?,
            });
        }
        Ok(VinGuideList { total, guides })
    }

    /// The newest filings of the manufacturers that build a make, newest
    /// first, at most `limit` (1 to [`MOST_VIN_GUIDES`]), and how many
    /// there are in all. A make no filing reaches gives an empty list.
    pub fn vin_guides_for_make(
        &self,
        make_id: i64,
        limit: usize,
    ) -> Result<VinGuideList, ShopError> {
        self.vin_guide_list(
            VIN_GUIDES_FOR_MAKE,
            VIN_GUIDE_COUNT_FOR_MAKE,
            make_id.into(),
            limit,
        )
    }

    /// The same for a manufacturer code, such as `JF1` or `1A9881`.
    pub fn vin_guides_for_wmi(&self, wmi: &str, limit: usize) -> Result<VinGuideList, ShopError> {
        self.vin_guide_list(
            VIN_GUIDES_FOR_WMI,
            VIN_GUIDE_COUNT_FOR_WMI,
            wmi.into(),
            limit,
        )
    }

    /// The day the index was fetched, as `2026-10-12`. `None` when the file
    /// does not hold it.
    pub fn vin_guides_as_of(&self) -> Option<&str> {
        self.datasets()
            .get(Dataset::VinGuides)
            .map(|entry| entry.as_of.as_str())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_nhtsas_copy_of_a_filing_is_a_document_address() {
        assert!(is_document_url(
            "https://vpic.nhtsa.dot.gov/mid/home/displayfile/29dfd65e-a57c-4da0-ac6e-78acd3d30f8b"
        ));
        for bad in [
            "",
            "http://vpic.nhtsa.dot.gov/mid/home/displayfile/29dfd65e-a57c-4da0-ac6e-78acd3d30f8b",
            "https://vpic.nhtsa.dot.gov/mid/home/displayfile/",
            "https://vpic.nhtsa.dot.gov/mid/home/displayfile/29dfd65e",
            "https://vpic.nhtsa.dot.gov/mid/home/displayfile/29DFD65E-A57C-4DA0-AC6E-78ACD3D30F8B",
            "https://vpic.nhtsa.dot.gov/mid/home/displayfile/29dfd65e-a57c-4da0-ac6e-78acd3d30f8b?x=1",
            "https://vpic.nhtsa.dot.gov/mid/home/displayfile/../../../x/aaaaaaaaaaaaaaaaaaaaaaaaaa",
            "https://example.com/mid/home/displayfile/29dfd65e-a57c-4da0-ac6e-78acd3d30f8b",
            "javascript:alert(1)",
        ] {
            assert!(!is_document_url(bad), "{bad}");
        }
    }
}
