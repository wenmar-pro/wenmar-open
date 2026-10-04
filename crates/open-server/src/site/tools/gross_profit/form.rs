//! The form of the gross profit page: what labor, parts and sublet sold
//! for in one period, and what each cost.

use shop_math::Money;
use shop_math::gross_profit::{
    GrossProfit, GrossProfitError, GrossProfitField, GrossProfitInput, Line, gross_profit,
};

use crate::site::tools::field::{Field, Sent};

/// The form as the page shows it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Form {
    pub labor_sales: Field,
    pub technician_cost: Field,
    pub parts_sales: Field,
    pub parts_cost: Field,
    pub sublet_sales: Field,
    pub sublet_cost: Field,
    /// A refusal that is about the form as a whole.
    pub error: Option<String>,
}

impl Form {
    fn fields_of(sent: &Sent) -> Form {
        Form {
            labor_sales: Field::new(sent, "labor_sales", "Labor sales"),
            technician_cost: Field::new(sent, "technician_cost", "Technician cost"),
            parts_sales: Field::new(sent, "parts_sales", "Parts sales"),
            parts_cost: Field::new(sent, "parts_cost", "Parts cost"),
            sublet_sales: Field::new(sent, "sublet_sales", "Sublet sales (optional)"),
            sublet_cost: Field::new(sent, "sublet_cost", "Sublet cost (optional)"),
            error: None,
        }
    }

    pub fn fields(&self) -> [&Field; 6] {
        [
            &self.labor_sales,
            &self.technician_cost,
            &self.parts_sales,
            &self.parts_cost,
            &self.sublet_sales,
            &self.sublet_cost,
        ]
    }

    /// Whether the form was sent at all: whether the query string names
    /// any of its fields. If not, the page shows its example.
    pub fn was_sent(sent: &Sent) -> bool {
        Form::fields_of(&Sent::default())
            .fields()
            .iter()
            .any(|field| sent.has(&field.name))
    }

    /// Reads what was sent and works the gross profit out. There is a
    /// result only when every field could be read and the figures were not
    /// refused. A sublet field left empty is nothing, and shows it.
    pub fn read(sent: &Sent) -> (Form, Option<GrossProfit>) {
        let mut form = Form::fields_of(sent);
        let nothing = Money::ZERO.input();
        let labor_sales = form.labor_sales.required(Money::parse);
        let technician_cost = form.technician_cost.required(Money::parse);
        let parts_sales = form.parts_sales.required(Money::parse);
        let parts_cost = form.parts_cost.required(Money::parse);
        let sublet_sales = form.sublet_sales.or_default(&nothing, Money::parse);
        let sublet_cost = form.sublet_cost.or_default(&nothing, Money::parse);
        let (
            Some(labor_sales),
            Some(technician_cost),
            Some(parts_sales),
            Some(parts_cost),
            Some(sublet_sales),
            Some(sublet_cost),
        ) = (
            labor_sales,
            technician_cost,
            parts_sales,
            parts_cost,
            sublet_sales,
            sublet_cost,
        )
        else {
            return (form, None);
        };
        let input = GrossProfitInput {
            labor: Line {
                sales: labor_sales,
                cost: technician_cost,
            },
            parts: Line {
                sales: parts_sales,
                cost: parts_cost,
            },
            sublet: Line {
                sales: sublet_sales,
                cost: sublet_cost,
            },
        };
        match gross_profit(&input) {
            Ok(result) => (form, Some(result)),
            Err(error) => {
                form.refuse(&error);
                (form, None)
            }
        }
    }

    /// Puts a refusal beside the field it names, or at the form.
    fn refuse(&mut self, error: &GrossProfitError) {
        let field = match error.field() {
            Some(GrossProfitField::LaborSales) => Some(&mut self.labor_sales),
            Some(GrossProfitField::TechnicianCost) => Some(&mut self.technician_cost),
            Some(GrossProfitField::PartsSales) => Some(&mut self.parts_sales),
            Some(GrossProfitField::PartsCost) => Some(&mut self.parts_cost),
            Some(GrossProfitField::SubletSales) => Some(&mut self.sublet_sales),
            Some(GrossProfitField::SubletCost) => Some(&mut self.sublet_cost),
            None => None,
        };
        match field {
            Some(field) => field.refuse(error.to_string()),
            None => self.error = Some(error.to_string()),
        }
    }

    /// What to correct, as the place on the page and the message, in the
    /// order of the form.
    pub fn troubles(&self) -> Vec<(String, String)> {
        let mut troubles: Vec<(String, String)> = Vec::new();
        if let Some(error) = &self.error {
            troubles.push(("calc".to_owned(), error.clone()));
        }
        troubles.extend(self.fields().iter().filter_map(|field| field.trouble()));
        troubles
    }
}

