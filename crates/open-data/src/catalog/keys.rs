//! vPIC pattern keys as bit masks over positions 4 to 8 of a VIN.

use std::collections::BTreeMap;

/// The characters a VIN may hold, in the order of the mask bits.
pub const ALPHABET: &str = "0123456789ABCDEFGHJKLMNPRSTUVWXYZ";

/// The mask that accepts every character.
pub const ANY: u64 = (1 << 33) - 1;

fn bit(character: char) -> u64 {
    ALPHABET.find(character).map_or(0, |index| 1 << index)
}

/// What a pattern accepts at each of positions 4 to 8.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Key(pub [u64; 5]);

impl Key {
    /// Reads the part of a pattern's keys before the pipe. `None` when the
    /// keys are malformed or could never match a VIN.
    pub fn parse(keys: &str) -> Option<Key> {
        let (section, has_pipe) = match keys.split_once('|') {
            Some((section, _)) => (section, true),
            None => (keys, false),
        };
        let characters: Vec<char> = section.chars().collect();
        let mut masks = [ANY; 5];
        let mut index = 0;
        let mut position = 0;
        while let Some(first) = characters.get(index) {
            let mask = match first {
                '*' => {
                    index += 1;
                    ANY
                }
                '[' => {
                    let length = characters.get(index..)?.iter().position(|c| *c == ']')?;
                    let mut body = characters.get(index + 1..index + length)?;
                    let negated = body.first() == Some(&'^');
                    if negated {
                        body = body.get(1..)?;
                    }
                    if body.is_empty() {
                        return None;
                    }
                    index += length + 1;
                    let members = set(body);
                    if negated { ANY & !members } else { members }
                }
                literal => {
                    index += 1;
                    bit(*literal)
                }
            };
            if let Some(slot) = masks.get_mut(position) {
                *slot = mask;
            }
            position += 1;
        }
        // The decoder matches the pipe as a literal, so it must be sixth.
        if position == 0 || (has_pipe && position != 5) {
            return None;
        }
        Some(Key(masks))
    }

    /// Whether some VIN could match both keys.
    pub fn compatible(&self, other: &Key) -> bool {
        self.0.iter().zip(other.0.iter()).all(|(a, b)| a & b != 0)
    }
}

/// The mask of a set body such as `1-46` or `ABCE`.
fn set(body: &[char]) -> u64 {
    let mut mask = 0;
    let mut index = 0;
    while let Some(first) = body.get(index) {
        match (body.get(index + 1), body.get(index + 2)) {
            (Some('-'), Some(last)) => {
                for character in ALPHABET.chars() {
                    if (*first..=*last).contains(&character) {
                        mask |= bit(character);
                    }
                }
                index += 3;
            }
            _ => {
                mask |= bit(*first);
                index += 1;
            }
        }
    }
    mask
}

/// Splits the alphabet into groups of characters that the same masks
/// contain, ordered by their first character. With no masks there is one
/// group holding everything.
pub fn atoms(masks: &[u64]) -> Vec<u64> {
    let mut groups: BTreeMap<Vec<bool>, u64> = BTreeMap::new();
    for index in 0..ALPHABET.len() {
        let bit = 1u64 << index;
        let membership: Vec<bool> = masks.iter().map(|mask| mask & bit != 0).collect();
        *groups.entry(membership).or_default() |= bit;
    }
    let mut atoms: Vec<u64> = groups.into_values().collect();
    atoms.sort_by_key(|atom| atom.trailing_zeros());
    atoms
}

/// The characters of a mask, in alphabet order.
pub fn characters(mask: u64) -> String {
    ALPHABET
        .chars()
        .enumerate()
        .filter(|(index, _)| mask & (1 << index) != 0)
        .map(|(_, character)| character)
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn mask(characters: &str) -> u64 {
        characters
            .chars()
            .map(|character| 1u64 << ALPHABET.find(character).unwrap())
            .sum()
    }

    #[test]
    fn reads_literals_wildcards_and_sets() {
        assert_eq!(
            Key::parse("*W1[CE]"),
            Some(Key([ANY, mask("W"), mask("1"), mask("CE"), ANY]))
        );
        assert_eq!(
            Key::parse("FC1E5"),
            Some(Key([mask("F"), mask("C"), mask("1"), mask("E"), mask("5")]))
        );
        assert_eq!(Key::parse("[1-46]").unwrap().0[0], mask("12346"));
        assert_eq!(Key::parse("[^13]").unwrap().0[0], ANY & !mask("13"));
    }

    #[test]
    fn only_positions_4_to_8_are_kept() {
        assert_eq!(Key::parse("*****|*U"), Some(Key([ANY; 5])));
        assert_eq!(Key::parse("FC1E5|K"), Key::parse("FC1E5"));
        // With no pipe, a short key is a prefix and a long one is cut.
        assert_eq!(Key::parse("AB"), Key::parse("AB***"));
        assert_eq!(Key::parse("ABCDEFG"), Key::parse("ABCDE"));
    }

    #[test]
    fn keys_the_decoder_could_never_match_are_refused() {
        // A pipe before the fifth position cannot line up with a VIN.
        for keys in ["", "|U", "AB|C", "ABCDEF|G", "K[2", "K[]C", "K[^]C", "["] {
            assert_eq!(Key::parse(keys), None, "{keys:?}");
        }
    }

    #[test]
    fn a_character_no_vin_holds_matches_nothing() {
        let key = Key::parse("I****").unwrap();
        assert_eq!(key.0[0], 0);
        assert!(!key.compatible(&Key([ANY; 5])));
        assert_eq!(Key::parse("é****").unwrap().0[0], 0);
    }

    #[test]
    fn keys_are_compatible_when_every_position_overlaps() {
        let model = Key::parse("*W1[CE]").unwrap();
        assert!(model.compatible(&Key::parse("****5").unwrap()));
        assert!(model.compatible(&Key::parse("*W1C").unwrap()));
        assert!(!model.compatible(&Key::parse("*W1R").unwrap()));
        assert!(!model.compatible(&Key::parse("*X***").unwrap()));
    }

    #[test]
    fn atoms_group_the_characters_no_mask_tells_apart() {
        assert_eq!(atoms(&[]), vec![ANY]);
        let groups = atoms(&[mask("5"), mask("5G"), mask("CER")]);
        let mut names: Vec<String> = groups.iter().map(|atom| characters(*atom)).collect();
        names.sort();
        assert_eq!(
            names,
            vec![
                "0123456789ABCDEFGHJKLMNPRSTUVWXYZ".replace(['5', 'G', 'C', 'E', 'R'], ""),
                "5".to_owned(),
                "CER".to_owned(),
                "G".to_owned()
            ]
        );
        assert_eq!(groups.iter().fold(0, |all, atom| all | atom), ANY);
    }

    #[test]
    fn characters_are_listed_in_alphabet_order() {
        assert_eq!(characters(mask("G54")), "45G");
        assert_eq!(characters(0), "");
        assert_eq!(characters(ANY), ALPHABET);
    }
}
