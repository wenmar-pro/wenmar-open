use shop_math::gross_profit::{
    GrossProfitError, GrossProfitField, GrossProfitInput, Line, LineResult, Shortfall, gross_profit,
};
use shop_math::targets::Standing;
use shop_math::{Money, Percent, TooLarge};

fn cents(cents: i64) -> Money {
    Money::from_cents(cents)
}

fn line(sales: i64, cost: i64) -> Line {
    Line {
        sales: cents(sales),
        cost: cents(cost),
    }
}

/// Labor 48,000 sold and 18,000 of technician cost; parts 40,000 sold and
/// 26,000 of cost; sublet 2,000 sold and 1,600 of cost.
fn month() -> GrossProfitInput {
    GrossProfitInput {
        labor: line(4_800_000, 1_800_000),
        parts: line(4_000_000, 2_600_000),
        sublet: line(200_000, 160_000),
    }
}

#[test]
fn each_line_has_its_gross_profit_in_dollars_and_as_a_percent_of_sales() {
    let result = gross_profit(&month()).unwrap();
    assert_eq!(
        result.labor,
        LineResult {
            sales: cents(4_800_000),
            cost: cents(1_800_000),
            profit: cents(3_000_000),
            percent: Some(Percent::from_thousandths(62_500)),
            standing: Some(Standing::Inside),
            shortfall: None,
        }
    );
    assert_eq!(result.sublet.profit, cents(40_000));
    assert_eq!(result.sublet.percent, Some(Percent::whole(20)));
}

#[test]
fn parts_below_their_range_show_the_sales_the_same_cost_would_need() {
    // Parts: (40,000 − 26,000) ÷ 40,000 = 35%, below 40% to 50%. At the
    // usual 50%, a cost of 26,000 needs sales of 52,000: 12,000 more.
    let parts = gross_profit(&month()).unwrap().parts;
    assert_eq!(parts.profit, cents(1_400_000));
    assert_eq!(parts.percent, Some(Percent::whole(35)));
    assert_eq!(parts.standing, Some(Standing::Below));
    let shortfall = Shortfall {
        sales_needed: cents(5_200_000),
        difference: cents(1_200_000),
    };
    assert_eq!(parts.shortfall, Some(shortfall));
}

#[test]
fn labor_below_its_range_shows_the_sales_the_usual_70_percent_needs() {
    // 18,000 of technician cost at 70% needs 18,000 ÷ 0.3 = 60,000.
    let input = GrossProfitInput {
        labor: line(4_000_000, 1_800_000),
        ..month()
    };
    let labor = gross_profit(&input).unwrap().labor;
    assert_eq!(labor.percent, Some(Percent::whole(55)));
    assert_eq!(labor.standing, Some(Standing::Below));
    let shortfall = labor.shortfall.unwrap();
    assert_eq!(shortfall.sales_needed, cents(6_000_000));
    assert_eq!(shortfall.difference, cents(2_000_000));
}

#[test]
fn the_overall_line_is_the_sum_of_the_three_and_is_set_beside_its_target() {
    // Sales 48,000 + 40,000 + 2,000 = 90,000; cost 18,000 + 26,000 + 1,600
    // = 45,600; profit 44,400, which is 49.333%: below 50% to 60%.
    let overall = gross_profit(&month()).unwrap().overall;
    assert_eq!(overall.sales, cents(9_000_000));
    assert_eq!(overall.cost, cents(4_560_000));
    assert_eq!(overall.profit, cents(4_440_000));
    assert_eq!(overall.percent, Some(Percent::from_thousandths(49_333)));
    assert_eq!(overall.standing, Some(Standing::Below));
    assert_eq!(overall.shortfall, None);
}

#[test]
fn a_line_inside_or_above_its_range_has_no_shortfall() {
    let input = GrossProfitInput {
        labor: line(1_000_000, 200_000),
        parts: line(1_000_000, 500_000),
        sublet: line(0, 0),
    };
    let result = gross_profit(&input).unwrap();
    assert_eq!(result.labor.standing, Some(Standing::Above));
    assert_eq!(result.parts.standing, Some(Standing::Inside));
    assert_eq!(result.overall.standing, Some(Standing::Above));
    assert_eq!(result.labor.shortfall, None);
    assert_eq!(result.parts.shortfall, None);
}

#[test]
fn sublet_has_no_target_whatever_it_makes() {
    for cost in [0, 160_000, 300_000] {
        let input = GrossProfitInput {
            sublet: line(200_000, cost),
            ..month()
        };
        let sublet = gross_profit(&input).unwrap().sublet;
        assert_eq!(sublet.standing, None);
        assert_eq!(sublet.shortfall, None);
    }
}

#[test]
fn a_line_with_no_sales_shows_no_percent() {
    let input = GrossProfitInput {
        parts: line(0, 50_000),
        sublet: line(0, 0),
        ..month()
    };
    let result = gross_profit(&input).unwrap();
    assert_eq!(result.parts.profit, cents(-50_000));
    assert_eq!(result.parts.percent, None);
    assert_eq!(result.parts.standing, None);
    assert_eq!(result.parts.shortfall, None);
    assert_eq!(result.sublet.percent, None);
    assert_eq!(result.overall.sales, cents(4_800_000));
    assert_eq!(result.overall.profit, cents(2_950_000));
}