/// What the page shows with no query string: one month of a shop whose
/// labor is inside its range and whose parts are below theirs.
pub fn example() -> Sent {
    Sent::of(&[
        ("labor_sales", "48000.00"),
        ("technician_cost", "18000.00"),
        ("parts_sales", "40000.00"),
        ("parts_cost", "26000.00"),
        ("sublet_sales", "2000.00"),
        ("sublet_cost", "1600.00"),
    ])
}

#[cfg(test)]
mod tests {
    use shop_math::targets::Standing;

    use super::*;

    fn read(pairs: &[(&str, &str)]) -> (Form, Option<GrossProfit>) {
        Form::read(&Sent::of(pairs))
    }

    fn messages(form: &Form) -> Vec<String> {
        form.troubles()
            .into_iter()
            .map(|(place, message)| format!("{place} | {message}"))
            .collect()
    }

    #[test]
    fn the_example_is_read_and_worked_out_as_plan_one_has_it() {
        let sent = example();
        assert_eq!(
            sent.query(),
            "labor_sales=48000.00&technician_cost=18000.00&parts_sales=40000.00&parts_cost=26000.00&sublet_sales=2000.00&sublet_cost=1600.00"
        );
        assert!(Form::was_sent(&sent));
        let (form, result) = Form::read(&sent);
        assert_eq!(messages(&form), Vec::<String>::new());
        let result = result.unwrap();
        assert_eq!(result.labor.profit.to_string(), "$30,000.00");
        assert_eq!(result.labor.percent.unwrap().to_string(), "62.5%");
        assert_eq!(result.labor.standing, Some(Standing::Inside));
        assert!(result.labor.shortfall.is_none());
        assert_eq!(result.parts.percent.unwrap().to_string(), "35%");
        assert_eq!(result.parts.standing, Some(Standing::Below));
        let short = result.parts.shortfall.unwrap();
        assert_eq!(short.sales_needed.to_string(), "$52,000.00");
        assert_eq!(short.difference.to_string(), "$12,000.00");
        assert_eq!(result.sublet.percent.unwrap().to_string(), "20%");
        assert!(result.sublet.standing.is_none());
        assert_eq!(result.overall.sales.to_string(), "$90,000.00");
        assert_eq!(result.overall.cost.to_string(), "$45,600.00");
        assert_eq!(result.overall.profit.to_string(), "$44,400.00");
        assert_eq!(result.overall.percent.unwrap().to_string(), "49.333%");
        assert_eq!(result.overall.standing, Some(Standing::Below));
    }

    #[test]
    fn sublet_left_empty_is_nothing_and_the_form_shows_it() {
        let (form, result) = read(&[
            ("labor_sales", "$48,000"),
            ("technician_cost", "18,000.00"),
            ("parts_sales", "40000"),
            ("parts_cost", "26000"),
            ("sublet_sales", ""),
            ("sublet_cost", "  "),
        ]);
        assert_eq!(messages(&form), Vec::<String>::new());
        // What was typed is kept as typed; what was left empty shows the
        // nothing that was used.
        let shown: Vec<&str> = form
            .fields()
            .iter()
            .map(|field| field.value.as_str())
            .collect();
        assert_eq!(
            shown,
            ["$48,000", "18,000.00", "40000", "26000", "0.00", "0.00"]
        );
        let result = result.unwrap();
        assert_eq!(result.sublet.sales, Money::ZERO);
        assert!(result.sublet.percent.is_none());
        assert_eq!(result.overall.sales.to_string(), "$88,000.00");
        // Sublet not sent at all is the same.
        let (form, again) = read(&[
            ("labor_sales", "48000"),
            ("technician_cost", "18000"),
            ("parts_sales", "40000"),
            ("parts_cost", "26000"),
        ]);
        assert_eq!(messages(&form), Vec::<String>::new());
        assert_eq!(again.unwrap(), result);
    }

