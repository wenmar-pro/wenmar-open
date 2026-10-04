//! What the parts matrix page shows for a matrix that was worked out.
//!
//! Every figure comes from `shop-math` and is written with its `Display`.
//! The same table and sentences are the worked example of the Markdown
//! version, so the two cannot disagree.

use shop_math::parts_matrix::{MatrixResult, PartResult, TierResult, margin_to_markup};
use shop_math::targets::PARTS;

use crate::site::markdown::Table;
use crate::site::tools::target::TargetView;

/// What the parts target is a target for, in the words of a sentence.
pub const PARTS_TARGET_OF: &str = "parts gross profit";

/// The result, in the order the page shows it.
#[derive(Debug, Clone)]
pub struct Shown {
    /// One row for each tier: its range of cost, its markup and margin, and
    /// a sample part at its "cost up to".
    pub table: Table,
    /// That the last row has no upper limit.
    pub last: String,
    /// The one part priced, when a cost was given.
    pub part: Option<String>,
    /// The blended margin beside the parts target, when shares were given.
    pub blended: Option<TargetView>,
}

fn range(tier: &TierResult) -> String {
    match tier.cost_up_to {
        Some(up_to) => format!("{} to {up_to}", tier.cost_from),
        None => format!("{} and up", tier.cost_from),
    }
}

fn table(tiers: &[TierResult]) -> Table {
    let shares = tiers.iter().any(|tier| tier.share.is_some());
    let mut head: Vec<String> = [
        "Row",
        "Cost of the part",
        "Markup",
        "Margin",
        "Sample cost",
        "Sells for",
        "Profit",
    ]
    .map(str::to_owned)
    .to_vec();
    if shares {
        head.push("Share of spend".to_owned());
    }
    let rows = tiers
        .iter()
        .map(|tier| {
            let mut cells = vec![
                tier.row.to_string(),
                range(tier),
                tier.markup.to_string(),
                tier.margin.to_string(),
                tier.sample_cost.to_string(),
                tier.sample_price.to_string(),
                tier.sample_profit.to_string(),
            ];
            if shares {
                cells.push(tier.share.unwrap_or_default().to_string());
            }
            cells
        })
        .collect();
    Table {
        caption: "The matrix".to_owned(),
        head,
        rows,
        // Every column but the last is kept on one line, and the table
        // scrolls in its own box on a narrow screen.
        prose: true,
    }
}

fn last(tiers: &[TierResult]) -> String {
    match tiers.last() {
        Some(tier) if tiers.len() > 1 => format!(
            "Row {} is the last row used, so it has no upper limit: a part that costs {} or more is priced by it, whatever its \"cost up to\" says.",
            tier.row, tier.cost_from
        ),
        Some(tier) => format!(
            "Row {} is the only row used, so it prices every part, whatever its \"cost up to\" says.",
            tier.row
        ),
        None => String::new(),
    }
}

fn part(part: &PartResult) -> String {
    format!(
        "A part that costs {} is in row {}. It sells for {}, a profit of {}.",
        part.cost, part.row, part.price, part.profit
    )
}

pub fn shown(result: &MatrixResult) -> Shown {
    Shown {
        table: table(&result.tiers),
        last: last(&result.tiers),
        part: result.part.as_ref().map(part),
        blended: result
            .blended_margin
            .map(|blended| TargetView::new("Blended margin", blended, PARTS_TARGET_OF, &PARTS)),
    }
}

/// The markup the usual parts target needs, as a sentence: the difference
/// between markup and margin made concrete.
pub fn usual_markup() -> String {
    match margin_to_markup(PARTS.usual) {
        Some(markup) => format!(
            "A margin of {} is a markup of {markup}: to make the usual parts target with one markup on every part, that is the markup it takes.",
            PARTS.usual
        ),
        None => String::new(),
    }
}

