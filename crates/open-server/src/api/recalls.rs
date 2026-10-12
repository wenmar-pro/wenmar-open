//! Recalls that may apply to a model year, one campaign, and the list of
//! campaigns.
//!
//! Neither NHTSA nor Transport Canada publishes recalls by VIN as data. A
//! campaign lists a year, a make and a model, so every answer here says
//! `may_apply` and gives the official lookups, where a VIN can be checked.
//!
//! Each route is a thin wrapper around a function ending in `_op`, as in
//! `vehicles.rs`.

use std::collections::HashMap;

use axum::Json;
use axum::extract::rejection::{PathRejection, QueryRejection};
use axum::extract::{Path, Query, State};
use serde::Deserialize;
use serde_json::json;
use utoipa::IntoParams;
use wenmar_shopdata::Dataset;
use wenmar_shopdata::recalls::{
    self as shopdata, MOST_RECALLS, RecallCursor, RecallQuery, RecallSource,
};
use wenmar_vehicles::text::{is_slug, normalize};
use wenmar_vehicles::{CatalogError, Scope, Source, Value, VehicleId};

use crate::api::types::{
    Recall, RecallAsOf, RecallCampaign, RecallCampaignVehicle, RecallCompletion, RecallDecode,
    RecallFrench, RecallList, RecallLookup, RecallVehicle, VehicleRecalls, VinRecalls, Warning,
};
use crate::api::vin::decode_many;
use crate::api::{bad_query, blank_is_none};
use crate::db::Worker;
use crate::error::{ApiError, ErrorBody};
use crate::shop::ShopDb;
use crate::state::AppState;

/// NHTSA's recall lookup by VIN, for the United States.
pub const NHTSA_LOOKUP: &str = "https://www.nhtsa.gov/recalls";

/// Transport Canada's recall search, for Canada.
pub const TC_LOOKUP: &str =
    "https://wwwapps.tc.gc.ca/Saf-Sec-Sur/7/VRDB-BDRV/search-recherche/menu.aspx?lang=eng";

/// What every page and answer about recalls says. The words "may apply"
/// are the point: nothing here is a lookup by VIN.
pub const MAY_APPLY: &str = "A recall listed here may apply to a vehicle of this year, make and model. Only the manufacturer can say whether it applies to one vehicle: check the VIN with the official lookups.";

/// What a VIN's recall answer says when the VIN names no model year.
pub const NO_CATALOG_VEHICLE: &str = "This VIN does not decode to a year, make and model in the catalog, so no recalls are listed for it. Check the VIN with the official lookups.";

/// Campaigns in a list when `limit` is not given.
pub const LIST_DEFAULT: usize = 25;

/// Longest campaign number read from an address.
const LONGEST_CAMPAIGN: usize = 20;

/// A model year of the catalog, with the ids the shop file knows it by.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ModelYear {
    pub vehicle: RecallVehicle,
    pub make_id: i64,
    pub model_id: i64,
    pub make_slug: String,
    pub model_slug: String,
    /// Whether it is a car, an MPV or a truck.
    pub light: bool,
}

/// `?1` make id, `?2` the model's id form, `?3` year.
const MODEL_YEAR_SQL: &str = "
SELECT md.id, v.light
FROM catalog_model md
JOIN catalog_vehicle v ON v.model_id = md.id
WHERE md.make_id = ?1 AND md.slug = ?2 AND v.year = ?3";

/// `?1` make id, `?2` the model as an id form, `?3` its matching form.
const MODEL_ID_SQL: &str = "
SELECT id FROM catalog_model
WHERE make_id = ?1 AND (slug = ?2 OR norm = ?3)
ORDER BY (slug = ?2) DESC, id
LIMIT 1";

