//! The form of the battery page: the kind of battery, its open-circuit
//! voltage, and, if it is known, its temperature in °C or °F.
//!
//! Only a battery whose table is confirmed can be chosen. Until one is, the
//! page has no form and no result.

use shop_math::battery_soc::{parse_volts, volts};
use shop_math::chart::{celsius_from_fahrenheit, parse_tenths};

use crate::site::tools::battery_soc::data::Battery;
use crate::site::tools::canada_invoice_tax::form::Choice;
use crate::site::tools::field::{Field, Sent};

/// The form as the page shows it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Form {
    /// The list of batteries. Its `value` is the key of the battery chosen,
    /// or what was typed when it is not on the list.
    pub battery: Field,
    /// The battery the form is for, when one was sent that is confirmed.
    pub chosen: Option<&'static Battery>,
    pub volts: Field,
    pub temperature: Field,
    /// Its value is "c" or "f".
    pub unit: Field,
    choices: &'static [Battery],
}

/// What was read: enough to work the result out.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Reading {
    pub battery: &'static Battery,
    pub millivolts: i64,
    pub tenths_c: Option<i64>,
}

impl Form {
    /// Whether the form was sent at all: whether the query string names any
    /// of its fields. If not, the page shows its example.
    pub fn was_sent(sent: &Sent) -> bool {
        ["battery", "volts", "temperature", "unit"]
            .iter()
            .any(|name| sent.has(name))
    }

    /// Reads what was sent. There is a reading only when a confirmed battery
    /// was chosen, the voltage and the temperature (if any) could be read.
    pub fn read(sent: &Sent, batteries: &'static [Battery]) -> (Form, Option<Reading>) {
        let mut form = Form {
            battery: Field::new(sent, "battery", "Kind of battery"),
            chosen: None,
            volts: Field::new(sent, "volts", "Open-circuit voltage, volts"),
            temperature: Field::new(sent, "temperature", "Battery temperature (optional)"),
            unit: Field::new(sent, "unit", "Temperature is in"),
            choices: batteries,
        };
        let typed = form.battery.value.trim().to_owned();
        let chosen = if typed.is_empty() {
            batteries.iter().find(|battery| battery.confirmed)
        } else {
            let found = batteries
                .iter()
                .find(|battery| battery.confirmed && battery.key == typed);
            if found.is_none() {
                form.battery
                    .refuse("Choose a kind of battery from the list.");
            }
            found
        };
        form.chosen = chosen;
        if let Some(battery) = chosen {
            form.battery.value = battery.key.to_owned();
        }
        let millivolts = form.volts.required(parse_volts);
        let fahrenheit = form.fahrenheit();
        let temperature = form.temperature.optional(parse_tenths).map(|tenths| {
            tenths.map(|tenths| {
                if fahrenheit {
                    celsius_from_fahrenheit(tenths)
                } else {
                    tenths
                }
            })
        });
        let reading = match (chosen, millivolts, temperature) {
            (Some(battery), Some(millivolts), Some(tenths_c)) => Some(Reading {
                battery,
                millivolts,
                tenths_c,
            }),
            _ => None,
        };
        (form, reading)
    }

    /// The batteries whose tables are confirmed, for the list.
    pub fn covered(&self) -> Vec<Choice> {
        self.choices(true)
    }

    /// The batteries not yet confirmed. They are in the list, and cannot be
    /// chosen.
    pub fn not_covered(&self) -> Vec<Choice> {
        self.choices(false)
    }

    fn choices(&self, confirmed: bool) -> Vec<Choice> {
        self.choices
            .iter()
            .filter(|battery| battery.confirmed == confirmed)
            .map(|battery| Choice {
                code: battery.key,
                name: battery.name,
                selected: self.chosen.is_some_and(|chosen| chosen.key == battery.key),
            })
            .collect()
    }

    /// Whether the temperature was typed in °F.
    pub fn fahrenheit(&self) -> bool {
        self.unit.value.trim().eq_ignore_ascii_case("f")
    }

    /// What to correct, as the place on the page and the message, in the
    /// order of the form.
    pub fn troubles(&self) -> Vec<(String, String)> {
        [&self.battery, &self.volts, &self.temperature]
            .into_iter()
            .filter_map(Field::trouble)
            .collect()
    }
}

