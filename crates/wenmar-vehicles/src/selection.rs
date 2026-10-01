//! From a decoded VIN to the catalog entry the cascade would have reached.

use serde::Serialize;
use wenmar_vin::Decoded;

use crate::catalog::{Catalog, CatalogError, EngineRow, Entry, Known, SubmodelRow};
use crate::id::VehicleId;
use crate::source::Source;
use crate::summary::short;
use crate::text::slug;

/// The catalog's answer for one decoded VIN: what to prefill in a form.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[non_exhaustive]
pub struct Selection {
    /// The most specific entry the decode reaches. Its body, drive and
    /// transmission are the decode's own where the decode has them.
    pub entry: Entry,
    /// The id of the year, make and model.
    pub vehicle_id: String,
    /// The submodel's id form, when the decode names exactly one.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub submodel_id: Option<String>,
    /// The engine's id form, when the decode settles on one.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub engine_id: Option<String>,
}

/// The one submodel the names point to. `None` when they point to none or
/// to several: vPIC sometimes lists trims together, as in `S, SE, SEL`, and
/// guessing one would be wrong more often than right.
fn only_submodel<'r>(rows: &'r [SubmodelRow], names: &[&str]) -> Option<&'r SubmodelRow> {
    let wanted: Vec<String> = names
        .iter()
        .map(|name| slug(name))
        .filter(|name| !name.is_empty())
        .collect();
    let mut matching = rows.iter().filter(|row| wanted.contains(&slug(&row.name)));
    match (matching.next(), matching.next()) {
        (Some(row), None) => Some(row),
        _ => None,
    }
}

fn only_engine<'r>(mut matching: impl Iterator<Item = &'r EngineRow>) -> Option<&'r EngineRow> {
    match (matching.next(), matching.next()) {
        (Some(row), None) => Some(row),
        _ => None,
    }
}

/// A label without its cylinder layout: `5.3L V8` gives `5.3L`. The data
/// build merges such pairs, so a decode may carry the shorter form.
fn without_layout(label: &str) -> String {
    label
        .split(' ')
        .filter(|word| {
            let digits = word.strip_prefix('V').unwrap_or("");
            digits.is_empty() || !digits.bytes().all(|byte| byte.is_ascii_digit())
        })
        .collect::<Vec<_>>()
        .join(" ")
}

impl<S: Source> Catalog<S> {
    /// The catalog entry and ids for a decoded VIN. `None` when the decode
    /// has no year, make or model, or the catalog has no such model year.
    pub fn selection(&self, decoded: &Decoded) -> Result<Option<Selection>, CatalogError> {
        let (Some(year), Some(make), Some(model)) = (
            decoded.year,
            decoded.make.as_deref(),
            decoded.model.as_deref(),
        ) else {
            return Ok(None);
        };
        let Some(vehicle) = self.vehicle(make, model, year)? else {
            return Ok(None);
        };

        let submodels = self.submodel_rows(vehicle.detail_id)?;
        let submodel = decoded
            .trim
            .as_deref()
            .and_then(|trim| only_submodel(&submodels, &trim.split(',').collect::<Vec<_>>()))
            .or_else(|| {
                decoded
                    .series
                    .as_deref()
                    .and_then(|series| only_submodel(&submodels, &[series]))
            });

        let engines = self.engine_rows(vehicle.detail_id, submodel.map(|row| row.id))?;
        let label = decoded
            .engine
            .as_ref()
            .and_then(|engine| engine.label.as_deref());
        let eighth = decoded.vin.chars().nth(7);
        let engine = label
            .and_then(|label| engines.iter().find(|row| row.label == label))
            .or_else(|| {
                label.and_then(|label| {
                    only_engine(
                        engines
                            .iter()
                            .filter(|row| without_layout(&row.label) == label),
                    )
                })
            })
            .or_else(|| {
                eighth.and_then(|character| {
                    only_engine(engines.iter().filter(|row| {
                        row.vin8
                            .as_deref()
                            .is_some_and(|claimed| claimed.contains(character))
                    }))
                })
            });

        let transmission = decoded.transmission.as_deref().map(short);
        let body = decoded.body.as_deref().map(short);
        let known = Known {
            transmission: transmission.as_deref(),
            drive: decoded.drivetrain.as_deref(),
            body: body.as_deref(),
        };
        Ok(Some(Selection {
            entry: self.entry_of(
                &vehicle,
                submodel,
                engine.map(|row| row.label.as_str()),
                &known,
            ),
            vehicle_id: VehicleId {
                year,
                make: vehicle.make.slug.clone(),
                model: vehicle.model.slug.clone(),
                submodel: None,
                engine: None,
            }
            .to_string(),
            submodel_id: submodel.map(|row| slug(&row.name)),
            engine_id: engine.map(|row| slug(&row.label)),
        }))
    }
}
