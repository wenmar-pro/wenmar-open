//! The trouble code pages: `/codes`, one code, and a code on a model
//! year.
//!
//! A page shows this project's own description of a generic code. It is
//! general: no page says what is wrong with a vehicle. The pages are
//! built from the same answers the API gives.

use askama::Template;
use axum::extract::rejection::{PathRejection, QueryRejection};
use axum::extract::{Path, Query, State};
use axum::http::StatusCode;
use axum::response::Response;
use serde::Deserialize;
use wenmar_shopdata::Dataset;
use wenmar_shopdata::codes::{
    CodeQuery, CodeSummary, MOST_CODES_LISTED, NOTE, SYSTEMS, VEHICLE_NOTE, is_code,
};
use wenmar_vehicles::text::is_slug;

use crate::api::codes::{code_op, vehicle_code_of, vehicle_code_shop};
use crate::api::recalls::model_year;
use crate::api::types::{Bulletin, Code, VehicleCode};
use crate::error::ApiError;
use crate::site::markdown::{self, Format, escape};
use crate::site::{self, Page, bulletins, jsonld, report, seo};
use crate::state::AppState;

/// What the index may be narrowed by.
#[derive(Debug, Default, Deserialize)]
pub struct IndexQuery {
    q: Option<String>,
}

/// The codes of one system, as the index lists them.
pub struct Group {
    pub name: &'static str,
    /// Code and title.
    pub codes: Vec<(String, String)>,
}

/// Codes by system, in the order of [`SYSTEMS`]. A system with no code is
/// left out.
fn grouped(codes: Vec<CodeSummary>) -> Vec<Group> {
    SYSTEMS
        .iter()
        .filter_map(|(id, name)| {
            let codes: Vec<(String, String)> = codes
                .iter()
                .filter(|code| code.system == *id)
                .map(|code| (code.code.clone(), code.title.clone()))
                .collect();
            (!codes.is_empty()).then_some(Group { name, codes })
        })
        .collect()
}

#[derive(Template)]
#[template(path = "codes.html")]
struct IndexPage {
    page: Page,
    note: &'static str,
    /// What was searched for. Empty when nothing was.
    q: String,
    searched: bool,
    total: usize,
    groups: Vec<Group>,
    dated: String,
}

/// The codes the index shows: all of them, or those matching `q`.
async fn listed(state: &AppState, q: Option<String>) -> Result<Vec<CodeSummary>, ApiError> {
    let shop = state.dataset(Dataset::Codes)?;
    let found = shop
        .run(move |reader| match q {
            None => reader.all_codes(),
            Some(q) => reader
                .codes(&CodeQuery {
                    q: Some(q),
                    limit: MOST_CODES_LISTED,
                    ..CodeQuery::default()
                })
                .map(|page| page.codes),
        })
        .await??;
    Ok(found)
}

fn as_of(state: &AppState) -> String {
    state
        .shop()
        .and_then(|shop| shop.datasets().get(Dataset::Codes))
        .map(|entry| format!("as of {}", entry.as_of))
        .unwrap_or_default()
}

/// `/codes`: every described code by system, and a search box.
pub async fn index(
    State(state): State<AppState>,
    query: Result<Query<IndexQuery>, QueryRejection>,
) -> Response {
    if state.dataset(Dataset::Codes).is_err() {
        return site::not_found(&state);
    }
    let q = query
        .ok()
        .and_then(|Query(query)| query.q)
        .map(|text| text.trim().chars().take(100).collect::<String>())
        .filter(|text| !text.is_empty());
    let codes = match listed(&state, q.clone()).await {
        Ok(codes) => codes,
        Err(error) => return site::failed(&state, &error),
    };
    let page = Page::new(
        &state,
        seo::title("OBD-II trouble codes"),
        "Generic OBD-II trouble codes, each described in plain words: what it means, common causes and what to check first.",
    )
    .with_markdown("/codes.md")
    .with_mono()
    .indexed(&state, "/codes");
    let page = if q.is_some() { page.with_query() } else { page };
    site::html(
        StatusCode::OK,
        &IndexPage {
            page,
            note: NOTE,
            searched: q.is_some(),
            q: q.unwrap_or_default(),
            total: codes.len(),
            groups: grouped(codes),
            dated: as_of(&state),
        },
    )
}

