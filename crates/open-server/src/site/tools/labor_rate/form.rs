//! The form of the labor rate page: two parts that share one button. The
//! first asks what the shop pays in a month, for the rate it needs. The
//! second asks what it sold in a period, for the rate it is getting.
//!
//! A part with nothing typed in it is not worked out and nothing in it is
//! reported, so a shop can use either part alone.

use shop_math::labor_rate::{
    DEFAULT_PAID_HOURS, DEFAULT_PRODUCTIVITY, DEFAULT_TARGET_PROFIT, EffectiveRate,
    EffectiveRateInput, LEAST_PRODUCTIVITY, LaborField, LaborRateError, MOST_PRODUCTIVITY,
    MOST_TARGET, RateNeeded, RateNeededInput, effective_rate, rate_needed,
};
use shop_math::targets::LABOR;
use shop_math::{Hours, Money, Percent, parse_count};

use crate::site::tools::field::{Field, Sent};

/// Where each part starts on the page. A message about a part as a whole
/// points here.
pub const NEEDED: &str = "needed";
pub const GETTING: &str = "getting";

fn all_empty(fields: &[&Field]) -> bool {
    fields.iter().all(|field| field.is_empty())
}

fn troubles(place: &str, error: Option<&String>, fields: &[&Field]) -> Vec<(String, String)> {
    let mut troubles: Vec<(String, String)> = Vec::new();
    if let Some(error) = error {
        troubles.push((place.to_owned(), error.clone()));
    }
    troubles.extend(fields.iter().filter_map(|field| field.trouble()));
    troubles
}

/// The first part: what the shop pays and expects in a month.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Needed {
    pub technicians: Field,
    pub paid_hours: Field,
    pub productivity: Field,
    pub technician_cost: Field,
    pub overhead: Field,
    pub parts_profit: Field,
    pub target_profit: Field,
    pub target_labor: Field,
    /// Whether nothing was typed in this part.
    pub left_empty: bool,
    /// A refusal that is about this part as a whole.
    pub error: Option<String>,
}

impl Needed {
    fn new(sent: &Sent) -> Needed {
        let mut part = Needed {
            technicians: Field::new(sent, "technicians", "Technicians"),
            paid_hours: Field::new(sent, "paid_hours", "Paid hours per technician, per month"),
            productivity: Field::new(sent, "productivity", "Productivity, percent"),
            technician_cost: Field::new(sent, "technician_cost", "Technician cost per month"),
            overhead: Field::new(sent, "overhead", "Overhead per month"),
            parts_profit: Field::new(
                sent,
                "parts_profit",
                "Gross profit on parts per month (optional)",
            ),
            target_profit: Field::new(
                sent,
                "target_profit",
                "Target profit, percent of labor sales",
            ),
            target_labor: Field::new(sent, "target_labor", "Target labor gross profit, percent"),
            left_empty: false,
            error: None,
        };
        part.left_empty = all_empty(&part.fields());
        part
    }

    pub fn fields(&self) -> [&Field; 8] {
        [
            &self.technicians,
            &self.paid_hours,
            &self.productivity,
            &self.technician_cost,
            &self.overhead,
            &self.parts_profit,
            &self.target_profit,
            &self.target_labor,
        ]
    }

    /// Reads the part and works the rate out. A field left empty takes its
    /// default from `shop-math` where it has one, and shows it.
    fn read(&mut self) -> Option<RateNeeded> {
        if self.left_empty {
            return None;
        }
        let productivity =
            |text: &str| Percent::parse_between(text, LEAST_PRODUCTIVITY, MOST_PRODUCTIVITY);
        let target = |text: &str| Percent::parse_between(text, Percent::ZERO, MOST_TARGET);
        let technicians = self.technicians.required(parse_count);
        let paid_hours = self
            .paid_hours
            .or_default(&DEFAULT_PAID_HOURS.input(), Hours::parse);
        let productivity = self
            .productivity
            .or_default(&DEFAULT_PRODUCTIVITY.input(), productivity);
        let technician_cost = self.technician_cost.required(Money::parse);
        let overhead = self.overhead.required(Money::parse);
        let parts_gross_profit = self
            .parts_profit
            .or_default(&Money::ZERO.input(), Money::parse);
        let target_profit = self
            .target_profit
            .or_default(&DEFAULT_TARGET_PROFIT.input(), target);
        let target_labor_gross_profit = self.target_labor.or_default(&LABOR.usual.input(), target);
        let input = RateNeededInput {
            technicians: technicians?,
            paid_hours: paid_hours?,
            productivity: productivity?,
            technician_cost: technician_cost?,
            overhead: overhead?,
            parts_gross_profit: parts_gross_profit?,
            target_profit: target_profit?,
            target_labor_gross_profit: target_labor_gross_profit?,
        };
        match rate_needed(&input) {
            Ok(result) => Some(result),
            Err(error) => {
                self.refuse(&error);
                None
            }
        }
    }

