//! What the gross profit page shows for a period that was worked out.
//!
//! Every figure comes from `shop-math` and is written with its `Display`.
//! The same table and sentences are the worked example of the Markdown
//! version, so the two cannot disagree.

use shop_math::gross_profit::{GrossProfit, LineResult};
use shop_math::targets::{LABOR, OVERALL, PARTS, Target};

use crate::site::markdown::Table;
use crate::site::tools::target::TargetView;

/// What each target is a target for, in the words of a sentence.
pub const LABOR_TARGET_OF: &str = "labor gross profit";
pub const PARTS_TARGET_OF: &str = "parts gross profit";
pub const OVERALL_TARGET_OF: &str = "gross profit across labor, parts and sublet";

/// What the page says of the line that has no target.
pub const SUBLET_ALONE: &str = "Sublet has no target here, so its figure is shown alone.";

/// The result, in the order the page shows it.
#[derive(Debug, Clone)]
pub struct Shown {
    /// One row for each of labor, parts and sublet, and one for all three.
    pub table: Table,
    /// Labor gross profit beside its target, when labor had sales.
    pub labor: Option<TargetView>,
    /// What labor would have to sell at the same cost, when it is below
    /// its range.
    pub labor_short: Option<String>,
    pub parts: Option<TargetView>,
    pub parts_short: Option<String>,
    /// The three lines together beside the overall target.
    pub overall: Option<TargetView>,
}

fn row(name: &str, line: &LineResult) -> Vec<String> {
    vec![
        name.to_owned(),
        line.sales.to_string(),
        line.cost.to_string(),
        line.profit.to_string(),
        line.percent
            .map_or_else(|| "No sales".to_owned(), |percent| percent.to_string()),
    ]
}

fn view(label: &str, of: &'static str, line: &LineResult, target: &Target) -> Option<TargetView> {
    line.percent
        .map(|percent| TargetView::new(label, percent, of, target))
}

/// The one figure an owner can act on: what a line below its range would
/// have to sell, at the same cost, to reach the usual target.
fn short(line_is: &str, cost_of: &str, line: &LineResult, target: &Target) -> Option<String> {
    let short = line.shortfall?;
    Some(format!(
        "{line_is} below the typical range. At the same {cost_of} of {}, sales of {} would reach the usual target of {}. That is {} more than the {} sold.",
        line.cost, short.sales_needed, target.usual, short.difference, line.sales
    ))
}

pub fn shown(result: &GrossProfit) -> Shown {
    Shown {
        table: Table {
            caption: "Gross profit for the period".to_owned(),
            head: ["Line", "Sales", "Cost", "Gross profit", "Percent of sales"]
                .map(str::to_owned)
                .to_vec(),
            rows: vec![
                row("Labor", &result.labor),
                row("Parts", &result.parts),
                row("Sublet", &result.sublet),
                row("All three", &result.overall),
            ],
            // Five columns of figures: each is kept on one line, and the
            // table scrolls in its own box on a narrow screen.
            prose: true,
        },
        labor: view("Labor gross profit", LABOR_TARGET_OF, &result.labor, &LABOR),
        labor_short: short("Labor is", "technician cost", &result.labor, &LABOR),
        parts: view("Parts gross profit", PARTS_TARGET_OF, &result.parts, &PARTS),
        parts_short: short("Parts are", "parts cost", &result.parts, &PARTS),
        overall: view(
            "Overall gross profit",
            OVERALL_TARGET_OF,
            &result.overall,
            &OVERALL,
        ),
    }
}

