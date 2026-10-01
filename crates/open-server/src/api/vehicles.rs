//! The vehicle catalog: the steps of a vehicle form, free-text search, and
//! one entry by its id.
//!
//! Each route is a thin wrapper around a function ending in `_op`, which the
//! MCP endpoint calls too, so both give the same answers.

use axum::Json;
use axum::extract::rejection::QueryRejection;
use axum::extract::{Path, Query, State};
use serde::Deserialize;
use serde_json::json;
use utoipa::IntoParams;
use wenmar_vehicles::Scope;

use crate::api::types::{EngineOption, Entry, Make, Model, Submodel};
use crate::api::{bad_query, blank_is_none};
use crate::error::{ApiError, ErrorBody};
use crate::search_index;
use crate::state::AppState;

/// Most items in a list, whatever `limit` says.
pub const MOST: usize = 500;
/// Search results when `limit` is not given, and the most it may be.
pub const SEARCH_DEFAULT: usize = 10;
pub const SEARCH_MOST: usize = 50;
/// Longest search text read. The rest is ignored.
pub const LONGEST_QUERY: usize = 200;

fn scope(text: Option<&str>) -> Result<Scope, ApiError> {
    match text.map(str::trim).filter(|text| !text.is_empty()) {
        None => Ok(Scope::Light),
        Some(text) => Scope::parse(text).ok_or_else(|| ApiError::Validation {
            message: "scope must be light, all, or a vehicle type id".to_owned(),
            details: json!({ "field": "scope" }),
        }),
    }
}

fn required(field: &'static str, value: Option<String>) -> Result<String, ApiError> {
    value
        .filter(|value| !value.trim().is_empty())
        .ok_or_else(|| ApiError::Validation {
            message: format!("{field} is required"),
            details: json!({ "field": field }),
        })
}

fn required_year(year: Option<u16>) -> Result<u16, ApiError> {
    year.ok_or_else(|| ApiError::Validation {
        message: "year is required".to_owned(),
        details: json!({ "field": "year" }),
    })
}

fn limit(given: Option<usize>, default: usize, most: usize) -> usize {
    given.unwrap_or(default).clamp(1, most)
}

#[derive(Debug, Default, Deserialize, IntoParams)]
#[into_params(parameter_in = Query)]
pub struct YearsQuery {
    /// `light` (cars, MPVs and trucks; the default), `all`, or a vPIC
    /// vehicle type id such as `6` for trailers.
    pub scope: Option<String>,
    /// Only years that start with this, such as `201`.
    pub term: Option<String>,
}

#[derive(Debug, Default, Deserialize, IntoParams)]
#[into_params(parameter_in = Query)]
pub struct MakesQuery {
    /// Only makes with a model in this model year.
    #[serde(default, deserialize_with = "blank_is_none")]
    #[param(value_type = Option<u16>, example = 2019)]
    pub year: Option<u16>,
    /// `light` (the default), `all`, or a vPIC vehicle type id.
    pub scope: Option<String>,
    /// Only makes whose name or alias starts with this. Case and punctuation
    /// are ignored.
    pub term: Option<String>,
    /// At most this many, up to 500.
    #[serde(default, deserialize_with = "blank_is_none")]
    #[param(value_type = Option<usize>)]
    pub limit: Option<usize>,
}

#[derive(Debug, Default, Deserialize, IntoParams)]
#[into_params(parameter_in = Query)]
pub struct ModelsQuery {
    /// A make's name, alias or id: `Chevrolet`, `chevy` or `chevrolet`.
    #[param(required = true, example = "honda")]
    pub make: Option<String>,
    /// Only models of this model year.
    #[serde(default, deserialize_with = "blank_is_none")]
    #[param(value_type = Option<u16>, example = 2019)]
    pub year: Option<u16>,
    /// `light` (the default), `all`, or a vPIC vehicle type id.
    pub scope: Option<String>,
    /// Only models whose name starts with this.
    pub term: Option<String>,
    /// At most this many, up to 500.
    #[serde(default, deserialize_with = "blank_is_none")]
    #[param(value_type = Option<usize>)]
    pub limit: Option<usize>,
}

