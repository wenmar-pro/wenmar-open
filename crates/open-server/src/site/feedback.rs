//! `/feedback`: report an error without a GitHub account.
//!
//! The server files the issue itself. A person reads it; nothing a visitor
//! sends changes data.

use std::net::{IpAddr, SocketAddr};
use std::time::Instant;

use askama::Template;
use axum::Form;
use axum::extract::rejection::QueryRejection;
use axum::extract::{ConnectInfo, FromRequest, Query, Request, State};
use axum::http::{HeaderValue, StatusCode, header};
use axum::response::{IntoResponse, Response};
use serde::Deserialize;

use crate::error::ApiError;
use crate::github::{self, Issue};
use crate::limit;
use crate::site::report::{self, ReportLinks};
use crate::site::{self, Page};
use crate::state::AppState;

/// Reports one address may send in a minute, and all addresses together.
pub const PER_ADDRESS: u32 = 3;
pub const IN_ALL: u32 = 20;
/// Most characters in the page address and in the vehicle, and in each box.
pub const LONGEST_LINE: usize = 300;
pub const LONGEST_TEXT: usize = 2000;

/// What the form holds. `website` is a trap: a person never sees it, and a
/// program that fills every field fills it.
#[derive(Debug, Clone, Default, PartialEq, Eq, Deserialize)]
#[serde(default)]
pub struct Fields {
    pub page: String,
    pub vehicle: String,
    pub wrong: String,
    pub correct: String,
    pub website: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Checked {
    /// Tidy, and fit to send.
    Send(Fields),
    /// The trap was filled. Thank the sender and file nothing.
    Trap,
    /// What the visitor must change.
    Refused(&'static str),
}

/// Text on one line: control characters become spaces, runs of spaces one.
fn one_line(text: &str) -> String {
    let spaced: String = text
        .chars()
        .map(|c| if c.is_control() { ' ' } else { c })
        .collect();
    spaced.split_whitespace().collect::<Vec<_>>().join(" ")
}

pub fn check(fields: &Fields) -> Checked {
    if !fields.website.trim().is_empty() {
        return Checked::Trap;
    }
    let tidy = Fields {
        page: one_line(&fields.page),
        vehicle: one_line(&fields.vehicle),
        wrong: fields.wrong.trim().to_owned(),
        correct: fields.correct.trim().to_owned(),
        website: String::new(),
    };
    if tidy.wrong.is_empty() {
        return Checked::Refused("Say what is wrong before sending.");
    }
    let long = |text: &str, most: usize| text.chars().count() > most;
    if long(&tidy.page, LONGEST_LINE)
        || long(&tidy.vehicle, LONGEST_LINE)
        || long(&tidy.wrong, LONGEST_TEXT)
        || long(&tidy.correct, LONGEST_TEXT)
    {
        return Checked::Refused(
            "That is too long. Each box holds 2,000 characters, and the page address and the vehicle 300 each.",
        );
    }
    Checked::Send(tidy)
}

/// A zero-width space after every `@`, so a report cannot notify a GitHub
/// user by naming one.
fn quiet(text: &str) -> String {
    text.replace('@', "@\u{200b}")
}

fn or_none(text: &str) -> String {
    if text.is_empty() {
        "_No response_".to_owned()
    } else {
        quiet(text)
    }
}

/// The issue for a checked report, laid out as GitHub lays out an issue
/// form's answers. `base` is this site's public address.
pub fn issue(base: &str, fields: &Fields) -> Issue {
    let title = if fields.vehicle.is_empty() {
        "Data correction".to_owned()
    } else {
        format!("Data correction: {}", fields.vehicle)
    };
    Issue {
        title: quiet(&title),
        body: format!(
            "### Page address\n\n{}\n\n### Vehicle\n\n{}\n\n### What is wrong\n\n{}\n\n### What it should be, and how you know\n\n{}\n\n_Sent through the form at {base}/feedback._\n",
            or_none(&fields.page),
            or_none(&fields.vehicle),
            quiet(&fields.wrong),
            or_none(&fields.correct)
        ),
    }
}

const NOT_SENT: &str = "The report could not be sent. What you typed is still below: try again in a minute, or use one of the links under the form.";

#[derive(Template)]
#[template(path = "feedback.html")]
struct FeedbackPage {
    page: Page,
    fields: Fields,
    /// Whether a token is set, so the form can be sent.
    form_on: bool,
    error: Option<&'static str>,
    report: ReportLinks,
}

#[derive(Template)]
#[template(path = "feedback_sent.html")]
struct SentPage {
    page: Page,
    issue: Option<String>,
}

fn form_page(
    state: &AppState,
    status: StatusCode,
    fields: Fields,
    error: Option<&'static str>,
) -> Response {
    let report = report::links_to(state.config(), &fields.page, &fields.vehicle);
    let page = Page::new(
        state,
        "Report an error - Wenmar Open",
        "Report something a page of Wenmar Open shows wrong.",
    )
    .without_vehicle_data();
    let form_on = state.config().github_token.is_some();
    let mut response = site::html(
        status,
        &FeedbackPage {
            page,
            fields,
            form_on,
            error,
            report,
        },
    );
    site::unindexed(&mut response);
    response
        .headers_mut()
        .insert(header::CACHE_CONTROL, HeaderValue::from_static("no-store"));
    response
}

fn sent(state: &AppState, issue: Option<String>) -> Response {
    let page = Page::new(state, "Report sent - Wenmar Open", "The report was sent.")
        .without_vehicle_data();
    let mut response = site::html(StatusCode::OK, &SentPage { page, issue });
    site::unindexed(&mut response);
    response
}

/// `GET /feedback`: the form, filled in from the query string.
pub async fn show(
    State(state): State<AppState>,
    query: Result<Query<Fields>, QueryRejection>,
) -> Response {
    let Query(mut fields) = query.unwrap_or_default();
    fields.website.clear();
    for (text, most) in [
        (&mut fields.page, LONGEST_LINE),
        (&mut fields.vehicle, LONGEST_LINE),
        (&mut fields.wrong, LONGEST_TEXT),
        (&mut fields.correct, LONGEST_TEXT),
    ] {
        *text = text.chars().take(most).collect();
    }
    form_page(&state, StatusCode::OK, fields, None)
}

/// `POST /feedback`: files the issue.
pub async fn send(State(state): State<AppState>, request: Request) -> Response {
    // The address is read as `limit::limit` reads it, before the body is taken.
    let peer = request
        .extensions()
        .get::<ConnectInfo<SocketAddr>>()
        .map(|info| info.0.ip());
    let address = limit::client_ip(request.headers(), peer, state.config().trusted_proxies);
    let Ok(Form(fields)) = Form::<Fields>::from_request(request, &state).await else {
        return form_page(
            &state,
            StatusCode::BAD_REQUEST,
            Fields::default(),
            Some("The form could not be read. Fill it in again."),
        );
    };
    let Some(token) = state.config().github_token.clone() else {
        return form_page(&state, StatusCode::OK, fields, None);
    };
    let fields = match check(&fields) {
        Checked::Trap => return sent(&state, None),
        Checked::Refused(why) => {
            return form_page(&state, StatusCode::BAD_REQUEST, fields, Some(why));
        }
        Checked::Send(fields) => fields,
    };
    let now = Instant::now();
    let everyone = IpAddr::from([0, 0, 0, 0]);
    let allowed = state
        .feedback_each()
        .check(address, now)
        .and_then(|()| state.feedback_all().check(everyone, now));
    if let Err(retry_after) = allowed {
        tracing::warn!(%address, "over the limit on reports");
        return ApiError::RateLimited { retry_after }.into_response();
    }
    let issue = issue(&state.config().base_url, &fields);
    let (api, repo) = (
        state.config().github_api.clone(),
        state.config().github_repo.clone(),
    );
    let filed =
        tokio::task::spawn_blocking(move || github::file(&api, &repo, token.expose(), &issue))
            .await;
    match filed {
        Ok(Ok(page)) => sent(&state, Some(page)),
        Ok(Err(error)) => {
            tracing::warn!(?error, "a report could not be filed");
            form_page(&state, StatusCode::BAD_GATEWAY, fields, Some(NOT_SENT))
        }
        Err(_) => form_page(&state, StatusCode::BAD_GATEWAY, fields, Some(NOT_SENT)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fields(wrong: &str) -> Fields {
        Fields {
            page: " https://open.example/makes/honda/civic/2019\n".to_owned(),
            vehicle: "2019  Honda\tCivic".to_owned(),
            wrong: wrong.to_owned(),
            ..Fields::default()
        }
    }

    #[test]
    fn a_report_is_tidied_or_refused() {
        let Checked::Send(tidy) = check(&fields("  The drive is AWD.\nNot FWD. ")) else {
            panic!("not sent")
        };
        assert_eq!(tidy.page, "https://open.example/makes/honda/civic/2019");
        assert_eq!(tidy.vehicle, "2019 Honda Civic");
        assert_eq!(tidy.wrong, "The drive is AWD.\nNot FWD.");
        assert_eq!(
            check(&fields(" \n ")),
            Checked::Refused("Say what is wrong before sending.")
        );
        assert!(matches!(
            check(&fields(&"é".repeat(LONGEST_TEXT))),
            Checked::Send(_)
        ));
        assert!(matches!(
            check(&fields(&"é".repeat(LONGEST_TEXT + 1))),
            Checked::Refused(_)
        ));
        let trapped = Fields {
            website: "https://spam.example".to_owned(),
            ..fields("x")
        };
        assert_eq!(check(&trapped), Checked::Trap);
    }

    #[test]
    fn the_issue_reads_like_the_form_on_github() {
        let Checked::Send(tidy) = check(&fields("Ask @someone")) else {
            panic!("not sent")
        };
        let filed = issue("https://open.example", &tidy);
        assert_eq!(filed.title, "Data correction: 2019 Honda Civic");
        assert_eq!(
            filed.body,
            "### Page address\n\nhttps://open.example/makes/honda/civic/2019\n\n### Vehicle\n\n2019 Honda Civic\n\n### What is wrong\n\nAsk @\u{200b}someone\n\n### What it should be, and how you know\n\n_No response_\n\n_Sent through the form at https://open.example/feedback._\n"
        );
        let bare = Fields {
            wrong: "x".to_owned(),
            ..Fields::default()
        };
        assert_eq!(
            issue("https://open.example", &bare).title,
            "Data correction"
        );
    }
}
