use shop_math::{Money, ParseError, Percent, TooLarge};

fn thousandths(text: &str) -> Result<i64, ParseError> {
    Percent::parse(text).map(Percent::thousandths)
}

#[test]
fn a_percent_is_read_with_or_without_its_sign() {
    assert_eq!(thousandths("50"), Ok(50_000));
    assert_eq!(thousandths("50%"), Ok(50_000));
    assert_eq!(thousandths(" 50 % "), Ok(50_000));
    assert_eq!(thousandths("9.975%"), Ok(9_975));
    assert_eq!(thousandths("1,000"), Ok(1_000_000));
    assert_eq!(thousandths("0"), Ok(0));
}

#[test]
fn a_percent_that_cannot_be_read_is_refused() {
    assert_eq!(thousandths(""), Err(ParseError::Empty));
    assert_eq!(thousandths("%"), Err(ParseError::Empty));
    assert_eq!(thousandths("fifty"), Err(ParseError::NotANumber));
    assert_eq!(thousandths("50%%"), Err(ParseError::NotANumber));
    assert_eq!(thousandths("$50"), Err(ParseError::NotANumber));
    assert_eq!(thousandths("-5%"), Err(ParseError::BelowZero));
    assert_eq!(thousandths("9.9755"), Err(ParseError::TooManyDecimals(3)));
}

#[test]
fn a_percent_is_from_0_to_1000_unless_a_field_says_otherwise() {
    assert_eq!(Percent::parse("1000"), Ok(Percent::MAX_INPUT));
    let error = Percent::parse("1000.001").unwrap_err();
    assert_eq!(error.to_string(), "This must be from 0% to 1,000%.");

    let (least, most) = (Percent::whole(1), Percent::whole(200));
    assert_eq!(
        Percent::parse_between("85", least, most),
        Ok(Percent::whole(85))
    );
    assert_eq!(Percent::parse_between("200%", least, most), Ok(most));
    for text in ["0", "0.999", "200.001", &"9".repeat(40)] {
        let error = Percent::parse_between(text, least, most).unwrap_err();
        assert_eq!(error.to_string(), "This must be from 1% to 200%.", "{text}");
    }
}

#[test]
fn a_percent_is_shown_without_zeros_it_does_not_need() {
    let shown = |thousandths: i64| Percent::from_thousandths(thousandths).to_string();
    assert_eq!(shown(50_000), "50%");
    assert_eq!(shown(9_975), "9.975%");
    assert_eq!(shown(37_500), "37.5%");
    assert_eq!(shown(0), "0%");
    assert_eq!(shown(-12_500), "-12.5%");
    assert_eq!(shown(1_000_000), "1,000%");
    assert_eq!(Percent::from_thousandths(1_000_000).input(), "1000");
    assert_eq!(Percent::from_thousandths(9_975).input(), "9.975");
}

#[test]
fn a_percent_is_rounded_to_a_tenth_with_halves_away_from_zero() {
    let tenth = |thousandths: i64| {
        Percent::from_thousandths(thousandths)
            .nearest_tenth()
            .thousandths()
    };
    assert_eq!(tenth(33_333), 33_300);
    assert_eq!(tenth(33_349), 33_300);
    assert_eq!(tenth(33_350), 33_400);
    assert_eq!(tenth(-33_350), -33_400);
    assert_eq!(tenth(-33_349), -33_300);
    assert_eq!(tenth(i64::MAX), 9_223_372_036_854_775_800);
}

#[test]
fn one_amount_as_a_percent_of_another_is_rounded_to_a_thousandth() {
    let of = |part: i64, whole: i64| Percent::of(Money::from_cents(part), Money::from_cents(whole));
    assert_eq!(of(50, 100), Ok(Some(Percent::whole(50))));
    assert_eq!(of(1, 3), Ok(Some(Percent::from_thousandths(33_333))));
    assert_eq!(of(2, 3), Ok(Some(Percent::from_thousandths(66_667))));
    // 1 in 200,000 is 0.0005%: a half, which goes away from zero.
    assert_eq!(of(1, 200_000), Ok(Some(Percent::from_thousandths(1))));
    assert_eq!(of(-1, 200_000), Ok(Some(Percent::from_thousandths(-1))));
    assert_eq!(of(-25, 100), Ok(Some(Percent::whole(-25))));
}

#[test]
fn a_percent_of_nothing_is_no_figure_and_one_too_large_is_reported() {
    let of = |part: i64, whole: i64| Percent::of(Money::from_cents(part), Money::from_cents(whole));
    assert_eq!(of(50, 0), Ok(None));
    assert_eq!(of(0, 0), Ok(None));
    assert_eq!(of(i64::MAX, 1), Err(TooLarge));
    assert_eq!(of(i64::MIN, 1), Err(TooLarge));
}
