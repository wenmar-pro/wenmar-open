//! Stable vehicle ids.
//!
//! An id is the year and the id forms of the names, joined by underscores:
//! `2019_ford_f-150`, then optionally a submodel and an engine, as in
//! `2019_ford_f-150_raptor_3-5l-turbo-v6`. An engine without a submodel
//! leaves the submodel empty: `2019_ford_f-150__5-0l-v8`. The names come
//! from NHTSA's public-domain vPIC data and from this project's own lists,
//! so an id stays the same from one data release to the next for as long as
//! the names do.

use std::fmt;

use crate::text::is_slug;

/// The parts of a vehicle id.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct VehicleId {
    pub year: u16,
    pub make: String,
    pub model: String,
    pub submodel: Option<String>,
    pub engine: Option<String>,
}

impl VehicleId {
    /// Reads an id. `None` for anything that is not exactly an id.
    pub fn parse(text: &str) -> Option<VehicleId> {
        let mut parts = text.split('_');
        let year = parts.next()?;
        let make = parts.next()?;
        let model = parts.next()?;
        let submodel = parts.next();
        let engine = parts.next();
        if parts.next().is_some() {
            return None;
        }
        if year.len() != 4 || !year.bytes().all(|byte| byte.is_ascii_digit()) {
            return None;
        }
        if !is_slug(make) || !is_slug(model) {
            return None;
        }
        let submodel = match (submodel, engine) {
            (None, _) | (Some(""), Some(_)) => None,
            (Some(slug), _) if is_slug(slug) => Some(slug.to_owned()),
            _ => return None,
        };
        let engine = match engine {
            None => None,
            Some(slug) if is_slug(slug) => Some(slug.to_owned()),
            Some(_) => return None,
        };
        Some(VehicleId {
            year: year.parse().ok()?,
            make: make.to_owned(),
            model: model.to_owned(),
            submodel,
            engine,
        })
    }
}

impl fmt::Display for VehicleId {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{:04}_{}_{}", self.year, self.make, self.model)?;
        match (&self.submodel, &self.engine) {
            (Some(submodel), Some(engine)) => write!(formatter, "_{submodel}_{engine}"),
            (Some(submodel), None) => write!(formatter, "_{submodel}"),
            (None, Some(engine)) => write!(formatter, "__{engine}"),
            (None, None) => Ok(()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn id(
        year: u16,
        make: &str,
        model: &str,
        submodel: Option<&str>,
        engine: Option<&str>,
    ) -> VehicleId {
        VehicleId {
            year,
            make: make.to_owned(),
            model: model.to_owned(),
            submodel: submodel.map(str::to_owned),
            engine: engine.map(str::to_owned),
        }
    }

    #[test]
    fn reads_and_writes_each_level() {
        for (text, expected) in [
            ("2019_ford_f-150", id(2019, "ford", "f-150", None, None)),
            (
                "2019_ford_f-150_raptor",
                id(2019, "ford", "f-150", Some("raptor"), None),
            ),
            (
                "2019_ford_f-150_raptor_3-5l-turbo-v6",
                id(2019, "ford", "f-150", Some("raptor"), Some("3-5l-turbo-v6")),
            ),
            (
                "2019_ford_f-150__5-0l-v8",
                id(2019, "ford", "f-150", None, Some("5-0l-v8")),
            ),
        ] {
            assert_eq!(VehicleId::parse(text), Some(expected.clone()), "{text}");
            assert_eq!(expected.to_string(), text);
        }
    }

    #[test]
    fn anything_else_is_not_an_id() {
        for text in [
            "",
            "2019",
            "2019_ford",
            "19_ford_f-150",
            "20190_ford_f-150",
            "2019_Ford_f-150",
            "2019_ford_f 150",
            "2019_ford_f-150_",
            "2019_ford_f-150__",
            "2019_ford_f-150_raptor_",
            "2019_ford_f-150_a_b_c",
            "2019__f-150",
            "２０１９_ford_f-150",
            "2019_ford_f-150_%",
        ] {
            assert_eq!(VehicleId::parse(text), None, "{text:?}");
        }
    }
}