    /// Puts a refusal beside the field it names, or at the part.
    fn refuse(&mut self, error: &LaborRateError) {
        let field = match error.field() {
            Some(LaborField::Technicians) => Some(&mut self.technicians),
            Some(LaborField::PaidHours) => Some(&mut self.paid_hours),
            Some(LaborField::Productivity) => Some(&mut self.productivity),
            Some(LaborField::TechnicianCost) => Some(&mut self.technician_cost),
            Some(LaborField::Overhead) => Some(&mut self.overhead),
            Some(LaborField::PartsGrossProfit) => Some(&mut self.parts_profit),
            Some(LaborField::TargetProfit) => Some(&mut self.target_profit),
            Some(LaborField::TargetLaborGrossProfit) => Some(&mut self.target_labor),
            _ => None,
        };
        match field {
            Some(field) => field.refuse(error.to_string()),
            None => self.error = Some(error.to_string()),
        }
    }

    /// What to correct in this part, as the place on the page and the
    /// message, in the order of the form.
    pub fn troubles(&self) -> Vec<(String, String)> {
        troubles(NEEDED, self.error.as_ref(), &self.fields())
    }
}

/// The second part: what the shop sold in a period.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Getting {
    pub labor_sales: Field,
    pub hours_billed: Field,
    pub posted_rate: Field,
    pub period_cost: Field,
    /// Whether nothing was typed in this part.
    pub left_empty: bool,
    /// A refusal that is about this part as a whole.
    pub error: Option<String>,
}

impl Getting {
    fn new(sent: &Sent) -> Getting {
        let mut part = Getting {
            labor_sales: Field::new(sent, "labor_sales", "Labor sales in the period"),
            hours_billed: Field::new(sent, "hours_billed", "Hours billed in the period"),
            posted_rate: Field::new(sent, "posted_rate", "Posted rate, per hour (optional)"),
            period_cost: Field::new(
                sent,
                "period_cost",
                "Technician cost in the period (optional)",
            ),
            left_empty: false,
            error: None,
        };
        part.left_empty = all_empty(&part.fields());
        part
    }

    pub fn fields(&self) -> [&Field; 4] {
        [
            &self.labor_sales,
            &self.hours_billed,
            &self.posted_rate,
            &self.period_cost,
        ]
    }

    fn read(&mut self) -> Option<EffectiveRate> {
        if self.left_empty {
            return None;
        }
        let labor_sales = self.labor_sales.required(Money::parse);
        let hours_billed = self.hours_billed.required(Hours::parse);
        let posted_rate = self.posted_rate.optional(Money::parse);
        let technician_cost = self.period_cost.optional(Money::parse);
        let input = EffectiveRateInput {
            labor_sales: labor_sales?,
            hours_billed: hours_billed?,
            posted_rate: posted_rate?,
            technician_cost: technician_cost?,
        };
        match effective_rate(&input) {
            Ok(result) => Some(result),
            Err(error) => {
                self.refuse(&error);
                None
            }
        }
    }

    fn refuse(&mut self, error: &LaborRateError) {
        let field = match error.field() {
            Some(LaborField::LaborSales) => Some(&mut self.labor_sales),
            Some(LaborField::HoursBilled) => Some(&mut self.hours_billed),
            Some(LaborField::PostedRate) => Some(&mut self.posted_rate),
            Some(LaborField::PeriodTechnicianCost) => Some(&mut self.period_cost),
            _ => None,
        };
        match field {
            Some(field) => field.refuse(error.to_string()),
            None => self.error = Some(error.to_string()),
        }
    }

