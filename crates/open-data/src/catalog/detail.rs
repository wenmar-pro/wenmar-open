//! Turns what a model's cells have in common into the rows the catalog
//! stores: submodels, engines, and which go together.

use std::collections::{BTreeMap, BTreeSet};

use wenmar_vehicles::summary::short;
use wenmar_vehicles::text::slug;

use crate::catalog::curated::Preset;
use crate::catalog::engine::folds;
use crate::catalog::keys::{ANY, characters};
use crate::catalog::names::{Speller, split_trims};
use crate::catalog::tally::{Agreed, Interner, Seen, Tally};

/// Where a submodel comes from. The order is the order they are listed in.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Kind {
    Trim,
    Preset,
    Series,
}

impl Kind {
    pub fn as_str(self) -> &'static str {
        match self {
            Kind::Trim => "trim",
            Kind::Preset => "preset",
            Kind::Series => "series",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Submodel {
    pub name: String,
    pub kind: Kind,
    /// Whether the submodel step offers it. A series is offered only when
    /// there is no trim and no preset.
    pub listed: bool,
    pub body: Option<String>,
    pub drive: Option<String>,
    pub transmission: Option<String>,
    /// The engines this submodel comes with, by label. Empty when it does
    /// not narrow the model's list.
    pub engines: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct EngineChoice {
    pub label: String,
    /// The position-8 characters that mean this engine and no other.
    pub vin8: Option<String>,
    pub preset: bool,
}

/// Everything below the model for one or more model years.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Detail {
    pub body: Option<String>,
    pub drive: Option<String>,
    pub transmission: Option<String>,
    pub submodels: Vec<Submodel>,
    pub engines: Vec<EngineChoice>,
}

struct Found {
    name: String,
    kind: Kind,
    seen: Seen,
}

/// Builds the detail of a model year. `preset` is the preset that applies
/// to it, if any. `None` when nothing beyond the model is known.
pub fn assemble(
    tally: &Tally,
    names: &Interner,
    speller: &Speller,
    preset: Option<&Preset>,
) -> Option<Detail> {
    // Engines, after folding labels that differ only by a cylinder layout.
    let labels: BTreeSet<&str> = tally.all.engines.iter().map(|id| names.name(*id)).collect();
    let folds = folds(&labels);
    let folded = |id: u32| -> &str {
        let label = names.name(id);
        folds.get(label).copied().unwrap_or(label)
    };
    let mut masks: BTreeMap<&str, u64> = BTreeMap::new();
    for id in &tally.all.engines {
        masks.entry(folded(*id)).or_default();
    }
    for (id, mask) in &tally.vin8 {
        *masks.entry(folded(*id)).or_default() |= mask;
    }
    let mut engines: Vec<EngineChoice> = masks
        .iter()
        .map(|(label, mask)| {
            let others = masks
                .iter()
                .filter(|(other, _)| other != &label)
                .fold(0, |all, (_, mask)| all | mask);
            let own = mask & !others;
            EngineChoice {
                label: (*label).to_owned(),
                // Every character means position 8 says nothing here.
                vin8: (own != 0 && own != ANY).then(|| characters(own)),
                preset: false,
            }
        })
        .collect();
    let all_engines: BTreeSet<&str> = masks.keys().copied().collect();

    // Submodels, keyed by id form so spellings that differ only in
    // punctuation become one. Trims first: a series never replaces a trim.
    let mut found: BTreeMap<String, Found> = BTreeMap::new();
    let mut add = |name: String, kind: Kind, seen: &Seen| {
        let key = slug(&name);
        if key.is_empty() {
            return;
        }
        match found.get_mut(&key) {
            Some(existing) if existing.kind == kind => {
                existing.name = speller.better(&existing.name, &name).to_owned();
                existing.seen.merge(seen);
            }
            Some(_) => {}
            None => {
                found.insert(
                    key,
                    Found {
                        name,
                        kind,
                        seen: seen.clone(),
                    },
                );
            }
        }
    };
    for (id, seen) in &tally.trims {
        for part in split_trims(names.name(*id)) {
            if let Some(name) = speller.tidy(&part) {
                add(name, Kind::Trim, seen);
            }
        }
    }
    for (id, seen) in &tally.series {
        if let Some(name) = speller.tidy(names.name(*id)) {
            add(name, Kind::Series, seen);
        }
    }

    if let Some(preset) = preset {
        for name in &preset.submodels {
            let key = slug(name);
            if key.is_empty() {
                continue;
            }
            match found.get_mut(&key) {
                // vPIC has this trim: keep its data, take the hand-written spelling.
                Some(existing) if existing.kind == Kind::Trim => existing.name = name.clone(),
                _ => {
                    found.insert(
                        key,
                        Found {
                            name: name.clone(),
                            kind: Kind::Preset,
                            seen: Seen::default(),
                        },
                    );
                }
            }
        }
        if engines.is_empty() {
            let mut taken = BTreeSet::new();
            for label in &preset.engines {
                if !slug(label).is_empty() && taken.insert(slug(label)) {
                    engines.push(EngineChoice {
                        label: label.clone(),
                        vin8: None,
                        preset: true,
                    });
                }
            }
        }
    }

    let text = |agreed: Agreed| agreed.value().map(|id| names.name(id));
    let series_listed = found.values().all(|entry| entry.kind == Kind::Series);
    let mut submodels: Vec<Submodel> = found
        .into_values()
        .map(|entry| {
            let own: BTreeSet<&str> = entry.seen.engines.iter().map(|id| folded(*id)).collect();
            Submodel {
                listed: entry.kind != Kind::Series || series_listed,
                body: text(entry.seen.body).map(short),
                drive: text(entry.seen.drive).map(str::to_owned),
                transmission: text(entry.seen.transmission).map(short),
                engines: if own.is_empty() || own == all_engines {
                    Vec::new()
                } else {
                    own.into_iter().map(str::to_owned).collect()
                },
                name: entry.name,
                kind: entry.kind,
            }
        })
        .collect();
    submodels.sort_by(|a, b| {
        (a.kind, a.name.to_lowercase(), &a.name).cmp(&(b.kind, b.name.to_lowercase(), &b.name))
    });

    let detail = Detail {
        body: text(tally.all.body).map(short),
        drive: text(tally.all.drive).map(str::to_owned),
        transmission: text(tally.all.transmission).map(short),
        submodels,
        engines,
    };
    let empty = detail.submodels.is_empty()
        && detail.engines.is_empty()
        && detail.body.is_none()
        && detail.drive.is_none()
        && detail.transmission.is_none();
    (!empty).then_some(detail)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::catalog::tally::Outcome;

    /// Builds tallies from readable cells.
    #[derive(Default)]
    struct Cells {
        names: Interner,
        tally: Tally,
    }

    impl Cells {
        #[allow(clippy::too_many_arguments)]
        fn cell(
            mut self,
            trim: Option<&str>,
            series: Option<&str>,
            body: Option<&str>,
            drive: Option<&str>,
            transmission: Option<&str>,
            engine: Option<&str>,
            vin8: &str,
        ) -> Self {
            let mut id = |text: Option<&str>| text.map(|text| self.names.intern(text));
            let outcome = Outcome {
                trim: id(trim),
                series: id(series),
                body: id(body),
                drive: id(drive),
                transmission: id(transmission),
                engine: id(engine),
                vin8: vin8
                    .chars()
                    .map(|character| {
                        1u64 << crate::catalog::keys::ALPHABET.find(character).unwrap()
                    })
                    .sum(),
            };
            self.tally.add(&outcome);
            self
        }

        fn detail(&self, speller: &Speller, preset: Option<&Preset>) -> Option<Detail> {
            assemble(&self.tally, &self.names, speller, preset)
        }
    }

    const EVERY: &str = crate::catalog::keys::ALPHABET;

    fn speller() -> Speller {
        let counted: Vec<(String, u64)> = [("Touring", 1), ("TOURING", 1), ("LX", 1), ("lx", 1)]
            .into_iter()
            .map(|(name, count)| (name.to_owned(), count))
            .collect();
        Speller::new(&counted, &BTreeMap::new())
    }

    fn preset(submodels: &[&str], engines: &[&str]) -> Preset {
        Preset {
            make: "Ford".to_owned(),
            model: "F-150".to_owned(),
            from: 2012,
            to: None,
            submodels: submodels.iter().map(|name| (*name).to_owned()).collect(),
            engines: engines.iter().map(|name| (*name).to_owned()).collect(),
        }
    }

    fn names(detail: &Detail) -> Vec<String> {
        detail
            .submodels
            .iter()
            .map(|submodel| {
                format!(
                    "{} {}{}",
                    submodel.name,
                    submodel.kind.as_str(),
                    if submodel.listed { "" } else { " unlisted" }
                )
            })
            .collect()
    }

    fn engines(detail: &Detail) -> Vec<String> {
        detail
            .engines
            .iter()
            .map(|engine| {
                format!(
                    "{} @{}",
                    engine.label,
                    engine.vin8.as_deref().unwrap_or("-")
                )
            })
            .collect()
    }

    /// A Civic: the engine is not in position 8, the trim is.
    fn civic() -> Cells {
        let sedan = Some("Sedan/Saloon");
        Cells::default()
            .cell(
                Some("Si"),
                None,
                sedan,
                None,
                Some("Manual/Standard"),
                Some("1.5L Turbo"),
                "5",
            )
            .cell(
                Some("TOURING"),
                None,
                sedan,
                None,
                None,
                Some("1.5L Turbo"),
                "9",
            )
            .cell(
                Some("Touring"),
                None,
                sedan,
                None,
                None,
                Some("1.5L Turbo"),
                "8",
            )
            .cell(
                None,
                None,
                sedan,
                None,
                None,
                Some("1.5L Turbo"),
                "0123467ABC",
            )
            .cell(Some("LX"), None, sedan, None, None, Some("2.0L"), "5")
            .cell(Some("lx"), None, sedan, None, None, Some("2.0L"), "6")
            .cell(Some("EX, EX-L"), None, sedan, None, None, Some("2.0L"), "7")
            .cell(None, None, sedan, None, None, Some("2.0L"), "012348ABC")
    }

    #[test]
    fn trims_are_tidied_split_and_tied_to_their_engines() {
        let detail = civic().detail(&speller(), None).unwrap();
        assert_eq!(
            names(&detail),
            vec!["EX trim", "EX-L trim", "LX trim", "Si trim", "Touring trim"]
        );
        let by_name = |name: &str| {
            detail
                .submodels
                .iter()
                .find(|submodel| submodel.name == name)
                .unwrap()
        };
        assert_eq!(by_name("Si").engines, vec!["1.5L Turbo"]);
        assert_eq!(by_name("Si").transmission.as_deref(), Some("Manual"));
        assert_eq!(by_name("Si").body.as_deref(), Some("Sedan"));
        assert_eq!(by_name("Touring").engines, vec!["1.5L Turbo"]);
        assert_eq!(by_name("LX").engines, vec!["2.0L"]);
        assert_eq!(by_name("EX").engines, vec!["2.0L"]);
        assert_eq!(by_name("EX-L").engines, vec!["2.0L"]);
        assert_eq!(by_name("LX").transmission, None);
        assert_eq!(
            (
                detail.body.as_deref(),
                detail.drive.as_deref(),
                detail.transmission.as_deref()
            ),
            (Some("Sedan"), None, None)
        );
    }

    #[test]
    fn position_8_is_claimed_only_where_it_means_one_engine() {
        // Only 9 is seen with one engine and never the other.
        let detail = civic().detail(&speller(), None).unwrap();
        assert_eq!(engines(&detail), vec!["1.5L Turbo @9", "2.0L @-"]);
    }

    #[test]
    fn a_cell_with_no_engine_does_not_undo_a_claim() {
        let detail = Cells::default()
            .cell(None, None, None, None, None, Some("5.0L V8"), "5")
            .cell(None, None, None, None, None, None, EVERY)
            .detail(&speller(), None)
            .unwrap();
        assert_eq!(engines(&detail), vec!["5.0L V8 @5"]);
    }

    #[test]
    fn one_engine_under_every_character_claims_none() {
        let detail = Cells::default()
            .cell(None, None, None, None, None, Some("Electric"), EVERY)
            .detail(&speller(), None)
            .unwrap();
        assert_eq!(engines(&detail), vec!["Electric @-"]);
    }

    #[test]
    fn labels_that_differ_only_by_a_layout_become_one_engine() {
        let detail = Cells::default()
            .cell(Some("LT"), None, None, None, None, Some("5.3L"), "C")
            .cell(Some("LT"), None, None, None, None, Some("5.3L V8"), "R")
            .cell(Some("LTZ"), None, None, None, None, Some("6.2L"), "J")
            .cell(Some("WT"), None, None, None, None, Some("4.3L V6"), "HP")
            .detail(&speller(), None)
            .unwrap();
        assert_eq!(
            engines(&detail),
            vec!["4.3L V6 @HP", "5.3L V8 @CR", "6.2L @J"]
        );
        let lt = &detail.submodels[0];
        assert_eq!(
            (lt.name.as_str(), &lt.engines),
            ("LT", &vec!["5.3L V8".to_owned()])
        );
    }

    #[test]
    fn a_submodel_that_comes_with_every_engine_narrows_nothing() {
        let detail = Cells::default()
            .cell(
                Some("Raptor"),
                None,
                Some("Pickup"),
                Some("4WD"),
                None,
                Some("5.0L V8"),
                "5",
            )
            .cell(
                Some("Raptor"),
                None,
                Some("Pickup"),
                Some("4WD"),
                None,
                Some("3.5L Turbo V6"),
                "G",
            )
            .cell(
                None,
                None,
                Some("Pickup"),
                Some("4x2"),
                None,
                Some("5.0L V8"),
                "5",
            )
            .detail(&speller(), None)
            .unwrap();
        let raptor = &detail.submodels[0];
        assert!(raptor.engines.is_empty());
        assert_eq!(raptor.drive.as_deref(), Some("4WD"));
        assert_eq!(
            (detail.body.as_deref(), detail.drive.as_deref()),
            (Some("Pickup"), None)
        );
    }

    #[test]
    fn series_are_offered_only_when_there_is_nothing_better() {
        let only_series = Cells::default()
            .cell(None, Some("1500"), None, None, None, None, EVERY)
            .cell(None, Some("2500"), None, None, None, None, EVERY)
            .detail(&speller(), None)
            .unwrap();
        assert_eq!(names(&only_series), vec!["1500 series", "2500 series"]);

        let with_trims = Cells::default()
            .cell(Some("LT"), Some("1500"), None, None, None, None, EVERY)
            .cell(Some("LT"), Some("LT"), None, None, None, None, EVERY)
            .detail(&speller(), None)
            .unwrap();
        // The series spelled like a trim is dropped; the other is kept for
        // search but not offered.
        assert_eq!(names(&with_trims), vec!["LT trim", "1500 series unlisted"]);
    }

    #[test]
    fn presets_add_submodels_and_fill_in_missing_engines() {
        let f150 = Cells::default()
            .cell(Some("RAPTOR"), None, None, None, None, None, EVERY)
            .cell(None, Some("Lariat"), None, None, None, None, EVERY);
        let preset = preset(
            &["XLT", "Raptor", "Lariat", "xlt", "--"],
            &["5.0L V8", "5.0L  V8", "2.7L"],
        );
        let detail = f150.detail(&speller(), Some(&preset)).unwrap();
        assert_eq!(
            names(&detail),
            // The trim keeps its kind and takes the preset's spelling; the
            // series gives way to the preset.
            vec!["Raptor trim", "Lariat preset", "xlt preset"]
        );
        assert_eq!(engines(&detail), vec!["5.0L V8 @-", "2.7L @-"]);
        assert!(detail.engines.iter().all(|engine| engine.preset));
    }

    #[test]
    fn preset_engines_are_not_added_to_engines_vpic_lists() {
        let detail = Cells::default()
            .cell(None, None, None, None, None, Some("5.0L V8"), "5")
            .detail(&speller(), Some(&preset(&[], &["2.7L EcoBoost"])))
            .unwrap();
        assert_eq!(engines(&detail), vec!["5.0L V8 @5"]);
        assert!(!detail.engines[0].preset);
    }

    #[test]
    fn placeholders_and_nothing_at_all() {
        let unknown = Cells::default().cell(
            Some("(not identified)"),
            Some("N/A"),
            None,
            None,
            None,
            None,
            EVERY,
        );
        assert_eq!(unknown.detail(&speller(), None), None);
        assert_eq!(Cells::default().detail(&speller(), None), None);
        // A preset alone is enough to have a detail.
        let detail = Cells::default()
            .detail(&speller(), Some(&preset(&["XLT"], &[])))
            .unwrap();
        assert_eq!(names(&detail), vec!["XLT preset"]);
    }
}
