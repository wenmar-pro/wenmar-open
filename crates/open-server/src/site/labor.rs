//! The labour pages: `/jobs`, one job, and a job on a model year.
//!
//! A page shows Wenmar's own estimate of the hours a job takes and says
//! what the figure is for. It shows a price only from a rate the visitor
//! typed. An estimate is not a quote.

use askama::Template;
use axum::extract::rejection::{PathRejection, QueryRejection};
use axum::extract::{Path, Query, State};
use axum::http::{StatusCode, Uri};
use axum::response::{IntoResponse, Redirect, Response};
use serde::Deserialize;
use serde_json::json;
use shop_math::labor_rate::labor_charge;
use shop_math::{Hours, Money};
use wenmar_shopdata::Dataset;
use wenmar_shopdata::labor::{
    AI_MOST_CONFIDENCE, AI_MOST_HUNDREDTHS, AI_MOST_SPREAD, ATTRIBUTION, LEAST_HUNDREDTHS,
    MIN_JOBS, MIN_SHOPS, NOTE,
};
use wenmar_vehicles::text::{is_slug, slug, squeeze};
use wenmar_vehicles::{Source, Value};

pub use crate::api::labor::METHOD_PATH;
use crate::api::labor::{
    JobVehiclesQuery, MOST_VEHICLES, job_op, job_vehicles_op, jobs_op, labor_of,
};
use crate::api::recalls::model_year;
use crate::api::types::{JobDetail, JobLink, JobVehicles, LaborFigure, VehicleLabor};
use crate::error::ApiError;
use crate::site::markdown::{self, Format, escape};
use crate::site::tools::canada_invoice_tax::rates::{self, Province};
use crate::site::{self, Page, jsonld, report, seo};
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
        None if figure.basis == "ai" => {
            "an AI estimate worked out from the repair steps, not checked by a person".to_owned()
        }
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

// ----- how estimates are made -----

/// The page's one heading, and the name its breadcrumb and structured data
/// give it.
pub const METHOD_HEADING: &str = "How Wenmar's labour estimates are made";

const METHOD_DESCRIPTION: &str = "How Wenmar's labour times are made: written by technicians, measured from repair orders, or worked out by AI from repair steps, never copied from a labour guide.";

/// `n` with the right noun: `1 figure`, `2 figures`.
fn counted(n: i64, one: &str, many: &str) -> String {
    if n == 1 {
        format!("1 {one}")
    } else {
        format!("{n} {many}")
    }
}

/// What a basis is, in words a reader knows.
fn basis_phrase(basis: &str) -> &'static str {
    match basis {
        "observed" => "measured from repair orders",
        "ai" => "worked out by a model",
        _ => "written by people",
    }
}

/// The page's text, made once so that HTML and Markdown say the same.
struct Method {
    lead: String,
    /// What the file holds, one line each.
    holds: Vec<String>,
    expert: Vec<String>,
    observed: Vec<String>,
    ai: Vec<String>,
    which: Vec<String>,
    not: Vec<String>,
    suggest: String,
    credit: String,
}

impl Method {
    fn of(counts: &wenmar_shopdata::labor::LaborCounts) -> Method {
        let mut holds = Vec::new();
        if counts.figures == 0 {
            holds.push("No figure is on file yet.".to_owned());
        } else {
            holds.push(format!(
                "{}, for {}, on {} of {}.",
                counted(counts.figures, "figure", "figures"),
                counted(counts.jobs, "job", "jobs"),
                counted(counts.models, "model", "models"),
                counted(counts.makes, "make", "makes")
            ));
            let by_basis: Vec<String> = counts
                .by_basis
                .iter()
                .map(|(basis, n)| {
                    format!(
                        "{} {}",
                        basis_phrase(basis),
                        counted(*n, "figure", "figures")
                    )
                })
                .collect();
            if !by_basis.is_empty() {
                holds.push(format!("By basis: {}.", by_basis.join("; ")));
            }
            holds.push(format!(
                "The AI figures worked out with the repair steps: {}.",
                counted(counts.ai_with_steps, "figure", "figures")
            ));
            holds.push(if counts.ai_answered_by.is_empty() {
                "No AI figure names the model that answered it.".to_owned()
            } else {
                format!("Answered by: {}.", counts.ai_answered_by.join(", "))
            });
            holds.push(if counts.ai_methods.is_empty() {
                "No AI figure names the prompt version it answered.".to_owned()
            } else {
                format!("Prompt versions: {}.", counts.ai_methods.join(", "))
            });
            holds.push(match &counts.newest {
                Some(day) => format!("Updated {day}."),
                None => "No figure names the day it was changed.".to_owned(),
            });
        }
        let least = Hours::from_hundredths(LEAST_HUNDREDTHS).to_string();
        let most = Hours::from_hundredths(AI_MOST_HUNDREDTHS).to_string();
        Method {
            lead: "Every figure is the hours one job takes on one vehicle or kind of vehicle. It is written by a technician, measured from repair orders, or worked out by a model from the vehicle, the job and a repair procedure's steps, and never copied from a labour guide.".to_owned(),
            holds,
            expert: vec![
                "The person who writes a figure does the work. It is never taken from a labour guide, a maker's warranty time, shop software or another website.".to_owned(),
                "Every written figure records who wrote it and who reviewed it, and the reviewer is someone else. Wenmar publishes a figure only with both.".to_owned(),
            ],
            observed: vec![format!(
                "A measured figure is the median of at least {MIN_JOBS} repair orders from at least {MIN_SHOPS} shops. The low and the high are the 10th and the 90th percentile of those orders, not the least and the most."
            ),
            "It says how many repair orders and shops stand behind it, and never which shops: no shop, customer, VIN or invoice number is in the data.".to_owned()],
            ai: vec![
                "The model is given the job's name, what it includes and excludes, the vehicle, and, when there is one, the steps of a repair procedure with every hour figure and every line about labour time removed. It is never given a labour time, and no procedure text is published.".to_owned(),
                "It answers a low, a typical and a high figure in hours, its confidence as a percentage, and what it assumed. The confidence and the assumptions are published with the figure.".to_owned(),
                format!("An answer is refused, not corrected, when a figure is below {least} or above {most} hours, when the high is more than {AI_MOST_SPREAD} times the typical, or when the confidence is above {AI_MOST_CONFIDENCE} percent. No refused figure is published."),
                "A job that does not exist on a vehicle, such as a timing belt on an engine with a chain, gets no figure: the model may answer that it does not apply, and nothing is written.".to_owned(),
                "No person checked an AI figure. It is labelled with its basis and its confidence, and within one scope it comes after a measured and a written figure.".to_owned(),
            ],
            which: vec![
                "A vehicle gets the narrowest figure that covers it: the engine, then the generation, then the model, then the class.".to_owned(),
                "Within one scope a measured figure comes first, then a written one, then an AI one. The answer says which scope answered, so a figure for a class of vehicles is never shown as one worked out on a particular vehicle.".to_owned(),
            ],
            not: vec![
                NOTE.to_owned(),
                "Agreement with the hours shops bill means agreement with what shops charge for the job, not with a clock: the time a technician spends also holds the diagnosis, the test drive, waiting for parts and the unexpected.".to_owned(),
            ],
            suggest: format!("/feedback/labor?page={}", report::encode(METHOD_PATH)),
            credit: format!(
                "{ATTRIBUTION}. Published under CC BY 4.0; reuse it with this credit."
            ),
        }
    }
}

