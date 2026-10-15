use chrono::NaiveDate;
use payroll_calc::{
    Frequency, NOTE, PDOC_URL, default_pay_date, editions, factors, note_parts, ordinal,
};

fn day(text: &str) -> NaiveDate {
    NaiveDate::parse_from_str(text, "%Y-%m-%d").unwrap()
}

#[test]
fn the_four_frequencies_have_their_names_and_periods() {
    let named: Vec<(&str, &str, u32)> = Frequency::ALL
        .iter()
        .map(|frequency| (frequency.slug(), frequency.label(), frequency.periods()))
        .collect();
    assert_eq!(
        named,
        [
            ("weekly", "Weekly", 52),
            ("biweekly", "Biweekly", 26),
            ("semi-monthly", "Semi-monthly", 24),
            ("monthly", "Monthly", 12)
        ]
    );
    assert_eq!(Frequency::parse(" BiWeekly "), Some(Frequency::Biweekly));
    assert_eq!(
        Frequency::parse("semi-monthly"),
        Some(Frequency::SemiMonthly)
    );
    assert_eq!(Frequency::parse("fortnightly"), None);
    assert_eq!(Frequency::parse(""), None);
}

#[test]
fn every_edition_the_engine_holds_has_its_address_and_its_name() {
    let held = editions();
    assert_eq!(
        held.len(),
        payroll_calc::payroll_ca::rates::editions().len()
    );
    for edition in &held {
        assert!(
            edition
                .url
                .is_some_and(|url| url.starts_with("https://www.canada.ca/")),
            "{}",
            edition.number
        );
        assert!(edition.effective <= edition.valid_until);
    }
    let first = &held[0];
    assert_eq!(first.number, 123);
    assert_eq!(
        first.title(),
        "T4127 Payroll Deductions Formulas, 123rd edition"
    );
    assert_eq!(first.span(), "for pay dates from 2026-07-01 to 2026-12-31");
    assert_eq!(
        first.url,
        Some("https://www.canada.ca/content/dam/cra-arc/formspubs/pub/t4127-jul/t4127-jul-26e.pdf")
    );
    assert_eq!(
        [1, 2, 3, 4, 11, 12, 13, 21, 22, 101, 111, 123, 124].map(ordinal),
        [
            "1st", "2nd", "3rd", "4th", "11th", "12th", "13th", "21st", "22nd", "101st", "111th",
            "123rd", "124th"
        ]
    );
}

#[test]
fn the_default_pay_date_is_today_moved_into_the_tables_held() {
    // Inside: today, not moved.
    assert_eq!(
        default_pay_date(day("2026-10-14")),
        (day("2026-10-14"), false)
    );
    assert_eq!(
        default_pay_date(day("2026-07-01")),
        (day("2026-07-01"), false)
    );
    // Before the first day held, and after the last: the nearest day held.
    assert_eq!(
        default_pay_date(day("2026-01-05")),
        (day("2026-07-01"), true)
    );
    assert_eq!(
        default_pay_date(day("2027-03-01")),
        (day("2026-12-31"), true)
    );
}

#[test]
fn the_note_says_estimate_and_liable_and_links_pdoc() {
    assert_eq!(
        NOTE,
        "An estimate. The employer remains liable to CRA for the amounts it deducts and remits. Check a result with CRA's Payroll Deductions Online Calculator (PDOC) before relying on it."
    );
    let (before, link, after) = note_parts();
    assert_eq!(format!("{before}{link}{after}"), NOTE);
    assert_eq!(link, "Payroll Deductions Online Calculator (PDOC)");
    assert_eq!(
        PDOC_URL,
        "https://www.canada.ca/en/revenue-agency/services/e-services/digital-services-businesses/payroll-deductions-online-calculator.html"
    );
}

#[test]
fn every_factor_the_engine_names_has_a_meaning_of_one_sentence() {
    for name in [
        "P", "PM", "C", "C2", "EI", "F5", "F5A", "A", "R", "K", "K1", "K2", "K4", "T3", "T1", "V",
        "KP", "K1P", "K2P", "K4P", "K5P", "T4", "V1", "V2", "S", "T2", "L", "T",
    ] {
        let meaning = factors::meaning(name).unwrap_or_else(|| panic!("{name} has no meaning"));
        assert!(
            meaning.ends_with('.') && !meaning.contains(". "),
            "{name}: {meaning}"
        );
        assert!(meaning.len() <= 200, "{name}");
    }
    assert_eq!(factors::meaning("ZZ"), None);
}

#[test]
fn no_published_crate_depends_on_the_payroll_engine() {
    for manifest in [
        include_str!("../../wenmar-vin/Cargo.toml"),
        include_str!("../../wenmar-vehicles/Cargo.toml"),
        include_str!("../../wenmar-open-db/Cargo.toml"),
    ] {
        assert!(!manifest.contains("payroll"), "{manifest}");
    }
}
