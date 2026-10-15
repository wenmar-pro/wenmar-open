//! The JSON of a worked pay, as the API, the MCP tools and the command-line
//! tool give it; money and factor values are numbers.

use rust_decimal::Decimal;
use rust_decimal::prelude::ToPrimitive;
use shop_math::Money;

/// The deductions from one regular pay in Ontario, by CRA's T4127 formulas.
#[derive(Debug, Clone, PartialEq, serde::Serialize)]
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
pub struct PayrollDeductions {
    /// "ON".
    #[cfg_attr(feature = "openapi", schema(example = "ON"))]
    pub province: String,
    /// The frequency slug: `weekly`, `biweekly`, `semi-monthly` or `monthly`.
    #[cfg_attr(feature = "openapi", schema(example = "biweekly"))]
    pub frequency: String,
    /// Pay periods in the year.
    #[cfg_attr(feature = "openapi", schema(example = 26))]
    pub periods_per_year: u32,
    /// The pay date, `YYYY-MM-DD`.
    #[cfg_attr(feature = "openapi", schema(example = "2026-07-15"))]
    pub pay_date: String,
    /// Said when the pay date was moved into the tables held.
    pub pay_date_note: Option<String>,
    /// The gross pay for the period, in dollars.
    #[cfg_attr(feature = "openapi", schema(example = 2400.0))]
    pub gross: f64,
    /// The federal claim amount used, and whether it is the basic amount (claim code 1).
    pub federal_claim: PayrollClaim,
    /// The Ontario claim amount used, and whether it is the basic amount (claim code 1).
    pub provincial_claim: PayrollClaim,
    /// What was deducted and earned on earlier pays this calendar year.
    pub year_to_date: PayrollYearToDate,
    /// The employee's deductions on this pay, in dollars.
    pub employee: PayrollEmployee,
    /// Gross pay less the employee's deductions, in dollars.
    #[cfg_attr(feature = "openapi", schema(example = 1880.15))]
    pub net_pay: f64,
    /// The employer's share on this pay, in dollars.
    pub employer: PayrollEmployer,
    /// Every T4127 factor used, with the value used and what it means.
    pub factors: Vec<PayrollFactor>,
    /// The edition of T4127 used, and its dates.
    pub edition: PayrollEdition,
    /// The note that the answer is an estimate.
    pub note: String,
    /// CRA's Payroll Deductions Online Calculator.
    pub pdoc_url: String,
    /// The data this answer came from; none, the engine's tables are the data.
    pub attribution: Vec<String>,
}

/// A claim amount used, in dollars, and whether it is the tables' basic amount.
#[derive(Debug, Clone, PartialEq, serde::Serialize)]
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
pub struct PayrollClaim {
    /// The claim amount, in dollars.
    pub amount: f64,
    /// `true` when the amount is the tables' basic amount (claim code 1).
    pub basic: bool,
}

/// What was deducted and earned on earlier pays this calendar year, in dollars.
#[derive(Debug, Clone, PartialEq, serde::Serialize)]
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
pub struct PayrollYearToDate {
    /// CPP (base and first additional) deducted earlier this year.
    pub cpp: f64,
    /// CPP2 deducted earlier this year.
    pub cpp2: f64,
    /// EI deducted earlier this year.
    pub ei: f64,
    /// Pensionable earnings earlier this year.
    pub pensionable: f64,
}

/// The employee's deductions on one pay, in dollars.
#[derive(Debug, Clone, PartialEq, serde::Serialize)]
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
pub struct PayrollEmployee {
    /// Federal income tax.
    pub federal_tax: f64,
    /// Ontario income tax.
    pub provincial_tax: f64,
    /// Federal and Ontario income tax.
    pub total_tax: f64,
    /// CPP (base and first additional).
    pub cpp: f64,
    /// CPP2.
    pub cpp2: f64,
    /// EI.
    pub ei: f64,
    /// Income tax, CPP, CPP2 and EI.
    pub total: f64,
}