    #[test]
    fn labor_and_parts_are_asked_for_and_a_line_with_no_sales_is_typed_as_zero() {
        let (form, result) = read(&[("labor_sales", "48000"), ("technician_cost", "18000")]);
        assert!(result.is_none());
        assert_eq!(
            messages(&form),
            [
                "parts_sales | Parts sales: Enter a number.",
                "parts_cost | Parts cost: Enter a number.",
            ]
        );
        // The fields that were read keep what was typed.
        assert_eq!(form.labor_sales.value, "48000");
        // A shop that sold no parts says so with a zero.
        let (form, result) = read(&[
            ("labor_sales", "48000"),
            ("technician_cost", "18000"),
            ("parts_sales", "0"),
            ("parts_cost", "0"),
        ]);
        assert_eq!(messages(&form), Vec::<String>::new());
        let result = result.unwrap();
        assert!(result.parts.percent.is_none() && result.parts.standing.is_none());
        assert!(result.parts.shortfall.is_none());
        assert_eq!(result.overall.percent.unwrap().to_string(), "62.5%");
    }

    #[test]
    fn a_field_that_cannot_be_read_is_reported_and_the_rest_keep_what_was_typed() {
        let (form, result) = read(&[
            ("labor_sales", "48,0"),
            ("technician_cost", "abc"),
            ("parts_sales", "-1"),
            ("parts_cost", "26000.005"),
            ("sublet_sales", "100000000"),
            ("sublet_cost", "1600"),
        ]);
        assert!(result.is_none());
        assert_eq!(
            messages(&form),
            [
                "labor_sales | Labor sales: This is not a number. Write it like 1,234.50.",
                "technician_cost | Technician cost: This is not a number. Write it like 1,234.50.",
                "parts_sales | Parts sales: This cannot be below zero.",
                "parts_cost | Parts cost: Use at most 2 decimal places.",
                "sublet_sales | Sublet sales (optional): This must be from 0 to 99,999,999.99.",
            ]
        );
        assert_eq!(form.labor_sales.value, "48,0");
        assert_eq!(form.sublet_cost.value, "1600");
        assert!(form.sublet_cost.error.is_none() && form.error.is_none());
    }

    #[test]
    fn no_sales_on_any_line_is_reported_at_the_form() {
        for pairs in [
            vec![
                ("labor_sales", "0"),
                ("technician_cost", "0"),
                ("parts_sales", "0"),
                ("parts_cost", "0"),
            ],
            // Costs with nothing sold are still no sales.
            vec![
                ("labor_sales", "0"),
                ("technician_cost", "18000"),
                ("parts_sales", "0.00"),
                ("parts_cost", "500"),
                ("sublet_cost", "20"),
            ],
        ] {
            let (form, result) = read(&pairs);
            assert!(result.is_none());
            assert_eq!(
                messages(&form),
                ["calc | Enter sales for labor, parts or sublet."]
            );
            assert!(form.fields().iter().all(|field| field.error.is_none()));
        }
    }

    #[test]
    fn the_form_was_sent_when_the_address_names_one_of_its_fields() {
        for name in [
            "labor_sales",
            "technician_cost",
            "parts_sales",
            "parts_cost",
            "sublet_sales",
            "sublet_cost",
        ] {
            assert!(Form::was_sent(&Sent::of(&[(name, "")])), "{name}");
        }
        for name in ["utm_source", "ref", "Labor_sales", "sales", "cost1", "kind"] {
            assert!(!Form::was_sent(&Sent::of(&[(name, "x")])), "{name}");
        }
        assert!(!Form::was_sent(&Sent::default()));
    }

    #[test]
    fn nothing_that_can_be_typed_stops_the_page() {
        let names: Vec<String> = Form::fields_of(&Sent::default())
            .fields()
            .iter()
            .map(|field| field.name.clone())
            .collect();
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
            "99,999,999.99",
            long.as_str(),
        ] {
            let pairs = names
                .iter()
                .map(|name| (name.clone(), text.to_owned()))
                .collect();
            let (form, result) = Form::read(&Sent::new(pairs));
            // A form that gave no result says why.
            assert!(
                result.is_some() || !form.troubles().is_empty(),
                "{text:.40}"
            );
        }
        // The largest figure in every field is a result, and the overall
        // line holds the sum.
        let most = "99,999,999.99";
        let (form, result) = read(&[
            ("labor_sales", most),
            ("technician_cost", "0"),
            ("parts_sales", most),
            ("parts_cost", most),
            ("sublet_sales", most),
            ("sublet_cost", "0.01"),
        ]);
        assert_eq!(messages(&form), Vec::<String>::new());
        assert_eq!(result.unwrap().overall.sales.to_string(), "$299,999,999.97");
        // The largest cost against the smallest sale: a loss, not a refusal.
        let (form, result) = read(&[
            ("labor_sales", "0.01"),
            ("technician_cost", most),
            ("parts_sales", "0"),
            ("parts_cost", "0"),
        ]);
        assert!(
            result.is_some() || !form.troubles().is_empty(),
            "{:?}",
            messages(&form)
        );
    }
}
