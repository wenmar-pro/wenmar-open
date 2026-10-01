//! Works through every combination of positions 4 to 8 a model's patterns
//! can tell apart, and reports what each decodes to.

use std::cmp::Reverse;

use crate::catalog::keys::{ANY, Key, atoms};

/// vPIC elements the catalog reads besides the model: trim, series, body
/// class, drive type, transmission style, displacement (L, cc, cubic
/// inches), cylinders, fuel, engine configuration, turbo, electrification
/// and engine model.
pub const CATALOG_ELEMENTS: [i64; 14] = [38, 34, 5, 15, 37, 13, 11, 12, 9, 24, 64, 135, 126, 18];

/// The most combinations worked through for one model in one set of
/// schemas. On the 2026.09 release one model exceeds it.
pub const CELL_CAP: u64 = 2_000_000;

/// A pattern row of one of the catalog's elements.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Row {
    pub id: i64,
    pub element: i64,
    pub key: Key,
    pub value: String,
    pub changed_on: String,
    /// How many characters of the keys are not `*`.
    fixed: usize,
    /// The keys with brackets removed, which is how NHTSA orders them.
    sortable: String,
}

impl Row {
    /// `None` when the keys do not parse.
    pub fn new(id: i64, element: i64, keys: &str, value: &str, changed_on: &str) -> Option<Row> {
        Some(Row {
            id,
            element,
            key: Key::parse(keys)?,
            value: value.to_owned(),
            changed_on: changed_on.to_owned(),
            fixed: keys.chars().filter(|character| *character != '*').count(),
            sortable: keys.replace(['[', ']'], ""),
        })
    }

    /// The decoder's order: latest schema, latest change, fewest fixed
    /// characters, keys in text order, lowest id.
    fn rank(&self, year_from: u16) -> (u16, &str, Reverse<usize>, Reverse<&str>, Reverse<i64>) {
        (
            year_from,
            self.changed_on.as_str(),
            Reverse(self.fixed),
            Reverse(self.sortable.as_str()),
            Reverse(self.id),
        )
    }
}

/// A model has more combinations than the cap allows.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TooManyCells;

type Candidate<'a> = (u16, &'a Row);
type Atoms = [Vec<u64>; 5];

/// Visits every cell of a model. See the module documentation of
/// `catalog` for the algorithm.
pub fn cells<'a>(
    models: &[Key],
    others: &[Candidate<'a>],
    cap: u64,
    visit: &mut dyn FnMut(&[&'a Row], u64),
) -> Result<u64, TooManyCells> {
    let relevant: Vec<Candidate<'a>> = others
        .iter()
        .copied()
        .filter(|(_, row)| models.iter().any(|model| model.compatible(&row.key)))
        .collect();
    let mut position_atoms: Atoms = Default::default();
    for (position, slot) in position_atoms.iter_mut().enumerate() {
        let mut masks: Vec<u64> = models
            .iter()
            .map(|key| key.0[position])
            .chain(relevant.iter().map(|(_, row)| row.key.0[position]))
            .filter(|mask| *mask != ANY)
            .collect();
        masks.sort_unstable();
        masks.dedup();
        *slot = atoms(&masks);
    }
    let models: Vec<&Key> = models.iter().collect();
    let total = count(&models, &position_atoms, 0, cap);
    if total > cap {
        return Err(TooManyCells);
    }
    let mut winners = Vec::new();
    walk(
        &models,
        &relevant,
        &position_atoms,
        0,
        ANY,
        &mut winners,
        visit,
    );
    Ok(total)
}

/// The model keys that accept an atom at a position. An atom is never split
/// by a mask it was built from, so sharing one character means accepting all.
fn accepting<'k>(models: &[&'k Key], position: usize, atom: u64) -> Vec<&'k Key> {
    models
        .iter()
        .copied()
        .filter(|key| key.0[position] & atom != 0)
        .collect()
}

