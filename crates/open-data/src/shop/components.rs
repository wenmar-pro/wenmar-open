//! `data/components.yaml`: the parts and systems a recall, a bulletin, a
//! trouble code or a job is about, and which of them each recall and each
//! bulletin names.

use std::collections::{HashMap, HashSet};
use std::path::Path;

use anyhow::{Context, Result, bail};
use rusqlite::params;
use serde::Deserialize;
use wenmar_shopdata::{Dataset, schema};
use wenmar_vehicles::text::{is_slug, squeeze};

use crate::shop::build::ShapeContext;
use crate::shop::nhtsa_recalls::COMPONENT_SEPARATOR;

/// One entry of `components.yaml`.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Component {
    /// Lowercase words joined by hyphens, such as `brake-pad`.
    pub id: String,
    /// The name a page shows, such as `Brake pads`.
    pub name: String,
    /// The id of the system a part belongs to. A system has none.
    #[serde(default)]
    pub parent: Option<String>,
    /// NHTSA component paths. An entry covers that path and every path
    /// below it.
    #[serde(default)]
    pub nhtsa: Vec<String>,
    /// Transport Canada system names.
    #[serde(default)]
    pub tc: Vec<String>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ComponentsFile {
    components: Vec<Component>,
}

/// Everything `components.yaml` says, checked.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Components {
    pub list: Vec<Component>,
}

/// An NHTSA component path in the form paths are compared in: capitals,
/// single spaces, and no space around a colon.
/// `Steering: Steering  Wheel` is `STEERING:STEERING WHEEL`.
pub fn normalize_path(text: &str) -> String {
    text.split(':')
        .map(|part| squeeze(part).to_uppercase())
        .collect::<Vec<_>>()
        .join(":")
}

/// Whether `path` is `prefix` or lies below it. Both are normalized.
fn covers(prefix: &str, path: &str) -> bool {
    path == prefix
        || (path.len() > prefix.len()
            && path.starts_with(prefix)
            && path.as_bytes()[prefix.len()] == b':')
}

impl Components {
    /// Reads `components.yaml` under `directory`.
    pub fn load(directory: &Path) -> Result<Components> {
        let path = directory.join("components.yaml");
        let text = std::fs::read_to_string(&path)
            .with_context(|| format!("reading {}", path.display()))?;
        Components::parse(&text)
    }

    /// Reads the text of the file and checks it.
    pub fn parse(yaml: &str) -> Result<Components> {
        let file: ComponentsFile =
            serde_saphyr::from_str(yaml).context("reading components.yaml")?;
        let mut ids: HashSet<String> = HashSet::new();
        let mut list = Vec::with_capacity(file.components.len());
        for entry in file.components {
            if !is_slug(&entry.id) {
                bail!(
                    "components.yaml: {:?} is not an id; use lowercase words joined by hyphens",
                    entry.id
                );
            }
            if !ids.insert(entry.id.clone()) {
                bail!("components.yaml lists {} twice", entry.id);
            }
            let name = squeeze(&entry.name);
            if name.is_empty() {
                bail!("components.yaml: {} has no name", entry.id);
            }
            let mut nhtsa = Vec::with_capacity(entry.nhtsa.len());
            for path in &entry.nhtsa {
                let path = normalize_path(path);
                if path.is_empty() || path.split(':').any(str::is_empty) {
                    bail!("components.yaml: {} has an empty NHTSA path", entry.id);
                }
                if nhtsa.contains(&path) {
                    bail!("components.yaml: {} lists {path} twice", entry.id);
                }
                nhtsa.push(path);
            }
            let mut tc = Vec::with_capacity(entry.tc.len());
            for name in &entry.tc {
                let name = squeeze(name).to_uppercase();
                if name.is_empty() {
                    bail!(
                        "components.yaml: {} has an empty Transport Canada name",
                        entry.id
                    );
                }
                tc.push(name);
            }
            list.push(Component {
                id: entry.id,
                name,
                parent: entry.parent,
                nhtsa,
                tc,
            });
        }
        // A part's parent is a system: an entry of this file with no parent.
        for entry in &list {
            let Some(parent) = &entry.parent else {
                continue;
            };
            match list.iter().find(|other| &other.id == parent) {
                None => bail!(
                    "components.yaml: {} has the parent {parent}, which is not in the file",
                    entry.id
                ),
                Some(other) if other.parent.is_some() => bail!(
                    "components.yaml: {} has the parent {parent}, which has a parent itself",
                    entry.id
                ),
                Some(_) => {}
            }
        }
        Ok(Components { list })
    }

