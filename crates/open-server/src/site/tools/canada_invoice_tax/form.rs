//! The form of the Canadian invoice page: a province, the invoice's three
//! amounts, and a number of new tires for each class the province has.
//!
//! The page has no script, so the form shows the tire classes of the
//! province that was sent. A class's field is named for the class, and a
//! count sent for a class the province does not have is left out and said
//! to be: it is never moved into another class.

use shop_math::invoice_tax::{InvoiceField, InvoiceTax, InvoiceTaxError};
use shop_math::{Money, parse_count};

use crate::site::tools::canada_invoice_tax::rates::{PROVINCES, Province, province};
use crate::site::tools::field::{Field, Sent};

/// The province the page's own example is in.
const EXAMPLE_PROVINCE: &str = "BC";

/// One entry of the list of provinces.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Choice {
    pub code: &'static str,
    pub name: &'static str,
    pub selected: bool,
}

/// The form as the page shows it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Form {
    /// The list of provinces. Its `value` is the code of the province
    /// chosen, or nothing; what was sent is never shown again.
    pub province: Field,
    /// The province the form is for, when one was sent that is confirmed.
    pub chosen: Option<&'static Province>,
    pub labour: Field,
    pub parts: Field,
    pub supplies: Field,
    /// One field for each tire class of the province chosen, in the order
    /// of the rates file. Empty when no province is chosen or it has none.
    pub tires: Vec<Field>,
    /// Said above the result when a tire count was sent for a class the
    /// province chosen does not have.
    pub left_out: Option<String>,
    /// A refusal that is about the form as a whole.
    pub error: Option<String>,
}

/// An invoice that was worked out, with what it was worked out from.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Worked {
    pub province: &'static Province,
    pub labour: Money,
    pub parts: Money,
    pub supplies: Money,
    pub result: InvoiceTax,
}

/// The fields every province's form has.
const FIXED: [&str; 4] = ["province", "labour", "parts", "supplies"];

/// Whether `text` holds a number of tires other than none.
fn is_a_count(text: &str) -> bool {
    !text.trim().is_empty() && parse_count(text) != Ok(0)
}

impl Form {
    fn fields_of(sent: &Sent, chosen: Option<&'static Province>) -> Form {
        let tires = chosen
            .iter()
            .flat_map(|province| province.tire_classes)
            .map(|class| Field::new(sent, class.field(), format!("New tires: {}", class.label)))
            .collect();
        Form {
            province: Field::new(sent, "province", "Province or territory"),
            chosen,
            labour: Field::new(sent, "labour", "Labour"),
            parts: Field::new(sent, "parts", "Parts"),
            supplies: Field::new(sent, "supplies", "Shop supplies (optional)"),
            tires,
            left_out: None,
            error: None,
        }
    }

    /// The provinces the calculator covers, for the list.
    pub fn covered(&self) -> Vec<Choice> {
        self.choices(true)
    }

    /// The provinces it does not cover yet. They are in the list, and
    /// cannot be chosen.
    pub fn not_covered(&self) -> Vec<Choice> {
        self.choices(false)
    }

    fn choices(&self, confirmed: bool) -> Vec<Choice> {
        PROVINCES
            .iter()
            .filter(|province| province.confirmed == confirmed)
            .map(|province| Choice {
                code: province.code,
                name: province.name,
                selected: self
                    .chosen
                    .is_some_and(|chosen| chosen.code == province.code),
            })
            .collect()
    }

    /// Whether the form was sent at all: whether the query string names
    /// any of its fields, for any province. If not, the page shows its
    /// example.
    pub fn was_sent(sent: &Sent) -> bool {
        FIXED.iter().any(|name| sent.has(name))
            || PROVINCES
                .iter()
                .flat_map(|province| province.tire_classes)
                .any(|class| sent.has(&class.field()))
    }