fn method_markdown(method: &Method, base: &str) -> String {
    let mut text = format!("# {METHOD_HEADING}\n\n{}\n", escape(&method.lead));
    text.push_str("\n## What the data holds\n\n");
    for line in &method.holds {
        text.push_str(&format!("- {}\n", escape(line)));
    }
    for (heading, paragraphs) in [
        ("Written by people who do the work", &method.expert),
        ("Measured from repair orders", &method.observed),
        ("Worked out by a model from the repair steps", &method.ai),
        ("Which figure a vehicle gets", &method.which),
        ("What a figure is not", &method.not),
    ] {
        text.push_str(&format!("\n## {heading}\n\n"));
        for paragraph in paragraphs {
            text.push_str(&format!("{}\n\n", escape(paragraph)));
        }
    }
    text.push_str(&format!("\n{}\n", escape(&method.credit)));
    text.push_str(&format!(
        "\n- [Suggest a time for a job]({base}{})\n- [Every job and its hours]({base}/jobs)\n- [The job list as JSON]({base}/v1/jobs)\n- [Wenmar Open for AI agents]({base}/llms.txt)\n",
        method.suggest
    ));
    text
}

#[derive(Template)]
#[template(path = "labor_method.html")]
struct MethodPage {
    page: Page,
    method: Method,
}

/// `/jobs/how-estimates-are-made` and its Markdown twin.
pub async fn method(State(state): State<AppState>, uri: Uri) -> Response {
    let Ok(shop) = state.dataset(Dataset::Labor) else {
        return site::not_found(&state);
    };
    let counts = match shop.run(|reader| reader.labor_counts()).await {
        Ok(Ok(counts)) => counts,
        Ok(Err(error)) => return site::failed(&state, &ApiError::from(error)),
        Err(error) => return site::failed(&state, &ApiError::from(error)),
    };
    let method = Method::of(&counts);
    let base = &state.config().base_url;
    if uri.path().ends_with(".md") {
        let canonical = format!("{base}{METHOD_PATH}");
        return markdown::response(method_markdown(&method, base), &canonical);
    }
    let description = seo::description(&[&[METHOD_DESCRIPTION.to_owned()]], 160);
    let page = Page::new(
        &state,
        seo::title("How labour estimates are made"),
        description.clone(),
    )
    .with_markdown(&format!("{METHOD_PATH}.md"))
    .with_json("/v1/jobs")
    .under(vec![("Jobs".to_owned(), "/jobs".to_owned())])
    .as_article();
    let trail = page.trail(METHOD_HEADING);
    let mut article = jsonld::article(base, METHOD_PATH, METHOD_HEADING, &description);
    if let Some(newest) = &counts.newest {
        article["dateModified"] = json!(newest);
    }
    let page = page
        .describing(vec![trail, article])
        .indexed(&state, METHOD_PATH);
    site::html(StatusCode::OK, &MethodPage { page, method })
}

// ----- /jobs -----

#[derive(Template)]
#[template(path = "jobs.html")]
struct JobsPage {
    page: Page,
    jobs: Vec<JobLink>,
    dated: String,
}

/// What the job list is, for the dataset a search engine lists.
const DATASET_DESCRIPTION: &str = "Wenmar's own estimates of the hours a repair job takes: written by people who do the work, measured from repair orders, or worked out by AI from the repair steps; never copied from a labour guide. An estimate, not a quote.";

