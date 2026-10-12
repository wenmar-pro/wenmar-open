//! What the model-year page says of NHTSA's crash test ratings.
//!
//! The section is built from the answer `GET /v1/vehicles/{id}/safety`
//! gives, so the page and the API cannot disagree. A server with no
//! ratings, and a model year NHTSA lists no vehicle of, have no section.

use wenmar_shopdata::Dataset;

use crate::api::recalls::model_year;
use crate::api::safety::safety_of;
use crate::api::types::SafetyRating;
use crate::site::markdown::escape;
use crate::state::AppState;

/// One rating, as a row of the section's table.
pub struct Line {
    /// The vehicle NHTSA rated, in NHTSA's words.
    pub description: String,
    /// The stars, the rollover risk and the weight, in words.
    pub text: String,
    /// NHTSA's safety concerns and footnotes.
    pub notes: Vec<String>,
}

/// The crash test ratings of one model year.
pub struct Section {
    pub lines: Vec<Line>,
    /// The day NHTSA's file was current.
    pub as_of: String,
    /// NHTSA's own pages of ratings.
    pub source_url: String,
    /// The same ratings as JSON, with the safety features.
    pub json_path: String,
}

/// The stars, the rollover risk and the tested weight of a rating, as
/// sentences. A vehicle with no stars is said to be not rated.
pub fn summary(rating: &SafetyRating) -> String {
    let stars = &rating.stars;
    let mut parts: Vec<String> = Vec::new();
    if let Some(overall) = stars.overall {
        parts.push(format!("Overall {overall} of 5."));
    }
    match (stars.frontal, stars.frontal_driver, stars.frontal_passenger) {
        (Some(frontal), _, _) => parts.push(format!("Frontal {frontal}.")),
        (None, None, None) => {}
        (None, driver, passenger) => parts.push(format!(
            "Frontal: driver {}, passenger {}.",
            given(driver),
            given(passenger)
        )),
    }
    match (stars.side, stars.side_driver, stars.side_passenger) {
        (Some(side), _, _) => parts.push(format!("Side {side}.")),
        (None, None, None) => {}
        (None, driver, passenger) => parts.push(format!(
            "Side: driver {}, rear passenger {}.",
            given(driver),
            given(passenger)
        )),
    }
    if let Some(rollover) = stars.rollover {
        parts.push(format!("Rollover {rollover}."));
    }
    if parts.is_empty() {
        parts.push("Not rated by NHTSA.".to_owned());
    }
    if let Some(risk) = rating.rollover_risk_percent {
        parts.push(format!("Rollover risk {risk:.1}%."));
    }
    if let (Some(kilograms), Some(pounds)) = (rating.curb_weight_kg, rating.curb_weight_lb) {
        parts.push(format!("Tested at {kilograms} kg ({pounds} lb)."));
    }
    parts.join(" ")
}

/// A star count, or that there is none.
fn given(stars: Option<u8>) -> String {
    stars.map_or_else(|| "not rated".to_owned(), |stars| stars.to_string())
}

/// The crash test ratings of the model year `vehicle_id` names.
///
/// `None` on a server that holds no ratings, for an id that names no model
/// year, when NHTSA lists no vehicle of the model year, and when the
/// ratings could not be read. The page that asked is then shown exactly as
/// it is without them: they are an addition to it, never a reason for it
/// to fail.
pub async fn section(state: &AppState, vehicle_id: &str) -> Option<Section> {
    let shop = state.dataset(Dataset::Ratings).ok()?;
    let id = vehicle_id.to_owned();
    let found = match state.db().run(move |worker| model_year(worker, &id)).await {
        Ok(Ok(found)) => found?,
        Ok(Err(error)) => {
            tracing::warn!(%error, "crash test ratings left off a page");
            return None;
        }
        Err(error) => {
            tracing::warn!(%error, "crash test ratings left off a page");
            return None;
        }
    };
    // A failure here is logged where it is turned into an `ApiError`.
    let answer = safety_of(shop, &found).await.ok()?;
    if answer.ratings.is_empty() {
        return None;
    }
    Some(Section {
        lines: answer
            .ratings
            .iter()
            .map(|rating| Line {
                description: rating.description.clone(),
                text: summary(rating),
                notes: rating.notes.clone(),
            })
            .collect(),
        as_of: answer.as_of,
        source_url: answer.source_url,
        json_path: format!("/v1/vehicles/{}/safety", answer.vehicle.id),
    })
}

