//! The form of the technician pay page: the hours and rates of the gross,
//! and the pay frequency, pay date and claims of the deductions on it.
//!
//! The page has no script, so every field is a text box, the frequency a
//! list, and the pay date a date box. What is typed goes into the address
//! and nowhere else.

use chrono::NaiveDate;
use payroll_calc::{Frequency, Query};
use shop_math::{Hours, Money, technician_pay};

use crate::site::tools::field::{Field, Sent};

/// One entry of the list of pay frequencies.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Choice {
    pub slug: &'static str,
    pub name: &'static str,
    pub selected: bool,
}

/// The gross pay worked out, and the deductions on it, when the pay
/// frequency was given.
#[derive(Debug, Clone, PartialEq)]
pub struct Worked {
    pub gross: technician_pay::Gross,
    pub deductions: Option<payroll_calc::Worked>,
}

/// The fields the form reads.
const FIELDS: [&str; 10] = [
    "flagged_hours",
    "flat_rate",
    "hourly_hours",
    "hourly_rate",
    "overtime_hours",
    "overtime_rate",
    "frequency",
    "pay_date",
    "federal_claim",
    "provincial_claim",
];

const NOT_MONEY: &str = "This is not a number. Write it like 1,234.50.";
const NOT_HOURS: &str = "This is not a number of hours. Write it like 8.5.";
const GROSS_TOO_LARGE: &str = "The gross pay is larger than this calculator works out. Keep the rates and hours below 100,000,000.";

/// The form as the page shows it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Form {
    pub flagged_hours: Field,
    pub flat_rate: Field,
    pub hourly_hours: Field,
    pub hourly_rate: Field,
    pub overtime_hours: Field,
    pub overtime_rate: Field,
    /// The list of pay frequencies for the deductions. Its `value` is the
    /// slug of the one chosen, or nothing.
    pub frequency: Field,
    pub chosen: Option<Frequency>,
    pub pay_date: Field,
    pub federal_claim: Field,
    pub provincial_claim: Field,
    /// A refusal that names no field, or the province.
    pub error: Option<String>,
}

/// The page's own example: 80 flagged hours at $32.00 and 6 hours of shop
/// time at $25.00, paid biweekly.
pub fn example() -> Sent {
    Sent::of(&[
        ("flagged_hours", "80"),
        ("flat_rate", "32.00"),
        ("hourly_hours", "6"),
        ("hourly_rate", "25.00"),
        ("frequency", "biweekly"),
    ])
}

impl Form {
    fn fields_of(sent: &Sent, chosen: Option<Frequency>) -> Form {
        Form {
            flagged_hours: Field::new(sent, "flagged_hours", "Flagged hours"),
            flat_rate: Field::new(sent, "flat_rate", "Flat rate per flagged hour"),
            hourly_hours: Field::new(sent, "hourly_hours", "Hourly hours"),
            hourly_rate: Field::new(sent, "hourly_rate", "Hourly rate"),
            overtime_hours: Field::new(sent, "overtime_hours", "Overtime hours"),
            overtime_rate: Field::new(sent, "overtime_rate", "Overtime rate"),
            frequency: Field::new(sent, "frequency", "Pay frequency"),
            chosen,
            pay_date: Field::new(sent, "pay_date", "Pay date"),
            federal_claim: Field::new(sent, "federal_claim", "Federal claim amount (TD1)"),
            provincial_claim: Field::new(sent, "provincial_claim", "Ontario claim amount (TD1ON)"),
            error: None,
        }
    }

    /// Whether the query string names any field of FIELDS.
    pub fn was_sent(sent: &Sent) -> bool {
        FIELDS.iter().any(|name| sent.has(name))
    }

    /// The four frequencies, the chosen one `selected`.
    pub fn choices(&self) -> Vec<Choice> {
        Frequency::ALL
            .iter()
            .copied()
            .map(|frequency| Choice {
                slug: frequency.slug(),
                name: frequency.label(),
                selected: self.chosen == Some(frequency),
            })
            .collect()
    }

