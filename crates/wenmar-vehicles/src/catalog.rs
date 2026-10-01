//! The catalog: the cascade a person steps through, and entries by id.

use serde::Serialize;

use crate::id::VehicleId;
use crate::index::{MakeIndex, MakeRef, Scope, term};
use crate::source::{Source, SourceError, Value};
use crate::sql;
use crate::summary::{self, Parts};
use crate::text::{normalize, slug};

/// Why a catalog question could not be answered.
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum CatalogError {
    /// The database failed.
    #[error("the catalog could not be read: {0}")]
    Source(#[source] SourceError),
    /// A row did not have the columns this crate expects.
    #[error("the catalog has an unexpected row in {0}")]
    Shape(&'static str),
}

/// A make in the make step.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[non_exhaustive]
pub struct Make {
    /// The make's id form, such as `mercedes-benz`.
    pub id: String,
    pub name: String,
    /// Whether it is on the curated list of popular makes.
    pub popular: bool,
}

/// A model in the model step.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[non_exhaustive]
pub struct Model {
    /// The model's id form, such as `f-150`. Unique within its make.
    pub id: String,
    pub name: String,
    /// The first and last model year the model exists in.
    pub year_from: u16,
    pub year_to: u16,
}

/// A submodel in the submodel step.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[non_exhaustive]
pub struct Submodel {
    /// The submodel's id form, such as `ex-l`.
    pub id: String,
    pub name: String,
    /// `trim` or `series` as vPIC has it, or `preset` from this project's list.
    pub kind: String,
}

/// An engine in the engine step.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[non_exhaustive]
pub struct EngineOption {
    /// The engine's id form, such as `3-5l-turbo-v6`.
    pub id: String,
    /// The short name, such as `3.5L Turbo V6`.
    pub label: String,
    /// The characters in position 8 of a VIN that mean this engine and no
    /// other for this model year, where the data settles it.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub vin8: Option<String>,
    /// Whether the engine comes from this project's list instead of vPIC.
    pub preset: bool,
}

/// One vehicle of the catalog, down to whatever level was asked for.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[non_exhaustive]
pub struct Entry {
    /// The stable id, such as `2019_honda_civic_si`.
    pub id: String,
    pub year: u16,
    pub make: String,
    pub model: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub submodel: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub engine: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub transmission: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub drive: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub body: Option<String>,
    /// vPIC's vehicle types for this model year, such as `Truck`.
    pub vehicle_types: Vec<String>,
    /// One line naming everything above that is known.
    pub summary: String,
}

pub(crate) struct ModelRow {
    pub id: i64,
    pub slug: String,
    pub name: String,
    pub year_from: u16,
    pub year_to: u16,
}

pub(crate) struct SubmodelRow {
    pub id: i64,
    pub name: String,
    pub kind: String,
    pub listed: bool,
    pub body: Option<String>,
    pub drive: Option<String>,
    pub transmission: Option<String>,
}

pub(crate) struct EngineRow {
    pub label: String,
    pub vin8: Option<String>,
    pub preset: bool,
}

/// One model year, with what all of its configurations share.
pub(crate) struct Vehicle<'a> {
    pub year: u16,
    pub make: &'a MakeRef,
    pub model: ModelRow,
    pub types: u32,
    pub detail_id: Option<i64>,
    pub body: Option<String>,
    pub drive: Option<String>,
    pub transmission: Option<String>,
}

/// Values a decode knows better than the catalog does.
#[derive(Default)]
pub(crate) struct Known<'a> {
    pub transmission: Option<&'a str>,
    pub drive: Option<&'a str>,
    pub body: Option<&'a str>,
}

pub(crate) fn integer(
    row: &[Value],
    column: usize,
    table: &'static str,
) -> Result<i64, CatalogError> {
    row.get(column)
        .and_then(Value::integer)
        .ok_or(CatalogError::Shape(table))
}

pub(crate) fn text(
    row: &[Value],
    column: usize,
    table: &'static str,
) -> Result<String, CatalogError> {
    row.get(column)
        .and_then(Value::text)
        .map(str::to_owned)
        .ok_or(CatalogError::Shape(table))
}