/// How the page works its figures out, in words and symbols. The page and
/// its Markdown version both show these lines.
pub fn formulas() -> Vec<String> {
    [
        "Markup is profit as a percent of cost. Margin is profit as a percent of the selling price. The same part has both, and the margin is always the smaller number.",
        "With a markup: price = cost \u{d7} (1 + markup). With a margin: price = cost \u{f7} (1 \u{2212} margin). The price is rounded to the nearest cent, and the profit is the price less the cost.",
        "margin = markup \u{f7} (1 + markup), and markup = margin \u{f7} (1 \u{2212} margin). A margin of 100% or more has no price, so it is refused.",
        "A part belongs to the first row whose \"cost up to\" is at or above its cost. A part that costs more than every row belongs to the last row.",
        "Blended margin = total profit \u{f7} total sales. With a share s of parts spend and a markup m in each row, sales are the sum of s \u{d7} (1 + m) and profit is the sum of s \u{d7} m. A row with no share counts as none, and the shares must add to 100%.",
    ]
    .map(str::to_owned)
    .to_vec()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::site::tools::field::Sent;
    use crate::site::tools::parts_matrix::form::Form;
    use crate::site::tools::parts_matrix::presets::{PRESETS, example};

    fn shown_for(sent: &Sent) -> Shown {
        let (form, result) = Form::read(sent);
        assert!(form.troubles().is_empty());
        shown(&result.unwrap())
    }

    #[test]
    fn the_example_is_shown_as_plan_one_worked_it() {
        let shown = shown_for(&example());
        assert_eq!(shown.table.caption, "The matrix");
        assert_eq!(
            shown.table.head,
            [
                "Row",
                "Cost of the part",
                "Markup",
                "Margin",
                "Sample cost",
                "Sells for",
                "Profit",
                "Share of spend"
            ]
        );
        let rows: Vec<String> = shown.table.rows.iter().map(|row| row.join(" | ")).collect();
        assert_eq!(
            rows,
            [
                "1 | $0.00 to $5.00 | 150% | 60% | $5.00 | $12.50 | $7.50 | 5%",
                "2 | $5.01 to $25.00 | 100% | 50% | $25.00 | $50.00 | $25.00 | 20%",
                "3 | $25.01 to $100.00 | 80% | 44.444% | $100.00 | $180.00 | $80.00 | 35%",
                "4 | $100.01 to $250.00 | 60% | 37.5% | $250.00 | $400.00 | $150.00 | 25%",
                "5 | $250.01 to $500.00 | 50% | 33.333% | $500.00 | $750.00 | $250.00 | 10%",
                "6 | $500.01 and up | 40% | 28.571% | $1,000.00 | $1,400.00 | $400.00 | 5%",
            ]
        );
        assert_eq!(
            shown.last,
            "Row 6 is the last row used, so it has no upper limit: a part that costs $500.01 or more is priced by it, whatever its \"cost up to\" says."
        );
        assert_eq!(
            shown.part.as_deref(),
            Some("A part that costs $42.50 is in row 3. It sells for $76.50, a profit of $34.00.")
        );
        assert_eq!(
            shown.blended.unwrap().sentence(),
            "Blended margin: 43.662%. That is inside the typical range of 40% to 50% for parts gross profit. The usual target is 50%."
        );
    }

    #[test]
    fn a_matrix_with_no_shares_and_no_part_shows_neither() {
        let shown = shown_for(&Sent::of(&[
            ("kind", "margin"),
            ("cost4", "50"),
            ("rate4", "30"),
        ]));
        assert_eq!(shown.table.head.len(), 7);
        assert_eq!(
            shown.table.rows,
            [[
                "4",
                "$0.00 and up",
                "42.857%",
                "30%",
                "$50.00",
                "$71.43",
                "$21.43"
            ]]
        );
        assert_eq!(
            shown.last,
            "Row 4 is the only row used, so it prices every part, whatever its \"cost up to\" says."
        );
        assert!(shown.part.is_none() && shown.blended.is_none());
    }

    #[test]
    fn a_row_with_no_share_among_rows_that_have_one_counts_as_none() {
        let shown = shown_for(&Sent::of(&[
            ("cost1", "10"),
            ("rate1", "100"),
            ("share1", "100"),
            ("cost2", "20"),
            ("rate2", "50"),
        ]));
        assert_eq!(shown.table.rows[1].last().map(String::as_str), Some("0%"));
        assert_eq!(shown.blended.unwrap().figure, "50%");
    }

    #[test]
    fn the_flat_preset_is_the_markup_the_usual_target_needs() {
        let shown = shown_for(&PRESETS[2].sent());
        assert_eq!(shown.table.rows[0][2], "100%");
        assert_eq!(shown.table.rows[0][3], "50%");
        assert_eq!(
            usual_markup(),
            "A margin of 50% is a markup of 100%: to make the usual parts target with one markup on every part, that is the markup it takes."
        );
    }

    #[test]
    fn the_formulas_are_stated_in_words_and_symbols() {
        let lines = formulas();
        assert_eq!(lines.len(), 5);
        let text = lines.join("\n");
        for formula in [
            "price = cost \u{d7} (1 + markup)",
            "price = cost \u{f7} (1 \u{2212} margin)",
            "margin = markup \u{f7} (1 + markup)",
            "markup = margin \u{f7} (1 \u{2212} margin)",
            "Blended margin = total profit \u{f7} total sales",
        ] {
            assert!(text.contains(formula), "{formula}");
        }
    }
}