    pub fn troubles(&self) -> Vec<(String, String)> {
        troubles(GETTING, self.error.as_ref(), &self.fields())
    }
}

/// The form as the page shows it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Form {
    pub needed: Needed,
    pub getting: Getting,
}

impl Form {
    fn fields_of(sent: &Sent) -> Form {
        Form {
            needed: Needed::new(sent),
            getting: Getting::new(sent),
        }
    }

    /// Whether the form was sent at all: whether the query string names
    /// any of its fields. If not, the page shows its example.
    pub fn was_sent(sent: &Sent) -> bool {
        let blank = Form::fields_of(&Sent::default());
        let (needed, getting) = (blank.needed.fields(), blank.getting.fields());
        needed
            .iter()
            .chain(getting.iter())
            .any(|field| sent.has(&field.name))
    }

    /// Reads what was sent and works out each part by itself: a part has a
    /// result when every field of that part could be read and its rate was
    /// not refused, whatever became of the other part.
    pub fn read(sent: &Sent) -> (Form, Option<RateNeeded>, Option<EffectiveRate>) {
        let mut form = Form::fields_of(sent);
        let needed = form.needed.read();
        let getting = form.getting.read();
        (form, needed, getting)
    }
}

/// What a field left empty takes, and what the page does not say, as the
/// sentences under the first part of the form. Every figure is read from
/// `shop-math`.
pub fn defaults() -> String {
    format!(
        "Left empty, paid hours are {DEFAULT_PAID_HOURS} (40 hours a week), productivity is {DEFAULT_PRODUCTIVITY}, gross profit on parts is {}, target profit is {DEFAULT_TARGET_PROFIT} and target labor gross profit is {}. These are examples to start from, not advice. This page does not say what a shop should charge, and says nothing of what other shops charge.",
        Money::ZERO,
        LABOR.usual
    )
}

/// What the page shows with no query string: a shop of three technicians
/// for the first part, and a month of its sales for the second. The
/// figures that have a default are the defaults.
pub fn example() -> Sent {
    Sent::new(
        [
            ("technicians", "3".to_owned()),
            ("paid_hours", DEFAULT_PAID_HOURS.input()),
            ("productivity", DEFAULT_PRODUCTIVITY.input()),
            ("technician_cost", "18000.00".to_owned()),
            ("overhead", "25000.00".to_owned()),
            ("parts_profit", "12000.00".to_owned()),
            ("target_profit", DEFAULT_TARGET_PROFIT.input()),
            ("target_labor", LABOR.usual.input()),
            ("labor_sales", "48000.00".to_owned()),
            ("hours_billed", "520".to_owned()),
            ("posted_rate", "120.00".to_owned()),
            ("period_cost", "18000.00".to_owned()),
        ]
        .map(|(name, text)| (name.to_owned(), text))
        .to_vec(),
    )
}

#[cfg(test)]
mod tests {
    use std::cmp::Ordering;

    use super::*;

    fn read(pairs: &[(&str, &str)]) -> (Form, Option<RateNeeded>, Option<EffectiveRate>) {
        Form::read(&Sent::of(pairs))
    }

    fn messages(form: &Form) -> Vec<String> {
        form.needed
            .troubles()
            .into_iter()
            .chain(form.getting.troubles())
            .map(|(place, message)| format!("{place} | {message}"))
            .collect()
    }

