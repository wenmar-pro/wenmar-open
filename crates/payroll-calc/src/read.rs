//! A pay as text, and the working out of one that was read.
//!
//! The form, a query string, a tool call and a command line all give a pay
//! as text. This module reads every field, fills the documented defaults,
//! and asks the `payroll-ca` engine. It writes no rate and no amount: every
//! figure is the engine's. Ontario only.

use chrono::NaiveDate;
use rust_decimal::Decimal;
use rust_decimal::prelude::ToPrimitive;
use shop_math::Money;

/// The names of the fields, in the order of the form.
pub const FIELDS: [&str; 10] = [
    "province",
    "frequency",
    "gross",
    "federal_claim",
    "provincial_claim",
    "pay_date",
    "ytd_cpp",
    "ytd_cpp2",
    "ytd_ei",
    "ytd_pensionable",
];

/// A pay as text, as a form, a query string, a tool call or a command line
/// gives it. `None` and empty text both mean "not given".
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Query {
    pub province: Option<String>,
    pub frequency: Option<String>,
    pub gross: Option<String>,
    pub federal_claim: Option<String>,
    pub provincial_claim: Option<String>,
    pub pay_date: Option<String>,
    pub ytd_cpp: Option<String>,
    pub ytd_cpp2: Option<String>,
    pub ytd_ei: Option<String>,
    pub ytd_pensionable: Option<String>,
}

impl Query {
    /// The text of the field `name` (one of FIELDS), or "".
    pub fn text(&self, name: &str) -> &str {
        match name {
            "province" => self.province.as_deref().unwrap_or(""),
            "frequency" => self.frequency.as_deref().unwrap_or(""),
            "gross" => self.gross.as_deref().unwrap_or(""),
            "federal_claim" => self.federal_claim.as_deref().unwrap_or(""),
            "provincial_claim" => self.provincial_claim.as_deref().unwrap_or(""),
            "pay_date" => self.pay_date.as_deref().unwrap_or(""),
            "ytd_cpp" => self.ytd_cpp.as_deref().unwrap_or(""),
            "ytd_cpp2" => self.ytd_cpp2.as_deref().unwrap_or(""),
            "ytd_ei" => self.ytd_ei.as_deref().unwrap_or(""),
            "ytd_pensionable" => self.ytd_pensionable.as_deref().unwrap_or(""),
            _ => "",
        }
    }

    /// Sets the field `name`; a name not in FIELDS is ignored.
    pub fn set(&mut self, name: &str, text: &str) {
        let text = text.to_owned();
        match name {
            "province" => self.province = Some(text),
            "frequency" => self.frequency = Some(text),
            "gross" => self.gross = Some(text),
            "federal_claim" => self.federal_claim = Some(text),
            "provincial_claim" => self.provincial_claim = Some(text),
            "pay_date" => self.pay_date = Some(text),
            "ytd_cpp" => self.ytd_cpp = Some(text),
            "ytd_cpp2" => self.ytd_cpp2 = Some(text),
            "ytd_ei" => self.ytd_ei = Some(text),
            "ytd_pensionable" => self.ytd_pensionable = Some(text),
            _ => {}
        }
    }
}

/// A field that could not be read, or a refusal of the pay as a whole
/// (`field` is "").
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Refusal {
    pub field: &'static str,
    pub message: String,
}

/// A claim amount and whether it is the tables' basic amount (claim code 1),
/// used because the field was left empty.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Claim {
    pub amount: Money,
    pub basic: bool,
}

/// A pay that was read, with every default filled in.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Input {
    pub frequency: crate::Frequency,
    pub gross: Money,
    pub federal_claim: Claim,
    pub provincial_claim: Claim,
    pub pay_date: NaiveDate,
    /// Whether the pay date was typed, not filled in.
    pub pay_date_given: bool,
    /// Said beside the pay date when the default was moved into the tables.
    pub pay_date_note: Option<String>,
    pub ytd_cpp: Money,
    pub ytd_cpp2: Money,
    pub ytd_ei: Money,
    pub ytd_pensionable: Money,
}