/// The newest day any labour figure was changed. `None` without estimates.
async fn newest_figure(state: &AppState) -> Option<String> {
    let shop = state.dataset(Dataset::Labor).ok()?;
    match shop.run(|reader| reader.labor_counts()).await {
        Ok(Ok(counts)) => counts.newest,
        _ => None,
    }
}

/// `/jobs`: the job list.
pub async fn index(State(state): State<AppState>) -> Response {
    let list = match jobs_op(&state).await {
        Ok(list) => list,
        Err(ApiError::DatasetUnavailable(_)) => return site::not_found(&state),
        Err(error) => return site::failed(&state, &error),
    };
    let newest = newest_figure(&state).await;
    let page = Page::new(
        &state,
        seo::title("Repair jobs and labour times"),
        "How long common repair jobs take, estimated by Wenmar: what each job includes, and its typical hours by class of vehicle.",
    )
    .with_markdown("/jobs.md")
    .with_json("/v1/jobs")
    .describing(vec![
        jsonld::dataset(
            &state.config().base_url,
            DATASET_DESCRIPTION,
            newest.as_deref(),
        ),
        jsonld::organization(&state.config().base_url),
    ])
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
    let mut text = format!(
        "# Repair jobs and labour times\n\nHow long common repair jobs take on each vehicle: written by people who do the work, measured from repair orders, or worked out by AI from the repair steps; never copied from a labour guide. [How estimates are made]({}{METHOD_PATH}).\n\n",
        state.config().base_url
    );
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

/// One line of "Vehicles with an estimate": a vehicle that has its own
/// figure for a job.
struct JobVehicleRow {
    /// `Hyundai Kona`.
    name: String,
    /// `2021 to 2024`, or `every year` for a model's figure.
    years: String,
    /// `13 hours`.
    typical: String,
    /// `AI, 45% confidence`, `written` or `measured`.
    basis: String,
    /// The vehicle-job page, or the job's own page if there is none.
    href: String,
}

/// A link to the Markdown twin of `href`, its query kept if it has one.
fn markdown_href(href: &str) -> String {
    match href.split_once('?') {
        Some((path, query)) => format!("{path}.md?{query}"),
        None => format!("{href}.md"),
    }
}

#[derive(Template)]
#[template(path = "job.html")]
struct JobPage {
    page: Page,
    job: JobDetail,
    /// Class name and the figure in words.
    classes: Vec<(String, String)>,
    /// The vehicles that have their own figure for the job.
    vehicles: Vec<JobVehicleRow>,
    /// Whether more vehicles have a figure than are listed here.
    more_vehicles: bool,
    labor_held: bool,
    /// The vehicle asked for, when the catalog does not have it.
    missing: Option<String>,
    /// Code and title of the trouble codes about the job's components.
    codes: Vec<(String, String)>,
    /// The form that suggests a time, with the job filled in.
    suggest: String,
    report: report::ReportLinks,
}

fn job_markdown(job: &JobDetail, vehicles: &[JobVehicleRow], base: &str) -> String {
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
    if !vehicles.is_empty() {
        text.push_str("\n## Vehicles with an estimate\n\n");
        for vehicle in vehicles {
            text.push_str(&format!(
                "- [{}]({}): {}, {}, {}\n",
                escape(&vehicle.name),
                markdown_href(&vehicle.href),
                vehicle.years,
                vehicle.typical,
                escape(&vehicle.basis)
            ));
        }
    }
    text.push_str(&format!(
        "\n[How estimates are made]({base}{METHOD_PATH})\n"
    ));
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
    // The vehicles that have a figure of their own for this job. A job with
    // only class figures has none, and on a server without estimates the
    // answer is the job's own page, with nothing to list.
    let labor_held = state.dataset(Dataset::Labor).is_ok();
    let listed = match job_vehicles_op(
        &state,
        found.id.clone(),
        JobVehiclesQuery {
            limit: Some(MOST_VEHICLES),
            cursor: None,
        },
    )
    .await
    {
        Ok(list) => list,
        Err(ApiError::NotFound(_) | ApiError::DatasetUnavailable(_)) => JobVehicles {
            job: JobLink {
                id: found.id.clone(),
                name: found.name.clone(),
                system: found.system.clone(),
                url: format!("{}{path}", state.config().base_url),
            },
            vehicles: Vec::new(),
            next_cursor: None,
            note: String::new(),
            attribution: Vec::new(),
        },
        Err(error) => return site::failed(&state, &error),
    };
    let base = &state.config().base_url;
    let vehicles: Vec<JobVehicleRow> = listed
        .vehicles
        .iter()
        .map(|vehicle| JobVehicleRow {
            name: format!("{} {}", vehicle.make, vehicle.model),
            years: vehicle.figure.years.as_ref().map_or_else(
                || "every year".to_owned(),
                |years| {
                    if years.from == years.to {
                        years.from.to_string()
                    } else {
                        format!("{} to {}", years.from, years.to)
                    }
                },
            ),
            typical: format!("{} hours", shown(vehicle.figure.hours.typical)),
            basis: basis_short(&vehicle.figure),
            href: vehicle
                .figure
                .page_url
                .as_deref()
                .and_then(|url| url.strip_prefix(base))
                .map_or_else(|| path.clone(), str::to_owned),
        })
        .collect();
    let more_vehicles = listed.next_cursor.is_some();
    if format == Format::Markdown {
        let canonical = format!("{}{path}", base);
        return markdown::response(job_markdown(&found, &vehicles, base), &canonical);
    }
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
    let counted = if more_vehicles {
        format!("more than {MOST_VEHICLES} vehicles")
    } else {
        seo::count(vehicles.len(), "vehicle")
    };
    let description = seo::description(
        &[&[
            if vehicles.is_empty() {
                format!(
                    "{}: what the job includes, and how long it typically takes by class of vehicle.",
                    found.name
                )
            } else {
                format!(
                    "{}: what the job includes, and how long it takes on {counted} and by class of vehicle.",
                    found.name
                )
            },
            format!("{}: what the job includes.", found.name),
        ]],
        160,
    );
    let page = Page::new(&state, seo::title(&found.name), description)
        .with_markdown(&format!("{path}.md"))
        .with_json(&format!("/v1{path}"))
        .under(vec![("Jobs".to_owned(), "/jobs".to_owned())]);
    let trail = page.trail(&found.name);
    let page = page.describing(vec![trail]).indexed(&state, &path);
    let page = if missing.is_some() {
        page.with_query()
    } else {
        page
    };
    let suggest = format!(
        "/feedback/labor?job={}&page={}",
        found.id,
        report::encode(&path)
    );
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
            vehicles,
            more_vehicles,
            labor_held,
            missing,
            codes,
            suggest,
            job: found,
            report,
        },
    )
}