    /// What to correct, in the order of the form, as `Field::trouble`
    /// gives it; `error` first as ("calc", error).
    pub fn troubles(&self) -> Vec<(String, String)> {
        let mut troubles: Vec<(String, String)> = Vec::new();
        if let Some(error) = &self.error {
            troubles.push(("calc".to_owned(), error.clone()));
        }
        let fields = [
            &self.flagged_hours,
            &self.flat_rate,
            &self.hourly_hours,
            &self.hourly_rate,
            &self.overtime_hours,
            &self.overtime_rate,
            &self.frequency,
            &self.pay_date,
            &self.federal_claim,
            &self.provincial_claim,
        ];
        troubles.extend(fields.into_iter().filter_map(Field::trouble));
        troubles
    }

    /// The value of one field, by name, for the deductions query.
    fn field_value(&self, name: &str) -> String {
        match name {
            "flagged_hours" => self.flagged_hours.value.clone(),
            "flat_rate" => self.flat_rate.value.clone(),
            "hourly_hours" => self.hourly_hours.value.clone(),
            "hourly_rate" => self.hourly_rate.value.clone(),
            "overtime_hours" => self.overtime_hours.value.clone(),
            "overtime_rate" => self.overtime_rate.value.clone(),
            "frequency" => self.frequency.value.clone(),
            "pay_date" => self.pay_date.value.clone(),
            "federal_claim" => self.federal_claim.value.clone(),
            "provincial_claim" => self.provincial_claim.value.clone(),
            _ => String::new(),
        }
    }

