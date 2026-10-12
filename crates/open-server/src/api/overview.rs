//! Answers that sit over the per-dataset routes, for an application that
//! shows one vehicle: everything held for a model year in one answer, the
//! model years of a model, and the campaigns reported since a day.
//!
//! Nothing here reads a dataset its own way. The recall block is made from
//! the same list `/v1/vehicles/{id}/recalls` gives, so the two cannot
//! disagree. A block whose data this server does not hold is `null`, and
//! the overview still answers.

use std::collections::HashMap;

use axum::Json;
use axum::extract::rejection::{PathRejection, QueryRejection};
use axum::extract::{Path, Query, State};
use serde::Deserialize;
use serde_json::json;
use utoipa::IntoParams;
use wenmar_shopdata::recalls::{
    self as shopdata, MOST_RECALLS, RecallCursor, RecallQuery, RecallSource,
};
use wenmar_vehicles::text::normalize;
use wenmar_vehicles::{CatalogError, Scope, Source, Value};

use crate::api::recalls::{
    LIST_DEFAULT, ModelRef, ModelYear, NO_CATALOG_VEHICLE, as_of, attribution, country, listed,
    lookup, model_slugs, model_year, recall_shop, recalls_of,
};
use crate::api::types::{
    MakeLinks, ModelYearSummary, ModelYears, NamedId, Overview, OverviewDecode, OverviewLinks,
    OverviewRecalls, Recall, RecallAsOf, RecallCampaignVehicle, RecallCount, RecentRecall,
    RecentRecalls, VinDecode, VinRecallCount, Warning,
};
use crate::api::vin::{DecodeQuery, decode_many};
use crate::api::{bad_query, blank_is_none};
use crate::db::Worker;
use crate::error::{ApiError, ErrorBody};
use crate::state::AppState;

/// Campaigns in the `newest` list of an overview's recall block.
pub const NEWEST: usize = 3;

/// Most vehicles given with one campaign of the recent list. The campaign
/// with the most lists 727.
pub const MOST_VEHICLES_EACH: usize = 200;

/// The address of a model year's page on this site.
pub fn model_year_url(base: &str, found: &ModelYear) -> String {
    format!(
        "{base}/makes/{}/{}/{}",
        found.make_slug, found.model_slug, found.vehicle.year
    )
}

/// How many campaigns of a model year's list each country has, and whether
/// any carries an advisory. A country the server holds no recalls for is
/// not counted: its count is `None`.
pub fn tally(recalls: &[Recall], held: &RecallAsOf) -> (RecallCount, bool, bool) {
    let of = |country: &str| {
        let count = recalls
            .iter()
            .filter(|recall| recall.country == country)
            .count();
        u32::try_from(count).unwrap_or(u32::MAX)
    };
    (
        RecallCount {
            us: held.us.as_ref().map(|_| of("us")),
            ca: held.ca.as_ref().map(|_| of("ca")),
        },
        recalls
            .iter()
            .any(|recall| recall.do_not_drive == Some(true)),
        recalls
            .iter()
            .any(|recall| recall.park_outside == Some(true)),
    )
}

/// The recall block of a model year, and the credit its data asks for.
/// `None` when this server holds no recalls: that is not an error here.
pub async fn recall_block(
    state: &AppState,
    found: &ModelYear,
) -> Result<Option<(OverviewRecalls, Vec<String>)>, ApiError> {
    let Ok(shop) = recall_shop(state, None) else {
        return Ok(None);
    };
    let answer = recalls_of(state, shop, found, None).await?;
    let (count, do_not_drive, park_outside) = tally(&answer.recalls, &answer.as_of);
    let url = format!(
        "{}/recalls",
        model_year_url(&state.config().base_url, found)
    );
    let mut newest = answer.recalls;
    newest.truncate(NEWEST);
    Ok(Some((
        OverviewRecalls {
            may_apply: true,
            count,
            as_of: answer.as_of,
            do_not_drive,
            park_outside,
            newest,
            lookup: answer.lookup,
            url,
        },
        answer.attribution,
    )))
}

