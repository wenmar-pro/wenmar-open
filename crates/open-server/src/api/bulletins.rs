//! Manufacturer communications NHTSA publishes: the bulletins that list a
//! model year, one bulletin, and the list of them all.
//!
//! An answer holds NHTSA's index of a communication and NHTSA's summary.
//! The document is the manufacturer's: it is linked at NHTSA and never
//! held or quoted here.
//!
//! Each route is a thin wrapper around a function ending in `_op`, as in
//! `recalls.rs`.

use axum::Json;
use axum::extract::rejection::{PathRejection, QueryRejection};
use axum::extract::{Path, Query, State};
use serde::Deserialize;
use serde_json::json;
use utoipa::IntoParams;
use wenmar_shopdata::Dataset;
use wenmar_shopdata::bulletins::{
    self as shopdata, BulletinCursor, BulletinQuery, BulletinType, MOST_BULLETINS, code_form,
    document_urls,
};
use wenmar_vehicles::text::is_slug;

use crate::api::recalls::{ModelYear, given, model_slugs, model_year, named};
use crate::api::types::{
    Bulletin, BulletinComponent, BulletinCount, BulletinCounts, BulletinDetail, BulletinDocument,
    BulletinList, BulletinVehicle, RecallDecode, VehicleBulletins, VinBulletins, Warning,
};
use crate::api::vin::decode_many;
use crate::api::{bad_query, blank_is_none};
use crate::error::{ApiError, ErrorBody};
use crate::shop::ShopDb;
use crate::state::AppState;

/// What every page and answer about bulletins says.
pub const NOTE: &str = "A bulletin is a manufacturer's communication to its dealers, as NHTSA indexes it. The summary is NHTSA's. The document is the manufacturer's: read it at NHTSA. A bulletin is not a recall, and listing a model year does not mean every vehicle of it is concerned.";

/// What a VIN's bulletin answer says when the VIN names no model year.
pub const NO_CATALOG_VEHICLE: &str = "This VIN does not decode to a year, make and model in the catalog, so no bulletins are listed for it.";

/// Bulletins in a list when `limit` is not given.
pub const LIST_DEFAULT: usize = 25;

/// The address of a bulletin's page on this site.
pub fn bulletin_url(base: &str, nhtsa_id: i64) -> String {
    format!("{base}/bulletins/{nhtsa_id}")
}

/// The day the bulletins were current.
pub fn as_of(shop: &ShopDb) -> Option<String> {
    shop.datasets()
        .get(Dataset::Bulletins)
        .map(|entry| entry.as_of.clone())
}

/// The credit line of the bulletin dataset, when its licence asks for one.
pub fn attribution(shop: &ShopDb) -> Vec<String> {
    shop.datasets()
        .get(Dataset::Bulletins)
        .map(|entry| entry.attribution.clone())
        .filter(|credit| !credit.is_empty())
        .into_iter()
        .collect()
}

fn document(nhtsa_id: i64, date: Option<&str>, date_added: &str) -> Option<BulletinDocument> {
    document_urls(nhtsa_id, date, date_added).map(|(url, alternate_url)| BulletinDocument {
        url,
        alternate_url,
        confirmed: false,
    })
}

/// The name of a type id, or the id itself for one this build does not
/// know.
pub fn type_name(id: &str) -> String {
    BulletinType::parse(id).map_or_else(|| id.to_owned(), |kind| kind.label().to_owned())
}

pub fn listed(base: &str, bulletin: shopdata::BulletinSummary) -> Bulletin {
    Bulletin {
        url: bulletin_url(base, bulletin.nhtsa_id),
        document: document(
            bulletin.nhtsa_id,
            bulletin.date.as_deref(),
            &bulletin.date_added,
        ),
        nhtsa_id: bulletin.nhtsa_id,
        type_name: type_name(&bulletin.kind),
        kind: bulletin.kind,
        date_added: bulletin.date_added,
        date: bulletin.date,
        document_id: bulletin.document_id,
        mfr_campaign: bulletin.mfr_campaign,
        component: bulletin.component,
        summary: bulletin.summary,
    }
}

fn counted(counts: Vec<shopdata::BulletinCount>) -> Vec<BulletinCount> {
    counts
        .into_iter()
        .map(|entry| BulletinCount {
            id: entry.id,
            name: entry.name,
            count: entry.count,
        })
        .collect()
}

