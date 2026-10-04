//! What a calculator's form sent, and one field of it.
//!
//! A form's method is `get`, so what was typed arrives in the query string.
//! A field that cannot be read keeps what was typed and gets a message;
//! nothing here can fail.

use shop_math::ParseError;

/// The fields a form sent, as name and text, in the order they were sent.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Sent(Vec<(String, String)>);

impl Sent {
    pub fn new(pairs: Vec<(String, String)>) -> Sent {
        Sent(pairs)
    }

    /// The same from borrowed text, for an example, a preset or a test.
    pub fn of(pairs: &[(&str, &str)]) -> Sent {
        Sent(
            pairs
                .iter()
                .map(|(name, text)| ((*name).to_owned(), (*text).to_owned()))
                .collect(),
        )
    }

    /// What was typed into the field `name`, or nothing. A name sent twice
    /// is read once, the first time.
    pub fn text(&self, name: &str) -> &str {
        self.0
            .iter()
            .find(|(sent, _)| sent == name)
            .map_or("", |(_, text)| text.as_str())
    }

    /// Whether the form sent the field `name`, filled in or not.
    pub fn has(&self, name: &str) -> bool {
        self.0.iter().any(|(sent, _)| sent == name)
    }

    /// The fields as a query string. Only for text made of letters, digits
    /// and points, which need no escaping: examples and presets.
    pub fn query(&self) -> String {
        self.0
            .iter()
            .map(|(name, text)| format!("{name}={text}"))
            .collect::<Vec<_>>()
            .join("&")
    }
}

/// One field of a form as the page shows it: what was typed, and the
/// message beside it when it could not be read.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Field {
    /// The field's name in the query string, and its `id` on the page.
    pub name: String,
    pub label: String,
    /// What was typed, shown again as it was.
    pub value: String,
    pub error: Option<String>,
}

impl Field {
    pub fn new(sent: &Sent, name: impl Into<String>, label: impl Into<String>) -> Field {
        let name = name.into();
        Field {
            value: sent.text(&name).to_owned(),
            name,
            label: label.into(),
            error: None,
        }
    }

    /// Whether nothing was typed. Spaces are nothing.
    pub fn is_empty(&self) -> bool {
        self.value.trim().is_empty()
    }

    /// The `id` of the message beside the field.
    pub fn error_id(&self) -> String {
        format!("{}-error", self.name)
    }

    /// The field's line in the list of what to correct: where to go, and
    /// what is wrong there. `None` when the field was read.
    pub fn trouble(&self) -> Option<(String, String)> {
        let error = self.error.as_ref()?;
        Some((self.name.clone(), format!("{}: {error}", self.label)))
    }

    /// Puts a message beside the field.
    pub fn refuse(&mut self, message: impl Into<String>) {
        self.error = Some(message.into());
    }

    /// Reads a field that must be filled in. `None` means it could not be
    /// read, and the field now carries the message.
    pub fn required<T>(&mut self, parse: impl FnOnce(&str) -> Result<T, ParseError>) -> Option<T> {
        match parse(&self.value) {
            Ok(read) => Some(read),
            Err(error) => {
                self.refuse(error.to_string());
                None
            }
        }
    }

    /// Reads a field that may be left empty: `Some(None)` when it was.
    pub fn optional<T>(
        &mut self,
        parse: impl FnOnce(&str) -> Result<T, ParseError>,
    ) -> Option<Option<T>> {
        if self.is_empty() {
            return Some(None);
        }
        self.required(parse).map(Some)
    }

    /// Reads a field that has a documented default. Left empty, it is
    /// filled with `default` as text, so the page shows what was used.
    pub fn or_default<T>(
        &mut self,
        default: &str,
        parse: impl FnOnce(&str) -> Result<T, ParseError>,
    ) -> Option<T> {
        if self.is_empty() {
            self.value = default.to_owned();
        }
        self.required(parse)
    }
}

#[cfg(test)]
mod tests {
    use shop_math::{Hours, Money, Percent};

    use super::*;

