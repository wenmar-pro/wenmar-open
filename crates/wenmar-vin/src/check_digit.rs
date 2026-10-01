use serde::Serialize;

use crate::vin::Vin;

const WEIGHTS: [u32; 17] = [8, 7, 6, 5, 4, 3, 2, 10, 0, 9, 8, 7, 6, 5, 4, 3, 2];

/// The result of checking position 9.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct CheckDigit {
    pub valid: bool,
    pub expected: char,
    pub actual: char,
}

fn transliterate(byte: u8) -> u32 {
    match byte {
        b'0'..=b'9' => u32::from(byte - b'0'),
        b'A'..=b'H' => u32::from(byte - b'A') + 1,
        b'J'..=b'N' => u32::from(byte - b'J') + 1,
        b'P' => 7,
        b'R' => 9,
        b'S'..=b'Z' => u32::from(byte - b'S') + 2,
        _ => 0,
    }
}

/// The check digit a 17-byte VIN should carry.
pub(crate) fn compute_bytes(bytes: &[u8]) -> char {
    let sum: u32 = bytes
        .iter()
        .zip(WEIGHTS)
        .map(|(byte, weight)| transliterate(*byte) * weight)
        .sum();
    match sum % 11 {
        10 => 'X',
        digit => char::from_digit(digit, 10).unwrap_or('0'),
    }
}

pub fn check(vin: &Vin) -> CheckDigit {
    let expected = compute_bytes(vin.as_str().as_bytes());
    let actual = vin.check_char();
    CheckDigit {
        valid: expected == actual,
        expected,
        actual,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn checked(vin: &str) -> CheckDigit {
        check(&Vin::parse(vin).unwrap())
    }

    #[test]
    fn accepts_valid_vins() {
        for vin in [
            "1HGCM82633A004352",
            "KM8K2CAB4PU001140",
            "11111111111111111",
        ] {
            assert!(checked(vin).valid, "{vin} should be valid");
        }
    }

    #[test]
    fn a_remainder_of_ten_is_written_as_x() {
        assert_eq!(
            checked("1M8GDM9AXKP042788"),
            CheckDigit {
                valid: true,
                expected: 'X',
                actual: 'X'
            }
        );
    }

    #[test]
    fn reports_what_the_digit_should_have_been() {
        assert_eq!(
            checked("1HGCM82633A004353"),
            CheckDigit {
                valid: false,
                expected: '5',
                actual: '3'
            }
        );
    }
}
