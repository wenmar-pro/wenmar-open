//! The form of the parts matrix page: eight rows of a cost, a percent and
//! a share, how the percent is read, and one part's cost.

use shop_math::parts_matrix::{
    MAX_TIERS, MatrixError, MatrixResult, RateKind, TierInput, work_out,
};
use shop_math::{Money, Percent};

use crate::site::tools::field::{Field, Sent};

/// The name of the choice between markup and margin, and of the field for
/// one part's cost.
pub const KIND: &str = "kind";
pub const PART: &str = "part";

/// One row of the form. Its fields are named `cost3`, `rate3` and `share3`
/// in row 3.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Row {
    /// The row's number on the form, from 1. Messages name it.
    pub number: usize,
    pub cost: Field,
    pub rate: Field,
    pub share: Field,
}

impl Row {
    fn new(sent: &Sent, number: usize) -> Row {
        Row {
            number,
            cost: Field::new(sent, format!("cost{number}"), "Cost up to"),
            rate: Field::new(sent, format!("rate{number}"), "Percent"),
            share: Field::new(sent, format!("share{number}"), "Share"),
        }
    }

    pub fn fields(&self) -> [&Field; 3] {
        [&self.cost, &self.rate, &self.share]
    }

    /// Whether nothing was typed in the row. Such a row is ignored.
    pub fn is_empty(&self) -> bool {
        self.fields().iter().all(|field| field.is_empty())
    }

    /// Reads the row. A message for a field of a row says which row and
    /// which field, because it is shown under the row and not the field.
    fn read(&mut self) -> Option<TierInput> {
        let number = self.number;
        let cost_up_to = self.cost.required(Money::parse);
        let rate = self.rate.required(Percent::parse);
        let share = self.share.optional(Percent::parse);
        for field in [&mut self.cost, &mut self.rate, &mut self.share] {
            if let Some(error) = field.error.take() {
                let label = field.label.to_lowercase();
                field.refuse(format!("Row {number}, {label}: {error}"));
            }
        }
        Some(TierInput {
            row: number,
            cost_up_to: cost_up_to?,
            rate: rate?,
            share: share?,
        })
    }
}

/// The form as the page shows it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Form {
    /// Whether the percent of every row is read as a margin. If not, it is
    /// a markup.
    pub margin: bool,
    pub rows: Vec<Row>,
    pub part: Field,
    /// A refusal that is about the form as a whole.
    pub error: Option<String>,
}

impl Form {
    /// Whether the form was sent at all: whether the query string names
    /// any of its fields. If not, the page shows its example.
    pub fn was_sent(sent: &Sent) -> bool {
        let blank = Form::fields_of(&Sent::default());
        sent.has(KIND)
            || sent.has(PART)
            || blank
                .rows
                .iter()
                .any(|row| row.fields().iter().any(|field| sent.has(&field.name)))
    }

    fn fields_of(sent: &Sent) -> Form {
        Form {
            margin: sent.text(KIND).trim() == "margin",
            rows: (1..=MAX_TIERS)
                .map(|number| Row::new(sent, number))
                .collect(),
            part: Field::new(sent, PART, "One part's cost (optional)"),
            error: None,
        }
    }

    /// Reads what was sent and works the matrix out. There is a result only
    /// when every field could be read and the matrix was not refused.
    pub fn read(sent: &Sent) -> (Form, Option<MatrixResult>) {
        let mut form = Form::fields_of(sent);
        let kind = if form.margin {
            RateKind::Margin
        } else {
            RateKind::Markup
        };
        let mut tiers = Vec::new();
        let mut readable = true;
        for row in form.rows.iter_mut().filter(|row| !row.is_empty()) {
            match row.read() {
                Some(tier) => tiers.push(tier),
                None => readable = false,
            }
        }
        let part = form.part.optional(Money::parse);
        let (true, Some(part)) = (readable, part) else {
            return (form, None);
        };
        match work_out(kind, &tiers, part) {
            Ok(result) => (form, Some(result)),
            Err(error) => {
                form.refuse(&error);
                (form, None)
            }
        }
    }

