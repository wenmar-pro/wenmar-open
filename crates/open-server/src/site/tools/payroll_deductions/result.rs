//! What the Ontario payroll page shows for a pay that was worked out, and
//! the words the page and its Markdown version both say.
//!
//! Every figure is the engine's, written with its `Display`. The same
//! tables and sentences are the worked example of the Markdown version, so
//! the two cannot disagree.

use payroll_calc::{Edition, Worked, money};

use crate::site::markdown::Table;

/// One line of a result table: what it is, and its amount.
fn line(name: impl Into<String>, amount: impl std::fmt::Display) -> Vec<String> {
    vec![name.into(), amount.to_string()]
}

/// "Deductions from one biweekly pay": the pay, each deduction, the total
/// and what is left.
pub fn table(worked: &Worked) -> Table {
    let d = &worked.deductions;
    Table {
        caption: format!(
            "Deductions from one {} pay",
            worked.input.frequency.label().to_lowercase()
        ),
        head: ["Pay", "Amount"].map(str::to_owned).to_vec(),
        rows: vec![
            line("Gross pay", worked.input.gross),
            line("Federal income tax", money(d.federal_tax)),
            line("Ontario income tax", money(d.provincial_tax)),
            line("CPP (base and first additional)", money(d.cpp)),
            line("CPP2 (second additional)", money(d.cpp2)),
            line("EI", money(d.ei)),
            line("Total deductions", money(worked.employee_total)),
            line("Net pay", money(worked.net_pay)),
        ],
        prose: false,
    }
}

/// "What the employer pays on top": the employer's CPP, CPP2 and EI.
pub fn employer(worked: &Worked) -> Table {
    let d = &worked.deductions;
    Table {
        caption: "What the employer pays on top".to_owned(),
        head: ["Employer", "Amount"].map(str::to_owned).to_vec(),
        rows: vec![
            line("CPP", money(d.employer_cpp)),
            line("CPP2", money(d.employer_cpp2)),
            line("EI", money(d.employer_ei)),
            line("Total", money(worked.employer_total)),
        ],
        prose: false,
    }
}

/// "The T4127 factors used": one row per factor in the engine's order, the
/// value the formulas used, and what the factor is.
pub fn factors(worked: &Worked) -> Table {
    Table {
        caption: "The T4127 factors used".to_owned(),
        head: ["Factor", "Value", "What it is"]
            .map(str::to_owned)
            .to_vec(),
        rows: worked
            .deductions
            .factors
            .iter()
            .map(|(name, value)| {
                vec![
                    name.clone(),
                    value.normalize().to_string(),
                    payroll_calc::factors::meaning(name)
                        .unwrap_or("A factor of T4127.")
                        .to_owned(),
                ]
            })
            .collect(),
        prose: true,
    }
}

/// The two claim sentences: the amount used, and whether it is the tables'
/// basic amount or what was typed.
pub fn claims(worked: &Worked) -> Vec<String> {
    let words = |basic: bool| {
        if basic {
            ", the basic personal amount of the tables (claim code 1)."
        } else {
            ", as typed."
        }
    };
    vec![
        format!(
            "Federal claim: {}{}",
            worked.input.federal_claim.amount,
            words(worked.input.federal_claim.basic)
        ),
        format!(
            "Ontario claim: {}{}",
            worked.input.provincial_claim.amount,
            words(worked.input.provincial_claim.basic)
        ),
    ]
}

/// "Tables of CRA's T4127 Payroll Deductions Formulas, 123rd edition, for
/// pay dates from 2026-07-01 to 2026-12-31."
pub fn edition_line(edition: &Edition) -> String {
    format!("Tables of CRA's {}, {}.", edition.title(), edition.span())
}

/// The page's "How this is worked out", in order.
pub fn how() -> Vec<String> {
    [
        "CPP is the employee's base and first additional contribution on the pay above the basic exemption for one pay period, up to the year's maximum. CPP2 is the second additional contribution on pensionable earnings above the year's maximum pensionable earnings, up to the additional maximum. EI is the employee's premium on insurable earnings, up to the year's maximum.",
        "For income tax, the pay is turned into an annual taxable income, A: the number of pay periods times the pay, less the CPP contributions that are deducted from income. The federal and Ontario tax on A is worked out with the brackets and credits of T4127; the Ontario surtax and Health Premium are added and the Ontario tax reduction is taken off. Each year's tax is divided by the number of pay periods and rounded to the cent.",
        "The employer pays the same CPP and CPP2 as the employee, and EI at the multiple of the employee's premium that T4127 sets.",
        "Pensionable and insurable earnings are taken to be the gross pay. Every factor the formulas used is listed with its value.",
    ]
    .map(str::to_owned)
    .to_vec()
}

/// The page's "Check it yourself", in order.
pub fn check() -> Vec<String> {
    [
        "Open CRA's Payroll Deductions Online Calculator (PDOC), accept the terms and choose Salary.",
        "Enter Ontario, the same pay frequency, the same date paid and the same gross salary, with nothing else in the income fields.",
        "Choose claim code 1 for the basic personal amounts, or enter the same claim amounts. Enter the same year-to-date CPP, CPP2, EI and pensionable earnings; leave a zero field blank, as PDOC refuses 0.",
        "PDOC's federal tax, provincial tax, CPP, CPP2 and EI should match this page to the cent. The pays this calculator was compared with are listed in the payroll-ca project.",
    ]
    .map(str::to_owned)
    .to_vec()
}

