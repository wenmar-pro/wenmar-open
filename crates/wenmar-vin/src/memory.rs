use crate::data::{DataError, Element, Manufacturer, Pattern, VinData};

#[derive(Debug, Clone)]
struct SchemaRange {
    wmi: String,
    schema_id: i64,
    year_from: u16,
    year_to: Option<u16>,
}

/// Decoding data held in memory. For tests, examples, and small fixed sets.
#[derive(Debug, Clone, Default)]
pub struct MemoryData {
    manufacturers: Vec<Manufacturer>,
    schemas: Vec<SchemaRange>,
    patterns: Vec<Pattern>,
}

impl MemoryData {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_manufacturer(mut self, manufacturer: Manufacturer) -> Self {
        self.manufacturers.push(manufacturer);
        self
    }

    /// Declares a schema valid for a manufacturer code from `year_from` to
    /// `year_to` inclusive. `None` means still current.
    pub fn with_schema(
        mut self,
        wmi: &str,
        schema_id: i64,
        year_from: u16,
        year_to: Option<u16>,
    ) -> Self {
        self.schemas.push(SchemaRange {
            wmi: wmi.to_owned(),
            schema_id,
            year_from,
            year_to,
        });
        self
    }

    /// Adds a pattern. Ids are assigned in insertion order, starting at 1.
    pub fn with_pattern(
        mut self,
        schema_id: i64,
        keys: &str,
        element: Element,
        value: &str,
    ) -> Self {
        let id = i64::try_from(self.patterns.len()).unwrap_or(i64::MAX - 1) + 1;
        self.patterns.push(Pattern {
            id,
            schema_id,
            keys: keys.to_owned(),
            element,
            value: value.to_owned(),
        });
        self
    }
}

impl VinData for MemoryData {
    fn manufacturer(&self, wmi: &str) -> Result<Option<Manufacturer>, DataError> {
        Ok(self
            .manufacturers
            .iter()
            .find(|manufacturer| manufacturer.wmi == wmi)
            .cloned())
    }

    fn schemas(&self, wmi: &str, year: u16) -> Result<Vec<i64>, DataError> {
        let mut ids: Vec<i64> = self
            .schemas
            .iter()
            .filter(|range| {
                range.wmi == wmi
                    && year >= range.year_from
                    && range.year_to.is_none_or(|year_to| year <= year_to)
            })
            .map(|range| range.schema_id)
            .collect();
        ids.sort_unstable();
        ids.dedup();
        Ok(ids)
    }

    fn patterns(&self, schema_ids: &[i64], _match_key: &str) -> Result<Vec<Pattern>, DataError> {
        Ok(self
            .patterns
            .iter()
            .filter(|pattern| schema_ids.contains(&pattern.schema_id))
            .cloned()
            .collect())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn data() -> MemoryData {
        MemoryData::new()
            .with_manufacturer(Manufacturer {
                wmi: "KM8".to_owned(),
                name: "Hyundai Motor Co".to_owned(),
                make: Some("Hyundai".to_owned()),
                country: None,
                vehicle_type: None,
            })
            .with_schema("KM8", 1, 2022, None)
            .with_schema("KM8", 2, 1990, Some(1995))
            .with_schema("KMH", 3, 2022, None)
            .with_pattern(1, "K2***", Element::Model, "Kona")
            .with_pattern(2, "K2***", Element::Model, "Old Model")
    }

    #[test]
    fn finds_a_manufacturer_by_code() {
        assert_eq!(
            data().manufacturer("KM8").unwrap().unwrap().name,
            "Hyundai Motor Co"
        );
        assert_eq!(data().manufacturer("ZZZ").unwrap(), None);
    }

    #[test]
    fn schema_year_ranges_are_inclusive_and_may_be_open_ended() {
        let data = data();
        assert_eq!(data.schemas("KM8", 2021).unwrap(), Vec::<i64>::new());
        assert_eq!(data.schemas("KM8", 2022).unwrap(), vec![1]);
        assert_eq!(data.schemas("KM8", 2050).unwrap(), vec![1]);
        assert_eq!(data.schemas("KM8", 1990).unwrap(), vec![2]);
        assert_eq!(data.schemas("KM8", 1995).unwrap(), vec![2]);
        assert_eq!(data.schemas("KM8", 1996).unwrap(), Vec::<i64>::new());
    }

    #[test]
    fn returns_patterns_for_the_requested_schemas_only() {
        let data = data();
        let patterns = data.patterns(&[1], "K2CAB|PU001140").unwrap();
        assert_eq!(patterns.len(), 1);
        assert_eq!(patterns[0].value, "Kona");
        assert_eq!(patterns[0].id, 1);
        assert_eq!(data.patterns(&[], "K2CAB|PU001140").unwrap(), Vec::new());
    }

    #[test]
    fn a_reference_to_data_is_also_data() {
        fn count(data: impl VinData) -> usize {
            data.patterns(&[1, 2], "K2CAB|PU001140").unwrap().len()
        }
        let data = data();
        assert_eq!(count(&data), 2);
    }
}