    /// Puts a refusal of the matrix where it belongs: beside the field of
    /// the row it names, or at the form.
    fn refuse(&mut self, error: &MatrixError) {
        let message = error.to_string();
        let row = error
            .row()
            .and_then(|number| self.rows.iter_mut().find(|row| row.number == number));
        match (row, error) {
            (Some(row), MatrixError::NotRising { .. }) => row.cost.refuse(message),
            (Some(row), _) => row.rate.refuse(message),
            (None, _) => self.error = Some(message),
        }
    }

    /// What to correct, as the place on the page and the message, in the
    /// order of the form.
    pub fn troubles(&self) -> Vec<(String, String)> {
        let mut troubles: Vec<(String, String)> = Vec::new();
        if let Some(error) = &self.error {
            troubles.push(("calc".to_owned(), error.clone()));
        }
        for field in self.rows.iter().flat_map(Row::fields) {
            if let Some(error) = &field.error {
                troubles.push((field.name.clone(), error.clone()));
            }
        }
        troubles.extend(self.part.trouble());
        troubles
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn read(pairs: &[(&str, &str)]) -> (Form, Option<MatrixResult>) {
        Form::read(&Sent::of(pairs))
    }

    fn messages(form: &Form) -> Vec<String> {
        form.troubles()
            .into_iter()
            .map(|(place, message)| format!("{place} | {message}"))
            .collect()
    }

    #[test]
    fn a_filled_form_is_read_and_worked_out() {
        let (form, result) = read(&[
            ("kind", "markup"),
            ("cost1", "25.00"),
            ("rate1", "100"),
            ("share1", "40"),
            ("cost2", "$1,000"),
            ("rate2", "50%"),
            ("share2", "60"),
            ("part", "42.50"),
        ]);
        assert_eq!(messages(&form), Vec::<String>::new());
        assert!(!form.margin);
        assert_eq!(form.rows.len(), 8);
        // Every field shows what was typed, as it was typed.
        assert_eq!(form.rows[1].cost.value, "$1,000");
        assert_eq!(form.rows[1].rate.value, "50%");
        let result = result.unwrap();
        assert_eq!(result.tiers.len(), 2);
        let part = result.part.unwrap();
        assert_eq!((part.row, part.price.to_string()), (2, "$63.75".to_owned()));
        assert_eq!(result.blended_margin.unwrap().to_string(), "41.176%");
    }

    #[test]
    fn empty_rows_are_ignored_and_a_message_names_the_row_as_the_form_numbers_it() {
        let (form, result) = read(&[
            ("cost2", "100"),
            ("rate2", "50"),
            ("cost3", "  "),
            ("cost5", "100"),
            ("rate5", "40"),
        ]);
        assert!(result.is_none());
        assert_eq!(
            messages(&form),
            ["cost5 | Row 5: \"cost up to\" must be higher than in the row above."]
        );
        assert!(form.rows[2].is_empty() && form.rows[2].cost.error.is_none());
        // The other fields keep what was typed.
        assert_eq!(form.rows[1].cost.value, "100");
        assert_eq!(form.rows[4].rate.value, "40");
    }

    #[test]
    fn a_field_that_cannot_be_read_is_reported_with_its_row_and_its_name() {
        let (form, result) = read(&[
            ("cost1", "1,5"),
            ("rate1", "abc"),
            ("share1", "-3"),
            ("cost2", "100"),
            ("part", "12.345"),
        ]);
        assert!(result.is_none());
        assert_eq!(
            messages(&form),
            [
                "cost1 | Row 1, cost up to: This is not a number. Write it like 1,234.50.",
                "rate1 | Row 1, percent: This is not a number. Write it like 1,234.50.",
                "share1 | Row 1, share: This cannot be below zero.",
                "rate2 | Row 2, percent: Enter a number.",
                "part | One part's cost (optional): Use at most 2 decimal places.",
            ]
        );
        assert_eq!(form.rows[0].cost.value, "1,5");
        assert_eq!(form.rows[1].cost.value, "100");
        assert_eq!(form.part.value, "12.345");
    }

    #[test]
    fn a_margin_of_a_hundred_percent_is_refused_beside_its_row() {
        let (form, result) = read(&[("kind", "margin"), ("cost1", "100"), ("rate1", "100")]);
        assert!(result.is_none() && form.margin);
        assert_eq!(
            messages(&form),
            ["rate1 | Row 1: a margin must be below 100%."]
        );
        // The same figures as a markup are a matrix.
        let (form, result) = read(&[("kind", "markup"), ("cost1", "100"), ("rate1", "100")]);
        assert!(result.is_some() && form.troubles().is_empty());
    }

    #[test]
    fn shares_that_do_not_add_to_a_hundred_are_refused_at_the_form_with_their_total() {
        let thirds = [
            ("cost1", "10"),
            ("rate1", "100"),
            ("share1", "33.333"),
            ("cost2", "20"),
            ("rate2", "80"),
            ("share2", "33.333"),
            ("cost3", "30"),
            ("rate3", "60"),
            ("share3", "33.333"),
        ];
        let (form, result) = read(&thirds);
        assert!(result.is_none());
        assert_eq!(
            messages(&form),
            ["calc | The shares of parts spend add to 99.999%. They must add to 100%."]
        );
    }

    #[test]
    fn a_form_sent_with_nothing_in_it_asks_for_a_tier() {
        let (form, result) = read(&[("kind", "markup"), ("cost1", ""), ("part", "")]);
        assert!(result.is_none());
        assert_eq!(
            messages(&form),
            ["calc | Enter at least one tier: a cost and a percent."]
        );
    }

    #[test]
    fn the_form_was_sent_when_the_address_names_one_of_its_fields() {
        for name in ["kind", "part", "cost1", "rate8", "share4"] {
            assert!(Form::was_sent(&Sent::of(&[(name, "")])), "{name}");
        }
        for name in ["utm_source", "cost9", "cost0", "cost", "Kind", "rate"] {
            assert!(!Form::was_sent(&Sent::of(&[(name, "x")])), "{name}");
        }
        assert!(!Form::was_sent(&Sent::default()));
    }

    #[test]
    fn nothing_that_can_be_typed_stops_the_page() {
        let hostile = [
            "99999999999999999999999999",
            "-0",
            "1e308",
            "NaN",
            "٣",
            "\u{0}",
            "<script>alert(1)</script>",
            "%",
            "$",
            ",",
            ".",
            "0",
            "99,999,999.99",
        ];
        let long = "9".repeat(9_000);
        for text in hostile.iter().copied().chain([long.as_str()]) {
            for kind in ["markup", "margin", text] {
                let mut pairs = vec![(KIND.to_owned(), kind.to_owned())];
                for number in 1..=9 {
                    for name in ["cost", "rate", "share"] {
                        pairs.push((format!("{name}{number}"), text.to_owned()));
                    }
                }
                pairs.push((PART.to_owned(), text.to_owned()));
                let (form, result) = Form::read(&Sent::new(pairs));
                // A form that gave no result says why.
                assert!(
                    result.is_some() || !form.troubles().is_empty(),
                    "{text:.40}"
                );
            }
        }
        // The largest figures a field accepts give a result.
        let (form, result) = read(&[
            ("cost1", "99,999,999.99"),
            ("rate1", "1000"),
            ("share1", "100"),
            ("part", "99,999,999.99"),
        ]);
        assert_eq!(messages(&form), Vec::<String>::new());
        assert_eq!(
            result.unwrap().part.unwrap().price.to_string(),
            "$1,099,999,999.89"
        );
    }
}
