//! The makes of the catalog, held in memory.
//!
//! There are about twelve thousand makes and a handful of aliases, so they
//! are read once and every question about a make is answered without the
//! database: which make a name or alias means, and the order makes are
//! listed in.

use std::collections::HashMap;

use crate::text::normalize;

/// Which vehicle types a question is about.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Scope {
    /// Passenger cars, multipurpose vehicles and trucks. vPIC's truck type
    /// does not separate light trucks from heavy ones.
    #[default]
    Light,
    /// Every vehicle type, including trailers and motorcycles.
    All,
    /// One vPIC vehicle type, by its id in `catalog_type`.
    Type(u8),
}

impl Scope {
    /// Reads `light`, `all`, or a vehicle type id such as `6`.
    pub fn parse(text: &str) -> Option<Scope> {
        match text.trim().to_ascii_lowercase().as_str() {
            "light" => Some(Scope::Light),
            "all" => Some(Scope::All),
            other => other.parse().ok().map(Scope::Type),
        }
    }

    /// Whether something with these vehicle types is in scope.
    pub fn admits(self, types: u32, light: bool) -> bool {
        match self {
            Scope::Light => light,
            Scope::All => true,
            Scope::Type(id) => 1u32
                .checked_shl(u32::from(id))
                .is_some_and(|bit| types & bit != 0),
        }
    }

    /// The two numbers the SQL filters take: whether only light vehicles
    /// count, and the type bit that must be set (0 for none). `None` when
    /// nothing can match.
    pub(crate) fn bits(self) -> Option<(i64, i64)> {
        match self {
            Scope::Light => Some((1, 0)),
            Scope::All => Some((0, 0)),
            Scope::Type(id) => 1u32
                .checked_shl(u32::from(id))
                .map(|bit| (0, i64::from(bit))),
        }
    }
}

/// The longest matching form that is looked up. No make, model or submodel
/// comes close, and SQLite refuses a `LIKE` pattern of tens of thousands of
/// characters.
pub const MAX_FORM: usize = 40;

/// The matching form of what a person typed, cut to [`MAX_FORM`].
pub fn form(text: &str) -> String {
    let mut form = normalize(text);
    // The matching form is ASCII, so any byte offset is a character boundary.
    form.truncate(MAX_FORM);
    form
}

/// The matching form of a prefix a person typed. An empty term filters
/// nothing (`Some("")`). A term with no letter or digit matches nothing
/// (`None`): it must not turn into "everything".
pub fn term(term: &str) -> Option<String> {
    if term.trim().is_empty() {
        return Some(String::new());
    }
    Some(form(term)).filter(|form| !form.is_empty())
}

/// A make as the catalog stores it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MakeRef {
    pub id: i64,
    pub slug: String,
    pub name: String,
    pub norm: String,
    /// Position in the curated list of popular makes, starting at 1.
    pub rank: Option<u32>,
    /// One bit per vPIC vehicle type id.
    pub types: u32,
    pub light: bool,
}

/// Every make, in listing order: ranked makes by rank, then the rest by name.
#[derive(Debug, Clone, Default)]
pub struct MakeIndex {
    makes: Vec<MakeRef>,
    by_id: HashMap<i64, usize>,
    by_slug: HashMap<String, usize>,
    /// Lowercase name to the makes called that, in listing order.
    by_name: HashMap<String, Vec<usize>>,
    /// Matching form of a name or alias to the makes it may mean, best first.
    by_form: HashMap<String, Vec<usize>>,
    aliases: HashMap<usize, Vec<String>>,
}

fn listing(make: &MakeRef) -> (bool, u32, String, i64) {
    (
        make.rank.is_none(),
        make.rank.unwrap_or(u32::MAX),
        make.name.to_lowercase(),
        make.id,
    )
}

impl MakeIndex {
    /// `aliases` pairs a matching form with a make id.
    pub fn new(mut makes: Vec<MakeRef>, aliases: &[(String, i64)]) -> MakeIndex {
        makes.sort_by_key(listing);
        let mut index = MakeIndex::default();
        for (position, make) in makes.iter().enumerate() {
            index.by_id.insert(make.id, position);
            index.by_slug.insert(make.slug.clone(), position);
            index
                .by_name
                .entry(make.name.to_lowercase())
                .or_default()
                .push(position);
            index
                .by_form
                .entry(make.norm.clone())
                .or_default()
                .push(position);
        }
        for (form, id) in aliases {
            if let Some(position) = index.by_id.get(id).copied() {
                index
                    .by_form
                    .entry(form.clone())
                    .or_default()
                    .push(position);
                index
                    .aliases
                    .entry(position)
                    .or_default()
                    .push(form.clone());
            }
        }
        for positions in index.by_form.values_mut() {
            positions.sort_unstable();
            positions.dedup();
        }
        index.makes = makes;
        index
    }

