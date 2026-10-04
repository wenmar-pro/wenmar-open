use shop_math::{Hours, ParseError, parse_count};

#[test]
fn hours_are_read_to_the_hundredth() {
    assert_eq!(Hours::parse("173"), Ok(Hours::from_hundredths(17_300)));
    assert_eq!(Hours::parse("1,470.5"), Ok(Hours::from_hundredths(147_050)));
    assert_eq!(Hours::parse("0.25"), Ok(Hours::from_hundredths(25)));
    assert_eq!(Hours::parse("99,999,999.99"), Ok(Hours::MAX_INPUT));
}

#[test]
fn hours_that_cannot_be_read_are_refused() {
    assert_eq!(Hours::parse(""), Err(ParseError::Empty));
    assert_eq!(Hours::parse("$173"), Err(ParseError::NotANumber));
    assert_eq!(Hours::parse("-1"), Err(ParseError::BelowZero));
    assert_eq!(Hours::parse("1.005"), Err(ParseError::TooManyDecimals(2)));
    let error = Hours::parse("100,000,000").unwrap_err();
    assert_eq!(error.to_string(), "This must be from 0 to 99,999,999.99.");
}

#[test]
fn hours_are_shown_without_zeros_they_do_not_need() {
    assert_eq!(Hours::from_hundredths(17_300).to_string(), "173");
    assert_eq!(Hours::from_hundredths(44_115).to_string(), "441.15");
    assert_eq!(Hours::from_hundredths(147_050).to_string(), "1,470.5");
    assert_eq!(Hours::from_hundredths(147_050).input(), "1470.5");
    assert_eq!(Hours::from_hundredths(17_300).hundredths(), 17_300);
}

#[test]
fn a_count_is_a_whole_number_from_0_to_1000() {
    assert_eq!(parse_count("3"), Ok(3));
    assert_eq!(parse_count("3.0"), Ok(3));
    assert_eq!(parse_count("1,000"), Ok(1_000));
    assert_eq!(parse_count("0"), Ok(0));
    assert_eq!(parse_count(""), Err(ParseError::Empty));
    assert_eq!(parse_count("2.5"), Err(ParseError::NotWhole));
    assert_eq!(parse_count("-2"), Err(ParseError::BelowZero));
    assert_eq!(parse_count("three"), Err(ParseError::NotANumber));
    for text in ["1001", &"9".repeat(40)] {
        let error = parse_count(text).unwrap_err();
        assert_eq!(error.to_string(), "This must be from 0 to 1,000.", "{text}");
    }
    assert_eq!(ParseError::NotWhole.to_string(), "Use a whole number.");
}
