/// One position of a pattern.
enum Token<'a> {
    Any,
    Literal(char),
    Set { negated: bool, members: &'a str },
}

/// Reads the token at the start of `rest` and what follows it. `None` if
/// the pattern is malformed there.
fn next_token(rest: &str) -> Option<(Token<'_>, &str)> {
    let first = rest.chars().next()?;
    match first {
        '*' => Some((Token::Any, &rest[1..])),
        '[' => {
            let end = rest.find(']')?;
            let body = &rest[1..end];
            let (negated, members) = match body.strip_prefix('^') {
                Some(stripped) => (true, stripped),
                None => (false, body),
            };
            if members.is_empty() {
                return None;
            }
            Some((Token::Set { negated, members }, &rest[end + 1..]))
        }
        literal => Some((Token::Literal(literal), &rest[literal.len_utf8()..])),
    }
}

impl Token<'_> {
    fn accepts(&self, character: char) -> bool {
        match self {
            Token::Any => true,
            Token::Literal(expected) => *expected == character,
            Token::Set { negated, members } => set_contains(members, character) != *negated,
        }
    }
}

/// Whether a set such as `1-46` or `ABCE` contains the character.
fn set_contains(members: &str, character: char) -> bool {
    let mut rest = members.chars();
    while let Some(first) = rest.next() {
        let mut ahead = rest.clone();
        if let (Some('-'), Some(last)) = (ahead.next(), ahead.next()) {
            if (first..=last).contains(&character) {
                return true;
            }
            rest = ahead;
        } else if first == character {
            return true;
        }
    }
    false
}

/// Whether a vPIC pattern matches a VIN's match key.
pub fn matches(keys: &str, input: &str) -> bool {
    if keys.is_empty() {
        return false;
    }
    let mut characters = input.chars();
    let mut rest = keys;
    // A malformed pattern never matches, even where its start does.
    let mut matched = true;
    while !rest.is_empty() {
        let Some((token, after)) = next_token(rest) else {
            return false;
        };
        rest = after;
        if matched {
            matched = characters
                .next()
                .is_some_and(|character| token.accepts(character));
        }
    }
    matched
}

/// The characters a match key holds: digits, the letters, and the `|`
/// between positions 8 and 10. Each has one bit of a position's mask.
const ALPHABET: &[u8; 37] = b"0123456789ABCDEFGHIJKLMNOPQRSTUVWXYZ|";

fn bit(character: u8) -> Option<u32> {
    match character {
        b'0'..=b'9' => Some(u32::from(character - b'0')),
        b'A'..=b'Z' => Some(u32::from(character - b'A') + 10),
        b'|' => Some(36),
        _ => None,
    }
}

/// A pattern made ready to be tested against many match keys: for each of
/// its positions, a mask of the characters it accepts there. It answers as
/// [`matches`] does, without reading the pattern again.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Compiled {
    masks: Box<[u64]>,
}

impl Compiled {
    /// `None` if the pattern is malformed, as [`matches`] treats it.
    pub fn new(keys: &str) -> Option<Compiled> {
        let mut masks = Vec::new();
        let mut rest = keys;
        while !rest.is_empty() {
            let (token, after) = next_token(rest)?;
            rest = after;
            let mut mask = 0u64;
            for (index, character) in ALPHABET.iter().enumerate() {
                if token.accepts(char::from(*character)) {
                    mask |= 1 << index;
                }
            }
            masks.push(mask);
        }
        if masks.is_empty() {
            return None;
        }
        Some(Compiled {
            masks: masks.into_boxed_slice(),
        })
    }

    /// Whether the pattern matches a match key. A key holding a character
    /// that no match key can hold is never matched.
    pub fn matches(&self, input: &str) -> bool {
        let input = input.as_bytes();
        if self.masks.len() > input.len() {
            return false;
        }
        self.masks
            .iter()
            .zip(input)
            .all(|(mask, character)| bit(*character).is_some_and(|bit| mask & (1 << bit) != 0))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const KEY: &str = "K2CAB|PU001140";

    #[test]
    fn literals_and_wildcards() {
        assert!(matches("K2***", KEY));
        assert!(matches("*2C*B", KEY));
        assert!(!matches("K3***", KEY));
    }

    #[test]
    fn a_pattern_is_a_prefix() {
        assert!(matches("K2", KEY));
        assert!(matches("K", KEY));
    }

    #[test]
    fn patterns_can_reach_past_the_pipe() {
        assert!(matches("*****|*U", KEY));
        assert!(!matches("*****|*X", KEY));
        assert!(matches("K2CAB|PU001140", KEY));
    }

    #[test]
    fn sets_and_ranges() {
        assert!(matches("K[1-3]C", KEY));
        assert!(!matches("K[4-9]C", KEY));
        assert!(matches("K[123]C", KEY));
        assert!(!matches("K[13]C", KEY));
        assert!(matches("[1-46]", "6XXXX|XXXXXXXX"));
        assert!(matches("[1-46]", "3XXXX|XXXXXXXX"));
        assert!(!matches("[1-46]", "5XXXX|XXXXXXXX"));
    }

    #[test]
    fn negated_sets() {
        assert!(matches("K[^13]C", KEY));
        assert!(!matches("K[^2]C", KEY));
    }

    #[test]
    fn a_pattern_longer_than_the_input_does_not_match() {
        assert!(!matches("K2CAB|PU0011401", KEY));
        assert!(!matches("***************", KEY));
    }

    #[test]
    fn compiled_patterns_answer_as_the_text_does() {
        let keys = [
            "K2***",
            "*2C*B",
            "K3***",
            "K2",
            "K",
            "*****|*U",
            "*****|*X",
            "K2CAB|PU001140",
            "K[1-3]C",
            "K[4-9]C",
            "K[123]C",
            "K[13]C",
            "[1-46]",
            "K[^13]C",
            "K[^2]C",
            "K2CAB|PU0011401",
            "***************",
            "*B[CFJM_______]",
            "8S[GKLMN][X__________]",
        ];
        let inputs = [
            KEY,
            "6XXXX|XXXXXXXX",
            "3XXXX|XXXXXXXX",
            "5XXXX|XXXXXXXX",
            "KBC00|000000",
            "",
        ];
        for keys in keys {
            let compiled = Compiled::new(keys).unwrap();
            for input in inputs {
                assert_eq!(
                    compiled.matches(input),
                    matches(keys, input),
                    "{keys} {input}"
                );
            }
        }
        for keys in ["", "K[2", "K[]C", "K[^]C", "["] {
            assert_eq!(Compiled::new(keys), None, "{keys:?}");
        }
    }

    #[test]
    fn a_malformed_tail_spoils_a_matching_start() {
        assert!(!matches("K2[", KEY));
        assert!(!matches("K[]", KEY));
    }

    #[test]
    fn malformed_patterns_never_match_and_never_panic() {
        for keys in ["", "K[2", "K[]C", "K[^]C", "["] {
            assert!(!matches(keys, KEY), "{keys:?} should not match");
        }
        assert!(!matches("K2***", ""));
        assert!(!matches("Ｋ2***", KEY));
    }
}
