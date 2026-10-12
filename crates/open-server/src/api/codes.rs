//! Trouble codes: one generic code as this project describes it, the list
//! of those described, and a code on a model year with the bulletins that
//! name it and the bulletins and recalls about the same components.
//!
//! A code number is a fact. The text is this project's own.
//!
//! Each route is a thin wrapper around a function ending in `_op`, as in
//! `recalls.rs`.

use axum::Json;
use axum::extract::rejection::{PathRejection, QueryRejection};
use axum::extract::{Path, Query, State};
use serde::Deserialize;
use serde_json::json;
use utoipa::IntoParams;
use wenmar_shopdata::bulletins::{BulletinQuery, MOST_FOR_CODE, code_form};
use wenmar_shopdata::codes::{
    self as shopdata, BAD_CURSOR, BAD_SYSTEM, CodeQuery, LIST_DEFAULT, MOST_CODES_LISTED,
    NOT_A_CODE, NOT_GENERIC, NOT_WRITTEN, NOTE, VEHICLE_NOTE, is_generic, system_name,
};
use wenmar_shopdata::{Dataset, ShopError};

use crate::api::bulletins as bulletin_api;
use crate::api::recalls::{self as recall_api, ModelYear, given, model_year};
use crate::api::types::{
    Code, CodeBulletins, CodeComponent, CodeLink, CodeList, CodeRecalls, CodeRelated, VehicleCode,
};
use crate::api::{bad_query, blank_is_none};
use crate::error::{ApiError, ErrorBody};
use crate::shop::ShopDb;
use crate::state::AppState;

/// The address of a code's page on this site.
pub fn code_url(base: &str, code: &str) -> String {
    format!("{base}/codes/{code}")
}

/// The address of a code's page for one model year.
pub fn vehicle_code_url(base: &str, found: &ModelYear, code: &str) -> String {
    format!(
        "{base}/makes/{}/{}/{}/codes/{code}",
        found.make_slug, found.model_slug, found.vehicle.year
    )
}

/// The credit line of the codes dataset, when its licence asks for one.
pub fn attribution(shop: &ShopDb) -> Vec<String> {
    shop.datasets()
        .get(Dataset::Codes)
        .map(|entry| entry.attribution.clone())
        .filter(|credit| !credit.is_empty())
        .into_iter()
        .collect()
}

fn link(base: &str, code: shopdata::CodeSummary) -> CodeLink {
    CodeLink {
        url: code_url(base, &code.code),
        code: code.code,
        system: code.system,
        title: code.title,
    }
}

fn component(found: shopdata::CodeComponent) -> CodeComponent {
    CodeComponent {
        id: found.id,
        name: found.name,
        parent: found.parent,
    }
}

/// A record as the API and the pages give it.
pub fn described(base: &str, record: shopdata::CodeRecord, attribution: Vec<String>) -> Code {
    Code {
        url: code_url(base, &record.code),
        generic: true,
        system_name: system_name(&record.system)
            .unwrap_or(record.system.as_str())
            .to_owned(),
        code: record.code,
        system: record.system,
        title: record.title,
        meaning: record.meaning,
        common_causes: record.common_causes,
        checks: record.checks,
        components: record.components.into_iter().map(component).collect(),
        related: record
            .related
            .into_iter()
            .map(|other| link(base, other))
            .collect(),
        reviewer: record.reviewer,
        checked: record.checked,
        note: NOTE.to_owned(),
        attribution,
    }
}

fn invalid(field: &str, message: &str) -> ApiError {
    ApiError::Validation {
        message: message.to_owned(),
        details: json!({ "field": field }),
    }
}

pub async fn code_op(state: &AppState, code: String) -> Result<Code, ApiError> {
    let shop = state.dataset(Dataset::Codes)?;
    let Some(code) = code_form(&code) else {
        return Err(ApiError::NotFound(NOT_A_CODE.to_owned()));
    };
    if !is_generic(&code) {
        return Err(ApiError::NotFound(NOT_GENERIC.to_owned()));
    }
    let record = shop.run(move |reader| reader.code(&code)).await??;
    let Some(record) = record else {
        return Err(ApiError::NotFound(NOT_WRITTEN.to_owned()));
    };
    Ok(described(
        &state.config().base_url,
        record,
        attribution(shop),
    ))
}

/// What the list of codes may be narrowed by.
#[derive(Debug, Default, Deserialize, IntoParams)]
#[into_params(parameter_in = Query)]
pub struct CodeSearchQuery {
    /// Only codes with this text in the code, the title or the meaning,
    /// in either case. At most 100 characters are read.
    #[param(example = "P04")]
    pub q: Option<String>,
    /// Only this system: `powertrain`, `body`, `chassis` or `network`, or
    /// an area of the powertrain such as `powertrain/emissions`. A group
    /// holds its areas.
    #[param(example = "powertrain/emissions")]
    pub system: Option<String>,
    /// At most this many, up to 200. The default is 50.
    #[serde(default, deserialize_with = "blank_is_none")]
    #[param(value_type = Option<usize>)]
    pub limit: Option<usize>,
    /// `next_cursor` from the page before, unchanged.
    pub cursor: Option<String>,
}

