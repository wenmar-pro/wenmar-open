//! The job list and Wenmar's labour estimates: the hours a job takes on a
//! vehicle, with the scope that answered and what stands behind it.
//!
//! An estimate is not a quote, and no answer here holds a price.
//!
//! Each route is a thin wrapper around a function ending in `_op`, as in
//! `recalls.rs`.

use axum::Json;
use axum::extract::rejection::{PathRejection, QueryRejection};
use axum::extract::{Path, Query, State};
pub use wenmar_shopdata::labor::METHOD_PATH;
use wenmar_shopdata::labor::{self as shopdata, NOTE, class_name};
use wenmar_shopdata::{Dataset, ShopError, jobs};
use wenmar_vehicles::VehicleId;
use wenmar_vehicles::text::is_slug;

use crate::api::bad_query;
use crate::api::fuel::VinYearQuery;
use crate::api::recalls::{ModelYear, model_year};
use crate::api::types::{
    JobComponent, JobDetail, JobLink, JobList, LaborClass, LaborFigure, LaborHours, LaborSample,
    LaborYears, RecallDecode, VehicleLabor, VinLabor, Warning,
};
use crate::api::vin::decode_many;
use crate::error::{ApiError, ErrorBody};
use crate::shop::ShopDb;
use crate::state::AppState;

/// `not_found` for an id that is not in the job list.
pub const NO_JOB: &str = "No job has that id.";

/// What a VIN's labour answer says when the VIN names no model year.
pub const NO_CATALOG_VEHICLE: &str = "This VIN does not decode to a year, make and model in the catalog, so no labour estimates are listed for it.";

/// The address of a job's page on this site.
pub fn job_url(base: &str, id: &str) -> String {
    format!("{base}/jobs/{id}")
}

/// The credit line of the labour dataset.
pub fn attribution(shop: &ShopDb) -> Vec<String> {
    shop.datasets()
        .get(Dataset::Labor)
        .map(|entry| entry.attribution.clone())
        .filter(|credit| !credit.is_empty())
        .into_iter()
        .collect()
}

fn link(base: &str, job: jobs::JobSummary) -> JobLink {
    JobLink {
        url: job_url(base, &job.id),
        id: job.id,
        name: job.name,
        system: job.system,
    }
}

/// Whole hundredths of an hour as hours.
pub fn hours(hundredths: i64) -> f64 {
    hundredths as f64 / 100.0
}

/// An estimate as the API and the pages give it.
pub fn figure(base: &str, found: shopdata::LaborEstimate) -> LaborFigure {
    let inputs = (found.basis == "ai").then(|| found.inputs.clone());
    LaborFigure {
        inputs,
        url: job_url(base, &found.job_id),
        job: found.job_id,
        name: found.job_name,
        hours: LaborHours {
            low: hours(found.hours_low),
            typical: hours(found.hours_typical),
            high: hours(found.hours_high),
        },
        basis: found.basis,
        scope: found.scope,
        class_name: found
            .class
            .as_deref()
            .and_then(class_name)
            .map(str::to_owned),
        class: found.class,
        years: match (found.year_from, found.year_to) {
            (Some(from), Some(to)) => Some(LaborYears { from, to }),
            _ => None,
        },
        engine: found.engine,
        sample: match (found.sample_jobs, found.sample_shops) {
            (Some(jobs), Some(shops)) => Some(LaborSample { jobs, shops }),
            _ => None,
        },
        confidence_pct: found.confidence_pct,
        model: found.model,
        method: found.method,
        notes: found.notes,
        method_url: Some(format!("{base}{METHOD_PATH}")),
        updated: found.updated,
    }
}

pub async fn jobs_op(state: &AppState) -> Result<JobList, ApiError> {
    let shop = state.dataset(Dataset::Jobs)?;
    let found = shop.run(|reader| reader.jobs()).await??;
    let base = &state.config().base_url;
    Ok(JobList {
        as_of: shop
            .datasets()
            .get(Dataset::Jobs)
            .map(|entry| entry.as_of.clone()),
        jobs: found.into_iter().map(|job| link(base, job)).collect(),
        attribution: Vec::new(),
    })
}