/// The section as Markdown, for the model-year page's `.md`.
pub fn markdown(section: &Section) -> String {
    let mut text = String::from("\n## Crash test ratings\n\n");
    for line in &section.lines {
        text.push_str(&format!(
            "- {}: {}",
            escape(&line.description),
            escape(&line.text)
        ));
        for note in &line.notes {
            text.push_str(&format!(" {}", escape(note)));
        }
        text.push('\n');
    }
    text.push_str(&format!(
        "\nStars are NHTSA's, from 1 to 5, each for the vehicle NHTSA names. From NHTSA's file as of {}. [NHTSA's ratings]({}). JSON, with safety features: [{path}]({path})\n",
        section.as_of,
        section.source_url,
        path = section.json_path
    ));
    text
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::api::types::{SafetyFeatures, SafetyStars};

    fn rating(change: impl FnOnce(&mut SafetyRating)) -> SafetyRating {
        let mut rating = SafetyRating {
            description: "CIVIC, 4 DR, FWD".to_owned(),
            model: "CIVIC".to_owned(),
            body_style: Some("4 DR".to_owned()),
            vehicle_type: Some("PC".to_owned()),
            drive: Some("FWD".to_owned()),
            release: 1,
            matched: "exact".to_owned(),
            stars: SafetyStars {
                overall: None,
                frontal: None,
                frontal_driver: None,
                frontal_passenger: None,
                side: None,
                side_driver: None,
                side_passenger: None,
                side_barrier: None,
                side_front_seat: None,
                side_rear_seat: None,
                side_pole: None,
                rollover: None,
            },
            rollover_risk_percent: None,
            rollover_tip: None,
            curb_weight_kg: None,
            curb_weight_lb: None,
            features: SafetyFeatures {
                esc: None,
                forward_collision_warning: None,
                forward_collision_warning_evaluation: None,
                lane_departure_warning: None,
                lane_departure_warning_evaluation: None,
                crash_imminent_braking: None,
                crash_imminent_braking_evaluation: None,
                dynamic_brake_support: None,
                dynamic_brake_support_evaluation: None,
                backup_camera: None,
                blind_spot_detection: None,
                head_side_airbag: None,
                torso_side_airbag: None,
            },
            notes: Vec::new(),
        };
        change(&mut rating);
        rating
    }

    #[test]
    fn a_rating_is_said_in_sentences() {
        let full = rating(|rating| {
            rating.stars.overall = Some(5);
            rating.stars.frontal = Some(4);
            rating.stars.frontal_driver = Some(5);
            rating.stars.side = Some(5);
            rating.stars.rollover = Some(4);
            rating.rollover_risk_percent = Some(19.1);
            rating.curb_weight_kg = Some(2295);
            rating.curb_weight_lb = Some(5060);
        });
        assert_eq!(
            summary(&full),
            "Overall 5 of 5. Frontal 4. Side 5. Rollover 4. Rollover risk 19.1%. Tested at 2295 kg (5060 lb)."
        );
        // Before 2011 NHTSA rated each seat and gave no overall figure.
        let old = rating(|rating| {
            rating.stars.frontal_driver = Some(4);
            rating.stars.frontal_passenger = Some(4);
            rating.stars.side_driver = Some(4);
        });
        assert_eq!(
            summary(&old),
            "Frontal: driver 4, passenger 4. Side: driver 4, rear passenger not rated."
        );
        let whole = rating(|rating| {
            rating.stars.rollover = Some(3);
            rating.rollover_risk_percent = Some(20.0);
        });
        assert_eq!(summary(&whole), "Rollover 3. Rollover risk 20.0%.");
        assert_eq!(summary(&rating(|_| {})), "Not rated by NHTSA.");
        let weighed = rating(|rating| {
            rating.curb_weight_kg = Some(1241);
            rating.curb_weight_lb = Some(2735);
        });
        assert_eq!(
            summary(&weighed),
            "Not rated by NHTSA. Tested at 1241 kg (2735 lb)."
        );
    }
}
