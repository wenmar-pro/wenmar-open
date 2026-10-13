//! The home page and the two forms on it.

use askama::Template;
use axum::extract::rejection::QueryRejection;
use axum::extract::{Query, State};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Redirect, Response};
use serde::Deserialize;
use wenmar_shopdata::Dataset;
use wenmar_vehicles::Scope;
use wenmar_vehicles::text::is_slug;

use crate::api::types::Make;
use crate::error::ApiError;
use crate::site::{self, Page, jsonld, seo};
use crate::state::AppState;

#[derive(Template)]
#[template(path = "home.html")]
struct Home {
    page: Page,
    years: Vec<u16>,
    makes: Vec<Make>,
    /// The lists this server holds beside the catalog, as name and
    /// address: recalls, bulletins, trouble codes, repair jobs.
    lookups: Vec<(&'static str, &'static str)>,
    guides: &'static [crate::site::guides::Guide],
    tools: &'static [&'static crate::site::tools::Tool],
}

pub async fn home(State(state): State<AppState>) -> Response {
    let years = state.years(Scope::Light).unwrap_or_default().to_vec();
    let makes = state
        .db()
        .run(|worker| worker.catalog.makes(None, Scope::Light, "", 500))
        .await;
    let makes = match makes {
        Ok(Ok(makes)) => makes,
        Ok(Err(error)) => return site::failed(&state, &ApiError::from(error)),
        Err(error) => return site::failed(&state, &ApiError::from(error)),
    };
    let lookups = lookups(&state);
    let (title, description) = describe(&lookups);
    site::html(
        StatusCode::OK,
        &Home {
            page: Page::new(&state, seo::title(title), description)
                .indexed(&state, "/")
                .with_mono()
                .describing(vec![
                    jsonld::website(&state.config().base_url),
                    jsonld::organization(&state.config().base_url),
                ]),
            years,
            makes: makes.into_iter().map(Make::from).collect(),
            lookups,
            guides: &crate::site::guides::GUIDES,
            tools: &crate::site::tools::TOOLS,
        },
    )
}

/// The lists a visitor can start from, on a server that holds them. They
/// are named here because the home page is where a crawler starts too.
fn lookups(state: &AppState) -> Vec<(&'static str, &'static str)> {
    let mut held = Vec::new();
    if crate::llms::holds_recalls(state) {
        held.push(("Check a vehicle for recalls", "/recalls"));
    }
    if crate::llms::holds_bulletins(state) {
        held.push(("Service bulletins by vehicle", "/bulletins"));
    }
    if state.dataset(Dataset::Codes).is_ok() {
        held.push(("OBD-II trouble codes", "/codes"));
    }
    if state.dataset(Dataset::Jobs).is_ok() {
        held.push(("Repair jobs and labour times", "/jobs"));
    }
    held
}

/// The home page's title and description. They name what this server
/// holds: a server with the decoder and the catalog alone says so.
fn describe(lookups: &[(&'static str, &'static str)]) -> (&'static str, String) {
    let holds = |path: &str| lookups.iter().any(|(_, held)| *held == path);
    let mut named = Vec::new();
    if holds("/recalls") {
        named.push("recalls");
    }
    if holds("/bulletins") {
        named.push("service bulletins");
    }
    if holds("/codes") {
        named.push("trouble codes");
    }
    if holds("/jobs") {
        named.push("labour times");
    }
    let Some((last, first)) = named.split_last() else {
        return (
            "Free VIN decoder and shop calculators",
            "Decode any 17-character VIN to its year, make, model, trim, engine and safety equipment, and use free calculators for a repair shop's prices. No account and no key.".to_owned(),
        );
    };
    let named = if first.is_empty() {
        (*last).to_owned()
    } else {
        format!("{} and {last}", first.join(", "))
    };
    let title = if holds("/recalls") {
        "Free VIN decoder, recalls and repair shop data"
    } else {
        "Free VIN decoder and repair shop data"
    };
    (
        title,
        format!(
            "Decode a VIN to its year, make, model, trim and engine, then look up {named} for that vehicle. Free shop calculators too. No account and no key."
        ),
    )
}

#[derive(Debug, Default, Deserialize)]
pub struct VinForm {
    vin: Option<String>,
}

/// Where the VIN box sends its text: on to the result page, with the VIN
/// tidied so one vehicle has one address.
pub async fn vin_form(query: Result<Query<VinForm>, QueryRejection>) -> Response {
    let typed = query
        .map(|Query(form)| form.vin.unwrap_or_default())
        .unwrap_or_default();
    // Only letters and digits go into the address. Whatever else was typed
    // is not a VIN character, and the result page says what is wrong.
    let tidy: String = wenmar_vin::vin::normalize(&typed)
        .chars()
        .filter(char::is_ascii_alphanumeric)
        .take(crate::api::vin::LONGEST_INPUT)
        .collect();
    if tidy.is_empty() {
        return Redirect::to("/").into_response();
    }
    Redirect::to(&format!("/vin/{tidy}")).into_response()
}

#[derive(Debug, Default, Deserialize)]
pub struct Pick {
    year: Option<String>,
    make: Option<String>,
}

/// Where the year and make picker sends its choice.
pub async fn pick(query: Result<Query<Pick>, QueryRejection>) -> Response {
    let Query(pick) = query.unwrap_or_default();
    let year = pick
        .year
        .and_then(|year| site::year_in(year.trim()))
        .map(|year| format!("?year={year}"))
        .unwrap_or_default();
    match pick.make.as_deref().map(str::trim) {
        Some(make) if is_slug(make) => Redirect::to(&format!("/makes/{make}{year}")),
        _ => Redirect::to(&format!("/makes{year}")),
    }
    .into_response()
}