/// The model year a vehicle id names. A submodel or an engine in the id
/// must exist, and is then set aside: recalls are listed by year, make and
/// model. `None` when no vehicle has that id.
pub fn model_year(worker: &Worker, id: &str) -> Result<Option<ModelYear>, CatalogError> {
    let Some(entry) = worker.catalog.entry(id)? else {
        return Ok(None);
    };
    let Some(parsed) = VehicleId::parse(&entry.id) else {
        return Ok(None);
    };
    let Some(make) = worker.catalog.index().by_slug(&parsed.make) else {
        return Ok(None);
    };
    let rows = worker
        .source
        .query(
            MODEL_YEAR_SQL,
            &[
                make.id.into(),
                parsed.model.as_str().into(),
                i64::from(parsed.year).into(),
            ],
        )
        .map_err(CatalogError::Source)?;
    let Some(row) = rows.first() else {
        return Ok(None);
    };
    let Some(model_id) = row.first().and_then(Value::integer) else {
        return Ok(None);
    };
    Ok(Some(ModelYear {
        vehicle: RecallVehicle {
            id: format!("{:04}_{}_{}", parsed.year, parsed.make, parsed.model),
            year: parsed.year,
            make: entry.make,
            model: entry.model,
        },
        make_id: make.id,
        model_id,
        make_slug: parsed.make,
        model_slug: parsed.model,
        light: row.get(1).and_then(Value::integer) == Some(1),
    }))
}

/// A catalog model, as far as an address needs it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ModelRef {
    /// The id forms of the make and the model.
    pub make: String,
    pub model: String,
    /// Whether the model is a car, an MPV or a truck.
    pub light: bool,
}

/// Catalog models by id, for building vehicle ids and addresses. Ids the
/// catalog lacks are left out.
pub fn model_slugs(
    worker: &Worker,
    model_ids: &[i64],
) -> Result<HashMap<i64, ModelRef>, CatalogError> {
    let mut found = HashMap::new();
    for chunk in model_ids.chunks(400) {
        // The ids are numbers from the shop file, so they are written into
        // the statement as they are.
        let list: Vec<String> = chunk.iter().map(i64::to_string).collect();
        let sql = format!(
            "SELECT md.id, mk.slug, md.slug, md.light FROM catalog_model md JOIN catalog_make mk ON mk.id = md.make_id WHERE md.id IN ({})",
            list.join(",")
        );
        for row in worker
            .source
            .query(&sql, &[])
            .map_err(CatalogError::Source)?
        {
            if let (Some(id), Some(make), Some(model)) = (
                row.first().and_then(Value::integer),
                row.get(1).and_then(Value::text),
                row.get(2).and_then(Value::text),
            ) && is_slug(make)
                && is_slug(model)
            {
                found.insert(
                    id,
                    ModelRef {
                        make: make.to_owned(),
                        model: model.to_owned(),
                        light: row.get(3).and_then(Value::integer) == Some(1),
                    },
                );
            }
        }
    }
    Ok(found)
}

/// The make and the model ids a list is narrowed by.
pub type NamedIds = (Option<i64>, Option<i64>);

/// The catalog ids a make and a model in words mean. `Ok(None)` when the
/// make, or the model, is not in the catalog: nothing can then be listed.
pub fn named(
    worker: &Worker,
    make: Option<&str>,
    model: Option<&str>,
) -> Result<Option<NamedIds>, CatalogError> {
    let Some(make) = make else {
        return Ok(Some((None, None)));
    };
    let Some(found) = worker.catalog.index().resolve(make, Scope::All) else {
        return Ok(None);
    };
    let Some(model) = model else {
        return Ok(Some((Some(found.id), None)));
    };
    let rows = worker
        .source
        .query(
            MODEL_ID_SQL,
            &[
                found.id.into(),
                model.trim().into(),
                normalize(model).as_str().into(),
            ],
        )
        .map_err(CatalogError::Source)?;
    Ok(rows
        .first()
        .and_then(|row| row.first())
        .and_then(Value::integer)
        .map(|model_id| (Some(found.id), Some(model_id))))
}

