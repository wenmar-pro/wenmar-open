//! The forms of a name used to match what people type and to build ids.

/// The matching form: ASCII letters and digits only, lowercased. `F-150`,
/// `f 150` and `F150` all give `f150`.
pub fn normalize(text: &str) -> String {
    text.chars()
        .filter(char::is_ascii_alphanumeric)
        .map(|character| character.to_ascii_lowercase())
        .collect()
}

/// The id form: lowercase words joined by single hyphens. `Mercedes-Benz`
/// gives `mercedes-benz` and `3.5L Turbo V6` gives `3-5l-turbo-v6`. Empty
/// when the text has no ASCII letter or digit.
pub fn slug(text: &str) -> String {
    let mut slug = String::with_capacity(text.len());
    let mut gap = false;
    for character in text.chars() {
        if character.is_ascii_alphanumeric() {
            if gap && !slug.is_empty() {
                slug.push('-');
            }
            gap = false;
            slug.push(character.to_ascii_lowercase());
        } else {
            gap = true;
        }
    }
    slug
}

/// Whether `text` could have come from [`slug`].
pub fn is_slug(text: &str) -> bool {
    !text.is_empty()
        && !text.starts_with('-')
        && !text.ends_with('-')
        && !text.contains("--")
        && text.chars().all(|character| {
            character.is_ascii_lowercase() || character.is_ascii_digit() || character == '-'
        })
}

/// Trims the text and turns every run of whitespace into one space.
pub fn squeeze(text: &str) -> String {
    text.split_whitespace().collect::<Vec<_>>().join(" ")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_matching_form_keeps_only_letters_and_digits() {
        assert_eq!(normalize("F-150"), "f150");
        assert_eq!(normalize("f 150"), "f150");
        assert_eq!(normalize("CR-V"), "crv");
        assert_eq!(normalize("  Mercedes Benz "), "mercedesbenz");
        assert_eq!(normalize("%_%"), "");
        assert_eq!(normalize("É"), "");
        assert_eq!(normalize(""), "");
    }

    #[test]
    fn the_id_form_joins_words_with_single_hyphens() {
        assert_eq!(slug("Mercedes-Benz"), "mercedes-benz");
        assert_eq!(slug("F-150"), "f-150");
        assert_eq!(slug("3.5L Turbo V6"), "3-5l-turbo-v6");
        assert_eq!(slug("  EX-L  "), "ex-l");
        assert_eq!(slug("Si/Si HPT"), "si-si-hpt");
        assert_eq!(slug("'34"), "34");
        assert_eq!(slug("Citroën C4"), "citro-n-c4");
    }

    #[test]
    fn different_punctuation_gives_the_same_id_form() {
        assert_eq!(slug("B & B Trailers"), "b-b-trailers");
        assert_eq!(slug("B+B Trailers"), "b-b-trailers");
    }

    #[test]
    fn a_name_with_no_letters_or_digits_has_no_id_form() {
        assert_eq!(slug("***"), "");
        assert_eq!(slug(""), "");
        assert_eq!(slug("²"), "");
    }

    #[test]
    fn recognises_its_own_output() {
        for good in ["f-150", "honda", "3-5l-turbo-v6", "34"] {
            assert!(is_slug(good), "{good}");
        }
        for bad in ["", "-f", "f-", "f--150", "F-150", "f_150", "f 150", "é"] {
            assert!(!is_slug(bad), "{bad}");
        }
    }

    #[test]
    fn squeezing_tidies_spaces_only() {
        assert_eq!(squeeze("  Sport   Touring "), "Sport Touring");
        assert_eq!(squeeze("EX-L"), "EX-L");
        assert_eq!(squeeze(" \t "), "");
    }
}
