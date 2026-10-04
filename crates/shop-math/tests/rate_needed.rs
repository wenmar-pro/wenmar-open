use std::cmp::Ordering;

use shop_math::labor_rate::{
    DEFAULT_PAID_HOURS, DEFAULT_PRODUCTIVITY, DEFAULT_TARGET_PROFIT, LEAST_PRODUCTIVITY,
    LaborField, LaborRateError, MOST_PRODUCTIVITY, MOST_TARGET, RateNeeded, RateNeededInput,
    rate_needed,
};
use shop_math::targets::LABOR;
use shop_math::{Hours, Money, Percent, TooLarge};

fn cents(cents: i64) -> Money {
    Money::from_cents(cents)
}

/// Three technicians at the defaults, 18,000 of technician cost, 25,000 of
/// overhead and 12,000 of parts gross profit.
fn shop() -> RateNeededInput {
    RateNeededInput {
        technicians: 3,
        paid_hours: DEFAULT_PAID_HOURS,
        productivity: DEFAULT_PRODUCTIVITY,
        technician_cost: cents(1_800_000),
        overhead: cents(2_500_000),
        parts_gross_profit: cents(1_200_000),
        target_profit: DEFAULT_TARGET_PROFIT,
        target_labor_gross_profit: LABOR.usual,
    }
}

#[test]
fn the_defaults_are_those_of_the_design() {
    assert_eq!(DEFAULT_PAID_HOURS.to_string(), "173");
    assert_eq!(DEFAULT_PRODUCTIVITY.to_string(), "85%");
    assert_eq!(DEFAULT_TARGET_PROFIT.to_string(), "10%");
    assert_eq!(LEAST_PRODUCTIVITY.to_string(), "1%");
    assert_eq!(MOST_PRODUCTIVITY.to_string(), "200%");
    assert_eq!(MOST_TARGET.to_string(), "99%");
}

#[test]
fn the_rate_needed_covers_costs_less_parts_profit_and_leaves_the_target_profit() {
    // Hours billed: 3 × 173 × 0.85 = 441.15.
    // Labor sales needed: (18,000 + 25,000 − 12,000) ÷ 0.9 = 34,444.44.
    // Rate needed: 34,444.44… ÷ 441.15 = 78.08. Break-even: 31,000 ÷ 441.15 = 70.27.
    // Labor gross profit: (34,444.44 − 18,000) ÷ 34,444.44 = 47.742%.
    // From the 70% target: 18,000 ÷ 441.15 ÷ 0.3 = 136.01.
    assert_eq!(
        rate_needed(&shop()),
        Ok(RateNeeded {
            hours_billed: Hours::from_hundredths(44_115),
            labor_sales_needed: cents(3_444_444),
            rate_needed: cents(7_808),
            break_even_rate: cents(7_027),
            covered_by_parts: false,
            labor_gross_profit: Some(Percent::from_thousandths(47_742)),
            target_rate: cents(13_601),
            needed_against_target: Ordering::Less,
        })
    );
}

#[test]
fn a_rate_is_rounded_to_the_cent_with_halves_away_from_zero() {
    let rate = |cost: i64, hours: i64| {
        let input = RateNeededInput {
            technicians: 1,
            paid_hours: Hours::from_hundredths(hours),
            productivity: Percent::HUNDRED,
            technician_cost: cents(cost),
            overhead: Money::ZERO,
            parts_gross_profit: Money::ZERO,
            target_profit: Percent::ZERO,
            target_labor_gross_profit: Percent::ZERO,
        };
        rate_needed(&input).unwrap().rate_needed.cents()
    };
    assert_eq!(rate(1, 200), 1); // half a cent an hour
    assert_eq!(rate(1, 201), 0); // just under half
    assert_eq!(rate(3, 200), 2); // a cent and a half
    assert_eq!(rate(10_000, 300), 3_333);
    assert_eq!(rate(20_000, 300), 6_667);
}

#[test]
fn overhead_that_asks_for_more_than_the_target_gives_is_said_to() {
    let heavy = RateNeededInput {
        overhead: cents(9_000_000),
        parts_gross_profit: Money::ZERO,
        ..shop()
    };
    let result = rate_needed(&heavy).unwrap();
    assert_eq!(result.labor_sales_needed, cents(12_000_000));
    assert_eq!(result.rate_needed, cents(27_202));
    assert_eq!(result.labor_gross_profit, Some(Percent::whole(85)));
    assert_eq!(result.target_rate, cents(13_601));
    assert_eq!(result.needed_against_target, Ordering::Greater);
}

#[test]
fn the_rate_from_the_target_needs_no_overhead_and_follows_the_target() {
    let at = |target: i64| {
        let input = RateNeededInput {
            target_labor_gross_profit: Percent::whole(target),
            ..shop()
        };
        rate_needed(&input).unwrap().target_rate.cents()
    };
    assert_eq!(at(0), 4_080); // 18,000 ÷ 441.15
    assert_eq!(at(60), 10_201);
    assert_eq!(at(70), 13_601);
    assert_eq!(at(99), 408_024); // 18,000 ÷ 441.15 ÷ 0.01 = 4,080.24
}

#[test]
fn parts_profit_that_covers_every_cost_gives_a_rate_of_zero_and_says_so() {
    for parts in [4_300_000, 5_000_000] {
        let input = RateNeededInput {
            parts_gross_profit: cents(parts),
            ..shop()
        };
        let result = rate_needed(&input).unwrap();
        assert!(result.covered_by_parts, "{parts}");
        assert_eq!(result.labor_sales_needed, Money::ZERO);
        assert_eq!(result.rate_needed, Money::ZERO);
        assert_eq!(result.break_even_rate, Money::ZERO);
        assert_eq!(result.labor_gross_profit, None);
        assert_eq!(result.target_rate, cents(13_601));
    }
    let short = RateNeededInput {
        parts_gross_profit: cents(4_299_999),
        ..shop()
    };
    assert!(!rate_needed(&short).unwrap().covered_by_parts);
}