// ----- every labour time of one vehicle -----

/// One line of a vehicle's list of labour times.
pub struct ListRow {
    pub job: String,
    pub name: String,
    /// `88 hours`.
    pub typical: String,
    /// `87 to 89 hours`.
    pub range: String,
    /// The figure in one line: `87 to 89 hours, typically 88 (…)`.
    pub words: String,
    /// What stands behind it: `written`, `measured` or `AI, 45% confidence`.
    pub basis: String,
    /// The labour charge at the visitor's rate: low, typical and high.
    pub charges: Vec<String>,
    /// The job's page for this vehicle.
    pub page: String,
}

/// What a basis is, in the few words a table has room for.
fn basis_short(figure: &LaborFigure) -> String {
    match figure.basis.as_str() {
        "ai" => match figure.confidence_pct {
            Some(pct) => format!("AI, {pct}% confidence"),
            None => "AI".to_owned(),
        },
        "observed" => "measured".to_owned(),
        _ => "written".to_owned(),
    }
}

#[derive(Template)]
#[template(path = "labor_model_year_list.html")]
struct ModelYearListPage {
    page: Page,
    /// `2023 Hyundai Kona`.
    name: String,
    path: String,
    model_year_path: String,
    /// The jobs with a figure narrower than their class.
    narrow: Vec<ListRow>,
    /// The jobs whose only figure is typical of the vehicle's class.
    class: Vec<ListRow>,
    class_name: String,
    /// The jobs with no figure at all.
    missing: Vec<JobLink>,
    /// The rate as the visitor typed it.
    rate: String,
    problem: Option<String>,
    /// What an estimate is not.
    note: String,
    attribution: Vec<String>,
    suggest: String,
    report: report::ReportLinks,
}

/// The Markdown twin of the vehicle's list of labour times.
fn model_year_list_markdown(
    name: &str,
    answer: &VehicleLabor,
    narrow: &[ListRow],
    class: &[ListRow],
    base: &str,
) -> String {
    let mut text = format!("# {name} labour times\n\n");
    text.push_str(&format!(
        "How long repair jobs take on the {name}, with the range and how each figure was made. [How estimates are made]({base}{METHOD_PATH}).\n"
    ));
    let class_heading = match &answer.class {
        Some(class) => format!("Typical for a {}", class.name),
        None => "Typical by class of vehicle".to_owned(),
    };
    for (heading, rows) in [
        ("For this vehicle", narrow),
        (class_heading.as_str(), class),
    ] {
        if rows.is_empty() {
            continue;
        }
        text.push_str(&format!("\n## {heading}\n\n"));
        for row in rows {
            text.push_str(&format!(
                "- [{}]({}.md): {}\n",
                escape(&row.name),
                row.page,
                escape(&row.words)
            ));
        }
    }
    if !answer.missing.is_empty() {
        text.push_str("\n## No estimate yet\n\n");
        for job in &answer.missing {
            text.push_str(&format!("- {}\n", escape(&job.name)));
        }
        text.push_str(&format!(
            "\n[Suggest a time for a job](/feedback/labor?vehicle={})\n",
            report::encode(name)
        ));
    }
    text.push_str(&format!("\n{}\n", escape(&answer.note)));
    for credit in &answer.attribution {
        text.push_str(&format!("\n{}.\n", escape(credit)));
    }
    text.push_str(&format!(
        "\nJSON: [/v1/vehicles/{id}/labor](/v1/vehicles/{id}/labor)\n",
        id = answer.vehicle.id
    ));
    text
}

