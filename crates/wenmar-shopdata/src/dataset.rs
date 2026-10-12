//! The datasets a shop file may hold, and what it says about each.

use std::collections::BTreeMap;
use std::fmt;

use serde::ser::SerializeMap;
use serde::{Deserialize, Serialize, Serializer};

use crate::error::ShopError;

/// One optional dataset. The order here is the order they are listed in.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Dataset {
    RecallsUs,
    RecallsCa,
    Investigations,
    RecallReports,
    Ratings,
    Bulletins,
    Complaints,
    Codes,
    FuelUs,
    FuelCa,
    Specs,
    OilGrade,
    Fluids,
    Tires,
    Manuals,
    Jobs,
    Labor,
}

impl Dataset {
    pub const ALL: [Dataset; 17] = [
        Dataset::RecallsUs,
        Dataset::RecallsCa,
        Dataset::Investigations,
        Dataset::RecallReports,
        Dataset::Ratings,
        Dataset::Bulletins,
        Dataset::Complaints,
        Dataset::Codes,
        Dataset::FuelUs,
        Dataset::FuelCa,
        Dataset::Specs,
        Dataset::OilGrade,
        Dataset::Fluids,
        Dataset::Tires,
        Dataset::Manuals,
        Dataset::Jobs,
        Dataset::Labor,
    ];

    /// The key in the `datasets` meta value.
    pub fn id(&self) -> &'static str {
        match self {
            Dataset::RecallsUs => "recalls_us",
            Dataset::RecallsCa => "recalls_ca",
            Dataset::Investigations => "investigations",
            Dataset::RecallReports => "recall_reports",
            Dataset::Ratings => "ratings",
            Dataset::Bulletins => "bulletins",
            Dataset::Complaints => "complaints",
            Dataset::Codes => "codes",
            Dataset::FuelUs => "fuel_us",
            Dataset::FuelCa => "fuel_ca",
            Dataset::Specs => "specs",
            Dataset::OilGrade => "oil_grade",
            Dataset::Fluids => "fluids",
            Dataset::Tires => "tires",
            Dataset::Manuals => "manuals",
            Dataset::Jobs => "jobs",
            Dataset::Labor => "labor",
        }
    }

    pub fn parse(id: &str) -> Option<Dataset> {
        Dataset::ALL.into_iter().find(|dataset| dataset.id() == id)
    }
}

impl fmt::Display for Dataset {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.id())
    }
}

impl Serialize for Dataset {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(self.id())
    }
}

/// What a shop file says about one dataset it holds.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DatasetEntry {
    /// The day the source was current, as `2026-10-11`.
    pub as_of: String,
    /// Rows in the dataset's main table.
    pub rows: u64,
    /// One of [`crate::licence::ALL`].
    pub licence: String,
    /// The credit the licence asks for. Empty when it asks for none.
    #[serde(default)]
    pub attribution: String,
}

/// The `datasets` meta value: the datasets a shop file holds.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Datasets {
    entries: BTreeMap<Dataset, DatasetEntry>,
}

impl Datasets {
    /// Reads the JSON object. A key this build does not know is skipped, so
    /// an older reader can open a newer file.
    pub fn parse(json: &str) -> Result<Datasets, ShopError> {
        let bad = |error: serde_json::Error| {
            ShopError::NotAShopFile(format!("meta datasets is not readable: {error}"))
        };
        let raw: BTreeMap<String, serde_json::Value> = serde_json::from_str(json).map_err(bad)?;
        let mut entries = BTreeMap::new();
        for (key, value) in raw {
            if let Some(dataset) = Dataset::parse(&key) {
                entries.insert(dataset, serde_json::from_value(value).map_err(bad)?);
            }
        }
        Ok(Datasets { entries })
    }

    pub fn get(&self, dataset: Dataset) -> Option<&DatasetEntry> {
        self.entries.get(&dataset)
    }