#[derive(Debug, Default, Deserialize, IntoParams)]
#[into_params(parameter_in = Query)]
pub struct SubmodelsQuery {
    #[param(required = true, example = "honda")]
    pub make: Option<String>,
    /// A model's name or id: `F-150`, `f150` or `f-150`.
    #[param(required = true, example = "civic")]
    pub model: Option<String>,
    #[serde(default, deserialize_with = "blank_is_none")]
    #[param(value_type = u16, required = true, example = 2019)]
    pub year: Option<u16>,
    /// Only submodels whose name starts with this.
    pub term: Option<String>,
}

#[derive(Debug, Default, Deserialize, IntoParams)]
#[into_params(parameter_in = Query)]
pub struct EnginesQuery {
    #[param(required = true, example = "honda")]
    pub make: Option<String>,
    #[param(required = true, example = "civic")]
    pub model: Option<String>,
    #[serde(default, deserialize_with = "blank_is_none")]
    #[param(value_type = u16, required = true, example = 2019)]
    pub year: Option<u16>,
    /// Only engines this submodel comes with.
    #[param(example = "si")]
    pub submodel: Option<String>,
    /// Only engines whose label starts with this.
    pub term: Option<String>,
}

#[derive(Debug, Default, Deserialize, IntoParams)]
#[into_params(parameter_in = Query)]
pub struct SearchQuery {
    /// What someone would type: `2019 civic si`, `chevy 1500`, `f150`.
    #[param(required = true, example = "2019 civic si")]
    pub q: Option<String>,
    /// `light` (the default), `all`, or a vPIC vehicle type id.
    pub scope: Option<String>,
    /// At most this many, up to 50. The default is 10.
    #[serde(default, deserialize_with = "blank_is_none")]
    #[param(value_type = Option<usize>)]
    pub limit: Option<usize>,
}

pub async fn years_op(state: &AppState, query: YearsQuery) -> Result<Vec<u16>, ApiError> {
    let scope = scope(query.scope.as_deref())?;
    let term = query.term.unwrap_or_default();
    // The whole list is kept in memory. Only a narrowed one is read.
    if term.trim().is_empty()
        && let Some(years) = state.years(scope)
    {
        return Ok(years.to_vec());
    }
    let mut years = state
        .db()
        .run(move |worker| worker.catalog.years(scope, &term))
        .await??;
    years.sort_unstable_by(|left, right| right.cmp(left));
    Ok(years)
}

pub async fn makes_op(state: &AppState, query: MakesQuery) -> Result<Vec<Make>, ApiError> {
    let scope = scope(query.scope.as_deref())?;
    let term = query.term.unwrap_or_default();
    let limit = limit(query.limit, MOST, MOST);
    let makes = state
        .db()
        .run(move |worker| worker.catalog.makes(query.year, scope, &term, limit))
        .await??;
    Ok(makes.into_iter().map(Make::from).collect())
}

pub async fn models_op(state: &AppState, query: ModelsQuery) -> Result<Vec<Model>, ApiError> {
    let make = required("make", query.make)?;
    let scope = scope(query.scope.as_deref())?;
    let term = query.term.unwrap_or_default();
    let limit = limit(query.limit, MOST, MOST);
    let models = state
        .db()
        .run(move |worker| {
            worker
                .catalog
                .models(&make, query.year, scope, &term, limit)
        })
        .await??;
    Ok(models.into_iter().map(Model::from).collect())
}

pub async fn submodels_op(
    state: &AppState,
    query: SubmodelsQuery,
) -> Result<Vec<Submodel>, ApiError> {
    let make = required("make", query.make)?;
    let model = required("model", query.model)?;
    let year = required_year(query.year)?;
    let term = query.term.unwrap_or_default();
    let submodels = state
        .db()
        .run(move |worker| worker.catalog.submodels(&make, &model, year, &term))
        .await??;
    Ok(submodels.into_iter().map(Submodel::from).collect())
}

