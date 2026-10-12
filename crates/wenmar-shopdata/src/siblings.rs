//! Models that share recall campaigns and bulletins with a model year.
//!
//! Derived when the shop file is built, from the documents that list both.
//! It says the two have documents in common. It does not say a part fits
//! both, and a page must say so.

use serde::Serialize;
use wenmar_vehicles::{Source, Value};

use crate::error::ShopError;
use crate::shop::ShopData;

/// Most models a model year's page names as sharing documents.
pub const MOST_SIBLINGS: usize = 5;

const TABLE: &str = "vehicle_sibling";

/// One model that shares documents with a model year.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Sibling {
    pub model_id: i64,
    /// Documents that list both models for the model year.
    pub shared: u32,
    /// Documents that list this model for the model year, among those
    /// that list four models or fewer.
    pub total: u32,
}

fn integer(row: &[Value], column: usize) -> Result<i64, ShopError> {
    row.get(column)
        .and_then(Value::integer)
        .ok_or(ShopError::Shape(TABLE))
}

fn sibling(row: &[Value]) -> Result<Sibling, ShopError> {
    Ok(Sibling {
        model_id: integer(row, 0)?,
        shared: u32::try_from(integer(row, 1)?).map_err(|_| ShopError::Shape(TABLE))?,
        total: u32::try_from(integer(row, 2)?).map_err(|_| ShopError::Shape(TABLE))?,
    })
}

impl<S: Source> ShopData<S> {
    /// The models that share documents with a model year, most shared
    /// first, then by model id. Empty when the file was built without
    /// recalls or bulletins.
    pub fn siblings(&self, model_id: i64, year: u16) -> Result<Vec<Sibling>, ShopError> {
        if !self.has_table(TABLE)? {
            return Ok(Vec::new());
        }
        let limit = i64::try_from(MOST_SIBLINGS).unwrap_or(5);
        self.query(
            "SELECT sibling_model_id, shared, total FROM vehicle_sibling
             WHERE model_id = ?1 AND year = ?2
             ORDER BY shared DESC, sibling_model_id LIMIT ?3",
            &[model_id.into(), i64::from(year).into(), limit.into()],
        )?
        .iter()
        .map(|row| sibling(row))
        .collect()
    }
}
