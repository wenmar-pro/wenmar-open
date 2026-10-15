//! Rows of CRA's Payroll Deductions Online Calculator (PDOC), captured by
//! hand on 2026-10-14 for Ontario, pay date 2026-07-15 (payroll-ca
//! cases/pdoc-123.csv). An empty claim is claim code 1.

use chrono::NaiveDate;
use payroll_calc::{Query, deductions, money};

fn query(frequency: &str, gross: &str, claims: Option<&str>) -> Query {
    Query {
        frequency: Some(frequency.to_owned()),
        gross: Some(gross.to_owned()),
        federal_claim: claims.map(str::to_owned),
        provincial_claim: claims.map(str::to_owned),
        pay_date: Some("2026-07-15".to_owned()),
        ..Query::default()
    }
}

/// federal, Ontario, CPP, CPP2, EI, total, net pay; employer EI, employer total.
fn shown(worked: &payroll_calc::Worked) -> [String; 9] {
    let d = &worked.deductions;
    [
        d.federal_tax,
        d.provincial_tax,
        d.cpp,
        d.cpp2,
        d.ei,
        worked.employee_total,
        worked.net_pay,
        d.employer_ei,
        worked.employer_total,
    ]
    .map(|amount| money(amount).to_string())
}

#[test]
fn pdoc_rows_come_out_to_the_cent() {
    let today = NaiveDate::from_ymd_opt(2026, 10, 14).unwrap();
    for (asked, expected) in [
        (
            query("weekly", "800.00", None),
            [
                "$55.74", "$33.58", "$43.60", "$0.00", "$13.04", "$145.96", "$654.04", "$18.26",
                "$61.86",
            ],
        ),
        (
            query("biweekly", "2400.00", None),
            [
                "$223.20",
                "$122.74",
                "$134.79",
                "$0.00",
                "$39.12",
                "$519.85",
                "$1,880.15",
                "$54.77",
                "$189.56",
            ],
        ),
        (
            query("semi-monthly", "3000.00", None),
            [
                "$319.59",
                "$167.98",
                "$169.82",
                "$0.00",
                "$48.90",
                "$706.29",
                "$2,293.71",
                "$68.46",
                "$238.28",
            ],
        ),
        // Claim code 0: both claims typed as 0.
        (
            query("weekly", "800.00", Some("0")),
            [
                "$100.03", "$46.19", "$43.60", "$0.00", "$13.04", "$202.86", "$597.14", "$18.26",
                "$61.86",
            ],
        ),
    ] {
        let worked = deductions(&asked, today).unwrap();
        assert_eq!(shown(&worked), expected.map(str::to_owned), "{asked:?}");
        assert!(!worked.deductions.factors.is_empty());
    }
}

#[test]
fn an_empty_claim_is_the_basic_amount_of_the_tables_and_is_said_to_be() {
    let today = NaiveDate::from_ymd_opt(2026, 10, 14).unwrap();
    let worked = deductions(&query("biweekly", "2400.00", None), today).unwrap();
    // The amounts PDOC was given for claim code 1 in this edition.
    assert_eq!(worked.input.federal_claim.amount.to_string(), "$16,452.00");
    assert_eq!(
        worked.input.provincial_claim.amount.to_string(),
        "$12,989.00"
    );
    assert!(worked.input.federal_claim.basic && worked.input.provincial_claim.basic);
    let typed = deductions(&query("weekly", "800.00", Some("0")), today).unwrap();
    assert!(!typed.input.federal_claim.basic);
    assert_eq!(typed.edition.number, 123);
}