pub async fn job_op(state: &AppState, id: String) -> Result<JobDetail, ApiError> {
    let shop = state.dataset(Dataset::Jobs)?;
    if !is_slug(&id) {
        return Err(ApiError::NotFound(NO_JOB.to_owned()));
    }
    let held = shop.datasets().contains(Dataset::Labor);
    let (job, estimates) = shop
        .run(move |reader| {
            let job = reader.job(&id)?;
            let estimates = match &job {
                Some(job) if held => Some(reader.class_estimates_for_job(&job.id)?),
                _ => None,
            };
            Ok::<_, ShopError>((job, estimates))
        })
        .await??;
    let Some(job) = job else {
        return Err(ApiError::NotFound(NO_JOB.to_owned()));
    };
    let base = &state.config().base_url;
    Ok(JobDetail {
        url: job_url(base, &job.id),
        id: job.id,
        name: job.name,
        system: job.system,
        system_name: job.system_name,
        components: job
            .components
            .into_iter()
            .map(|component| JobComponent {
                id: component.id,
                name: component.name,
                parent: component.parent,
            })
            .collect(),
        includes: job.includes,
        excludes: job.excludes,
        related: job
            .related
            .into_iter()
            .map(|other| link(base, other))
            .collect(),
        estimates: estimates
            .map(|list| list.into_iter().map(|found| figure(base, found)).collect()),
        note: NOTE.to_owned(),
        attribution: if held { attribution(shop) } else { Vec::new() },
    })
}

/// The engine a vehicle id names, as an id form.
pub fn engine_of(id: &str) -> Option<String> {
    VehicleId::parse(id.trim()).and_then(|parsed| parsed.engine)
}

/// The labour estimates of one model year, as the API and the pages give
/// them. `shop` is the shop file, known to hold them. `Ok(None)` when
/// `job` is given and is not in the job list.
pub async fn labor_of(
    state: &AppState,
    shop: &ShopDb,
    found: &ModelYear,
    engine: Option<String>,
    job: Option<String>,
) -> Result<Option<VehicleLabor>, ApiError> {
    let (model_id, year) = (found.model_id, found.vehicle.year);
    let asked_engine = engine.clone();
    let answer = shop
        .run(move |reader| {
            if let Some(job) = &job
                && (!is_slug(job) || reader.job(job)?.is_none())
            {
                return Ok::<_, ShopError>(None);
            }
            let class = reader.vehicle_class(model_id, year)?;
            let estimates =
                reader.labor_for_vehicle(model_id, year, engine.as_deref(), job.as_deref())?;
            Ok(Some((class, estimates)))
        })
        .await??;
    let Some((class, estimates)) = answer else {
        return Ok(None);
    };
    let base = &state.config().base_url;
    Ok(Some(VehicleLabor {
        vehicle: found.vehicle.clone(),
        engine: asked_engine,
        class: class.map(|(id, source)| LaborClass {
            name: class_name(&id).unwrap_or(id.as_str()).to_owned(),
            id,
            source,
        }),
        as_of: shop
            .datasets()
            .get(Dataset::Labor)
            .map(|entry| entry.as_of.clone()),
        estimates: estimates
            .into_iter()
            .map(|found| figure(base, found))
            .collect(),
        note: NOTE.to_owned(),
        attribution: attribution(shop),
    }))
}

pub async fn vehicle_labor_op(
    state: &AppState,
    id: String,
    job: Option<String>,
) -> Result<VehicleLabor, ApiError> {
    let shop = state.dataset(Dataset::Labor)?;
    let engine = engine_of(&id);
    let found = state
        .db()
        .run(move |worker| model_year(worker, &id))
        .await??;
    let Some(found) = found else {
        return Err(ApiError::NotFound("No vehicle has that id.".to_owned()));
    };
    labor_of(state, shop, &found, engine, job)
        .await?
        .ok_or_else(|| ApiError::NotFound(NO_JOB.to_owned()))
}

