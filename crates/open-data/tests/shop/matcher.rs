use open_data::shop::matcher::{CuratedModels, MatchKind, Matcher, Resolved};
use rusqlite::Connection;

use crate::common::{MODELS_YAML, Scratch};

fn matcher(name: &str, yaml: &str) -> (Scratch, anyhow::Result<Matcher>) {
    let scratch = Scratch::new(name);
    let connection = Connection::open(scratch.core()).unwrap();
    let matcher = Matcher::load(&connection, &CuratedModels::parse(yaml).unwrap());
    (scratch, matcher)
}

fn found(make_id: i64, model_ids: &[i64], kind: MatchKind) -> Resolved {
    Resolved {
        make_id: Some(make_id),
        model_ids: model_ids.to_vec(),
        kind: Some(kind),
    }
}

fn unmatched(make_id: Option<i64>) -> Resolved {
    Resolved {
        make_id,
        model_ids: Vec::new(),
        kind: None,
    }
}

#[test]
fn resolves_every_case_in_the_plan() {
    use MatchKind::{Base, Curated, Exact};
    let (_scratch, matcher) = matcher("cases", MODELS_YAML);
    let matcher = matcher.unwrap();
    let cases: Vec<(Option<i32>, &str, &str, Resolved)> = vec![
        (Some(2018), "FORD", "F-150", found(460, &[1801], Exact)),
        (Some(2018), "Ford", "f150", found(460, &[1801], Exact)),
        (
            Some(2019),
            "CHEVROLET",
            "SILVERADO 1500",
            found(467, &[1850], Base),
        ),
        (Some(2020), "CHEVY", "TAHOE", found(467, &[1852], Exact)),
        (
            Some(2015),
            "GMC",
            "SIERRA DENALI",
            found(472, &[1857], Base),
        ),
        (Some(1990), "HONDA", "CIVIC SI", found(474, &[29417], Exact)),
        (Some(2018), "HONDA", "CIVIC SI", found(474, &[1863], Base)),
        (
            Some(2020),
            "CHEVROLET",
            "SILVERADO LD",
            found(467, &[1850], Base),
        ),
        (
            Some(2019),
            "CHEVROLET",
            "SILVERADO LD",
            found(467, &[25279], Exact),
        ),
        (
            Some(2025),
            "MAZDA",
            "CX-50 HYBRID",
            found(473, &[30046], Base),
        ),
        (Some(2005), "DODGE", "RAM 1500", found(476, &[1938], Base)),
        (Some(2019), "RAM", "1500", found(496, &[13620], Exact)),
        (
            Some(2013),
            "TOYOTA",
            "PRIUS PLUG-IN",
            found(448, &[2209], Base),
        ),
        (
            Some(2015),
            "BMW",
            "3 SERIES",
            found(452, &[1709, 1711, 2172, 2173], Curated),
        ),
        (Some(2019), "BMW", "3 SERIES", found(452, &[2777], Curated)),
        (Some(2022), "BMW", "3 SERIES", unmatched(Some(452))),
        (
            None,
            "BMW",
            "3 SERIES",
            found(452, &[1709, 1711, 2172, 2173, 2777], Curated),
        ),
        (Some(2016), "BMW", "M3 CONVERTIBLE", unmatched(Some(452))),
        (
            Some(2016),
            "MERCEDES BENZ",
            "C300 4MATIC",
            found(449, &[2085], Curated),
        ),
        (Some(2016), "MERCEDES-BENZ", "GLK350", unmatched(Some(449))),
        (Some(2022), "FORD", "FLEX", unmatched(Some(460))),
        (None, "HONDA", "ACCORD", found(474, &[1861], Exact)),
        (Some(9999), "HONDA", "ACCORD", found(474, &[1861], Exact)),
        (
            Some(2010),
            "OSHKOSH",
            "H SERIES",
            found(517, &[2191], Exact),
        ),
        (
            Some(2015),
            "OSHKOSH",
            "H SERIES",
            found(517, &[21696], Exact),
        ),
        (None, "OSHKOSH", "H SERIES", found(517, &[2191], Exact)),
        (Some(2012), "SCION", "TC", unmatched(None)),
        (Some(2019), "NEWMAR", "DUTCH STAR", unmatched(None)),
        (Some(2019), "HONDA", "", unmatched(Some(474))),
    ];
    for (year, make, model, expected) in cases {
        assert_eq!(
            matcher.resolve(year, make, model),
            expected,
            "{year:?} {make} {model}"
        );
    }
}

#[test]
fn a_match_says_how_it_matched_in_the_words_the_tables_store() {
    let (_scratch, matcher) = matcher("words", MODELS_YAML);
    let matcher = matcher.unwrap();
    let text = |make: &str, model: &str| matcher.resolve(Some(2019), make, model).match_text();
    assert_eq!(text("FORD", "FLEX"), Some("exact"));
    assert_eq!(text("CHEVROLET", "SILVERADO 1500"), Some("base"));
    assert_eq!(text("BMW", "3 SERIES"), Some("curated"));
    assert_eq!(text("BMW", "ISETTA"), None);
    assert_eq!(MatchKind::Exact.as_str(), "exact");
}

#[test]
fn knows_which_makes_are_light() {
    let (_scratch, matcher) = matcher("light", MODELS_YAML);
    let matcher = matcher.unwrap();
    assert!(matcher.is_light_make(474));
    assert!(!matcher.is_light_make(12503), "THOR is not a light make");
    assert!(!matcher.is_light_make(1));
}

#[test]
fn a_curated_entry_the_catalog_cannot_honour_stops_the_load() {
    for (name, yaml, expected) in [
        (
            "no-make",
            "models:\n  - make: Scion\n    model: tC\n    to: [SCION tC]\n",
            "models.yaml: no catalog make matches Scion",
        ),
        (
            "no-model",
            "models:\n  - make: BMW\n    model: 3 SERIES\n    to: [320i, 329i]\n",
            "models.yaml: BMW has no model 329i",
        ),
    ] {
        let (_scratch, matcher) = matcher(name, yaml);
        let error = matcher.err().unwrap();
        assert_eq!(format!("{error:#}"), expected);
    }
}

#[test]
fn a_curated_name_shared_by_two_models_stands_for_both() {
    let (_scratch, matcher) = matcher(
        "shared",
        "models:\n  - make: Oshkosh\n    model: H\n    to: [H Series]\n",
    );
    let matcher = matcher.unwrap();
    assert_eq!(
        matcher.resolve(None, "OSHKOSH", "H"),
        found(517, &[2191, 21696], MatchKind::Curated)
    );
    assert_eq!(
        matcher.resolve(Some(2015), "OSHKOSH", "H"),
        found(517, &[21696], MatchKind::Curated)
    );
}