/// The employer's share on one pay, in dollars.
#[derive(Debug, Clone, PartialEq, serde::Serialize)]
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
pub struct PayrollEmployer {
    /// CPP (base and first additional).
    pub cpp: f64,
    /// CPP2.
    pub cpp2: f64,
    /// EI.
    pub ei: f64,
    /// CPP, CPP2 and EI.
    pub total: f64,
}

/// One T4127 factor used, with the value used and what it means.
#[derive(Debug, Clone, PartialEq, serde::Serialize)]
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
pub struct PayrollFactor {
    /// The name the engine gives the factor.
    pub name: String,
    /// The value used, after rounding.
    pub value: f64,
    /// What the factor means.
    pub meaning: String,
}

/// The edition of T4127 the answer was worked out with.
#[derive(Debug, Clone, PartialEq, serde::Serialize)]
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
pub struct PayrollEdition {
    /// The edition number.
    pub number: u32,
    /// "T4127 Payroll Deductions Formulas, 123rd edition".
    pub title: String,
    /// The first pay date the tables cover, `YYYY-MM-DD`.
    pub effective: String,
    /// The last pay date the tables cover, `YYYY-MM-DD`.
    pub valid_until: String,
    /// Where CRA publishes the edition.
    pub url: Option<String>,
}

/// A worked pay as its JSON.
pub fn answer(worked: &crate::Worked) -> PayrollDeductions {
    let input = &worked.input;
    let deductions = &worked.deductions;
    let dollars = |amount: Money| amount.cents() as f64 / 100.0;
    let figure = |amount: Decimal| amount.to_f64().unwrap_or(0.0);
    PayrollDeductions {
        province: "ON".to_owned(),
        frequency: input.frequency.slug().to_owned(),
        periods_per_year: input.frequency.periods(),
        pay_date: input.pay_date.to_string(),
        pay_date_note: input.pay_date_note.clone(),
        gross: dollars(input.gross),
        federal_claim: PayrollClaim {
            amount: dollars(input.federal_claim.amount),
            basic: input.federal_claim.basic,
        },
        provincial_claim: PayrollClaim {
            amount: dollars(input.provincial_claim.amount),
            basic: input.provincial_claim.basic,
        },
        year_to_date: PayrollYearToDate {
            cpp: dollars(input.ytd_cpp),
            cpp2: dollars(input.ytd_cpp2),
            ei: dollars(input.ytd_ei),
            pensionable: dollars(input.ytd_pensionable),
        },
        employee: PayrollEmployee {
            federal_tax: figure(deductions.federal_tax),
            provincial_tax: figure(deductions.provincial_tax),
            total_tax: figure(deductions.total_tax),
            cpp: figure(deductions.cpp),
            cpp2: figure(deductions.cpp2),
            ei: figure(deductions.ei),
            total: figure(worked.employee_total),
        },
        net_pay: figure(worked.net_pay),
        employer: PayrollEmployer {
            cpp: figure(deductions.employer_cpp),
            cpp2: figure(deductions.employer_cpp2),
            ei: figure(deductions.employer_ei),
            total: figure(worked.employer_total),
        },
        factors: deductions
            .factors
            .iter()
            .map(|(name, value)| PayrollFactor {
                name: name.clone(),
                value: figure(*value),
                meaning: crate::factors::meaning(name)
                    .unwrap_or("A factor of T4127.")
                    .to_owned(),
            })
            .collect(),
        edition: PayrollEdition {
            number: worked.edition.number,
            title: worked.edition.title(),
            effective: worked.edition.effective.to_string(),
            valid_until: worked.edition.valid_until.to_string(),
            url: worked.edition.url.map(str::to_owned),
        },
        note: crate::NOTE.to_owned(),
        pdoc_url: crate::PDOC_URL.to_owned(),
        attribution: Vec::new(),
    }
}
