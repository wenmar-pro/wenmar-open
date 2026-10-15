use shop_math::technician_pay::gross;
use shop_math::{Hours, Money, TooLarge};

fn hours(hundredths: i64) -> Hours {
    Hours::from_hundredths(hundredths)
}

fn dollars(cents: i64) -> Money {
    Money::from_cents(cents)
}

/// A made-up week: 12 flagged hours at a $40 flat, 30 hourly hours at $25,
/// and 4 overtime hours at $37.50.
fn week() -> shop_math::technician_pay::Gross {
    gross(
        hours(1_200),
        dollars(4_000),
        hours(3_000),
        dollars(2_500),
        hours(400),
        dollars(3_750),
    )
    .unwrap()
}

#[test]
fn the_gross_is_the_three_parts_to_the_cent() {
    let gross = week();
    assert_eq!(gross.flagged, dollars(48_000)); // 12 × $40.00 = $480.00
    assert_eq!(gross.hourly, dollars(75_000)); // 30 × $25.00 = $750.00
    assert_eq!(gross.overtime, dollars(15_000)); // 4 × $37.50 = $150.00
    assert_eq!(gross.total, dollars(138_000)); // $1,380.00
}

#[test]
fn each_part_is_rounded_to_the_cent_before_the_parts_are_added() {
    // Awkward hours and rates: the parts do not land on a whole dollar.
    let gross = gross(
        hours(1_033),
        dollars(13_456),
        hours(2_177),
        dollars(7_891),
        hours(120),
        dollars(45_678),
    )
    .unwrap();
    // 10.33 × $134.56 = $1,389.9948 → $1,390.00
    assert_eq!(gross.flagged, dollars(139_000));
    // 21.77 × $78.91 = $1,717.8707 → $1,717.87
    assert_eq!(gross.hourly, dollars(171_787));
    // 1.20 × $456.78 = $548.136 → $548.14
    assert_eq!(gross.overtime, dollars(54_814));
    assert_eq!(
        gross.total,
        gross
            .flagged
            .checked_add(gross.hourly)
            .unwrap()
            .checked_add(gross.overtime)
            .unwrap()
    );
    assert_eq!(gross.total, dollars(365_601));
}

#[test]
fn empty_parts_leave_the_rest() {
    let gross = gross(
        hours(0),
        dollars(4_000),
        hours(3_000),
        dollars(2_500),
        hours(0),
        dollars(3_750),
    )
    .unwrap();
    assert_eq!(gross.flagged, Money::ZERO);
    assert_eq!(gross.hourly, dollars(75_000));
    assert_eq!(gross.overtime, Money::ZERO);
    assert_eq!(gross.total, dollars(75_000));
}

#[test]
fn nothing_is_below_zero_here() {
    // This module adds and multiplies; the form refuses a negative figure
    // before it gets here. A zero figure is a zero part.
    let gross = gross(
        hours(0),
        dollars(0),
        hours(0),
        dollars(0),
        hours(0),
        dollars(0),
    )
    .unwrap();
    assert_eq!(gross.total, Money::ZERO);
}

#[test]
fn a_result_that_does_not_fit_is_too_large() {
    // Beyond what a form field accepts: the product does not fit in the
    // cents this crate holds.
    let big = Hours::from_hundredths(i64::MAX);
    let rate = Money::from_cents(i64::MAX);
    assert_eq!(
        gross(big, rate, hours(0), dollars(0), hours(0), dollars(0)),
        Err(TooLarge)
    );
}