    #[test]
    fn the_example_is_read_and_both_rates_are_worked_out() {
        let sent = example();
        assert_eq!(
            sent.query(),
            "technicians=3&paid_hours=173&productivity=85&technician_cost=18000.00&overhead=25000.00&parts_profit=12000.00&target_profit=10&target_labor=70&labor_sales=48000.00&hours_billed=520&posted_rate=120.00&period_cost=18000.00"
        );
        assert!(Form::was_sent(&sent));
        let (form, needed, getting) = Form::read(&sent);
        assert_eq!(messages(&form), Vec::<String>::new());
        assert!(!form.needed.left_empty && !form.getting.left_empty);
        // The figures plan 1 holds for this shop.
        let needed = needed.unwrap();
        assert_eq!(needed.hours_billed.to_string(), "441.15");
        assert_eq!(needed.labor_sales_needed.to_string(), "$34,444.44");
        assert_eq!(needed.rate_needed.to_string(), "$78.08");
        assert_eq!(needed.break_even_rate.to_string(), "$70.27");
        assert_eq!(needed.labor_gross_profit.unwrap().to_string(), "47.742%");
        assert_eq!(needed.target_rate.to_string(), "$136.01");
        assert_eq!(needed.needed_against_target, Ordering::Less);
        let getting = getting.unwrap();
        assert_eq!(getting.rate.to_string(), "$92.31");
        let posted = getting.against_posted.unwrap();
        assert_eq!(posted.percent_of_posted.to_string(), "76.925%");
        assert_eq!(posted.per_hour.to_string(), "$27.69");
        assert_eq!(posted.over_the_period.to_string(), "$14,400.00");
        assert_eq!(getting.labor_gross_profit.unwrap().to_string(), "62.5%");
    }

    #[test]
    fn a_field_left_empty_takes_its_default_and_shows_it() {
        let (form, needed, _) = read(&[
            ("technicians", "3"),
            ("paid_hours", ""),
            ("productivity", " "),
            ("technician_cost", "$18,000"),
            ("overhead", "25,000.00"),
            ("parts_profit", ""),
            ("target_profit", ""),
            ("target_labor", ""),
        ]);
        assert_eq!(messages(&form), Vec::<String>::new());
        let shown: Vec<&str> = form
            .needed
            .fields()
            .iter()
            .map(|field| field.value.as_str())
            .collect();
        assert_eq!(
            shown,
            ["3", "173", "85", "$18,000", "25,000.00", "0.00", "10", "70"]
        );
        let needed = needed.unwrap();
        assert_eq!(needed.hours_billed.to_string(), "441.15");
        // (18,000 + 25,000) / 0.9, and that over 441.15 hours.
        assert_eq!(needed.labor_sales_needed.to_string(), "$47,777.78");
        assert_eq!(needed.rate_needed.to_string(), "$108.30");
        // A field that has no default is asked for.
        let (form, needed, _) = read(&[("technicians", "3")]);
        assert!(needed.is_none());
        assert_eq!(
            messages(&form),
            [
                "technician_cost | Technician cost per month: Enter a number.",
                "overhead | Overhead per month: Enter a number.",
            ]
        );
        assert_eq!(form.needed.paid_hours.value, "173");
    }

    #[test]
    fn a_part_left_empty_is_not_worked_out_and_nothing_in_it_is_reported() {
        // Only the second part.
        let (form, needed, getting) = read(&[
            ("technicians", ""),
            ("paid_hours", ""),
            ("overhead", "  "),
            ("labor_sales", "48000"),
            ("hours_billed", "520"),
        ]);
        assert!(form.needed.left_empty && !form.getting.left_empty);
        assert_eq!(messages(&form), Vec::<String>::new());
        assert!(needed.is_none());
        // A part that is not worked out is left as it was: no default is
        // filled in.
        assert_eq!(form.needed.paid_hours.value, "");
        let getting = getting.unwrap();
        assert_eq!(getting.rate.to_string(), "$92.31");
        assert!(getting.against_posted.is_none() && getting.labor_gross_profit.is_none());
        // Only the first part.
        let (form, needed, getting) = read(&[
            ("technicians", "1"),
            ("technician_cost", "6000"),
            ("overhead", "9000"),
            ("labor_sales", ""),
        ]);
        assert!(!form.needed.left_empty && form.getting.left_empty);
        assert_eq!(messages(&form), Vec::<String>::new());
        assert!(needed.is_some() && getting.is_none());
        // Neither: the form was sent with nothing in it.
        let (form, needed, getting) = read(&[("technicians", ""), ("labor_sales", "")]);
        assert!(form.needed.left_empty && form.getting.left_empty);
        assert_eq!(messages(&form), Vec::<String>::new());
        assert!(needed.is_none() && getting.is_none());
    }