/// How the page works its figures out, in words and symbols. The page and
/// its Markdown version both show these lines.
pub fn formulas() -> Vec<String> {
    [
        "Gross profit = sales \u{2212} cost. As a percent of sales: gross profit \u{f7} sales. A line with no sales has no percent.",
        "For labor, the cost is what the technicians are paid, with what the shop pays on top of wages. For parts and for sublet, it is what the shop paid for them.",
        "The line for all three is labor, parts and sublet added together: total sales \u{2212} total cost, and that \u{f7} total sales.",
        "For labor or parts below its typical range: sales needed = cost \u{f7} (1 \u{2212} usual target). The difference is the sales needed \u{2212} the sales made.",
        "Sales, cost and gross profit are exact to the cent, so the line for all three is the sum of the three lines as they are shown.",
    ]
    .map(str::to_owned)
    .to_vec()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::site::tools::field::Sent;
    use crate::site::tools::gross_profit::form::{Form, example};

    fn shown_for(sent: &Sent) -> Shown {
        let (form, result) = Form::read(sent);
        assert!(form.troubles().is_empty(), "{:?}", form.troubles());
        shown(&result.unwrap())
    }

    fn rows(shown: &Shown) -> Vec<String> {
        shown.table.rows.iter().map(|row| row.join(" | ")).collect()
    }

    #[test]
    fn the_example_is_shown_as_plan_one_worked_it() {
        let shown = shown_for(&example());
        assert_eq!(shown.table.caption, "Gross profit for the period");
        assert_eq!(
            shown.table.head,
            ["Line", "Sales", "Cost", "Gross profit", "Percent of sales"]
        );
        assert!(shown.table.prose);
        assert_eq!(
            rows(&shown),
            [
                "Labor | $48,000.00 | $18,000.00 | $30,000.00 | 62.5%",
                "Parts | $40,000.00 | $26,000.00 | $14,000.00 | 35%",
                "Sublet | $2,000.00 | $1,600.00 | $400.00 | 20%",
                "All three | $90,000.00 | $45,600.00 | $44,400.00 | 49.333%",
            ]
        );
        assert_eq!(
            shown.labor.unwrap().sentence(),
            "Labor gross profit: 62.5%. That is inside the typical range of 60% to 75% for labor gross profit. The usual target is 70%."
        );
        // Labor is inside its range, so there is nothing to act on there.
        assert!(shown.labor_short.is_none());
        assert_eq!(
            shown.parts.unwrap().sentence(),
            "Parts gross profit: 35%. That is below the typical range of 40% to 50% for parts gross profit. The usual target is 50%."
        );
        assert_eq!(
            shown.parts_short.as_deref(),
            Some(
                "Parts are below the typical range. At the same parts cost of $26,000.00, sales of $52,000.00 would reach the usual target of 50%. That is $12,000.00 more than the $40,000.00 sold."
            )
        );
        assert_eq!(
            shown.overall.unwrap().sentence(),
            "Overall gross profit: 49.333%. That is below the typical range of 50% to 60% for gross profit across labor, parts and sublet. The usual target is 60%."
        );
    }

    #[test]
    fn a_line_with_no_sales_shows_no_percent_and_no_target() {
        // A shop that sold labor only, and left sublet empty.
        let shown = shown_for(&Sent::of(&[
            ("labor_sales", "48000"),
            ("technician_cost", "18000"),
            ("parts_sales", "0"),
            ("parts_cost", "0"),
        ]));
        assert_eq!(
            rows(&shown),
            [
                "Labor | $48,000.00 | $18,000.00 | $30,000.00 | 62.5%",
                "Parts | $0.00 | $0.00 | $0.00 | No sales",
                "Sublet | $0.00 | $0.00 | $0.00 | No sales",
                "All three | $48,000.00 | $18,000.00 | $30,000.00 | 62.5%",
            ]
        );
        assert!(shown.labor.is_some() && shown.overall.is_some());
        assert!(shown.parts.is_none() && shown.parts_short.is_none());
        // A cost with nothing sold is a loss with no percent.
        let shown = shown_for(&Sent::of(&[
            ("labor_sales", "0"),
            ("technician_cost", "5000"),
            ("parts_sales", "40000"),
            ("parts_cost", "20000"),
        ]));
        assert_eq!(
            shown.table.rows[0],
            ["Labor", "$0.00", "$5,000.00", "-$5,000.00", "No sales"]
        );
        assert!(shown.labor.is_none() && shown.labor_short.is_none());
        assert_eq!(shown.overall.unwrap().figure, "37.5%");
    }

    #[test]
    fn a_line_below_its_range_says_what_the_same_cost_would_need_to_sell() {
        // Labor that cost more than it sold for: a percent below zero, and
        // still the sales the usual target would take.
        let shown = shown_for(&Sent::of(&[
            ("labor_sales", "10000"),
            ("technician_cost", "12000"),
            ("parts_sales", "30000"),
            ("parts_cost", "12000"),
        ]));
        assert_eq!(
            shown.table.rows[0],
            ["Labor", "$10,000.00", "$12,000.00", "-$2,000.00", "-20%"]
        );
        let labor = shown.labor.unwrap();
        assert!(labor.sentence().contains("That is below the typical range"));
        assert_eq!(
            shown.labor_short.as_deref(),
            Some(
                "Labor is below the typical range. At the same technician cost of $12,000.00, sales of $40,000.00 would reach the usual target of 70%. That is $30,000.00 more than the $10,000.00 sold."
            )
        );
        // Parts above their range: said in words, and nothing to act on.
        let parts = shown.parts.unwrap();
        assert_eq!(parts.figure, "60%");
        assert!(parts.sentence().contains("That is above the typical range"));
        assert!(shown.parts_short.is_none());
        // The overall line has a target and no single cost to act on.
        assert_eq!(shown.overall.unwrap().figure, "40%");
    }

    #[test]
    fn the_formulas_are_stated_in_words_and_symbols() {
        let lines = formulas();
        assert_eq!(lines.len(), 5);
        let text = lines.join("\n");
        for formula in [
            "Gross profit = sales \u{2212} cost",
            "gross profit \u{f7} sales",
            "total sales \u{2212} total cost",
            "sales needed = cost \u{f7} (1 \u{2212} usual target)",
            "A line with no sales has no percent.",
        ] {
            assert!(text.contains(formula), "{formula}");
        }
        assert_eq!(
            SUBLET_ALONE,
            "Sublet has no target here, so its figure is shown alone."
        );
    }
}