pub fn counts(found: shopdata::BulletinCounts) -> BulletinCounts {
    BulletinCounts {
        count: found.count,
        by_type: counted(found.by_type),
        components: counted(found.components),
        codes: counted(found.codes),
        topics: counted(found.topics),
    }
}

/// What a list of bulletins may be narrowed by, as a caller writes it.
#[derive(Debug, Clone, Default, Deserialize, IntoParams)]
#[into_params(parameter_in = Query)]
pub struct BulletinFilter {
    /// Only this type: `service_bulletin`, `service_campaign`,
    /// `warranty_extension`, `over_the_air`, `emissions` or `other`.
    #[serde(rename = "type")]
    #[param(example = "warranty_extension")]
    pub kind: Option<String>,
    /// Only bulletins about this component: an id from `counts.components`,
    /// such as `brakes` or `brake-pad`.
    #[param(example = "brakes")]
    pub component: Option<String>,
    /// Only bulletins whose summary names this trouble code. Letters may
    /// be in either case.
    #[param(example = "P0420")]
    pub code: Option<String>,
    /// Only bulletins with this text in them, in either case: in the
    /// summary, the manufacturer's document number or campaign id, or the
    /// component. At most 100 characters are read.
    #[param(example = "noise")]
    pub q: Option<String>,
    /// Only bulletins with this topic. `maintenance` is the only one.
    #[param(example = "maintenance")]
    pub topic: Option<String>,
    /// At most this many, up to 100. The default is 25.
    #[serde(default, deserialize_with = "blank_is_none")]
    #[param(value_type = Option<usize>)]
    pub limit: Option<usize>,
    /// `next_cursor` from the page before, unchanged.
    pub cursor: Option<String>,
}

/// [`BulletinFilter`] and the model year to use for a VIN. The fields are
/// written out again because a query string is read into one flat struct.
#[derive(Debug, Default, Deserialize, IntoParams)]
#[into_params(parameter_in = Query)]
pub struct VinBulletinsQuery {
    /// Use this model year instead of working it out from the VIN.
    #[serde(default, deserialize_with = "blank_is_none")]
    #[param(value_type = Option<u16>, example = 2023)]
    pub year: Option<u16>,
    /// Only this type: `service_bulletin`, `service_campaign`,
    /// `warranty_extension`, `over_the_air`, `emissions` or `other`.
    #[serde(rename = "type")]
    #[param(example = "warranty_extension")]
    pub kind: Option<String>,
    /// Only bulletins about this component: an id such as `brakes` or
    /// `brake-pad`.
    #[param(example = "brakes")]
    pub component: Option<String>,
    /// Only bulletins whose summary names this trouble code.
    #[param(example = "P0420")]
    pub code: Option<String>,
    /// Only bulletins with this text in them, in either case. At most 100
    /// characters are read.
    #[param(example = "noise")]
    pub q: Option<String>,
    /// Only bulletins with this topic. `maintenance` is the only one.
    pub topic: Option<String>,
    /// At most this many, up to 100. The default is 25.
    #[serde(default, deserialize_with = "blank_is_none")]
    #[param(value_type = Option<usize>)]
    pub limit: Option<usize>,
    /// `next_cursor` from the page before, unchanged.
    pub cursor: Option<String>,
}

impl VinBulletinsQuery {
    fn filter(self) -> (Option<u16>, BulletinFilter) {
        let year = self.year;
        (
            year,
            BulletinFilter {
                kind: self.kind,
                component: self.component,
                code: self.code,
                q: self.q,
                topic: self.topic,
                limit: self.limit,
                cursor: self.cursor,
            },
        )
    }
}