pub async fn vin_labor_op(
    state: &AppState,
    vin: String,
    year: Option<u16>,
    job: Option<String>,
) -> Result<VinLabor, ApiError> {
    let shop = state.dataset(Dataset::Labor)?;
    let mut decoded = decode_many(state, vec![vin], year).await?;
    let decode = match decoded.pop() {
        Some(result) => result?,
        None => return Err(ApiError::internal("a decode gave no answer")),
    };
    let vehicle_id = decode
        .catalog
        .as_ref()
        .map(|selection| selection.vehicle_id.clone());
    let engine = vehicle_id.as_deref().and_then(engine_of);
    let found = match vehicle_id {
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
        // The job is still judged, so that a wrong id is told either way.
        if let Some(job) = job {
            let known = is_slug(&job)
                && shop
                    .run(move |reader| reader.job(&job).map(|found| found.is_some()))
                    .await??;
            if !known {
                return Err(ApiError::NotFound(NO_JOB.to_owned()));
            }
        }
        warnings.push(Warning {
            code: "no_catalog_vehicle".to_owned(),
            message: NO_CATALOG_VEHICLE.to_owned(),
            suggestions: Vec::new(),
        });
        return Ok(VinLabor {
            decode: summary,
            vehicle: None,
            engine: None,
            class: None,
            as_of: shop
                .datasets()
                .get(Dataset::Labor)
                .map(|entry| entry.as_of.clone()),
            estimates: Vec::new(),
            note: NOTE.to_owned(),
            attribution: attribution(shop),
            warnings,
        });
    };
    let Some(answer) = labor_of(state, shop, &found, engine, job).await? else {
        return Err(ApiError::NotFound(NO_JOB.to_owned()));
    };
    Ok(VinLabor {
        decode: summary,
        vehicle: Some(answer.vehicle),
        engine: answer.engine,
        class: answer.class,
        as_of: answer.as_of,
        estimates: answer.estimates,
        note: answer.note,
        attribution: answer.attribution,
        warnings,
    })
}

/// The repair jobs a labour estimate can be about, in the order of the
/// list. A job's id never changes.
#[utoipa::path(
    get,
    path = "/jobs",
    tag = "Labour",
    responses(
        (status = 200, description = "Every job.", body = JobList),
        (status = 404, description = "`dataset_unavailable`: this server holds no job list.", body = ErrorBody)
    )
)]
pub async fn jobs(State(state): State<AppState>) -> Result<Json<JobList>, ApiError> {
    Ok(Json(jobs_op(&state).await?))
}

/// One repair job: what it covers, what it does not, and what it
/// typically takes by class of vehicle.
#[utoipa::path(
    get,
    path = "/jobs/{job}",
    tag = "Labour",
    params(("job" = String, Path, description = "A job id.", example = "brake-pads-front")),
    responses(
        (status = 200, description = "The job.", body = JobDetail),
        (status = 404, description = "`not_found`: no job has that id. `dataset_unavailable`: this server holds no job list.", body = ErrorBody)
    )
)]
pub async fn job(
    State(state): State<AppState>,
    path: Result<Path<String>, PathRejection>,
) -> Result<Json<JobDetail>, ApiError> {
    let Ok(Path(id)) = path else {
        return Err(ApiError::NotFound(NO_JOB.to_owned()));
    };
    Ok(Json(job_op(&state, id).await?))
}