    pub fn contains(&self, dataset: Dataset) -> bool {
        self.entries.contains_key(&dataset)
    }

    pub fn insert(&mut self, dataset: Dataset, entry: DatasetEntry) -> Option<DatasetEntry> {
        self.entries.insert(dataset, entry)
    }

    pub fn iter(&self) -> impl Iterator<Item = (Dataset, &DatasetEntry)> {
        self.entries
            .iter()
            .map(|(dataset, entry)| (*dataset, entry))
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// The JSON object stored in `meta`.
    pub fn to_json(&self) -> String {
        serde_json::to_string(self).unwrap_or_else(|_| "{}".to_owned())
    }
}

impl Serialize for Datasets {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        let mut map = serializer.serialize_map(Some(self.entries.len()))?;
        for (dataset, entry) in &self.entries {
            map.serialize_entry(dataset.id(), entry)?;
        }
        map.end()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn entry(rows: u64) -> DatasetEntry {
        DatasetEntry {
            as_of: "2026-10-11".to_owned(),
            rows,
            licence: crate::licence::PUBLIC_DOMAIN.to_owned(),
            attribution: String::new(),
        }
    }

    #[test]
    fn every_dataset_has_a_distinct_id_that_parses_back() {
        let ids: Vec<&str> = Dataset::ALL.iter().map(Dataset::id).collect();
        assert_eq!(
            ids,
            [
                "recalls_us",
                "recalls_ca",
                "investigations",
                "recall_reports",
                "ratings",
                "bulletins",
                "complaints",
                "codes",
                "fuel_us",
                "fuel_ca",
                "specs",
                "oil_grade",
                "fluids",
                "tires",
                "manuals",
                "jobs",
                "labor"
            ]
        );
        for dataset in Dataset::ALL {
            assert_eq!(Dataset::parse(dataset.id()), Some(dataset));
            assert_eq!(dataset.to_string(), dataset.id());
        }
        assert_eq!(Dataset::parse("Recalls_US"), None);
        assert_eq!(Dataset::parse(""), None);
    }

    #[test]
    fn datasets_round_trip_through_json() {
        let mut datasets = Datasets::default();
        assert_eq!(datasets.to_json(), "{}");
        assert!(datasets.is_empty());
        datasets.insert(Dataset::RecallsCa, entry(7));
        datasets.insert(Dataset::RecallsUs, entry(9));
        let json = datasets.to_json();
        assert_eq!(
            json,
            r#"{"recalls_us":{"as_of":"2026-10-11","rows":9,"licence":"public-domain","attribution":""},"recalls_ca":{"as_of":"2026-10-11","rows":7,"licence":"public-domain","attribution":""}}"#
        );
        assert_eq!(Datasets::parse(&json).unwrap(), datasets);
        assert_eq!(datasets.len(), 2);
        assert!(datasets.contains(Dataset::RecallsUs));
        assert!(!datasets.contains(Dataset::Codes));
        assert_eq!(datasets.get(Dataset::RecallsCa).unwrap().rows, 7);
        let listed: Vec<Dataset> = datasets.iter().map(|(dataset, _)| dataset).collect();
        assert_eq!(listed, [Dataset::RecallsUs, Dataset::RecallsCa]);
    }

    #[test]
    fn an_unknown_dataset_is_skipped_and_a_malformed_one_is_refused() {
        let newer = r#"{"from_the_future":{"anything":1},"codes":{"as_of":"2026-10-11","rows":3,"licence":"mit"}}"#;
        let datasets = Datasets::parse(newer).unwrap();
        assert_eq!(datasets.len(), 1);
        assert_eq!(datasets.get(Dataset::Codes).unwrap().attribution, "");
        for bad in ["", "[]", "not json", r#"{"codes":{"rows":"three"}}"#] {
            let error = Datasets::parse(bad).unwrap_err().to_string();
            assert!(
                error.starts_with("not a Wenmar Open shop file: meta datasets"),
                "{error}"
            );
        }
    }
}
