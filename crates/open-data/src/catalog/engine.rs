//! Engine labels for the catalog, from the same facts a decode has.

use std::collections::{BTreeMap, BTreeSet};

use wenmar_vin::Engine;

/// The vPIC elements that describe an engine: displacement in litres, cubic
/// centimetres and cubic inches, cylinders, fuel, configuration, turbo and
/// electrification level.
pub const ENGINE_ELEMENTS: [i64; 8] = [13, 11, 12, 9, 24, 64, 135, 126];

/// vPIC's element id for the engine model, whose name looks up more facts.
pub const ENGINE_MODEL_ELEMENT: i64 = 18;

const CC_PER_CUBIC_INCH: f64 = 16.387_064;
/// No road vehicle in vPIC comes close; anything larger is bad data.
const LARGEST_LITRES: f64 = 100.0;

/// What is known about one engine, as vPIC's text.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
pub struct Facts<'a> {
    pub litres: Option<&'a str>,
    pub cc: Option<&'a str>,
    pub cubic_inches: Option<&'a str>,
    pub cylinders: Option<&'a str>,
    pub fuel: Option<&'a str>,
    pub configuration: Option<&'a str>,
    pub turbo: Option<&'a str>,
    pub electrification: Option<&'a str>,
}

impl<'a> Facts<'a> {
    /// Records the value of an engine element unless one is already known.
    /// Other elements are ignored.
    pub fn fill(&mut self, element: i64, value: &'a str) {
        let slot = match element {
            13 => &mut self.litres,
            11 => &mut self.cc,
            12 => &mut self.cubic_inches,
            9 => &mut self.cylinders,
            24 => &mut self.fuel,
            64 => &mut self.configuration,
            135 => &mut self.turbo,
            126 => &mut self.electrification,
            _ => return,
        };
        if slot.is_none() {
            *slot = Some(value);
        }
    }
}

fn amount(text: Option<&str>) -> Option<f64> {
    text.and_then(|text| text.trim().parse::<f64>().ok())
        .filter(|amount| amount.is_finite() && *amount > 0.0)
}

/// The short label a decode of this engine would carry, such as
/// `3.5L Turbo V6`. `None` when there is nothing to name it by.
pub fn label(facts: &Facts<'_>) -> Option<String> {
    let stated_litres = amount(facts.litres);
    let stated_cc = amount(facts.cc);
    let from_cubic_inches = amount(facts.cubic_inches).map(|ci| ci * CC_PER_CUBIC_INCH);

    let litres = stated_litres
        .or(stated_cc.map(|cc| cc / 1000.0))
        .or(from_cubic_inches.map(|cc| cc / 1000.0))
        .filter(|litres| *litres <= LARGEST_LITRES)
        .map(|litres| (litres * 10.0).round() / 10.0)
        .filter(|litres| *litres >= 0.1);
    let cc = stated_cc
        .or(from_cubic_inches)
        .or(stated_litres.map(|litres| litres * 1000.0))
        .filter(|cc| *cc <= LARGEST_LITRES * 1000.0)
        .map(f64::round)
        .filter(|cc| *cc >= 1.0)
        .map(|cc| cc as u32);

    // `Engine` cannot be built with a struct expression outside its crate,
    // so start from the default and assign.
    let mut engine = Engine::default();
    engine.displacement_l = litres;
    engine.displacement_cc = cc;
    engine.cylinders = facts.cylinders.and_then(|text| text.trim().parse().ok());
    engine.configuration = facts.configuration.map(|text| text.trim().to_owned());
    engine.fuel = facts.fuel.map(|text| text.trim().to_owned());
    engine.turbo = facts
        .turbo
        .map(|text| text.trim().eq_ignore_ascii_case("yes"));
    engine.electrification = facts.electrification.map(|text| text.trim().to_owned());
    wenmar_vin::engine::label(&engine)
}

/// A label without its cylinder layout: `5.3L V8` gives `5.3L`.
fn without_layout(label: &str) -> String {
    label
        .split(' ')
        .filter(|word| {
            let digits = word.strip_prefix('V').unwrap_or("");
            digits.is_empty() || !digits.bytes().all(|byte| byte.is_ascii_digit())
        })
        .collect::<Vec<_>>()
        .join(" ")
}

