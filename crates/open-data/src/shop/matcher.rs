//! Matches a source's make and model text to the catalog.

use std::collections::{HashMap, HashSet};
use std::path::Path;

use anyhow::{Context, Result, bail};
use rusqlite::Connection;
use serde::Deserialize;
use wenmar_vehicles::text::{normalize, squeeze};

/// One entry of `shop/models.yaml`.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CuratedModel {
    /// The source's make text.
    pub make: String,
    /// The source's model text.
    pub model: String,
    /// The catalog model names it stands for.
    pub to: Vec<String>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ModelsFile {
    models: Vec<CuratedModel>,
}

/// Everything `shop/models.yaml` says.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct CuratedModels {
    pub models: Vec<CuratedModel>,
}

impl CuratedModels {
    /// Reads `shop/models.yaml` under `directory`.
    pub fn load(directory: &Path) -> Result<CuratedModels> {
        let path = directory.join("shop/models.yaml");
        let text = std::fs::read_to_string(&path)
            .with_context(|| format!("reading {}", path.display()))?;
        CuratedModels::parse(&text)
    }

    /// Reads the text of the file and checks it.
    pub fn parse(yaml: &str) -> Result<CuratedModels> {
        let file: ModelsFile = serde_saphyr::from_str(yaml).context("reading models.yaml")?;
        let mut taken: HashSet<(String, String)> = HashSet::new();
        let mut models = Vec::with_capacity(file.models.len());
        for entry in file.models {
            let (make, model) = (squeeze(&entry.make), squeeze(&entry.model));
            if normalize(&make).is_empty() || normalize(&model).is_empty() {
                bail!("models.yaml: an entry has no make or no model");
            }
            if entry.to.is_empty() {
                bail!("models.yaml: {make} {model} names no catalog model");
            }
            let mut to = Vec::with_capacity(entry.to.len());
            for name in &entry.to {
                let name = squeeze(name);
                if normalize(&name).is_empty() {
                    bail!("models.yaml: {make} {model} has an empty name");
                }
                to.push(name);
            }
            if !taken.insert((normalize(&make), normalize(&model))) {
                bail!("models.yaml lists {make} {model} twice");
            }
            models.push(CuratedModel { make, model, to });
        }
        Ok(CuratedModels { models })
    }
}

/// A model year a source uses to mean "every year".
pub const NO_YEAR: i32 = 9999;

/// The shortest catalog model form the base step may match on. Shorter
/// forms, such as BMW's `m3`, are the start of too many unrelated names.
pub const SHORTEST_BASE: usize = 3;

/// How a row was matched to a catalog model.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MatchKind {
    /// The source named the model.
    Exact,
    /// The source named a series or trim of the model.
    Base,
    /// `shop/models.yaml` says which models the source means.
    Curated,
}

impl MatchKind {
    /// The value of the `match` column.
    pub fn as_str(self) -> &'static str {
        match self {
            MatchKind::Exact => "exact",
            MatchKind::Base => "base",
            MatchKind::Curated => "curated",
        }
    }
}

/// What one `(year, make, model)` of a source is in the catalog.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Resolved {
    /// `catalog_make.id`. `None` when the make is not in the catalog.
    pub make_id: Option<i64>,
    /// `catalog_model.id`, ascending. Empty when the model is unmatched.
    pub model_ids: Vec<i64>,
    /// `None` exactly when `model_ids` is empty.
    pub kind: Option<MatchKind>,
}

impl Resolved {
    /// The value of the `match` column: `None` is stored as NULL.
    pub fn match_text(&self) -> Option<&'static str> {
        self.kind.map(MatchKind::as_str)
    }
}

struct ModelRow {
    id: i64,
    norm: String,
}

/// Everything needed to match names, read once from the core data file.
pub struct Matcher {
    /// Matching form or alias to make id.
    makes: HashMap<String, i64>,
    light_makes: HashSet<i64>,
    /// Make id to its models, light models first, then by id.
    models: HashMap<i64, Vec<ModelRow>>,
    /// Every `(year, model id)` in `catalog_vehicle`.
    vehicles: HashSet<(i32, i64)>,
    /// `(make id, source model form)` to model ids, ascending.
    curated: HashMap<(i64, String), Vec<i64>>,
}