/// [`BulletinFilter`] and the vehicles to narrow a list of every bulletin
/// by.
#[derive(Debug, Default, Deserialize, IntoParams)]
#[into_params(parameter_in = Query)]
pub struct BulletinSearchQuery {
    /// Only bulletins that list a vehicle of this make: a name, an alias
    /// or an id, as in `/v1/vehicles/models`.
    #[param(example = "ford")]
    pub make: Option<String>,
    /// Only bulletins that list this model: a name or an id. Needs `make`.
    #[param(example = "f-150")]
    pub model: Option<String>,
    /// Only bulletins that list this model year.
    #[serde(default, deserialize_with = "blank_is_none")]
    #[param(value_type = Option<u16>, example = 2018)]
    pub year: Option<u16>,
    /// Only this type: `service_bulletin`, `service_campaign`,
    /// `warranty_extension`, `over_the_air`, `emissions` or `other`.
    #[serde(rename = "type")]
    #[param(example = "warranty_extension")]
    pub kind: Option<String>,
    /// Only bulletins about this component: an id such as `brakes` or
    /// `brake-pad`.
    #[param(example = "brakes")]
    pub component: Option<String>,
    /// Only bulletins whose summary names this trouble code.
    #[param(example = "P0420")]
    pub code: Option<String>,
    /// Only bulletins with this text in them, in either case. At most 100
    /// characters are read.
    #[param(example = "noise")]
    pub q: Option<String>,
    /// Only bulletins with this topic. `maintenance` is the only one.
    pub topic: Option<String>,
    /// At most this many, up to 100. The default is 25.
    #[serde(default, deserialize_with = "blank_is_none")]
    #[param(value_type = Option<usize>)]
    pub limit: Option<usize>,
    /// `next_cursor` from the page before, unchanged.
    pub cursor: Option<String>,
}

fn invalid(field: &str, message: &str) -> ApiError {
    ApiError::Validation {
        message: message.to_owned(),
        details: json!({ "field": field }),
    }
}

/// Reads a filter into the reader's query, with no vehicle yet.
pub fn narrowed(filter: BulletinFilter) -> Result<BulletinQuery, ApiError> {
    let kind = match given(filter.kind) {
        None => None,
        Some(text) => Some(BulletinType::parse(&text.to_ascii_lowercase()).ok_or_else(|| {
            invalid(
                "type",
                "type must be service_bulletin, service_campaign, warranty_extension, over_the_air, emissions or other",
            )
        })?),
    };
    let code = match given(filter.code) {
        None => None,
        Some(text) => Some(
            code_form(&text)
                .ok_or_else(|| invalid("code", "code must be a trouble code such as P0420"))?,
        ),
    };
    let slug = |field: &str, value: Option<String>| -> Result<Option<String>, ApiError> {
        match given(value) {
            None => Ok(None),
            Some(text) => {
                let text = text.to_ascii_lowercase();
                if is_slug(&text) && text.len() <= 60 {
                    Ok(Some(text))
                } else {
                    Err(invalid(
                        field,
                        &format!("{field} must be lowercase words joined by hyphens"),
                    ))
                }
            }
        }
    };
    let component = slug("component", filter.component)?;
    let topic = slug("topic", filter.topic)?;
    let cursor = match given(filter.cursor) {
        None => None,
        Some(text) => Some(
            BulletinCursor::parse(&text)
                .ok_or_else(|| invalid("cursor", "cursor is not one this API gave out"))?,
        ),
    };
    Ok(BulletinQuery {
        make_id: None,
        model_id: None,
        year: None,
        kind,
        component,
        code,
        topic,
        q: given(filter.q),
        limit: filter
            .limit
            .unwrap_or(LIST_DEFAULT)
            .clamp(1, MOST_BULLETINS),
        cursor,
    })
}

/// The bulletins of one model year, as the API and the pages give them.
pub async fn bulletins_of(
    state: &AppState,
    shop: &ShopDb,
    found: &ModelYear,
    mut asked: BulletinQuery,
) -> Result<VehicleBulletins, ApiError> {
    let (make_id, model_id, year) = (found.make_id, found.model_id, found.vehicle.year);
    asked.make_id = Some(make_id);
    asked.model_id = Some(model_id);
    asked.year = Some(year);
    let (page, found_counts) = shop
        .run(move |reader| {
            let page = reader.bulletins(&asked)?;
            let counts = reader.bulletin_counts(make_id, model_id, year)?;
            Ok::<_, wenmar_shopdata::ShopError>((page, counts))
        })
        .await??;
    let base = &state.config().base_url;
    Ok(VehicleBulletins {
        vehicle: found.vehicle.clone(),
        as_of: as_of(shop),
        counts: counts(found_counts),
        bulletins: page
            .bulletins
            .into_iter()
            .map(|bulletin| listed(base, bulletin))
            .collect(),
        next_cursor: page.next_cursor.map(|cursor| cursor.to_string()),
        note: NOTE.to_owned(),
        attribution: attribution(shop),
    })
}