/// The overview of a model year, or of nothing when `found` is `None`.
///
/// This is the one place a block is filled. A plan that brings a new
/// dataset replaces one `None` below with a call to its reader, and gives
/// the field its own type in `Overview`.
pub async fn overview_of(
    state: &AppState,
    found: Option<&ModelYear>,
    decode: Option<OverviewDecode>,
    warnings: Vec<Warning>,
) -> Result<Overview, ApiError> {
    let (recalls, attribution) = match found {
        Some(found) => match recall_block(state, found).await? {
            Some((block, credit)) => (Some(block), credit),
            None => (None, Vec::new()),
        },
        None => (None, Vec::new()),
    };
    // The maker's links go by the make's name: the catalog's when the model
    // year is known, and otherwise the one the VIN decoded to.
    let make = found
        .map(|found| found.vehicle.make.clone())
        .or_else(|| decode.as_ref().and_then(|decode| decode.make.clone()));
    let links = OverviewLinks {
        page: found.map(|found| model_year_url(&state.config().base_url, found)),
        maker: make
            .as_deref()
            .and_then(crate::site::links::for_make)
            .map(MakeLinks::from),
    };
    // The ratings are an addition: a server without them answers `null`.
    let safety = match (found, state.dataset(wenmar_shopdata::Dataset::Ratings)) {
        (Some(found), Ok(shop)) => {
            let answer = crate::api::safety::safety_of(shop, found).await?;
            Some(serde_json::to_value(answer).map_err(ApiError::internal)?)
        }
        _ => None,
    };
    Ok(Overview {
        vehicle: found.map(|found| found.vehicle.clone()),
        decode,
        specs: None,
        fuel: None,
        fluids: None,
        tires: None,
        recalls,
        safety,
        labor: None,
        bulletins: None,
        links,
        attribution,
        warnings,
    })
}

pub async fn vehicle_overview_op(state: &AppState, id: String) -> Result<Overview, ApiError> {
    let found = state
        .db()
        .run(move |worker| model_year(worker, &id))
        .await??;
    let Some(found) = found else {
        return Err(ApiError::NotFound("No vehicle has that id.".to_owned()));
    };
    overview_of(state, Some(&found), None, Vec::new()).await
}

pub async fn vin_overview_op(
    state: &AppState,
    vin: String,
    year: Option<u16>,
) -> Result<Overview, ApiError> {
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
    if found.is_none() {
        warnings.push(Warning {
            code: "no_catalog_vehicle".to_owned(),
            message: NO_CATALOG_VEHICLE.to_owned(),
            suggestions: Vec::new(),
        });
    }
    let summary = OverviewDecode {
        vin: decode.vin,
        valid: decode.valid,
        year: decode.year,
        make: decode.make,
        model: decode.model,
        trim: decode.trim,
        engine: decode.engine.and_then(|engine| engine.label),
        engine_settled: decode
            .catalog
            .as_ref()
            .is_some_and(|selection| selection.engine_id.is_some()),
        entry_id: decode.catalog.map(|selection| selection.entry.id),
    };
    overview_of(state, found.as_ref(), Some(summary), warnings).await
}

/// What `include` on the batch route asks for: only `recalls` today.
/// `Ok(false)` when it is left out or blank.
pub fn include_recalls(include: Option<&str>) -> Result<bool, ApiError> {
    let mut asked = false;
    for word in include.unwrap_or_default().split(',') {
        match word.trim().to_ascii_lowercase().as_str() {
            "" => {}
            "recalls" => asked = true,
            _ => {
                return Err(ApiError::Validation {
                    message: "include must be recalls".to_owned(),
                    details: json!({ "field": "include", "allowed": ["recalls"] }),
                });
            }
        }
    }
    Ok(asked)
}

/// Adds the recall count to every decode of a batch that reaches a model
/// year of the catalog. A server with no recalls answers 404
/// `dataset_unavailable`, as it does for every recall it is asked for by
/// name.
pub async fn add_recall_counts(
    state: &AppState,
    decoded: &mut [Result<VinDecode, ApiError>],
) -> Result<(), ApiError> {
    let shop = recall_shop(state, None)?;
    let mut ids: Vec<String> = decoded
        .iter()
        .filter_map(|result| result.as_ref().ok())
        .filter_map(|decode| decode.catalog.as_ref())
        .map(|selection| selection.vehicle_id.clone())
        .collect();
    ids.sort_unstable();
    ids.dedup();
    let found: Vec<(String, Option<ModelYear>)> = state
        .db()
        .run(move |worker| {
            ids.into_iter()
                .map(|id| Ok((id.clone(), model_year(worker, &id)?)))
                .collect::<Result<_, CatalogError>>()
        })
        .await??;
    let mut counts: HashMap<String, VinRecallCount> = HashMap::new();
    for (id, found) in found {
        let Some(found) = found else { continue };
        let answer = recalls_of(state, shop, &found, None).await?;
        let (count, do_not_drive, park_outside) = tally(&answer.recalls, &answer.as_of);
        counts.insert(
            id,
            VinRecallCount {
                may_apply: true,
                count,
                do_not_drive,
                park_outside,
                lookup: answer.lookup,
            },
        );
    }
    for decode in decoded.iter_mut().filter_map(|result| result.as_mut().ok()) {
        decode.recalls = decode
            .catalog
            .as_ref()
            .and_then(|selection| counts.get(&selection.vehicle_id).cloned());
    }
    Ok(())
}

