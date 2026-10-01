//! The reference pages: every make, one make's models, and one model year.
//!
//! Each page is loaded once into a view, then shown as HTML or as Markdown.

use askama::Template;
use axum::extract::rejection::QueryRejection;
use axum::extract::{Path, Query, State};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Redirect, Response};
use serde::Deserialize;
use wenmar_vehicles::text::is_slug;
use wenmar_vehicles::{CatalogError, Scope};

use crate::api::types::{EngineOption, Entry, Make, Model};
use crate::db::Worker;
use crate::error::ApiError;
use crate::site::markdown::{self, Format, escape};
use crate::site::{self, Page};
use crate::state::AppState;

/// Most makes or models listed on one page, and most trims on a model year.
const MOST_LISTED: usize = 500;
const MOST_TRIMS: usize = 60;

#[derive(Debug, Default, Deserialize)]
pub struct YearQuery {
    year: Option<String>,
}

fn year_of(query: Result<Query<YearQuery>, QueryRejection>) -> Option<u16> {
    query
        .ok()
        .and_then(|Query(query)| query.year)
        .and_then(|year| year.trim().parse().ok())
}

/// Runs a page's loader on a blocking thread.
async fn load<T, F>(state: &AppState, loader: F) -> Result<T, ApiError>
where
    F: FnOnce(&Worker) -> Result<T, CatalogError> + Send + 'static,
    T: Send + 'static,
{
    Ok(state.db().run(loader).await??)
}

// ----- every make -----

#[derive(Template)]
#[template(path = "makes.html")]
struct MakesPage {
    page: Page,
    year: Option<u16>,
    popular: Vec<Make>,
    others: Vec<Make>,
}

fn makes_markdown(year: Option<u16>, makes: &[Make]) -> String {
    let mut text = match year {
        Some(year) => format!("# Makes with a {year} model\n\n"),
        None => "# Makes\n\n".to_owned(),
    };
    text.push_str("Cars, multipurpose vehicles and trucks. Popular makes first.\n\n");
    for make in makes {
        text.push_str(&format!(
            "- [{}](/makes/{}.md)\n",
            escape(&make.name),
            make.id
        ));
    }
    text
}

async fn makes_in(state: AppState, year: Option<u16>, format: Format) -> Response {
    let makes = match load(&state, move |worker| {
        worker.catalog.makes(year, Scope::Light, "", MOST_LISTED)
    })
    .await
    {
        Ok(makes) => makes.into_iter().map(Make::from).collect::<Vec<_>>(),
        Err(error) => return site::failed(&state, &error),
    };
    if format == Format::Markdown {
        let canonical = format!("{}/makes", state.config().base_url);
        return markdown::response(makes_markdown(year, &makes), &canonical);
    }
    let page = Page::new(
        &state,
        "Vehicle makes - Wenmar Open",
        "Every make of car, multipurpose vehicle and truck in NHTSA's data, with its models by year.",
    );
    // Only the full list is indexed; a list for one year is a filter of it.
    let page = match year {
        None => page.indexed(&state, "/makes").with_markdown("/makes.md"),
        Some(_) => page,
    };
    let (popular, others) = makes.into_iter().partition(|make| make.popular);
    site::html(
        StatusCode::OK,
        &MakesPage {
            page,
            year,
            popular,
            others,
        },
    )
}

pub async fn makes(
    State(state): State<AppState>,
    query: Result<Query<YearQuery>, QueryRejection>,
) -> Response {
    makes_in(state, year_of(query), Format::Html).await
}

pub async fn makes_md(
    State(state): State<AppState>,
    query: Result<Query<YearQuery>, QueryRejection>,
) -> Response {
    makes_in(state, year_of(query), Format::Markdown).await
}

// ----- one make -----

/// One make, with its models in one model year.
pub struct MakeView {
    pub slug: String,
    pub name: String,
    /// The year shown: the one asked for, or the newest the make has.
    pub year: u16,
    /// Every year the make has a model in, newest first.
    pub years: Vec<u16>,
    pub models: Vec<Model>,
}

/// What a make's address led to.
enum Found<T> {
    View(T),
    /// The address used a name or an alias; this is the id form to use.
    Elsewhere(String),
    Nothing,
}