pub async fn vehicle_bulletins_op(
    state: &AppState,
    id: String,
    filter: BulletinFilter,
) -> Result<VehicleBulletins, ApiError> {
    let shop = state.dataset(Dataset::Bulletins)?;
    let asked = narrowed(filter)?;
    let found = state
        .db()
        .run(move |worker| model_year(worker, &id))
        .await??;
    let Some(found) = found else {
        return Err(ApiError::NotFound("No vehicle has that id.".to_owned()));
    };
    bulletins_of(state, shop, &found, asked).await
}

pub async fn vin_bulletins_op(
    state: &AppState,
    vin: String,
    query: VinBulletinsQuery,
) -> Result<VinBulletins, ApiError> {
    let shop = state.dataset(Dataset::Bulletins)?;
    let (year, filter) = query.filter();
    let asked = narrowed(filter)?;
    let mut decoded = decode_many(state, vec![vin], year).await?;
    let decode = match decoded.pop() {
        Some(result) => result?,
        None => return Err(ApiError::internal("a decode gave no answer")),
    };
    let found = match decode
        .catalog
        .as_ref()
        .map(|selection| selection.vehicle_id.clone())
    {
        Some(id) => {
            state
                .db()
                .run(move |worker| model_year(worker, &id))
                .await??
        }
        None => None,
    };
    let mut warnings = decode.warnings;
    let summary = RecallDecode {
        vin: decode.vin,
        valid: decode.valid,
        year: decode.year,
        make: decode.make,
        model: decode.model,
        trim: decode.trim,
    };
    let Some(found) = found else {
        // Not an error: the VIN decoded, there is only no model year to
        // list bulletins for.
        warnings.push(Warning {
            code: "no_catalog_vehicle".to_owned(),
            message: NO_CATALOG_VEHICLE.to_owned(),
            suggestions: Vec::new(),
        });
        return Ok(VinBulletins {
            decode: summary,
            vehicle: None,
            as_of: as_of(shop),
            counts: BulletinCounts::default(),
            bulletins: Vec::new(),
            next_cursor: None,
            note: NOTE.to_owned(),
            attribution: attribution(shop),
            warnings,
        });
    };
    let answer = bulletins_of(state, shop, &found, asked).await?;
    Ok(VinBulletins {
        decode: summary,
        vehicle: Some(answer.vehicle),
        as_of: answer.as_of,
        counts: answer.counts,
        bulletins: answer.bulletins,
        next_cursor: answer.next_cursor,
        note: answer.note,
        attribution: answer.attribution,
        warnings,
    })
}

pub async fn bulletin_op(state: &AppState, nhtsa_id: &str) -> Result<BulletinDetail, ApiError> {
    Ok(bulletin_and_reach(state, nhtsa_id).await?.0)
}