    #[test]
    fn a_field_sent_twice_is_read_once_and_a_field_not_sent_is_empty() {
        let sent = Sent::of(&[("cost", "5.00"), ("cost", "9.00"), ("rate", "")]);
        assert_eq!(sent.text("cost"), "5.00");
        assert_eq!(sent.text("rate"), "");
        assert_eq!(sent.text("share"), "");
        assert!(sent.has("rate") && !sent.has("share"));
        assert_eq!(sent.query(), "cost=5.00&cost=9.00&rate=");
        assert_eq!(Sent::default().query(), "");
    }

    #[test]
    fn a_field_that_is_read_has_no_message_and_keeps_what_was_typed() {
        let sent = Sent::of(&[("cost", " $1,234.50 ")]);
        let mut field = Field::new(&sent, "cost", "Cost");
        assert_eq!(
            field.required(Money::parse),
            Some(Money::from_cents(123_450))
        );
        assert_eq!(field.value, " $1,234.50 ");
        assert_eq!(field.error, None);
        assert_eq!(field.error_id(), "cost-error");
    }

    #[test]
    fn a_field_that_cannot_be_read_gets_the_message_and_keeps_what_was_typed() {
        for (typed, message) in [
            ("abc", "This is not a number. Write it like 1,234.50."),
            ("1,5", "This is not a number. Write it like 1,234.50."),
            ("-5", "This cannot be below zero."),
            ("12.345", "Use at most 2 decimal places."),
            ("", "Enter a number."),
            ("   ", "Enter a number."),
            ("100000000", "This must be from 0 to 99,999,999.99."),
            ("<b>1</b>", "This is not a number. Write it like 1,234.50."),
        ] {
            let sent = Sent::of(&[("cost", typed)]);
            let mut field = Field::new(&sent, "cost", "Cost");
            assert_eq!(field.required(Money::parse), None, "{typed}");
            assert_eq!(field.value, typed);
            assert_eq!(field.error.as_deref(), Some(message), "{typed}");
        }
    }

    #[test]
    fn a_field_left_empty_is_nothing_where_that_is_allowed() {
        let sent = Sent::of(&[("part", "  "), ("share", "x")]);
        let mut part = Field::new(&sent, "part", "Part");
        assert_eq!(part.optional(Money::parse), Some(None));
        assert_eq!(part.error, None);
        let mut share = Field::new(&sent, "share", "Share");
        assert_eq!(share.optional(Percent::parse), None);
        assert!(share.error.is_some());
        let mut missing = Field::new(&sent, "other", "Other");
        assert_eq!(missing.optional(Money::parse), Some(None));
    }

    #[test]
    fn a_field_left_empty_takes_its_default_and_shows_it() {
        let sent = Sent::of(&[("hours", ""), ("rate", "90")]);
        let mut hours = Field::new(&sent, "hours", "Paid hours");
        assert_eq!(
            hours.or_default("173", Hours::parse),
            Some(Hours::from_hundredths(17_300))
        );
        assert_eq!(hours.value, "173");
        let mut rate = Field::new(&sent, "rate", "Productivity");
        let between =
            |text: &str| Percent::parse_between(text, Percent::whole(1), Percent::whole(200));
        assert_eq!(rate.or_default("85", between), Some(Percent::whole(90)));
        assert_eq!(rate.value, "90");
        // What was typed and cannot be read is reported, not replaced.
        let sent = Sent::of(&[("rate", "500")]);
        let mut rate = Field::new(&sent, "rate", "Productivity");
        assert_eq!(rate.or_default("85", between), None);
        assert_eq!(rate.value, "500");
        assert_eq!(rate.error.as_deref(), Some("This must be from 1% to 200%."));
    }

    #[test]
    fn a_calculator_can_put_its_own_message_beside_a_field() {
        let mut field = Field::new(&Sent::default(), "hours", "Hours");
        assert_eq!(field.trouble(), None);
        field.refuse("No hours would be billed with this.");
        assert_eq!(
            field.trouble(),
            Some((
                "hours".to_owned(),
                "Hours: No hours would be billed with this.".to_owned()
            ))
        );
        assert_eq!(
            field.error.as_deref(),
            Some("No hours would be billed with this.")
        );
    }
}