/// `/makes/{make}/{model}/{year}/labor` and its Markdown twin: every
/// labour time of one vehicle, the class figures under their heading, and
/// the jobs with no figure at all.
pub async fn model_year_list(
    State(state): State<AppState>,
    path: Result<Path<(String, String, String, String)>, PathRejection>,
    query: Result<Query<PriceQuery>, QueryRejection>,
    uri: Uri,
) -> Response {
    let Ok(Path((make, model, year, segment))) = path else {
        return site::not_found(&state);
    };
    let (_, format) = markdown::split(&segment);
    let Some(year) = site::year_in(&year) else {
        return site::not_found(&state);
    };
    if !is_slug(&make) || !is_slug(&model) {
        return site::not_found(&state);
    }
    let Ok(shop) = state.dataset(Dataset::Labor) else {
        return site::not_found(&state);
    };
    let id = format!("{year:04}_{make}_{model}");
    let found = match state.db().run(move |worker| model_year(worker, &id)).await {
        Ok(Ok(Some(found))) => found,
        Ok(Ok(None)) => return site::not_found(&state),
        Ok(Err(error)) => return site::failed(&state, &ApiError::from(error)),
        Err(error) => return site::failed(&state, &ApiError::from(error)),
    };
    let answer = match labor_of(&state, shop, &found, None, None).await {
        Ok(Some(answer)) => answer,
        Ok(None) => return site::not_found(&state),
        Err(error) => return site::failed(&state, &error),
    };
    let model_year_path = format!("/makes/{}/{}/{year}", found.make_slug, found.model_slug);
    let path = format!("{model_year_path}/labor");
    let name = format!(
        "{} {} {}",
        answer.vehicle.year, answer.vehicle.make, answer.vehicle.model
    );
    let heading = format!("{name} labour times");
    let base = &state.config().base_url;

    let asked = query.ok().map(|Query(query)| query).unwrap_or_default();
    let rate = asked
        .rate
        .as_deref()
        .map(|rate| rate.trim().chars().take(12).collect::<String>())
        .unwrap_or_default();
    let charges_at = |figure: &LaborFigure| -> Result<Vec<String>, String> {
        if rate.is_empty() {
            return Ok(Vec::new());
        }
        prices(figure, &rate, None).map(|rows| rows.into_iter().map(|row| row.charge).collect())
    };
    let page_for = |job: &str| format!("{path}/{job}");
    let mut narrow = Vec::new();
    let mut class = Vec::new();
    let mut problem = None;
    for figure in &answer.estimates {
        let charges = match charges_at(figure) {
            Ok(charges) => charges,
            Err(why) => {
                problem = Some(why);
                Vec::new()
            }
        };
        let row = ListRow {
            job: figure.job.clone(),
            name: figure.name.clone(),
            typical: format!("{} hours", shown(figure.hours.typical)),
            range: format!(
                "{} to {} hours",
                shown(figure.hours.low),
                shown(figure.hours.high)
            ),
            words: range_words(figure),
            basis: basis_short(figure),
            charges,
            page: page_for(&figure.job),
        };
        if figure.scope == "class" {
            class.push(row);
        } else {
            narrow.push(row);
        }
    }
    let class_name = answer
        .class
        .as_ref()
        .map(|class| class.name.clone())
        .unwrap_or_default();
    if format == Format::Markdown {
        let canonical = format!("{base}{path}");
        return markdown::response(
            model_year_list_markdown(&name, &answer, &narrow, &class, base),
            &canonical,
        );
    }

    let description = seo::description(
        &[&[format!(
            "How long repair jobs take on the {name}: {} from Wenmar, with the range and how each was made.",
            seo::count(answer.estimates.len(), "labour time")
        )]],
        160,
    );
    let page = Page::new(&state, seo::title(&heading), description)
        .with_markdown(&format!("{path}.md"))
        .with_json(&format!("/v1/vehicles/{}/labor", answer.vehicle.id))
        .in_section("makes")
        .under(vec![
            ("Makes".to_owned(), "/makes".to_owned()),
            (
                answer.vehicle.make.clone(),
                format!("/makes/{}", found.make_slug),
            ),
            (name.clone(), model_year_path.clone()),
        ]);
    let trail = page.trail("Labour times");
    let page = page.describing(vec![trail]);
    // Offered to search engines when the vehicle has a figure of its own,
    // is a light vehicle, and the address has no query string.
    let page = match (!narrow.is_empty() && found.light, uri.query().is_some()) {
        (true, false) => page.indexed(&state, &path),
        (true, true) => page.indexed(&state, &path).with_query(),
        (false, _) => page,
    };
    let indexed = page.index;
    let suggest = format!(
        "/feedback/labor?vehicle={}&page={}",
        report::encode(&name),
        report::encode(&path)
    );
    let report = report::links(state.config(), &path, &name);
    let mut response = site::html(
        StatusCode::OK,
        &ModelYearListPage {
            page,
            name: name.clone(),
            path,
            model_year_path,
            narrow,
            class,
            class_name,
            missing: answer.missing,
            rate,
            problem,
            note: answer.note,
            attribution: answer.attribution,
            suggest,
            report,
        },
    );
    if !indexed {
        site::unindexed(&mut response);
    }
    response
}

// ----- a job on a model year -----

/// What the price form sent, and the engine asked for.
#[derive(Debug, Default, Deserialize)]
pub struct PriceQuery {
    rate: Option<String>,
    province: Option<String>,
    engine: Option<String>,
}

/// One line of the price: the hours, the labour charge, and with a
/// province the tax and the total.
pub struct PriceRow {
    pub label: &'static str,
    pub hours: String,
    pub charge: String,
    pub tax: Option<String>,
    pub total: String,
}

/// What a rate could not be read as.
pub const BAD_RATE: &str = "Enter your hourly rate as dollars, such as 150 or 149.50.";