/// A pay worked out.
#[derive(Debug, Clone, PartialEq)]
pub struct Worked {
    pub input: Input,
    pub deductions: payroll_ca::Deductions,
    pub edition: crate::Edition,
    /// Income tax, CPP, CPP2 and EI.
    pub employee_total: Decimal,
    /// Gross pay less employee_total.
    pub net_pay: Decimal,
    /// The employer's CPP, CPP2 and EI.
    pub employer_total: Decimal,
}

/// Dollars as Money, to the cent (the engine's amounts are already cents).
pub fn money(amount: Decimal) -> Money {
    let cents = (amount * Decimal::new(100, 0))
        .round()
        .to_i64()
        .unwrap_or(i64::MAX);
    Money::from_cents(cents)
}

/// Money as dollars, as the engine takes them.
fn dollars(amount: Money) -> Decimal {
    Decimal::new(amount.cents(), 2)
}

/// The message for a pay date no held table covers, with the dates the
/// tables do cover.
fn no_tables(date: NaiveDate) -> String {
    let held = crate::editions();
    let (words, span) = if held.len() == 1 {
        (
            format!("{} edition", crate::ordinal(held[0].number)),
            held[0].span(),
        )
    } else {
        (
            format!(
                "{} to {} editions",
                crate::ordinal(held[0].number),
                crate::ordinal(held[held.len() - 1].number)
            ),
            format!(
                "for pay dates from {} to {}",
                held[0].effective,
                held[held.len() - 1].valid_until
            ),
        )
    };
    format!("No tables are held for {date}. This calculator holds T4127's {words}, {span}.")
}

/// Reads a claim field. Left empty, it is the tables' basic amount (claim
/// code 1), when the pay date was read; a typed amount is the TD1 total
/// (claim code 0 when it is 0).
fn claim(
    text: &str,
    field: &'static str,
    basic: Option<Money>,
) -> (Option<Claim>, Option<Refusal>) {
    if text.trim().is_empty() {
        return match basic {
            Some(amount) => (
                Some(Claim {
                    amount,
                    basic: true,
                }),
                None,
            ),
            None => (None, None),
        };
    }
    match Money::parse(text) {
        Ok(amount) => (
            Some(Claim {
                amount,
                basic: false,
            }),
            None,
        ),
        Err(error) => (
            None,
            Some(Refusal {
                field,
                message: error.to_string(),
            }),
        ),
    }
}

