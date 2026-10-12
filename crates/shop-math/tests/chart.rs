use shop_math::ParseError;
use shop_math::chart::{
    Place, celsius_from_fahrenheit, fahrenheit_from_celsius, kpa_from_psi, parse_tenths, place,
    psi_from_kpa, show_tenths,
};

#[test]
fn a_reading_is_placed_among_the_rows() {
    let rows = [-100, 0, 100, 250];
    assert_eq!(place(&rows, -101), Some(Place::Below));
    assert_eq!(place(&rows, -100), Some(Place::At(0)));
    assert_eq!(place(&rows, -1), Some(Place::Between(0, 1)));
    assert_eq!(place(&rows, 0), Some(Place::At(1)));
    assert_eq!(place(&rows, 249), Some(Place::Between(2, 3)));
    assert_eq!(place(&rows, 250), Some(Place::At(3)));
    assert_eq!(place(&rows, 251), Some(Place::Above));
    assert_eq!(place(&[], 0), None);
    assert_eq!(place(&[5], 5), Some(Place::At(0)));
    assert_eq!(place(&[5], 4), Some(Place::Below));
}

#[test]
fn tenths_are_read_and_shown_below_zero_too() {
    assert_eq!(parse_tenths("-12.5"), Ok(-125));
    assert_eq!(parse_tenths(" 30 "), Ok(300));
    assert_eq!(parse_tenths("-0"), Ok(0));
    assert_eq!(parse_tenths("999.9"), Ok(9_999));
    assert_eq!(parse_tenths(""), Err(ParseError::Empty));
    assert_eq!(parse_tenths("--5"), Err(ParseError::NotANumber));
    assert_eq!(parse_tenths("warm"), Err(ParseError::NotANumber));
    assert_eq!(parse_tenths("1.25"), Err(ParseError::TooManyDecimals(1)));
    assert_eq!(
        parse_tenths("1000").unwrap_err().to_string(),
        "This must be from -999.9 to 999.9."
    );
    assert_eq!(
        parse_tenths("-1000").unwrap_err().to_string(),
        "This must be from -999.9 to 999.9."
    );
    assert_eq!(
        (show_tenths(-125), show_tenths(0), show_tenths(1_013)),
        ("-12.5".to_owned(), "0.0".to_owned(), "101.3".to_owned())
    );
}

#[test]
fn degrees_and_pressures_are_converted_exactly_and_rounded_once() {
    // (77 − 32) × 5 ÷ 9 = 25. (40 − 32) × 5 ÷ 9 = 4.44. −40 is −40 in both.
    assert_eq!(celsius_from_fahrenheit(770), 250);
    assert_eq!(celsius_from_fahrenheit(400), 44);
    assert_eq!(celsius_from_fahrenheit(-400), -400);
    assert_eq!(celsius_from_fahrenheit(320), 0);
    assert_eq!(fahrenheit_from_celsius(250), 770);
    assert_eq!(fahrenheit_from_celsius(-400), -400);
    assert_eq!(fahrenheit_from_celsius(5), 329);
    assert_eq!(fahrenheit_from_celsius(-100), 140);
    // 1 psi is 6.894757 kPa: 100 psi is 689.5 kPa, and 101.3 kPa is 14.7 psi.
    assert_eq!(kpa_from_psi(1_000), 6_895);
    assert_eq!(psi_from_kpa(6_895), 1_000);
    assert_eq!(psi_from_kpa(1_013), 147);
    assert_eq!(psi_from_kpa(1_000), 145);
    assert_eq!(psi_from_kpa(2_000), 290);
    assert_eq!(kpa_from_psi(100), 689);
    assert_eq!(kpa_from_psi(-50), -345);
    assert_eq!(psi_from_kpa(0), 0);
}