pub(crate) fn optional_text(row: &[Value], column: usize) -> Option<String> {
    row.get(column).and_then(Value::text).map(str::to_owned)
}

pub(crate) fn year(row: &[Value], column: usize, table: &'static str) -> Result<u16, CatalogError> {
    u16::try_from(integer(row, column, table)?).map_err(|_| CatalogError::Shape(table))
}

fn model_row(row: &[Value]) -> Result<ModelRow, CatalogError> {
    const TABLE: &str = "catalog_model";
    Ok(ModelRow {
        id: integer(row, 0, TABLE)?,
        slug: text(row, 1, TABLE)?,
        name: text(row, 2, TABLE)?,
        year_from: year(row, 3, TABLE)?,
        year_to: year(row, 4, TABLE)?,
    })
}

/// A cap on list sizes a caller cannot exceed.
const MOST: usize = 500;

/// The digits in a model year.
const YEAR_DIGITS: usize = 4;

fn limit(limit: usize) -> Value {
    Value::Integer(i64::try_from(limit.min(MOST)).unwrap_or(0))
}

/// The catalog of one data file.
#[derive(Debug)]
pub struct Catalog<S> {
    pub(crate) source: S,
    pub(crate) index: MakeIndex,
    types: Vec<(u32, String)>,
    pub(crate) years: (u16, u16),
}

impl<S: Source> Catalog<S> {
    /// Reads the makes, aliases, vehicle types and year range once.
    pub fn new(source: S) -> Result<Self, CatalogError> {
        let run = |sql: &str| source.query(sql, &[]).map_err(CatalogError::Source);

        let mut makes = Vec::new();
        for row in run(sql::MAKES)? {
            const TABLE: &str = "catalog_make";
            makes.push(MakeRef {
                id: integer(&row, 0, TABLE)?,
                slug: text(&row, 1, TABLE)?,
                name: text(&row, 2, TABLE)?,
                norm: text(&row, 3, TABLE)?,
                rank: row
                    .get(4)
                    .and_then(Value::integer)
                    .and_then(|rank| u32::try_from(rank).ok()),
                types: u32::try_from(integer(&row, 5, TABLE)?)
                    .map_err(|_| CatalogError::Shape(TABLE))?,
                light: integer(&row, 6, TABLE)? != 0,
            });
        }
        let mut aliases = Vec::new();
        for row in run(sql::ALIASES)? {
            const TABLE: &str = "catalog_alias";
            aliases.push((text(&row, 0, TABLE)?, integer(&row, 1, TABLE)?));
        }
        let mut types = Vec::new();
        for row in run(sql::TYPES)? {
            const TABLE: &str = "catalog_type";
            if let Ok(id) = u32::try_from(integer(&row, 0, TABLE)?) {
                types.push((id, text(&row, 1, TABLE)?));
            }
        }
        let years = match run(sql::YEAR_RANGE)?.first() {
            Some(row) => match (
                row.first().and_then(Value::integer),
                row.get(1).and_then(Value::integer),
            ) {
                (Some(first), Some(last)) => (
                    u16::try_from(first).unwrap_or(0),
                    u16::try_from(last).unwrap_or(0),
                ),
                _ => (0, 0),
            },
            None => (0, 0),
        };
        Ok(Catalog {
            index: MakeIndex::new(makes, &aliases),
            source,
            types,
            years,
        })
    }

    /// The makes, for anything that needs to match names without a query.
    pub fn index(&self) -> &MakeIndex {
        &self.index
    }

    /// The first and last model year. `(0, 0)` when the catalog is empty.
    pub fn year_range(&self) -> (u16, u16) {
        self.years
    }

    /// vPIC's vehicle types, by id.
    pub fn types(&self) -> &[(u32, String)] {
        &self.types
    }

    pub(crate) fn query(
        &self,
        sql: &str,
        params: &[Value],
    ) -> Result<Vec<Vec<Value>>, CatalogError> {
        self.source.query(sql, params).map_err(CatalogError::Source)
    }

