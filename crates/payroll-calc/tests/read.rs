use chrono::NaiveDate;
use payroll_calc::{FIELDS, Query, Refusal, deductions};

fn today() -> NaiveDate {
    NaiveDate::from_ymd_opt(2026, 10, 14).unwrap()
}

fn refused(query: Query) -> Vec<(String, String)> {
    deductions(&query, today())
        .unwrap_err()
        .into_iter()
        .map(|Refusal { field, message }| (field.to_owned(), message))
        .collect()
}

fn pay(pairs: &[(&str, &str)]) -> Query {
    let mut query = Query::default();
    for (name, text) in pairs {
        query.set(name, text);
    }
    query
}

#[test]
fn the_fields_are_named_in_the_order_of_the_form() {
    assert_eq!(
        FIELDS,
        [
            "province",
            "frequency",
            "gross",
            "federal_claim",
            "provincial_claim",
            "pay_date",
            "ytd_cpp",
            "ytd_cpp2",
            "ytd_ei",
            "ytd_pensionable"
        ]
    );
    let query = pay(&[("gross", "1"), ("nothing", "2")]);
    assert_eq!(query.text("gross"), "1");
    assert_eq!(query.text("frequency"), "");
}

#[test]
fn what_cannot_be_read_is_refused_at_its_field_and_every_field_is_read() {
    assert_eq!(
        refused(pay(&[
            ("frequency", "fortnightly"),
            ("gross", "abc"),
            ("pay_date", "15/07/2026"),
            ("ytd_ei", "-5")
        ])),
        [
            (
                "frequency".to_owned(),
                "Choose weekly, biweekly, semi-monthly or monthly.".to_owned()
            ),
            (
                "gross".to_owned(),
                "This is not a number. Write it like 1,234.50.".to_owned()
            ),
            (
                "pay_date".to_owned(),
                "Write the date like 2026-07-15.".to_owned()
            ),
            ("ytd_ei".to_owned(), "This cannot be below zero.".to_owned()),
        ]
    );
    assert_eq!(
        refused(pay(&[("gross", "800")])),
        [(
            "frequency".to_owned(),
            "Choose weekly, biweekly, semi-monthly or monthly.".to_owned()
        )]
    );
    assert_eq!(
        refused(pay(&[("frequency", "weekly")])),
        [("gross".to_owned(), "Enter a number.".to_owned())]
    );
    assert_eq!(
        refused(pay(&[
            ("province", "QC"),
            ("frequency", "weekly"),
            ("gross", "800")
        ])),
        [(
            "province".to_owned(),
            "Only Ontario is covered for now.".to_owned()
        )]
    );
}

#[test]
fn a_pay_date_outside_the_tables_is_refused_with_their_dates() {
    for date in ["2027-01-15", "2026-06-30"] {
        assert_eq!(
            refused(pay(&[
                ("frequency", "weekly"),
                ("gross", "800"),
                ("pay_date", date)
            ])),
            [(
                "pay_date".to_owned(),
                format!(
                    "No tables are held for {date}. This calculator holds T4127's 123rd edition, for pay dates from 2026-07-01 to 2026-12-31."
                )
            )]
        );
    }
}

#[test]
fn an_empty_pay_date_is_today_and_a_moved_one_says_so() {
    let query = pay(&[
        ("frequency", "weekly"),
        ("gross", "800"),
        ("province", " on "),
    ]);
    let worked = deductions(&query, today()).unwrap();
    assert_eq!(worked.input.pay_date.to_string(), "2026-10-14");
    assert_eq!(worked.input.pay_date_note, None);
    let later = deductions(&query, NaiveDate::from_ymd_opt(2027, 2, 1).unwrap()).unwrap();
    assert_eq!(later.input.pay_date.to_string(), "2026-12-31");
    assert_eq!(
        later.input.pay_date_note.as_deref(),
        Some("Today is outside the tables held, so the pay date is 2026-12-31.")
    );
}

#[test]
fn year_to_date_figures_reach_the_engine() {
    // PDOC: biweekly 2400.00 after the year's CPP, CPP2 and EI maxima.
    let worked = deductions(
        &pay(&[
            ("frequency", "biweekly"),
            ("gross", "2400.00"),
            ("pay_date", "2026-07-15"),
            ("ytd_cpp", "4230.45"),
            ("ytd_cpp2", "416.00"),
            ("ytd_ei", "1123.07"),
            ("ytd_pensionable", "85000.00"),
        ]),
        today(),
    )
    .unwrap();
    let d = &worked.deductions;
    assert_eq!(
        [d.federal_tax, d.provincial_tax, d.cpp, d.cpp2, d.ei]
            .map(|a| payroll_calc::money(a).to_string()),
        ["$224.02", "$123.43", "$0.00", "$0.00", "$0.00"]
    );
}