    #[test]
    fn a_field_that_cannot_be_read_is_reported_and_the_other_part_is_still_worked_out() {
        let mut pairs: Vec<(String, String)> = Vec::new();
        for name in [
            "technicians",
            "paid_hours",
            "productivity",
            "technician_cost",
            "overhead",
            "parts_profit",
            "target_profit",
            "target_labor",
            "labor_sales",
            "posted_rate",
            "period_cost",
        ] {
            pairs.push((name.to_owned(), example().text(name).to_owned()));
        }
        // A comma typed for the decimal point is not read as 5,205.
        pairs.push(("hours_billed".to_owned(), "520,5".to_owned()));
        let (form, needed, getting) = Form::read(&Sent::new(pairs));
        assert_eq!(
            messages(&form),
            [
                "hours_billed | Hours billed in the period: This is not a number. Write it like 1,234.50."
            ]
        );
        assert!(getting.is_none());
        assert_eq!(needed.unwrap().rate_needed.to_string(), "$78.08");
        assert_eq!(form.getting.hours_billed.value, "520,5");
        assert_eq!(form.getting.labor_sales.value, "48000.00");
        // Each field has its own limits, and says them.
        let (form, needed, getting) = read(&[
            ("technicians", "2.5"),
            ("paid_hours", "abc"),
            ("productivity", "500"),
            ("technician_cost", "-1"),
            ("overhead", "1.005"),
            ("target_profit", "100"),
            ("target_labor", "99.5"),
            ("labor_sales", "100000000"),
            ("hours_billed", "$5"),
            ("posted_rate", "x"),
        ]);
        assert!(needed.is_none() && getting.is_none());
        assert_eq!(
            messages(&form),
            [
                "technicians | Technicians: Use a whole number.",
                "paid_hours | Paid hours per technician, per month: This is not a number. Write it like 1,234.50.",
                "productivity | Productivity, percent: This must be from 1% to 200%.",
                "technician_cost | Technician cost per month: This cannot be below zero.",
                "overhead | Overhead per month: Use at most 2 decimal places.",
                "target_profit | Target profit, percent of labor sales: This must be from 0% to 99%.",
                "target_labor | Target labor gross profit, percent: This must be from 0% to 99%.",
                "labor_sales | Labor sales in the period: This must be from 0 to 99,999,999.99.",
                "hours_billed | Hours billed in the period: This is not a number. Write it like 1,234.50.",
                "posted_rate | Posted rate, per hour (optional): This is not a number. Write it like 1,234.50.",
            ]
        );
    }

    #[test]
    fn no_hours_and_a_posted_rate_of_zero_are_reported_beside_their_fields() {
        let shop = [("technician_cost", "6000"), ("overhead", "9000")];
        for (pairs, place) in [
            (vec![("technicians", "0")], "technicians"),
            (
                vec![("technicians", "2"), ("paid_hours", "0")],
                "paid_hours",
            ),
            // Hours too few to count as a hundredth of an hour.
            (
                vec![
                    ("technicians", "1"),
                    ("paid_hours", "0.01"),
                    ("productivity", "1"),
                ],
                "paid_hours",
            ),
        ] {
            let pairs: Vec<(&str, &str)> = pairs.into_iter().chain(shop).collect();
            let (form, needed, _) = read(&pairs);
            assert!(needed.is_none(), "{place}");
            let troubles = form.needed.troubles();
            assert_eq!(troubles.len(), 1, "{troubles:?}");
            assert_eq!(troubles[0].0, place);
            assert!(
                troubles[0]
                    .1
                    .ends_with(": No hours would be billed with this."),
                "{troubles:?}"
            );
        }
        let (form, _, getting) = read(&[("labor_sales", "48000"), ("hours_billed", "0")]);
        assert!(getting.is_none());
        assert_eq!(
            messages(&form),
            ["hours_billed | Hours billed in the period: No hours would be billed with this."]
        );
        let (form, _, getting) = read(&[
            ("labor_sales", "48000"),
            ("hours_billed", "520"),
            ("posted_rate", "0"),
        ]);
        assert!(getting.is_none());
        assert_eq!(
            messages(&form),
            ["posted_rate | Posted rate, per hour (optional): This cannot be zero."]
        );
        // No sales in the period is a rate of nothing, not a refusal.
        let (form, _, getting) = read(&[
            ("labor_sales", "0"),
            ("hours_billed", "520"),
            ("period_cost", "18000"),
        ]);
        assert_eq!(messages(&form), Vec::<String>::new());
        let getting = getting.unwrap();
        assert_eq!(getting.rate.to_string(), "$0.00");
        assert!(getting.labor_gross_profit.is_none());
    }

