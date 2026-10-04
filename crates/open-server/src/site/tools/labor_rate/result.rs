//! What the labor rate page shows for each of its two results.
//!
//! Every figure comes from `shop-math` and is written with its `Display`.
//! The same tables and sentences are the worked example of the Markdown
//! version, so the two cannot disagree.

use std::cmp::Ordering;

use shop_math::Money;
use shop_math::labor_rate::{EffectiveRate, RateNeeded};
use shop_math::targets::LABOR;

use crate::site::markdown::Table;
use crate::site::tools::target::TargetView;

/// What the labor target is a target for, in the words of a sentence.
pub const LABOR_TARGET_OF: &str = "labor gross profit";

/// Why a shop does not get the rate it posts. It is one of the lines under
/// "How this is worked out".
pub const WHY_THEY_DIFFER: &str = "The posted rate and the effective rate differ because not every hour billed is sold at the posted rate: discounts, warranty and internal work, and jobs sold at a menu price all change what an hour brings in.";

/// A table of two columns: what a figure is, and the figure.
fn figures(caption: &str, rows: Vec<(&str, String)>) -> Table {
    Table {
        caption: caption.to_owned(),
        head: vec!["Figure".to_owned(), "Amount".to_owned()],
        rows: rows
            .into_iter()
            .map(|(name, amount)| vec![name.to_owned(), amount])
            .collect(),
        // Two columns share the width and wrap, so nothing scrolls.
        prose: false,
    }
}

/// The rate the shop needs, in the order the page shows it.
#[derive(Debug, Clone)]
pub struct NeededShown {
    pub table: Table,
    /// Why the rate needed is nothing, when gross profit on parts covers
    /// every cost.
    pub covered: Option<String>,
    /// Labor gross profit at the rate needed, beside the labor target.
    pub gross_profit: Option<TargetView>,
    /// Which of the rate needed and the rate from the target is higher,
    /// and what that means.
    pub compared: String,
}

pub fn needed(result: &RateNeeded) -> NeededShown {
    let table = figures(
        "The rate the shop needs",
        vec![
            ("Hours billed per month", result.hours_billed.to_string()),
            (
                "Labor sales needed per month",
                result.labor_sales_needed.to_string(),
            ),
            (
                "Rate needed, per hour billed",
                result.rate_needed.to_string(),
            ),
            (
                "Break-even rate, with no profit",
                result.break_even_rate.to_string(),
            ),
            (
                "Rate from the target labor gross profit",
                result.target_rate.to_string(),
            ),
        ],
    );
    let covered = result.covered_by_parts.then(|| {
        format!(
            "Gross profit on parts covers the technician cost and the overhead, so labor has nothing left to cover and the rate needed is {}.",
            result.rate_needed
        )
    });
    let compared = match result.needed_against_target {
        Ordering::Greater => {
            "The rate needed is the higher of the two rates: the shop's overhead asks for more than the target labor gross profit gives."
        }
        Ordering::Less => {
            "The rate needed is the lower of the two rates: the target labor gross profit leaves more profit than was asked for."
        }
        Ordering::Equal => {
            "The rate needed and the rate from the target labor gross profit are the same."
        }
    };
    NeededShown {
        table,
        covered,
        gross_profit: result.labor_gross_profit.map(|percent| {
            TargetView::new(
                "Labor gross profit at the rate needed",
                percent,
                LABOR_TARGET_OF,
                &LABOR,
            )
        }),
        compared: compared.to_owned(),
    }
}

/// The rate the shop is getting, in the order the page shows it.
#[derive(Debug, Clone)]
pub struct GettingShown {
    pub table: Table,
    /// Whether the shop gets less or more than it posts, when a posted
    /// rate was given.
    pub posted: Option<String>,
    /// Labor gross profit for the period, beside the labor target, when a
    /// technician cost was given and there were labor sales.
    pub gross_profit: Option<TargetView>,
}

pub fn getting(result: &EffectiveRate) -> GettingShown {
    let mut rows = vec![(
        "Effective labor rate, per hour billed",
        result.rate.to_string(),
    )];
    let mut posted = None;
    if let Some(against) = &result.against_posted {
        rows.push((
            "Effective rate as a percent of the posted rate",
            against.percent_of_posted.to_string(),
        ));
        rows.push((
            "Posted rate less effective rate, per hour",
            against.per_hour.to_string(),
        ));
        rows.push((
            "Posted rate × hours billed, less labor sales",
            against.over_the_period.to_string(),
        ));
        posted = Some(match against.per_hour.cmp(&Money::ZERO) {
            Ordering::Greater => "The shop is getting less for an hour billed than it posts.".to_owned(),
            Ordering::Less => "The shop is getting more for an hour billed than it posts, so the two differences are below zero.".to_owned(),
            Ordering::Equal => "The shop is getting the rate it posts.".to_owned(),
        });
    }
    GettingShown {
        table: figures("The rate the shop is getting", rows),
        posted,
        gross_profit: result
            .labor_gross_profit
            .map(|percent| TargetView::new("Labor gross profit", percent, LABOR_TARGET_OF, &LABOR)),
    }
}