/// `?1` make id, `?2` the model as an id form, `?3` its matching form.
const MODEL_SQL: &str = "
SELECT id, slug, name FROM catalog_model
WHERE make_id = ?1 AND (slug = ?2 OR norm = ?3)
ORDER BY (slug = ?2) DESC, id
LIMIT 1";

/// `?1` model id.
const MODEL_YEARS_SQL: &str = "
SELECT DISTINCT year FROM catalog_vehicle
WHERE model_id = ?1
ORDER BY year DESC";

/// A model of the catalog and its model years, newest first.
struct ModelFound {
    make_id: i64,
    make: NamedId,
    model_id: i64,
    model: NamedId,
    years: Vec<u16>,
}

fn model_found(
    worker: &Worker,
    make: &str,
    model: &str,
) -> Result<Option<ModelFound>, CatalogError> {
    let Some(make) = worker.catalog.index().resolve(make, Scope::All) else {
        return Ok(None);
    };
    let rows = worker
        .source
        .query(
            MODEL_SQL,
            &[
                make.id.into(),
                model.trim().into(),
                normalize(model).as_str().into(),
            ],
        )
        .map_err(CatalogError::Source)?;
    let Some((model_id, slug, name)) = rows.first().and_then(|row| {
        Some((
            row.first().and_then(Value::integer)?,
            row.get(1).and_then(Value::text)?,
            row.get(2).and_then(Value::text)?,
        ))
    }) else {
        return Ok(None);
    };
    let years = worker
        .source
        .query(MODEL_YEARS_SQL, &[model_id.into()])
        .map_err(CatalogError::Source)?
        .iter()
        .filter_map(|row| row.first().and_then(Value::integer))
        .filter_map(|year| u16::try_from(year).ok())
        .collect();
    Ok(Some(ModelFound {
        make_id: make.id,
        make: NamedId {
            id: make.slug.clone(),
            name: make.name.clone(),
        },
        model_id,
        model: NamedId {
            id: slug.to_owned(),
            name: name.to_owned(),
        },
        years,
    }))
}

pub async fn model_years_op(
    state: &AppState,
    make: String,
    model: String,
) -> Result<ModelYears, ApiError> {
    let found = state
        .db()
        .run(move |worker| model_found(worker, &make, &model))
        .await??;
    let Some(found) = found else {
        return Err(ApiError::NotFound("No model has that id.".to_owned()));
    };
    let shop = recall_shop(state, None).ok();
    let counts = match shop {
        Some(shop) => {
            let (make_id, model_id) = (found.make_id, found.model_id);
            let counted = shop
                .run(move |reader| reader.recall_counts_by_year(make_id, model_id, None))
                .await??;
            Some(counted)
        }
        None => None,
    };
    let held = shop.map(as_of);
    let base = &state.config().base_url;
    let years = found
        .years
        .iter()
        .map(|year| {
            // A year no campaign lists is zero for each country held.
            let recalls = counts.as_ref().map(|counted| {
                counted
                    .iter()
                    .find(|entry| entry.year == *year)
                    .map(|entry| RecallCount {
                        us: entry.us,
                        ca: entry.ca,
                    })
                    .unwrap_or(RecallCount {
                        us: held.as_ref().and_then(|held| held.us.as_ref()).map(|_| 0),
                        ca: held.as_ref().and_then(|held| held.ca.as_ref()).map(|_| 0),
                    })
            });
            ModelYearSummary {
                id: format!("{year:04}_{}_{}", found.make.id, found.model.id),
                year: *year,
                recalls,
                bulletins: None,
                url: format!("{base}/makes/{}/{}/{year}", found.make.id, found.model.id),
            }
        })
        .collect();
    Ok(ModelYears {
        lookup: lookup(Some(&found.make.name)),
        make: found.make,
        model: found.model,
        may_apply: true,
        as_of: held.unwrap_or(RecallAsOf { us: None, ca: None }),
        years,
        attribution: shop.map(|shop| attribution(shop, None)).unwrap_or_default(),
    })
}

