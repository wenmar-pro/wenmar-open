/// One position of a pattern.
enum Token<'a> {
    Any,
    Literal(char),
    Set { negated: bool, members: &'a str },
}

/// Splits a pattern into one token per position. `None` if it is malformed.
fn tokens(keys: &str) -> Option<Vec<Token<'_>>> {
    let mut parsed = Vec::new();
    let mut rest = keys;
    while let Some(first) = rest.chars().next() {
        match first {
            '*' => {
                parsed.push(Token::Any);
                rest = &rest[1..];
            }
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
                parsed.push(Token::Set { negated, members });
                rest = &rest[end + 1..];
            }
            literal => {
                parsed.push(Token::Literal(literal));
                rest = &rest[literal.len_utf8()..];
            }
        }
    }
    if parsed.is_empty() {
        None
    } else {
        Some(parsed)
    }
}

/// Whether a set such as `1-46` or `ABCE` contains the character.
fn set_contains(members: &str, character: char) -> bool {
    let members: Vec<char> = members.chars().collect();
    let mut index = 0;
    while index < members.len() {
        if index + 2 < members.len() && members[index + 1] == '-' {
            if (members[index]..=members[index + 2]).contains(&character) {
                return true;
            }
            index += 3;
        } else {
            if members[index] == character {
                return true;
            }
            index += 1;
        }
    }
    false
}

/// Whether a vPIC pattern matches a VIN's match key.
pub fn matches(keys: &str, input: &str) -> bool {
    let Some(parsed) = tokens(keys) else {
        return false;
    };
    let mut characters = input.chars();
    for token in &parsed {
        let Some(character) = characters.next() else {
            return false;
        };
        let matched = match token {
            Token::Any => true,
            Token::Literal(expected) => *expected == character,
            Token::Set { negated, members } => set_contains(members, character) != *negated,
        };
        if !matched {
            return false;
        }
    }
    true
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
    fn malformed_patterns_never_match_and_never_panic() {
        for keys in ["", "K[2", "K[]C", "K[^]C", "["] {
            assert!(!matches(keys, KEY), "{keys:?} should not match");
        }
        assert!(!matches("K2***", ""));
        assert!(!matches("Ｋ2***", KEY));
    }
}