pub async fn engines_op(
    state: &AppState,
    query: EnginesQuery,
) -> Result<Vec<EngineOption>, ApiError> {
    let make = required("make", query.make)?;
    let model = required("model", query.model)?;
    let year = required_year(query.year)?;
    let submodel = query.submodel.filter(|name| !name.trim().is_empty());
    let term = query.term.unwrap_or_default();
    let engines = state
        .db()
        .run(move |worker| {
            worker
                .catalog
                .engines(&make, &model, year, submodel.as_deref(), &term)
        })
        .await??;
    Ok(engines.into_iter().map(EngineOption::from).collect())
}

pub async fn search_op(state: &AppState, query: SearchQuery) -> Result<Vec<Entry>, ApiError> {
    let text: String = required("q", query.q)?
        .chars()
        .take(LONGEST_QUERY)
        .collect();
    let scope = scope(query.scope.as_deref())?;
    let limit = limit(query.limit, SEARCH_DEFAULT, SEARCH_MOST);

    // First the catalog's own reading: a year, a make, a model, a submodel.
    let typed = text.clone();
    let entries = state
        .db()
        .run(move |worker| worker.catalog.search(&typed, scope, limit))
        .await??;
    if !entries.is_empty() {
        return Ok(entries.into_iter().map(Entry::from).collect());
    }

    // Then the full-text index, which finds a model from its words in any
    // order. It knows light vehicles from the rest, not single types.
    let light_only = match scope {
        Scope::Light => true,
        Scope::All => false,
        Scope::Type(_) => return Ok(Vec::new()),
    };
    let Some(index) = state.search() else {
        return Ok(Vec::new());
    };
    let found = match index.find(&text, light_only, limit).await {
        Ok(found) => found,
        Err(error) => {
            tracing::warn!(%error, "search index failed");
            return Ok(Vec::new());
        }
    };
    if found.is_empty() {
        return Ok(Vec::new());
    }
    let entries = state
        .db()
        .run(move |worker| {
            let (first, last) = worker.catalog.year_range();
            let typed_year = search_index::words(&text)
                .iter()
                .filter(|word| word.len() == 4)
                .filter_map(|word| word.parse::<u16>().ok())
                .find(|year| (first..=last).contains(year));
            let mut entries = Vec::new();
            for model in found {
                let year = match typed_year {
                    Some(year) if (model.year_from..=model.year_to).contains(&year) => year,
                    Some(_) => continue,
                    None => model.year_to,
                };
                let id = format!("{year}_{}_{}", model.make, model.model);
                if let Some(entry) = worker.catalog.entry(&id)? {
                    entries.push(Entry::from(entry));
                }
            }
            Ok::<_, wenmar_vehicles::CatalogError>(entries)
        })
        .await??;
    Ok(entries)
}

pub async fn entry_op(state: &AppState, id: String) -> Result<Entry, ApiError> {
    let found = state
        .db()
        .run(move |worker| worker.catalog.entry(&id))
        .await??;
    found
        .map(Entry::from)
        .ok_or_else(|| ApiError::NotFound("No vehicle has that id.".to_owned()))
}

/// Model years, newest first.
#[utoipa::path(
    get,
    path = "/vehicles/years",
    tag = "Vehicles",
    params(YearsQuery),
    responses(
        (status = 200, description = "Model years, newest first.", body = Vec<u16>, example = json!([2027, 2026, 2025])),
        (status = 400, description = "`validation_failed`.", body = ErrorBody)
    )
)]
pub async fn years(
    State(state): State<AppState>,
    query: Result<Query<YearsQuery>, QueryRejection>,
) -> Result<Json<Vec<u16>>, ApiError> {
    let Query(query) = query.map_err(bad_query)?;
    Ok(Json(years_op(&state, query).await?))
}

/// Makes, popular ones first and the rest by name.
#[utoipa::path(
    get,
    path = "/vehicles/makes",
    tag = "Vehicles",
    params(MakesQuery),
    responses(
        (status = 200, description = "Makes.", body = Vec<Make>),
        (status = 400, description = "`validation_failed`.", body = ErrorBody)
    )
)]
pub async fn makes(
    State(state): State<AppState>,
    query: Result<Query<MakesQuery>, QueryRejection>,
) -> Result<Json<Vec<Make>>, ApiError> {
    let Query(query) = query.map_err(bad_query)?;
    Ok(Json(makes_op(&state, query).await?))
}