#[test]
fn no_hours_billed_is_reported_at_the_field_that_is_zero() {
    let zero = [
        (
            RateNeededInput {
                technicians: 0,
                ..shop()
            },
            LaborField::Technicians,
        ),
        (
            RateNeededInput {
                paid_hours: Hours::ZERO,
                ..shop()
            },
            LaborField::PaidHours,
        ),
        (
            RateNeededInput {
                productivity: Percent::ZERO,
                ..shop()
            },
            LaborField::Productivity,
        ),
    ];
    for (input, field) in zero {
        let error = rate_needed(&input).unwrap_err();
        assert_eq!(error, LaborRateError::NoHours { field });
        assert_eq!(error.field(), Some(field));
        assert_eq!(error.to_string(), "No hours would be billed with this.");
    }
}

#[test]
fn hours_too_few_to_count_are_reported_and_give_no_rate() {
    // One technician paid 0.01 hours at 1% bills 0.0001 hours. That is
    // shown as no hours, so it is refused, not turned into a huge rate.
    let input = RateNeededInput {
        technicians: 1,
        paid_hours: Hours::from_hundredths(1),
        productivity: LEAST_PRODUCTIVITY,
        ..shop()
    };
    let field = LaborField::PaidHours;
    assert_eq!(rate_needed(&input), Err(LaborRateError::NoHours { field }));
}

#[test]
fn a_target_of_100_percent_or_more_is_refused_at_its_field() {
    for percent in [100, 150] {
        let profit = RateNeededInput {
            target_profit: Percent::whole(percent),
            ..shop()
        };
        let error = rate_needed(&profit).unwrap_err();
        let field = LaborField::TargetProfit;
        assert_eq!(error, LaborRateError::NotBelowHundred { field });
        assert_eq!(error.to_string(), "This must be below 100%.");

        let target = RateNeededInput {
            target_labor_gross_profit: Percent::whole(percent),
            ..shop()
        };
        let field = LaborField::TargetLaborGrossProfit;
        assert_eq!(rate_needed(&target).unwrap_err().field(), Some(field));
    }
    let most = RateNeededInput {
        target_profit: MOST_TARGET,
        ..shop()
    };
    assert_eq!(
        rate_needed(&most).unwrap().labor_sales_needed,
        cents(310_000_000)
    );
}

#[test]
fn a_figure_below_zero_is_refused_at_its_field() {
    let below = Percent::from_thousandths(-1);
    let cases = [
        (
            RateNeededInput {
                paid_hours: Hours::from_hundredths(-1),
                ..shop()
            },
            LaborField::PaidHours,
        ),
        (
            RateNeededInput {
                productivity: below,
                ..shop()
            },
            LaborField::Productivity,
        ),
        (
            RateNeededInput {
                technician_cost: cents(-1),
                ..shop()
            },
            LaborField::TechnicianCost,
        ),
        (
            RateNeededInput {
                overhead: cents(-1),
                ..shop()
            },
            LaborField::Overhead,
        ),
        (
            RateNeededInput {
                parts_gross_profit: cents(-1),
                ..shop()
            },
            LaborField::PartsGrossProfit,
        ),
        (
            RateNeededInput {
                target_profit: below,
                ..shop()
            },
            LaborField::TargetProfit,
        ),
        (
            RateNeededInput {
                target_labor_gross_profit: below,
                ..shop()
            },
            LaborField::TargetLaborGrossProfit,
        ),
    ];
    for (input, field) in cases {
        let error = rate_needed(&input).unwrap_err();
        assert_eq!(error, LaborRateError::BelowZero { field });
        assert_eq!(error.to_string(), "This cannot be below zero.");
    }
}

#[test]
fn the_largest_inputs_give_a_rate_and_larger_ones_are_too_large() {
    let most = RateNeededInput {
        technicians: 1,
        paid_hours: Hours::from_hundredths(100),
        productivity: LEAST_PRODUCTIVITY,
        technician_cost: Money::MAX_INPUT,
        overhead: Money::MAX_INPUT,
        parts_gross_profit: Money::ZERO,
        target_profit: MOST_TARGET,
        target_labor_gross_profit: MOST_TARGET,
    };
    let result = rate_needed(&most).unwrap();
    assert_eq!(result.hours_billed, Hours::from_hundredths(1));
    assert_eq!(result.rate_needed, cents(199_999_999_980_000));

    let busiest = RateNeededInput {
        technicians: 1_000,
        paid_hours: Hours::MAX_INPUT,
        productivity: MOST_PRODUCTIVITY,
        ..most
    };
    assert_eq!(
        rate_needed(&busiest).unwrap().hours_billed,
        Hours::from_hundredths(19_999_999_998_000)
    );

    let beyond = RateNeededInput {
        technician_cost: cents(i64::MAX),
        ..most
    };
    let error = rate_needed(&beyond).unwrap_err();
    assert_eq!(error, LaborRateError::TooLarge(TooLarge));
    assert_eq!(error.field(), None);
    let beyond = RateNeededInput {
        technicians: u32::MAX,
        paid_hours: Hours::from_hundredths(i64::MAX),
        productivity: Percent::from_thousandths(i64::MAX),
        ..most
    };
    assert_eq!(
        rate_needed(&beyond),
        Err(LaborRateError::TooLarge(TooLarge))
    );
}
