//! `wenmar-open payroll deductions`, worked out on the computer. The
//! figures are PDOC's, captured 2026-10-14, Ontario, paid 2026-07-15.

use serde_json::json;

use crate::common;

#[test]
fn a_pdoc_row_is_worked_out_with_no_data_file_and_no_network() {
    let fixture = common::empty_dir();
    let env = common::env(&fixture);
    for offline in [false, true] {
        let mut args = vec![
            "payroll",
            "deductions",
            "--frequency",
            "biweekly",
            "--gross",
            "2400.00",
            "--pay-date",
            "2026-07-15",
        ];
        if offline {
            args.push("--offline");
        }
        let run = common::run(&env, &args);
        assert_eq!(run.code, 0, "{}", run.stderr);
        let answer = run.json();
        assert_eq!(
            answer["employee"],
            json!({ "federal_tax": 223.2, "provincial_tax": 122.74, "total_tax": 345.94, "cpp": 134.79, "cpp2": 0.0, "ei": 39.12, "total": 519.85 })
        );
        assert_eq!(answer["net_pay"], 1880.15);
        assert_eq!(answer["note"], payroll_calc::NOTE);
    }
    let run = common::run(
        &env,
        &[
            "payroll",
            "deductions",
            "--frequency",
            "weekly",
            "--gross",
            "800",
            "--federal-claim",
            "0",
            "--provincial-claim",
            "0",
            "--pay-date",
            "2026-07-15",
        ],
    );
    assert_eq!(run.json()["employee"]["federal_tax"], 100.03);
    assert_eq!(run.json()["employee"]["provincial_tax"], 46.19);
}

#[test]
fn what_cannot_be_read_is_validation_failed_naming_the_field() {
    let fixture = common::empty_dir();
    let run = common::run(
        &common::env(&fixture),
        &[
            "payroll",
            "deductions",
            "--frequency",
            "weekly",
            "--gross",
            "800",
            "--pay-date",
            "2027-01-15",
        ],
    );
    assert_ne!(run.code, 0);
    let error = &run.error()["error"];
    assert_eq!(error["code"], "validation_failed");
    assert_eq!(error["details"]["field"], "pay_date");
    assert!(
        error["message"]
            .as_str()
            .unwrap()
            .starts_with("No tables are held for 2027-01-15.")
    );
}
