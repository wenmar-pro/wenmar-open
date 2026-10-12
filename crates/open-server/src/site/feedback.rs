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

// ----- suggest a labour time -----

/// What a person who suggests a labour time may say they do: the value
/// the form sends, and the words shown.
pub const ROLES: [(&str, &str); 4] = [
    ("technician", "Technician"),
    ("service-advisor", "Service advisor"),
    ("shop-owner", "Shop owner or manager"),
    ("other", "Other"),
];

/// The labels a labour time suggestion is filed with.
pub const LABOR_LABELS: [&str; 2] = ["data", "labour-time"];

/// Most characters in a job id.
pub const LONGEST_JOB: usize = 60;

/// What the labour form holds. `website` is the same trap as in
/// [`Fields`].
#[derive(Debug, Clone, Default, PartialEq, Eq, Deserialize)]
#[serde(default)]
pub struct LaborFields {
    pub job: String,
    pub vehicle: String,
    pub hours: String,
    pub role: String,
    pub notes: String,
    pub page: String,
    pub website: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CheckedLabor {
    /// Tidy, and fit to send.
    Send(LaborFields),
    /// The trap was filled. Thank the sender and file nothing.
    Trap,
    /// What the visitor must change.
    Refused(&'static str),
}

/// Checks a suggestion's shape. Whether the job is one of the list is
/// asked of the shop file afterwards, where there is one.
pub fn check_labor(fields: &LaborFields) -> CheckedLabor {
    if !fields.website.trim().is_empty() {
        return CheckedLabor::Trap;
    }
    let tidy = LaborFields {
        job: one_line(&fields.job).to_ascii_lowercase(),
        vehicle: one_line(&fields.vehicle),
        hours: one_line(&fields.hours),
        role: one_line(&fields.role),
        notes: fields.notes.trim().to_owned(),
        page: one_line(&fields.page),
        website: String::new(),
    };
    if !wenmar_vehicles::text::is_slug(&tidy.job) || tidy.job.len() > LONGEST_JOB {
        return CheckedLabor::Refused(
            "Say which job: its id from the job list, such as brake-pads-front.",
        );
    }
    if tidy.vehicle.is_empty() {
        return CheckedLabor::Refused(
            "Say which vehicle: the year, the make and the model, and the engine if it matters.",
        );
    }
    let hours = shop_math::Hours::parse(&tidy.hours)
        .ok()
        .map(shop_math::Hours::hundredths);
    if !hours.is_some_and(|hundredths| (1..=99_999).contains(&hundredths)) {
        return CheckedLabor::Refused(
            "Give the hours as a number, such as 1.5, from 0.01 to 999.99.",
        );
    }
    if !ROLES.iter().any(|(id, _)| *id == tidy.role) {
        return CheckedLabor::Refused("Say what you do: choose one of the four.");
    }
    let long = |text: &str, most: usize| text.chars().count() > most;
    if long(&tidy.vehicle, LONGEST_LINE)
        || long(&tidy.page, LONGEST_LINE)
        || long(&tidy.notes, LONGEST_TEXT)
    {
        return CheckedLabor::Refused(
            "That is too long. The note holds 2,000 characters and the vehicle 300.",
        );
    }
    CheckedLabor::Send(tidy)
}

/// The title of a suggestion's issue.
fn labor_title(fields: &LaborFields) -> String {
    format!("Labour time suggestion: {}, {}", fields.job, fields.vehicle)
}

/// The issue for a checked suggestion. `base` is this site's public
/// address.
pub fn labor_issue(base: &str, fields: &LaborFields) -> Issue {
    let role = ROLES
        .iter()
        .find(|(id, _)| *id == fields.role)
        .map_or("Other", |(_, name)| *name);
    Issue {
        title: quiet(&labor_title(fields)),
        body: format!(
            "### Job\n\n{}\n\n### Vehicle\n\n{}\n\n### Hours suggested\n\n{}\n\n### Who is suggesting\n\n{role}\n\n### Notes\n\n{}\n\n### Page address\n\n{}\n\n_Sent through the form at {base}/feedback/labor. A suggestion changes no data: a person who does the work reads it, and an estimate changes only by a reviewed pull request to data/labor/._\n",
            quiet(&fields.job),
            quiet(&fields.vehicle),
            quiet(&fields.hours),
            or_none(&fields.notes),
            or_none(&fields.page)
        ),
    }
}

#[derive(Template)]
#[template(path = "feedback_labor.html")]
struct LaborPage {
    page: Page,
    fields: LaborFields,
    /// Value, words, and whether it is the one chosen.
    roles: Vec<(&'static str, &'static str, bool)>,
    /// Whether a token is set, so the form can be sent.
    form_on: bool,
    error: Option<&'static str>,
    /// GitHub's new-issue page, filled in.
    issue: String,
    mail: Option<String>,
}

fn labor_page(
    state: &AppState,
    status: StatusCode,
    fields: LaborFields,
    error: Option<&'static str>,
) -> Response {
    let config = state.config();
    let title = labor_title(&fields);
    let body = format!(
        "Job: {}\r\nVehicle: {}\r\nHours: {}\r\nWhat I do: \r\n\r\nWhat makes it take that long, and how many times I have done it:\r\n",
        fields.job, fields.vehicle, fields.hours
    );
    let issue = format!(
        "https://github.com/{}/issues/new?labels=labour-time&title={}&body={}",
        config.github_repo,
        report::encode(&title),
        report::encode(&body)
    );
    let mail = config.feedback_email.as_ref().map(|email| {
        format!(
            "mailto:{email}?subject={}&body={}",
            report::encode(&title),
            report::encode(&body)
        )
    });
    let page = Page::new(
        state,
        "Suggest a labour time - Wenmar Open",
        "Suggest how long a repair job takes on a vehicle, from work you have done.",
    )
    .without_vehicle_data();
    let mut response = site::html(
        status,
        &LaborPage {
            page,
            roles: ROLES
                .iter()
                .map(|(id, name)| (*id, *name, fields.role == *id))
                .collect(),
            fields,
            form_on: config.github_token.is_some(),
            error,
            issue,
            mail,
        },
    );
    site::unindexed(&mut response);
    response
        .headers_mut()
        .insert(header::CACHE_CONTROL, HeaderValue::from_static("no-store"));
    response
}

/// `GET /feedback/labor`: the form, filled in from the query string.
pub async fn show_labor(
    State(state): State<AppState>,
    query: Result<Query<LaborFields>, QueryRejection>,
) -> Response {
    let Query(mut fields) = query.unwrap_or_default();
    fields.website.clear();
    for (text, most) in [
        (&mut fields.job, LONGEST_JOB),
        (&mut fields.vehicle, LONGEST_LINE),
        (&mut fields.hours, 7),
        (&mut fields.role, 20),
        (&mut fields.notes, LONGEST_TEXT),
        (&mut fields.page, LONGEST_LINE),
    ] {
        *text = text.chars().take(most).collect();
    }
    labor_page(&state, StatusCode::OK, fields, None)
}

/// `POST /feedback/labor`: files the suggestion as an issue. It writes
/// nothing else anywhere.
pub async fn send_labor(State(state): State<AppState>, request: Request) -> Response {
    let peer = request
        .extensions()
        .get::<ConnectInfo<SocketAddr>>()
        .map(|info| info.0.ip());
    let address = limit::client_ip(request.headers(), peer, state.config().trusted_proxies);
    let Ok(Form(fields)) = Form::<LaborFields>::from_request(request, &state).await else {
        return labor_page(
            &state,
            StatusCode::BAD_REQUEST,
            LaborFields::default(),
            Some("The form could not be read. Fill it in again."),
        );
    };
    let Some(token) = state.config().github_token.clone() else {
        return labor_page(&state, StatusCode::OK, fields, None);
    };
    let tidy = match check_labor(&fields) {
        CheckedLabor::Trap => return sent(&state, None),
        CheckedLabor::Refused(why) => {
            return labor_page(&state, StatusCode::BAD_REQUEST, fields, Some(why));
        }
        CheckedLabor::Send(tidy) => tidy,
    };
    // Where the job list is held, the job must be one of it.
    if let Ok(shop) = state.dataset(wenmar_shopdata::Dataset::Jobs) {
        let job = tidy.job.clone();
        let known = shop
            .run(move |reader| reader.job(&job).map(|found| found.is_some()))
            .await;
        if !matches!(known, Ok(Ok(true))) {
            return labor_page(
                &state,
                StatusCode::BAD_REQUEST,
                fields,
                Some("That is not a job of the list. Use an id from the job list."),
            );
        }
    }
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
    let issue = labor_issue(&state.config().base_url, &tidy);
    let (api, repo) = (
        state.config().github_api.clone(),
        state.config().github_repo.clone(),
    );
    let filed = tokio::task::spawn_blocking(move || {
        github::file_with(&api, &repo, token.expose(), &issue, &LABOR_LABELS)
    })
    .await;
    match filed {
        Ok(Ok(page)) => sent(&state, Some(page)),
        Ok(Err(error)) => {
            tracing::warn!(?error, "a suggestion could not be filed");
            labor_page(&state, StatusCode::BAD_GATEWAY, tidy, Some(NOT_SENT))
        }
        Err(_) => labor_page(&state, StatusCode::BAD_GATEWAY, tidy, Some(NOT_SENT)),
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