/// Reads `country`: `us`, `ca`, or nothing for both.
pub fn country(text: Option<&str>) -> Result<Option<RecallSource>, ApiError> {
    match text.map(str::trim).filter(|text| !text.is_empty()) {
        None => Ok(None),
        Some(text) => RecallSource::from_country(&text.to_ascii_lowercase())
            .map(Some)
            .ok_or_else(|| ApiError::Validation {
                message: "country must be us or ca".to_owned(),
                details: json!({ "field": "country" }),
            }),
    }
}

/// The shop file, when it holds the recalls asked for: one country's, or
/// with no country either one's. A server with the recalls of one country
/// answers for that country, and answers 404 `dataset_unavailable` only
/// when the other is asked for by name.
pub fn recall_shop(state: &AppState, source: Option<RecallSource>) -> Result<&ShopDb, ApiError> {
    match source {
        Some(source) => state.dataset(source.dataset()),
        None => state
            .dataset(Dataset::RecallsUs)
            .or_else(|_| state.dataset(Dataset::RecallsCa))
            .map_err(|_| ApiError::DatasetUnavailable(Dataset::RecallsUs)),
    }
}

/// The day each country's recalls were current.
pub fn as_of(shop: &ShopDb) -> RecallAsOf {
    let day = |dataset: Dataset| {
        shop.datasets()
            .get(dataset)
            .map(|entry| entry.as_of.clone())
    };
    RecallAsOf {
        us: day(Dataset::RecallsUs),
        ca: day(Dataset::RecallsCa),
    }
}

/// The credit lines of the recall datasets an answer drew on.
pub fn attribution(shop: &ShopDb, source: Option<RecallSource>) -> Vec<String> {
    RecallSource::ALL
        .into_iter()
        .filter(|held| source.is_none_or(|asked| asked == *held))
        .filter_map(|held| shop.datasets().get(held.dataset()))
        .map(|entry| entry.attribution.clone())
        .filter(|credit| !credit.is_empty())
        .collect()
}

/// The maker's own lookup by VIN, where one is on file.
fn maker_lookup(make: &str) -> Option<String> {
    crate::site::links::for_make(make)
        .and_then(|links| links.recall_lookup_url)
        .map(str::to_owned)
}

/// The official lookups, and the maker's own for `make` where there is one.
pub fn lookup(make: Option<&str>) -> RecallLookup {
    RecallLookup {
        nhtsa: NHTSA_LOOKUP.to_owned(),
        tc: TC_LOOKUP.to_owned(),
        maker: make.and_then(maker_lookup),
    }
}

/// The address of a campaign's page on this site.
pub fn campaign_url(base: &str, source: &str, campaign: &str) -> String {
    format!("{base}/recalls/{source}/{campaign}")
}

/// A campaign as a list that is not about one model year gives it.
pub fn listed(base: &str, recall: shopdata::RecallSummary) -> Recall {
    Recall {
        url: campaign_url(base, &recall.source, &recall.campaign),
        source: recall.source,
        country: recall.country,
        campaign: recall.campaign,
        date: recall.date,
        manufacturer: recall.manufacturer,
        component: recall.component,
        summary: recall.summary,
        mfr_campaign: recall.mfr_campaign,
        units: recall.units,
        do_not_drive: recall.do_not_drive,
        park_outside: recall.park_outside,
        notification: recall.notification,
        matched: None,
        models: Vec::new(),
        built_from: None,
        built_to: None,
    }
}

fn for_vehicle(base: &str, found: shopdata::VehicleRecall) -> Recall {
    let mut recall = listed(base, found.recall);
    recall.matched = Some(found.matched);
    recall.models = found.models;
    recall.built_from = found.built_from;
    recall.built_to = found.built_to;
    recall
}

/// Text a caller gave, or `None` when it is blank.
pub fn given(value: Option<String>) -> Option<String> {
    value
        .map(|value| value.trim().to_owned())
        .filter(|value| !value.is_empty())
}

#[derive(Debug, Default, Deserialize, IntoParams)]
#[into_params(parameter_in = Query)]
pub struct VehicleRecallsQuery {
    /// `us` or `ca`. Left out: both, where the server holds both.
    #[param(example = "us")]
    pub country: Option<String>,
}