/// Labels to merge into a fuller one. vPIC gives some engines a cylinder
/// layout and leaves it off others of the same size, so a model would list
/// both `5.3L` and `5.3L V8`. A label folds into another when the other is
/// the only one that is the same but for a layout.
pub fn folds<'a>(labels: &BTreeSet<&'a str>) -> BTreeMap<&'a str, &'a str> {
    let mut folds = BTreeMap::new();
    for bare in labels {
        let mut fuller = labels
            .iter()
            .filter(|other| *other != bare && without_layout(other) == **bare);
        if let (Some(only), None) = (fuller.next(), fuller.next()) {
            folds.insert(*bare, *only);
        }
    }
    folds
}

#[cfg(test)]
mod tests {
    use super::*;

    fn facts<'a>(pairs: &[(i64, &'a str)]) -> Facts<'a> {
        let mut facts = Facts::default();
        for (element, value) in pairs {
            facts.fill(*element, value);
        }
        facts
    }

    #[test]
    fn labels_match_what_a_decode_shows() {
        for (pairs, expected) in [
            (
                vec![(13, "3.5"), (9, "6"), (64, "V-Shaped"), (135, "Yes")],
                Some("3.5L Turbo V6"),
            ),
            (
                vec![(13, "5"), (9, "8"), (64, "V-Shaped"), (24, "Gasoline")],
                Some("5.0L V8"),
            ),
            (
                vec![(13, "3.00"), (9, "6"), (64, "V-Shaped"), (24, "Diesel")],
                Some("3.0L V6 Diesel"),
            ),
            (
                vec![(12, "350"), (9, "8"), (64, "V-Shaped")],
                Some("5.7L V8"),
            ),
            (vec![(11, "1998"), (135, "No")], Some("2.0L")),
            (vec![(11, "689")], Some("689cc")),
            (
                vec![(13, "2.5"), (126, "Strong HEV (Hybrid Electric Vehicle)")],
                Some("2.5L Hybrid"),
            ),
            (vec![(24, "Electric")], Some("Electric")),
            (
                vec![(126, "BEV (Battery Electric Vehicle)")],
                Some("Electric"),
            ),
            (vec![(9, "4"), (24, "Gasoline")], None),
            (vec![], None),
        ] {
            assert_eq!(label(&facts(&pairs)).as_deref(), expected, "{pairs:?}");
        }
    }

    #[test]
    fn a_size_that_is_not_a_sensible_number_is_ignored() {
        for size in ["0", "-2", "abc", "", "NaN", "inf", "1e309", "250"] {
            assert_eq!(label(&facts(&[(13, size)])), None, "{size:?}");
        }
        // A bad litre value does not hide a good one in another unit.
        assert_eq!(
            label(&facts(&[(13, "abc"), (11, "1998")])).as_deref(),
            Some("2.0L")
        );
    }

    #[test]
    fn the_first_value_of_an_element_is_kept() {
        let facts = facts(&[(9, "6"), (9, "8"), (999, "ignored"), (18, "ignored")]);
        assert_eq!(facts.cylinders, Some("6"));
        assert_eq!(facts.litres, None);
    }

    #[test]
    fn a_label_without_a_layout_folds_into_the_one_that_has_it() {
        let labels = BTreeSet::from(["4.3L V6", "5.3L", "5.3L V8", "6.2L"]);
        assert_eq!(folds(&labels), BTreeMap::from([("5.3L", "5.3L V8")]));

        let diesel = BTreeSet::from(["3.0L Diesel", "3.0L V6 Diesel", "3.0L V6"]);
        assert_eq!(
            folds(&diesel),
            BTreeMap::from([("3.0L Diesel", "3.0L V6 Diesel")])
        );
    }

    #[test]
    fn nothing_folds_when_two_layouts_fit() {
        let labels = BTreeSet::from(["6.0L", "6.0L V8", "6.0L V12"]);
        assert!(folds(&labels).is_empty());
        assert!(folds(&BTreeSet::from(["2.0L", "2.0L Turbo"])).is_empty());
        assert!(folds(&BTreeSet::new()).is_empty());
    }
}