/// Models of a make, by name. An unknown make has no models.
#[utoipa::path(
    get,
    path = "/vehicles/models",
    tag = "Vehicles",
    params(ModelsQuery),
    responses(
        (status = 200, description = "Models.", body = Vec<Model>),
        (status = 400, description = "`validation_failed`: `make` is missing.", body = ErrorBody)
    )
)]
pub async fn models(
    State(state): State<AppState>,
    query: Result<Query<ModelsQuery>, QueryRejection>,
) -> Result<Json<Vec<Model>>, ApiError> {
    let Query(query) = query.map_err(bad_query)?;
    Ok(Json(models_op(&state, query).await?))
}

/// Submodels of a model year: trims where NHTSA has them, else series, else
/// this project's own list.
#[utoipa::path(
    get,
    path = "/vehicles/submodels",
    tag = "Vehicles",
    params(SubmodelsQuery),
    responses(
        (status = 200, description = "Submodels.", body = Vec<Submodel>),
        (status = 400, description = "`validation_failed`: `make`, `model` or `year` is missing.", body = ErrorBody)
    )
)]
pub async fn submodels(
    State(state): State<AppState>,
    query: Result<Query<SubmodelsQuery>, QueryRejection>,
) -> Result<Json<Vec<Submodel>>, ApiError> {
    let Query(query) = query.map_err(bad_query)?;
    Ok(Json(submodels_op(&state, query).await?))
}

/// The same as `/v1/vehicles/submodels`, under the name most people use.
#[utoipa::path(
    get,
    path = "/vehicles/trims",
    tag = "Vehicles",
    params(SubmodelsQuery),
    responses(
        (status = 200, description = "Submodels.", body = Vec<Submodel>),
        (status = 400, description = "`validation_failed`: `make`, `model` or `year` is missing.", body = ErrorBody)
    )
)]
pub async fn trims(
    State(state): State<AppState>,
    query: Result<Query<SubmodelsQuery>, QueryRejection>,
) -> Result<Json<Vec<Submodel>>, ApiError> {
    let Query(query) = query.map_err(bad_query)?;
    Ok(Json(submodels_op(&state, query).await?))
}

/// Engines of a model year, in short form, optionally for one submodel.
#[utoipa::path(
    get,
    path = "/vehicles/engines",
    tag = "Vehicles",
    params(EnginesQuery),
    responses(
        (status = 200, description = "Engines.", body = Vec<EngineOption>),
        (status = 400, description = "`validation_failed`: `make`, `model` or `year` is missing.", body = ErrorBody)
    )
)]
pub async fn engines(
    State(state): State<AppState>,
    query: Result<Query<EnginesQuery>, QueryRejection>,
) -> Result<Json<Vec<EngineOption>>, ApiError> {
    let Query(query) = query.map_err(bad_query)?;
    Ok(Json(engines_op(&state, query).await?))
}

/// Catalog entries for free text, best first. Text that names nothing gives
/// an empty list.
#[utoipa::path(
    get,
    path = "/vehicles/search",
    tag = "Vehicles",
    params(SearchQuery),
    responses(
        (status = 200, description = "Entries, best first.", body = Vec<Entry>),
        (status = 400, description = "`validation_failed`: `q` is missing.", body = ErrorBody)
    )
)]
pub async fn search(
    State(state): State<AppState>,
    query: Result<Query<SearchQuery>, QueryRejection>,
) -> Result<Json<Vec<Entry>>, ApiError> {
    let Query(query) = query.map_err(bad_query)?;
    Ok(Json(search_op(&state, query).await?))
}

/// One catalog entry by its stable id.
#[utoipa::path(
    get,
    path = "/vehicles/{id}",
    tag = "Vehicles",
    params(("id" = String, Path, description = "A vehicle id: year, make and model, then optionally submodel and engine, joined by underscores.", example = "2019_honda_civic_si")),
    responses(
        (status = 200, description = "The entry.", body = Entry),
        (status = 404, description = "`not_found`: no vehicle has that id.", body = ErrorBody)
    )
)]
pub async fn entry(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<Json<Entry>, ApiError> {
    Ok(Json(entry_op(&state, id).await?))
}