    /// Model years, newest first. `term` is the leading digits typed so far.
    pub fn years(&self, scope: Scope, term: &str) -> Result<Vec<u16>, CatalogError> {
        let Some((light, bit)) = scope.bits() else {
            return Ok(Vec::new());
        };
        let term = term.trim();
        // A model year has four digits, so a longer term names none. It
        // must not reach the `LIKE`, which refuses a very long pattern.
        if term.len() > YEAR_DIGITS || !term.bytes().all(|byte| byte.is_ascii_digit()) {
            return Ok(Vec::new());
        }
        self.query(sql::YEARS, &[light.into(), bit.into(), term.into()])?
            .iter()
            .map(|row| year(row, 0, "catalog_vehicle"))
            .collect()
    }

    /// Makes, popular ones first and the rest by name. With a year, only
    /// makes that have a model that year.
    pub fn makes(
        &self,
        year: Option<u16>,
        scope: Scope,
        term: &str,
        limit: usize,
    ) -> Result<Vec<Make>, CatalogError> {
        let listed = self.index.list(scope, term);
        let in_year = match (year, scope.bits()) {
            (None, _) => None,
            (Some(_), None) => return Ok(Vec::new()),
            (Some(year), Some((light, bit))) => Some(
                self.query(
                    sql::MAKES_FOR_YEAR,
                    &[i64::from(year).into(), light.into(), bit.into()],
                )?
                .iter()
                .map(|row| integer(row, 0, "catalog_vehicle"))
                .collect::<Result<std::collections::HashSet<i64>, _>>()?,
            ),
        };
        Ok(listed
            .into_iter()
            .filter(|make| in_year.as_ref().is_none_or(|ids| ids.contains(&make.id)))
            .take(limit.min(MOST))
            .map(|make| Make {
                id: make.slug.clone(),
                name: make.name.clone(),
                popular: make.rank.is_some(),
            })
            .collect())
    }

    /// Models of a make, by name. `make` is a name, an alias or an id form.
    /// An unknown make has no models.
    pub fn models(
        &self,
        make: &str,
        year: Option<u16>,
        scope: Scope,
        prefix: &str,
        most: usize,
    ) -> Result<Vec<Model>, CatalogError> {
        let (Some(make), Some((light, bit)), Some(form)) =
            (self.index.resolve(make, scope), scope.bits(), term(prefix))
        else {
            return Ok(Vec::new());
        };
        let rows = match year {
            Some(year) => self.query(
                sql::MODELS_FOR_YEAR,
                &[
                    make.id.into(),
                    i64::from(year).into(),
                    light.into(),
                    bit.into(),
                    form.as_str().into(),
                    limit(most),
                ],
            )?,
            None => self.query(
                sql::MODELS_FOR_MAKE,
                &[
                    make.id.into(),
                    light.into(),
                    bit.into(),
                    form.as_str().into(),
                    limit(most),
                ],
            )?,
        };
        rows.iter()
            .map(|row| {
                let model = model_row(row)?;
                Ok(Model {
                    id: model.slug,
                    name: model.name,
                    year_from: model.year_from,
                    year_to: model.year_to,
                })
            })
            .collect()
    }

