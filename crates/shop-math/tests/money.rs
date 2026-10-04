use shop_math::{Money, ParseError, TooLarge};

fn cents(text: &str) -> Result<i64, ParseError> {
    Money::parse(text).map(Money::cents)
}

#[test]
fn money_is_read_plain_with_commas_and_with_a_dollar_sign() {
    assert_eq!(cents("1234.5"), Ok(123_450));
    assert_eq!(cents("1,234.50"), Ok(123_450));
    assert_eq!(cents("$1,234.50"), Ok(123_450));
    assert_eq!(cents("  $ 12  "), Ok(1_200));
    assert_eq!(cents(".5"), Ok(50));
    assert_eq!(cents("5."), Ok(500));
    assert_eq!(cents("0"), Ok(0));
    assert_eq!(cents("007"), Ok(700));
}

#[test]
fn zeros_after_the_cents_are_accepted_and_other_digits_are_not() {
    assert_eq!(cents("12.3400"), Ok(1_234));
    assert_eq!(cents("12.345"), Err(ParseError::TooManyDecimals(2)));
}

#[test]
fn a_comma_that_does_not_group_thousands_is_refused() {
    for text in [
        "1,5", "12,50", "1,2345", "1234,567", ",500", "1,,000", "1,000,00",
    ] {
        assert_eq!(cents(text), Err(ParseError::NotANumber), "{text}");
    }
    assert_eq!(cents("12,345,678.90"), Ok(1_234_567_890));
}

#[test]
fn text_that_is_not_a_number_is_refused() {
    for text in [
        "abc", "12abc", "1.2.3", "1e5", "+5", "1 234", "(5)", "$$5", ".", "٣",
    ] {
        assert_eq!(cents(text), Err(ParseError::NotANumber), "{text}");
    }
}

#[test]
fn an_empty_field_is_reported_as_empty() {
    for text in ["", "   ", "$"] {
        assert_eq!(cents(text), Err(ParseError::Empty), "{text:?}");
    }
}

#[test]
fn money_below_zero_is_refused_and_minus_zero_is_zero() {
    assert_eq!(cents("-5"), Err(ParseError::BelowZero));
    assert_eq!(cents("-$5.00"), Err(ParseError::BelowZero));
    assert_eq!(cents("$-5.00"), Err(ParseError::BelowZero));
    assert_eq!(cents("-0.00"), Ok(0));
}

#[test]
fn money_is_read_up_to_its_limit_and_no_further() {
    assert_eq!(cents("99,999,999.99"), Ok(9_999_999_999));
    assert_eq!(Money::parse("99999999.99"), Ok(Money::MAX_INPUT));
    let over = ParseError::OutOfRange {
        least: "0".to_owned(),
        most: "99,999,999.99".to_owned(),
    };
    assert_eq!(cents("100,000,000"), Err(over.clone()));
    assert_eq!(cents(&"9".repeat(60)), Err(over));
}

#[test]
fn each_refusal_says_what_was_wrong_in_plain_words() {
    let message = |text: &str| Money::parse(text).unwrap_err().to_string();
    assert_eq!(message(""), "Enter a number.");
    assert_eq!(
        message("abc"),
        "This is not a number. Write it like 1,234.50."
    );
    assert_eq!(message("-1"), "This cannot be below zero.");
    assert_eq!(message("1.234"), "Use at most 2 decimal places.");
    assert_eq!(
        message("100000000"),
        "This must be from 0 to 99,999,999.99."
    );
}

#[test]
fn money_is_shown_with_a_dollar_sign_commas_and_two_decimals() {
    let shown = |cents: i64| Money::from_cents(cents).to_string();
    assert_eq!(shown(0), "$0.00");
    assert_eq!(shown(5), "$0.05");
    assert_eq!(shown(123_450), "$1,234.50");
    assert_eq!(shown(100_000_000), "$1,000,000.00");
    assert_eq!(shown(-123_450), "-$1,234.50");
    assert_eq!(shown(i64::MIN), "-$92,233,720,368,547,758.08");
}

#[test]
fn money_for_a_form_field_has_no_sign_and_no_commas() {
    assert_eq!(Money::from_cents(123_450).input(), "1234.50");
    assert_eq!(Money::from_cents(-5).input(), "-0.05");
    assert_eq!(
        Money::parse(&Money::MAX_INPUT.input()),
        Ok(Money::MAX_INPUT)
    );
}

#[test]
fn adding_and_subtracting_money_never_overflows() {
    let most = Money::from_cents(i64::MAX);
    let one = Money::from_cents(1);
    assert_eq!(most.checked_add(one), Err(TooLarge));
    assert_eq!(Money::from_cents(i64::MIN).checked_sub(one), Err(TooLarge));
    assert_eq!(one.checked_add(one), Ok(Money::from_cents(2)));
    assert_eq!(Money::ZERO.checked_sub(one), Ok(Money::from_cents(-1)));
    assert_eq!(TooLarge.to_string(), "The result is too large to show.");
}
