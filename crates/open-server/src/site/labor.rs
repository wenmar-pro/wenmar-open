//! The labour pages: `/jobs`, one job, and a job on a model year.
//!
//! A page shows Wenmar's own estimate of the hours a job takes and says
//! what the figure is for. It shows a price only from a rate the visitor
//! typed. An estimate is not a quote.

use askama::Template;
use axum::extract::rejection::{PathRejection, QueryRejection};
use axum::extract::{Path, Query, State};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Redirect, Response};
use serde::Deserialize;
use shop_math::Hours;
use wenmar_shopdata::Dataset;
use wenmar_vehicles::text::{is_slug, slug, squeeze};

use crate::api::labor::{job_op, jobs_op};
use crate::api::recalls::model_year;
use crate::api::types::{JobDetail, JobLink, LaborFigure};
use crate::error::ApiError;
use crate::site::markdown::{self, Format, escape};
use crate::site::{self, Page, report, seo};
use crate::state::AppState;

/// Hours as a page shows them: `98`, `97.5`, `99.25`.
pub fn shown(hours: f64) -> String {
    Hours::from_hundredths((hours * 100.0).round() as i64).to_string()
}

/// What stands behind a figure, in words.
pub fn basis_words(figure: &LaborFigure) -> String {
    match &figure.sample {
        Some(sample) => format!(
            "measured from {} repair orders at {} shops",
            sample.jobs, sample.shops
        ),
        None => "written by a person who does this work".to_owned(),
    }
}

/// A figure in one line: `97.5 to 99.25 hours, typically 98 (…)`.
pub fn range_words(figure: &LaborFigure) -> String {
    format!(
        "{} to {} hours, typically {} ({})",
        shown(figure.hours.low),
        shown(figure.hours.high),
        shown(figure.hours.typical),
        basis_words(figure)
    )
}

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
        seo::title("Repair jobs and labour times"),
        "How long common repair jobs take, estimated by Wenmar: what each job includes, and its typical hours by class of vehicle.",
    )
    .with_markdown("/jobs.md")
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
    let mut text = "# Repair jobs and labour times\n\nHow long a repair job takes, as estimated by Wenmar: written by people who do the work, and never taken from a labour guide.\n\n".to_owned();
    for job in &list.jobs {
        text.push_str(&format!("- [{}]({})\n", escape(&job.name), job.url));
    }
    markdown::response(text, &format!("{}/jobs", state.config().base_url))
}

// ----- one job -----

/// The vehicle a visitor asked a job's page for.
#[derive(Debug, Default, Deserialize)]
pub struct PickQuery {
    year: Option<String>,
    make: Option<String>,
    model: Option<String>,
}

#[derive(Template)]
#[template(path = "job.html")]
struct JobPage {
    page: Page,
    job: JobDetail,
    /// Class name and the figure in words.
    classes: Vec<(String, String)>,
    labor_held: bool,
    /// The vehicle asked for, when the catalog does not have it.
    missing: Option<String>,
    /// Code and title of the trouble codes about the job's components.
    codes: Vec<(String, String)>,
    report: report::ReportLinks,
}

fn job_markdown(job: &JobDetail) -> String {
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
    let figures = job.estimates.as_deref().unwrap_or_default();
    if !figures.is_empty() {
        text.push_str("\n## Typical hours by class of vehicle\n\n");
        for figure in figures {
            text.push_str(&format!(
                "- {}: {}\n",
                escape(figure.class_name.as_deref().unwrap_or_default()),
                escape(&range_words(figure))
            ));
        }
        text.push_str(&format!("\n{}\n", escape(&job.note)));
        for credit in &job.attribution {
            text.push_str(&format!("\n{}.\n", escape(credit)));
        }
    }
    text
}

/// `/jobs/{job}` and its Markdown twin. With `?year=&make=&model=` it
/// sends the visitor to that vehicle's labour page.
pub async fn job(
    State(state): State<AppState>,
    path: Result<Path<String>, PathRejection>,
    query: Result<Query<PickQuery>, QueryRejection>,
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
    if format == Format::Markdown {
        let canonical = format!("{}{path}", state.config().base_url);
        return markdown::response(job_markdown(&found), &canonical);
    }
    let labor_held = state.dataset(Dataset::Labor).is_ok();
    // A vehicle was asked for: go to its page, or say it is not known.
    let asked = query.ok().map(|Query(query)| query).unwrap_or_default();
    let mut missing = None;
    if let (Some(year), Some(make), Some(model)) = (&asked.year, &asked.make, &asked.model) {
        let (make, model) = (squeeze(make), squeeze(model));
        let id = site::year_in(year.trim())
            .map(|year| format!("{year:04}_{}_{}", slug(&make), slug(&model)));
        let vehicle = match id {
            Some(id) if labor_held => {
                match state.db().run(move |worker| model_year(worker, &id)).await {
                    Ok(Ok(vehicle)) => vehicle,
                    _ => None,
                }
            }
            _ => None,
        };
        match vehicle {
            Some(vehicle) => {
                return Redirect::to(&format!(
                    "/makes/{}/{}/{}/labor/{}",
                    vehicle.make_slug, vehicle.model_slug, vehicle.vehicle.year, found.id
                ))
                .into_response();
            }
            None => {
                let year: String = year.trim().chars().take(4).collect();
                missing = Some(squeeze(&format!("{year} {make} {model}")));
            }
        }
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
        &[&[
            format!(
                "{}: what the job includes, and how long it typically takes by class of vehicle.",
                found.name
            ),
            format!("{}: what the job includes.", found.name),
        ]],
        160,
    );
    let page = Page::new(&state, seo::title(&found.name), description)
        .with_markdown(&format!("{path}.md"))
        .under(vec![("Jobs".to_owned(), "/jobs".to_owned())]);
    let trail = page.trail(&found.name);
    let page = page.describing(vec![trail]).indexed(&state, &path);
    let page = if missing.is_some() {
        page.with_query()
    } else {
        page
    };
    let report = report::links(state.config(), &path, &found.name);
    let classes = found
        .estimates
        .iter()
        .flatten()
        .map(|figure| {
            (
                figure.class_name.clone().unwrap_or_default(),
                range_words(figure),
            )
        })
        .collect();
    site::html(
        StatusCode::OK,
        &JobPage {
            page,
            classes,
            labor_held,
            missing,
            codes,
            job: found,
            report,
        },
    )
}