/// Reads every field and reports every one that cannot be read, in the
/// order of FIELDS.
pub fn read(query: &Query, today: NaiveDate) -> Result<Input, Vec<Refusal>> {
    let mut refusals: Vec<Refusal> = Vec::new();

    let province = query.text("province").trim();
    if !province.is_empty() && !province.eq_ignore_ascii_case("on") {
        refusals.push(Refusal {
            field: "province",
            message: "Only Ontario is covered for now.".to_owned(),
        });
    }

    let frequency = crate::Frequency::parse(query.text("frequency"));
    if frequency.is_none() {
        refusals.push(Refusal {
            field: "frequency",
            message: "Choose weekly, biweekly, semi-monthly or monthly.".to_owned(),
        });
    }

    let gross = match Money::parse(query.text("gross")) {
        Ok(gross) => Some(gross),
        Err(error) => {
            refusals.push(Refusal {
                field: "gross",
                message: error.to_string(),
            });
            None
        }
    };

    // The pay date is read before the claims: their defaults are the basic
    // amounts of its tables.
    let pay_date_text = query.text("pay_date").trim();
    let (pay_date, pay_date_given, pay_date_note) = if pay_date_text.is_empty() {
        let (date, moved) = crate::default_pay_date(today);
        let note =
            moved.then(|| format!("Today is outside the tables held, so the pay date is {date}."));
        (Some(date), false, note)
    } else {
        match NaiveDate::parse_from_str(pay_date_text, "%Y-%m-%d") {
            Ok(date) => (Some(date), true, None),
            Err(_) => {
                refusals.push(Refusal {
                    field: "pay_date",
                    message: "Write the date like 2026-07-15.".to_owned(),
                });
                (None, true, None)
            }
        }
    };
    let tables = pay_date.and_then(|date| payroll_ca::rates::rate_set_for(date).ok());
    if let (Some(date), None) = (pay_date, tables) {
        refusals.push(Refusal {
            field: "pay_date",
            message: no_tables(date),
        });
    }

    let (federal_claim, refusal) = claim(
        query.text("federal_claim"),
        "federal_claim",
        tables.map(|set| money(set.federal.basic_amount)),
    );
    if let Some(refusal) = refusal {
        refusals.push(refusal);
    }
    let (provincial_claim, refusal) = claim(
        query.text("provincial_claim"),
        "provincial_claim",
        tables.map(|set| money(set.province(payroll_ca::Province::ON).basic_amount)),
    );
    if let Some(refusal) = refusal {
        refusals.push(refusal);
    }

    let mut ytd = |name: &str, field: &'static str| -> Option<Money> {
        let text = query.text(name);
        if text.trim().is_empty() {
            return Some(Money::ZERO);
        }
        match Money::parse(text) {
            Ok(amount) => Some(amount),
            Err(error) => {
                refusals.push(Refusal {
                    field,
                    message: error.to_string(),
                });
                None
            }
        }
    };
    let ytd_cpp = ytd("ytd_cpp", "ytd_cpp");
    let ytd_cpp2 = ytd("ytd_cpp2", "ytd_cpp2");
    let ytd_ei = ytd("ytd_ei", "ytd_ei");
    let ytd_pensionable = ytd("ytd_pensionable", "ytd_pensionable");

    if !refusals.is_empty() {
        return Err(refusals);
    }
    Ok(Input {
        frequency: frequency.unwrap(),
        gross: gross.unwrap(),
        federal_claim: federal_claim.unwrap(),
        provincial_claim: provincial_claim.unwrap(),
        pay_date: pay_date.unwrap(),
        pay_date_given,
        pay_date_note,
        ytd_cpp: ytd_cpp.unwrap(),
        ytd_cpp2: ytd_cpp2.unwrap(),
        ytd_ei: ytd_ei.unwrap(),
        ytd_pensionable: ytd_pensionable.unwrap(),
    })
}

/// Asks the engine. A refusal from it names no field.
pub fn work(input: &Input) -> Result<Worked, Refusal> {
    let gross = dollars(input.gross);
    let pay = payroll_ca::Pay {
        pay_date: input.pay_date,
        province: payroll_ca::Province::ON,
        periods_per_year: input.frequency.periods(),
        gross_taxable: gross,
        pensionable: gross,
        insurable: gross,
        federal_claim: dollars(input.federal_claim.amount),
        provincial_claim: dollars(input.provincial_claim.amount),
        ytd_cpp_base_and_first: dollars(input.ytd_cpp),
        ytd_cpp2: dollars(input.ytd_cpp2),
        ytd_ei: dollars(input.ytd_ei),
        ytd_pensionable: dollars(input.ytd_pensionable),
        cpp_exempt: false,
        ei_exempt: false,
        birth_date: None,
        additional_tax: Decimal::ZERO,
    };
    let deductions = payroll_ca::calculate(&pay).map_err(|error| Refusal {
        field: "",
        message: error.to_string(),
    })?;
    let employee_total = deductions.total_tax + deductions.cpp + deductions.cpp2 + deductions.ei;
    Ok(Worked {
        input: input.clone(),
        edition: crate::Edition::of(
            payroll_ca::rates::rate_set_for(input.pay_date)
                .expect("read checked the pay date against the tables held"),
        ),
        employee_total,
        net_pay: gross - employee_total,
        employer_total: deductions.employer_cpp + deductions.employer_cpp2 + deductions.employer_ei,
        deductions,
    })
}

/// read, then work.
pub fn deductions(query: &Query, today: NaiveDate) -> Result<Worked, Vec<Refusal>> {
    read(query, today).and_then(|input| work(&input).map_err(|refusal| vec![refusal]))
}
