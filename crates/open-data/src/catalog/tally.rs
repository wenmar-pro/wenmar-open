//! What the cells of a model have in common, in a form that can be merged.

use std::collections::{BTreeMap, BTreeSet, HashMap};

/// Gives each distinct text a small number, so tallies stay small.
#[derive(Debug, Default)]
pub struct Interner {
    ids: HashMap<String, u32>,
    names: Vec<String>,
}

impl Interner {
    pub fn intern(&mut self, text: &str) -> u32 {
        if let Some(id) = self.ids.get(text) {
            return *id;
        }
        let id = u32::try_from(self.names.len()).unwrap_or(u32::MAX);
        self.ids.insert(text.to_owned(), id);
        self.names.push(text.to_owned());
        id
    }

    /// The text of an id, or an empty string for an id never given out.
    pub fn name(&self, id: u32) -> &str {
        usize::try_from(id)
            .ok()
            .and_then(|index| self.names.get(index))
            .map_or("", String::as_str)
    }
}

/// Whether every cell seen so far had the same value. A cell with no value
/// counts as disagreement, so `One` means "always this".
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Agreed {
    #[default]
    Unseen,
    One(u32),
    Mixed,
}

impl Agreed {
    pub fn see(&mut self, value: Option<u32>) {
        *self = match (*self, value) {
            (Agreed::Unseen, Some(value)) => Agreed::One(value),
            (Agreed::One(current), Some(value)) if current == value => Agreed::One(current),
            _ => Agreed::Mixed,
        };
    }

    pub fn merge(&mut self, other: Agreed) {
        *self = match (*self, other) {
            (Agreed::Unseen, other) => other,
            (current, Agreed::Unseen) => current,
            (Agreed::One(a), Agreed::One(b)) if a == b => Agreed::One(a),
            _ => Agreed::Mixed,
        };
    }

    pub fn value(self) -> Option<u32> {
        match self {
            Agreed::One(value) => Some(value),
            Agreed::Unseen | Agreed::Mixed => None,
        }
    }
}

/// What one cell decodes to. Texts are interned.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Outcome {
    pub trim: Option<u32>,
    pub series: Option<u32>,
    pub body: Option<u32>,
    pub drive: Option<u32>,
    pub transmission: Option<u32>,
    /// The engine's short label.
    pub engine: Option<u32>,
    /// The position-8 characters the cell stands for.
    pub vin8: u64,
}

/// What a group of cells has in common.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Seen {
    pub body: Agreed,
    pub drive: Agreed,
    pub transmission: Agreed,
    pub engines: BTreeSet<u32>,
}

impl Seen {
    fn add(&mut self, outcome: &Outcome) {
        self.body.see(outcome.body);
        self.drive.see(outcome.drive);
        self.transmission.see(outcome.transmission);
        if let Some(engine) = outcome.engine {
            self.engines.insert(engine);
        }
    }

    pub fn merge(&mut self, other: &Seen) {
        self.body.merge(other.body);
        self.drive.merge(other.drive);
        self.transmission.merge(other.transmission);
        self.engines.extend(other.engines.iter().copied());
    }
}

/// Everything the catalog keeps about one model: all its cells together,
/// the cells of each trim and each series, and the position-8 characters
/// each engine was seen with.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Tally {
    pub all: Seen,
    pub trims: BTreeMap<u32, Seen>,
    pub series: BTreeMap<u32, Seen>,
    pub vin8: BTreeMap<u32, u64>,
}

impl Tally {
    pub fn add(&mut self, outcome: &Outcome) {
        self.all.add(outcome);
        if let Some(trim) = outcome.trim {
            self.trims.entry(trim).or_default().add(outcome);
        }
        if let Some(series) = outcome.series {
            self.series.entry(series).or_default().add(outcome);
        }
        if let Some(engine) = outcome.engine {
            *self.vin8.entry(engine).or_default() |= outcome.vin8;
        }
    }

