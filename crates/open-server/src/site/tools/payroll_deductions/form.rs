//! The form of the Ontario payroll page: a pay frequency, the gross pay,
//! the two claim amounts, the pay date and the year-to-date figures.
//!
//! The page has no script, so every field is a text box, the frequency a
//! list, and the pay date a date box. What is typed goes into the address
//! and nowhere else.

use chrono::NaiveDate;
use payroll_calc::{FIELDS, Frequency, Query, Worked};

use crate::site::tools::field::{Field, Sent};

/// One entry of the list of pay frequencies.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Choice {
    pub slug: &'static str,
    pub name: &'static str,
    pub selected: bool,
}

/// The form as the page shows it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Form {
    /// The list of pay frequencies. Its `value` is the slug of the one
    /// chosen, or nothing; what was sent is never shown again.
    pub frequency: Field,
    pub chosen: Option<Frequency>,
    pub gross: Field,
    pub federal_claim: Field,
    pub provincial_claim: Field,
    pub pay_date: Field,
    pub ytd_cpp: Field,
    pub ytd_cpp2: Field,
    pub ytd_ei: Field,
    pub ytd_pensionable: Field,
    /// A refusal that names no field, or the province.
    pub error: Option<String>,
    /// Said under the pay date when its default was moved into the tables.
    pub note: Option<String>,
}

/// The page's own example: a biweekly pay of 2400.00, the claims and the
/// pay date left to their defaults.
pub fn example() -> Sent {
    Sent::of(&[("frequency", "biweekly"), ("gross", "2400.00")])
}

impl Form {
    fn fields_of(sent: &Sent, chosen: Option<Frequency>) -> Form {
        Form {
            frequency: Field::new(sent, "frequency", "Pay frequency"),
            chosen,
            gross: Field::new(sent, "gross", "Gross pay for this pay period"),
            federal_claim: Field::new(sent, "federal_claim", "Federal claim amount (TD1)"),
            provincial_claim: Field::new(sent, "provincial_claim", "Ontario claim amount (TD1ON)"),
            pay_date: Field::new(sent, "pay_date", "Pay date"),
            ytd_cpp: Field::new(sent, "ytd_cpp", "CPP deducted earlier this year"),
            ytd_cpp2: Field::new(sent, "ytd_cpp2", "CPP2 deducted earlier this year"),
            ytd_ei: Field::new(sent, "ytd_ei", "EI deducted earlier this year"),
            ytd_pensionable: Field::new(
                sent,
                "ytd_pensionable",
                "Pensionable earnings earlier this year",
            ),
            error: None,
            note: None,
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
            &self.frequency,
            &self.gross,
            &self.federal_claim,
            &self.provincial_claim,
            &self.pay_date,
            &self.ytd_cpp,
            &self.ytd_cpp2,
            &self.ytd_ei,
            &self.ytd_pensionable,
        ];
        troubles.extend(fields.into_iter().filter_map(Field::trouble));
        troubles
    }

