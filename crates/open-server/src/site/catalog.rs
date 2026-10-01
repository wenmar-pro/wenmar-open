//! The reference pages: every make, one make's models, and one model year.
//!
//! Each page is loaded once into a view, then shown as HTML or as Markdown.

use askama::Template;
use axum::extract::rejection::{PathRejection, QueryRejection};
use axum::extract::{Path, Query, State};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Redirect, Response};
use serde::Deserialize;
use wenmar_vehicles::text::is_slug;
use wenmar_vehicles::{CatalogError, Scope, Source, Value};

use crate::api::types::{Entry, Make, Model};
use crate::db::Worker;
use crate::error::ApiError;
use crate::site::markdown::{self, Format, escape};
use crate::site::{self, Page, jsonld, seo};
use crate::state::AppState;

/// Most makes or models listed on one page, and most trims on a model year.
const MOST_LISTED: usize = 500;
const MOST_TRIMS: usize = 60;

/// The model years a make has a model in, newest first. `?1` make id, `?2`
/// whether only light-vehicle model years count.
const MAKE_YEARS_SQL: &str = "
SELECT DISTINCT v.year
FROM catalog_model md
JOIN catalog_vehicle v ON v.model_id = md.id
WHERE md.make_id = ?1 AND (?2 = 0 OR v.light = 1)
ORDER BY v.year DESC";

/// Most manufacturer codes listed on a make's page.
const MOST_CODES: usize = 60;

/// The manufacturer codes a make's vehicles are built under. `?1` make id.
const MAKE_CODES_SQL: &str = "SELECT wm.wmi FROM wmi_make wm WHERE wm.make_id = ?1 ORDER BY wm.wmi";

/// Every model year of one model, newest first, with whether it is a light
/// vehicle: a car, an MPV or a truck. `?1` and `?2` are the id forms of the
/// make and the model.
const MODEL_YEARS_SQL: &str = "
SELECT v.year, v.light
FROM catalog_make mk
JOIN catalog_model md ON md.make_id = mk.id
JOIN catalog_vehicle v ON v.model_id = md.id
WHERE mk.slug = ?1 AND md.slug = ?2
ORDER BY v.year DESC";

#[derive(Debug, Default, Deserialize)]
pub struct YearQuery {
    year: Option<String>,
}

fn year_of(query: Result<Query<YearQuery>, QueryRejection>) -> Option<u16> {
    query
        .ok()
        .and_then(|Query(query)| query.year)
        .and_then(|year| site::year_in(year.trim()))
}

/// Runs a page's loader on a blocking thread.
async fn load<T, F>(state: &AppState, loader: F) -> Result<T, ApiError>
where
    F: FnOnce(&Worker) -> Result<T, CatalogError> + Send + 'static,
    T: Send + 'static,
{
    Ok(state.db().run(loader).await??)
}

/// The same as [`load`], for a page that reads every model year. Only half
/// of the connections do such work at once, so a decode always has one.
async fn load_slow<T, F>(state: &AppState, loader: F) -> Result<T, ApiError>
where
    F: FnOnce(&Worker) -> Result<T, CatalogError> + Send + 'static,
    T: Send + 'static,
{
    Ok(state.db().run_slow(loader).await??)
}

/// Keeps the first `most` of a list and says whether anything was left off.
fn cut<T>(mut list: Vec<T>, most: usize) -> (Vec<T>, bool) {
    let over = list.len() > most;
    list.truncate(most);
    (list, over)
}

// ----- every make -----

#[derive(Template)]
#[template(path = "makes.html")]
struct MakesPage {
    page: Page,
    year: Option<u16>,
    popular: Vec<Make>,
    others: Vec<Make>,
    /// How many makes are shown, when the list was cut.
    cut: Option<usize>,
}

fn makes_markdown(year: Option<u16>, makes: &[Make], cut: bool) -> String {
    let mut text = match year {
        Some(year) => format!("# Makes with a {year} model\n\n"),
        None => "# Makes\n\n".to_owned(),
    };
    text.push_str("Cars, multipurpose vehicles and trucks. Popular makes first.\n\n");
    let query = year.map(|year| format!("?year={year}")).unwrap_or_default();
    for make in makes {
        text.push_str(&format!(
            "- [{}](/makes/{}.md{query})\n",
            escape(&make.name),
            make.id
        ));
    }
    if cut {
        text.push_str(&format!(
            "\nOnly the first {} makes are listed.\n",
            makes.len()
        ));
    }
    text
}