    pub fn get(&self, id: i64) -> Option<&MakeRef> {
        self.by_id
            .get(&id)
            .and_then(|position| self.makes.get(*position))
    }

    /// The make with exactly this id form.
    pub fn by_slug(&self, slug: &str) -> Option<&MakeRef> {
        self.by_slug
            .get(slug)
            .and_then(|position| self.makes.get(*position))
    }

    /// The make a matching form means: a make's name or an alias. When
    /// several fit, the first in listing order.
    pub fn by_form(&self, form: &str, scope: Scope) -> Option<&MakeRef> {
        self.by_form
            .get(form)?
            .iter()
            .filter_map(|position| self.makes.get(*position))
            .find(|make| scope.admits(make.types, make.light))
    }

    /// The make a person means by an id form, a name or an alias, tried in
    /// that order. The id form comes first so that an id the make step
    /// hands out always names the same make in the next step, even when
    /// another make shares its matching form or has it as an alias. `mb`
    /// is Mercedes-Benz by alias, except where a trailer maker with the id
    /// form `mb` is in scope.
    pub fn resolve(&self, text: &str, scope: Scope) -> Option<&MakeRef> {
        self.by_slug(text.trim())
            .filter(|make| scope.admits(make.types, make.light))
            .or_else(|| self.named(text, scope))
    }

    /// The make a name means: the make called exactly that, without regard
    /// to case, and otherwise the make the matching form means. Two makes
    /// can share a matching form (`B & B Trailers` and `B+B Trailers`), and
    /// a make's name can be another's alias. For a name that is known not
    /// to be an id form, such as the make of a decoded VIN.
    pub fn named(&self, text: &str, scope: Scope) -> Option<&MakeRef> {
        self.by_name
            .get(&text.trim().to_lowercase())
            .and_then(|positions| {
                positions
                    .iter()
                    .filter_map(|position| self.makes.get(*position))
                    .find(|make| scope.admits(make.types, make.light))
            })
            .or_else(|| self.by_form(&normalize(text), scope))
    }