#[derive(Debug, Default, Deserialize, IntoParams)]
#[into_params(parameter_in = Query)]
pub struct RecentQuery {
    /// Only campaigns reported on or after this day, as `2026-09-01`.
    /// Required.
    #[param(example = "2026-09-01")]
    pub since: Option<String>,
    /// `us` or `ca`. Left out: both, where the server holds both.
    pub country: Option<String>,
    /// Only campaigns that list a vehicle of this make: a name, an alias
    /// or an id.
    #[param(example = "ford")]
    pub make: Option<String>,
    /// At most this many, up to 100. The default is 25.
    #[serde(default, deserialize_with = "blank_is_none")]
    #[param(value_type = Option<usize>)]
    pub limit: Option<usize>,
    /// `next_cursor` from the page before, unchanged.
    pub cursor: Option<String>,
}

/// Whether `text` is a day written `2026-09-01`.
fn is_day(text: &str) -> bool {
    text.len() == 10
        && text.bytes().enumerate().all(|(index, byte)| match index {
            4 | 7 => byte == b'-',
            _ => byte.is_ascii_digit(),
        })
}

fn given(value: Option<String>) -> Option<String> {
    value
        .map(|value| value.trim().to_owned())
        .filter(|value| !value.is_empty())
}

fn campaign_vehicle(
    vehicle: shopdata::RecallVehicle,
    slugs: &HashMap<i64, ModelRef>,
) -> RecallCampaignVehicle {
    let model = vehicle.model_id.and_then(|id| slugs.get(&id));
    let vehicle_id = match (vehicle.year, model) {
        (Some(year), Some(found)) => Some(format!("{year:04}_{}_{}", found.make, found.model)),
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
}

pub async fn recent_recalls_op(
    state: &AppState,
    query: RecentQuery,
) -> Result<RecentRecalls, ApiError> {
    let source = country(query.country.as_deref())?;
    let shop = recall_shop(state, source)?;
    let since = match given(query.since) {
        Some(since) if is_day(&since) => since,
        Some(_) => {
            return Err(ApiError::Validation {
                message: "since must be a day written like 2026-09-01".to_owned(),
                details: json!({ "field": "since" }),
            });
        }
        None => {
            return Err(ApiError::Validation {
                message: "since is required".to_owned(),
                details: json!({ "field": "since" }),
            });
        }
    };
    let cursor = match given(query.cursor) {
        None => None,
        Some(text) => Some(
            RecallCursor::parse(&text).ok_or_else(|| ApiError::Validation {
                message: "cursor is not one this API gave out".to_owned(),
                details: json!({ "field": "cursor" }),
            })?,
        ),
    };
    let mut answer = RecentRecalls {
        may_apply: true,
        since: since.clone(),
        as_of: as_of(shop),
        recalls: Vec::new(),
        next_cursor: None,
        lookup: lookup(None),
        attribution: attribution(shop, source),
    };
    // A make the catalog does not have lists nothing, as in `/v1/recalls`.
    let make_id = match given(query.make) {
        None => None,
        Some(make) => {
            let found = state
                .db()
                .run(move |worker| {
                    worker
                        .catalog
                        .index()
                        .resolve(&make, Scope::All)
                        .map(|make| make.id)
                })
                .await?;
            match found {
                Some(id) => Some(id),
                None => return Ok(answer),
            }
        }
    };
    let asked = RecallQuery {
        source,
        make_id,
        since: Some(since),
        limit: query.limit.unwrap_or(LIST_DEFAULT).clamp(1, MOST_RECALLS),
        cursor,
        ..RecallQuery::default()
    };
    let (page, vehicles) = shop
        .run(move |reader| {
            let page = reader.recalls(&asked)?;
            let mut vehicles = Vec::with_capacity(page.recalls.len());
            for recall in &page.recalls {
                let listed = RecallSource::parse(&recall.source)
                    .map(|source| reader.recall(source, &recall.campaign))
                    .transpose()?
                    .flatten()
                    .map(|detail| detail.vehicles)
                    .unwrap_or_default();
                vehicles.push(listed);
            }
            Ok::<_, wenmar_shopdata::ShopError>((page, vehicles))
        })
        .await??;
    let mut model_ids: Vec<i64> = vehicles
        .iter()
        .flat_map(|listed| listed.iter().take(MOST_VEHICLES_EACH))
        .filter_map(|vehicle| vehicle.model_id)
        .collect();
    model_ids.sort_unstable();
    model_ids.dedup();
    let slugs = state
        .db()
        .run(move |worker| model_slugs(worker, &model_ids))
        .await??;
    let base = &state.config().base_url;
    answer.next_cursor = page.next_cursor.map(|cursor| cursor.to_string());
    answer.recalls = page
        .recalls
        .into_iter()
        .zip(vehicles)
        .map(|(recall, vehicles)| RecentRecall {
            recall: listed(base, recall),
            vehicles_listed: u32::try_from(vehicles.len()).unwrap_or(u32::MAX),
            vehicles: vehicles
                .into_iter()
                .take(MOST_VEHICLES_EACH)
                .map(|vehicle| campaign_vehicle(vehicle, &slugs))
                .collect(),
        })
        .collect();
    Ok(answer)
}

/// Everything held for a model year, in one answer.
///
/// A block this server has no data for is `null`. The `recalls` block
/// counts the campaigns that may apply to the year, make and model; it is
/// not a lookup by VIN.
#[utoipa::path(
    get,
    path = "/vehicles/{id}/overview",
    tag = "Vehicles",
    params(
        ("id" = String, Path, description = "A vehicle id. The overview is of the year, make and model; a submodel or an engine in the id is set aside.", example = "2018_ford_f-150")
    ),
    responses(
        (status = 200, description = "The overview. `decode` is `null`.", body = Overview),
        (status = 404, description = "`not_found`: no vehicle has that id.", body = ErrorBody)
    )
)]
pub async fn vehicle_overview(
    State(state): State<AppState>,
    path: Result<Path<String>, PathRejection>,
) -> Result<Json<Overview>, ApiError> {
    let Ok(Path(id)) = path else {
        return Err(ApiError::NotFound("No vehicle has that id.".to_owned()));
    };
    Ok(Json(vehicle_overview_op(&state, id).await?))
}