#[derive(Debug, Default, Deserialize, IntoParams)]
#[into_params(parameter_in = Query)]
pub struct VinRecallsQuery {
    /// `us` or `ca`. Left out: both, where the server holds both.
    #[param(example = "us")]
    pub country: Option<String>,
    /// Use this model year instead of working it out from the VIN.
    #[serde(default, deserialize_with = "blank_is_none")]
    #[param(value_type = Option<u16>, example = 2023)]
    pub year: Option<u16>,
}

#[derive(Debug, Default, Deserialize, IntoParams)]
#[into_params(parameter_in = Query)]
pub struct RecallSearchQuery {
    /// Only campaigns that list a vehicle of this make: a name, an alias
    /// or an id, as in `/v1/vehicles/models`.
    #[param(example = "ford")]
    pub make: Option<String>,
    /// Only campaigns that list this model: a name or an id. Needs `make`.
    #[param(example = "f-150")]
    pub model: Option<String>,
    /// Only campaigns that list this model year.
    #[serde(default, deserialize_with = "blank_is_none")]
    #[param(value_type = Option<u16>, example = 2018)]
    pub year: Option<u16>,
    /// Only campaigns whose component text contains this, in either case.
    #[param(example = "air bags")]
    pub component: Option<String>,
    /// Only campaigns with this text in them, in either case: in the
    /// campaign number, the manufacturer's campaign number, the
    /// manufacturer, the component, the summary, the consequence, the
    /// remedy, the notes, or the make or model of a vehicle listed. At
    /// most 100 characters are read.
    #[param(example = "tailgate")]
    pub q: Option<String>,
    /// `us` or `ca`. Left out: both, where the server holds both.
    pub country: Option<String>,
    /// At most this many, up to 100. The default is 25.
    #[serde(default, deserialize_with = "blank_is_none")]
    #[param(value_type = Option<usize>)]
    pub limit: Option<usize>,
    /// `next_cursor` from the page before, unchanged.
    pub cursor: Option<String>,
}

/// The recalls of one model year, as the API and the pages give them.
pub async fn recalls_of(
    state: &AppState,
    shop: &ShopDb,
    found: &ModelYear,
    source: Option<RecallSource>,
) -> Result<VehicleRecalls, ApiError> {
    let (make_id, model_id, year) = (found.make_id, found.model_id, found.vehicle.year);
    let recalls = shop
        .run(move |reader| reader.recalls_for_vehicle(make_id, model_id, year, source))
        .await??;
    let base = &state.config().base_url;
    Ok(VehicleRecalls {
        vehicle: found.vehicle.clone(),
        may_apply: true,
        as_of: as_of(shop),
        recalls: recalls
            .into_iter()
            .map(|recall| for_vehicle(base, recall))
            .collect(),
        lookup: lookup(Some(&found.vehicle.make)),
        attribution: attribution(shop, source),
    })
}

pub async fn vehicle_recalls_op(
    state: &AppState,
    id: String,
    query: VehicleRecallsQuery,
) -> Result<VehicleRecalls, ApiError> {
    let source = country(query.country.as_deref())?;
    let shop = recall_shop(state, source)?;
    let found = state
        .db()
        .run(move |worker| model_year(worker, &id))
        .await??;
    let Some(found) = found else {
        return Err(ApiError::NotFound("No vehicle has that id.".to_owned()));
    };
    recalls_of(state, shop, &found, source).await
}

