use shop_math::labor_rate::{LaborRateError, labor_charge};
use shop_math::{Hours, Money};

fn charge(rate_cents: i64, hundredths: i64) -> Result<Money, LaborRateError> {
    labor_charge(
        Money::from_cents(rate_cents),
        Hours::from_hundredths(hundredths),
    )
}

#[test]
fn hours_times_a_rate_is_money_to_the_cent() {
    // Made-up figures: 98 hours at $100.00, then awkward ones.
    assert_eq!(charge(10_000, 9_800), Ok(Money::from_cents(980_000)));
    assert_eq!(charge(12_345, 9_925), Ok(Money::from_cents(1_225_241)));
    assert_eq!(charge(10_000, 0), Ok(Money::ZERO));
    assert_eq!(charge(0, 9_800), Ok(Money::ZERO));
    assert_eq!(charge(1, 100), Ok(Money::from_cents(1)));
}

#[test]
fn a_rate_or_hours_below_zero_is_refused() {
    assert!(matches!(
        charge(-1, 100),
        Err(LaborRateError::BelowZero { .. })
    ));
    assert!(matches!(
        charge(100, -1),
        Err(LaborRateError::BelowZero { .. })
    ));
}
