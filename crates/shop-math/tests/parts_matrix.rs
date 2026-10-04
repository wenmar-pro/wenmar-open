use shop_math::parts_matrix::{
    MAX_TIERS, MatrixError, MatrixResult, RateKind, TierInput, margin_to_markup, markup_to_margin,
    work_out,
};
use shop_math::targets::{PARTS, Standing};
use shop_math::{Money, Percent, TooLarge};

fn cents(cents: i64) -> Money {
    Money::from_cents(cents)
}

/// Tiers from (cost up to in cents, percent in thousandths, share in
/// thousandths; a share below zero means none), in rows 1, 2, 3 and on.
fn tiers(rows: &[(i64, i64, i64)]) -> Vec<TierInput> {
    let tier = |(index, &(cost, rate, share)): (usize, &(i64, i64, i64))| TierInput {
        row: index + 1,
        cost_up_to: cents(cost),
        rate: Percent::from_thousandths(rate),
        share: (share >= 0).then_some(Percent::from_thousandths(share)),
    };
    rows.iter().enumerate().map(tier).collect()
}

fn markup(rows: &[(i64, i64, i64)], part: Option<i64>) -> Result<MatrixResult, MatrixError> {
    work_out(RateKind::Markup, &tiers(rows), part.map(cents))
}

fn margin(rows: &[(i64, i64, i64)], part: Option<i64>) -> Result<MatrixResult, MatrixError> {
    work_out(RateKind::Margin, &tiers(rows), part.map(cents))
}

/// The three example matrices the parts matrix page offers as presets.
const SLIDING: [(i64, i64, i64); 6] = [
    (500, 150_000, 5_000),
    (2_500, 100_000, 20_000),
    (10_000, 80_000, 35_000),
    (25_000, 60_000, 25_000),
    (50_000, 50_000, 10_000),
    (100_000, 40_000, 5_000),
];
const STEEPER: [(i64, i64, i64); 6] = [
    (500, 200_000, 15_000),
    (2_500, 120_000, 30_000),
    (10_000, 70_000, 30_000),
    (25_000, 50_000, 15_000),
    (50_000, 35_000, 7_000),
    (100_000, 25_000, 3_000),
];
const FLAT: [(i64, i64, i64); 1] = [(100_000, 100_000, 100_000)];

#[test]
fn each_tier_has_its_range_its_markup_its_margin_and_a_sample_price() {
    let result = markup(&SLIDING, None).unwrap();
    let rows: Vec<_> = result
        .tiers
        .iter()
        .map(|tier| {
            (
                tier.row,
                tier.cost_from.cents(),
                tier.cost_up_to.map(Money::cents),
                tier.margin.thousandths(),
                tier.sample_price.cents(),
                tier.sample_profit.cents(),
            )
        })
        .collect();
    assert_eq!(
        rows,
        [
            (1, 0, Some(500), 60_000, 1_250, 750),
            (2, 501, Some(2_500), 50_000, 5_000, 2_500),
            (3, 2_501, Some(10_000), 44_444, 18_000, 8_000),
            (4, 10_001, Some(25_000), 37_500, 40_000, 15_000),
            (5, 25_001, Some(50_000), 33_333, 75_000, 25_000),
            (6, 50_001, None, 28_571, 140_000, 40_000),
        ]
    );
    let last = result.tiers[5];
    assert_eq!(last.markup, Percent::whole(40));
    assert_eq!(last.sample_cost, cents(100_000));
    assert_eq!(last.share, Some(Percent::whole(5)));
    assert_eq!(result.part, None);
}

