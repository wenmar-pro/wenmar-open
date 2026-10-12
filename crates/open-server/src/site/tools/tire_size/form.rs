//! The form of the tire size page: the size on the vehicle now, and the
//! size to compare with it.

use shop_math::tire_size::TireSize;

use crate::site::tools::field::{Field, Sent};

/// The form as the page shows it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Form {
    pub first: Field,
    pub second: Field,
}

impl Form {
    fn fields_of(sent: &Sent) -> Form {
        Form {
            first: Field::new(sent, "first", "Size on the vehicle now"),
            second: Field::new(sent, "second", "Size to compare"),
        }
    }

    pub fn fields(&self) -> [&Field; 2] {
        [&self.first, &self.second]
    }

    /// Whether the form was sent at all: whether the query string names
    /// either of its fields. If not, the page shows its example.
    pub fn was_sent(sent: &Sent) -> bool {
        sent.has("first") || sent.has("second")
    }

    /// Reads both sizes. There is a comparison only when both could be
    /// read; each field that could not be read gets its message, and both
    /// are read before either result is looked at, so both messages show.
    pub fn read(sent: &Sent) -> (Form, Option<(TireSize, TireSize)>) {
        let mut form = Form::fields_of(sent);
        let first = size(&mut form.first);
        let second = size(&mut form.second);
        let sizes = match (first, second) {
            (Some(first), Some(second)) => Some((first, second)),
            _ => None,
        };
        (form, sizes)
    }

    /// What to correct, as the place on the page and the message, in the
    /// order of the form.
    pub fn troubles(&self) -> Vec<(String, String)> {
        self.fields()
            .iter()
            .filter_map(|field| field.trouble())
            .collect()
    }
}

fn size(field: &mut Field) -> Option<TireSize> {
    match TireSize::parse(&field.value) {
        Ok(size) => Some(size),
        Err(error) => {
            field.refuse(error.to_string());
            None
        }
    }
}

/// What the page shows with no query string: the two sizes of the
/// comparison the page opens with.
pub fn example() -> Sent {
    Sent::of(&[("first", "275/65R18"), ("second", "LT285/70R17")])
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_example_is_read() {
        let sent = example();
        assert_eq!(sent.query(), "first=275/65R18&second=LT285/70R17");
        assert!(Form::was_sent(&sent));
        let (form, sizes) = Form::read(&sent);
        assert!(form.troubles().is_empty());
        let (first, second) = sizes.unwrap();
        assert_eq!(format!("{first} {second}"), "275/65R18 LT285/70R17");
    }

    #[test]
    fn each_field_that_cannot_be_read_says_why_and_keeps_what_was_typed() {
        let (form, sizes) = Form::read(&Sent::of(&[("first", "35x12.50R17"), ("second", " ")]));
        assert!(sizes.is_none());
        assert_eq!(form.first.value, "35x12.50R17");
        let troubles: Vec<String> = form
            .troubles()
            .into_iter()
            .map(|(place, message)| format!("{place} | {message}"))
            .collect();
        assert_eq!(
            troubles,
            [
                "first | Size on the vehicle now: This is not a tire size this page reads. Write it like 275/65R18 or LT285/70R17.",
                "second | Size to compare: Enter a tire size.",
            ]
        );
    }

    #[test]
    fn the_form_was_sent_when_the_address_names_one_of_its_fields() {
        assert!(
            Form::was_sent(&Sent::of(&[("first", "")]))
                && Form::was_sent(&Sent::of(&[("second", "x")]))
        );
        assert!(
            !Form::was_sent(&Sent::of(&[("utm_source", "x"), ("First", "x")]))
                && !Form::was_sent(&Sent::default())
        );
    }

    #[test]
    fn nothing_that_can_be_typed_stops_the_page() {
        let long = "9".repeat(9_000);
        for text in [
            "",
            "-0",
            "1e308",
            "NaN",
            "\u{0}",
            "<script>alert(1)</script>",
            "/",
            "R",
            "999/99R99",
            long.as_str(),
        ] {
            let (form, sizes) = Form::read(&Sent::of(&[("first", text), ("second", text)]));
            assert!(sizes.is_some() || form.troubles().len() == 2, "{text:.40}");
        }
    }
}
