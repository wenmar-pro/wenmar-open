//! `/vins`: decode a pasted list of VINs, as a table or a CSV file.
//!
//! Like a single VIN's page, it is never indexed and never kept by anyone
//! but the visitor. It needs no JavaScript: the form posts the list.

use askama::Template;
use axum::extract::{Form, State};
use axum::http::{HeaderValue, StatusCode, header};
use axum::response::{IntoResponse, Response};
use serde::Deserialize;

use crate::api::types::VinDecode;
use crate::api::vin::{MOST_VINS, decode_many};
use crate::error::ApiError;
use crate::site::{self, Page};
use crate::state::AppState;
use wenmar_shopdata::batch as batch_out;

/// One line of the result table.
struct Line {
    vin: String,
    /// Whether the VIN decoded, so its own page exists.
    link: bool,
    vehicle: String,
    engine: String,
    drive: String,
    body: String,
    generation: String,
    note: String,
}

#[derive(Template)]
#[template(path = "vins.html")]
struct Vins {
    page: Page,
    most: usize,
    typed: String,
    note: Option<String>,
    rows: Vec<Line>,
}

#[derive(Debug, Default, Deserialize)]
pub struct Pasted {
    #[serde(default)]
    vins: String,
    format: Option<String>,
}

fn page(state: &AppState) -> Page {
    let mut page = Page::new(
        state,
        "Decode a list of VINs - Wenmar Open",
        "Paste a list of VINs and get each vehicle back, as a table or a CSV file.",
    )
    .with_mono();
    page.section = "";
    page
}

fn private(mut response: Response) -> Response {
    let headers = response.headers_mut();
    headers.insert(header::CACHE_CONTROL, HeaderValue::from_static("no-store"));
    headers.insert("x-robots-tag", HeaderValue::from_static("noindex"));
    response
}

fn render(
    state: &AppState,
    status: StatusCode,
    typed: String,
    note: Option<String>,
    rows: Vec<Line>,
) -> Response {
    private(site::html(
        status,
        &Vins {
            page: page(state),
            most: MOST_VINS,
            typed,
            note,
            rows,
        },
    ))
}

pub async fn show(State(state): State<AppState>) -> Response {
    render(&state, StatusCode::OK, String::new(), None, Vec::new())
}

fn line(vin: &str, result: &Result<VinDecode, ApiError>) -> Line {
    match result {
        Ok(decode) => {
            let vehicle = crate::site::vin::headline(decode);
            let warning = decode
                .warnings
                .iter()
                .map(|warning| warning.message.clone())
                .collect::<Vec<_>>();
            let mut notes = warning;
            if decode.year.is_some() && !decode.year_conclusive {
                notes.push("Model year not settled by the VIN.".to_owned());
            }
            Line {
                vin: decode.vin.clone(),
                link: true,
                vehicle,
                engine: decode
                    .engine
                    .as_ref()
                    .and_then(|engine| engine.label.clone())
                    .unwrap_or_default(),
                drive: decode.drivetrain.clone().unwrap_or_default(),
                body: decode.body.clone().unwrap_or_default(),
                generation: decode
                    .generation
                    .as_ref()
                    .map(crate::site::vin::generation_line)
                    .unwrap_or_default(),
                note: notes.join(" "),
            }
        }
        Err(error) => Line {
            vin: vin.to_owned(),
            link: false,
            vehicle: String::new(),
            engine: String::new(),
            drive: String::new(),
            body: String::new(),
            generation: String::new(),
            note: error.body().error.message.clone(),
        },
    }
}

pub async fn decode(
    State(state): State<AppState>,
    form: Result<Form<Pasted>, axum::extract::rejection::FormRejection>,
) -> Response {
    let Ok(Form(pasted)) = form else {
        return render(
            &state,
            StatusCode::BAD_REQUEST,
            String::new(),
            Some("The list could not be read. Paste it again.".to_owned()),
            Vec::new(),
        );
    };
    let mut vins = batch_out::find_vins(&pasted.vins);
    if vins.is_empty() {
        return render(
            &state,
            StatusCode::OK,
            pasted.vins,
            Some("No 17-character VIN was found in what was pasted.".to_owned()),
            Vec::new(),
        );
    }
    let found = vins.len();
    vins.truncate(MOST_VINS);
    let note = (found > MOST_VINS).then(|| {
        format!("{found} VINs were pasted. The first {MOST_VINS} are decoded; paste the rest next.")
    });
    let decoded = match decode_many(&state, vins.clone(), None).await {
        Ok(decoded) => decoded,
        Err(error) => return private(site::failed(&state, &error)),
    };
    if pasted.format.as_deref() == Some("csv") {
        let items: Vec<serde_json::Value> = decoded
            .iter()
            .map(|result| match result {
                Ok(decode) => serde_json::to_value(decode).unwrap_or_default(),
                Err(error) => serde_json::to_value(error.body()).unwrap_or_default(),
            })
            .collect();
        let text = batch_out::csv(&items, &vins, None);
        return private(
            (
                [
                    (header::CONTENT_TYPE, "text/csv; charset=utf-8"),
                    (
                        header::CONTENT_DISPOSITION,
                        "attachment; filename=\"vins.csv\"",
                    ),
                ],
                text,
            )
                .into_response(),
        );
    }
    let rows = vins
        .iter()
        .zip(decoded.iter())
        .map(|(vin, result)| line(vin, result))
        .collect();
    render(&state, StatusCode::OK, pasted.vins, note, rows)
}