/// One bulletin, and whether any vehicle it lists is a car, an MPV or a
/// truck of the catalog.
pub async fn bulletin_and_reach(
    state: &AppState,
    nhtsa_id: &str,
) -> Result<(BulletinDetail, bool), ApiError> {
    let missing = || ApiError::NotFound("No bulletin has that NHTSA id.".to_owned());
    let shop = state.dataset(Dataset::Bulletins)?;
    let text = nhtsa_id.trim();
    if text.is_empty() || text.len() > 12 || !text.bytes().all(|byte| byte.is_ascii_digit()) {
        return Err(missing());
    }
    let id: i64 = text.parse().map_err(|_| missing())?;
    let found = shop.run(move |reader| reader.bulletin(id)).await??;
    let Some(detail) = found else {
        return Err(missing());
    };
    let mut model_ids: Vec<i64> = detail
        .vehicles
        .iter()
        .filter_map(|vehicle| vehicle.model_id)
        .collect();
    model_ids.sort_unstable();
    model_ids.dedup();
    let slugs = state
        .db()
        .run(move |worker| model_slugs(worker, &model_ids))
        .await??;
    let light = detail
        .vehicles
        .iter()
        .filter_map(|vehicle| vehicle.model_id)
        .any(|id| slugs.get(&id).is_some_and(|model| model.light));
    let vehicles = detail
        .vehicles
        .into_iter()
        .map(|vehicle| {
            let model = vehicle.model_id.and_then(|id| slugs.get(&id));
            let vehicle_ids = match (vehicle.year_from, vehicle.year_to, model) {
                (Some(from), Some(to), Some(found)) => (from..=to)
                    .map(|year| format!("{year:04}_{}_{}", found.make, found.model))
                    .collect(),
                _ => Vec::new(),
            };
            BulletinVehicle {
                make: vehicle.make_text,
                model: vehicle.model_text,
                year_from: vehicle.year_from,
                year_to: vehicle.year_to,
                matched: vehicle.matched,
                vehicle_ids,
            }
        })
        .collect();
    let bulletin = detail.bulletin;
    let found = BulletinDetail {
        url: bulletin_url(&state.config().base_url, bulletin.nhtsa_id),
        document: document(
            bulletin.nhtsa_id,
            bulletin.date.as_deref(),
            &bulletin.date_added,
        ),
        nhtsa_id: bulletin.nhtsa_id,
        type_name: type_name(&bulletin.kind),
        kind: bulletin.kind,
        date_added: bulletin.date_added,
        date: bulletin.date,
        document_id: bulletin.document_id,
        mfr_campaign: bulletin.mfr_campaign,
        replaces: detail.replaces,
        component: bulletin.component,
        mfr_system: detail.mfr_system,
        summary: bulletin.summary,
        components: detail
            .components
            .into_iter()
            .map(|component| BulletinComponent {
                id: component.id,
                name: component.name,
                parent: component.parent,
            })
            .collect(),
        codes: detail.codes,
        topics: detail.topics,
        vehicles,
        as_of: as_of(shop),
        note: NOTE.to_owned(),
        attribution: attribution(shop),
    };
    Ok((found, light))
}

pub async fn bulletin_search_op(
    state: &AppState,
    query: BulletinSearchQuery,
) -> Result<BulletinList, ApiError> {
    let shop = state.dataset(Dataset::Bulletins)?;
    let (make, model) = (given(query.make), given(query.model));
    if model.is_some() && make.is_none() {
        return Err(invalid("make", "model needs make"));
    }
    let mut asked = narrowed(BulletinFilter {
        kind: query.kind,
        component: query.component,
        code: query.code,
        q: query.q,
        topic: query.topic,
        limit: query.limit,
        cursor: query.cursor,
    })?;
    let empty = BulletinList {
        as_of: as_of(shop),
        bulletins: Vec::new(),
        next_cursor: None,
        note: NOTE.to_owned(),
        attribution: attribution(shop),
    };
    let ids = state
        .db()
        .run(move |worker| named(worker, make.as_deref(), model.as_deref()))
        .await??;
    // A make or a model the catalog does not have lists nothing, as in
    // `/v1/recalls`.
    let Some((make_id, model_id)) = ids else {
        return Ok(empty);
    };
    asked.make_id = make_id;
    asked.model_id = model_id;
    asked.year = query.year;
    // Free text is a scan of the summaries the other narrowings leave: up
    // to a fifth of a second on the real file when nothing matches.
    let page = shop.run(move |reader| reader.bulletins(&asked)).await??;
    let base = &state.config().base_url;
    Ok(BulletinList {
        bulletins: page
            .bulletins
            .into_iter()
            .map(|bulletin| listed(base, bulletin))
            .collect(),
        next_cursor: page.next_cursor.map(|cursor| cursor.to_string()),
        ..empty
    })
}

/// Manufacturer communications that list a model year: service bulletins,
/// service campaigns, warranty extensions and over-the-air updates.
///
/// One page, newest first; `counts` says how many there are in all and of
/// what kind, so a caller can offer the narrowings that have something.
#[utoipa::path(
    get,
    path = "/vehicles/{id}/bulletins",
    tag = "Bulletins",
    params(
        ("id" = String, Path, description = "A vehicle id. Bulletins are listed by year, make and model; a submodel or an engine in the id is set aside.", example = "2018_ford_f-150"),
        BulletinFilter
    ),
    responses(
        (status = 200, description = "One page of bulletins, newest first.", body = VehicleBulletins),
        (status = 400, description = "`validation_failed`: `type`, `component`, `code`, `topic`, `limit` or `cursor` is wrong.", body = ErrorBody),
        (status = 404, description = "`not_found`: no vehicle has that id. `dataset_unavailable`: this server holds no bulletins.", body = ErrorBody)
    )
)]
pub async fn vehicle_bulletins(
    State(state): State<AppState>,
    path: Result<Path<String>, PathRejection>,
    query: Result<Query<BulletinFilter>, QueryRejection>,
) -> Result<Json<VehicleBulletins>, ApiError> {
    let Ok(Path(id)) = path else {
        return Err(ApiError::NotFound("No vehicle has that id.".to_owned()));
    };
    let Query(filter) = query.map_err(bad_query)?;
    Ok(Json(vehicle_bulletins_op(&state, id, filter).await?))
}