/// How the page works its figures out, in words and symbols. The page and
/// its Markdown version both show these lines.
pub fn formulas() -> Vec<String> {
    [
        "Hours billed = technicians \u{d7} paid hours per technician \u{d7} productivity. Productivity is hours billed as a percent of hours paid.",
        "Labor sales needed = (technician cost + overhead \u{2212} gross profit on parts) \u{f7} (1 \u{2212} target profit). Rate needed = labor sales needed \u{f7} hours billed. The break-even rate is the same with a target profit of zero.",
        "Labor gross profit = (labor sales \u{2212} technician cost) \u{f7} labor sales. Technician cost is wages and what the shop pays on top of wages.",
        "Rate from the target labor gross profit = technician cost \u{f7} hours billed \u{f7} (1 \u{2212} target labor gross profit). It needs no overhead figure, so the page shows it beside the rate needed and says which is the higher.",
        "Effective labor rate = labor sales \u{f7} hours billed, for one period. As a percent of the posted rate it is effective rate \u{f7} posted rate. Over the period, the difference is posted rate \u{d7} hours billed \u{2212} labor sales.",
        WHY_THEY_DIFFER,
        "Each amount is rounded to the nearest cent, once, where it is shown.",
    ]
    .map(str::to_owned)
    .to_vec()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::site::tools::field::Sent;
    use crate::site::tools::labor_rate::form::{Form, example};

    fn shown_for(sent: &Sent) -> (Option<NeededShown>, Option<GettingShown>) {
        let (form, rate_needed, effective) = Form::read(sent);
        assert!(form.needed.troubles().is_empty() && form.getting.troubles().is_empty());
        (
            rate_needed.as_ref().map(needed),
            effective.as_ref().map(getting),
        )
    }

    fn rows(table: &Table) -> Vec<String> {
        table.rows.iter().map(|row| row.join(" | ")).collect()
    }

    #[test]
    fn the_example_is_shown_as_plan_one_worked_it() {
        let (needed, getting) = shown_for(&example());
        let needed = needed.unwrap();
        assert_eq!(needed.table.caption, "The rate the shop needs");
        assert_eq!(needed.table.head, ["Figure", "Amount"]);
        assert!(!needed.table.prose);
        assert_eq!(
            rows(&needed.table),
            [
                "Hours billed per month | 441.15",
                "Labor sales needed per month | $34,444.44",
                "Rate needed, per hour billed | $78.08",
                "Break-even rate, with no profit | $70.27",
                "Rate from the target labor gross profit | $136.01",
            ]
        );
        assert!(needed.covered.is_none());
        assert_eq!(
            needed.gross_profit.unwrap().sentence(),
            "Labor gross profit at the rate needed: 47.742%. That is below the typical range of 60% to 75% for labor gross profit. The usual target is 70%."
        );
        assert_eq!(
            needed.compared,
            "The rate needed is the lower of the two rates: the target labor gross profit leaves more profit than was asked for."
        );
        let getting = getting.unwrap();
        assert_eq!(getting.table.caption, "The rate the shop is getting");
        assert_eq!(
            rows(&getting.table),
            [
                "Effective labor rate, per hour billed | $92.31",
                "Effective rate as a percent of the posted rate | 76.925%",
                "Posted rate less effective rate, per hour | $27.69",
                "Posted rate × hours billed, less labor sales | $14,400.00",
            ]
        );
        assert_eq!(
            getting.posted.as_deref(),
            Some("The shop is getting less for an hour billed than it posts.")
        );
        assert_eq!(
            getting.gross_profit.unwrap().sentence(),
            "Labor gross profit: 62.5%. That is inside the typical range of 60% to 75% for labor gross profit. The usual target is 70%."
        );
    }

    #[test]
    fn parts_that_cover_every_cost_leave_a_rate_of_nothing_and_the_page_says_why() {
        let (needed, getting) = shown_for(&Sent::of(&[
            ("technicians", "2"),
            ("technician_cost", "6000"),
            ("overhead", "9000"),
            ("parts_profit", "15000"),
        ]));
        assert!(getting.is_none());
        let needed = needed.unwrap();
        assert_eq!(
            needed.covered.as_deref(),
            Some(
                "Gross profit on parts covers the technician cost and the overhead, so labor has nothing left to cover and the rate needed is $0.00."
            )
        );
        assert_eq!(needed.table.rows[1][1], "$0.00");
        assert_eq!(needed.table.rows[2][1], "$0.00");
        assert_eq!(needed.table.rows[3][1], "$0.00");
        // With no labor sales there is no percent to show beside the target.
        assert!(needed.gross_profit.is_none());
        assert!(needed.compared.contains("the lower of the two rates"));
    }

    #[test]
    fn the_page_says_which_of_the_two_rates_is_the_higher() {
        // Overhead that asks for more than the target gives.
        let (needed, _) = shown_for(&Sent::of(&[
            ("technicians", "3"),
            ("technician_cost", "18000"),
            ("overhead", "60000"),
        ]));
        let needed = needed.unwrap();
        assert_eq!(needed.table.rows[2][1], "$196.46");
        assert_eq!(needed.table.rows[4][1], "$136.01");
        assert_eq!(
            needed.compared,
            "The rate needed is the higher of the two rates: the shop's overhead asks for more than the target labor gross profit gives."
        );
        let profit = needed.gross_profit.unwrap();
        assert_eq!(profit.figure, "79.231%");
        assert!(
            profit
                .sentence()
                .contains("That is above the typical range")
        );
        // Overhead of twice the technician cost, a profit of a tenth and a
        // target of seven tenths ask for the same rate.
        let (needed, _) = shown_for(&Sent::of(&[
            ("technicians", "1"),
            ("paid_hours", "100"),
            ("productivity", "100"),
            ("technician_cost", "10000"),
            ("overhead", "20000"),
        ]));
        let needed = needed.unwrap();
        assert_eq!(needed.table.rows[2][1], "$333.33");
        assert_eq!(needed.table.rows[4][1], "$333.33");
        assert_eq!(
            needed.compared,
            "The rate needed and the rate from the target labor gross profit are the same."
        );
    }

    #[test]
    fn a_shop_that_gets_more_than_it_posts_is_told_so_and_the_differences_are_below_zero() {
        let (_, getting) = shown_for(&Sent::of(&[
            ("labor_sales", "52000"),
            ("hours_billed", "400"),
            ("posted_rate", "120"),
        ]));
        let getting = getting.unwrap();
        assert_eq!(
            rows(&getting.table),
            [
                "Effective labor rate, per hour billed | $130.00",
                "Effective rate as a percent of the posted rate | 108.333%",
                "Posted rate less effective rate, per hour | -$10.00",
                "Posted rate × hours billed, less labor sales | -$4,000.00",
            ]
        );
        assert_eq!(
            getting.posted.as_deref(),
            Some(
                "The shop is getting more for an hour billed than it posts, so the two differences are below zero."
            )
        );
        assert!(getting.gross_profit.is_none());
        // The rate it posts, to the cent.
        let (_, getting) = shown_for(&Sent::of(&[
            ("labor_sales", "48000"),
            ("hours_billed", "400"),
            ("posted_rate", "120"),
        ]));
        assert_eq!(
            getting.unwrap().posted.as_deref(),
            Some("The shop is getting the rate it posts.")
        );
        // With no posted rate and no technician cost there is one figure.
        let (_, getting) = shown_for(&Sent::of(&[
            ("labor_sales", "48000"),
            ("hours_billed", "520"),
        ]));
        let getting = getting.unwrap();
        assert_eq!(
            rows(&getting.table),
            ["Effective labor rate, per hour billed | $92.31"]
        );
        assert!(getting.posted.is_none() && getting.gross_profit.is_none());
        // Technicians who cost more than the labor sold: a percent below zero.
        let (_, getting) = shown_for(&Sent::of(&[
            ("labor_sales", "10000"),
            ("hours_billed", "100"),
            ("period_cost", "12000"),
        ]));
        let profit = getting.unwrap().gross_profit.unwrap();
        assert_eq!(profit.figure, "-20%");
        assert!(
            profit
                .sentence()
                .contains("That is below the typical range")
        );
    }

    #[test]
    fn the_formulas_are_stated_in_words_and_symbols() {
        let lines = formulas();
        assert_eq!(lines.len(), 7);
        let text = lines.join("\n");
        for formula in [
            "Hours billed = technicians \u{d7} paid hours per technician \u{d7} productivity",
            "Labor sales needed = (technician cost + overhead \u{2212} gross profit on parts) \u{f7} (1 \u{2212} target profit)",
            "Rate needed = labor sales needed \u{f7} hours billed",
            "Labor gross profit = (labor sales \u{2212} technician cost) \u{f7} labor sales",
            "Rate from the target labor gross profit = technician cost \u{f7} hours billed \u{f7} (1 \u{2212} target labor gross profit)",
            "Effective labor rate = labor sales \u{f7} hours billed",
            "discounts, warranty and internal work, and jobs sold at a menu price",
        ] {
            assert!(text.contains(formula), "{formula}");
        }
    }
}
