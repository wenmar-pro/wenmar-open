//! What the pages say of NHTSA's defect investigations: "Under
//! investigation by NHTSA" on a model-year page and a VIN result page, and
//! "Investigated as ..." on a recall campaign's page.
//!
//! Both are built from the answers the API gives, so the two cannot
//! disagree. An investigation is not a finding of a defect and is never
//! about one vehicle; every section here says so. A server that holds no
//! investigations, and a vehicle or a campaign with none, gets no section
//! at all.

use wenmar_shopdata::Dataset;
use wenmar_shopdata::investigations::NOT_A_FINDING;

use crate::api::investigations::{investigations_of, recall_investigations_op};
use crate::api::recalls::model_year;
use crate::api::types::Investigation;
use crate::site::markdown::escape;
use crate::site::seo;
use crate::state::AppState;

/// One investigation in a list on a page.
pub struct Line {
    /// NHTSA's action number, such as `EA23001`.
    pub action: String,
    /// Its kind, dates, subject, component and the recalls it led to, as
    /// sentences.
    pub details: String,
}

/// Names in words: `A`, `A and B`, `A, B and C`.
pub fn in_words(names: &[String]) -> String {
    let mut text = String::new();
    for (index, name) in names.iter().enumerate() {
        if index > 0 {
            text.push_str(if index + 1 == names.len() {
                " and "
            } else {
                ", "
            });
        }
        text.push_str(name);
    }
    text
}

fn line(found: &Investigation) -> Line {
    let mut details = found
        .kind_name
        .clone()
        .unwrap_or_else(|| found.kind.clone());
    match (&found.opened, &found.closed) {
        (Some(opened), Some(closed)) => {
            details.push_str(&format!(", opened {opened}, closed {closed}"));
        }
        (Some(opened), None) => details.push_str(&format!(", opened {opened}")),
        (None, Some(closed)) => details.push_str(&format!(", closed {closed}")),
        (None, None) => {}
    }
    details.push('.');
    for part in [&found.subject, &found.component].into_iter().flatten() {
        details.push_str(&format!(" {part}."));
    }
    if !found.campaigns.is_empty() {
        details.push_str(&format!(
            " Led to {} {}.",
            if found.campaigns.len() == 1 {
                "recall"
            } else {
                "recalls"
            },
            in_words(&found.campaigns)
        ));
    }
    Line {
        action: found.action.clone(),
        details,
    }
}

/// "As of" for the investigation file.
fn dated(day: &str) -> String {
    format!("as of {day}")
}

fn markdown_lines(lines: &[Line]) -> String {
    let mut text = String::new();
    for line in lines {
        text.push_str(&format!(
            "- {}: {}\n",
            escape(&line.action),
            escape(&line.details)
        ));
    }
    text
}

// ----- a model year -----

/// What a page says of the open investigations that list a model year.
pub struct Notice {
    /// `2019 Ram 1500`.
    pub name: String,
    /// `1 open NHTSA investigation lists this model year.`
    pub text: String,
    /// That an investigation is not a finding of a defect.
    pub note: &'static str,
    /// Newest first.
    pub lines: Vec<Line>,
    /// When the file was current, in words.
    pub dated: String,
    /// The JSON these lines came from.
    pub json_path: String,
}

/// The open investigations that list the model year `vehicle_id` names.
///
/// `None` on a server that holds no investigations, for an id that names
/// no model year, when none is open, and when they could not be read. The
/// page that asked is then shown exactly as it is without this: it is an
/// addition to the page, never a reason for it to fail.
pub async fn open_for(state: &AppState, vehicle_id: &str) -> Option<Notice> {
    let shop = state.dataset(Dataset::Investigations).ok()?;
    let id = vehicle_id.to_owned();
    let found = match state.db().run(move |worker| model_year(worker, &id)).await {
        Ok(Ok(found)) => found?,
        Ok(Err(error)) => {
            tracing::warn!(%error, "investigations left off a page");
            return None;
        }
        Err(error) => {
            tracing::warn!(%error, "investigations left off a page");
            return None;
        }
    };
    // A failure here is logged where it is turned into an `ApiError`.
    let answer = investigations_of(shop, &found, true).await.ok()?;
    if answer.investigations.is_empty() {
        return None;
    }
    let total = answer.investigations.len();
    let text = if total == 1 {
        "1 open NHTSA investigation lists this model year.".to_owned()
    } else {
        format!("{total} open NHTSA investigations list this model year.")
    };
    Some(Notice {
        name: format!(
            "{} {} {}",
            answer.vehicle.year, answer.vehicle.make, answer.vehicle.model
        ),
        text,
        note: NOT_A_FINDING,
        lines: answer.investigations.iter().map(line).collect(),
        dated: dated(&answer.as_of),
        json_path: format!("/v1/vehicles/{}/investigations", answer.vehicle.id),
    })
}