/// The labour charge for the low, typical and high hours of `figure` at
/// `rate`, with the taxes of `province` when it is confirmed. `Err` is
/// what to tell the visitor.
pub fn prices(
    figure: &LaborFigure,
    rate: &str,
    province: Option<&'static Province>,
) -> Result<Vec<PriceRow>, String> {
    let rate = Money::parse(rate.trim()).map_err(|_| BAD_RATE.to_owned())?;
    let mut rows = Vec::with_capacity(3);
    for (label, hours) in [
        ("Low", figure.hours.low),
        ("Typical", figure.hours.typical),
        ("High", figure.hours.high),
    ] {
        let hundredths = Hours::from_hundredths((hours * 100.0).round() as i64);
        let too_large = || "That rate is too large to work with.".to_owned();
        let charge = labor_charge(rate, hundredths).map_err(|_| too_large())?;
        let taxed = match province
            .and_then(|province| province.invoice(charge, Money::ZERO, Money::ZERO, &[]))
        {
            Some(Ok(taxed)) => Some(taxed),
            Some(Err(_)) => return Err(too_large()),
            None => None,
        };
        rows.push(PriceRow {
            label,
            hours: hundredths.to_string(),
            charge: charge.to_string(),
            tax: match &taxed {
                Some(taxed) => Some(
                    taxed
                        .total
                        .checked_sub(taxed.subtotal)
                        .map_err(|_| too_large())?
                        .to_string(),
                ),
                None => None,
            },
            total: taxed.map_or(charge, |taxed| taxed.total).to_string(),
        });
    }
    Ok(rows)
}

/// What a figure is for, as a sentence.
pub fn scope_words(figure: &LaborFigure, make: &str, model: &str) -> String {
    let years = figure
        .years
        .as_ref()
        .map(|years| {
            if years.from == years.to {
                years.from.to_string()
            } else {
                format!("{} to {}", years.from, years.to)
            }
        })
        .unwrap_or_default();
    match figure.scope.as_str() {
        "engine" => format!(
            "This figure is for the {years} {make} {model} with the {} engine.",
            figure.engine.as_deref().unwrap_or_default()
        ),
        "generation" => format!("This figure is for the {years} {make} {model}."),
        "model" => format!("This figure is for the {make} {model}, every model year."),
        _ => format!(
            "This figure is typical for a {}. It was not worked out on this vehicle.",
            figure
                .class_name
                .as_deref()
                .unwrap_or("vehicle of its class")
        ),
    }
}

/// The one-sentence answer a vehicle-job page leads with: "Test job C on
/// a 2023 Hyundai Kona takes about 13 hours (12 to 15 hours)." It is the
/// page's first paragraph, the start of its description and the first line
/// of its Markdown twin, so the number is in the first sentence an answer
/// engine quotes.
pub fn answer_sentence(job: &str, name: &str, figure: &LaborFigure) -> String {
    format!(
        "{job} on a {name} takes about {} hours ({} to {} hours).",
        shown(figure.hours.typical),
        shown(figure.hours.low),
        shown(figure.hours.high)
    )
}

/// What an AI figure assumed and how it was made.
struct Assumed {
    /// What the model said it assumed, in its own words.
    assumed: Option<String>,
    confidence: String,
    model: String,
    updated: String,
}

impl Assumed {
    fn of(figure: &LaborFigure) -> Assumed {
        Assumed {
            assumed: figure.notes.clone(),
            confidence: figure
                .confidence_pct
                .map_or_else(|| "unknown".to_owned(), |pct| pct.to_string()),
            model: figure.model.clone().unwrap_or_else(|| "unknown".to_owned()),
            updated: figure.updated.clone(),
        }
    }
}

/// The light model years of a model, newest first, as the model-year page
/// lists them.
async fn model_years_of(state: &AppState, make: &str, model: &str) -> Vec<u16> {
    let (make, model) = (make.to_owned(), model.to_owned());
    let rows = state
        .db()
        .run(move |worker| {
            worker.source.query(
                crate::site::catalog::MODEL_YEARS_SQL,
                &[make.as_str().into(), model.as_str().into()],
            )
        })
        .await;
    match rows {
        Ok(Ok(rows)) => rows
            .iter()
            .filter_map(|row| {
                let year = row
                    .first()
                    .and_then(Value::integer)
                    .and_then(|year| u16::try_from(year).ok())?;
                (row.get(1).and_then(Value::integer) == Some(1)).then_some(year)
            })
            .collect(),
        _ => Vec::new(),
    }
}

/// The model years a figure covers, in words: `2021 to 2024`, or `2023`.
fn years_words(years: &crate::api::types::LaborYears) -> String {
    if years.from == years.to {
        years.from.to_string()
    } else {
        format!("{} to {}", years.from, years.to)
    }
}

/// What stands behind a figure, as a sentence.
fn basis_sentence(figure: &LaborFigure) -> String {
    let words = basis_words(figure);
    let mut letters = words.chars();
    match letters.next() {
        Some(first) => format!("{}{}.", first.to_uppercase(), letters.as_str()),
        None => String::new(),
    }
}