fn count(models: &[&Key], atoms: &Atoms, position: usize, cap: u64) -> u64 {
    let Some(choices) = atoms.get(position) else {
        return 1;
    };
    let mut total = 0;
    for atom in choices {
        let remaining = accepting(models, position, *atom);
        if remaining.is_empty() {
            continue;
        }
        total += count(&remaining, atoms, position + 1, cap);
        if total > cap {
            break;
        }
    }
    total
}

fn walk<'a>(
    models: &[&Key],
    rows: &[Candidate<'a>],
    atoms: &Atoms,
    position: usize,
    vin8: u64,
    winners: &mut Vec<&'a Row>,
    visit: &mut dyn FnMut(&[&'a Row], u64),
) {
    let Some(choices) = atoms.get(position) else {
        let mut best: Vec<Candidate<'a>> = Vec::new();
        for candidate in rows {
            match best
                .iter_mut()
                .find(|(_, row)| row.element == candidate.1.element)
            {
                Some(current) => {
                    if candidate.1.rank(candidate.0) > current.1.rank(current.0) {
                        *current = *candidate;
                    }
                }
                None => best.push(*candidate),
            }
        }
        best.sort_by_key(|(_, row)| row.element);
        winners.clear();
        winners.extend(best.iter().map(|(_, row)| *row));
        visit(winners.as_slice(), vin8);
        return;
    };
    for atom in choices {
        let remaining = accepting(models, position, *atom);
        if remaining.is_empty() {
            continue;
        }
        let matching: Vec<Candidate<'a>> = rows
            .iter()
            .copied()
            .filter(|(_, row)| row.key.0[position] & atom != 0)
            .collect();
        walk(
            &remaining,
            &matching,
            atoms,
            position + 1,
            *atom,
            winners,
            visit,
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::catalog::keys::{ALPHABET, characters};

    const REST: &str = "012346789ABCDEFHJKLMNPRSTUVWXYZ";

    fn row(id: i64, element: i64, keys: &str, value: &str) -> Row {
        Row::new(id, element, keys, value, "2020-01-01 00:00:00").unwrap()
    }

    fn keys(list: &[&str]) -> Vec<Key> {
        list.iter().map(|keys| Key::parse(keys).unwrap()).collect()
    }

    /// Every cell as `element=value` pairs and then its position-8 characters.
    fn run(models: &[&str], others: &[(u16, &Row)]) -> Vec<String> {
        let mut seen = Vec::new();
        cells(&keys(models), others, CELL_CAP, &mut |winners, vin8| {
            let values: Vec<String> = winners
                .iter()
                .map(|row| format!("{}={}", row.element, row.value))
                .collect();
            seen.push(format!("{} @{}", values.join(" "), characters(vin8)));
        })
        .unwrap();
        seen.sort();
        seen
    }

    fn sorted(mut expected: Vec<String>) -> Vec<String> {
        expected.sort();
        expected
    }

    #[test]
    fn every_distinguishable_combination_is_decoded() {
        // An F-150: the cab and drive are in position 7, the engine in 8.
        let raptor = row(1, 38, "*W1R", "Raptor");
        let two_wheel = row(2, 15, "*W1C", "4x2");
        let four_wheel = row(3, 15, "*W1[ER]", "4WD");
        let v8 = row(4, 13, "****5", "5.0");
        let v6 = row(5, 13, "****G", "3.5");
        let another_model = row(6, 38, "*X2**", "Other");
        let others = [
            (2015, &raptor),
            (2015, &two_wheel),
            (2015, &four_wheel),
            (2015, &v8),
            (2015, &v6),
            (2015, &another_model),
        ];
        assert_eq!(
            run(&["*W1[CE]", "*W1R"], &others),
            sorted(vec![
                "13=5.0 15=4x2 @5".to_owned(),
                "13=3.5 15=4x2 @G".to_owned(),
                format!("15=4x2 @{REST}"),
                "13=5.0 15=4WD @5".to_owned(),
                "13=3.5 15=4WD @G".to_owned(),
                format!("15=4WD @{REST}"),
                "13=5.0 15=4WD 38=Raptor @5".to_owned(),
                "13=3.5 15=4WD 38=Raptor @G".to_owned(),
                format!("15=4WD 38=Raptor @{REST}"),
            ])
        );
    }

    #[test]
    fn a_model_with_nothing_else_is_one_cell() {
        assert_eq!(run(&["RD1**"], &[]), vec![format!(" @{ALPHABET}")]);
    }

    #[test]
    fn overlapping_model_keys_are_not_counted_twice() {
        let trim = row(1, 38, "AB***", "Sport");
        let others = [(2015, &trim)];
        let found = run(&["AB***", "A****"], &others);
        assert_eq!(
            found,
            sorted(vec![
                format!("38=Sport @{ALPHABET}"),
                format!(" @{ALPHABET}")
            ])
        );
    }

    #[test]
    fn a_model_key_no_vin_can_match_has_no_cells() {
        let mut calls = 0;
        let count = cells(&keys(&["I****"]), &[], CELL_CAP, &mut |_, _| calls += 1);
        assert_eq!((count, calls), (Ok(0), 0));
    }

    #[test]
    fn rows_are_ranked_as_the_decoder_ranks_them() {
        let winner = |first: (u16, &Row), second: (u16, &Row)| {
            let both = run(&["AB***"], &[first, second]);
            let reversed = run(&["AB***"], &[second, first]);
            assert_eq!(both, reversed);
            both[0].clone()
        };
        let plain =
            |id, keys, value, changed_on| Row::new(id, 38, keys, value, changed_on).unwrap();

        // The schema that starts later wins.
        let (a, b) = (
            plain(1, "AB***", "Older schema", "2020"),
            plain(2, "AB***", "Newer schema", "2010"),
        );
        assert!(winner((2010, &a), (2012, &b)).starts_with("38=Newer schema"));
        // Then the later change.
        let (a, b) = (
            plain(1, "AB***", "Changed first", "2015"),
            plain(2, "AB***", "Changed last", "2016"),
        );
        assert!(winner((2010, &a), (2010, &b)).starts_with("38=Changed last"));
        // Then the key with fewer fixed characters.
        let (a, b) = (
            plain(1, "AB***", "Two fixed", "2015"),
            plain(2, "A****", "One fixed", "2015"),
        );
        assert!(winner((2010, &a), (2010, &b)).starts_with("38=One fixed"));
        // Then the key that sorts first as text, brackets removed.
        let (a, b) = (
            plain(1, "A[BC]***", "ABC", "2015"),
            plain(2, "A[BD]***", "ABD", "2015"),
        );
        assert!(winner((2010, &a), (2010, &b)).starts_with("38=ABC"));
        // Then the lower id.
        let (a, b) = (
            plain(7, "AB***", "Id 7", "2015"),
            plain(3, "AB***", "Id 3", "2015"),
        );
        assert!(winner((2010, &a), (2010, &b)).starts_with("38=Id 3"));
    }

    #[test]
    fn too_many_combinations_are_refused_before_any_is_visited() {
        let v8 = row(4, 13, "****5", "5.0");
        let v6 = row(5, 13, "****G", "3.5");
        let others = [(2015, &v8), (2015, &v6)];
        let mut calls = 0;
        let outcome = cells(&keys(&["*****"]), &others, 2, &mut |_, _| calls += 1);
        assert_eq!((outcome, calls), (Err(TooManyCells), 0));
        let outcome = cells(&keys(&["*****"]), &others, 3, &mut |_, _| calls += 1);
        assert_eq!((outcome, calls), (Ok(3), 3));
    }

    #[test]
    fn keys_that_do_not_parse_make_no_row() {
        assert!(Row::new(1, 38, "K[2", "Broken", "").is_none());
        assert!(Row::new(1, 38, "", "Empty", "").is_none());
    }
}