    /// Reads what was sent and works the invoice out. There is a result
    /// only when a confirmed province was chosen, every field could be
    /// read and the figures were not refused. Shop supplies and a tire
    /// count left empty are nothing, and show it.
    pub fn read(sent: &Sent) -> (Form, Option<Worked>) {
        let code = sent.text("province").trim();
        let found = province(code);
        let chosen = found.filter(|province| province.confirmed);
        let mut form = Form::fields_of(sent, chosen);
        form.province.value = chosen.map_or("", |province| province.code).to_owned();
        match (found, chosen) {
            (_, Some(_)) => {}
            (Some(province), None) => form.province.refuse(format!(
                "{} is not covered yet. {}",
                province.name,
                province.missing.join(" ")
            )),
            (None, _) if code.is_empty() => form.province.refuse("Choose a province or territory."),
            (None, _) => form.province.refuse(
                "This is not a province or territory this page knows. Choose one from the list.",
            ),
        }
        let labour = form.labour.required(Money::parse);
        let parts = form.parts.required(Money::parse);
        let supplies = form.supplies.or_default(&Money::ZERO.input(), Money::parse);
        let counts: Vec<Option<u32>> = form
            .tires
            .iter_mut()
            .map(|field| field.or_default("0", parse_count))
            .collect();
        let Some(province) = chosen else {
            return (form, None);
        };
        form.left_out = left_out(sent, province);
        let counts: Option<Vec<u32>> = counts.into_iter().collect();
        let (Some(labour), Some(parts), Some(supplies), Some(counts)) =
            (labour, parts, supplies, counts)
        else {
            return (form, None);
        };
        match province.invoice(labour, parts, supplies, &counts) {
            Some(Ok(result)) => {
                let worked = Worked {
                    province,
                    labour,
                    parts,
                    supplies,
                    result,
                };
                (form, Some(worked))
            }
            Some(Err(error)) => {
                form.refuse(&error);
                (form, None)
            }
            None => (form, None),
        }
    }