/// `/codes.md`.
pub async fn index_md(State(state): State<AppState>) -> Response {
    let codes = match listed(&state, None).await {
        Ok(codes) => codes,
        Err(ApiError::DatasetUnavailable(_)) => return site::not_found(&state),
        Err(error) => return site::failed(&state, &error),
    };
    let base = &state.config().base_url;
    let mut text = format!(
        "# Trouble codes\n\nGeneric OBD-II trouble codes, each described for this site in its own words. {}\n",
        escape(NOTE)
    );
    for group in grouped(codes) {
        text.push_str(&format!("\n## {}\n\n", group.name));
        for (code, title) in group.codes {
            text.push_str(&format!(
                "- [{code}]({base}/codes/{code}): {}\n",
                escape(&title)
            ));
        }
    }
    markdown::response(text, &format!("{base}/codes"))
}

#[derive(Template)]
#[template(path = "code.html")]
struct CodePage {
    page: Page,
    code: Code,
    report: report::ReportLinks,
}

/// A description as Markdown, from its heading down. `level` is the
/// heading's depth.
pub fn description_markdown(code: &Code, level: usize) -> String {
    let marks = "#".repeat(level);
    let mut text = format!(
        "{marks} {}: {}\n\n{}\n",
        code.code,
        escape(&code.title),
        escape(&code.meaning)
    );
    for (heading, entries) in [
        ("Common causes", &code.common_causes),
        ("Check first", &code.checks),
    ] {
        text.push_str(&format!("\n{marks}# {heading}\n\n"));
        for entry in entries {
            text.push_str(&format!("- {}\n", escape(entry)));
        }
    }
    text.push_str(&format!("\nSystem: {}\n", code.system_name));
    if !code.components.is_empty() {
        let names: Vec<String> = code
            .components
            .iter()
            .map(|component| escape(&component.name))
            .collect();
        text.push_str(&format!("\nComponents: {}\n", names.join(", ")));
    }
    if !code.related.is_empty() {
        let links: Vec<String> = code
            .related
            .iter()
            .map(|other| format!("[{}]({}) {}", other.code, other.url, escape(&other.title)))
            .collect();
        text.push_str(&format!("\nRelated codes: {}\n", links.join(", ")));
    }
    text.push_str(&format!(
        "\n{} Reviewed by {} on {}.\n",
        escape(&code.note),
        escape(&code.reviewer),
        code.checked
    ));
    text
}

/// `/codes/{code}` and its Markdown twin. Only the stored form of a code
/// is an address: capitals, five characters.
pub async fn code(
    State(state): State<AppState>,
    path: Result<Path<String>, PathRejection>,
) -> Response {
    let Ok(Path(segment)) = path else {
        return site::not_found(&state);
    };
    let (stem, format) = markdown::split(&segment);
    if !wenmar_shopdata::codes::is_code(stem) {
        return site::not_found(&state);
    }
    let found = match code_op(&state, stem.to_owned()).await {
        Ok(found) => found,
        Err(ApiError::NotFound(_) | ApiError::DatasetUnavailable(_)) => {
            return site::not_found(&state);
        }
        Err(error) => return site::failed(&state, &error),
    };
    let path = format!("/codes/{}", found.code);
    if format == Format::Markdown {
        let canonical = format!("{}{path}", state.config().base_url);
        return markdown::response(description_markdown(&found, 1), &canonical);
    }
    let heading = format!("{} {}", found.code, found.title);
    let description = seo::description(
        &[&[
            format!("{}: {}.", found.code, found.title),
            format!("Trouble code {}.", found.code),
        ]],
        160,
    );
    let page = Page::new(&state, seo::title(&heading), description)
        .with_markdown(&format!("{path}.md"))
        .with_json(&format!("/v1{path}"))
        .with_mono()
        .as_article()
        .under(vec![("Trouble codes".to_owned(), "/codes".to_owned())]);
    let things = vec![
        page.trail(&found.code),
        jsonld::defined_term(&state.config().base_url, &found.code, &found.title),
    ];
    let page = page.describing(things).indexed(&state, &path);
    let report = report::links(state.config(), &path, &found.code);
    site::html(
        StatusCode::OK,
        &CodePage {
            page,
            code: found,
            report,
        },
    )
}