/// The page's "What this covers", in order.
pub fn scope() -> Vec<String> {
    [
        "Ontario only, by T4127 Option 1, the regular method, for one regular pay. A bonus, retroactive pay or commission is worked out another way and is not covered.",
        "Not taken into account: registered pension plan and RRSP deductions, union dues and other amounts a TD1 lets an employee deduct, additional tax asked for on a TD1, a CPP or EI exemption, and an employee who turns 18 or 70 this year.",
        "An employee whose net income for the year will be high may be able to claim less than the basic federal amount: type the amount from their TD1.",
    ]
    .map(str::to_owned)
    .to_vec()
}

/// The help line above the two claim fields.
pub const CLAIMS_HELP: &str = "Leave a claim empty for the basic personal amount (claim code 1). Type 0 for claim code 0, or the total from the employee's TD1 and TD1ON.";
/// The help line above the year-to-date fields.
pub const YEAR_TO_DATE_HELP: &str = "What was deducted and earned on earlier pays this calendar year. Left empty, each is nothing. They matter once an employee nears the year's maximum.";

#[cfg(test)]
mod tests {
    use chrono::NaiveDate;
    use payroll_calc::deductions;

    use super::*;

    /// The PDOC biweekly row, worked out.
    fn worked() -> Worked {
        let query = payroll_calc::Query {
            frequency: Some("biweekly".to_owned()),
            gross: Some("2400.00".to_owned()),
            pay_date: Some("2026-07-15".to_owned()),
            ..payroll_calc::Query::default()
        };
        deductions(
            &query,
            NaiveDate::parse_from_str("2026-10-14", "%Y-%m-%d").unwrap(),
        )
        .unwrap()
    }

    #[test]
    fn the_result_is_shown_line_by_line_with_its_net_pay() {
        let worked = worked();
        let table = table(&worked);
        assert_eq!(table.caption, "Deductions from one biweekly pay");
        assert_eq!(table.head, ["Pay", "Amount"]);
        let rows: Vec<String> = table.rows.iter().map(|row| row.join(" | ")).collect();
        assert_eq!(
            rows,
            [
                "Gross pay | $2,400.00",
                "Federal income tax | $223.20",
                "Ontario income tax | $122.74",
                "CPP (base and first additional) | $134.79",
                "CPP2 (second additional) | $0.00",
                "EI | $39.12",
                "Total deductions | $519.85",
                "Net pay | $1,880.15",
            ]
        );
        let employer = employer(&worked);
        assert_eq!(employer.caption, "What the employer pays on top");
        let rows: Vec<String> = employer.rows.iter().map(|row| row.join(" | ")).collect();
        assert_eq!(
            rows,
            [
                "CPP | $134.79",
                "CPP2 | $0.00",
                "EI | $54.77",
                "Total | $189.56",
            ]
        );
    }

    #[test]
    fn every_factor_is_listed_with_its_value_and_its_meaning() {
        let worked = worked();
        let table = factors(&worked);
        assert_eq!(table.caption, "The T4127 factors used");
        assert_eq!(table.head, ["Factor", "Value", "What it is"]);
        assert_eq!(table.rows.len(), worked.deductions.factors.len());
        for row in &table.rows {
            assert_ne!(row[2], "A factor of T4127.");
        }
    }

    #[test]
    fn the_claims_are_said_with_whether_they_were_typed() {
        let worked = worked();
        assert_eq!(
            claims(&worked),
            [
                "Federal claim: $16,452.00, the basic personal amount of the tables (claim code 1).",
                "Ontario claim: $12,989.00, the basic personal amount of the tables (claim code 1).",
            ]
        );
        let typed = payroll_calc::deductions(
            &payroll_calc::Query {
                frequency: Some("weekly".to_owned()),
                gross: Some("800.00".to_owned()),
                federal_claim: Some("0".to_owned()),
                provincial_claim: Some("0".to_owned()),
                pay_date: Some("2026-07-15".to_owned()),
                ..payroll_calc::Query::default()
            },
            NaiveDate::parse_from_str("2026-10-14", "%Y-%m-%d").unwrap(),
        )
        .unwrap();
        assert_eq!(
            claims(&typed),
            [
                "Federal claim: $0.00, as typed.",
                "Ontario claim: $0.00, as typed.",
            ]
        );
    }

    #[test]
    fn the_edition_line_names_the_tables_and_their_dates() {
        let worked = worked();
        assert_eq!(
            edition_line(&worked.edition),
            "Tables of CRA's T4127 Payroll Deductions Formulas, 123rd edition, for pay dates from 2026-07-01 to 2026-12-31."
        );
    }

    #[test]
    fn the_words_are_stated() {
        assert_eq!(how().len(), 4);
        assert_eq!(check().len(), 4);
        assert_eq!(scope().len(), 3);
        assert_eq!(
            CLAIMS_HELP,
            "Leave a claim empty for the basic personal amount (claim code 1). Type 0 for claim code 0, or the total from the employee's TD1 and TD1ON."
        );
        assert_eq!(
            YEAR_TO_DATE_HELP,
            "What was deducted and earned on earlier pays this calendar year. Left empty, each is nothing. They matter once an employee nears the year's maximum."
        );
    }
}
