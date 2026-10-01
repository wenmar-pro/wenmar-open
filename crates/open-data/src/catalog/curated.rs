//! The hand-written files under `data/` that shape the catalog.

use std::collections::{BTreeMap, HashSet};
use std::path::Path;

use anyhow::{Context, Result, bail};
use serde::Deserialize;
use wenmar_vehicles::FIRST_YEAR;
use wenmar_vehicles::text::{normalize, squeeze};

/// A make listed in `makes.yaml`. Its position in the list is its rank.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RankedMake {
    /// How the make is shown.
    pub name: String,
    /// Other things people type for it, in matching form.
    #[serde(default)]
    pub aliases: Vec<String>,
}

/// Trims and engines to add for one model.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Preset {
    pub make: String,
    pub model: String,
    /// First model year the preset applies to.
    pub from: u16,
    /// Last model year, or every later year when left out.
    #[serde(default)]
    pub to: Option<u16>,
    #[serde(default)]
    pub submodels: Vec<String>,
    #[serde(default)]
    pub engines: Vec<String>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct MakesFile {
    makes: Vec<RankedMake>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct NamesFile {
    #[serde(default)]
    submodels: BTreeMap<String, String>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct PresetsFile {
    presets: Vec<Preset>,
}

/// Everything the hand-written files say.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Curated {
    /// Makes to list first, most popular first.
    pub makes: Vec<RankedMake>,
    /// Replacement spellings for submodels, keyed by the lowercase spelling
    /// they replace.
    pub submodel_names: BTreeMap<String, String>,
    pub presets: Vec<Preset>,
}

impl Curated {
    /// Reads `catalog/makes.yaml`, `catalog/names.yaml` and `presets.yaml`
    /// under `directory`.
    pub fn load(directory: &Path) -> Result<Curated> {
        let read = |name: &str| {
            let path = directory.join(name);
            std::fs::read_to_string(&path).with_context(|| format!("reading {}", path.display()))
        };
        Curated::parse(
            &read("catalog/makes.yaml")?,
            &read("catalog/names.yaml")?,
            &read("presets.yaml")?,
        )
    }

    /// Reads the text of the three files and checks it.
    pub fn parse(makes_yaml: &str, names_yaml: &str, presets_yaml: &str) -> Result<Curated> {
        let makes: MakesFile = serde_saphyr::from_str(makes_yaml).context("reading makes.yaml")?;
        let names: NamesFile = serde_saphyr::from_str(names_yaml).context("reading names.yaml")?;
        let presets: PresetsFile =
            serde_saphyr::from_str(presets_yaml).context("reading presets.yaml")?;
        Ok(Curated {
            makes: checked_makes(makes.makes)?,
            submodel_names: checked_names(names.submodels)?,
            presets: checked_presets(presets.presets)?,
        })
    }
}

fn checked_makes(makes: Vec<RankedMake>) -> Result<Vec<RankedMake>> {
    // A matching form may belong to one make only, as its name or an alias.
    let mut taken: HashSet<String> = HashSet::new();
    for make in &makes {
        let form = normalize(&make.name);
        if form.is_empty() {
            bail!("makes.yaml: a make has no letters or digits in its name");
        }
        if !taken.insert(form) {
            bail!("makes.yaml lists {} twice", squeeze(&make.name));
        }
    }
    let mut checked = Vec::with_capacity(makes.len());
    for make in makes {
        let name = squeeze(&make.name);
        let mut aliases = Vec::with_capacity(make.aliases.len());
        for alias in &make.aliases {
            let form = normalize(alias);
            if form.is_empty() {
                bail!("makes.yaml: an alias of {name} has no letters or digits");
            }
            if !taken.insert(form.clone()) {
                bail!("makes.yaml: the alias {form} is used twice");
            }
            aliases.push(form);
        }
        checked.push(RankedMake { name, aliases });
    }
    Ok(checked)
}

fn checked_names(names: BTreeMap<String, String>) -> Result<BTreeMap<String, String>> {
    let mut checked = BTreeMap::new();
    for (from, to) in names {
        let (from, to) = (squeeze(&from).to_lowercase(), squeeze(&to));
        if from.is_empty() || to.is_empty() {
            bail!("names.yaml: every entry needs text on both sides");
        }
        checked.insert(from, to);
    }
    Ok(checked)
}

fn checked_presets(presets: Vec<Preset>) -> Result<Vec<Preset>> {
    let mut checked = Vec::with_capacity(presets.len());
    for preset in presets {
        let (make, model) = (squeeze(&preset.make), squeeze(&preset.model));
        if normalize(&make).is_empty() || normalize(&model).is_empty() {
            bail!("presets.yaml: a preset has no make or no model");
        }
        if preset.from < FIRST_YEAR {
            bail!(
                "presets.yaml: {make} {model} starts in {}, before {FIRST_YEAR}",
                preset.from
            );
        }
        if let Some(to) = preset.to
            && to < preset.from
        {
            bail!("presets.yaml: {make} {model} ends in {to}, before it starts");
        }
        let tidy = |values: &[String]| -> Result<Vec<String>> {
            values
                .iter()
                .map(|value| {
                    let value = squeeze(value);
                    if value.is_empty() {
                        bail!("presets.yaml: {make} {model} has an empty name");
                    }
                    Ok(value)
                })
                .collect()
        };
        let (submodels, engines) = (tidy(&preset.submodels)?, tidy(&preset.engines)?);
        checked.push(Preset {
            make,
            model,
            from: preset.from,
            to: preset.to,
            submodels,
            engines,
        });
    }
    Ok(checked)
}

#[cfg(test)]
mod tests {
    use super::*;

    const MAKES: &str = "
makes:
  - name: Toyota
    aliases: [yota]
  - name: Chevrolet
    aliases: [chevy, Chev]
  - name: '  Land   Rover '
    aliases: [range rover]
";
    const NAMES: &str = "
submodels:
  \"Si/Si  HPT\": \" Si \"
";
    const PRESETS: &str = "
presets:
  - make: \"Honda\"
    model: \"Civic\"
    from: 2012
    submodels: [\"LX\", \"Si\", \"392\", \"N\"]
    engines: [\"2.0L\"]
";

    #[test]
    fn reads_the_three_files() {
        let curated = Curated::parse(MAKES, NAMES, PRESETS).unwrap();
        let names: Vec<&str> = curated
            .makes
            .iter()
            .map(|make| make.name.as_str())
            .collect();
        assert_eq!(names, vec!["Toyota", "Chevrolet", "Land Rover"]);
        assert_eq!(curated.makes[1].aliases, vec!["chevy", "chev"]);
        assert_eq!(curated.makes[2].aliases, vec!["rangerover"]);
        assert_eq!(
            curated.submodel_names.get("si/si hpt").map(String::as_str),
            Some("Si")
        );
        let preset = &curated.presets[0];
        assert_eq!(
            (preset.make.as_str(), preset.model.as_str()),
            ("Honda", "Civic")
        );
        assert_eq!((preset.from, preset.to), (2012, None));
        assert_eq!(preset.submodels, vec!["LX", "Si", "392", "N"]);
        assert_eq!(preset.engines, vec!["2.0L"]);
    }

    #[test]
    fn empty_lists_are_allowed() {
        let curated = Curated::parse("makes: []\n", "submodels: {}\n", "presets: []\n").unwrap();
        assert_eq!(curated, Curated::default());
    }

    #[test]
    fn a_mistake_is_refused_with_a_message_that_names_it() {
        const NO_MAKES: &str = "makes: []\n";
        const NO_NAMES: &str = "submodels: {}\n";
        const NO_PRESETS: &str = "presets: []\n";
        for (makes, names, presets, expected) in [
            (
                "makes:\n  - name: Honda\n  - name: HONDA\n",
                NO_NAMES,
                NO_PRESETS,
                "lists HONDA twice",
            ),
            (
                "makes:\n  - name: Honda\n    aliases: ['!!']\n",
                NO_NAMES,
                NO_PRESETS,
                "an alias of Honda has no letters or digits",
            ),
            (
                "makes:\n  - name: Honda\n    aliases: [h]\n  - name: Ford\n    aliases: [H]\n",
                NO_NAMES,
                NO_PRESETS,
                "the alias h is used twice",
            ),
            (
                "makes:\n  - name: Honda\n    aliases: [ford]\n  - name: Ford\n",
                NO_NAMES,
                NO_PRESETS,
                "the alias ford is used twice",
            ),
            (
                "makes:\n  - name: '--'\n",
                NO_NAMES,
                NO_PRESETS,
                "a make has no letters or digits",
            ),
            (
                "makes:\n  - nmae: Honda\n",
                NO_NAMES,
                NO_PRESETS,
                "makes.yaml",
            ),
            (
                NO_MAKES,
                "submodels:\n  '': Si\n",
                NO_PRESETS,
                "needs text on both sides",
            ),
            (
                NO_MAKES,
                "submodels:\n  Si: ' '\n",
                NO_PRESETS,
                "needs text on both sides",
            ),
            (
                NO_MAKES,
                NO_NAMES,
                "presets:\n  - make: Honda\n    model: Civic\n    from: 1950\n",
                "Honda Civic starts in 1950, before 1981",
            ),
            (
                NO_MAKES,
                NO_NAMES,
                "presets:\n  - make: Honda\n    model: Civic\n    from: 2012\n    to: 2010\n",
                "Honda Civic ends in 2010, before it starts",
            ),
            (
                NO_MAKES,
                NO_NAMES,
                "presets:\n  - make: Honda\n    model: Civic\n    from: 2012\n    submodels: ['']\n",
                "Honda Civic has an empty name",
            ),
            (
                NO_MAKES,
                NO_NAMES,
                "presets:\n  - make: ''\n    model: Civic\n    from: 2012\n",
                "a preset has no make or no model",
            ),
        ] {
            let error = Curated::parse(makes, names, presets).unwrap_err();
            let message = format!("{error:#}");
            assert!(message.contains(expected), "{message}");
        }
    }

    #[test]
    fn the_files_in_the_repository_load() {
        let directory = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../data");
        let curated = Curated::load(&directory).unwrap();
        assert_eq!(curated.makes.len(), 48);
        assert_eq!(curated.makes[0].name, "Toyota");
        assert!(curated.makes.iter().any(|make| {
            make.name == "Chevrolet" && make.aliases.contains(&"chevy".to_owned())
        }));
        assert_eq!(
            curated.submodel_names.get("si/si hpt").map(String::as_str),
            Some("Si")
        );
        assert_eq!(curated.presets.len(), 35);
        let f150 = curated
            .presets
            .iter()
            .find(|preset| preset.make == "Ford" && preset.model == "F-150")
            .unwrap();
        assert_eq!(f150.from, 2012);
        assert!(f150.submodels.contains(&"Lariat".to_owned()));
        let wrangler = curated
            .presets
            .iter()
            .find(|preset| preset.model == "Wrangler")
            .unwrap();
        assert!(wrangler.submodels.contains(&"392".to_owned()));
    }

    #[test]
    fn a_missing_file_is_named() {
        let error = Curated::load(Path::new("/nonexistent")).unwrap_err();
        assert!(format!("{error:#}").contains("makes.yaml"), "{error:#}");
    }
}