/// Labour estimates for a model year: for each job, the hours of the
/// narrowest scope that has a figure, and which scope that was.
///
/// An estimate, not a quote. `scope` says what a figure is for: `class` is
/// typical for vehicles of that class and was not worked out on this one.
/// An id that ends with an engine asks for that engine.
#[utoipa::path(
    get,
    path = "/vehicles/{id}/labor",
    tag = "Labour",
    params(("id" = String, Path, description = "A vehicle id. An engine in the id narrows the answer to it.", example = "2018_ford_f-150")),
    responses(
        (status = 200, description = "One estimate for each job that has a figure covering the vehicle.", body = VehicleLabor),
        (status = 404, description = "`not_found`: no vehicle has that id. `dataset_unavailable`: this server holds no labour estimates.", body = ErrorBody)
    )
)]
pub async fn vehicle_labor(
    State(state): State<AppState>,
    path: Result<Path<String>, PathRejection>,
) -> Result<Json<VehicleLabor>, ApiError> {
    let Ok(Path(id)) = path else {
        return Err(ApiError::NotFound("No vehicle has that id.".to_owned()));
    };
    Ok(Json(vehicle_labor_op(&state, id, None).await?))
}

/// The labour estimate of one job for a model year. `estimates` has one
/// entry, or none when no figure covers the vehicle.
#[utoipa::path(
    get,
    path = "/vehicles/{id}/labor/{job}",
    tag = "Labour",
    params(
        ("id" = String, Path, description = "A vehicle id.", example = "2018_ford_f-150"),
        ("job" = String, Path, description = "A job id.", example = "brake-pads-front")
    ),
    responses(
        (status = 200, description = "The estimate, when there is one.", body = VehicleLabor),
        (status = 404, description = "`not_found`: no vehicle, or no job, has that id. `dataset_unavailable`: this server holds no labour estimates.", body = ErrorBody)
    )
)]
pub async fn vehicle_labor_job(
    State(state): State<AppState>,
    path: Result<Path<(String, String)>, PathRejection>,
) -> Result<Json<VehicleLabor>, ApiError> {
    let Ok(Path((id, job))) = path else {
        return Err(ApiError::NotFound("No vehicle has that id.".to_owned()));
    };
    Ok(Json(vehicle_labor_op(&state, id, Some(job)).await?))
}

/// Labour estimates for the vehicle a VIN decodes to. When the decode
/// settles the engine, an engine-scope figure can answer.
#[utoipa::path(
    get,
    path = "/vin/{vin}/labor",
    tag = "Labour",
    params(("vin" = String, Path, description = "A 17-character VIN.", example = "KM8K2CAB4PU001140"), VinYearQuery),
    responses(
        (status = 200, description = "The estimates. `vehicle` is `null` and `estimates` empty when the VIN reaches no catalog vehicle.", body = VinLabor),
        (status = 400, description = "`invalid_vin`.", body = ErrorBody),
        (status = 404, description = "`dataset_unavailable`: this server holds no labour estimates.", body = ErrorBody)
    )
)]
pub async fn vin_labor(
    State(state): State<AppState>,
    Path(vin): Path<String>,
    query: Result<Query<VinYearQuery>, QueryRejection>,
) -> Result<Json<VinLabor>, ApiError> {
    let Query(query) = query.map_err(bad_query)?;
    Ok(Json(vin_labor_op(&state, vin, query.year, None).await?))
}

/// The labour estimate of one job for the vehicle a VIN decodes to.
#[utoipa::path(
    get,
    path = "/vin/{vin}/labor/{job}",
    tag = "Labour",
    params(
        ("vin" = String, Path, description = "A 17-character VIN.", example = "KM8K2CAB4PU001140"),
        ("job" = String, Path, description = "A job id.", example = "brake-pads-front"),
        VinYearQuery
    ),
    responses(
        (status = 200, description = "The estimate, when there is one.", body = VinLabor),
        (status = 400, description = "`invalid_vin`.", body = ErrorBody),
        (status = 404, description = "`not_found`: no job has that id. `dataset_unavailable`: this server holds no labour estimates.", body = ErrorBody)
    )
)]
pub async fn vin_labor_job(
    State(state): State<AppState>,
    Path((vin, job)): Path<(String, String)>,
    query: Result<Query<VinYearQuery>, QueryRejection>,
) -> Result<Json<VinLabor>, ApiError> {
    let Query(query) = query.map_err(bad_query)?;
    Ok(Json(
        vin_labor_op(&state, vin, query.year, Some(job)).await?,
    ))
}