#[test]
fn a_line_sold_at_a_loss_has_a_percent_below_zero_and_is_below_its_range() {
    let input = GrossProfitInput {
        parts: line(1_000_000, 1_250_000),
        ..month()
    };
    let parts = gross_profit(&input).unwrap().parts;
    assert_eq!(parts.profit, cents(-250_000));
    assert_eq!(parts.percent, Some(Percent::whole(-25)));
    assert_eq!(parts.standing, Some(Standing::Below));
    let shortfall = parts.shortfall.unwrap();
    assert_eq!(shortfall.sales_needed, cents(2_500_000));
    assert_eq!(shortfall.difference, cents(1_500_000));
}

#[test]
fn a_percent_and_a_shortfall_are_rounded_with_halves_away_from_zero() {
    // 1 of 3 cents is 33.333%; 2 of 3 is 66.667%.
    let input = GrossProfitInput {
        labor: line(3, 2),
        parts: line(3, 1),
        sublet: line(0, 0),
    };
    let result = gross_profit(&input).unwrap();
    assert_eq!(
        result.labor.percent,
        Some(Percent::from_thousandths(33_333))
    );
    assert_eq!(
        result.parts.percent,
        Some(Percent::from_thousandths(66_667))
    );
    // Labor: 2 cents of cost at 70% needs 6.67 cents, shown as 7.
    assert_eq!(result.labor.shortfall.unwrap().sales_needed, cents(7));
    // 5 cents of cost at 70% needs 16.67 cents, shown as 17; 4 needs 13.33.
    let needed = |cost: i64| {
        let input = GrossProfitInput {
            labor: line(cost, cost),
            ..month()
        };
        let labor = gross_profit(&input).unwrap().labor;
        labor.shortfall.unwrap().sales_needed.cents()
    };
    assert_eq!(needed(5), 17);
    assert_eq!(needed(4), 13);
    assert_eq!(needed(3), 10);
}

#[test]
fn a_period_with_no_sales_at_all_is_reported_at_the_form() {
    let input = GrossProfitInput {
        labor: line(0, 1_800_000),
        parts: line(0, 0),
        sublet: line(0, 0),
    };
    let error = gross_profit(&input).unwrap_err();
    assert_eq!(error, GrossProfitError::NoSales);
    assert_eq!(error.field(), None);
    assert_eq!(error.to_string(), "Enter sales for labor, parts or sublet.");
}

#[test]
fn a_figure_below_zero_is_refused_at_its_field() {
    let cases = [
        (
            GrossProfitInput {
                labor: line(-1, 0),
                ..month()
            },
            GrossProfitField::LaborSales,
        ),
        (
            GrossProfitInput {
                labor: line(0, -1),
                ..month()
            },
            GrossProfitField::TechnicianCost,
        ),
        (
            GrossProfitInput {
                parts: line(-1, 0),
                ..month()
            },
            GrossProfitField::PartsSales,
        ),
        (
            GrossProfitInput {
                parts: line(0, -1),
                ..month()
            },
            GrossProfitField::PartsCost,
        ),
        (
            GrossProfitInput {
                sublet: line(-1, 0),
                ..month()
            },
            GrossProfitField::SubletSales,
        ),
        (
            GrossProfitInput {
                sublet: line(0, -1),
                ..month()
            },
            GrossProfitField::SubletCost,
        ),
    ];
    for (input, field) in cases {
        let error = gross_profit(&input).unwrap_err();
        assert_eq!(error, GrossProfitError::BelowZero { field });
        assert_eq!(error.field(), Some(field));
        assert_eq!(error.to_string(), "This cannot be below zero.");
    }
}

#[test]
fn the_largest_inputs_give_a_result_and_larger_ones_are_too_large() {
    let most = Money::MAX_INPUT.cents();
    let input = GrossProfitInput {
        labor: line(most, most),
        parts: line(most, most),
        sublet: line(most, most),
    };
    let result = gross_profit(&input).unwrap();
    assert_eq!(result.overall.sales, cents(29_999_999_997));
    assert_eq!(result.overall.percent, Some(Percent::ZERO));
    assert_eq!(
        result.labor.shortfall.unwrap().sales_needed,
        cents(33_333_333_330)
    );

    let beyond = GrossProfitInput {
        labor: line(i64::MAX, 0),
        parts: line(1, 0),
        sublet: line(0, 0),
    };
    let error = gross_profit(&beyond).unwrap_err();
    assert_eq!(error, GrossProfitError::TooLarge(TooLarge));
    assert_eq!(error.to_string(), "The result is too large to show.");
    let beyond = GrossProfitInput {
        labor: line(1, i64::MAX),
        parts: line(1, i64::MAX),
        sublet: line(0, 0),
    };
    assert_eq!(
        gross_profit(&beyond),
        Err(GrossProfitError::TooLarge(TooLarge))
    );
}
