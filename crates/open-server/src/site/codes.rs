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
use wenmar_shopdata::codes::{CodeQuery, CodeSummary, MOST_CODES_LISTED, NOTE, SYSTEMS};

use crate::api::codes::code_op;
use crate::api::types::Code;
use crate::error::ApiError;
use crate::site::markdown::{self, Format, escape};
use crate::site::{self, Page, jsonld, report, seo};
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