    /// The model year a person means by a make and a model, each given as a
    /// name, an alias or an id form.
    pub(crate) fn vehicle(
        &self,
        make: &str,
        model: &str,
        year: u16,
    ) -> Result<Option<Vehicle<'_>>, CatalogError> {
        let Some(make) = self.index.resolve(make, Scope::All) else {
            return Ok(None);
        };
        let rows = self.query(
            sql::MODEL,
            &[
                make.id.into(),
                model.trim().into(),
                slug(model).as_str().into(),
                normalize(model).as_str().into(),
            ],
        )?;
        match rows.first() {
            Some(row) => self.vehicle_of(make, model_row(row)?, year),
            None => Ok(None),
        }
    }

    pub(crate) fn vehicle_of<'a>(
        &self,
        make: &'a MakeRef,
        model: ModelRow,
        year: u16,
    ) -> Result<Option<Vehicle<'a>>, CatalogError> {
        const TABLE: &str = "catalog_vehicle";
        let rows = self.query(
            sql::VEHICLE,
            &[i64::from(year).into(), make.id.into(), model.id.into()],
        )?;
        let Some(row) = rows.first() else {
            return Ok(None);
        };
        Ok(Some(Vehicle {
            year,
            make,
            model,
            types: u32::try_from(integer(row, 0, TABLE)?)
                .map_err(|_| CatalogError::Shape(TABLE))?,
            detail_id: row.get(1).and_then(Value::integer),
            body: optional_text(row, 2),
            drive: optional_text(row, 3),
            transmission: optional_text(row, 4),
        }))
    }

    pub(crate) fn submodel_rows(
        &self,
        detail_id: Option<i64>,
    ) -> Result<Vec<SubmodelRow>, CatalogError> {
        const TABLE: &str = "catalog_submodel";
        let Some(detail_id) = detail_id else {
            return Ok(Vec::new());
        };
        self.query(sql::SUBMODELS, &[detail_id.into()])?
            .iter()
            .map(|row| {
                Ok(SubmodelRow {
                    id: integer(row, 0, TABLE)?,
                    name: text(row, 1, TABLE)?,
                    kind: text(row, 2, TABLE)?,
                    listed: integer(row, 3, TABLE)? != 0,
                    body: optional_text(row, 4),
                    drive: optional_text(row, 5),
                    transmission: optional_text(row, 6),
                })
            })
            .collect()
    }

    pub(crate) fn engine_rows(
        &self,
        detail_id: Option<i64>,
        submodel_id: Option<i64>,
    ) -> Result<Vec<EngineRow>, CatalogError> {
        const TABLE: &str = "catalog_engine";
        let Some(detail_id) = detail_id else {
            return Ok(Vec::new());
        };
        self.query(sql::ENGINES, &[detail_id.into(), submodel_id.into()])?
            .iter()
            .map(|row| {
                Ok(EngineRow {
                    label: text(row, 1, TABLE)?,
                    vin8: optional_text(row, 2),
                    preset: text(row, 3, TABLE)? == "preset",
                })
            })
            .collect()
    }

    /// The submodels offered for a model year, by name. Trims and presets,
    /// or series when there are neither.
    pub fn submodels(
        &self,
        make: &str,
        model: &str,
        year: u16,
        prefix: &str,
    ) -> Result<Vec<Submodel>, CatalogError> {
        let (Some(vehicle), Some(form)) = (self.vehicle(make, model, year)?, term(prefix)) else {
            return Ok(Vec::new());
        };
        let mut submodels: Vec<Submodel> = self
            .submodel_rows(vehicle.detail_id)?
            .into_iter()
            .filter(|row| row.listed && normalize(&row.name).starts_with(&form))
            .map(|row| Submodel {
                id: slug(&row.name),
                name: row.name,
                kind: row.kind,
            })
            .collect();
        submodels.sort_by_key(|submodel| (submodel.name.to_lowercase(), submodel.name.clone()));
        Ok(submodels)
    }

    /// The engines of a model year, by label. With a submodel that comes
    /// with only some of them, just those. `submodel` is a name or id form;
    /// one the model year does not have gives no engines.
    pub fn engines(
        &self,
        make: &str,
        model: &str,
        year: u16,
        submodel: Option<&str>,
        prefix: &str,
    ) -> Result<Vec<EngineOption>, CatalogError> {
        let Some(vehicle) = self.vehicle(make, model, year)? else {
            return Ok(Vec::new());
        };
        let submodel_id = match submodel {
            None => None,
            Some(wanted) => {
                let wanted = slug(wanted);
                match self
                    .submodel_rows(vehicle.detail_id)?
                    .into_iter()
                    .find(|row| slug(&row.name) == wanted)
                {
                    Some(row) => Some(row.id),
                    None => return Ok(Vec::new()),
                }
            }
        };
        // Engines are matched as typed, so `3.5` finds `3.5L Turbo V6`.
        let typed = prefix.trim().to_lowercase();
        Ok(self
            .engine_rows(vehicle.detail_id, submodel_id)?
            .into_iter()
            .filter(|row| row.label.to_lowercase().starts_with(&typed))
            .map(|row| EngineOption {
                id: slug(&row.label),
                label: row.label,
                vin8: row.vin8,
                preset: row.preset,
            })
            .collect())
    }

    /// The names of the vehicle types in a mask, in id order.
    fn type_names(&self, types: u32) -> Vec<String> {
        self.types
            .iter()
            .filter(|(id, _)| 1u32.checked_shl(*id).is_some_and(|bit| types & bit != 0))
            .map(|(_, name)| name.clone())
            .collect()
    }

    /// Builds the entry for a model year and, optionally, one of its
    /// submodels and engines. Body, drive and transmission come from
    /// `known` first, then the submodel, then the model year.
    pub(crate) fn entry_of(
        &self,
        vehicle: &Vehicle<'_>,
        submodel: Option<&SubmodelRow>,
        engine: Option<&str>,
        known: &Known<'_>,
    ) -> Entry {
        let pick = |known: Option<&str>,
                    own: fn(&SubmodelRow) -> &Option<String>,
                    shared: &Option<String>| {
            known
                .map(str::to_owned)
                .or_else(|| submodel.and_then(|row| own(row).clone()))
                .or_else(|| shared.clone())
        };
        let transmission = pick(
            known.transmission,
            |row| &row.transmission,
            &vehicle.transmission,
        );
        let drive = pick(known.drive, |row| &row.drive, &vehicle.drive);
        let body = pick(known.body, |row| &row.body, &vehicle.body);
        let id = VehicleId {
            year: vehicle.year,
            make: vehicle.make.slug.clone(),
            model: vehicle.model.slug.clone(),
            submodel: submodel.map(|row| slug(&row.name)),
            engine: engine.map(slug),
        };
        let summary = summary::line(&Parts {
            year: vehicle.year,
            make: &vehicle.make.name,
            model: &vehicle.model.name,
            submodel: submodel.map(|row| row.name.as_str()),
            engine,
            transmission: transmission.as_deref(),
            drive: drive.as_deref(),
            body: body.as_deref(),
        });
        Entry {
            id: id.to_string(),
            year: vehicle.year,
            make: vehicle.make.name.clone(),
            model: vehicle.model.name.clone(),
            submodel: submodel.map(|row| row.name.clone()),
            engine: engine.map(str::to_owned),
            transmission,
            drive,
            body,
            vehicle_types: self.type_names(vehicle.types),
            summary,
        }
    }

    /// The entry a stable id names. `None` when the text is not an id or
    /// names nothing in this data file.
    pub fn entry(&self, id: &str) -> Result<Option<Entry>, CatalogError> {
        let Some(id) = VehicleId::parse(id) else {
            return Ok(None);
        };
        let Some(make) = self.index.by_slug(&id.make) else {
            return Ok(None);
        };
        let rows = self.query(
            sql::MODEL_BY_SLUG,
            &[make.id.into(), id.model.as_str().into()],
        )?;
        let Some(row) = rows.first() else {
            return Ok(None);
        };
        let Some(vehicle) = self.vehicle_of(make, model_row(row)?, id.year)? else {
            return Ok(None);
        };
        let submodel = match &id.submodel {
            None => None,
            Some(wanted) => {
                match self
                    .submodel_rows(vehicle.detail_id)?
                    .into_iter()
                    .find(|row| &slug(&row.name) == wanted)
                {
                    Some(row) => Some(row),
                    None => return Ok(None),
                }
            }
        };
        let engine = match &id.engine {
            None => None,
            Some(wanted) => {
                match self
                    .engine_rows(vehicle.detail_id, submodel.as_ref().map(|row| row.id))?
                    .into_iter()
                    .find(|row| &slug(&row.label) == wanted)
                {
                    Some(row) => Some(row.label),
                    None => return Ok(None),
                }
            }
        };
        Ok(Some(self.entry_of(
            &vehicle,
            submodel.as_ref(),
            engine.as_deref(),
            &Known::default(),
        )))
    }
}