impl Matcher {
    /// Reads the catalog's makes, aliases, models and model years, and
    /// checks every curated entry against them.
    pub fn load(catalog: &Connection, curated: &CuratedModels) -> Result<Matcher> {
        let mut makes: HashMap<String, i64> = HashMap::new();
        let mut light_makes = HashSet::new();
        // Where two makes share a form, the light one, then the lower id.
        let mut statement =
            catalog.prepare("SELECT id, norm, light FROM catalog_make ORDER BY light DESC, id")?;
        let mut rows = statement.query([])?;
        while let Some(row) = rows.next()? {
            let (id, norm, light): (i64, String, i64) = (row.get(0)?, row.get(1)?, row.get(2)?);
            makes.entry(norm).or_insert(id);
            if light != 0 {
                light_makes.insert(id);
            }
        }
        // An alias never takes the place of a make's own name.
        let mut statement =
            catalog.prepare("SELECT norm, make_id FROM catalog_alias ORDER BY norm, make_id")?;
        let mut rows = statement.query([])?;
        while let Some(row) = rows.next()? {
            let (norm, make_id): (String, i64) = (row.get(0)?, row.get(1)?);
            makes.entry(norm).or_insert(make_id);
        }
        let mut models: HashMap<i64, Vec<ModelRow>> = HashMap::new();
        let mut statement = catalog.prepare(
            "SELECT make_id, id, norm FROM catalog_model ORDER BY make_id, light DESC, id",
        )?;
        let mut rows = statement.query([])?;
        while let Some(row) = rows.next()? {
            let make_id: i64 = row.get(0)?;
            models.entry(make_id).or_default().push(ModelRow {
                id: row.get(1)?,
                norm: row.get(2)?,
            });
        }
        let mut vehicles = HashSet::new();
        let mut statement = catalog.prepare("SELECT year, model_id FROM catalog_vehicle")?;
        let mut rows = statement.query([])?;
        while let Some(row) = rows.next()? {
            vehicles.insert((row.get::<_, i32>(0)?, row.get::<_, i64>(1)?));
        }

        let mut by_name: HashMap<(i64, String), Vec<i64>> = HashMap::new();
        for entry in &curated.models {
            let Some(&make_id) = makes.get(&normalize(&entry.make)) else {
                bail!("models.yaml: no catalog make matches {}", entry.make);
            };
            let of_make = models.get(&make_id).map(Vec::as_slice).unwrap_or_default();
            let mut ids = Vec::new();
            for name in &entry.to {
                let form = normalize(name);
                let before = ids.len();
                ids.extend(
                    of_make
                        .iter()
                        .filter(|model| model.norm == form)
                        .map(|model| model.id),
                );
                if ids.len() == before {
                    bail!("models.yaml: {} has no model {name}", entry.make);
                }
            }
            ids.sort_unstable();
            ids.dedup();
            by_name.insert((make_id, normalize(&entry.model)), ids);
        }
        Ok(Matcher {
            makes,
            light_makes,
            models,
            vehicles,
            curated: by_name,
        })
    }

    /// Whether the make is one of the catalog's light makes: cars, MPVs
    /// and trucks.
    pub fn is_light_make(&self, make_id: i64) -> bool {
        self.light_makes.contains(&make_id)
    }

