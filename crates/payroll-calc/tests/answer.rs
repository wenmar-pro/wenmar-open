use chrono::NaiveDate;
use payroll_calc::{NOTE, PDOC_URL, Query, answer, deductions};
use serde_json::{Value, json};

fn today() -> NaiveDate {
    NaiveDate::from_ymd_opt(2026, 10, 14).unwrap()
}

#[test]
fn a_query_is_read_from_json_with_numbers_or_text_and_unknown_fields_ignored() {
    let query: Query = serde_json::from_value(json!({
        "frequency": "biweekly", "gross": 2400, "federal_claim": "16,452.00",
        "pay_date": "2026-07-15", "ytd_ei": 0.5, "ytd_cpp": null, "something": true
    }))
    .unwrap();
    assert_eq!(query.text("frequency"), "biweekly");
    assert_eq!(query.text("gross"), "2400");
    assert_eq!(query.text("federal_claim"), "16,452.00");
    assert_eq!(query.text("ytd_ei"), "0.5");
    assert_eq!(query.text("ytd_cpp"), "");
    assert!(serde_json::from_value::<Query>(json!({ "gross": [1] })).is_err());
}

#[test]
fn the_answer_of_a_pdoc_row_has_every_part_as_numbers() {
    // PDOC, 2026-10-14: Ontario, biweekly 2400.00, paid 2026-07-15, claim code 1.
    let query: Query = serde_json::from_value(
        json!({ "frequency": "biweekly", "gross": "2400.00", "pay_date": "2026-07-15" }),
    )
    .unwrap();
    let value: Value = serde_json::to_value(answer(&deductions(&query, today()).unwrap())).unwrap();
    assert_eq!(value["province"], "ON");
    assert_eq!(value["frequency"], "biweekly");
    assert_eq!(value["periods_per_year"], 26);
    assert_eq!(value["pay_date"], "2026-07-15");
    assert_eq!(value["pay_date_note"], Value::Null);
    assert_eq!(value["gross"], 2400.0);
    assert_eq!(
        value["federal_claim"],
        json!({ "amount": 16452.0, "basic": true })
    );
    assert_eq!(
        value["provincial_claim"],
        json!({ "amount": 12989.0, "basic": true })
    );
    assert_eq!(
        value["year_to_date"],
        json!({ "cpp": 0.0, "cpp2": 0.0, "ei": 0.0, "pensionable": 0.0 })
    );
    assert_eq!(
        value["employee"],
        json!({ "federal_tax": 223.2, "provincial_tax": 122.74, "total_tax": 345.94, "cpp": 134.79, "cpp2": 0.0, "ei": 39.12, "total": 519.85 })
    );
    assert_eq!(value["net_pay"], 1880.15);
    assert_eq!(
        value["employer"],
        json!({ "cpp": 134.79, "cpp2": 0.0, "ei": 54.77, "total": 189.56 })
    );
    let factors = value["factors"].as_array().unwrap();
    assert!(!factors.is_empty());
    for factor in factors {
        assert!(
            factor["name"].is_string()
                && factor["value"].is_number()
                && factor["meaning"].is_string(),
            "{factor}"
        );
    }
    assert_eq!(
        value["edition"],
        json!({ "number": 123, "title": "T4127 Payroll Deductions Formulas, 123rd edition", "effective": "2026-07-01", "valid_until": "2026-12-31", "url": "https://www.canada.ca/content/dam/cra-arc/formspubs/pub/t4127-jul/t4127-jul-26e.pdf" })
    );
    assert_eq!(value["note"], NOTE);
    assert_eq!(value["pdoc_url"], PDOC_URL);
    let keys: Vec<&String> = value.as_object().unwrap().keys().collect();
    assert_eq!(keys.len(), 17, "{keys:?}");
}