pub async fn code_search_op(
    state: &AppState,
    query: CodeSearchQuery,
) -> Result<CodeList, ApiError> {
    let shop = state.dataset(Dataset::Codes)?;
    let system = match given(query.system) {
        None => None,
        Some(text) => {
            let text = text.to_ascii_lowercase();
            if system_name(&text).is_none() {
                return Err(invalid("system", BAD_SYSTEM));
            }
            Some(text)
        }
    };
    let after = match given(query.cursor) {
        None => None,
        Some(text) => Some(code_form(&text).ok_or_else(|| invalid("cursor", BAD_CURSOR))?),
    };
    let asked = CodeQuery {
        q: given(query.q),
        system,
        limit: query
            .limit
            .unwrap_or(LIST_DEFAULT)
            .clamp(1, MOST_CODES_LISTED),
        after,
    };
    let page = shop.run(move |reader| reader.codes(&asked)).await??;
    let base = &state.config().base_url;
    Ok(CodeList {
        as_of: shop
            .datasets()
            .get(Dataset::Codes)
            .map(|entry| entry.as_of.clone()),
        codes: page
            .codes
            .into_iter()
            .map(|code| link(base, code))
            .collect(),
        next_cursor: page.next,
        note: NOTE.to_owned(),
        attribution: attribution(shop),
    })
}

/// A code on one model year, as the API and the pages give it. `shop`
/// holds the codes, the bulletins, or both.
pub async fn vehicle_code_of(
    state: &AppState,
    shop: &ShopDb,
    found: &ModelYear,
    code: String,
) -> Result<VehicleCode, ApiError> {
    let datasets = shop.datasets();
    let codes = datasets.contains(Dataset::Codes);
    let bulletins = datasets.contains(Dataset::Bulletins);
    let recalls = datasets.contains(Dataset::RecallsUs) || datasets.contains(Dataset::RecallsCa);
    let generic = is_generic(&code);
    let (make_id, model_id, year) = (found.make_id, found.model_id, found.vehicle.year);
    let asked = code.clone();
    let (record, naming, about, related) = shop
        .run(move |reader| {
            let record = if codes && generic {
                reader.code(&asked)?
            } else {
                None
            };
            let components: Vec<String> = record
                .iter()
                .flat_map(|record| record.components.iter().map(|part| part.id.clone()))
                .collect();
            let (naming, about) = if bulletins {
                let count = reader.bulletins_naming_code_count(model_id, year, &asked)?;
                let page = reader.bulletins(&BulletinQuery {
                    model_id: Some(model_id),
                    year: Some(year),
                    code: Some(asked.clone()),
                    limit: MOST_FOR_CODE,
                    ..BulletinQuery::default()
                })?;
                let about = reader.bulletins_about_for_vehicle(
                    model_id,
                    year,
                    &components,
                    &asked,
                    MOST_FOR_CODE,
                )?;
                (Some((count, page.bulletins)), Some(about))
            } else {
                (None, None)
            };
            let related = if recalls {
                let mut found =
                    reader.recalls_about_for_vehicle(make_id, model_id, year, &components)?;
                found.truncate(MOST_FOR_CODE);
                Some(found)
            } else {
                None
            };
            Ok::<_, ShopError>((record, naming, about, related))
        })
        .await??;
    let base = &state.config().base_url;
    let mut credit = attribution(shop);
    if related.is_some() {
        credit.extend(recall_api::attribution(shop, None));
    }
    let description = record.map(|record| described(base, record, attribution(shop)));
    Ok(VehicleCode {
        vehicle: found.vehicle.clone(),
        generic,
        bulletins: naming.map(|(count, list)| CodeBulletins {
            as_of: bulletin_api::as_of(shop),
            count,
            bulletins: list
                .into_iter()
                .map(|bulletin| bulletin_api::listed(base, bulletin))
                .collect(),
            url: format!(
                "{base}/makes/{}/{}/{}/bulletins?code={code}",
                found.make_slug, found.model_slug, found.vehicle.year
            ),
        }),
        related: CodeRelated {
            components: description
                .as_ref()
                .map(|description| description.components.clone())
                .unwrap_or_default(),
            bulletins: about.map(|list| {
                list.into_iter()
                    .map(|bulletin| bulletin_api::listed(base, bulletin))
                    .collect()
            }),
            recalls: related.map(|list| CodeRecalls {
                may_apply: true,
                recalls: list
                    .into_iter()
                    .map(|recall| recall_api::for_vehicle(base, recall))
                    .collect(),
                lookup: recall_api::lookup(Some(&found.vehicle.make)),
            }),
        },
        description,
        url: vehicle_code_url(base, found, &code),
        code,
        note: VEHICLE_NOTE.to_owned(),
        attribution: credit,
    })
}

