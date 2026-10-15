//! What the technician pay page shows for a pay that was worked out, and
//! the words the page and its Markdown version both say.
//!
//! The gross pay is the `shop-math` figure, written with its `Display`.
//! The deductions are the payroll engine's table, so the two pages cannot
//! disagree on a cent.

use crate::site::markdown::Table;
use crate::site::tools::payroll_deductions::result as payroll;
use crate::site::tools::technician_pay::form;

/// One line of a result table: what it is, and its amount.
fn line(name: impl Into<String>, amount: impl std::fmt::Display) -> Vec<String> {
    vec![name.into(), amount.to_string()]
}

/// "Gross pay": the three parts and the total, in whole cents.
pub fn gross(worked: &form::Worked) -> Table {
    let g = &worked.gross;
    Table {
        caption: "Gross pay".to_owned(),
        head: ["Part", "Amount"].map(str::to_owned).to_vec(),
        rows: vec![
            line("Flagged hours", g.flagged),
            line("Hourly hours", g.hourly),
            line("Overtime hours", g.overtime),
            line("Gross pay", g.total),
        ],
        prose: false,
    }
}

/// "Deductions from one <frequency> pay": the payroll engine's table on
/// the gross, when the pay frequency was given.
pub fn deductions(worked: &form::Worked) -> Option<Table> {
    worked.deductions.as_ref().map(payroll::table)
}

/// The page's "How this is worked out", in order.
pub fn formulas() -> Vec<String> {
    [
        "The gross pay is worked out in whole cents: the flagged hours times the flat rate per flagged hour, plus the hourly hours times the hourly rate, plus the overtime hours times the overtime rate. Each part is rounded to the cent before the total.",
        "The Ontario deductions are the payroll calculator's: the federal and Ontario income tax, CPP, CPP2 and EI on the gross pay, by CRA's T4127 formulas. Leave the pay frequency empty for the gross only.",
        "A flagged hour is one the shop bills as a flat rate, not by the clock. An overtime hour is paid at the overtime rate, which a shop usually sets at the hourly rate times one and a half.",
    ]
    .map(str::to_owned)
    .to_vec()
}

/// The date the example is worked out on: the first day of the newest
/// edition the tables cover.
pub fn example_date() -> chrono::NaiveDate {
    let held = payroll_calc::editions();
    held[held.len() - 1].effective
}

/// The worked example, worked out on the first day of the newest edition.
pub fn example() -> form::Worked {
    let (_, worked) = form::Form::read(&form::example(), example_date());
    worked.expect("the example works out")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_gross_is_shown_part_by_part() {
        let worked = example();
        let table = gross(&worked);
        assert_eq!(table.caption, "Gross pay");
        let rows: Vec<String> = table.rows.iter().map(|row| row.join(" | ")).collect();
        assert_eq!(
            rows,
            [
                "Flagged hours | $2,560.00",
                "Hourly hours | $150.00",
                "Overtime hours | $0.00",
                "Gross pay | $2,710.00",
            ]
        );
    }

    #[test]
    fn the_deductions_are_the_payrolls_table_on_the_gross() {
        let worked = example();
        let table = deductions(&worked).unwrap();
        assert!(
            table
                .caption
                .starts_with("Deductions from one biweekly pay")
        );
        // The gross of the deductions is the gross pay.
        assert!(table.rows[0][1].contains("$2,710.00"));
    }

    #[test]
    fn the_words_are_stated() {
        assert_eq!(formulas().len(), 3);
    }
}