pub async fn vin_recalls_op(
    state: &AppState,
    vin: String,
    query: VinRecallsQuery,
) -> Result<VinRecalls, ApiError> {
    let source = country(query.country.as_deref())?;
    let shop = recall_shop(state, source)?;
    let mut decoded = decode_many(state, vec![vin], query.year).await?;
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
        // list recalls for.
        warnings.push(Warning {
            code: "no_catalog_vehicle".to_owned(),
            message: NO_CATALOG_VEHICLE.to_owned(),
            suggestions: Vec::new(),
        });
        return Ok(VinRecalls {
            lookup: lookup(summary.make.as_deref()),
            decode: summary,
            vehicle: None,
            may_apply: true,
            as_of: as_of(shop),
            recalls: Vec::new(),
            attribution: attribution(shop, source),
            warnings,
        });
    };
    let answer = recalls_of(state, shop, &found, source).await?;
    Ok(VinRecalls {
        decode: summary,
        vehicle: Some(answer.vehicle),
        may_apply: true,
        as_of: answer.as_of,
        recalls: answer.recalls,
        lookup: answer.lookup,
        attribution: answer.attribution,
        warnings,
    })
}

/// The newest report of remedy progress for an NHTSA campaign. `None` for
/// a Canadian campaign, for a campaign with no report, and on a server
/// that holds no reports: the campaign is then answered without it.
pub async fn completion_of(
    state: &AppState,
    source: RecallSource,
    campaign: &str,
) -> Result<Option<RecallCompletion>, ApiError> {
    if source != RecallSource::Nhtsa {
        return Ok(None);
    }
    let Ok(shop) = state.dataset(Dataset::RecallReports) else {
        return Ok(None);
    };
    let number = campaign.to_owned();
    let found = shop
        .run(move |reader| reader.recall_report(&number))
        .await??;
    let as_of = shop
        .datasets()
        .get(Dataset::RecallReports)
        .map(|entry| entry.as_of.clone());
    Ok(found.map(|report| RecallCompletion {
        remedied_percent: report.remedied_percent(),
        kind: report.kind,
        report_number: report.report_number,
        period_from: report.period_from,
        period_to: report.period_to,
        submitted: report.submitted,
        involved: report.involved,
        remedied: report.remedied,
        unreachable: report.unreachable,
        removed: report.removed,
        as_of,
    }))
}

pub async fn recall_campaign_op(
    state: &AppState,
    source: &str,
    campaign: &str,
) -> Result<RecallCampaign, ApiError> {
    Ok(campaign_and_reach(state, source, campaign).await?.0)
}

/// One campaign, and whether any vehicle it lists is a car, an MPV or a
/// truck of the catalog. The campaign's page is offered to search engines
/// only when one is.
pub async fn campaign_and_reach(
    state: &AppState,
    source: &str,
    campaign: &str,
) -> Result<(RecallCampaign, bool), ApiError> {
    let missing = || ApiError::NotFound("No recall campaign has that number.".to_owned());
    let Some(source) = RecallSource::parse(source) else {
        return Err(missing());
    };
    let shop = state.dataset(source.dataset())?;
    let number = campaign.trim().to_owned();
    if number.is_empty()
        || number.len() > LONGEST_CAMPAIGN
        || !number.bytes().all(|byte| byte.is_ascii_alphanumeric())
    {
        return Err(missing());
    }
    let found = shop
        .run(move |reader| reader.recall(source, &number))
        .await??;
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
            let vehicle_id = match (vehicle.year, model) {
                (Some(year), Some(found)) => {
                    Some(format!("{year:04}_{}_{}", found.make, found.model))
                }
                _ => None,
            };
            RecallCampaignVehicle {
                year: vehicle.year,
                make: vehicle.make_text,
                model: vehicle.model_text,
                vehicle_id,
                matched: vehicle.matched,
                built_from: vehicle.built_from,
                built_to: vehicle.built_to,
            }
        })
        .collect();
    let has_french = detail.component_fr.is_some()
        || detail.notification_fr.is_some()
        || detail.summary_fr.is_some();
    let french = has_french.then_some(RecallFrench {
        component: detail.component_fr,
        notification: detail.notification_fr,
        summary: detail.summary_fr,
    });
    let recall = detail.recall;
    let completion = completion_of(state, source, &recall.campaign).await?;
    let found = RecallCampaign {
        url: campaign_url(&state.config().base_url, &recall.source, &recall.campaign),
        source: recall.source,
        country: recall.country,
        campaign: recall.campaign,
        date: recall.date,
        owner_notified: detail.owner_notified,
        manufacturer: recall.manufacturer,
        component: recall.component,
        summary: recall.summary,
        consequence: detail.consequence,
        remedy: detail.remedy,
        notes: detail.notes,
        mfr_campaign: recall.mfr_campaign,
        part_number: detail.part_number,
        units: recall.units,
        completion,
        do_not_drive: recall.do_not_drive,
        park_outside: recall.park_outside,
        notification: recall.notification,
        french,
        vehicles,
        may_apply: true,
        as_of: shop
            .datasets()
            .get(source.dataset())
            .map(|entry| entry.as_of.clone()),
        lookup: lookup(None),
        attribution: attribution(shop, Some(source)),
    };
    Ok((found, light))
}