#[derive(Template)]
#[template(path = "labor_model_year.html")]
struct ModelYearLaborPage {
    page: Page,
    job: JobDetail,
    /// `2023 Hyundai Kona`.
    name: String,
    path: String,
    model_year_path: String,
    /// The answer, in one sentence, when a figure covers the vehicle.
    answer: Option<String>,
    /// `87 to 89 hours, typically 88.` when a figure covers the vehicle.
    range: Option<String>,
    scope_words: String,
    basis: String,
    note: String,
    attribution: Vec<String>,
    engine: Option<String>,
    /// The rate as the visitor typed it.
    rate: String,
    /// Code, name and whether it is the one chosen.
    provinces: Vec<(&'static str, &'static str, bool)>,
    problem: Option<String>,
    prices: Vec<PriceRow>,
    tax_words: String,
    recalls_href: Option<String>,
    /// Component name and the bulletin list narrowed to it.
    bulletin_links: Vec<(String, String)>,
    codes: Vec<(String, String)>,
    /// What an AI figure assumed, and how it was made. `None` for a
    /// figure of any other basis.
    ai: Option<Assumed>,
    /// The other catalog years the figure covers, with their pages.
    other_years: Vec<(u16, String)>,
    /// `2021 to 2024`.
    years_words: String,
    /// `Hyundai Kona`.
    vehicle: String,
    /// The form that suggests a time, with the job and the vehicle filled in.
    suggest: String,
    report: report::ReportLinks,
}

