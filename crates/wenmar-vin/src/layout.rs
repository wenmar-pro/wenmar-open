//! How a VIN is laid out.
//!
//! VINs for vehicles sold in the United States and Canada follow 49 CFR 565
//! (and CMVSS 115): position 9 is a check digit and position 10 the model
//! year. ISO 3779, which most other markets follow, makes both optional, and
//! makers use that freedom differently. NHTSA's data covers only vehicles sold
//! in the United States, so it has no patterns for these VINs either.
//!
//! Only layouts that can be told from the VIN alone are recognised here.

use crate::vin::Vin;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum Layout {
    /// Positions 9 and 10 may be a check digit and a model year.
    NorthAmerican,
    /// A European Mercedes-Benz VIN: positions 4 to 9 are the six-digit
    /// model code (221156 is an S-Class of the 221 series) and the VIN
    /// carries neither a check digit nor a model year.
    MercedesModelCode(String),
    /// The Volkswagen group's layout outside North America, with Z as a
    /// filler in positions 4 to 6 and 9: no check digit, but position 10 is
    /// still the model year.
    ZFiller,
    /// Position 9 holds a letter other than X, so it is not a check digit
    /// and the VIN does not follow North American rules.
    NoCheckDigit,
}

impl Layout {
    pub(crate) fn of(vin: &Vin, make: Option<&str>) -> Layout {
        let text = vin.as_str();
        let mercedes = make.is_some_and(|make| make.eq_ignore_ascii_case("Mercedes-Benz"));
        if mercedes && text[3..9].bytes().all(|byte| byte.is_ascii_digit()) {
            return Layout::MercedesModelCode(text[3..9].to_owned());
        }
        let check = vin.check_char();
        if check.is_ascii_digit() || check == 'X' {
            Layout::NorthAmerican
        } else if &text[3..6] == "ZZZ" && check == 'Z' {
            Layout::ZFiller
        } else {
            Layout::NoCheckDigit
        }
    }

    /// Whether position 9 can be a check digit at all.
    pub(crate) fn has_check_digit(&self) -> bool {
        *self == Layout::NorthAmerican
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn layout(vin: &str, make: &str) -> Layout {
        Layout::of(&Vin::parse(vin).unwrap(), Some(make))
    }

    #[test]
    fn a_european_mercedes_carries_its_model_code() {
        assert_eq!(
            layout("WDD2211561A123456", "Mercedes-Benz"),
            Layout::MercedesModelCode("221156".to_owned())
        );
    }

    #[test]
    fn a_north_american_mercedes_does_not() {
        assert_eq!(
            layout("WDDNG71X17A123456", "Mercedes-Benz"),
            Layout::NorthAmerican
        );
    }

    #[test]
    fn digits_alone_do_not_make_another_maker_a_mercedes() {
        assert_eq!(layout("1HGCM82633A004352", "Honda"), Layout::NorthAmerican);
    }

    #[test]
    fn volkswagen_group_fillers_are_recognised() {
        assert_eq!(layout("WVWZZZ1KZ6W123456", "Volkswagen"), Layout::ZFiller);
        assert_eq!(layout("WAUZZZ8PZ7A123456", "Audi"), Layout::ZFiller);
    }

    #[test]
    fn a_letter_in_position_9_is_not_a_check_digit() {
        assert_eq!(layout("VF3ABCDEF12345678", "Peugeot"), Layout::NoCheckDigit);
    }
}
