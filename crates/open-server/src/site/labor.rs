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
use shop_math::labor_rate::labor_charge;
use shop_math::{Hours, Money};
use wenmar_shopdata::Dataset;
use wenmar_vehicles::text::{is_slug, slug, squeeze};

use crate::api::labor::{job_op, jobs_op, labor_of};
use crate::api::recalls::model_year;
use crate::api::types::{JobDetail, JobLink, LaborFigure};
use crate::error::ApiError;
use crate::site::markdown::{self, Format, escape};
use crate::site::tools::canada_invoice_tax::rates::{self, Province};
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
    if format == Format::Markdown {
        let mut text = format!("# {}\n", escape(&heading));
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

    let description = seo::description(
        &[&[
            format!(
                "How long it takes to do the job \"{}\" on the {name}: Wenmar's estimate in hours, and the labour charge at your own rate.",
                job.name
            ),
            format!("{} on the {name}: labour time.", job.name),
        ]],
        160,
    );
    let page = Page::new(&state, seo::title(&heading), description)
        .with_markdown(&format!("{path}.md"))
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
            job,
            report,
        },
    );
    if !indexed {
        site::unindexed(&mut response);
    }
    response
}
