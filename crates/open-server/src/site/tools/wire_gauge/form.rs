//! The form of the wire gauge page: the system voltage, the current, the
//! length of wire the current flows through, the unit of that length, and
//! the voltage drop allowed.

use shop_math::Percent;
use shop_math::voltage_drop::{
    Circuit, DEFAULT_ALLOWED, LEAST_ALLOWED, MOST_ALLOWED, PER_HUNDREDTH_FOOT, PER_HUNDREDTH_METRE,
    parse_current, parse_length,
};

use crate::site::tools::field::{Field, Sent};

/// The form as the page shows it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Form {
    pub system: Field,  // "system", "System voltage"; "12" or "24"
    pub current: Field, // "current", "Current, amperes"
    pub length: Field,  // "length", "Length of wire the current flows through"
    pub unit: Field,    // "unit", "Length is in"; "m" or "ft"
    pub allowed: Field, // "allowed", "Voltage drop allowed, percent"
}

impl Form {
    pub fn fields(&self) -> [&Field; 5] {
        [
            &self.system,
            &self.current,
            &self.length,
            &self.unit,
            &self.allowed,
        ]
    }

    /// Whether the form was sent at all: whether the query string names any
    /// of its fields. If not, the page shows its example.
    pub fn was_sent(sent: &Sent) -> bool {
        ["system", "current", "length", "unit", "allowed"]
            .iter()
            .any(|name| sent.has(name))
    }

    /// Reads what was sent and says what the circuit is. There is a circuit
    /// only when every field could be read.
    pub fn read(sent: &Sent) -> (Form, Option<Circuit>) {
        let mut form = Form {
            system: Field::new(sent, "system", "System voltage"),
            current: Field::new(sent, "current", "Current, amperes"),
            length: Field::new(sent, "length", "Length of wire the current flows through"),
            unit: Field::new(sent, "unit", "Length is in"),
            allowed: Field::new(sent, "allowed", "Voltage drop allowed, percent"),
        };
        let system = match form.system.value.trim() {
            "" | "12" => Some(12_000),
            "24" => Some(24_000),
            _ => {
                form.system.refuse("Choose 12 V or 24 V.");
                None
            }
        };
        let current = form.current.required(parse_current);
        let length = form.length.required(parse_length);
        let per_unit = if form.feet() {
            PER_HUNDREDTH_FOOT
        } else {
            PER_HUNDREDTH_METRE
        };
        let allowed = form.allowed.or_default(&DEFAULT_ALLOWED.input(), |text| {
            Percent::parse_between(text, LEAST_ALLOWED, MOST_ALLOWED)
        });
        let circuit = match (system, current, length, allowed) {
            (Some(system), Some(current), Some(length), Some(allowed)) => Some(Circuit {
                current_tenths_amp: current,
                length_micrometres: length * per_unit,
                system_millivolts: system,
                allowed,
            }),
            _ => None,
        };
        (form, circuit)
    }

    /// Whether the system is 24 V, as it was typed.
    pub fn twenty_four(&self) -> bool {
        self.system.value.trim() == "24"
    }

    /// Whether the length was typed in feet.
    pub fn feet(&self) -> bool {
        self.unit.value.trim().eq_ignore_ascii_case("ft")
    }

    /// What to correct, as the place on the page and the message, in the
    /// order of the form.
    pub fn troubles(&self) -> Vec<(String, String)> {
        [&self.system, &self.current, &self.length, &self.allowed]
            .into_iter()
            .filter_map(Field::trouble)
            .collect()
    }
}

/// What the form says when the allowed drop is left empty.
pub fn defaults() -> String {
    format!(
        "Left empty, the voltage drop allowed is {DEFAULT_ALLOWED}. That is an example to start from, not a standard: use the figure the equipment's maker gives."
    )
}

/// What the page shows with no query string: 20 A through 5 m at 12 V,
/// with 3% allowed.
pub fn example() -> Sent {
    Sent::of(&[
        ("system", "12"),
        ("current", "20"),
        ("length", "5"),
        ("unit", "m"),
        ("allowed", "3"),
    ])
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_example_and_a_form_in_feet_are_read() {
        assert_eq!(
            example().query(),
            "system=12&current=20&length=5&unit=m&allowed=3"
        );
        let (form, circuit) = Form::read(&example());
        assert!(form.troubles().is_empty());
        assert_eq!(
            circuit.unwrap(),
            Circuit {
                current_tenths_amp: 200,
                length_micrometres: 5_000_000,
                system_millivolts: 12_000,
                allowed: Percent::whole(3)
            }
        );
        let (form, circuit) = Form::read(&Sent::of(&[
            ("system", "24"),
            ("current", "15.5"),
            ("length", "10"),
            ("unit", "FT"),
            ("allowed", ""),
        ]));
        assert!(form.twenty_four() && form.feet());
        assert_eq!(form.allowed.value, "3");
        assert_eq!(
            circuit.unwrap(),
            Circuit {
                current_tenths_amp: 155,
                length_micrometres: 3_048_000,
                system_millivolts: 24_000,
                allowed: Percent::whole(3)
            }
        );
        assert!(
            Form::was_sent(&Sent::of(&[("current", "")]))
                && !Form::was_sent(&Sent::of(&[("utm_source", "x")]))
        );
    }

    #[test]
    fn what_cannot_be_read_is_reported_beside_its_field() {
        let (form, circuit) = Form::read(&Sent::of(&[
            ("system", "48"),
            ("current", "0"),
            ("length", "ten"),
            ("allowed", "25"),
        ]));
        assert!(circuit.is_none());
        let troubles: Vec<String> = form
            .troubles()
            .into_iter()
            .map(|(place, message)| format!("{place} | {message}"))
            .collect();
        assert_eq!(
            troubles,
            [
                "system | System voltage: Choose 12 V or 24 V.",
                "current | Current, amperes: This must be from 0.1 to 500.",
                "length | Length of wire the current flows through: This is not a number. Write it like 1,234.50.",
                "allowed | Voltage drop allowed, percent: This must be from 0.1% to 20%.",
            ]
        );
        assert_eq!(
            defaults(),
            "Left empty, the voltage drop allowed is 3%. That is an example to start from, not a standard: use the figure the equipment's maker gives."
        );
    }
}
