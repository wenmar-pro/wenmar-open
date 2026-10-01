use crate::check_digit::{check, compute_bytes};
use crate::vin::{Vin, normalize};

/// Pairs of characters that are easily misread for each other.
const LOOK_ALIKES: [(u8, u8); 7] = [
    (b'S', b'5'),
    (b'B', b'8'),
    (b'Z', b'2'),
    (b'G', b'6'),
    (b'D', b'0'),
    (b'U', b'V'),
    (b'L', b'1'),
];

fn look_alike(byte: u8) -> Option<u8> {
    LOOK_ALIKES.iter().find_map(|(left, right)| {
        if *left == byte {
            Some(*right)
        } else if *right == byte {
            Some(*left)
        } else {
            None
        }
    })
}

/// Corrections for input that is not a well-formed VIN.
///
/// Replaces I with 1, and O and Q with 0. Returns the result only if that
/// produces a well-formed VIN different from the input.
pub fn for_malformed(input: &str) -> Vec<String> {
    let normalized = normalize(input);
    let replaced: String = normalized
        .chars()
        .map(|character| match character {
            'I' => '1',
            'O' | 'Q' => '0',
            other => other,
        })
        .collect();
    match Vin::parse(&replaced) {
        Ok(vin) if vin.as_str() != normalized => vec![vin.as_str().to_owned()],
        _ => Vec::new(),
    }
}

/// Corrections for a well-formed VIN whose check digit is wrong: each single
/// look-alike swap that makes the check digit right, in position order.
pub fn for_check_digit(vin: &Vin) -> Vec<String> {
    if check(vin).valid {
        return Vec::new();
    }
    let original = vin.as_str().as_bytes();
    let mut fixes = Vec::new();
    for (index, byte) in original.iter().enumerate() {
        // Position 9 is the check digit itself.
        if index == 8 {
            continue;
        }
        let Some(swap) = look_alike(*byte) else {
            continue;
        };
        let mut candidate = original.to_vec();
        candidate[index] = swap;
        if compute_bytes(&candidate) == char::from(candidate[8])
            && let Ok(text) = String::from_utf8(candidate)
        {
            fixes.push(text);
        }
    }
    fixes
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn replaces_letters_a_vin_never_uses() {
        assert_eq!(
            for_malformed("KM8K2CAB4PU0O1140"),
            vec!["KM8K2CAB4PU001140"]
        );
        assert_eq!(
            for_malformed("km8k2cab4pu00ii40"),
            vec!["KM8K2CAB4PU001140"]
        );
    }

    #[test]
    fn has_nothing_to_offer_for_the_wrong_length() {
        assert_eq!(for_malformed("KM8K2CAB4PU00114"), Vec::<String>::new());
    }

    #[test]
    fn does_not_suggest_a_vin_that_was_already_well_formed() {
        assert_eq!(for_malformed("KM8K2CAB4PU001140"), Vec::<String>::new());
    }

    #[test]
    fn finds_the_look_alike_swap_that_fixes_the_check_digit() {
        let vin = Vin::parse("1HGCM8Z633A004352").unwrap();
        assert_eq!(for_check_digit(&vin), vec!["1HGCM82633A004352"]);
    }

    #[test]
    fn lists_every_swap_that_works_in_position_order() {
        let vin = Vin::parse("KM8K2CA84PU001140").unwrap();
        assert_eq!(
            for_check_digit(&vin),
            vec![
                "KM8KZCA84PU001140",
                "KM8K2CAB4PU001140",
                "KM8K2CA84PUD01140"
            ]
        );
    }

    #[test]
    fn offers_nothing_when_the_check_digit_is_already_right() {
        let vin = Vin::parse("KM8K2CAB4PU001140").unwrap();
        assert_eq!(for_check_digit(&vin), Vec::<String>::new());
    }
}
