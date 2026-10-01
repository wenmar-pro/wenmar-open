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

/// Crawlers that build AI training sets or AI search indexes, by the
/// tokens their operators publish. They are welcome on everything but
/// single VINs. `Google-Extended` and `Applebot-Extended` are not crawlers:
/// they are the names under which Google and Apple take instructions about
/// AI use.
pub const AI_CRAWLERS: [&str; 13] = [
    "GPTBot",
    "OAI-SearchBot",
    "ClaudeBot",
    "Claude-SearchBot",
    "Google-Extended",
    "PerplexityBot",
    "Applebot-Extended",
    "Amazonbot",
    "meta-externalagent",
    "CCBot",
    "MistralAI-Training",
    "MistralAI-Index",
    "YouBot",
];

/// Fetchers that act for one person who asked an assistant a question.
/// They are welcome everywhere, a VIN decode included. Several of them say
/// they do not read robots.txt for such requests; naming them states the
/// policy either way.
pub const AI_FETCHERS: [&str; 8] = [
    "Claude-User",
    "ChatGPT-User",
    "Perplexity-User",
    "MistralAI-User",
    "DuckAssistBot",
    "Amzn-User",
    "meta-externalfetcher",
    "Google-Agent",
];

/// The text of `/robots.txt` for a site at `base`.
pub fn robots_text(base: &str) -> String {
    let agents = |tokens: &[&str]| -> String {
        tokens
            .iter()
            .map(|token| format!("User-agent: {token}\n"))
            .collect()
    };
    format!(
        "# Wenmar Open: free vehicle data. Anyone may read the reference pages,
# the guides, the API documentation and the API.
#
# A page or an API answer for one VIN describes one real vehicle. It is not
# for search indexes or training sets. Search engines may fetch a result
# page, and are told \"noindex\" on it.

User-agent: *
Allow: /

# Crawlers that build AI training sets or AI search indexes: welcome
# everywhere except the pages and API answers for single VINs.
{}Allow: /
Disallow: /vin/
Disallow: /v1/vin/

# Fetchers that act for one person who asked an assistant a question:
# welcome everywhere, a VIN decode included.
{}Allow: /

Sitemap: {base}/sitemap.xml
",
        agents(&AI_CRAWLERS),
        agents(&AI_FETCHERS)
    )
}

pub async fn robots(State(state): State<AppState>) -> Response {
    text(
        "text/plain; charset=utf-8",
        robots_text(&state.config().base_url),
    )
}

/// `/.well-known/api-catalog` (RFC 9727): where a program looks for a
/// site's APIs. It names the OpenAPI description and the documentation.
pub async fn api_catalog(State(state): State<AppState>) -> Response {
    let base = &state.config().base_url;
    let catalog = serde_json::json!({
        "linkset": [{
            "anchor": format!("{base}/v1"),
            "service-desc": [
                { "href": format!("{base}/v1/openapi.json"), "type": "application/json" }
            ],
            "service-doc": [
                { "href": format!("{base}/docs"), "type": "text/html" }
            ]
        }]
    });
    let mut response = (
        [
            (
                header::CONTENT_TYPE,
                HeaderValue::from_static(
                    "application/linkset+json; profile=\"https://www.rfc-editor.org/info/rfc9727\"",
                ),
            ),
            (
                header::CACHE_CONTROL,
                HeaderValue::from_static("public, max-age=3600"),
            ),
        ],
        catalog.to_string(),
    )
        .into_response();
    // The RFC asks for this header on a HEAD request; a GET carries it too.
    if let Ok(link) = HeaderValue::from_str(&format!(
        "<{base}/.well-known/api-catalog>; rel=\"api-catalog\""
    )) {
        response.headers_mut().insert(header::LINK, link);
    }
    response
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

/// A page's title: what the page is, then the site's name where there is
/// room. A search result shows about 60 characters, and the first words
/// are the ones people typed.
pub fn title(text: &str) -> String {
    const SITE: &str = " - Wenmar Open";
    if text.chars().count() + SITE.len() <= 60 {
        format!("{text}{SITE}")
    } else {
        text.to_owned()
    }
}

/// A description of at most `most` characters, cut at the end of a word
/// and not left hanging on a comma or an open bracket.
pub fn clip(text: &str, most: usize) -> String {
    if text.chars().count() <= most {
        return text.to_owned();
    }
    let mut characters = text.chars();
    let cut: String = characters.by_ref().take(most).collect();
    // If the cut fell between two words, every word kept is whole.
    // Otherwise the last one is a piece of a word, and it goes.
    let whole = characters.next().is_none_or(char::is_whitespace);
    let kept = if whole {
        cut.as_str()
    } else {
        cut.rsplit_once(' ').map_or(cut.as_str(), |(kept, _)| kept)
    };
    kept.trim_end_matches([',', ';', ':', '(', ' ']).to_owned()
}

/// A number of things, in words: `1 trim`, `3 trims`.
pub fn count(number: usize, noun: &str) -> String {
    if number == 1 {
        format!("1 {noun}")
    } else {
        format!("{number} {noun}s")
    }
}

/// The first three of a list of names, and whether there are more.
pub fn first_of(names: &[&str]) -> String {
    let shown: Vec<&str> = names.iter().copied().take(3).collect();
    if names.len() > shown.len() {
        format!("{} and more", shown.join(", "))
    } else {
        shown.join(", ")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn no_crawler_is_named_twice_and_none_is_one_of_the_fetchers() {
        let mut seen = std::collections::HashSet::new();
        for token in AI_CRAWLERS.iter().chain(AI_FETCHERS.iter()) {
            assert!(seen.insert(token.to_ascii_lowercase()), "{token}");
            assert!(
                token
                    .bytes()
                    .all(|byte| byte.is_ascii_alphanumeric() || byte == b'-'),
                "{token}"
            );
        }
        let robots = robots_text("https://open.example");
        assert_eq!(robots.matches("User-agent: ").count(), 1 + 13 + 8);
        assert_eq!(robots.matches("Disallow: ").count(), 2);
    }

    #[test]
    fn the_sites_name_is_added_where_there_is_room() {
        assert_eq!(
            title("Honda VIN decoder and models by year"),
            "Honda VIN decoder and models by year - Wenmar Open"
        );
        let long = "Crane Carrier Company (CCC) VIN decoder and models by year";
        assert_eq!(title(long), long);
        assert_eq!(title(&"x".repeat(46)).chars().count(), 60);
        assert_eq!(title(&"x".repeat(47)).chars().count(), 47);
    }

    #[test]
    fn a_description_is_cut_at_a_word() {
        assert_eq!(clip("short", 160), "short");
        assert_eq!(clip("one two three", 9), "one two");
        assert_eq!(clip("one two, three", 9), "one two");
        assert_eq!(clip("has 9 trims (LX, Si, Touring)", 13), "has 9 trims");
        assert_eq!(clip("abcdefghij", 4), "abcd");
        // Counted in characters, and never cut inside one.
        assert_eq!(clip("naïve café olé", 10), "naïve café");
        assert!(clip(&"word ".repeat(100), 160).chars().count() <= 160);
    }

    #[test]
    fn things_are_counted_and_listed_in_words() {
        assert_eq!(count(1, "trim"), "1 trim");
        assert_eq!(count(3, "engine"), "3 engines");
        assert_eq!(first_of(&["LX", "Si"]), "LX, Si");
        assert_eq!(first_of(&["LX", "Si", "Touring"]), "LX, Si, Touring");
        assert_eq!(first_of(&["A", "B", "C", "D"]), "A, B, C and more");
        assert_eq!(first_of(&[]), "");
    }
}