#[test]
fn the_example_matrices_land_where_the_page_says() {
    let sliding = markup(&SLIDING, None).unwrap().blended_margin.unwrap();
    let steeper = markup(&STEEPER, None).unwrap().blended_margin.unwrap();
    let flat = markup(&FLAT, None).unwrap().blended_margin.unwrap();
    assert_eq!(sliding, Percent::from_thousandths(43_662));
    assert_eq!(steeper, Percent::from_thousandths(49_418));
    assert_eq!(PARTS.standing(sliding), Standing::Inside);
    assert_eq!(PARTS.standing(steeper), Standing::Inside);
    assert_eq!(flat, PARTS.usual);
    assert_eq!(margin_to_markup(PARTS.usual), Some(Percent::whole(100)));
}

#[test]
fn a_price_is_rounded_to_the_cent_with_halves_away_from_zero() {
    let price = |cost: i64, rate: i64| {
        let result = markup(&[(100_000, rate, -1)], Some(cost)).unwrap();
        result.part.unwrap().price.cents()
    };
    assert_eq!(price(1, 50_000), 2); // 1.5 cents
    assert_eq!(price(3, 50_000), 5); // 4.5 cents
    assert_eq!(price(1, 49_000), 1); // 1.49 cents
    assert_eq!(price(1, 51_000), 2); // 1.51 cents
    assert_eq!(price(1_999, 37_500), 2_749); // 2,748.625 cents
    assert_eq!(price(0, 50_000), 0);
}

#[test]
fn a_part_is_priced_by_the_first_tier_at_or_above_its_cost() {
    let part = |cost: i64| {
        let part = markup(&SLIDING, Some(cost)).unwrap().part.unwrap();
        (part.row, part.price.cents(), part.profit.cents())
    };
    assert_eq!(part(0), (1, 0, 0));
    assert_eq!(part(500), (1, 1_250, 750));
    assert_eq!(part(501), (2, 1_002, 501));
    assert_eq!(part(4_250), (3, 7_650, 3_400));
}

#[test]
fn a_cost_above_the_last_tier_is_priced_by_the_last_tier() {
    let part = markup(&SLIDING, Some(250_000)).unwrap().part.unwrap();
    assert_eq!(
        (part.row, part.price, part.profit),
        (6, cents(350_000), cents(100_000))
    );
    assert_eq!(part.cost, cents(250_000));
}

#[test]
fn a_price_from_a_margin_is_worked_from_the_margin_itself() {
    // A 30% margin on 70,000.00 is 100,000.00. The markup shown, 42.857%,
    // is rounded, and would give 99,999.90.
    let result = margin(&[(7_000_000, 30_000, -1)], Some(7_000_000)).unwrap();
    assert_eq!(result.tiers[0].markup, Percent::from_thousandths(42_857));
    assert_eq!(result.tiers[0].margin, Percent::whole(30));
    assert_eq!(result.tiers[0].sample_price, cents(10_000_000));
    assert_eq!(result.part.unwrap().price, cents(10_000_000));
    assert_eq!(result.part.unwrap().profit, cents(3_000_000));
}

#[test]
fn a_margin_of_100_percent_or_more_is_refused_at_its_row() {
    for rate in [100_000, 100_001, 250_000] {
        let error = margin(&[(500, 50_000, -1), (2_500, rate, -1)], None).unwrap_err();
        assert_eq!(error, MatrixError::MarginTooHigh { row: 2 });
        assert_eq!(error.row(), Some(2));
        assert_eq!(error.to_string(), "Row 2: a margin must be below 100%.");
    }
    let result = margin(&[(Money::MAX_INPUT.cents(), 99_999, -1)], None).unwrap();
    assert_eq!(
        result.tiers[0].markup,
        Percent::from_thousandths(9_999_900_000)
    );
    assert_eq!(result.tiers[0].sample_price, cents(999_999_999_900_000));
}

#[test]
fn tiers_that_do_not_rise_are_refused_at_the_row_that_does_not() {
    for second in [2_500, 2_499, 0] {
        let rows = [(500, 50_000, -1), (2_500, 40_000, -1), (second, 30_000, -1)];
        let error = markup(&rows, None).unwrap_err();
        assert_eq!(error, MatrixError::NotRising { row: 3 });
        assert_eq!(error.row(), Some(3));
        assert_eq!(
            error.to_string(),
            "Row 3: \"cost up to\" must be higher than in the row above."
        );
    }
}

