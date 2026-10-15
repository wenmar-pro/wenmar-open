//! `GET /v1/payroll/deductions`. The figures are PDOC's, captured
//! 2026-10-14 for Ontario, pay date 2026-07-15.

use axum::http::StatusCode;
use serde_json::json;

use crate::common;

#[tokio::test]
async fn a_pdoc_row_is_answered_with_its_figures() {
    let app = common::app().await;
    for (query, employee) in [
        (
            "frequency=weekly&gross=800.00&pay_date=2026-07-15",
            json!({ "federal_tax": 55.74, "provincial_tax": 33.58, "total_tax": 89.32, "cpp": 43.6, "cpp2": 0.0, "ei": 13.04, "total": 145.96 }),
        ),
        (
            "frequency=biweekly&gross=2400.00&pay_date=2026-07-15",
            json!({ "federal_tax": 223.2, "provincial_tax": 122.74, "total_tax": 345.94, "cpp": 134.79, "cpp2": 0.0, "ei": 39.12, "total": 519.85 }),
        ),
        (
            "frequency=weekly&gross=800&federal_claim=0&provincial_claim=0&pay_date=2026-07-15",
            json!({ "federal_tax": 100.03, "provincial_tax": 46.19, "total_tax": 146.22, "cpp": 43.6, "cpp2": 0.0, "ei": 13.04, "total": 202.86 }),
        ),
    ] {
        let (status, body) = app.json(&format!("/v1/payroll/deductions?{query}")).await;
        assert_eq!(status, StatusCode::OK, "{query}: {body}");
        assert_eq!(body["employee"], employee, "{query}");
        assert_eq!(body["note"], payroll_calc::NOTE);
        assert_eq!(body["pdoc_url"], payroll_calc::PDOC_URL);
    }
}

#[tokio::test]
async fn what_cannot_be_read_is_validation_failed_naming_the_field() {
    let app = common::app().await;
    for (query, field, message) in [
        (
            "gross=800",
            "frequency",
            "Choose weekly, biweekly, semi-monthly or monthly.",
        ),
        ("frequency=weekly", "gross", "Enter a number."),
        (
            "frequency=weekly&gross=800&pay_date=2027-01-15",
            "pay_date",
            "No tables are held for 2027-01-15. This calculator holds T4127's 123rd edition, for pay dates from 2026-07-01 to 2026-12-31.",
        ),
        (
            "frequency=weekly&gross=800&province=BC",
            "province",
            "Only Ontario is covered for now.",
        ),
    ] {
        let (status, body) = app.json(&format!("/v1/payroll/deductions?{query}")).await;
        assert_eq!(status, StatusCode::BAD_REQUEST, "{query}");
        assert_eq!(
            body,
            json!({ "error": { "code": "validation_failed", "message": message, "details": { "field": field } } }),
            "{query}"
        );
    }
}