    /// Reads what was sent: the gross pay from the hours and rates, and,
    /// when a pay frequency was given, the Ontario deductions on the gross.
    /// Each refusal goes beside its field; one that names no field, or the
    /// province, goes to `error`. An empty hours or rate field is zero.
    pub fn read(sent: &Sent, today: NaiveDate) -> (Form, Option<Worked>) {
        let mut form = Form::fields_of(sent, None);
        // The gross: every field is optional, and an empty one is zero.
        let flagged = match Hours::parse(form.flagged_hours.value.trim()) {
            Ok(hours) => hours,
            Err(_) if form.flagged_hours.value.trim().is_empty() => Hours::ZERO,
            Err(_) => {
                form.flagged_hours.refuse(NOT_HOURS);
                return (form, None);
            }
        };
        let flat = match Money::parse(form.flat_rate.value.trim()) {
            Ok(money) => money,
            Err(_) if form.flat_rate.value.trim().is_empty() => Money::ZERO,
            Err(_) => {
                form.flat_rate.refuse(NOT_MONEY);
                return (form, None);
            }
        };
        let hourly = match Hours::parse(form.hourly_hours.value.trim()) {
            Ok(hours) => hours,
            Err(_) if form.hourly_hours.value.trim().is_empty() => Hours::ZERO,
            Err(_) => {
                form.hourly_hours.refuse(NOT_HOURS);
                return (form, None);
            }
        };
        let hourly_rate = match Money::parse(form.hourly_rate.value.trim()) {
            Ok(money) => money,
            Err(_) if form.hourly_rate.value.trim().is_empty() => Money::ZERO,
            Err(_) => {
                form.hourly_rate.refuse(NOT_MONEY);
                return (form, None);
            }
        };
        let overtime = match Hours::parse(form.overtime_hours.value.trim()) {
            Ok(hours) => hours,
            Err(_) if form.overtime_hours.value.trim().is_empty() => Hours::ZERO,
            Err(_) => {
                form.overtime_hours.refuse(NOT_HOURS);
                return (form, None);
            }
        };
        let overtime_rate = match Money::parse(form.overtime_rate.value.trim()) {
            Ok(money) => money,
            Err(_) if form.overtime_rate.value.trim().is_empty() => Money::ZERO,
            Err(_) => {
                form.overtime_rate.refuse(NOT_MONEY);
                return (form, None);
            }
        };
        let Ok(gross) =
            technician_pay::gross(flagged, flat, hourly, hourly_rate, overtime, overtime_rate)
        else {
            form.error = Some(GROSS_TOO_LARGE.to_owned());
            return (form, None);
        };
        // The deductions, when a pay frequency was given.
        if form.frequency.value.trim().is_empty() {
            return (
                form,
                Some(Worked {
                    gross,
                    deductions: None,
                }),
            );
        }
        let mut query = Query::default();
        for name in FIELDS {
            let text = form.field_value(name);
            query.set(name, &text);
        }
        let gross_text = gross.total.input();
        query.set("gross", &gross_text);
        match payroll_calc::deductions(&query, today) {
            Ok(deductions) => {
                form.chosen = Some(deductions.input.frequency);
                form.frequency.value = deductions.input.frequency.slug().to_owned();
                if form.federal_claim.is_empty() {
                    form.federal_claim.value = deductions.input.federal_claim.amount.input();
                }
                if form.provincial_claim.is_empty() {
                    form.provincial_claim.value = deductions.input.provincial_claim.amount.input();
                }
                if form.pay_date.is_empty() {
                    form.pay_date.value = deductions.input.pay_date.to_string();
                }
                (
                    form,
                    Some(Worked {
                        gross,
                        deductions: Some(deductions),
                    }),
                )
            }
            Err(refusals) => {
                for refusal in refusals {
                    match refusal.field {
                        "frequency" => form.frequency.refuse(refusal.message),
                        "pay_date" => form.pay_date.refuse(refusal.message),
                        "federal_claim" => form.federal_claim.refuse(refusal.message),
                        "provincial_claim" => form.provincial_claim.refuse(refusal.message),
                        // A refusal of the pay as a whole, and the
                        // province, which the form does not show.
                        _ => form.error = Some(refusal.message),
                    }
                }
                (form, None)
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use chrono::NaiveDate;

    use super::*;

    fn today() -> NaiveDate {
        NaiveDate::parse_from_str("2026-10-14", "%Y-%m-%d").unwrap()
    }

    #[test]
    fn the_form_was_sent_when_the_address_names_one_of_its_fields() {
        for name in FIELDS {
            assert!(Form::was_sent(&Sent::of(&[(name, "")])), "{name}");
        }
        assert!(!Form::was_sent(&Sent::default()));
        assert!(Form::was_sent(&example()));
    }

    #[test]
    fn the_example_is_read_and_worked_out() {
        let (form, worked) = Form::read(&example(), today());
        assert!(form.troubles().is_empty(), "{:?}", form.troubles());
        let worked = worked.unwrap();
        // 80 × 32 + 6 × 25 = 2,560 + 150 = 2,710 gross.
        assert_eq!(worked.gross.total, Money::from_cents(271_000));
        assert_eq!(worked.gross.flagged, Money::from_cents(256_000));
        assert_eq!(worked.gross.hourly, Money::from_cents(15_000));
        let deductions = worked.deductions.unwrap();
        assert_eq!(deductions.input.gross, Money::from_cents(271_000));
        assert_eq!(form.chosen, Some(Frequency::Biweekly));
    }

    #[test]
    fn an_empty_field_is_zero_and_a_bad_one_keeps_what_was_typed() {
        let sent = Sent::of(&[
            ("flagged_hours", "12"),
            ("flat_rate", "abc"),
            ("hourly_hours", ""),
            ("hourly_rate", "30.00"),
        ]);
        let (form, worked) = Form::read(&sent, today());
        assert!(worked.is_none());
        assert_eq!(
            form.troubles(),
            [(
                "flat_rate".to_owned(),
                "Flat rate per flagged hour: This is not a number. Write it like 1,234.50."
                    .to_owned()
            ),]
        );
        assert_eq!(form.flat_rate.value, "abc");
    }
}