/// Everything held for the model year a VIN decodes to, in one answer.
///
/// The VIN only names the model year: `recalls` counts the campaigns that
/// may apply to it, and no public data says a recall applies to one VIN. A
/// VIN that decodes to no vehicle in the catalog gets `vehicle: null`,
/// every block `null` and the warning `no_catalog_vehicle`, not an error.
#[utoipa::path(
    get,
    path = "/vin/{vin}/overview",
    tag = "VIN",
    params(
        ("vin" = String, Path, description = "A 17-character VIN.", example = "KM8K2CAB4PU001140"),
        DecodeQuery
    ),
    responses(
        (status = 200, description = "The overview, with `decode`.", body = Overview),
        (status = 400, description = "`invalid_vin`, or `validation_failed` for `year`.", body = ErrorBody),
        (status = 404, description = "`not_found`: no manufacturer is registered for the first three characters.", body = ErrorBody)
    )
)]
pub async fn vin_overview(
    State(state): State<AppState>,
    path: Result<Path<String>, PathRejection>,
    query: Result<Query<DecodeQuery>, QueryRejection>,
) -> Result<Json<Overview>, ApiError> {
    let Ok(Path(vin)) = path else {
        return Err(ApiError::InvalidVin {
            message: "a VIN uses only digits and letters other than I, O and Q".to_owned(),
            details: json!({ "suggestions": [] }),
        });
    };
    let Query(query) = query.map_err(bad_query)?;
    Ok(Json(vin_overview_op(&state, vin, query.year).await?))
}