/// Manufacturer communications that list the year, make and model a VIN
/// decodes to.
///
/// A VIN that decodes to no vehicle in the catalog gets an empty list and
/// the warning `no_catalog_vehicle`, not an error.
#[utoipa::path(
    get,
    path = "/vin/{vin}/bulletins",
    tag = "Bulletins",
    params(
        ("vin" = String, Path, description = "A 17-character VIN.", example = "KM8K2CAB4PU001140"),
        VinBulletinsQuery
    ),
    responses(
        (status = 200, description = "One page of bulletins, newest first.", body = VinBulletins),
        (status = 400, description = "`invalid_vin`, or `validation_failed` for a query value.", body = ErrorBody),
        (status = 404, description = "`not_found`: no manufacturer is registered for the first three characters. `dataset_unavailable`: this server holds no bulletins.", body = ErrorBody)
    )
)]
pub async fn vin_bulletins(
    State(state): State<AppState>,
    path: Result<Path<String>, PathRejection>,
    query: Result<Query<VinBulletinsQuery>, QueryRejection>,
) -> Result<Json<VinBulletins>, ApiError> {
    let Ok(Path(vin)) = path else {
        return Err(ApiError::InvalidVin {
            message: "a VIN uses only digits and letters other than I, O and Q".to_owned(),
            details: json!({ "suggestions": [] }),
        });
    };
    let Query(query) = query.map_err(bad_query)?;
    Ok(Json(vin_bulletins_op(&state, vin, query).await?))
}

/// One manufacturer communication, by NHTSA's id number, with every
/// vehicle it lists, the components it is about and the trouble codes its
/// summary names.
#[utoipa::path(
    get,
    path = "/bulletins/{nhtsa_id}",
    tag = "Bulletins",
    params(
        ("nhtsa_id" = String, Path, description = "NHTSA's id number for the communication.", example = "10152456")
    ),
    responses(
        (status = 200, description = "The bulletin.", body = BulletinDetail),
        (status = 404, description = "`not_found`: no bulletin has that id. `dataset_unavailable`: this server holds no bulletins.", body = ErrorBody)
    )
)]
pub async fn bulletin(
    State(state): State<AppState>,
    path: Result<Path<String>, PathRejection>,
) -> Result<Json<BulletinDetail>, ApiError> {
    let Ok(Path(nhtsa_id)) = path else {
        return Err(ApiError::NotFound(
            "No bulletin has that NHTSA id.".to_owned(),
        ));
    };
    Ok(Json(bulletin_op(&state, &nhtsa_id).await?))
}

/// Manufacturer communications, newest first: every one, or those
/// narrowed by make, model, year, type, component, trouble code, topic or
/// free text.
///
/// A make or a model the catalog does not have gives an empty list.
#[utoipa::path(
    get,
    path = "/bulletins",
    tag = "Bulletins",
    params(BulletinSearchQuery),
    responses(
        (status = 200, description = "One page of bulletins.", body = BulletinList),
        (status = 400, description = "`validation_failed`: a query value is wrong, or `model` was given without `make`.", body = ErrorBody),
        (status = 404, description = "`dataset_unavailable`: this server holds no bulletins.", body = ErrorBody)
    )
)]
pub async fn bulletin_search(
    State(state): State<AppState>,
    query: Result<Query<BulletinSearchQuery>, QueryRejection>,
) -> Result<Json<BulletinList>, ApiError> {
    let Query(query) = query.map_err(bad_query)?;
    Ok(Json(bulletin_search_op(&state, query).await?))
}
