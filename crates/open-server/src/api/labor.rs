//! The list of repair jobs: what each covers and what it does not.
//!
//! Each route is a thin wrapper around a function ending in `_op`, as in
//! `recalls.rs`.

use axum::Json;
use axum::extract::rejection::PathRejection;
use axum::extract::{Path, State};
use wenmar_shopdata::{Dataset, jobs};
use wenmar_vehicles::text::is_slug;

use crate::api::types::{JobComponent, JobDetail, JobLink, JobList};
use crate::error::{ApiError, ErrorBody};
use crate::state::AppState;

/// `not_found` for an id that is not in the job list.
pub const NO_JOB: &str = "No job has that id.";

/// The address of a job's page on this site.
pub fn job_url(base: &str, id: &str) -> String {
    format!("{base}/jobs/{id}")
}

fn link(base: &str, job: jobs::JobSummary) -> JobLink {
    JobLink {
        url: job_url(base, &job.id),
        id: job.id,
        name: job.name,
        system: job.system,
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
    let job = shop.run(move |reader| reader.job(&id)).await??;
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
    })
}

/// The repair jobs of Wenmar's list, in the order of the list. A job's id
/// never changes.
#[utoipa::path(
    get,
    path = "/jobs",
    tag = "Jobs",
    responses(
        (status = 200, description = "Every job.", body = JobList),
        (status = 404, description = "`dataset_unavailable`: this server holds no job list.", body = ErrorBody)
    )
)]
pub async fn jobs(State(state): State<AppState>) -> Result<Json<JobList>, ApiError> {
    Ok(Json(jobs_op(&state).await?))
}

/// One repair job: what it covers and what it does not.
#[utoipa::path(
    get,
    path = "/jobs/{job}",
    tag = "Jobs",
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