/// Every model year of one model, newest first, each with its vehicle id
/// and the number of recall campaigns that may apply to it.
#[utoipa::path(
    get,
    path = "/models/{make}/{model}",
    tag = "Vehicles",
    params(
        ("make" = String, Path, description = "A make's id, name or alias.", example = "ford"),
        ("model" = String, Path, description = "A model's id or name.", example = "f-150")
    ),
    responses(
        (status = 200, description = "The model years. `recalls` is `null` in each when this server holds no recalls.", body = ModelYears),
        (status = 404, description = "`not_found`: no model has that id.", body = ErrorBody)
    )
)]
pub async fn model_years(
    State(state): State<AppState>,
    path: Result<Path<(String, String)>, PathRejection>,
) -> Result<Json<ModelYears>, ApiError> {
    let Ok(Path((make, model))) = path else {
        return Err(ApiError::NotFound("No model has that id.".to_owned()));
    };
    Ok(Json(model_years_op(&state, make, model).await?))
}

/// Recall campaigns reported on or after a day, newest first, each with
/// the vehicles it lists.
///
/// For an application that keeps a list of vehicles: ask once a day with
/// the day of the last answer, and compare `vehicles[].vehicle_id` with
/// its own. Each campaign may apply to the vehicles it lists.
#[utoipa::path(
    get,
    path = "/recalls/recent",
    tag = "Recalls",
    params(RecentQuery),
    responses(
        (status = 200, description = "One page of campaigns.", body = RecentRecalls),
        (status = 400, description = "`validation_failed`: `since` is missing or is not a day, or `country`, `limit` or `cursor` is wrong.", body = ErrorBody),
        (status = 404, description = "`dataset_unavailable`: this server holds no recalls, or none for the country asked for.", body = ErrorBody)
    )
)]
pub async fn recent_recalls(
    State(state): State<AppState>,
    query: Result<Query<RecentQuery>, QueryRejection>,
) -> Result<Json<RecentRecalls>, ApiError> {
    let Query(query) = query.map_err(bad_query)?;
    Ok(Json(recent_recalls_op(&state, query).await?))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn recall(country: &str, do_not_drive: Option<bool>, park_outside: Option<bool>) -> Recall {
        Recall {
            source: if country == "us" { "nhtsa" } else { "tc" }.to_owned(),
            country: country.to_owned(),
            campaign: "X".to_owned(),
            date: None,
            manufacturer: None,
            component: None,
            summary: None,
            mfr_campaign: None,
            units: None,
            do_not_drive,
            park_outside,
            notification: None,
            matched: None,
            models: Vec::new(),
            built_from: None,
            built_to: None,
            url: String::new(),
        }
    }

    #[test]
    fn a_country_not_held_is_not_counted_and_one_held_may_be_zero() {
        let day = || Some("2026-10-11".to_owned());
        let list = [
            recall("us", Some(false), Some(true)),
            recall("us", Some(false), Some(false)),
            recall("ca", None, None),
        ];
        let both = RecallAsOf {
            us: day(),
            ca: day(),
        };
        assert_eq!(
            tally(&list, &both),
            (
                RecallCount {
                    us: Some(2),
                    ca: Some(1)
                },
                false,
                true
            )
        );
        assert_eq!(
            tally(&[], &both),
            (
                RecallCount {
                    us: Some(0),
                    ca: Some(0)
                },
                false,
                false
            )
        );
        let american = RecallAsOf {
            us: day(),
            ca: None,
        };
        assert_eq!(
            tally(&list[..2], &american).0,
            RecallCount {
                us: Some(2),
                ca: None
            }
        );
        assert!(tally(&[recall("us", Some(true), None)], &american).1);
    }

    #[test]
    fn include_reads_recalls_and_refuses_anything_else() {
        assert_eq!(include_recalls(None), Ok(false));
        assert_eq!(include_recalls(Some("  ")), Ok(false));
        assert_eq!(include_recalls(Some("recalls")), Ok(true));
        assert_eq!(include_recalls(Some(" Recalls, ")), Ok(true));
        for bad in ["bulletins", "recalls,fuel", "all"] {
            let error = include_recalls(Some(bad)).unwrap_err();
            assert_eq!(error.body().error.code, "validation_failed", "{bad}");
            assert_eq!(error.body().error.details["field"], "include", "{bad}");
        }
    }

    #[test]
    fn a_day_is_ten_characters_of_digits_and_dashes() {
        for good in ["2026-09-01", "1968-03-19"] {
            assert!(is_day(good), "{good}");
        }
        for bad in [
            "",
            "2026-9-1",
            "2026/09/01",
            "20260901",
            "2026-09-01T00",
            "soon",
        ] {
            assert!(!is_day(bad), "{bad}");
        }
    }
}