fn load_make(
    worker: &Worker,
    text: &str,
    year: Option<u16>,
) -> Result<Found<MakeView>, CatalogError> {
    let index = worker.catalog.index();
    let Some(make) = index.by_slug(text) else {
        return Ok(match index.resolve(text, Scope::All) {
            Some(make) => Found::Elsewhere(make.slug.clone()),
            None => Found::Nothing,
        });
    };
    // A trailer maker has no cars to list, so list what it has.
    let scope = if make.light { Scope::Light } else { Scope::All };
    let all = worker
        .catalog
        .models(&make.slug, None, scope, "", MOST_LISTED)?;
    let first = all.iter().map(|model| model.year_from).min();
    let last = all.iter().map(|model| model.year_to).max();
    let (Some(first), Some(last)) = (first, last) else {
        return Ok(Found::Nothing);
    };
    let year = year
        .filter(|year| (first..=last).contains(year))
        .unwrap_or(last);
    let models = worker
        .catalog
        .models(&make.slug, Some(year), scope, "", MOST_LISTED)?;
    Ok(Found::View(MakeView {
        slug: make.slug.clone(),
        name: make.name.clone(),
        year,
        years: (first..=last).rev().collect(),
        models: models.into_iter().map(Model::from).collect(),
    }))
}

#[derive(Template)]
#[template(path = "make.html")]
struct MakePage {
    page: Page,
    view: MakeView,
}

fn make_markdown(view: &MakeView) -> String {
    let mut text = format!("# {} models, {}\n\n", escape(&view.name), view.year);
    if view.models.is_empty() {
        text.push_str("No models are on file for this year.\n");
    }
    for model in &view.models {
        text.push_str(&format!(
            "- [{}](/makes/{}/{}/{}.md)\n",
            escape(&model.name),
            view.slug,
            model.id,
            view.year
        ));
    }
    text.push_str("\n## Other years\n\n");
    let years: Vec<String> = view
        .years
        .iter()
        .map(|year| format!("[{year}](/makes/{}.md?year={year})", view.slug))
        .collect();
    text.push_str(&years.join(" "));
    text.push('\n');
    text
}

pub async fn make(
    State(state): State<AppState>,
    Path(segment): Path<String>,
    query: Result<Query<YearQuery>, QueryRejection>,
) -> Response {
    let (text, format) = markdown::split(&segment);
    let asked = year_of(query);
    let text = text.to_owned();
    let found = match load(&state, move |worker| load_make(worker, &text, asked)).await {
        Ok(found) => found,
        Err(error) => return site::failed(&state, &error),
    };
    let view = match found {
        Found::View(view) => view,
        // The id form came from the data file and is checked before it is
        // put in an address.
        Found::Elsewhere(slug) if is_slug(&slug) => {
            let suffix = if format == Format::Markdown {
                ".md"
            } else {
                ""
            };
            return Redirect::permanent(&format!("/makes/{slug}{suffix}")).into_response();
        }
        Found::Elsewhere(_) | Found::Nothing => return site::not_found(&state),
    };
    let path = format!("/makes/{}", view.slug);
    if format == Format::Markdown {
        let canonical = format!("{}{path}", state.config().base_url);
        return markdown::response(make_markdown(&view), &canonical);
    }
    let page = Page::new(
        &state,
        format!("{} models by year - Wenmar Open", view.name),
        format!(
            "{} models for {} and every other model year, with trims and engines.",
            view.name, view.year
        ),
    );
    // The page without a year is the one to index.
    let page = match asked {
        None => page
            .indexed(&state, &path)
            .with_markdown(&format!("{path}.md")),
        Some(_) => page,
    };
    site::html(StatusCode::OK, &MakePage { page, view })
}

// ----- one model year -----

/// A trim or series, with what is particular to it.
pub struct TrimLine {
    pub name: String,
    pub kind: String,
    /// Transmission, drive and body, where the data has them for this trim.
    pub details: String,
}

pub struct ModelYearView {
    pub entry: Entry,
    pub make_slug: String,
    pub model_slug: String,
    pub trims: Vec<TrimLine>,
    pub engines: Vec<EngineOption>,
}