// ----- a code on a model year -----

/// A recall beside a code.
pub struct RecallLine {
    pub campaign: String,
    pub href: String,
    pub date: String,
    pub component: String,
}

#[derive(Template)]
#[template(path = "code_model_year.html")]
struct ModelYearCodePage {
    page: Page,
    code: String,
    /// `2023 Hyundai Kona`.
    name: String,
    note: &'static str,
    generic: bool,
    description: Option<Code>,
    /// How many bulletins of the model year name the code. `None` where
    /// bulletins are not held.
    named_count: Option<u64>,
    naming: Vec<bulletins::Line>,
    bulletins_href: String,
    about: Vec<bulletins::Line>,
    recalls: Vec<RecallLine>,
    dated: String,
    model_year_path: String,
    /// Whether `/codes` exists on this server.
    codes_index: bool,
    report: report::ReportLinks,
}

fn bulletin_markdown(lines: &[Bulletin]) -> String {
    let mut text = String::new();
    for bulletin in lines {
        text.push_str(&format!(
            "- [{}]({}), {}",
            bulletin.nhtsa_id, bulletin.url, bulletin.date_added
        ));
        if let Some(summary) = &bulletin.summary {
            text.push_str(&format!(": {}", escape(summary)));
        }
        text.push('\n');
    }
    text
}

fn vehicle_markdown(name: &str, answer: &VehicleCode) -> String {
    let code = &answer.code;
    let mut text = format!(
        "# {code} on the {}\n\n{}\n",
        escape(name),
        escape(&answer.note)
    );
    match &answer.description {
        Some(found) => text.push_str(&format!("\n{}", description_markdown(found, 2))),
        None if answer.generic => {
            text.push_str("\nNobody has written a description of this code yet.\n");
        }
        None => {
            text.push_str("\nThis code is specific to a manufacturer and is not described here.\n")
        }
    }
    if let Some(named) = &answer.bulletins {
        text.push_str(&format!(
            "\n## Bulletins for this model year that name {code}\n\n"
        ));
        if named.bulletins.is_empty() {
            text.push_str("None on file.\n");
        } else {
            text.push_str(&bulletin_markdown(&named.bulletins));
        }
    }
    if let Some(about) = answer
        .related
        .bulletins
        .as_ref()
        .filter(|list| !list.is_empty())
    {
        text.push_str("\n## Other bulletins about the same components\n\n");
        text.push_str(&bulletin_markdown(about));
    }
    if let Some(recalls) = answer
        .related
        .recalls
        .as_ref()
        .filter(|found| !found.recalls.is_empty())
    {
        text.push_str("\n## Recalls about the same components\n\n");
        for recall in &recalls.recalls {
            text.push_str(&format!(
                "- [{}]({}), {}\n",
                escape(&recall.campaign),
                recall.url,
                recall.date.as_deref().unwrap_or("date not on file")
            ));
        }
        text.push_str(
            "\nA recall listed here may apply to the model year. Only the manufacturer can say whether it applies to one vehicle.\n",
        );
    }
    text
}