/// `/makes/{make}/{model}/{year}/labor/{job}` and its Markdown twin.
/// Offered to search engines only when the figure's scope is narrower
/// than a class and the address has no query string.
pub async fn model_year_labor(
    State(state): State<AppState>,
    path: Result<Path<(String, String, String, String)>, PathRejection>,
    query: Result<Query<PriceQuery>, QueryRejection>,
    uri: Uri,
) -> Response {
    let Ok(Path((make, model, year, segment))) = path else {
        return site::not_found(&state);
    };
    let (stem, format) = markdown::split(&segment);
    let Some(year) = site::year_in(&year) else {
        return site::not_found(&state);
    };
    if !is_slug(stem) || !is_slug(&make) || !is_slug(&model) {
        return site::not_found(&state);
    }
    let Ok(shop) = state.dataset(Dataset::Labor) else {
        return site::not_found(&state);
    };
    let asked = query.ok().map(|Query(query)| query).unwrap_or_default();
    let engine = asked.engine.clone().filter(|engine| is_slug(engine));
    let id = format!("{year:04}_{make}_{model}");
    let found = match state.db().run(move |worker| model_year(worker, &id)).await {
        Ok(Ok(Some(found))) => found,
        Ok(Ok(None)) => return site::not_found(&state),
        Ok(Err(error)) => return site::failed(&state, &ApiError::from(error)),
        Err(error) => return site::failed(&state, &ApiError::from(error)),
    };
    let answer = match labor_of(&state, shop, &found, engine.clone(), Some(stem.to_owned())).await {
        Ok(Some(answer)) => answer,
        Ok(None) => return site::not_found(&state),
        Err(error) => return site::failed(&state, &error),
    };
    let job = match job_op(&state, stem.to_owned()).await {
        Ok(job) => job,
        Err(ApiError::NotFound(_)) => return site::not_found(&state),
        Err(error) => return site::failed(&state, &error),
    };
    let figure = answer.estimates.first();
    let model_year_path = format!("/makes/{}/{}/{year}", found.make_slug, found.model_slug);
    let path = format!("{model_year_path}/labor/{}", job.id);
    let name = format!(
        "{} {} {}",
        answer.vehicle.year, answer.vehicle.make, answer.vehicle.model
    );
    let heading = format!("{} on the {name}: labour time", job.name);
    let answer_sentence = figure.map(|figure| answer_sentence(&job.name, &name, figure));
    let range = figure.map(|figure| {
        format!(
            "{} to {} hours, typically {}.",
            shown(figure.hours.low),
            shown(figure.hours.high),
            shown(figure.hours.typical)
        )
    });
    let scope = figure
        .map(|figure| scope_words(figure, &answer.vehicle.make, &answer.vehicle.model))
        .unwrap_or_default();
    let basis = figure.map(basis_sentence).unwrap_or_default();
    let ai = figure
        .filter(|figure| figure.basis == "ai")
        .map(Assumed::of);
    let vehicle = format!("{} {}", answer.vehicle.make, answer.vehicle.model);
    let figures_years = figure.and_then(|figure| figure.years.as_ref());
    let other_years: Vec<(u16, String)> = match figures_years {
        Some(years) => model_years_of(&state, &found.make_slug, &found.model_slug)
            .await
            .into_iter()
            .filter(|year| *year != answer.vehicle.year && (years.from..=years.to).contains(year))
            .map(|year| {
                (
                    year,
                    format!(
                        "/makes/{}/{}/{year}/labor/{}",
                        found.make_slug, found.model_slug, job.id
                    ),
                )
            })
            .collect(),
        None => Vec::new(),
    };
    let years_words = figures_years.map(years_words).unwrap_or_default();
    if format == Format::Markdown {
        let mut text = format!("# {}\n", escape(&heading));
        if let Some(answer) = &answer_sentence {
            text.push_str(&format!("\n{}\n", escape(answer)));
        }
        match &range {
            Some(range) => {
                text.push_str(&format!(
                    "\n{} {} {}\n\n{}\n",
                    escape(range),
                    escape(&scope),
                    escape(&basis),
                    escape(&answer.note)
                ));
                for credit in &answer.attribution {
                    text.push_str(&format!("\n{}.\n", escape(credit)));
                }
            }
            None => text.push_str("\nNo estimate covers this job on this vehicle yet.\n"),
        }
        if let Some(ai) = &ai {
            text.push_str("\n## What the estimate assumed\n\n");
            if let Some(assumed) = &ai.assumed {
                text.push_str(&format!("{}\n\n", escape(assumed)));
            }
            text.push_str(&format!(
                "An AI estimate, not checked by a person: confidence {}%, model {}, figure last changed on {}.\n",
                ai.confidence, ai.model, ai.updated
            ));
        }
        if !other_years.is_empty() {
            text.push_str(&format!(
                "\n## The same estimate covers\n\nThe same estimate covers the {years_words} {vehicle}: "
            ));
            for (index, (year, href)) in other_years.iter().enumerate() {
                if index > 0 {
                    text.push_str(", ");
                }
                text.push_str(&format!("[{year}]({href}.md)"));
            }
            text.push_str(".\n");
        }
        text.push_str(&format!(
            "\n[How estimates are made]({}{METHOD_PATH})\n",
            state.config().base_url
        ));
        let canonical = format!("{}{path}", state.config().base_url);
        return markdown::response(text, &canonical);
    }

    // The price: only from a rate the visitor typed.
    let rate = asked
        .rate
        .as_deref()
        .map(|rate| rate.trim().chars().take(12).collect::<String>())
        .unwrap_or_default();
    let chosen = asked.province.as_deref().and_then(rates::province);
    let confirmed = chosen.filter(|province| province.confirmed);
    let (prices, problem) = match (figure, rate.is_empty()) {
        (Some(figure), false) => match prices(figure, &rate, confirmed) {
            Ok(rows) => (rows, None),
            Err(problem) => (Vec::new(), Some(problem)),
        },
        _ => (Vec::new(), None),
    };
    let tax_words = match (chosen, confirmed) {
        (Some(province), Some(_)) => format!(
            "With {} in {}, as on the Canadian invoice tax calculator.",
            province
                .taxes
                .iter()
                .map(|rule| rule.name)
                .collect::<Vec<_>>()
                .join(" and "),
            province.name
        ),
        (Some(province), None) => format!(
            "The rates for {} are not confirmed yet, so no tax is added.",
            province.name
        ),
        _ => "No province was chosen, so no tax is added.".to_owned(),
    };

    let components: Vec<(String, String)> = job
        .components
        .iter()
        .map(|component| (component.id.clone(), component.name.clone()))
        .collect();
    let bulletin_links = if state.dataset(Dataset::Bulletins).is_ok() {
        components
            .iter()
            .map(|(id, name)| {
                (
                    name.to_lowercase(),
                    format!("{model_year_path}/bulletins?component={id}"),
                )
            })
            .collect()
    } else {
        Vec::new()
    };
    let ids: Vec<String> = components.iter().map(|(id, _)| id.clone()).collect();
    let codes: Vec<(String, String)> = match state.dataset(Dataset::Codes) {
        Ok(shop) if !ids.is_empty() => {
            match shop
                .run(move |reader| reader.codes_for_components(&ids, 10))
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
    let recalls_href = crate::api::recalls::recall_shop(&state, None)
        .is_ok()
        .then(|| format!("{model_year_path}/recalls"));

    // The answer is the description's first sentence: a search result and
    // an answer engine both quote the first thing they read.
    let first_sentence = answer_sentence.clone().unwrap_or_else(|| {
        format!(
            "How long it takes to do the job \"{}\" on the {name}: Wenmar's estimate in hours, and the labour charge at your own rate.",
            job.name
        )
    });
    let description = seo::description(
        &[
            &[first_sentence],
            &[format!("{} on the {name}: labour time.", job.name)],
        ],
        160,
    );
    let title = match figure {
        Some(figure) => seo::title(&format!(
            "{name} {}: {} hours",
            job.name,
            shown(figure.hours.typical)
        )),
        None => seo::title(&heading),
    };
    let page = Page::new(&state, title, description)
        .with_markdown(&format!("{path}.md"))
        .with_json(&format!(
            "/v1/vehicles/{}/labor/{}",
            answer.vehicle.id, job.id
        ))
        .in_section("makes")
        .under(vec![
            ("Makes".to_owned(), "/makes".to_owned()),
            (
                answer.vehicle.make.clone(),
                format!("/makes/{}", found.make_slug),
            ),
            (name.clone(), model_year_path.clone()),
        ]);
    let trail = page.trail(&job.name);
    let page = page.describing(vec![trail]);
    // A class figure is one fact about many vehicles, not a page each.
    let narrow = figure.is_some_and(|figure| figure.scope != "class");
    let page = match (found.light && narrow, uri.query().is_some()) {
        (true, false) => page.indexed(&state, &path),
        (true, true) => page.indexed(&state, &path).with_query(),
        (false, _) => page,
    };
    let indexed = page.index;
    let suggest = format!(
        "/feedback/labor?job={}&vehicle={}&page={}",
        job.id,
        report::encode(&name),
        report::encode(&path)
    );
    let report = report::links(state.config(), &path, &format!("{} {name}", job.name));
    let mut response = site::html(
        StatusCode::OK,
        &ModelYearLaborPage {
            page,
            name,
            path,
            model_year_path,
            range,
            scope_words: scope,
            basis,
            note: answer.note,
            attribution: answer.attribution,
            engine,
            rate,
            provinces: rates::PROVINCES
                .iter()
                .filter(|province| province.confirmed)
                .map(|province| {
                    (
                        province.code,
                        province.name,
                        chosen.is_some_and(|chosen| chosen.code == province.code),
                    )
                })
                .collect(),
            problem,
            prices,
            tax_words,
            recalls_href,
            bulletin_links,
            codes,
            answer: answer_sentence,
            ai,
            other_years,
            years_words,
            vehicle,
            suggest,
            job,
            report,
        },
    );
    if !indexed {
        site::unindexed(&mut response);
    }
    response
}
