use crate::data::{
    DataError, Element, EngineRow, Manufacturer, Pattern, SchemaRef, SpecRow, VinData,
};

#[derive(Debug, Clone)]
struct SchemaRange {
    wmi: String,
    schema_id: i64,
    year_from: u16,
    year_to: Option<u16>,
}

/// A specification row with the model and year its sheet was declared for.
#[derive(Debug, Clone)]
struct SpecEntry {
    model_attribute: String,
    /// `None` means every year.
    year: Option<u16>,
    row: SpecRow,
}

/// Decoding data held in memory. For tests, examples, and small fixed sets.
#[derive(Debug, Clone, Default)]
pub struct MemoryData {
    manufacturers: Vec<Manufacturer>,
    schemas: Vec<SchemaRange>,
    patterns: Vec<Pattern>,
    engine_rows: Vec<(String, EngineRow)>,
    spec_rows: Vec<SpecEntry>,
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
    pub fn with_pattern(self, schema_id: i64, keys: &str, element: Element, value: &str) -> Self {
        self.with_pattern_changed(schema_id, keys, element, value, "")
    }

    /// Adds a pattern with the time it was last changed.
    pub fn with_pattern_changed(
        self,
        schema_id: i64,
        keys: &str,
        element: Element,
        value: &str,
        changed_on: &str,
    ) -> Self {
        self.push_pattern(schema_id, keys, element, value, value, changed_on)
    }

    /// Adds a pattern whose raw attribute differs from its resolved value, as
    /// a lookup id does.
    pub fn with_pattern_attribute(
        self,
        schema_id: i64,
        keys: &str,
        element: Element,
        attribute: &str,
        value: &str,
    ) -> Self {
        self.push_pattern(schema_id, keys, element, attribute, value, "")
    }

    /// Adds a value an engine model implies. Ids are assigned in insertion
    /// order, starting at 1.
    pub fn with_engine_row(
        mut self,
        engine_model: &str,
        element: Element,
        value: &str,
        changed_on: &str,
    ) -> Self {
        let id = i64::try_from(self.engine_rows.len()).unwrap_or(i64::MAX - 1) + 1;
        self.engine_rows.push((
            engine_model.trim().to_lowercase(),
            EngineRow {
                id,
                element,
                attribute: value.to_owned(),
                value: value.to_owned(),
                changed_on: changed_on.to_owned(),
            },
        ));
        self
    }

    /// Adds a key row to a specification sheet: the sheet applies only to a
    /// vehicle whose raw attribute for `element` is `attribute`. `year: None`
    /// means the sheet applies to every year. Ids are assigned in insertion
    /// order across key and value rows, starting at 1.
    pub fn with_spec_key(
        self,
        model_attribute: &str,
        year: Option<u16>,
        spec_pattern_id: i64,
        element: Element,
        attribute: &str,
    ) -> Self {
        let row = SpecRow {
            id: 0,
            spec_pattern_id,
            is_key: true,
            element,
            attribute: attribute.to_owned(),
            value: attribute.to_owned(),
            changed_on: String::new(),
        };
        self.push_spec(model_attribute, year, row)
    }

    /// Adds a value a specification sheet supplies when all its keys match.
    /// `year: None` means the sheet applies to every year.
    pub fn with_spec_value(
        self,
        model_attribute: &str,
        year: Option<u16>,
        spec_pattern_id: i64,
        element: Element,
        value: &str,
        changed_on: &str,
    ) -> Self {
        let row = SpecRow {
            id: 0,
            spec_pattern_id,
            is_key: false,
            element,
            attribute: value.to_owned(),
            value: value.to_owned(),
            changed_on: changed_on.to_owned(),
        };
        self.push_spec(model_attribute, year, row)
    }

    /// Stores a specification row, giving it the next id.
    fn push_spec(mut self, model_attribute: &str, year: Option<u16>, row: SpecRow) -> Self {
        let id = i64::try_from(self.spec_rows.len()).unwrap_or(i64::MAX - 1) + 1;
        self.spec_rows.push(SpecEntry {
            model_attribute: model_attribute.trim().to_owned(),
            year,
            row: SpecRow { id, ..row },
        });
        self
    }