    /// Reads what was sent through `payroll_calc::deductions`. Each refusal
    /// goes beside its field; one that names no field, or the province,
    /// goes to `error`. When worked, an empty claim field is filled with
    /// the amount that was used, an empty pay date with the date used, and
    /// the frequency's value is the slug of the one read.
    pub fn read(sent: &Sent, today: NaiveDate) -> (Form, Option<Worked>) {
        let mut form = Form::fields_of(sent, None);
        let mut query = Query::default();
        for name in FIELDS {
            query.set(name, sent.text(name));
        }
        match payroll_calc::deductions(&query, today) {
            Ok(worked) => {
                form.chosen = Some(worked.input.frequency);
                form.frequency.value = worked.input.frequency.slug().to_owned();
                if form.federal_claim.is_empty() {
                    form.federal_claim.value = worked.input.federal_claim.amount.input();
                }
                if form.provincial_claim.is_empty() {
                    form.provincial_claim.value = worked.input.provincial_claim.amount.input();
                }
                if form.pay_date.is_empty() {
                    form.pay_date.value = worked.input.pay_date.to_string();
                }
                form.note = worked.input.pay_date_note.clone();
                (form, Some(worked))
            }
            Err(refusals) => {
                for refusal in refusals {
                    match refusal.field {
                        "frequency" => form.frequency.refuse(refusal.message),
                        "gross" => form.gross.refuse(refusal.message),
                        "federal_claim" => form.federal_claim.refuse(refusal.message),
                        "provincial_claim" => form.provincial_claim.refuse(refusal.message),
                        "pay_date" => form.pay_date.refuse(refusal.message),
                        "ytd_cpp" => form.ytd_cpp.refuse(refusal.message),
                        "ytd_cpp2" => form.ytd_cpp2.refuse(refusal.message),
                        "ytd_ei" => form.ytd_ei.refuse(refusal.message),
                        "ytd_pensionable" => form.ytd_pensionable.refuse(refusal.message),
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
        // Someone else's marker on a link is not the form.
        for name in ["utm_source", "ref", "frequency_", "pay", "date"] {
            assert!(!Form::was_sent(&Sent::of(&[(name, "x")])), "{name}");
        }
        assert!(!Form::was_sent(&Sent::default()));
        assert!(Form::was_sent(&example()));
    }

    #[test]
    fn the_example_is_read_and_worked_out() {
        let (form, worked) = Form::read(&example(), today());
        assert!(form.troubles().is_empty(), "{:?}", form.troubles());
        let worked = worked.unwrap();
        assert_eq!(form.chosen, Some(Frequency::Biweekly));
        assert_eq!(form.frequency.value, "biweekly");
        // The empty claims are filled with the basic amounts that were
        // used, and the empty pay date with the day that was.
        assert_eq!(form.federal_claim.value, "16452.00");
        assert_eq!(form.provincial_claim.value, "12989.00");
        assert_eq!(form.pay_date.value, "2026-10-14");
        assert_eq!(form.note, None);
        let selected: Vec<&str> = form
            .choices()
            .iter()
            .filter(|choice| choice.selected)
            .map(|choice| choice.slug)
            .collect();
        assert_eq!(selected, ["biweekly"]);
        let slugs: Vec<&str> = form.choices().iter().map(|choice| choice.slug).collect();
        assert_eq!(slugs, ["weekly", "biweekly", "semi-monthly", "monthly"]);
        assert_eq!(worked.input.gross, shop_math::Money::from_cents(240_000));
    }

    #[test]
    fn a_field_that_cannot_be_read_gets_the_message_and_the_rest_keep_what_was_typed() {
        let sent = Sent::of(&[
            ("frequency", "fortnightly"),
            ("gross", "abc"),
            ("federal_claim", "-5"),
            ("pay_date", "15/07/2026"),
            ("ytd_ei", "12.345"),
        ]);
        let (form, worked) = Form::read(&sent, today());
        assert!(worked.is_none());
        assert_eq!(
            form.troubles(),
            [
                (
                    "frequency".to_owned(),
                    "Pay frequency: Choose weekly, biweekly, semi-monthly or monthly.".to_owned()
                ),
                (
                    "gross".to_owned(),
                    "Gross pay for this pay period: This is not a number. Write it like 1,234.50."
                        .to_owned()
                ),
                (
                    "federal_claim".to_owned(),
                    "Federal claim amount (TD1): This cannot be below zero.".to_owned()
                ),
                (
                    "pay_date".to_owned(),
                    "Pay date: Write the date like 2026-07-15.".to_owned()
                ),
                (
                    "ytd_ei".to_owned(),
                    "EI deducted earlier this year: Use at most 2 decimal places.".to_owned()
                ),
            ]
        );
        assert_eq!(form.frequency.value, "fortnightly");
        assert_eq!(form.gross.value, "abc");
        assert_eq!(form.pay_date.value, "15/07/2026");
    }

    #[test]
    fn a_pay_date_outside_the_tables_is_said_at_the_field() {
        let sent = Sent::of(&[
            ("frequency", "weekly"),
            ("gross", "800"),
            ("pay_date", "2027-01-15"),
        ]);
        let (form, worked) = Form::read(&sent, today());
        assert!(worked.is_none());
        assert_eq!(
            form.pay_date.error.as_deref(),
            Some(
                "No tables are held for 2027-01-15. This calculator holds T4127's 123rd edition, for pay dates from 2026-07-01 to 2026-12-31."
            )
        );
    }

    #[test]
    fn an_empty_pay_date_is_today_and_a_moved_one_says_so() {
        let sent = example();
        let (form, _) = Form::read(&sent, today());
        assert_eq!(form.pay_date.value, "2026-10-14");
        assert_eq!(form.note, None);
        let (form, _) = Form::read(
            &sent,
            NaiveDate::parse_from_str("2027-02-01", "%Y-%m-%d").unwrap(),
        );
        assert_eq!(form.pay_date.value, "2026-12-31");
        assert_eq!(
            form.note.as_deref(),
            Some("Today is outside the tables held, so the pay date is 2026-12-31.")
        );
    }

    #[test]
    fn a_province_that_is_not_ontario_is_said_at_the_form() {
        let sent = Sent::of(&[
            ("province", "QC"),
            ("frequency", "weekly"),
            ("gross", "800"),
        ]);
        let (form, worked) = Form::read(&sent, today());
        assert!(worked.is_none());
        assert_eq!(
            form.error.as_deref(),
            Some("Only Ontario is covered for now.")
        );
        assert_eq!(
            form.troubles(),
            [(
                "calc".to_owned(),
                "Only Ontario is covered for now.".to_owned()
            )]
        );
    }

    #[test]
    fn nothing_that_can_be_typed_stops_the_form() {
        let long = "9".repeat(5_000);
        for text in [
            "99999999999999999999999999",
            "1e308",
            "NaN",
            "-0",
            "\u{0}",
            "<script>",
            long.as_str(),
        ] {
            let sent = Sent::of(&[
                ("province", "QC"),
                ("frequency", "fortnightly"),
                ("gross", text),
                ("federal_claim", text),
                ("provincial_claim", text),
                ("pay_date", text),
                ("ytd_cpp", text),
                ("ytd_cpp2", text),
                ("ytd_ei", text),
                ("ytd_pensionable", text),
            ]);
            let (form, _) = Form::read(&sent, today());
            let _ = form.troubles();
        }
        // The largest of everything a field accepts.
        let sent = Sent::of(&[
            ("frequency", "weekly"),
            ("gross", "99,999,999.99"),
            ("federal_claim", "99,999,999.99"),
            ("provincial_claim", "99,999,999.99"),
            ("pay_date", "2026-07-15"),
            ("ytd_cpp", "99,999,999.99"),
            ("ytd_cpp2", "99,999,999.99"),
            ("ytd_ei", "99,999,999.99"),
            ("ytd_pensionable", "99,999,999.99"),
        ]);
        let (form, worked) = Form::read(&sent, today());
        assert!(worked.is_some());
        assert!(form.troubles().is_empty());
    }
}