    /// The ids of every component one NHTSA component text is about, in
    /// file order: each whose path is the text or lies above it.
    pub fn for_nhtsa(&self, component: &str) -> Vec<&str> {
        let path = normalize_path(component);
        self.list
            .iter()
            .filter(|entry| entry.nhtsa.iter().any(|prefix| covers(prefix, &path)))
            .map(|entry| entry.id.as_str())
            .collect()
    }

    /// The ids of every component one Transport Canada system name is
    /// about, in file order.
    pub fn for_tc(&self, system: &str) -> Vec<&str> {
        let name = squeeze(system).to_uppercase();
        self.list
            .iter()
            .filter(|entry| entry.tc.contains(&name))
            .map(|entry| entry.id.as_str())
            .collect()
    }
}

/// Fills `component` from `components.yaml`, then `recall_component` and
/// `bulletin_component` for whichever of those datasets the file holds.
/// Runs after every source's shape step.
pub fn shape(context: &mut ShapeContext<'_>) -> Result<()> {
    let components = Components::load(context.curated)?;
    let transaction = context.transaction;
    transaction.execute_batch(schema::COMPONENTS)?;
    {
        let mut insert =
            transaction.prepare("INSERT INTO component (id, name, parent) VALUES (?1, ?2, ?3)")?;
        for entry in &components.list {
            insert.execute(params![entry.id, entry.name, entry.parent])?;
        }
    }
    let datasets = context.datasets();
    if datasets.contains(Dataset::RecallsUs) || datasets.contains(Dataset::RecallsCa) {
        transaction.execute_batch(schema::RECALL_COMPONENTS)?;
        let mut select = transaction
            .prepare("SELECT id, source, component FROM recall WHERE component IS NOT NULL")?;
        let mut insert = transaction.prepare(
            "INSERT OR IGNORE INTO recall_component (recall_id, component_id) VALUES (?1, ?2)",
        )?;
        let mut rows = select.query([])?;
        while let Some(row) = rows.next()? {
            let (id, source, text): (i64, String, String) = (row.get(0)?, row.get(1)?, row.get(2)?);
            for part in text.split(COMPONENT_SEPARATOR) {
                let ids = if source == "tc" {
                    components.for_tc(part)
                } else {
                    components.for_nhtsa(part)
                };
                for component in ids {
                    insert.execute(params![id, component])?;
                }
            }
        }
    }
    if datasets.contains(Dataset::Bulletins) {
        transaction.execute_batch(schema::BULLETIN_COMPONENTS)?;
        let mut select = transaction
            .prepare("SELECT id, component FROM bulletin WHERE component IS NOT NULL")?;
        let mut insert = transaction.prepare(
            "INSERT OR IGNORE INTO bulletin_component (bulletin_id, component_id) VALUES (?1, ?2)",
        )?;
        let mut known: HashMap<String, Vec<String>> = HashMap::new();
        let mut rows = select.query([])?;
        while let Some(row) = rows.next()? {
            let (id, text): (i64, String) = (row.get(0)?, row.get(1)?);
            for part in text.split(COMPONENT_SEPARATOR) {
                // A few hundred texts are repeated over a quarter of a
                // million bulletins.
                if !known.contains_key(part) {
                    let ids = components.for_nhtsa(part);
                    known.insert(
                        part.to_owned(),
                        ids.into_iter().map(str::to_owned).collect(),
                    );
                }
                for component in &known[part] {
                    insert.execute(params![id, component])?;
                }
            }
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    const YAML: &str = "
components:
  - id: brakes
    name: ' Brakes '
    nhtsa: ['Service Brakes, Hydraulic', 'PARKING BRAKE']
    tc: [Brakes]
  - id: brake-pad
    name: Brake pads
    parent: brakes
    nhtsa: ['SERVICE BRAKES, HYDRAULIC: FOUNDATION  COMPONENTS:DISC:PADS']
  - id: timing-belt
    name: Timing belt
";

    #[test]
    fn a_path_is_compared_in_one_form() {
        assert_eq!(
            normalize_path(" Steering:  Steering  wheel/handle bar "),
            "STEERING:STEERING WHEEL/HANDLE BAR"
        );
        assert_eq!(
            normalize_path("FUEL SYSTEM, GASOLINE"),
            "FUEL SYSTEM, GASOLINE"
        );
        assert!(covers("SERVICE BRAKES", "SERVICE BRAKES"));
        assert!(covers("SERVICE BRAKES", "SERVICE BRAKES:DISC"));
        // Another name that starts the same way is another system.
        assert!(!covers("SERVICE BRAKES", "SERVICE BRAKES, AIR"));
        assert!(!covers("SERVICE BRAKES:DISC", "SERVICE BRAKES"));
    }

    #[test]
    fn a_text_is_about_every_component_above_it() {
        let components = Components::parse(YAML).unwrap();
        assert_eq!(components.list[0].name, "Brakes");
        assert_eq!(
            components.for_nhtsa("SERVICE BRAKES, HYDRAULIC:FOUNDATION COMPONENTS:DISC:PADS "),
            ["brakes", "brake-pad"]
        );
        assert_eq!(
            components.for_nhtsa("SERVICE BRAKES, HYDRAULIC:FOUNDATION COMPONENTS:DISC"),
            ["brakes"]
        );
        assert_eq!(components.for_nhtsa("parking brake:electrical"), ["brakes"]);
        assert!(components.for_nhtsa("SERVICE BRAKES, AIR").is_empty());
        assert!(components.for_nhtsa("UNKNOWN OR OTHER").is_empty());
        assert_eq!(components.for_tc(" brakes "), ["brakes"]);
        assert!(components.for_tc("Other").is_empty());
        // A component may cover nothing in either source.
        assert!(components.list[2].nhtsa.is_empty());
    }

    #[test]
    fn a_mistake_in_the_file_is_named() {
        for (yaml, expected) in [
            (
                "components:\n  - id: Brake Pad\n    name: x\n",
                "\"Brake Pad\" is not an id",
            ),
            (
                "components:\n  - id: a\n    name: x\n  - id: a\n    name: y\n",
                "lists a twice",
            ),
            ("components:\n  - id: a\n    name: ' '\n", "a has no name"),
            (
                "components:\n  - id: a\n    name: x\n    nhtsa: ['A::B']\n",
                "a has an empty NHTSA path",
            ),
            (
                "components:\n  - id: a\n    name: x\n    nhtsa: [A, 'a ']\n",
                "a lists A twice",
            ),
            (
                "components:\n  - id: a\n    name: x\n    tc: ['']\n",
                "a has an empty Transport Canada name",
            ),
            (
                "components:\n  - id: a\n    name: x\n    parent: b\n",
                "a has the parent b, which is not in the file",
            ),
            (
                "components:\n  - id: a\n    name: x\n  - id: b\n    name: y\n    parent: a\n  - id: c\n    name: z\n    parent: b\n",
                "c has the parent b, which has a parent itself",
            ),
            (
                "components:\n  - id: a\n    name: x\n    colour: red\n",
                "components.yaml",
            ),
        ] {
            let error = Components::parse(yaml).unwrap_err();
            let message = format!("{error:#}");
            assert!(message.contains(expected), "{message}");
        }
    }

    #[test]
    fn the_file_in_the_repository_loads() {
        let directory = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../data");
        let components = Components::load(&directory).unwrap();
        assert!(components.list.len() >= 100);
        assert_eq!(
            components.for_nhtsa("SERVICE BRAKES, HYDRAULIC:FOUNDATION COMPONENTS:DISC:PADS"),
            ["brakes", "brake-pad"]
        );
        assert_eq!(
            components.for_nhtsa("ENGINE AND ENGINE COOLING:COOLING SYSTEM:PUMP"),
            ["engine", "cooling-system", "water-pump"]
        );
        assert_eq!(components.for_tc("Airbag"), ["air-bags"]);
        let error = Components::load(Path::new("/nonexistent")).unwrap_err();
        assert!(
            format!("{error:#}").contains("components.yaml"),
            "{error:#}"
        );
    }
}