/// `/makes/{make}/{model}/{year}/codes/{code}` and its Markdown twin. Any
/// code answers where the server holds codes or bulletins. The page is
/// offered to search engines only when it shows a bulletin or a recall.
pub async fn model_year_code(
    State(state): State<AppState>,
    path: Result<Path<(String, String, String, String)>, PathRejection>,
) -> Response {
    let Ok(Path((make, model, year, segment))) = path else {
        return site::not_found(&state);
    };
    let (stem, format) = markdown::split(&segment);
    let Some(year) = site::year_in(&year) else {
        return site::not_found(&state);
    };
    if !is_code(stem) || !is_slug(&make) || !is_slug(&model) {
        return site::not_found(&state);
    }
    let Ok(shop) = vehicle_code_shop(&state) else {
        return site::not_found(&state);
    };
    let id = format!("{year:04}_{make}_{model}");
    let found = match state.db().run(move |worker| model_year(worker, &id)).await {
        Ok(Ok(Some(found))) => found,
        Ok(Ok(None)) => return site::not_found(&state),
        Ok(Err(error)) => return site::failed(&state, &ApiError::from(error)),
        Err(error) => return site::failed(&state, &ApiError::from(error)),
    };
    let answer = match vehicle_code_of(&state, shop, &found, stem.to_owned()).await {
        Ok(answer) => answer,
        Err(error) => return site::failed(&state, &error),
    };
    let model_year_path = format!("/makes/{}/{}/{year}", found.make_slug, found.model_slug);
    let path = format!("{model_year_path}/codes/{}", answer.code);
    let name = format!(
        "{} {} {}",
        answer.vehicle.year, answer.vehicle.make, answer.vehicle.model
    );
    if format == Format::Markdown {
        let canonical = format!("{}{path}", state.config().base_url);
        return markdown::response(vehicle_markdown(&name, &answer), &canonical);
    }
    let naming: Vec<bulletins::Line> = answer
        .bulletins
        .iter()
        .flat_map(|named| named.bulletins.iter().map(bulletins::line))
        .collect();
    let about: Vec<bulletins::Line> = answer
        .related
        .bulletins
        .iter()
        .flat_map(|list| list.iter().map(bulletins::line))
        .collect();
    let recalls: Vec<RecallLine> = answer
        .related
        .recalls
        .iter()
        .flat_map(|found| found.recalls.iter())
        .map(|recall| RecallLine {
            campaign: recall.campaign.clone(),
            href: format!("/recalls/{}/{}", recall.source, recall.campaign),
            date: recall
                .date
                .clone()
                .unwrap_or_else(|| "date not on file".to_owned()),
            component: recall.component.clone().unwrap_or_default(),
        })
        .collect();
    // Worth a search engine's while only with something of this vehicle
    // on it, and only for a car, an MPV or a truck.
    let shows_something = !naming.is_empty() || !about.is_empty() || !recalls.is_empty();
    let heading = format!("{} on the {name}", answer.code);
    let description = seo::description(
        &[&[
            format!(
                "Trouble code {} on the {name}: what it means, the service bulletins that name it and the recalls about the same components.",
                answer.code
            ),
            format!("Trouble code {} on the {name}.", answer.code),
        ]],
        160,
    );
    let page = Page::new(&state, seo::title(&heading), description)
        .with_markdown(&format!("{path}.md"))
        .with_json(&format!(
            "/v1/vehicles/{}/codes/{}",
            answer.vehicle.id, answer.code
        ))
        .with_mono()
        .in_section("makes")
        .under(vec![
            ("Makes".to_owned(), "/makes".to_owned()),
            (
                answer.vehicle.make.clone(),
                format!("/makes/{}", found.make_slug),
            ),
            (name.clone(), model_year_path.clone()),
        ]);
    let trail = page.trail(&answer.code);
    let page = page.describing(vec![trail]);
    let page = if found.light && shows_something {
        page.indexed(&state, &path)
    } else {
        page
    };
    let indexed = page.index;
    let report = report::links(state.config(), &path, &format!("{} {name}", answer.code));
    let mut response = site::html(
        StatusCode::OK,
        &ModelYearCodePage {
            page,
            name,
            note: VEHICLE_NOTE,
            generic: answer.generic,
            named_count: answer.bulletins.as_ref().map(|named| named.count),
            bulletins_href: format!("{model_year_path}/bulletins?code={}", answer.code),
            naming,
            about,
            recalls,
            dated: answer
                .bulletins
                .as_ref()
                .and_then(|named| named.as_of.as_ref())
                .map(|day| format!("as of {day}"))
                .unwrap_or_default(),
            model_year_path,
            codes_index: state.dataset(Dataset::Codes).is_ok(),
            description: answer.description,
            code: answer.code,
            report,
        },
    );
    if !indexed {
        site::unindexed(&mut response);
    }
    response
}