    #[test]
    fn the_form_was_sent_when_the_address_names_one_of_its_fields() {
        for name in [
            "technicians",
            "paid_hours",
            "productivity",
            "technician_cost",
            "overhead",
            "parts_profit",
            "target_profit",
            "target_labor",
            "labor_sales",
            "hours_billed",
            "posted_rate",
            "period_cost",
        ] {
            assert!(Form::was_sent(&Sent::of(&[(name, "")])), "{name}");
        }
        for name in ["utm_source", "ref", "Technicians", "cost1", "kind", "hours"] {
            assert!(!Form::was_sent(&Sent::of(&[(name, "x")])), "{name}");
        }
        assert!(!Form::was_sent(&Sent::default()));
    }

    #[test]
    fn the_defaults_are_named_as_examples_with_the_figures_shop_math_holds() {
        assert_eq!(
            defaults(),
            "Left empty, paid hours are 173 (40 hours a week), productivity is 85%, gross profit on parts is $0.00, target profit is 10% and target labor gross profit is 70%. These are examples to start from, not advice. This page does not say what a shop should charge, and says nothing of what other shops charge."
        );
    }

    #[test]
    fn nothing_that_can_be_typed_stops_the_page() {
        let names: Vec<String> = Form::fields_of(&Sent::default())
            .needed
            .fields()
            .iter()
            .chain(Form::fields_of(&Sent::default()).getting.fields().iter())
            .map(|field| field.name.clone())
            .collect();
        assert_eq!(names.len(), 12);
        let long = "9".repeat(9_000);
        for text in [
            "99999999999999999999999999",
            "-0",
            "1e308",
            "NaN",
            "\u{663}",
            "\u{0}",
            "<script>alert(1)</script>",
            "%",
            "$",
            ",",
            ".",
            "0",
            "0.01",
            "1",
            "99",
            "200",
            "1000",
            "99,999,999.99",
            long.as_str(),
        ] {
            let pairs = names
                .iter()
                .map(|name| (name.clone(), text.to_owned()))
                .collect();
            let (form, needed, getting) = Form::read(&Sent::new(pairs));
            // A part that gave no result says why.
            assert!(
                needed.is_some() || !form.needed.troubles().is_empty(),
                "{text:.40}"
            );
            assert!(
                getting.is_some() || !form.getting.troubles().is_empty(),
                "{text:.40}"
            );
        }
        // The largest figures every field accepts, and the smallest hours.
        let (form, needed, getting) = read(&[
            ("technicians", "1"),
            ("paid_hours", "0.01"),
            ("productivity", "200"),
            ("technician_cost", "99,999,999.99"),
            ("overhead", "99,999,999.99"),
            ("target_profit", "99"),
            ("target_labor", "99"),
            ("labor_sales", "99,999,999.99"),
            ("hours_billed", "0.01"),
            ("posted_rate", "99,999,999.99"),
            ("period_cost", "99,999,999.99"),
        ]);
        assert_eq!(messages(&form), Vec::<String>::new());
        assert_eq!(
            needed.unwrap().rate_needed.to_string(),
            "$999,999,999,900.00"
        );
        assert_eq!(getting.unwrap().rate.to_string(), "$9,999,999,999.00");
        let (form, needed, getting) = read(&[
            ("technicians", "1000"),
            ("paid_hours", "99,999,999.99"),
            ("productivity", "200"),
            ("technician_cost", "0"),
            ("overhead", "0"),
            ("labor_sales", "0"),
            ("hours_billed", "99,999,999.99"),
            ("posted_rate", "99,999,999.99"),
        ]);
        assert!(needed.is_some(), "{:?}", messages(&form));
        assert!(
            getting.is_some() || !form.getting.troubles().is_empty(),
            "{:?}",
            messages(&form)
        );
    }
}
