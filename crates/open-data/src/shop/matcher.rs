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