    /// Makes in listing order whose name or alias starts with the term.
    pub fn list(&self, scope: Scope, prefix: &str) -> Vec<&MakeRef> {
        let Some(form) = term(prefix) else {
            return Vec::new();
        };
        self.makes
            .iter()
            .enumerate()
            .filter(|(_, make)| scope.admits(make.types, make.light))
            .filter(|(position, make)| {
                make.norm.starts_with(&form)
                    || self
                        .aliases
                        .get(position)
                        .is_some_and(|aliases| aliases.iter().any(|alias| alias.starts_with(&form)))
            })
            .map(|(_, make)| make)
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make(id: i64, name: &str, rank: Option<u32>, types: u32) -> MakeRef {
        MakeRef {
            id,
            slug: crate::text::slug(name),
            name: name.to_owned(),
            norm: normalize(name),
            rank,
            types,
            light: types & 0b1000_1100 != 0,
        }
    }

    const TRUCK: u32 = 1 << 3;
    const CAR: u32 = 1 << 2;
    const TRAILER: u32 = 1 << 6;

    fn index() -> MakeIndex {
        MakeIndex::new(
            vec![
                make(1, "Checker", None, CAR),
                make(2, "Chevrolet", Some(3), CAR | TRUCK),
                make(3, "Ford", Some(2), CAR | TRUCK),
                make(4, "Ranger Trailers", None, TRAILER),
                make(5, "Mercedes-Benz", Some(14), CAR),
                make(6, "MB", None, TRAILER),
                make(7, "alfa romeo", None, CAR),
            ],
            &[
                ("chevy".to_owned(), 2),
                ("mb".to_owned(), 5),
                ("merc".to_owned(), 5),
                ("ghost".to_owned(), 99),
            ],
        )
    }

    fn names(makes: Vec<&MakeRef>) -> Vec<&str> {
        makes.into_iter().map(|make| make.name.as_str()).collect()
    }

    #[test]
    fn popular_makes_come_first_then_the_rest_by_name() {
        let index = index();
        assert_eq!(
            names(index.list(Scope::All, "")),
            vec![
                "Ford",
                "Chevrolet",
                "Mercedes-Benz",
                "alfa romeo",
                "Checker",
                "MB",
                "Ranger Trailers"
            ]
        );
        assert_eq!(
            names(index.list(Scope::Light, "")),
            vec![
                "Ford",
                "Chevrolet",
                "Mercedes-Benz",
                "alfa romeo",
                "Checker"
            ]
        );
        assert_eq!(
            names(index.list(Scope::Type(6), "")),
            vec!["MB", "Ranger Trailers"]
        );
        assert!(index.list(Scope::Type(200), "").is_empty());
    }

    #[test]
    fn a_term_is_a_prefix_of_a_name_or_an_alias() {
        let index = index();
        assert_eq!(
            names(index.list(Scope::Light, "che")),
            vec!["Chevrolet", "Checker"]
        );
        assert_eq!(names(index.list(Scope::Light, "Chevy")), vec!["Chevrolet"]);
        assert_eq!(
            names(index.list(Scope::Light, "mercedes b")),
            vec!["Mercedes-Benz"]
        );
        assert_eq!(
            names(index.list(Scope::All, "m")),
            vec!["Mercedes-Benz", "MB"]
        );
        assert!(index.list(Scope::Light, "zz").is_empty());
    }

    #[test]
    fn a_term_with_no_letters_or_digits_matches_nothing() {
        let index = index();
        for junk in ["%", "_", "%%%", "--", "'"] {
            assert!(index.list(Scope::All, junk).is_empty(), "{junk}");
        }
        assert_eq!(index.list(Scope::All, "  ").len(), 7);
        assert_eq!(term("%"), None);
        assert_eq!(term(" "), Some(String::new()));
        assert_eq!(term("F-1"), Some("f1".to_owned()));
        assert_eq!(
            term(&"a".repeat(100_000)).map(|form| form.len()),
            Some(MAX_FORM)
        );
    }

    #[test]
    fn names_aliases_and_id_forms_all_find_a_make() {
        let index = index();
        let found =
            |text: &str, scope: Scope| index.resolve(text, scope).map(|make| make.name.as_str());
        assert_eq!(found("chevy", Scope::Light), Some("Chevrolet"));
        assert_eq!(found("CHEVROLET", Scope::Light), Some("Chevrolet"));
        assert_eq!(found("mercedes benz", Scope::Light), Some("Mercedes-Benz"));
        assert_eq!(found("mercedes-benz", Scope::Light), Some("Mercedes-Benz"));
        assert_eq!(found("Merc", Scope::Light), Some("Mercedes-Benz"));
        assert_eq!(
            found("ranger-trailers", Scope::All),
            Some("Ranger Trailers")
        );
        assert_eq!(found("ranger", Scope::All), None);
        assert_eq!(found("", Scope::All), None);
        assert_eq!(found("%", Scope::All), None);
    }

    #[test]
    fn a_shared_name_goes_to_the_make_in_scope_listed_first() {
        let index = index();
        let found = |scope: Scope| index.resolve("mb", scope).map(|make| make.id);
        // `mb` is the trailer maker's id form and its name, so where it is
        // in scope it is the make meant.
        assert_eq!(found(Scope::All), Some(6));
        // Elsewhere it is the alias of a popular make.
        assert_eq!(found(Scope::Light), Some(5));
        assert_eq!(index.by_form("mb", Scope::All).map(|make| make.id), Some(5));
        assert_eq!(found(Scope::Type(6)), Some(6));
        assert_eq!(index.resolve("ranger-trailers", Scope::Light), None);
    }

    #[test]
    fn an_id_form_then_an_exact_name_come_before_a_shared_matching_form() {
        // Listed in this order. The second holds the plain id form.
        let mut first = make(8, "B & B Trailers", None, TRAILER);
        first.slug = "b-b-trailers-2".to_owned();
        let second = make(9, "B+B Trailers", None, TRAILER);
        let index = MakeIndex::new(
            vec![first, second, make(10, "Infiniti", Some(1), CAR)],
            &[("bbtrailers".to_owned(), 10)],
        );
        let found = |text: &str| index.resolve(text, Scope::All).map(|make| make.id);
        assert_eq!(found("b-b-trailers"), Some(9));
        assert_eq!(found("b-b-trailers-2"), Some(8));
        assert_eq!(found(" b-b-trailers "), Some(9));
        assert_eq!(found("B+B Trailers"), Some(9));
        assert_eq!(found("b+b trailers"), Some(9));
        assert_eq!(found("B & B Trailers"), Some(8));
        // Neither an id form nor a name: the make listed first, here by alias.
        assert_eq!(found("BB Trailers"), Some(10));
        // A decoded name is never read as an id form.
        let named = |text: &str| index.named(text, Scope::All).map(|make| make.id);
        assert_eq!(named("B+B Trailers"), Some(9));
        assert_eq!(named("b-b-trailers"), Some(10));
    }

    #[test]
    fn an_alias_for_a_make_that_is_not_there_is_ignored() {
        assert_eq!(index().resolve("ghost", Scope::All), None);
    }

    #[test]
    fn reads_a_scope() {
        assert_eq!(Scope::parse("light"), Some(Scope::Light));
        assert_eq!(Scope::parse(" ALL "), Some(Scope::All));
        assert_eq!(Scope::parse("6"), Some(Scope::Type(6)));
        assert_eq!(Scope::parse("trailer"), None);
        assert_eq!(Scope::parse("999"), None);
        assert_eq!(Scope::Type(40).bits(), None);
        assert_eq!(Scope::Type(6).bits(), Some((0, 64)));
        assert_eq!(Scope::default(), Scope::Light);
    }
}