fn load_model_year(
    worker: &Worker,
    make: &str,
    model: &str,
    year: u16,
) -> Result<Option<ModelYearView>, CatalogError> {
    if !is_slug(make) || !is_slug(model) {
        return Ok(None);
    }
    let id = format!("{year}_{make}_{model}");
    let Some(entry) = worker.catalog.entry(&id)? else {
        return Ok(None);
    };
    let mut trims = Vec::new();
    for submodel in worker
        .catalog
        .submodels(make, model, year, "")?
        .into_iter()
        .take(MOST_TRIMS)
    {
        let details = worker
            .catalog
            .entry(&format!("{id}_{}", submodel.id))?
            .map(|own| {
                [own.transmission, own.drive, own.body]
                    .into_iter()
                    .flatten()
                    .collect::<Vec<_>>()
                    .join(", ")
            })
            .unwrap_or_default();
        trims.push(TrimLine {
            name: submodel.name,
            kind: submodel.kind,
            details,
        });
    }
    let engines = worker.catalog.engines(make, model, year, None, "")?;
    Ok(Some(ModelYearView {
        entry: Entry::from(entry),
        make_slug: make.to_owned(),
        model_slug: model.to_owned(),
        trims,
        engines: engines.into_iter().map(EngineOption::from).collect(),
    }))
}

#[derive(Template)]
#[template(path = "model_year.html")]
struct ModelYearPage {
    page: Page,
    view: ModelYearView,
    name: String,
}

fn model_year_markdown(view: &ModelYearView) -> String {
    let entry = &view.entry;
    let mut text = format!(
        "# {} {} {}\n\n",
        entry.year,
        escape(&entry.make),
        escape(&entry.model)
    );
    text.push_str(&format!("- Vehicle id: `{}`\n", entry.id));
    for (label, value) in [
        ("Body", &entry.body),
        ("Drive", &entry.drive),
        ("Transmission", &entry.transmission),
    ] {
        if let Some(value) = value {
            text.push_str(&format!("- {label}: {}\n", escape(value)));
        }
    }
    if !entry.vehicle_types.is_empty() {
        let types: Vec<String> = entry
            .vehicle_types
            .iter()
            .map(|name| escape(name))
            .collect();
        text.push_str(&format!("- Vehicle type: {}\n", types.join(", ")));
    }
    text.push_str("\n## Trims\n\n");
    if view.trims.is_empty() {
        text.push_str("None on file.\n");
    }
    for trim in &view.trims {
        text.push_str(&format!("- {}", escape(&trim.name)));
        if !trim.details.is_empty() {
            text.push_str(&format!(": {}", escape(&trim.details)));
        }
        text.push('\n');
    }
    text.push_str("\n## Engines\n\n");
    if view.engines.is_empty() {
        text.push_str("None on file.\n");
    }
    for engine in &view.engines {
        text.push_str(&format!("- {}", escape(&engine.label)));
        if let Some(vin8) = &engine.vin8 {
            text.push_str(&format!(" (eighth VIN character {})", escape(vin8)));
        }
        text.push('\n');
    }
    text.push_str(&format!(
        "\nJSON: [/v1/vehicles/{id}](/v1/vehicles/{id})\n",
        id = entry.id
    ));
    text
}

pub async fn model_year(
    State(state): State<AppState>,
    Path((make, model, segment)): Path<(String, String, String)>,
) -> Response {
    let (year, format) = markdown::split(&segment);
    let Ok(year) = year.parse::<u16>() else {
        return site::not_found(&state);
    };
    let loaded = load(&state, move |worker| {
        load_model_year(worker, &make, &model, year)
    })
    .await;
    let view = match loaded {
        Ok(Some(view)) => view,
        Ok(None) => return site::not_found(&state),
        Err(error) => return site::failed(&state, &error),
    };
    let path = format!(
        "/makes/{}/{}/{}",
        view.make_slug, view.model_slug, view.entry.year
    );
    if format == Format::Markdown {
        let canonical = format!("{}{path}", state.config().base_url);
        return markdown::response(model_year_markdown(&view), &canonical);
    }
    let name = format!(
        "{} {} {}",
        view.entry.year, view.entry.make, view.entry.model
    );
    let page = Page::new(
        &state,
        format!("{name} trims and engines - Wenmar Open"),
        format!("Trims, engines and equipment of the {name}, from NHTSA's data."),
    )
    .indexed(&state, &path)
    .with_markdown(&format!("{path}.md"));
    site::html(StatusCode::OK, &ModelYearPage { page, view, name })
}