#[test]
fn a_refusal_names_the_row_of_the_form_not_the_place_in_the_list() {
    let mut rows = tiers(&[(500, 50_000, -1), (400, 40_000, -1)]);
    rows[1].row = 7;
    let error = work_out(RateKind::Markup, &rows, None).unwrap_err();
    assert_eq!(error, MatrixError::NotRising { row: 7 });
}

#[test]
fn a_matrix_needs_one_tier_and_has_at_most_eight() {
    let error = markup(&[], None).unwrap_err();
    assert_eq!(error, MatrixError::NoTiers);
    assert_eq!(error.row(), None);
    assert_eq!(
        error.to_string(),
        "Enter at least one tier: a cost and a percent."
    );

    let rows: Vec<_> = (1..=9).map(|row| (row * 100, 50_000, -1)).collect();
    assert_eq!(MAX_TIERS, 8);
    assert!(markup(&rows[..8], None).is_ok());
    let error = markup(&rows, None).unwrap_err();
    assert_eq!(error, MatrixError::TooManyTiers);
    assert_eq!(error.to_string(), "A matrix has at most 8 tiers.");
}

#[test]
fn a_figure_below_zero_is_refused_at_its_row() {
    let mut cost = tiers(&[(500, 50_000, -1), (900, 40_000, -1)]);
    cost[1].cost_up_to = cents(-1);
    let mut rate = tiers(&[(500, 50_000, -1), (900, 40_000, -1)]);
    rate[1].rate = Percent::from_thousandths(-1);
    let mut share = tiers(&[(500, 50_000, 100_000), (900, 40_000, 0)]);
    share[1].share = Some(Percent::from_thousandths(-1));
    for rows in [cost, rate, share] {
        let error = work_out(RateKind::Markup, &rows, None).unwrap_err();
        assert_eq!(error, MatrixError::BelowZero { row: 2 });
        assert_eq!(error.row(), Some(2));
        assert_eq!(error.to_string(), "Row 2: a figure is below zero.");
    }
    let error = markup(&FLAT, Some(-1)).unwrap_err();
    assert_eq!(error, MatrixError::PartCostBelowZero);
    assert_eq!(error.to_string(), "The part's cost is below zero.");
}

#[test]
fn the_blended_margin_is_total_profit_over_total_sales() {
    // Half the spend at a 100% markup and half at 50%: sales are
    // 50 × 2 + 50 × 1.5 = 175 and profit is 75, so the margin is 42.857%.
    let rows = [(5_000, 100_000, 50_000), (10_000, 50_000, 50_000)];
    let result = markup(&rows, None).unwrap();
    assert_eq!(
        result.blended_margin,
        Some(Percent::from_thousandths(42_857))
    );
    // The same tiers given as margins of 50% and 33.333%.
    let rows = [(5_000, 50_000, 50_000), (10_000, 33_333, 50_000)];
    let result = margin(&rows, None).unwrap();
    assert_eq!(
        result.blended_margin,
        Some(Percent::from_thousandths(42_857))
    );
}

#[test]
fn with_no_share_there_is_no_blended_margin_and_a_missing_share_is_zero() {
    let rows = [(5_000, 100_000, -1), (10_000, 50_000, -1)];
    assert_eq!(markup(&rows, None).unwrap().blended_margin, None);
    let rows = [(5_000, 100_000, 100_000), (10_000, 50_000, -1)];
    assert_eq!(
        markup(&rows, None).unwrap().blended_margin,
        Some(Percent::whole(50))
    );
}

