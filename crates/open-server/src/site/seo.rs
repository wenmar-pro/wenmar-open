//! What search engines are told: `robots.txt` and the sitemaps.
//!
//! Reference pages are listed. Result pages for single VINs are not: they
//! carry `noindex`, and they are not blocked in `robots.txt`, because a
//! crawler must be able to fetch a page to see that it is not to be indexed.

use axum::extract::rejection::PathRejection;
use axum::extract::{Path, State};
use axum::http::{HeaderValue, header};
use axum::response::{IntoResponse, Response};
use wenmar_vehicles::text::is_slug;
use wenmar_vehicles::{Scope, Source, SourceError, Value};

use crate::error::ApiError;
use crate::site;
use crate::site::wmi::is_code;
use crate::state::AppState;

const MODELS_SQL: &str = "
SELECT mk.slug, md.slug
FROM catalog_vehicle v
JOIN catalog_make mk ON mk.id = v.make_id
JOIN catalog_model md ON md.id = v.model_id
WHERE v.year = ?1 AND v.light = 1";

const CODES_SQL: &str = "SELECT code FROM wmi";

fn text(content_type: &'static str, body: String) -> Response {
    (
        [(header::CONTENT_TYPE, HeaderValue::from_static(content_type))],
        body,
    )
        .into_response()
}

pub async fn robots(State(state): State<AppState>) -> Response {
    text(
        "text/plain; charset=utf-8",
        format!(
            "User-agent: *\nAllow: /\n\nSitemap: {}/sitemap.xml\n",
            state.config().base_url
        ),
    )
}

/// The date the data was built, for `lastmod`, when it is a date.
fn built_on(state: &AppState) -> Option<String> {
    let date: String = state.db().meta().built_at.chars().take(10).collect();
    let digits = date.bytes().enumerate().all(|(index, byte)| match index {
        4 | 7 => byte == b'-',
        _ => byte.is_ascii_digit(),
    });
    (date.len() == 10 && digits).then_some(date)
}

fn url_set(state: &AppState, paths: &[String]) -> Response {
    let base = &state.config().base_url;
    let lastmod = built_on(state)
        .map(|date| format!("<lastmod>{date}</lastmod>"))
        .unwrap_or_default();
    let mut xml = String::from(
        "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<urlset xmlns=\"http://www.sitemaps.org/schemas/sitemap/0.9\">\n",
    );
    for path in paths {
        xml.push_str(&format!("<url><loc>{base}{path}</loc>{lastmod}</url>\n"));
    }
    xml.push_str("</urlset>\n");
    text("application/xml; charset=utf-8", xml)
}

/// The list of sitemaps: one for the fixed pages, one for makes, one for
/// manufacturer codes, and one per model year, so none comes near the
/// limit of 50,000 addresses.
pub async fn index(State(state): State<AppState>) -> Response {
    let years = state.years(Scope::Light).unwrap_or_default();
    let base = &state.config().base_url;
    let mut names = vec!["pages".to_owned(), "makes".to_owned(), "wmi".to_owned()];
    names.extend(years.iter().map(|year| format!("models-{year}")));
    let mut xml = String::from(
        "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<sitemapindex xmlns=\"http://www.sitemaps.org/schemas/sitemap/0.9\">\n",
    );
    for name in names {
        xml.push_str(&format!(
            "<sitemap><loc>{base}/sitemaps/{name}.xml</loc></sitemap>\n"
        ));
    }
    xml.push_str("</sitemapindex>\n");
    text("application/xml; charset=utf-8", xml)
}

fn pairs(rows: Vec<Vec<Value>>) -> Vec<(String, String)> {
    rows.iter()
        .filter_map(|row| {
            let make = row.first().and_then(Value::text)?;
            let model = row.get(1).and_then(Value::text)?;
            // An id form is the only thing put into an address.
            (is_slug(make) && is_slug(model)).then(|| (make.to_owned(), model.to_owned()))
        })
        .collect()
}

pub async fn sitemap(
    State(state): State<AppState>,
    path: Result<Path<String>, PathRejection>,
) -> Response {
    let Ok(Path(file)) = path else {
        return site::not_found(&state);
    };
    let Some(name) = file.strip_suffix(".xml") else {
        return site::not_found(&state);
    };
    let paths: Result<Option<Vec<String>>, ApiError> = match name {
        "pages" => {
            let mut paths: Vec<String> = ["/", "/makes", "/guides"].map(str::to_owned).to_vec();
            paths.extend(site::guides::GUIDES.iter().map(|guide| guide.path()));
            paths.extend(["/docs", "/data", "/about"].map(str::to_owned));
            Ok(Some(paths))
        }
        "makes" => state
            .db()
            .run(|worker| worker.catalog.makes(None, Scope::Light, "", 500))
            .await
            .map_err(ApiError::from)
            .and_then(|makes| makes.map_err(ApiError::from))
            .map(|makes| {
                Some(
                    makes
                        .into_iter()
                        .filter(|make| is_slug(&make.id))
                        .map(|make| format!("/makes/{}", make.id))
                        .collect(),
                )
            }),
        "wmi" => state
            .db()
            .run_slow(|worker| worker.source.query(CODES_SQL, &[]))
            .await
            .map_err(ApiError::from)
            .and_then(|rows: Result<_, SourceError>| rows.map_err(ApiError::internal))
            .map(|rows| {
                let mut codes: Vec<String> = rows
                    .iter()
                    .filter_map(|row| row.first().and_then(Value::text))
                    .filter(|code| is_code(code))
                    .map(|code| format!("/wmi/{code}"))
                    .collect();
                codes.sort();
                Some(codes)
            }),
        other => match other.strip_prefix("models-").and_then(site::year_in) {
            None => Ok(None),
            Some(year) => state
                .db()
                .run_slow(move |worker| worker.source.query(MODELS_SQL, &[i64::from(year).into()]))
                .await
                .map_err(ApiError::from)
                .and_then(|rows: Result<_, SourceError>| rows.map_err(ApiError::internal))
                .map(|rows| {
                    let mut found = pairs(rows);
                    found.sort();
                    found.dedup();
                    // A year with no models has no sitemap.
                    (!found.is_empty()).then(|| {
                        found
                            .into_iter()
                            .map(|(make, model)| format!("/makes/{make}/{model}/{year}"))
                            .collect()
                    })
                }),
        },
    };
    match paths {
        Ok(Some(paths)) => url_set(&state, &paths),
        Ok(None) => site::not_found(&state),
        Err(error) => site::failed(&state, &error),
    }
}