    fn push_pattern(
        mut self,
        schema_id: i64,
        keys: &str,
        element: Element,
        attribute: &str,
        value: &str,
        changed_on: &str,
    ) -> Self {
        let id = i64::try_from(self.patterns.len()).unwrap_or(i64::MAX - 1) + 1;
        self.patterns.push(Pattern {
            id,
            schema_id,
            keys: keys.to_owned(),
            element,
            attribute: attribute.to_owned(),
            value: value.to_owned(),
            changed_on: changed_on.to_owned(),
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

    fn schemas(&self, wmi: &str, year: u16) -> Result<Vec<SchemaRef>, DataError> {
        Ok(self
            .schemas
            .iter()
            .filter(|range| {
                range.wmi == wmi
                    && year >= range.year_from
                    && range.year_to.is_none_or(|year_to| year <= year_to)
            })
            .map(|range| SchemaRef {
                id: range.schema_id,
                year_from: range.year_from,
            })
            .collect())
    }

    fn patterns(&self, schema_ids: &[i64], _match_key: &str) -> Result<Vec<Pattern>, DataError> {
        Ok(self
            .patterns
            .iter()
            .filter(|pattern| schema_ids.contains(&pattern.schema_id))
            .cloned()
            .collect())
    }

    fn engine_model(&self, name: &str) -> Result<Vec<EngineRow>, DataError> {
        let name = name.trim().to_lowercase();
        Ok(self
            .engine_rows
            .iter()
            .filter(|(stored, _)| *stored == name)
            .map(|(_, row)| row.clone())
            .collect())
    }

    fn specs(
        &self,
        _wmi: &str,
        model_attribute: &str,
        year: u16,
    ) -> Result<Vec<SpecRow>, DataError> {
        let model_attribute = model_attribute.trim();
        Ok(self
            .spec_rows
            .iter()
            .filter(|entry| {
                entry.model_attribute == model_attribute
                    && entry.year.is_none_or(|declared| declared == year)
            })
            .map(|entry| entry.row.clone())
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
                light_vehicle: true,
            })
            .with_schema("KM8", 1, 2022, None)
            .with_schema("KM8", 2, 1990, Some(1995))
            .with_schema("KMH", 3, 2022, None)
            .with_pattern(1, "K2***", Element::Model, "Kona")
            .with_pattern(2, "K2***", Element::Model, "Old Model")
    }

    fn ids(data: &MemoryData, year: u16) -> Vec<i64> {
        data.schemas("KM8", year)
            .unwrap()
            .iter()
            .map(|schema| schema.id)
            .collect()
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
        assert_eq!(ids(&data, 2021), Vec::<i64>::new());
        assert_eq!(ids(&data, 2022), vec![1]);
        assert_eq!(ids(&data, 2050), vec![1]);
        assert_eq!(ids(&data, 1990), vec![2]);
        assert_eq!(ids(&data, 1995), vec![2]);
        assert_eq!(ids(&data, 1996), Vec::<i64>::new());
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

    #[test]
    fn spec_rows_are_returned_for_their_model_and_year_only() {
        let data = MemoryData::new()
            .with_spec_key("900", Some(2023), 10, Element::Trim, "SE")
            .with_spec_value(
                "900",
                Some(2023),
                10,
                Element::Abs,
                "Standard",
                "2023-01-01",
            )
            .with_spec_value("900", None, 11, Element::Esc, "Standard", "")
            .with_spec_value("901", Some(2023), 12, Element::Esc, "Optional", "");
        let rows = data.specs("ANY", " 900 ", 2023).unwrap();
        assert_eq!(
            rows.iter().map(|row| row.id).collect::<Vec<_>>(),
            vec![1, 2, 3]
        );
        assert!(rows[0].is_key);
        assert_eq!(rows[0].attribute, "SE");
        assert!(!rows[1].is_key);
        assert_eq!(
            (rows[1].value.as_str(), rows[1].changed_on.as_str()),
            ("Standard", "2023-01-01")
        );
        let other_year = data.specs("ANY", "900", 2021).unwrap();
        assert_eq!(
            other_year
                .iter()
                .map(|row| row.spec_pattern_id)
                .collect::<Vec<_>>(),
            vec![11]
        );
    }

    #[test]
    fn specs_default_to_none_and_are_forwarded_by_reference() {
        struct Bare;
        impl VinData for Bare {
            fn manufacturer(&self, _wmi: &str) -> Result<Option<Manufacturer>, DataError> {
                Ok(None)
            }
            fn schemas(&self, _wmi: &str, _year: u16) -> Result<Vec<SchemaRef>, DataError> {
                Ok(Vec::new())
            }
            fn patterns(&self, _ids: &[i64], _key: &str) -> Result<Vec<Pattern>, DataError> {
                Ok(Vec::new())
            }
        }
        fn specs(data: impl VinData) -> Vec<SpecRow> {
            data.specs("KM8", "900", 2023).unwrap()
        }
        assert_eq!(specs(Bare), Vec::new());
        assert_eq!(specs(&Bare), Vec::new());
        let sheet = MemoryData::new().with_spec_key("900", None, 10, Element::Trim, "SE");
        assert_eq!(specs(&sheet).len(), 1);
    }

    #[test]
    fn a_pattern_keeps_its_raw_attribute() {
        let data = MemoryData::new()
            .with_pattern(1, "K2***", Element::Trim, "SE")
            .with_pattern_attribute(1, "K2***", Element::DriveType, "1", "FWD/Front-Wheel Drive");
        let patterns = data.patterns(&[1], "K2CAB|PU001140").unwrap();
        assert_eq!(
            (patterns[0].attribute.as_str(), patterns[0].value.as_str()),
            ("SE", "SE")
        );
        assert_eq!(
            (patterns[1].attribute.as_str(), patterns[1].value.as_str()),
            ("1", "FWD/Front-Wheel Drive")
        );
    }
}
