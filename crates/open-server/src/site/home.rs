//! The home page and the two forms on it.

use askama::Template;
use axum::extract::rejection::QueryRejection;
use axum::extract::{Query, State};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Redirect, Response};
use serde::Deserialize;
use wenmar_vehicles::Scope;
use wenmar_vehicles::text::is_slug;

use crate::api::types::Make;
use crate::error::ApiError;
use crate::site::{self, Page};
use crate::state::AppState;

#[derive(Template)]
#[template(path = "home.html")]
struct Home {
    page: Page,
    years: Vec<u16>,
    makes: Vec<Make>,
    guides: &'static [crate::site::guides::Guide],
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
    site::html(
        StatusCode::OK,
        &Home {
            page: Page::new(
                &state,
                "Wenmar Open - free VIN decoder for auto repair shops",
                "Decode a VIN or pick a year, make and model. Free vehicle data for auto repair shops, with no account and no key.",
            )
            .indexed(&state, "/")
            .with_mono(),
            years,
            makes: makes.into_iter().map(Make::from).collect(),
            guides: &crate::site::guides::GUIDES,
        },
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
