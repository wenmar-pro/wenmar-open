//! Ontario payroll deductions, worked out on this computer by the same
//! code as the hosted API.

use serde_json::Value;

use crate::error::{CliError, VALIDATION_FAILED};

/// The answer the API would give, or its error.
pub fn local(query: &payroll_calc::Query) -> Result<Value, CliError> {
    let worked =
        payroll_calc::deductions(query, payroll_calc::today()).map_err(
            |refusals| match refusals.into_iter().next() {
                Some(refusal) if !refusal.field.is_empty() => {
                    CliError::validation(refusal.field, refusal.message)
                }
                Some(refusal) => CliError::new(VALIDATION_FAILED, refusal.message),
                None => CliError::new(VALIDATION_FAILED, "The pay could not be worked out."),
            },
        )?;
    serde_json::to_value(payroll_calc::answer(&worked)).map_err(|error| {
        CliError::new(
            crate::error::IO,
            format!("the answer could not be written: {error}"),
        )
    })
}
