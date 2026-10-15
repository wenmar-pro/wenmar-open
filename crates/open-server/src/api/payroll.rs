//! Ontario payroll deductions for one pay, by CRA's T4127 formulas. The
//! answer is worked out by `payroll_calc`, the same code the calculator
//! page, the MCP tools and the command-line tool use. It is an estimate:
//! the employer remains liable to CRA.

use axum::Json;
use axum::extract::Query;
use axum::extract::rejection::QueryRejection;
use payroll_calc::PayrollDeductions;
use serde_json::json;

use crate::api::bad_query;
use crate::error::{ApiError, ErrorBody};

/// The answer to a query, or the first refusal as `validation_failed`
/// with the field it is about. Shared with the MCP tool.
pub fn deductions_op(query: &payroll_calc::Query) -> Result<PayrollDeductions, ApiError> {
    match payroll_calc::deductions(query, payroll_calc::today()) {
        Ok(worked) => Ok(payroll_calc::answer(&worked)),
        Err(refusals) => {
            let first = refusals
                .into_iter()
                .next()
                .map(|refusal| (refusal.field, refusal.message));
            let (field, message) =
                first.unwrap_or(("", "The pay could not be worked out.".to_owned()));
            let details = if field.is_empty() {
                json!({})
            } else {
                json!({ "field": field })
            };
            Err(ApiError::Validation { message, details })
        }
    }
}

#[utoipa::path(
    get,
    path = "/payroll/deductions",
    tag = "Payroll",
    params(payroll_calc::Query),
    responses(
        (status = 200, description = "The deductions from the pay, the net pay, the employer's share and every T4127 factor used. An estimate.", body = PayrollDeductions),
        (status = 400, description = "`validation_failed`: a field cannot be read, the pay date is outside the tables held, or the province is not Ontario; `details.field` names the field.", body = ErrorBody)
    )
)]
pub async fn payroll_deductions(
    query: Result<Query<payroll_calc::Query>, QueryRejection>,
) -> Result<Json<PayrollDeductions>, ApiError> {
    let Query(query) = query.map_err(bad_query)?;
    Ok(Json(deductions_op(&query)?))
}