    /// Puts a refusal beside the field it names, or at the form.
    fn refuse(&mut self, error: &InvoiceTaxError) {
        let field = match error.field() {
            Some(InvoiceField::Labour) => Some(&mut self.labour),
            Some(InvoiceField::Parts) => Some(&mut self.parts),
            Some(InvoiceField::Supplies) => Some(&mut self.supplies),
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
        let fixed = [&self.province, &self.labour, &self.parts, &self.supplies];
        troubles.extend(
            fixed
                .into_iter()
                .chain(&self.tires)
                .filter_map(Field::trouble),
        );
        troubles
    }
}

/// What the page says when a number of tires was sent for a class
/// `chosen` does not have: the class of another province.
fn left_out(sent: &Sent, chosen: &Province) -> Option<String> {
    let has = |key: &str| chosen.tire_classes.iter().any(|class| class.key == key);
    let stray = PROVINCES
        .iter()
        .flat_map(|province| province.tire_classes)
        .any(|class| !has(class.key) && is_a_count(sent.text(&class.field())));
    if !stray {
        return None;
    }
    let name = chosen.name;
    Some(if chosen.tire_classes.is_empty() {
        format!(
            "A number of tires was sent for a tire class of another province. It was not used: {name} has no tire classes."
        )
    } else {
        format!(
            "A number of tires was sent for a class {name} does not have. It was not used: enter the tires in {name}'s classes above."
        )
    })
}

/// What the page shows with no query string: a repair with four new tires
/// of the first class the example's province has.
pub fn example() -> Sent {
    let mut pairs = vec![
        ("province".to_owned(), EXAMPLE_PROVINCE.to_owned()),
        ("labour".to_owned(), "180.00".to_owned()),
        ("parts".to_owned(), "640.00".to_owned()),
        ("supplies".to_owned(), "14.37".to_owned()),
    ];
    let first = province(EXAMPLE_PROVINCE).and_then(|province| province.tire_classes.first());
    if let Some(class) = first {
        pairs.push((class.field(), "4".to_owned()));
    }
    Sent::new(pairs)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cents(cents: i64) -> Money {
        Money::from_cents(cents)
    }

    fn values(fields: &[Field]) -> Vec<(&str, &str)> {
        fields
            .iter()
            .map(|field| (field.name.as_str(), field.value.as_str()))
            .collect()
    }

    #[test]
    fn the_form_was_sent_when_the_address_names_one_of_its_fields() {
        for name in [
            "province",
            "labour",
            "parts",
            "supplies",
            "tires_passenger_light_truck",
            "tires_rim_over_17_in",
        ] {
            assert!(Form::was_sent(&Sent::of(&[(name, "")])), "{name}");
        }
        // Someone else's marker on a link is not the form.
        for name in ["utm_source", "ref", "labor", "tires", "tires_", "cost1"] {
            assert!(!Form::was_sent(&Sent::of(&[(name, "x")])), "{name}");
        }
        assert!(!Form::was_sent(&Sent::default()));
        assert!(Form::was_sent(&example()));
    }

    #[test]
    fn the_example_is_read_and_worked_out() {
        let sent = example();
        assert_eq!(
            sent.query(),
            "province=BC&labour=180.00&parts=640.00&supplies=14.37&tires_passenger_light_truck=4"
        );
        let (form, worked) = Form::read(&sent);
        assert!(form.troubles().is_empty(), "{:?}", form.troubles());
        assert_eq!(form.left_out, None);
        let worked = worked.unwrap();
        assert_eq!(worked.province.code, "BC");
        // 834.37 and four tires at 6.50: 860.37. 5% is 43.0185 and 7% is
        // 60.2259. 834.37 + 26.00 + 43.02 + 60.23 = 963.62.
        assert_eq!(worked.result.subtotal, cents(83_437));
        assert_eq!(worked.result.fees[0].amount, cents(2_600));
        assert_eq!(worked.result.taxes[0].amount, cents(4_302));
        assert_eq!(worked.result.taxes[1].amount, cents(6_023));
        assert_eq!(worked.result.total, cents(96_362));
        // The form shows the province's two classes, and the one left
        // empty shows the nothing that was used.
        assert_eq!(
            values(&form.tires),
            [
                ("tires_passenger_light_truck", "4"),
                ("tires_medium_truck", "0")
            ]
        );
        assert_eq!(form.tires[0].label, "New tires: passenger and light truck");
        let chosen: Vec<&str> = form
            .covered()
            .iter()
            .filter(|choice| choice.selected)
            .map(|choice| choice.code)
            .collect();
        assert_eq!(chosen, ["BC"]);
        assert!(form.not_covered().iter().all(|choice| !choice.selected));
        assert_eq!(
            form.covered().len() + form.not_covered().len(),
            PROVINCES.len()
        );
    }

    #[test]
    fn supplies_and_tire_counts_left_empty_are_nothing_and_the_form_shows_it() {
        // A tire shop: no labour, the tires in parts, nothing else typed.
        let sent = Sent::of(&[
            ("province", "ab"),
            ("labour", "0"),
            ("parts", "$1,200"),
            ("supplies", " "),
            ("tires_passenger_light_truck", ""),
        ]);
        let (form, worked) = Form::read(&sent);
        assert!(form.troubles().is_empty(), "{:?}", form.troubles());
        assert_eq!(form.province.value, "AB");
        assert_eq!(form.parts.value, "$1,200");
        assert_eq!(form.supplies.value, "0.00");
        assert_eq!(
            values(&form.tires),
            [
                ("tires_passenger_light_truck", "0"),
                ("tires_medium_truck", "0")
            ]
        );
        let worked = worked.unwrap();
        assert_eq!(worked.supplies, Money::ZERO);
        assert_eq!(worked.result.subtotal, cents(120_000));
        assert!(worked.result.fees.iter().all(|fee| fee.count == 0));
        // A repair with no tires and no tire fields sent at all.
        let sent = Sent::of(&[("province", "ON"), ("labour", "100"), ("parts", "0")]);
        let (form, worked) = Form::read(&sent);
        assert!(form.tires.is_empty());
        assert_eq!(worked.unwrap().result.subtotal, cents(10_000));
    }

    #[test]
    fn the_form_shows_the_tire_classes_of_the_province_chosen() {
        for (code, fields) in [
            (
                "QC",
                &[
                    "tires_diameter_83_82_cm_or_less",
                    "tires_diameter_over_83_82_cm_to_123_19_cm",
                ][..],
            ),
            ("PE", &["tires_rim_17_in_or_less", "tires_rim_over_17_in"]),
            (
                "NL",
                &["tires_rim_17_in_or_less", "tires_rim_over_17_in_to_24_5_in"],
            ),
            ("ON", &[]),
        ] {
            let sent = Sent::of(&[("province", code), ("labour", "1"), ("parts", "1")]);
            let (form, worked) = Form::read(&sent);
            let names: Vec<&str> = form.tires.iter().map(|field| field.name.as_str()).collect();
            assert_eq!(names, fields, "{code}");
            assert!(worked.is_some(), "{code}");
        }
        // Every confirmed province's form has one field for each of its
        // classes, labelled with the words of the rates file.
        for province in PROVINCES.iter().filter(|province| province.confirmed) {
            let (form, _) = Form::read(&Sent::of(&[("province", province.code)]));
            assert_eq!(form.tires.len(), province.tire_classes.len());
            for (field, class) in form.tires.iter().zip(province.tire_classes) {
                assert_eq!(field.label, format!("New tires: {}", class.label));
            }
        }
    }

    #[test]
    fn a_tire_count_for_another_province_s_class_is_left_out_and_the_form_says_so() {
        // The example's four tires, and Quebec chosen in the list.
        let sent = Sent::of(&[
            ("province", "QC"),
            ("labour", "180.00"),
            ("parts", "640.00"),
            ("supplies", "14.37"),
            ("tires_passenger_light_truck", "4"),
            ("tires_medium_truck", "0"),
        ]);
        let (form, worked) = Form::read(&sent);
        assert_eq!(
            form.left_out.as_deref(),
            Some(
                "A number of tires was sent for a class Quebec does not have. It was not used: enter the tires in Quebec's classes above."
            )
        );
        // Quebec's own fields, each empty of tires, and a result without
        // a tire fee. It is not a refusal.
        assert_eq!(
            values(&form.tires),
            [
                ("tires_diameter_83_82_cm_or_less", "0"),
                ("tires_diameter_over_83_82_cm_to_123_19_cm", "0")
            ]
        );
        assert!(form.troubles().is_empty());
        let worked = worked.unwrap();
        assert!(
            worked
                .result
                .fees
                .iter()
                .all(|fee| fee.amount == Money::ZERO)
        );
        // A province with no classes says that instead.
        let sent = Sent::of(&[
            ("province", "ON"),
            ("labour", "1"),
            ("parts", "1"),
            ("tires_medium_truck", "abc"),
        ]);
        let (form, worked) = Form::read(&sent);
        assert_eq!(
            form.left_out.as_deref(),
            Some(
                "A number of tires was sent for a tire class of another province. It was not used: Ontario has no tire classes."
            )
        );
        assert!(worked.is_some());
        // A class two provinces share carries over, and a count of none
        // sent for another's class is nothing to speak of.
        let sent = Sent::of(&[
            ("province", "AB"),
            ("labour", "1"),
            ("parts", "1"),
            ("tires_passenger_light_truck", "4"),
            ("tires_rim_over_17_in", "0"),
            ("tires_rim_17_in_or_less", ""),
        ]);
        let (form, worked) = Form::read(&sent);
        assert_eq!(form.left_out, None);
        assert_eq!(worked.unwrap().result.fees[0].count, 4);
    }

    #[test]
    fn an_unknown_or_empty_province_is_no_province_chosen_and_a_message() {
        for (code, message) in [
            ("", "Choose a province or territory."),
            ("  ", "Choose a province or territory."),
            (
                "XX",
                "This is not a province or territory this page knows. Choose one from the list.",
            ),
            (
                "British Columbia",
                "This is not a province or territory this page knows. Choose one from the list.",
            ),
            (
                "\"><script>",
                "This is not a province or territory this page knows. Choose one from the list.",
            ),
        ] {
            let sent = Sent::of(&[
                ("province", code),
                ("labour", "100"),
                ("parts", "200"),
                ("tires_passenger_light_truck", "4"),
            ]);
            let (form, worked) = Form::read(&sent);
            assert!(worked.is_none(), "{code}");
            assert_eq!(form.chosen, None);
            assert_eq!(form.province.error.as_deref(), Some(message), "{code}");
            // What was sent is not kept: the list shows nothing chosen.
            assert_eq!(form.province.value, "");
            assert!(form.covered().iter().all(|choice| !choice.selected));
            // No province, so no tire fields; the other fields keep what
            // was typed, and only the province is to be corrected.
            assert!(form.tires.is_empty());
            assert_eq!(form.labour.value, "100");
            assert_eq!(
                form.troubles(),
                [(
                    "province".to_owned(),
                    format!("Province or territory: {message}")
                )]
            );
            assert_eq!(form.left_out, None);
        }
        // The province not sent at all, with another field sent.
        let (form, worked) = Form::read(&Sent::of(&[("labour", "100"), ("parts", "1")]));
        assert!(worked.is_none());
        assert_eq!(
            form.province.error.as_deref(),
            Some("Choose a province or territory.")
        );
    }

    #[test]
    fn a_province_that_is_not_confirmed_says_what_is_missing_and_works_nothing_out() {
        for province in PROVINCES.iter().filter(|province| !province.confirmed) {
            let sent = Sent::of(&[
                ("province", province.code),
                ("labour", "100"),
                ("parts", "200"),
            ]);
            let (form, worked) = Form::read(&sent);
            assert!(worked.is_none(), "{}", province.code);
            assert_eq!(form.chosen, None);
            let message = form.province.error.clone().unwrap();
            assert!(
                message.starts_with(&format!("{} is not covered yet. ", province.name)),
                "{message}"
            );
            for missing in province.missing {
                assert!(message.contains(missing), "{message}");
            }
            assert!(form.covered().iter().all(|choice| !choice.selected));
            assert!(form.not_covered().iter().all(|choice| !choice.selected));
            assert!(
                form.not_covered()
                    .iter()
                    .any(|choice| choice.code == province.code)
            );
            assert!(form.tires.is_empty());
        }
    }

    #[test]
    fn a_field_that_cannot_be_read_is_reported_and_the_rest_keep_what_was_typed() {
        let sent = Sent::of(&[
            ("province", "SK"),
            ("labour", "180,5"),
            ("parts", ""),
            ("supplies", "abc"),
            ("tires_passenger_light_truck", "2.5"),
            ("tires_medium_truck", "1001"),
        ]);
        let (form, worked) = Form::read(&sent);
        assert!(worked.is_none());
        assert_eq!(form.chosen.unwrap().code, "SK");
        assert_eq!(
            form.troubles(),
            [
                (
                    "labour".to_owned(),
                    "Labour: This is not a number. Write it like 1,234.50.".to_owned()
                ),
                ("parts".to_owned(), "Parts: Enter a number.".to_owned()),
                (
                    "supplies".to_owned(),
                    "Shop supplies (optional): This is not a number. Write it like 1,234.50."
                        .to_owned()
                ),
                (
                    "tires_passenger_light_truck".to_owned(),
                    "New tires: passenger and light truck: Use a whole number.".to_owned()
                ),
                (
                    "tires_medium_truck".to_owned(),
                    "New tires: medium truck: This must be from 0 to 1,000.".to_owned()
                ),
            ]
        );
        assert_eq!(form.labour.value, "180,5");
        assert_eq!(form.supplies.value, "abc");
        assert_eq!(form.tires[0].value, "2.5");
        // One bad field among good ones.
        let sent = Sent::of(&[
            ("province", "SK"),
            ("labour", "180"),
            ("parts", "-5"),
            ("tires_medium_truck", "2"),
        ]);
        let (form, worked) = Form::read(&sent);
        assert!(worked.is_none());
        assert_eq!(
            form.troubles(),
            [(
                "parts".to_owned(),
                "Parts: This cannot be below zero.".to_owned()
            )]
        );
        assert_eq!(form.labour.value, "180");
        assert_eq!(form.tires[1].value, "2");
    }

    #[test]
    fn an_invoice_of_nothing_is_reported_at_the_form() {
        let sent = Sent::of(&[
            ("province", "MB"),
            ("labour", "0"),
            ("parts", "0.00"),
            ("supplies", ""),
            ("tires_passenger_light_truck", "0"),
        ]);
        let (form, worked) = Form::read(&sent);
        assert!(worked.is_none());
        assert_eq!(
            form.error.as_deref(),
            Some("Enter labour, parts, shop supplies or a number of new tires.")
        );
        assert_eq!(
            form.troubles(),
            [(
                "calc".to_owned(),
                "Enter labour, parts, shop supplies or a number of new tires.".to_owned()
            )]
        );
        // Tires alone are an invoice.
        let sent = Sent::of(&[
            ("province", "MB"),
            ("labour", "0"),
            ("parts", "0"),
            ("tires_passenger_light_truck", "4"),
        ]);
        assert!(Form::read(&sent).1.is_some());
    }

    #[test]
    fn nothing_that_can_be_typed_stops_the_page() {
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
            for code in ["BC", "QC", "ON", "NT", text] {
                let sent = Sent::of(&[
                    ("province", code),
                    ("labour", text),
                    ("parts", text),
                    ("supplies", text),
                    ("tires_passenger_light_truck", text),
                    ("tires_diameter_83_82_cm_or_less", text),
                ]);
                let (form, _) = Form::read(&sent);
                let _ = form.troubles();
            }
        }
        // The largest of everything a field accepts, in every province.
        for province in PROVINCES.iter() {
            let mut pairs = vec![
                ("province".to_owned(), province.code.to_owned()),
                ("labour".to_owned(), "99,999,999.99".to_owned()),
                ("parts".to_owned(), "99,999,999.99".to_owned()),
                ("supplies".to_owned(), "99,999,999.99".to_owned()),
            ];
            for class in province.tire_classes {
                pairs.push((class.field(), "1,000".to_owned()));
            }
            let (form, worked) = Form::read(&Sent::new(pairs));
            assert_eq!(worked.is_some(), province.confirmed, "{}", province.code);
            assert_eq!(form.error, None);
        }
    }
}