    /// Matches one `(year, make, model)` of a source. `None` and
    /// [`NO_YEAR`] mean the source gave no year, and then a model matches
    /// whatever years it was made in.
    pub fn resolve(&self, year: Option<i32>, make_text: &str, model_text: &str) -> Resolved {
        let unmatched = |make_id: Option<i64>| Resolved {
            make_id,
            model_ids: Vec::new(),
            kind: None,
        };
        let Some(&make_id) = self.makes.get(&normalize(make_text)) else {
            return unmatched(None);
        };
        let wanted = normalize(model_text);
        if wanted.is_empty() {
            return unmatched(Some(make_id));
        }
        let year = year.filter(|year| *year != NO_YEAR);
        let exists =
            |model_id: i64| year.is_none_or(|year| self.vehicles.contains(&(year, model_id)));
        let one = |model_id: i64, kind: MatchKind| Resolved {
            make_id: Some(make_id),
            model_ids: vec![model_id],
            kind: Some(kind),
        };
        let models = self
            .models
            .get(&make_id)
            .map(Vec::as_slice)
            .unwrap_or_default();

        if let Some(model) = models
            .iter()
            .find(|model| model.norm == wanted && exists(model.id))
        {
            return one(model.id, MatchKind::Exact);
        }

        let mut bases: Vec<&ModelRow> = models
            .iter()
            .filter(|model| {
                model.norm.len() >= SHORTEST_BASE
                    && model.norm.len() < wanted.len()
                    && wanted.starts_with(&model.norm)
            })
            .collect();
        // Longest first. The sort is stable, so equal lengths keep the
        // order they were loaded in: light models first, then by id.
        bases.sort_by_key(|model| std::cmp::Reverse(model.norm.len()));
        if let Some(model) = bases.into_iter().find(|model| exists(model.id)) {
            return one(model.id, MatchKind::Base);
        }

        if let Some(ids) = self.curated.get(&(make_id, wanted)) {
            let kept: Vec<i64> = ids.iter().copied().filter(|id| exists(*id)).collect();
            if !kept.is_empty() {
                return Resolved {
                    make_id: Some(make_id),
                    model_ids: kept,
                    kind: Some(MatchKind::Curated),
                };
            }
        }
        unmatched(Some(make_id))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_the_curated_models() {
        let curated = CuratedModels::parse(
            "models:\n  - make: ' BMW '\n    model: '3  SERIES'\n    to: ['320i', ' 328i ']\n",
        )
        .unwrap();
        assert_eq!(
            curated.models,
            [CuratedModel {
                make: "BMW".to_owned(),
                model: "3 SERIES".to_owned(),
                to: vec!["320i".to_owned(), "328i".to_owned()],
            }]
        );
        assert_eq!(
            CuratedModels::parse("models: []\n").unwrap(),
            CuratedModels::default()
        );
    }

    #[test]
    fn a_mistake_in_the_curated_models_is_named() {
        for (yaml, expected) in [
            (
                "models:\n  - make: ''\n    model: X\n    to: [Y]\n",
                "an entry has no make or no model",
            ),
            (
                "models:\n  - make: BMW\n    model: '--'\n    to: [Y]\n",
                "an entry has no make or no model",
            ),
            (
                "models:\n  - make: BMW\n    model: 3 SERIES\n    to: []\n",
                "BMW 3 SERIES names no catalog model",
            ),
            (
                "models:\n  - make: BMW\n    model: 3 SERIES\n    to: ['']\n",
                "BMW 3 SERIES has an empty name",
            ),
            (
                "models:\n  - make: BMW\n    model: 3 SERIES\n    to: [320i]\n  - make: bmw\n    model: 3-series\n    to: [328i]\n",
                "lists bmw 3-series twice",
            ),
            (
                "models:\n  - make: BMW\n    modle: 3 SERIES\n    to: [320i]\n",
                "models.yaml",
            ),
            ("nothing: []\n", "models.yaml"),
        ] {
            let error = CuratedModels::parse(yaml).unwrap_err();
            let message = format!("{error:#}");
            assert!(message.contains(expected), "{message}");
        }
    }

    #[test]
    fn the_file_in_the_repository_loads() {
        let directory = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../data");
        let curated = CuratedModels::load(&directory).unwrap();
        let series = curated
            .models
            .iter()
            .find(|entry| entry.make == "BMW" && entry.model == "3 SERIES")
            .unwrap();
        assert!(series.to.contains(&"328i".to_owned()));
        let error = CuratedModels::load(Path::new("/nonexistent")).unwrap_err();
        assert!(
            format!("{error:#}").contains("shop/models.yaml"),
            "{error:#}"
        );
    }
}
