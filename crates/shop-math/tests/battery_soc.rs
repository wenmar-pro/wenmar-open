use shop_math::ParseError;
use shop_math::battery_soc::{Correction, corrected, parse_volts, volts};

/// Made up: 10 mV a degree and a table measured at 25 °C are no real battery's figures.
const FAKE: Correction = Correction {
    reference_tenths_c: 250,
    microvolts_per_degree: 10_000,
};

#[test]
fn a_reading_is_corrected_to_the_reference_temperature() {
    // 20 degrees below: 11.400 + 0.010 × 20 = 11.600.
    assert_eq!(corrected(11_400, 50, &FAKE), Some(11_600));
    // At the reference temperature nothing is added.
    assert_eq!(corrected(11_400, 250, &FAKE), Some(11_400));
    // 10 degrees above: 0.100 is taken off.
    assert_eq!(corrected(11_400, 350, &FAKE), Some(11_300));
    // 4.4 °C is 20.6 degrees below: 0.206 is added.
    assert_eq!(corrected(11_400, 44, &FAKE), Some(11_606));
    // Rounded once, halves away from zero: 0.0015 V a degree over 1 degree is 0.002 V.
    let fine = Correction {
        reference_tenths_c: 250,
        microvolts_per_degree: 1_500,
    };
    assert_eq!(corrected(12_000, 240, &fine), Some(12_002));
    assert_eq!(corrected(12_000, 260, &fine), Some(11_998));
    assert_eq!(corrected(i64::MAX, 0, &FAKE), None);
}

#[test]
fn volts_are_read_and_shown() {
    assert_eq!(parse_volts("12.6"), Ok(12_600));
    assert_eq!(parse_volts(" 12.065 V"), Ok(12_065));
    assert_eq!(parse_volts("12.60v"), Ok(12_600));
    assert_eq!(parse_volts(""), Err(ParseError::Empty));
    assert_eq!(parse_volts("12.0655"), Err(ParseError::TooManyDecimals(3)));
    assert_eq!(parse_volts("-1"), Err(ParseError::BelowZero));
    assert_eq!(
        parse_volts("100").unwrap_err().to_string(),
        "This must be from 0 to 99.999."
    );
    assert_eq!(
        (volts(12_600), volts(12_065), volts(0)),
        ("12.60".to_owned(), "12.065".to_owned(), "0.00".to_owned())
    );
}