pub async fn recall_search_op(
    state: &AppState,
    query: RecallSearchQuery,
) -> Result<RecallList, ApiError> {
    let source = country(query.country.as_deref())?;
    let shop = recall_shop(state, source)?;
    let (make, model) = (given(query.make), given(query.model));
    if model.is_some() && make.is_none() {
        return Err(ApiError::Validation {
            message: "model needs make".to_owned(),
            details: json!({ "field": "make" }),
        });
    }
    let cursor = match given(query.cursor) {
        None => None,
        Some(text) => Some(
            RecallCursor::parse(&text).ok_or_else(|| ApiError::Validation {
                message: "cursor is not one this API gave out".to_owned(),
                details: json!({ "field": "cursor" }),
            })?,
        ),
    };
    let empty = RecallList {
        may_apply: true,
        as_of: as_of(shop),
        recalls: Vec::new(),
        next_cursor: None,
        lookup: lookup(None),
        attribution: attribution(shop, source),
    };
    let ids = state
        .db()
        .run(move |worker| named(worker, make.as_deref(), model.as_deref()))
        .await??;
    // A make or a model the catalog does not have lists nothing. It is not
    // an error, as an unknown make is not one in `/v1/vehicles/models`.
    let Some((make_id, model_id)) = ids else {
        return Ok(empty);
    };
    let asked = RecallQuery {
        source,
        make_id,
        model_id,
        year: query.year,
        component: given(query.component),
        q: given(query.q),
        since: None,
        limit: query.limit.unwrap_or(LIST_DEFAULT).clamp(1, MOST_RECALLS),
        cursor,
    };
    // Free text is a scan of every campaign: up to a quarter of a second on
    // the real file when nothing matches.
    let page = shop.run(move |reader| reader.recalls(&asked)).await??;
    let base = &state.config().base_url;
    Ok(RecallList {
        recalls: page
            .recalls
            .into_iter()
            .map(|recall| listed(base, recall))
            .collect(),
        next_cursor: page.next_cursor.map(|cursor| cursor.to_string()),
        ..empty
    })
}

/// Recalls that may apply to a model year.
///
/// The list is for the year, make and model. It is not a lookup by VIN:
/// `may_apply` is always `true`, and `lookup` gives the official addresses
/// where a VIN can be checked.
#[utoipa::path(
    get,
    path = "/vehicles/{id}/recalls",
    tag = "Recalls",
    params(
        ("id" = String, Path, description = "A vehicle id. Recalls are listed by year, make and model; a submodel or an engine in the id is set aside.", example = "2018_ford_f-150"),
        VehicleRecallsQuery
    ),
    responses(
        (status = 200, description = "Recalls that may apply, newest first.", body = VehicleRecalls),
        (status = 400, description = "`validation_failed`: `country` is not `us` or `ca`.", body = ErrorBody),
        (status = 404, description = "`not_found`: no vehicle has that id. `dataset_unavailable`: this server holds no recalls, or none for the country asked for.", body = ErrorBody)
    )
)]
pub async fn vehicle_recalls(
    State(state): State<AppState>,
    path: Result<Path<String>, PathRejection>,
    query: Result<Query<VehicleRecallsQuery>, QueryRejection>,
) -> Result<Json<VehicleRecalls>, ApiError> {
    let Ok(Path(id)) = path else {
        return Err(ApiError::NotFound("No vehicle has that id.".to_owned()));
    };
    let Query(query) = query.map_err(bad_query)?;
    Ok(Json(vehicle_recalls_op(&state, id, query).await?))
}

