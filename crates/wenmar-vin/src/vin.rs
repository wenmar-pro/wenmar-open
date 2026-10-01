use std::fmt;

use crate::error::{InvalidChar, VinError};

/// A well-formed VIN: 17 uppercase ASCII characters, none of them I, O or Q.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Vin(String);

/// Uppercases the input and drops whitespace and dashes.
pub fn normalize(input: &str) -> String {
    input
        .chars()
        .filter(|character| !character.is_whitespace() && *character != '-')
        .map(|character| character.to_ascii_uppercase())
        .collect()
}

fn is_vin_char(character: char) -> bool {
    matches!(character, '0'..='9' | 'A'..='Z') && !matches!(character, 'I' | 'O' | 'Q')
}

impl Vin {
    pub fn parse(input: &str) -> Result<Vin, VinError> {
        let normalized = normalize(input);
        let length = normalized.chars().count();
        if length != 17 {
            return Err(VinError::InvalidLength(length));
        }
        let invalid: Vec<InvalidChar> = normalized
            .chars()
            .enumerate()
            .filter(|(_, character)| !is_vin_char(*character))
            .map(|(index, character)| InvalidChar {
                position: index + 1,
                character,
            })
            .collect();
        if !invalid.is_empty() {
            return Err(VinError::InvalidCharacters(invalid));
        }
        Ok(Vin(normalized))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// The character at a 1-based position. Callers pass 1 to 17.
    pub(crate) fn char_at(&self, position: usize) -> char {
        char::from(self.0.as_bytes()[position - 1])
    }

    /// World Manufacturer Identifier, positions 1 to 3.
    pub fn wmi(&self) -> &str {
        &self.0[0..3]
    }

    /// Six-character code used by low-volume manufacturers, whose third
    /// character is 9: positions 1 to 3 followed by positions 12 to 14.
    pub fn extended_wmi(&self) -> Option<String> {
        (self.char_at(3) == '9').then(|| format!("{}{}", &self.0[0..3], &self.0[11..14]))
    }

    /// Vehicle Descriptor Section, positions 4 to 8.
    pub fn vds(&self) -> &str {
        &self.0[3..8]
    }

    /// Vehicle Identifier Section, positions 10 to 17.
    pub fn vis(&self) -> &str {
        &self.0[9..17]
    }

    pub fn check_char(&self) -> char {
        self.char_at(9)
    }

    pub fn year_char(&self) -> char {
        self.char_at(10)
    }

    pub fn plant_char(&self) -> char {
        self.char_at(11)
    }

    /// What vPIC pattern keys are matched against: positions 4 to 8, a pipe,
    /// then positions 10 to 17.
    pub fn match_key(&self) -> String {
        format!("{}|{}", self.vds(), self.vis())
    }
}

impl fmt::Display for Vin {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn exposes_each_section() {
        let vin = Vin::parse("KM8K2CAB4PU001140").unwrap();
        assert_eq!(vin.as_str(), "KM8K2CAB4PU001140");
        assert_eq!(vin.wmi(), "KM8");
        assert_eq!(vin.vds(), "K2CAB");
        assert_eq!(vin.check_char(), '4');
        assert_eq!(vin.year_char(), 'P');
        assert_eq!(vin.plant_char(), 'U');
        assert_eq!(vin.vis(), "PU001140");
        assert_eq!(vin.match_key(), "K2CAB|PU001140");
        assert_eq!(vin.extended_wmi(), None);
    }

    #[test]
    fn low_volume_manufacturers_have_a_six_character_code() {
        let vin = Vin::parse("1Z9AB1C23DE456789").unwrap();
        assert_eq!(vin.extended_wmi().as_deref(), Some("1Z9456"));
    }

    #[test]
    fn normalizes_pasted_input() {
        let vin = Vin::parse(" km8k2-cab4 pu001140\n").unwrap();
        assert_eq!(vin.as_str(), "KM8K2CAB4PU001140");
    }

    #[test]
    fn rejects_the_wrong_length() {
        assert_eq!(
            Vin::parse("KM8K2CAB4PU00114"),
            Err(VinError::InvalidLength(16))
        );
        assert_eq!(Vin::parse(""), Err(VinError::InvalidLength(0)));
    }

    #[test]
    fn rejects_letters_a_vin_never_uses() {
        assert_eq!(
            Vin::parse("KM8K2CAB4PU0O1140"),
            Err(VinError::InvalidCharacters(vec![InvalidChar {
                position: 13,
                character: 'O'
            }]))
        );
    }

    #[test]
    fn rejects_non_ascii_without_panicking() {
        assert_eq!(
            Vin::parse("ＫM8K2CAB4PU001140"),
            Err(VinError::InvalidCharacters(vec![InvalidChar {
                position: 1,
                character: 'Ｋ'
            }]))
        );
        assert_eq!(Vin::parse("🚗"), Err(VinError::InvalidLength(1)));
        let seventeen_cars = "🚗".repeat(17);
        let Err(VinError::InvalidCharacters(invalid)) = Vin::parse(&seventeen_cars) else {
            panic!("expected invalid characters");
        };
        assert_eq!(invalid.len(), 17);
    }
}