async fn makes_in(state: AppState, year: Option<u16>, format: Format) -> Response {
    let loader = move |worker: &Worker| {
        worker
            .catalog
            .makes(year, Scope::Light, "", MOST_LISTED + 1)
    };
    // A list for one year reads every model year of that year.
    let loaded = match year {
        Some(_) => load_slow(&state, loader).await,
        None => load(&state, loader).await,
    };
    let (makes, cut_short) = match loaded {
        Ok(makes) => cut(
            makes.into_iter().map(Make::from).collect::<Vec<_>>(),
            MOST_LISTED,
        ),
        Err(error) => return site::failed(&state, &error),
    };
    if format == Format::Markdown {
        let canonical = format!("{}/makes", state.config().base_url);
        return markdown::response(makes_markdown(year, &makes, cut_short), &canonical);
    }
    let page = Page::new(
        &state,
        seo::title("Car and truck makes, with models by year"),
        "Every make of car, multipurpose vehicle and truck in NHTSA's data, with its models by year, their trims and engines.",
    )
    .in_section("makes");
    // Only the full list is indexed; a list for one year is a filter of it.
    let page = match year {
        None => page.indexed(&state, "/makes").with_markdown("/makes.md"),
        Some(_) => page,
    };
    let cut = cut_short.then_some(MOST_LISTED);
    let (popular, others) = makes.into_iter().partition(|make| make.popular);
    site::html(
        StatusCode::OK,
        &MakesPage {
            page,
            year,
            popular,
            others,
            cut,
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
    /// Whether the make has more models that year than are listed.
    pub models_cut: bool,
    /// Whether the make builds cars, MPVs or trucks. Only those are offered
    /// to search engines.
    pub light: bool,
    /// The manufacturer codes the make's vehicles are built under.
    pub codes: Vec<String>,
    pub codes_cut: bool,
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
    // The years come from the model years themselves, in the scope the
    // models are listed in. A model's own range will not do: it runs over
    // years the model was not built in, and over years in which it was not
    // a light vehicle.
    let years: Vec<u16> = worker
        .source
        .query(
            MAKE_YEARS_SQL,
            &[make.id.into(), i64::from(make.light).into()],
        )
        .map_err(CatalogError::Source)?
        .iter()
        .filter_map(|row| row.first().and_then(Value::integer))
        .filter_map(|year| u16::try_from(year).ok())
        .collect();
    let Some(newest) = years.first().copied() else {
        return Ok(Found::Nothing);
    };
    let year = year.filter(|year| years.contains(year)).unwrap_or(newest);
    let (models, models_cut) = cut(
        worker
            .catalog
            .models(&make.slug, Some(year), scope, "", MOST_LISTED + 1)?,
        MOST_LISTED,
    );
    let found: Vec<String> = worker
        .source
        .query(MAKE_CODES_SQL, &[make.id.into()])
        .map_err(CatalogError::Source)?
        .iter()
        .filter_map(|row| row.first().and_then(Value::text))
        // A code goes into an address, so only what is a code is kept.
        .filter(|code| site::wmi::is_code(code))
        .map(str::to_owned)
        .collect();
    let (codes, codes_cut) = cut(found, MOST_CODES);
    Ok(Found::View(MakeView {
        slug: make.slug.clone(),
        name: make.name.clone(),
        year,
        years,
        models: models.into_iter().map(Model::from).collect(),
        models_cut,
        light: make.light,
        codes,
        codes_cut,
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
    if view.models_cut {
        text.push_str(&format!(
            "\nOnly the first {} models are listed.\n",
            view.models.len()
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
    if !view.codes.is_empty() {
        text.push_str("\n## Manufacturer codes\n\n");
        let codes: Vec<String> = view
            .codes
            .iter()
            .map(|code| format!("[{code}](/wmi/{code}.md)"))
            .collect();
        text.push_str(&codes.join(" "));
        text.push('\n');
    }
    text
}

pub async fn make(
    State(state): State<AppState>,
    path: Result<Path<String>, PathRejection>,
    query: Result<Query<YearQuery>, QueryRejection>,
) -> Response {
    // An address that is not text names no make.
    let Ok(Path(segment)) = path else {
        return site::not_found(&state);
    };
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
            let year = asked
                .map(|year| format!("?year={year}"))
                .unwrap_or_default();
            return Redirect::permanent(&format!("/makes/{slug}{suffix}{year}")).into_response();
        }
        Found::Elsewhere(_) | Found::Nothing => return site::not_found(&state),
    };
    let path = format!("/makes/{}", view.slug);
    if format == Format::Markdown {
        let canonical = format!("{}{path}", state.config().base_url);
        return markdown::response(make_markdown(&view), &canonical);
    }
    let name = &view.name;
    let models = match (view.years.last(), view.years.first()) {
        (Some(first), Some(last)) if first != last => format!(
            "{name} models for every model year from {first} to {last}, with trims and engines."
        ),
        _ => format!(
            "{name} models for {year}, with trims and engines.",
            year = view.year
        ),
    };
    let description = seo::description(
        &[
            &[models],
            &[
                format!("Decode {name} VINs free, with no account and no key."),
                format!("Decode {name} VINs free."),
            ],
        ],
        160,
    );
    let page = Page::new(
        &state,
        seo::title(&format!("{} VIN decoder and models by year", view.name)),
        description,
    )
    .in_section("makes")
    .under(vec![("Makes".to_owned(), "/makes".to_owned())]);
    let trail = page.trail(&view.name);
    let page = page.describing(vec![trail]);
    // The page without a year is the one to index, and only for a make of
    // cars, MPVs or trucks: a trailer maker's page is there for whoever
    // asks for it, and is in no sitemap.
    let page = match asked {
        None if view.light => page
            .indexed(&state, &path)
            .with_markdown(&format!("{path}.md")),
        None => page.with_markdown(&format!("{path}.md")),
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

/// An engine, with how a VIN tells it from the others.
pub struct EngineLine {
    pub label: String,
    /// Whether the engine comes from this project's list instead of NHTSA's.
    pub preset: bool,
    /// The characters in position 8 of a VIN that mean this engine. Any one
    /// of them does: they are alternatives, not a sequence. Empty where the
    /// data does not settle it.
    pub vin8: Vec<char>,
}

/// Alternatives in words: `5`, `4 or G`, `3, 4 or 9`.
fn any_of(characters: &[char]) -> String {
    let mut text = String::new();
    for (index, character) in characters.iter().enumerate() {
        if index > 0 {
            text.push_str(if index + 1 == characters.len() {
                " or "
            } else {
                ", "
            });
        }
        text.push(*character);
    }
    text
}

pub struct ModelYearView {
    pub entry: Entry,
    pub make_slug: String,
    pub model_slug: String,
    pub trims: Vec<TrimLine>,
    /// How many trims the model year has; `trims` holds at most 60 of them.
    pub trims_total: usize,
    pub engines: Vec<EngineLine>,
    /// Whether this model year is a car, an MPV or a truck. Only those are
    /// offered to search engines.
    pub light: bool,
    /// The model's other light-vehicle years, newest first, this one among
    /// them. Empty for a trailer, a motorcycle or a bus.
    pub years: Vec<u16>,
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
    let submodels = worker.catalog.submodels(make, model, year, "")?;
    let trims_total = submodels.len();
    let mut trims = Vec::new();
    for submodel in submodels.into_iter().take(MOST_TRIMS) {
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
    let engines = worker
        .catalog
        .engines(make, model, year, None, "")?
        .into_iter()
        .map(|engine| EngineLine {
            label: engine.label,
            preset: engine.preset,
            vin8: engine
                .vin8
                .unwrap_or_default()
                .chars()
                .filter(|character| !character.is_whitespace())
                .collect(),
        })
        .collect();
    // The same question the sitemap for a model year asks, so a page is
    // marked for indexing exactly when a sitemap lists it.
    let model_years: Vec<(u16, bool)> = worker
        .source
        .query(MODEL_YEARS_SQL, &[make.into(), model.into()])
        .map_err(CatalogError::Source)?
        .iter()
        .filter_map(|row| {
            let found = row
                .first()
                .and_then(Value::integer)
                .and_then(|found| u16::try_from(found).ok())?;
            Some((found, row.get(1).and_then(Value::integer) == Some(1)))
        })
        .collect();
    let light = model_years
        .iter()
        .any(|(found, light)| *found == year && *light);
    let mut years: Vec<u16> = model_years
        .iter()
        .filter(|(_, light)| *light)
        .map(|(found, _)| *found)
        .collect();
    years.dedup();
    Ok(Some(ModelYearView {
        entry: Entry::from(entry),
        make_slug: make.to_owned(),
        model_slug: model.to_owned(),
        trims,
        trims_total,
        engines,
        light,
        years,
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
        if trim.kind == "preset" {
            text.push_str(" (from our own list, not NHTSA's)");
        }
        text.push('\n');
    }
    if view.trims_total > view.trims.len() {
        text.push_str(&format!(
            "\nThe first {} of {} trims are shown.\n",
            view.trims.len(),
            view.trims_total
        ));
    }
    text.push_str("\n## Engines\n\n");
    if view.engines.is_empty() {
        text.push_str("None on file.\n");
    }
    for engine in &view.engines {
        text.push_str(&format!("- {}", escape(&engine.label)));
        if !engine.vin8.is_empty() {
            text.push_str(&format!(
                " (eighth VIN character {})",
                escape(&any_of(&engine.vin8))
            ));
        }
        if engine.preset {
            text.push_str(" (from our own list, not NHTSA's)");
        }
        text.push('\n');
    }
    let others: Vec<String> = view
        .years
        .iter()
        .filter(|year| **year != entry.year)
        .map(|year| {
            format!(
                "[{year}](/makes/{}/{}/{year}.md)",
                view.make_slug, view.model_slug
            )
        })
        .collect();
    if !others.is_empty() {
        text.push_str("\n## Other years\n\n");
        text.push_str(&others.join(" "));
        text.push('\n');
    }
    text.push_str(&format!(
        "\nJSON: [/v1/vehicles/{id}](/v1/vehicles/{id})\n",
        id = entry.id
    ));
    text
}

/// What a search result says of a model year: what is on file, counted.
fn describe_model_year(view: &ModelYearView, name: &str) -> String {
    let trims: Vec<&str> = view.trims.iter().map(|trim| trim.name.as_str()).collect();
    let engines: Vec<&str> = view
        .engines
        .iter()
        .map(|engine| engine.label.as_str())
        .collect();
    let trims_counted = seo::count(view.trims_total, "trim");
    let engines_counted = seo::count(engines.len(), "engine");
    let trims_in_words = format!("{trims_counted} ({})", seo::first_of(&trims));
    let engines_in_words = format!("{engines_counted} ({})", seo::first_of(&engines));
    // Each sentence from its fullest wording to its shortest: the closing
    // clause goes first, then the names, and the counts stay.
    let (what, missing) = match (trims.is_empty(), engines.is_empty()) {
        (true, true) => (
            vec![format!("The {name}, from NHTSA's data.")],
            Some("No trims or engines are on file for this model year."),
        ),
        (false, true) => (
            vec![
                format!("The {name} has {trims_in_words}."),
                format!("The {name} has {trims_counted}."),
            ],
            Some("No engines are on file."),
        ),
        (true, false) => (
            vec![
                format!(
                    "The {name} has {engines_in_words}, with the VIN character for each engine."
                ),
                format!("The {name} has {engines_in_words}."),
                format!("The {name} has {engines_counted}."),
            ],
            Some("No trims are on file."),
        ),
        (false, false) => (
            vec![
                format!(
                    "The {name} has {trims_in_words} and {engines_in_words}, with the VIN character for each engine."
                ),
                format!("The {name} has {trims_in_words} and {engines_in_words}."),
                format!(
                    "The {name} has {trims_counted} and {engines_counted}, with the VIN character for each engine."
                ),
                format!("The {name} has {trims_counted} and {engines_counted}."),
            ],
            None,
        ),
    };
    let missing: Vec<String> = missing.iter().map(|text| (*text).to_owned()).collect();
    seo::description(&[&what, &missing], 160)
}

pub async fn model_year(
    State(state): State<AppState>,
    path: Result<Path<(String, String, String)>, PathRejection>,
) -> Response {
    let Ok(Path((make, model, segment))) = path else {
        return site::not_found(&state);
    };
    let (year, format) = markdown::split(&segment);
    let Some(year) = site::year_in(year) else {
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
        seo::title(&format!("{name} trims and engines")),
        describe_model_year(&view, &name),
    )
    .with_markdown(&format!("{path}.md"))
    .in_section("makes")
    .under(vec![
        ("Makes".to_owned(), "/makes".to_owned()),
        (
            view.entry.make.clone(),
            format!("/makes/{}", view.make_slug),
        ),
    ]);
    let engines: Vec<String> = view
        .engines
        .iter()
        .map(|engine| engine.label.clone())
        .collect();
    let things = vec![
        page.trail(&name),
        jsonld::car(
            &format!("{}{path}", state.config().base_url),
            &view.entry,
            &engines,
        ),
    ];
    let page = page.describing(things);
    // A trailer, a motorcycle or a bus has a page for whoever asks for it,
    // and is in no sitemap.
    let page = if view.light {
        page.indexed(&state, &path)
    } else {
        page
    };
    site::html(StatusCode::OK, &ModelYearPage { page, view, name })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn alternatives_are_listed_in_words() {
        assert_eq!(any_of(&[]), "");
        assert_eq!(any_of(&['5']), "5");
        assert_eq!(any_of(&['4', 'G']), "4 or G");
        assert_eq!(any_of(&['3', '4', '9']), "3, 4 or 9");
        assert_eq!(any_of(&['A', 'B', 'C', 'D']), "A, B, C or D");
    }

    #[test]
    fn a_list_is_cut_at_the_most_and_says_so() {
        assert_eq!(cut(vec![1, 2, 3], 2), (vec![1, 2], true));
        assert_eq!(cut(vec![1, 2], 2), (vec![1, 2], false));
        assert_eq!(cut(Vec::<u8>::new(), 2), (Vec::new(), false));
    }
}