/// Recalls that may apply to the year, make and model a VIN decodes to.
///
/// This decodes the VIN and lists the recalls of its model year. It does
/// not look the VIN itself up, which no public data allows: `may_apply` is
/// always `true`. A VIN that decodes to no vehicle in the catalog gets an
/// empty list and the warning `no_catalog_vehicle`, not an error.
#[utoipa::path(
    get,
    path = "/vin/{vin}/recalls",
    tag = "Recalls",
    params(
        ("vin" = String, Path, description = "A 17-character VIN.", example = "KM8K2CAB4PU001140"),
        VinRecallsQuery
    ),
    responses(
        (status = 200, description = "Recalls that may apply, newest first.", body = VinRecalls),
        (status = 400, description = "`invalid_vin`, or `validation_failed` for `country` or `year`.", body = ErrorBody),
        (status = 404, description = "`not_found`: no manufacturer is registered for the first three characters. `dataset_unavailable`: this server holds no recalls, or none for the country asked for.", body = ErrorBody)
    )
)]
pub async fn vin_recalls(
    State(state): State<AppState>,
    path: Result<Path<String>, PathRejection>,
    query: Result<Query<VinRecallsQuery>, QueryRejection>,
) -> Result<Json<VinRecalls>, ApiError> {
    let Ok(Path(vin)) = path else {
        return Err(ApiError::InvalidVin {
            message: "a VIN uses only digits and letters other than I, O and Q".to_owned(),
            details: json!({ "suggestions": [] }),
        });
    };
    let Query(query) = query.map_err(bad_query)?;
    Ok(Json(vin_recalls_op(&state, vin, query).await?))
}

/// One recall campaign with every vehicle it lists.
#[utoipa::path(
    get,
    path = "/recalls/{source}/{campaign}",
    tag = "Recalls",
    params(
        ("source" = String, Path, description = "Who published it: `nhtsa` or `tc` (Transport Canada).", example = "nhtsa"),
        ("campaign" = String, Path, description = "The publisher's campaign number. Letters may be in either case.", example = "19V888000")
    ),
    responses(
        (status = 200, description = "The campaign.", body = RecallCampaign),
        (status = 404, description = "`not_found`: no campaign has that number. `dataset_unavailable`: this server holds no recalls from that publisher.", body = ErrorBody)
    )
)]
pub async fn recall_campaign(
    State(state): State<AppState>,
    path: Result<Path<(String, String)>, PathRejection>,
) -> Result<Json<RecallCampaign>, ApiError> {
    let Ok(Path((source, campaign))) = path else {
        return Err(ApiError::NotFound(
            "No recall campaign has that number.".to_owned(),
        ));
    };
    Ok(Json(recall_campaign_op(&state, &source, &campaign).await?))
}

/// Recall campaigns, newest first: every one, or those narrowed by make,
/// model, year, component, free text or country.
///
/// Each campaign may apply to the vehicles it lists. A make or a model the
/// catalog does not have gives an empty list.
#[utoipa::path(
    get,
    path = "/recalls",
    tag = "Recalls",
    params(RecallSearchQuery),
    responses(
        (status = 200, description = "One page of campaigns.", body = RecallList),
        (status = 400, description = "`validation_failed`: `country`, `year`, `limit` or `cursor` is wrong, or `model` was given without `make`.", body = ErrorBody),
        (status = 404, description = "`dataset_unavailable`: this server holds no recalls, or none for the country asked for.", body = ErrorBody)
    )
)]
pub async fn recall_search(
    State(state): State<AppState>,
    query: Result<Query<RecallSearchQuery>, QueryRejection>,
) -> Result<Json<RecallList>, ApiError> {
    let Query(query) = query.map_err(bad_query)?;
    Ok(Json(recall_search_op(&state, query).await?))
}