/// The shop file, when it can say anything about a code on a vehicle: it
/// holds the codes or the bulletins.
pub fn vehicle_code_shop(state: &AppState) -> Result<&ShopDb, ApiError> {
    state
        .dataset(Dataset::Codes)
        .or_else(|_| state.dataset(Dataset::Bulletins))
        .map_err(|_| ApiError::DatasetUnavailable(Dataset::Codes))
}

pub async fn vehicle_code_op(
    state: &AppState,
    id: String,
    code: String,
) -> Result<VehicleCode, ApiError> {
    let shop = vehicle_code_shop(state)?;
    let Some(code) = code_form(&code) else {
        return Err(ApiError::NotFound(NOT_A_CODE.to_owned()));
    };
    let found = state
        .db()
        .run(move |worker| model_year(worker, &id))
        .await??;
    let Some(found) = found else {
        return Err(ApiError::NotFound("No vehicle has that id.".to_owned()));
    };
    vehicle_code_of(state, shop, &found, code).await
}

/// One generic trouble code, described in this project's own words.
///
/// Only codes every maker uses the same way are described: `P0`, `P2`,
/// `P3400` and up, `B0`, `C0`, `U0` and `U3`. The text is general and is
/// not about one make.
#[utoipa::path(
    get,
    path = "/codes/{code}",
    tag = "Codes",
    params(("code" = String, Path, description = "A trouble code, in either case. A longer form such as `P0420:00` is cut to its first five characters.", example = "P0420")),
    responses(
        (status = 200, description = "The code's description.", body = Code),
        (status = 404, description = "`not_found`: the text is not a code, the code is specific to a manufacturer, or nobody has described it yet; the message says which. `dataset_unavailable`: this server holds no code descriptions.", body = ErrorBody)
    )
)]
pub async fn code(
    State(state): State<AppState>,
    path: Result<Path<String>, PathRejection>,
) -> Result<Json<Code>, ApiError> {
    let Ok(Path(code)) = path else {
        return Err(ApiError::NotFound(NOT_A_CODE.to_owned()));
    };
    Ok(Json(code_op(&state, code).await?))
}

/// The trouble codes that are described, by code.
#[utoipa::path(
    get,
    path = "/codes",
    tag = "Codes",
    params(CodeSearchQuery),
    responses(
        (status = 200, description = "One page of codes.", body = CodeList),
        (status = 400, description = "`validation_failed`: `system` or `cursor` is not one this API knows; `details.field` names it.", body = ErrorBody),
        (status = 404, description = "`dataset_unavailable`: this server holds no code descriptions.", body = ErrorBody)
    )
)]
pub async fn code_search(
    State(state): State<AppState>,
    query: Result<Query<CodeSearchQuery>, QueryRejection>,
) -> Result<Json<CodeList>, ApiError> {
    let Query(query) = query.map_err(bad_query)?;
    Ok(Json(code_search_op(&state, query).await?))
}

/// A trouble code on one model year: its description, the bulletins of
/// that model year whose summary names it, and the bulletins and recalls
/// of that model year about the components the description names.
///
/// Any code answers, a manufacturer-specific one included: such a code has
/// `description: null` and whatever bulletins name it. `bulletins` and
/// `related.bulletins` are `null` where bulletins are not held, and
/// `related.recalls` is `null` where recalls are not. Nothing here says a
/// recall applies to one vehicle, or what is wrong with it.
#[utoipa::path(
    get,
    path = "/vehicles/{id}/codes/{code}",
    tag = "Codes",
    params(
        ("id" = String, Path, description = "A vehicle id. A submodel or an engine in the id is set aside.", example = "2018_ford_f-150"),
        ("code" = String, Path, description = "A trouble code, in either case.", example = "P0420")
    ),
    responses(
        (status = 200, description = "What is held about the code for the model year.", body = VehicleCode),
        (status = 404, description = "`not_found`: no vehicle has that id, or the text is not a code. `dataset_unavailable`: this server holds neither code descriptions nor bulletins.", body = ErrorBody)
    )
)]
pub async fn vehicle_code(
    State(state): State<AppState>,
    path: Result<Path<(String, String)>, PathRejection>,
) -> Result<Json<VehicleCode>, ApiError> {
    let Ok(Path((id, code))) = path else {
        return Err(ApiError::NotFound("No vehicle has that id.".to_owned()));
    };
    Ok(Json(vehicle_code_op(&state, id, code).await?))
}