/// The section of a model year's Markdown page.
pub fn notice_markdown(notice: &Notice) -> String {
    let mut text = String::from("\n## Under investigation by NHTSA\n\n");
    text.push_str(&format!("{} {}\n\n", notice.text, notice.note));
    text.push_str(&markdown_lines(&notice.lines));
    text.push_str(&format!(
        "\nFrom NHTSA's investigation file, {}. JSON: [{path}]({path})\n",
        notice.dated,
        path = notice.json_path
    ));
    text
}

// ----- a recall campaign -----

/// What a campaign's page says of the investigations that led to it.
pub struct Behind {
    /// `Investigated as PE18008`.
    pub heading: String,
    /// `This recall followed 1 NHTSA investigation.`
    pub text: String,
    pub note: &'static str,
    /// Oldest first.
    pub lines: Vec<Line>,
    pub dated: String,
    pub json_path: String,
}

/// The investigations that led to a campaign. `None` on a server that
/// holds no investigations, for a campaign that followed none, and when
/// they could not be read.
pub async fn behind(state: &AppState, source: &str, campaign: &str) -> Option<Behind> {
    let answer = recall_investigations_op(state, source, campaign)
        .await
        .ok()?;
    if answer.investigations.is_empty() {
        return None;
    }
    let actions: Vec<String> = answer
        .investigations
        .iter()
        .map(|found| found.action.clone())
        .collect();
    Some(Behind {
        heading: format!("Investigated as {}", in_words(&actions)),
        text: format!(
            "This recall followed {}.",
            seo::count(actions.len(), "NHTSA investigation")
        ),
        note: NOT_A_FINDING,
        // The campaign is this page: it is not said again on each line.
        lines: answer
            .investigations
            .iter()
            .map(|found| {
                line(&Investigation {
                    campaigns: Vec::new(),
                    ..found.clone()
                })
            })
            .collect(),
        dated: dated(&answer.as_of),
        json_path: format!(
            "/v1/recalls/{}/{}/investigations",
            answer.source, answer.campaign
        ),
    })
}

/// The section of a campaign's Markdown page.
pub fn behind_markdown(behind: &Behind) -> String {
    let mut text = format!("\n## {}\n\n", escape(&behind.heading));
    text.push_str(&format!("{} {}\n\n", behind.text, behind.note));
    text.push_str(&markdown_lines(&behind.lines));
    text.push_str(&format!(
        "\nFrom NHTSA's investigation file, {}. JSON: [{path}]({path})\n",
        behind.dated,
        path = behind.json_path
    ));
    text
}

#[cfg(test)]
mod tests {
    use super::*;

    fn investigation() -> Investigation {
        Investigation {
            action: "EA23001".to_owned(),
            kind: "EA".to_owned(),
            kind_name: Some("Engineering analysis".to_owned()),
            opened: Some("2023-03-17".to_owned()),
            closed: None,
            open: true,
            manufacturer: Some("Chrysler (FCA US, LLC)".to_owned()),
            component: Some("FUEL SYSTEM, DIESEL".to_owned()),
            subject: Some("High pressure fuel pump failure".to_owned()),
            summary: None,
            campaigns: vec!["24V957000".to_owned()],
            matched: Some("exact".to_owned()),
            models: vec!["1500".to_owned()],
        }
    }

    #[test]
    fn names_are_listed_in_words() {
        let names =
            |list: &[&str]| -> Vec<String> { list.iter().map(|name| (*name).to_owned()).collect() };
        assert_eq!(in_words(&names(&[])), "");
        assert_eq!(in_words(&names(&["A"])), "A");
        assert_eq!(in_words(&names(&["A", "B"])), "A and B");
        assert_eq!(in_words(&names(&["A", "B", "C"])), "A, B and C");
    }

    #[test]
    fn a_line_says_the_kind_the_dates_the_subject_and_the_recalls() {
        assert_eq!(
            line(&investigation()).details,
            "Engineering analysis, opened 2023-03-17. High pressure fuel pump failure. FUEL SYSTEM, DIESEL. Led to recall 24V957000."
        );
        let closed = Investigation {
            closed: Some("2024-01-05".to_owned()),
            campaigns: vec!["24V957000".to_owned(), "25V001000".to_owned()],
            component: None,
            ..investigation()
        };
        assert_eq!(
            line(&closed).details,
            "Engineering analysis, opened 2023-03-17, closed 2024-01-05. High pressure fuel pump failure. Led to recalls 24V957000 and 25V001000."
        );
        // A kind with no name is shown by its letters; nothing else known.
        let bare = Investigation {
            kind: "ID".to_owned(),
            kind_name: None,
            opened: None,
            subject: None,
            component: None,
            campaigns: Vec::new(),
            ..investigation()
        };
        assert_eq!(line(&bare).details, "ID.");
    }
}