    pub fn merge(&mut self, other: &Tally) {
        self.all.merge(&other.all);
        for (trim, seen) in &other.trims {
            self.trims.entry(*trim).or_default().merge(seen);
        }
        for (series, seen) in &other.series {
            self.series.entry(*series).or_default().merge(seen);
        }
        for (engine, mask) in &other.vin8 {
            *self.vin8.entry(*engine).or_default() |= mask;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn interning_gives_the_same_number_for_the_same_text() {
        let mut names = Interner::default();
        let (sedan, coupe) = (names.intern("Sedan"), names.intern("Coupe"));
        assert_ne!(sedan, coupe);
        assert_eq!(names.intern("Sedan"), sedan);
        assert_eq!(names.name(coupe), "Coupe");
        assert_eq!(names.name(999), "");
    }

    #[test]
    fn a_value_is_agreed_only_while_every_cell_has_it() {
        let mut agreed = Agreed::default();
        assert_eq!(agreed.value(), None);
        agreed.see(Some(7));
        agreed.see(Some(7));
        assert_eq!(agreed.value(), Some(7));
        agreed.see(None);
        assert_eq!(agreed, Agreed::Mixed);
        agreed.see(Some(7));
        assert_eq!(agreed.value(), None);

        let mut other = Agreed::One(7);
        other.see(Some(8));
        assert_eq!(other, Agreed::Mixed);
    }

    #[test]
    fn merging_agreement() {
        for (a, b, expected) in [
            (Agreed::Unseen, Agreed::Unseen, Agreed::Unseen),
            (Agreed::Unseen, Agreed::One(1), Agreed::One(1)),
            (Agreed::One(1), Agreed::Unseen, Agreed::One(1)),
            (Agreed::One(1), Agreed::One(1), Agreed::One(1)),
            (Agreed::One(1), Agreed::One(2), Agreed::Mixed),
            (Agreed::Mixed, Agreed::One(1), Agreed::Mixed),
            (Agreed::Unseen, Agreed::Mixed, Agreed::Mixed),
        ] {
            let mut merged = a;
            merged.merge(b);
            assert_eq!(merged, expected, "{a:?} with {b:?}");
        }
    }

    fn outcome(trim: Option<u32>, body: Option<u32>, engine: Option<u32>, vin8: u64) -> Outcome {
        Outcome {
            trim,
            body,
            engine,
            vin8,
            ..Outcome::default()
        }
    }

    #[test]
    fn a_tally_groups_cells_by_trim_and_collects_engine_characters() {
        let mut tally = Tally::default();
        tally.add(&outcome(Some(1), Some(10), Some(20), 0b01));
        tally.add(&outcome(Some(1), Some(10), Some(21), 0b10));
        tally.add(&outcome(None, Some(11), Some(20), 0b100));
        tally.add(&outcome(None, Some(11), None, 0b1000));

        assert_eq!(tally.all.body, Agreed::Mixed);
        assert_eq!(tally.all.engines, BTreeSet::from([20, 21]));
        assert_eq!(tally.trims[&1].body, Agreed::One(10));
        assert_eq!(tally.trims[&1].engines, BTreeSet::from([20, 21]));
        // Nothing here set a drive type, so none is agreed.
        assert_eq!(tally.trims[&1].drive, Agreed::Mixed);
        // A cell with no engine claims no characters.
        assert_eq!(tally.vin8, BTreeMap::from([(20, 0b101), (21, 0b10)]));
        assert!(tally.series.is_empty());
    }

    #[test]
    fn merging_tallies_is_the_same_as_adding_all_their_cells_to_one() {
        let cells = [
            outcome(Some(1), Some(10), Some(20), 0b01),
            outcome(Some(2), Some(10), Some(21), 0b10),
            outcome(Some(1), None, Some(20), 0b100),
            outcome(None, Some(10), None, 0b1000),
        ];
        let mut whole = Tally::default();
        for cell in &cells {
            whole.add(cell);
        }
        for split in 0..=cells.len() {
            let (mut left, mut right) = (Tally::default(), Tally::default());
            for cell in &cells[..split] {
                left.add(cell);
            }
            for cell in &cells[split..] {
                right.add(cell);
            }
            left.merge(&right);
            assert_eq!(left, whole, "split at {split}");
        }
    }
}
