//! `/wmi/{code}`: who a manufacturer code belongs to and what it builds.

use askama::Template;
use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Redirect, Response};
use wenmar_vehicles::{Source, SourceError, Value};

use crate::db::Worker;
use crate::error::ApiError;
use crate::site::markdown::{self, Format, escape};
use crate::site::{self, Page};
use crate::state::AppState;

const MANUFACTURER_SQL: &str = "
SELECT code, manufacturer, make, country, vehicle_type FROM wmi WHERE code = ?1";

const MAKES_SQL: &str = "
SELECT mk.slug, mk.name
FROM wmi_make wm
JOIN catalog_make mk ON mk.id = wm.make_id
WHERE wm.wmi = ?1";

const YEARS_SQL: &str = "SELECT year_from, year_to FROM wmi_schema WHERE wmi = ?1";

/// One manufacturer code.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WmiView {
    pub code: String,
    pub manufacturer: String,
    pub make: Option<String>,
    pub country: Option<String>,
    pub vehicle_type: Option<String>,
    /// The catalog makes built under this code, as id form and name.
    pub makes: Vec<(String, String)>,
    /// The first model year NHTSA has data for under this code.
    pub year_from: Option<u16>,
    /// The last one, or `None` when the code is still in use.
    pub year_to: Option<u16>,
}

/// Whether `code` could be a manufacturer code: three characters, or six
/// for a low-volume maker, all capital letters and digits.
pub fn is_code(code: &str) -> bool {
    matches!(code.len(), 3 | 6)
        && code
            .bytes()
            .all(|byte| byte.is_ascii_digit() || byte.is_ascii_uppercase())
}

fn optional(row: &[Value], column: usize) -> Option<String> {
    row.get(column)
        .and_then(Value::text)
        .map(str::to_owned)
        .filter(|text| !text.trim().is_empty())
}

fn load_wmi(worker: &Worker, code: &str) -> Result<Option<WmiView>, SourceError> {
    let source = &worker.source;
    let found = source.query(MANUFACTURER_SQL, &[code.into()])?;
    let Some(row) = found.first() else {
        return Ok(None);
    };
    let mut makes: Vec<(String, String)> = source
        .query(MAKES_SQL, &[code.into()])?
        .iter()
        .filter_map(|row| Some((optional(row, 0)?, optional(row, 1)?)))
        .collect();
    makes.sort_by(|left, right| left.1.cmp(&right.1));
    makes.dedup();

    let mut year_from: Option<u16> = None;
    let mut year_to: Option<u16> = None;
    let mut current = false;
    for row in source.query(YEARS_SQL, &[code.into()])? {
        let year = |column: usize| {
            row.get(column)
                .and_then(Value::integer)
                .and_then(|year| u16::try_from(year).ok())
        };
        if let Some(from) = year(0) {
            year_from = Some(year_from.map_or(from, |known| known.min(from)));
        }
        match year(1) {
            Some(to) => year_to = Some(year_to.map_or(to, |known| known.max(to))),
            None => current = true,
        }
    }
    Ok(Some(WmiView {
        code: code.to_owned(),
        manufacturer: optional(row, 1).unwrap_or_default(),
        make: optional(row, 2),
        country: optional(row, 3),
        vehicle_type: optional(row, 4),
        makes,
        year_from,
        year_to: if current { None } else { year_to },
    }))
}

/// The model years on file, in words.
pub fn years(view: &WmiView) -> Option<String> {
    match (view.year_from, view.year_to) {
        (Some(from), None) => Some(format!("{from} to now")),
        (Some(from), Some(to)) if from == to => Some(from.to_string()),
        (Some(from), Some(to)) => Some(format!("{from} to {to}")),
        (None, _) => None,
    }
}

#[derive(Template)]
#[template(path = "wmi.html")]
struct WmiPage {
    page: Page,
    view: WmiView,
    years: Option<String>,
    /// Positions 1 to 3 of a VIN.
    first: String,
    /// Positions 12 to 14, for a six-character code.
    rest: Option<String>,
}

fn wmi_markdown(view: &WmiView) -> String {
    let mut text = format!(
        "# Manufacturer code {}\n\n- Manufacturer: {}\n",
        view.code,
        escape(&view.manufacturer)
    );
    for (label, value) in [
        ("Make", &view.make),
        ("Country", &view.country),
        ("Vehicle type", &view.vehicle_type),
    ] {
        if let Some(value) = value {
            text.push_str(&format!("- {label}: {}\n", escape(value)));
        }
    }
    if let Some(years) = years(view) {
        text.push_str(&format!("- Model years on file: {years}\n"));
    }
    if !view.makes.is_empty() {
        text.push_str("\n## Makes\n\n");
        for (slug, name) in &view.makes {
            text.push_str(&format!("- [{}](/makes/{slug}.md)\n", escape(name)));
        }
    }
    text
}

pub async fn wmi(State(state): State<AppState>, Path(segment): Path<String>) -> Response {
    let (typed, format) = markdown::split(&segment);
    let code = typed.to_ascii_uppercase();
    if !is_code(&code) {
        return site::not_found(&state);
    }
    // One code, one address.
    if code != typed {
        let suffix = if format == Format::Markdown {
            ".md"
        } else {
            ""
        };
        return Redirect::permanent(&format!("/wmi/{code}{suffix}")).into_response();
    }
    let loaded = {
        let code = code.clone();
        state.db().run(move |worker| load_wmi(worker, &code)).await
    };
    let view = match loaded {
        Ok(Ok(Some(view))) => view,
        Ok(Ok(None)) => return site::not_found(&state),
        Ok(Err(error)) => return site::failed(&state, &ApiError::internal(error)),
        Err(error) => return site::failed(&state, &ApiError::from(error)),
    };
    let path = format!("/wmi/{code}");
    if format == Format::Markdown {
        let canonical = format!("{}{path}", state.config().base_url);
        return markdown::response(wmi_markdown(&view), &canonical);
    }
    let page = Page::new(
        &state,
        format!("{code}: {} - Wenmar Open", view.manufacturer),
        format!(
            "VINs that start with {code} were built by {}.",
            view.manufacturer
        ),
    )
    .indexed(&state, &path)
    .with_markdown(&format!("{path}.md"));
    let years = years(&view);
    let first = code.get(..3).unwrap_or(&code).to_owned();
    let rest = code
        .get(3..)
        .filter(|rest| !rest.is_empty())
        .map(str::to_owned);
    site::html(
        StatusCode::OK,
        &WmiPage {
            page,
            view,
            years,
            first,
            rest,
        },
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_code_is_three_or_six_capital_letters_and_digits() {
        assert!(is_code("KM8"));
        assert!(is_code("1A9881"));
        for bad in [
            "", "KM", "KM8K", "km8", "K M", "../", "KM8.md", "ÉÉÉ", "1A98811",
        ] {
            assert!(!is_code(bad), "{bad}");
        }
    }
}
