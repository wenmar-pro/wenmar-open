use shop_math::labor_rate::{EffectiveRateInput, LaborField, LaborRateError, effective_rate};
use shop_math::{Hours, Money, Percent, TooLarge};

fn cents(cents: i64) -> Money {
    Money::from_cents(cents)
}

/// 48,000 of labor sold in 520 hours, with 120.00 posted and 18,000 of
/// technician cost.
fn period() -> EffectiveRateInput {
    EffectiveRateInput {
        labor_sales: cents(4_800_000),
        hours_billed: Hours::from_hundredths(52_000),
        posted_rate: Some(cents(12_000)),
        technician_cost: Some(cents(1_800_000)),
    }
}

#[test]
fn the_effective_rate_is_labor_sales_over_hours_billed() {
    // 48,000 ÷ 520 = 92.31, which is 76.925% of 120.00 and 27.69 short an
    // hour. At 120.00 the 520 hours would be 62,400: 14,400 more.
    // Labor gross profit: (48,000 − 18,000) ÷ 48,000 = 62.5%.
    let result = effective_rate(&period()).unwrap();
    assert_eq!(result.rate, cents(9_231));
    let posted = result.against_posted.unwrap();
    assert_eq!(posted.percent_of_posted, Percent::from_thousandths(76_925));
    assert_eq!(posted.per_hour, cents(2_769));
    assert_eq!(posted.over_the_period, cents(1_440_000));
    assert_eq!(
        result.labor_gross_profit,
        Some(Percent::from_thousandths(62_500))
    );
}

#[test]
fn what_was_not_given_is_not_worked_out() {
    let input = EffectiveRateInput {
        posted_rate: None,
        technician_cost: None,
        ..period()
    };
    let result = effective_rate(&input).unwrap();
    assert_eq!(result.rate, cents(9_231));
    assert_eq!(result.against_posted, None);
    assert_eq!(result.labor_gross_profit, None);
}

#[test]
fn a_shop_getting_more_than_its_posted_rate_has_differences_below_zero() {
    let input = EffectiveRateInput {
        posted_rate: Some(cents(9_000)),
        ..period()
    };
    let posted = effective_rate(&input).unwrap().against_posted.unwrap();
    assert_eq!(posted.percent_of_posted, Percent::from_thousandths(102_567));
    assert_eq!(posted.per_hour, cents(-231));
    assert_eq!(posted.over_the_period, cents(-120_000));
}

#[test]
fn technicians_who_cost_more_than_labor_sold_for_give_a_gross_profit_below_zero() {
    let input = EffectiveRateInput {
        technician_cost: Some(cents(6_000_000)),
        ..period()
    };
    let result = effective_rate(&input).unwrap();
    assert_eq!(result.labor_gross_profit, Some(Percent::whole(-25)));
    let nothing_sold = EffectiveRateInput {
        labor_sales: Money::ZERO,
        ..period()
    };
    let result = effective_rate(&nothing_sold).unwrap();
    assert_eq!(result.rate, Money::ZERO);
    assert_eq!(result.labor_gross_profit, None);
}

#[test]
fn the_effective_rate_refuses_no_hours_a_posted_rate_of_zero_and_figures_below_zero() {
    let refused = |input: EffectiveRateInput| effective_rate(&input).unwrap_err();
    let field = LaborField::HoursBilled;
    let error = refused(EffectiveRateInput {
        hours_billed: Hours::ZERO,
        ..period()
    });
    assert_eq!(error, LaborRateError::NoHours { field });

    let field = LaborField::PostedRate;
    let error = refused(EffectiveRateInput {
        posted_rate: Some(Money::ZERO),
        ..period()
    });
    assert_eq!(error, LaborRateError::Zero { field });
    assert_eq!(error.field(), Some(field));
    assert_eq!(error.to_string(), "This cannot be zero.");

    let below = [
        (
            EffectiveRateInput {
                labor_sales: cents(-1),
                ..period()
            },
            LaborField::LaborSales,
        ),
        (
            EffectiveRateInput {
                hours_billed: Hours::from_hundredths(-1),
                ..period()
            },
            LaborField::HoursBilled,
        ),
        (
            EffectiveRateInput {
                posted_rate: Some(cents(-1)),
                ..period()
            },
            LaborField::PostedRate,
        ),
        (
            EffectiveRateInput {
                technician_cost: Some(cents(-1)),
                ..period()
            },
            LaborField::PeriodTechnicianCost,
        ),
    ];
    for (input, field) in below {
        assert_eq!(refused(input), LaborRateError::BelowZero { field });
    }
}

#[test]
fn the_largest_sales_give_a_rate_and_larger_ones_are_too_large() {
    let most = EffectiveRateInput {
        labor_sales: Money::MAX_INPUT,
        hours_billed: Hours::from_hundredths(1),
        posted_rate: Some(cents(1)),
        technician_cost: Some(Money::MAX_INPUT),
    };
    assert_eq!(effective_rate(&most).unwrap().rate, cents(999_999_999_900));
    let beyond = EffectiveRateInput {
        labor_sales: cents(i64::MAX),
        ..most
    };
    assert_eq!(
        effective_rate(&beyond),
        Err(LaborRateError::TooLarge(TooLarge))
    );
    let beyond = EffectiveRateInput {
        hours_billed: Hours::from_hundredths(i64::MAX),
        posted_rate: Some(cents(i64::MAX)),
        ..most
    };
    assert_eq!(
        effective_rate(&beyond),
        Err(LaborRateError::TooLarge(TooLarge))
    );
}