#[test]
fn shares_of_a_third_each_are_refused_with_their_total() {
    let third = [
        (500, 50_000, 33_333),
        (900, 50_000, 33_333),
        (1_500, 50_000, 33_333),
    ];
    let error = markup(&third, None).unwrap_err();
    let total = Percent::from_thousandths(99_999);
    assert_eq!(error, MatrixError::SharesNotHundred { total });
    assert_eq!(error.row(), None);
    assert_eq!(
        error.to_string(),
        "The shares of parts spend add to 99.999%. They must add to 100%."
    );
    let exact = [
        (500, 50_000, 33_333),
        (900, 50_000, 33_333),
        (1_500, 50_000, 33_334),
    ];
    assert_eq!(
        markup(&exact, None).unwrap().blended_margin,
        Some(Percent::from_thousandths(33_333))
    );
    let over = [(500, 50_000, 60_000), (900, 50_000, 60_000)];
    let total = Percent::whole(120);
    assert_eq!(
        markup(&over, None).unwrap_err(),
        MatrixError::SharesNotHundred { total }
    );
}

#[test]
fn a_margin_becomes_a_markup_and_comes_back_for_every_whole_percent() {
    for percent in 0..100 {
        let start = Percent::whole(percent);
        let markup = margin_to_markup(start).unwrap();
        assert_eq!(markup_to_margin(markup), Some(start), "{percent}%");
    }
    assert_eq!(
        margin_to_markup(Percent::whole(50)),
        Some(Percent::whole(100))
    );
    assert_eq!(
        margin_to_markup(Percent::whole(20)),
        Some(Percent::whole(25))
    );
    assert_eq!(margin_to_markup(Percent::whole(100)), None);
    assert_eq!(margin_to_markup(Percent::from_thousandths(-1)), None);
}

#[test]
fn a_markup_becomes_a_margin_and_comes_back_within_a_tenth_of_a_percent() {
    // A margin is kept to a thousandth of a percent, which is not enough to
    // return every markup exactly: 50% is a margin of 33.333%, and that is
    // a markup of 49.999%.
    for percent in 0..=1_000 {
        let start = Percent::whole(percent);
        let back = margin_to_markup(markup_to_margin(start).unwrap()).unwrap();
        let apart = (back.thousandths() - start.thousandths()).abs();
        assert!(apart < 100, "{percent}% came back as {back}");
    }
    assert_eq!(
        markup_to_margin(Percent::whole(100)),
        Some(Percent::whole(50))
    );
    assert_eq!(
        markup_to_margin(Percent::whole(50)),
        Some(Percent::from_thousandths(33_333))
    );
    assert_eq!(markup_to_margin(Percent::ZERO), Some(Percent::ZERO));
    assert_eq!(markup_to_margin(Percent::from_thousandths(-1)), None);
}

#[test]
fn the_largest_inputs_give_a_price_and_a_larger_one_is_too_large() {
    let most = Money::MAX_INPUT.cents();
    let result = markup(&[(most, 1_000_000, 100_000)], Some(most)).unwrap();
    assert_eq!(result.part.unwrap().price, cents(109_999_999_989));
    assert_eq!(
        result.blended_margin,
        Some(Percent::from_thousandths(90_909))
    );

    let error = markup(&[(i64::MAX, 1_000_000, -1)], None).unwrap_err();
    assert_eq!(error, MatrixError::TooLarge(TooLarge));
    assert_eq!(error.to_string(), "The result is too large to show.");
    let error = markup(&[(i64::MAX - 1, 0, -1), (i64::MAX, 0, -1)], None);
    assert!(error.is_ok());
    // Shares and markups far beyond any form: refused, with no overflow.
    let rows = [
        (500, i64::MAX, i64::MAX),
        (900, i64::MAX, i64::MAX),
        (950, i64::MAX, i64::MAX),
    ];
    let total = Percent::from_thousandths(i64::MAX);
    assert_eq!(
        markup(&rows, None).unwrap_err(),
        MatrixError::SharesNotHundred { total }
    );
    assert!(markup(&rows[..1], Some(i64::MAX)).is_err());
}
