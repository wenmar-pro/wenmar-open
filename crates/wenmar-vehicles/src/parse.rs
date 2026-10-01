//! Reads free text such as `2019 civic si` or `chevy 1500` into a year, a
//! make and the words left over. No database is involved.

use crate::index::{MakeIndex, Scope, form};

/// How many words of a search are read. The rest are ignored.
pub const MAX_WORDS: usize = 6;

/// What a search text says.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Parsed {
    /// A four-digit model year the catalog covers.
    pub year: Option<u16>,
    /// The make the text names, by id.
    pub make: Option<i64>,
    /// The words that are neither the year nor the make, in matching form.
    pub words: Vec<String>,
    /// The words that are not the year, including the make's.
    pub all_words: Vec<String>,
}

fn year_of(word: &str, years: (u16, u16)) -> Option<u16> {
    if word.len() != 4 || !word.bytes().all(|byte| byte.is_ascii_digit()) {
        return None;
    }
    word.parse()
        .ok()
        .filter(|year| (years.0..=years.1).contains(year))
}

/// Reads a search text. `years` is the first and last model year of the
/// catalog. With `with_year` false no word is taken as the year.
pub fn parse(
    text: &str,
    index: &MakeIndex,
    scope: Scope,
    years: (u16, u16),
    with_year: bool,
) -> Parsed {
    let mut words: Vec<String> = text
        .split(|character: char| character.is_whitespace() || character == ',')
        .map(form)
        .filter(|word| !word.is_empty())
        .take(MAX_WORDS + 1)
        .collect();

    let mut year = None;
    if with_year
        && let Some(position) = words.iter().position(|word| year_of(word, years).is_some())
    {
        year = year_of(&words.remove(position), years);
    }
    words.truncate(MAX_WORDS);

    // The make is the longest run of up to three words that names one,
    // and the earliest of that length.
    let mut make = None;
    'lengths: for length in (1..=3).rev() {
        for (start, window) in words.windows(length).enumerate() {
            if let Some(found) = index.by_form(&window.concat(), scope) {
                make = Some((found.id, start, length));
                break 'lengths;
            }
        }
    }

    let all_words = words.clone();
    if let Some((_, start, length)) = make {
        words.drain(start..start + length);
    }
    Parsed {
        year,
        make: make.map(|(id, _, _)| id),
        words,
        all_words,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::index::{MAX_FORM, MakeRef};
    use crate::text::normalize;

    fn index() -> MakeIndex {
        let make = |id: i64, name: &str, rank: Option<u32>, light: bool| MakeRef {
            id,
            slug: crate::text::slug(name),
            name: name.to_owned(),
            norm: normalize(name),
            rank,
            types: if light { 1 << 2 } else { 1 << 6 },
            light,
        };
        MakeIndex::new(
            vec![
                make(1, "Honda", Some(4), true),
                make(2, "Chevrolet", Some(3), true),
                make(3, "Land Rover", Some(28), true),
                make(4, "Ram", Some(9), true),
                make(5, "Ranger", None, false),
                make(6, "Land", None, true),
            ],
            &[("chevy".to_owned(), 2)],
        )
    }

    const YEARS: (u16, u16) = (1981, 2027);

    fn read(text: &str) -> (Option<u16>, Option<i64>, Vec<String>) {
        let parsed = parse(text, &index(), Scope::Light, YEARS, true);
        (parsed.year, parsed.make, parsed.words)
    }

    fn words(list: &[&str]) -> Vec<String> {
        list.iter().map(|word| (*word).to_owned()).collect()
    }

    #[test]
    fn finds_the_year_the_make_and_the_rest() {
        assert_eq!(
            read("2019 civic si"),
            (Some(2019), None, words(&["civic", "si"]))
        );
        assert_eq!(read("chevy 1500"), (None, Some(2), words(&["1500"])));
        assert_eq!(
            read("Honda CR-V 2019 EX-L"),
            (Some(2019), Some(1), words(&["crv", "exl"]))
        );
        assert_eq!(read("f150"), (None, None, words(&["f150"])));
        assert_eq!(read("2019, Honda"), (Some(2019), Some(1), words(&[])));
    }

    #[test]
    fn a_number_outside_the_catalogs_years_is_not_a_year() {
        assert_eq!(read("ram 2500"), (None, Some(4), words(&["2500"])));
        assert_eq!(read("chevy 1500"), (None, Some(2), words(&["1500"])));
        assert_eq!(read("1980 civic"), (None, None, words(&["1980", "civic"])));
        assert_eq!(read("2028 civic"), (None, None, words(&["2028", "civic"])));
        assert_eq!(
            read("20190 civic"),
            (None, None, words(&["20190", "civic"]))
        );
    }

    #[test]
    fn only_the_first_year_is_the_year() {
        assert_eq!(read("2019 2500"), (Some(2019), None, words(&["2500"])));
        assert_eq!(read("2500 2019"), (Some(2019), None, words(&["2500"])));
        assert_eq!(read("2019 2000"), (Some(2019), None, words(&["2000"])));
        let parsed = parse("pontiac 2000", &index(), Scope::Light, YEARS, false);
        assert_eq!(
            (parsed.year, parsed.words),
            (None, words(&["pontiac", "2000"]))
        );
    }

    #[test]
    fn the_longest_make_name_wins() {
        assert_eq!(
            read("land rover discovery"),
            (None, Some(3), words(&["discovery"]))
        );
        assert_eq!(read("land cruiser"), (None, Some(6), words(&["cruiser"])));
        assert_eq!(read("2019 landrover"), (Some(2019), Some(3), words(&[])));
    }

    #[test]
    fn a_make_outside_the_scope_is_just_a_word() {
        assert_eq!(read("2019 ranger"), (Some(2019), None, words(&["ranger"])));
        let parsed = parse("2019 ranger", &index(), Scope::All, YEARS, true);
        assert_eq!((parsed.make, parsed.words), (Some(5), words(&[])));
    }

    #[test]
    fn all_words_keep_the_makes_words() {
        let parsed = parse("2019 chevy silverado", &index(), Scope::Light, YEARS, true);
        assert_eq!(parsed.words, words(&["silverado"]));
        assert_eq!(parsed.all_words, words(&["chevy", "silverado"]));
    }

    #[test]
    fn junk_and_very_long_text_are_handled() {
        assert_eq!(read(""), (None, None, words(&[])));
        assert_eq!(read("   "), (None, None, words(&[])));
        assert_eq!(read("% _ '; --"), (None, None, words(&[])));
        assert_eq!(read("🚗 ２０１９"), (None, None, words(&[])));
        let long = "civic ".repeat(5_000);
        assert_eq!(read(&long).2.len(), MAX_WORDS);
        let one_word = "a".repeat(100_000);
        assert_eq!(read(&one_word).2, vec!["a".repeat(MAX_FORM)]);
    }
}