/// The example the page shows when the form was not sent: the first
/// confirmed battery and the voltage of the middle row of its table, with no
/// temperature. `None` when no battery is confirmed.
pub fn example(batteries: &'static [Battery]) -> Option<Sent> {
    let battery = batteries.iter().find(|battery| battery.confirmed)?;
    let (millivolts, _) = *battery.rows.get(battery.rows.len() / 2)?;
    Some(Sent::new(vec![
        ("battery".to_owned(), battery.key.to_owned()),
        ("volts".to_owned(), volts(millivolts)),
        ("temperature".to_owned(), String::new()),
        ("unit".to_owned(), "c".to_owned()),
    ]))
}

#[cfg(test)]
pub mod tests {
    use super::*;
    use crate::site::tools::battery_soc::data::{Correction, Source};

    const PAGE: Source = Source {
        title: "A made-up page",
        url: "https://example.org/battery",
        archived: None,
    };
    /// Made up: no figure here is a real battery's.
    pub static FAKES: [Battery; 2] = [
        Battery {
            key: "made_up",
            name: "Made-up battery",
            as_of: "2026-01-31",
            confirmed: true,
            missing: &[],
            notes: &[],
            conditions: &["A condition the page shows."],
            rows: &[(10_000, 0), (11_000, 50), (12_000, 100)],
            correction: Some(Correction {
                reference_tenths_c: 250,
                microvolts_per_degree: 10_000,
            }),
            correction_sources: &[PAGE],
            sources: &[PAGE],
        },
        Battery {
            key: "not_read",
            name: "Battery not read",
            as_of: "2026-01-31",
            confirmed: false,
            missing: &["No table has been read."],
            notes: &[],
            conditions: &[],
            rows: &[],
            correction: None,
            correction_sources: &[],
            sources: &[],
        },
    ];

    #[test]
    fn a_reading_is_read_with_its_battery_and_its_temperature_in_celsius() {
        let (form, reading) = Form::read(
            &Sent::of(&[
                ("battery", "made_up"),
                ("volts", "11.4 V"),
                ("temperature", "41"),
                ("unit", "f"),
            ]),
            &FAKES,
        );
        assert!(form.troubles().is_empty() && form.fahrenheit());
        let reading = reading.unwrap();
        assert_eq!(
            (reading.battery.key, reading.millivolts, reading.tenths_c),
            ("made_up", 11_400, Some(50))
        );
        let (_, reading) = Form::read(
            &Sent::of(&[("volts", "11"), ("temperature", "-5.5")]),
            &FAKES,
        );
        assert_eq!(reading.unwrap().tenths_c, Some(-55));
        let (form, reading) = Form::read(&Sent::of(&[("volts", "11")]), &FAKES);
        assert_eq!(
            (
                form.chosen.map(|battery| battery.key),
                reading.unwrap().tenths_c
            ),
            (Some("made_up"), None)
        );
        assert_eq!(form.covered().len(), 1);
        assert_eq!(form.not_covered()[0].name, "Battery not read");
    }

    #[test]
    fn what_cannot_be_read_is_reported_beside_its_field() {
        let (form, reading) = Form::read(
            &Sent::of(&[
                ("battery", "not_read"),
                ("volts", ""),
                ("temperature", "cold"),
            ]),
            &FAKES,
        );
        assert!(reading.is_none());
        let troubles: Vec<String> = form
            .troubles()
            .into_iter()
            .map(|(place, message)| format!("{place} | {message}"))
            .collect();
        assert_eq!(
            troubles,
            [
                "battery | Kind of battery: Choose a kind of battery from the list.",
                "volts | Open-circuit voltage, volts: Enter a number.",
                "temperature | Battery temperature (optional): This is not a number. Write it like 1,234.50.",
            ]
        );
    }

    #[test]
    fn the_example_is_the_middle_row_of_the_first_confirmed_battery() {
        assert_eq!(
            example(&FAKES).unwrap().query(),
            "battery=made_up&volts=11.00&temperature=&unit=c"
        );
        assert!(example(&FAKES[1..]).is_none());
        assert!(
            Form::was_sent(&Sent::of(&[("volts", "")]))
                && !Form::was_sent(&Sent::of(&[("utm_source", "x")]))
        );
    }
}
