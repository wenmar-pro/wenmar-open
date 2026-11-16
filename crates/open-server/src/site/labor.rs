//! The job list and one job: what each covers and what it does not.

use askama::Template;
use axum::extract::rejection::PathRejection;
use axum::extract::{Path, State};
use axum::http::{StatusCode, header};
use axum::response::{IntoResponse, Response};
use wenmar_shopdata::Dataset;
use wenmar_vehicles::text::is_slug;

use crate::api::labor::{job_op, jobs_op};
use crate::api::types::{JobDetail, JobLink};
use crate::error::ApiError;
use crate::site::markdown::{self, Format, escape};
use crate::site::{self, Page, jsonld, report, seo};
use crate::state::AppState;

// ----- /jobs -----

#[derive(Template)]
#[template(path = "jobs.html")]
struct JobsPage {
    page: Page,
    jobs: Vec<JobLink>,
    dated: String,
}

/// `/jobs`: the job list.
pub async fn index(State(state): State<AppState>) -> Response {
    let list = match jobs_op(&state).await {
        Ok(list) => list,
        Err(ApiError::DatasetUnavailable(_)) => return site::not_found(&state),
        Err(error) => return site::failed(&state, &error),
    };
    let page = Page::new(
        &state,
        seo::title("Repair jobs"),
        "The repair jobs of Wenmar's list: what each includes and excludes.",
    )
    .with_markdown("/jobs.md")
    .with_json("/v1/jobs")
    .describing(vec![jsonld::organization(&state.config().base_url)])
    .indexed(&state, "/jobs");
    site::html(
        StatusCode::OK,
        &JobsPage {
            page,
            dated: list
                .as_of
                .map(|day| format!("as of {day}"))
                .unwrap_or_default(),
            jobs: list.jobs,
        },
    )
}

/// `/jobs.md`.
pub async fn index_md(State(state): State<AppState>) -> Response {
    let list = match jobs_op(&state).await {
        Ok(list) => list,
        Err(ApiError::DatasetUnavailable(_)) => return site::not_found(&state),
        Err(error) => return site::failed(&state, &error),
    };
    let mut text =
        "# Repair jobs\n\nThe repair jobs of Wenmar's list: what each includes and excludes.\n\n"
            .to_owned();
    for job in &list.jobs {
        text.push_str(&format!("- [{}]({})\n", escape(&job.name), job.url));
    }
    markdown::response(text, &format!("{}/jobs", state.config().base_url))
}

fn moved(to: &str) -> Response {
    (StatusCode::MOVED_PERMANENTLY, [(header::LOCATION, to)]).into_response()
}

/// The old "how estimates are made" page now sends people to the job list.
pub async fn old_method() -> Response {
    moved("/jobs")
}

// ----- one job -----

#[derive(Template)]
#[template(path = "job.html")]
struct JobPage {
    page: Page,
    job: JobDetail,
    /// Code and title of the trouble codes about the job's components.
    codes: Vec<(String, String)>,
    report: report::ReportLinks,
}

fn job_markdown(job: &JobDetail, base: &str) -> String {
    let mut text = format!("# {}\n", escape(&job.name));
    for (heading, entries) in [
        ("Includes", &job.includes),
        ("Does not include", &job.excludes),
    ] {
        if !entries.is_empty() {
            text.push_str(&format!("\n## {heading}\n\n"));
            for entry in entries {
                text.push_str(&format!("- {}\n", escape(entry)));
            }
        }
    }
    text.push_str("\n## About this job\n\n");
    text.push_str(&format!("- System: {}\n", escape(&job.system_name)));
    if !job.components.is_empty() {
        text.push_str(&format!(
            "- Components: {}\n",
            job.components
                .iter()
                .map(|component| escape(&component.name))
                .collect::<Vec<_>>()
                .join(", ")
        ));
    }
    if !job.related.is_empty() {
        text.push_str(&format!(
            "- Related jobs: {}\n",
            job.related
                .iter()
                .map(|other| format!("[{}]({})", escape(&other.name), other.url))
                .collect::<Vec<_>>()
                .join(", ")
        ));
    }
    text.push_str(&format!("\n[Every job]({base}/jobs.md)\n"));
    text
}

/// `/jobs/{job}` and its Markdown twin.
pub async fn job(
    State(state): State<AppState>,
    path: Result<Path<String>, PathRejection>,
) -> Response {
    let Ok(Path(segment)) = path else {
        return site::not_found(&state);
    };
    let (stem, format) = markdown::split(&segment);
    if !is_slug(stem) {
        return site::not_found(&state);
    }
    let found = match job_op(&state, stem.to_owned()).await {
        Ok(found) => found,
        Err(ApiError::NotFound(_) | ApiError::DatasetUnavailable(_)) => {
            return site::not_found(&state);
        }
        Err(error) => return site::failed(&state, &error),
    };
    let path = format!("/jobs/{}", found.id);
    let base = &state.config().base_url;
    if format == Format::Markdown {
        let canonical = format!("{base}{path}");
        return markdown::response(job_markdown(&found, base), &canonical);
    }
    let components: Vec<String> = found
        .components
        .iter()
        .map(|component| component.id.clone())
        .collect();
    let codes: Vec<(String, String)> = match state.dataset(Dataset::Codes) {
        Ok(shop) if !components.is_empty() => {
            match shop
                .run(move |reader| reader.codes_for_components(&components, 20))
                .await
            {
                Ok(Ok(codes)) => codes
                    .into_iter()
                    .map(|code| (code.code, code.title))
                    .collect(),
                _ => Vec::new(),
            }
        }
        _ => Vec::new(),
    };
    let description = seo::description(
        &[&[format!(
            "{}: what the job includes and excludes.",
            found.name
        )]],
        160,
    );
    let page = Page::new(&state, seo::title(&found.name), description)
        .with_markdown(&format!("{path}.md"))
        .with_json(&format!("/v1{path}"))
        .under(vec![("Jobs".to_owned(), "/jobs".to_owned())]);
    let trail = page.trail(&found.name);
    let page = page.describing(vec![trail]).indexed(&state, &path);
    let report = report::links(state.config(), &path, &found.name);
    site::html(
        StatusCode::OK,
        &JobPage {
            page,
            codes,
            job: found,
            report,
        },
    )
}

/// The old vehicle-job page now sends people to the job's page.
pub async fn old_vehicle_job(
    Path((_make, _model, _year, job)): Path<(String, String, String, String)>,
) -> Response {
    moved(&format!("/jobs/{job}"))
}

/// A request for the old labour suggestion form is a request for feedback.
pub async fn old_feedback() -> Response {
    moved("/feedback")
}

/// The old form no longer accepts a suggestion: there are no hours to send.
pub async fn old_feedback_post() -> StatusCode {
    StatusCode::NOT_FOUND
}
