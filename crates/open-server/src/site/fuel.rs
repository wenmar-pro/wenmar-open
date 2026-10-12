//! What the model-year page says of fuel consumption and engine oil.
//!
//! A line is never more exact than its match. Ratings matched to one
//! catalog engine are shown under that engine, as a range over its
//! drives and transmissions. Ratings matched to the model year alone are
//! shown under the engine size the source gives, marked as not matched
//! to an engine.

use wenmar_shopdata::Dataset;

use crate::api::fuel::{OIL_SOURCE, fuel_of, fuel_shop, oil_of};
use crate::api::recalls::model_year;
use crate::api::types::{EngineOil, FuelRating};
use crate::state::AppState;

/// What marks a line whose engine was not settled.
pub const NOT_MATCHED: &str = "not matched to an engine";

/// One line of the fuel table: one engine, one country.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FuelLine {
    /// `2.0L`, or `2.0 L, 4 cylinders` when no engine was settled.
    pub engine: String,
    /// Empty, or [`NOT_MATCHED`].
    pub note: String,
    /// `EPA` or `NRCan`.
    pub source: String,
    /// `7.4 to 7.8 L/100 km (30 to 32 MPG)`, or `17.4 kWh/100 km`.
    pub combined: String,
    /// `7.8 to 8.4 L/100 km (28 to 30 MPG)`. Empty for an electric line.
    pub city: String,
    pub highway: String,
    /// `Regular Gasoline`, or two grades joined by ` or `.
    pub fuel: String,
}

/// One line of the oil table: one engine.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OilLine {
    pub engine: String,
    pub note: String,
    /// `0W-20`, or `5W-30 or 0W-30`.
    pub grades: String,
}

/// What a page shows of a model year's fuel data.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Brief {
    /// `2023 Hyundai Kona`.
    pub name: String,
    pub fuel: Vec<FuelLine>,
    pub oil: Vec<OilLine>,
    /// `EPA ratings as of 2026-09-29. Natural Resources Canada ratings as of 2026-10-06.`
    pub as_of: String,
    /// `per EPA fuel economy certification`.
    pub oil_source: String,
    pub attribution: Vec<String>,
}

/// `7.4`, or `7.4 to 7.8` when the figures differ. Empty for none.
fn span(mut figures: Vec<f64>) -> String {
    figures.sort_by(f64::total_cmp);
    match (figures.first(), figures.last()) {
        (Some(low), Some(high)) if low == high => format!("{low:.1}"),
        (Some(low), Some(high)) => format!("{low:.1} to {high:.1}"),
        _ => String::new(),
    }
}

fn whole_span(mut figures: Vec<u16>) -> String {
    figures.sort_unstable();
    match (figures.first(), figures.last()) {
        (Some(low), Some(high)) if low == high => low.to_string(),
        (Some(low), Some(high)) => format!("{low} to {high}"),
        _ => String::new(),
    }
}

/// `7.4 to 7.8 L/100 km (30 to 32 MPG)`. Empty when there is no figure.
fn consumption(litres: Vec<f64>, mpg: Vec<u16>) -> String {
    let (litres, mpg) = (span(litres), whole_span(mpg));
    match (litres.is_empty(), mpg.is_empty()) {
        (true, _) => String::new(),
        (false, true) => format!("{litres} L/100 km"),
        (false, false) => format!("{litres} L/100 km ({mpg} MPG)"),
    }
}

/// How a rating or an oil grade names its engine, and the note beside it.
fn engine_name(
    label: Option<&str>,
    litres: Option<f64>,
    cylinders: Option<u8>,
    electric: bool,
) -> (String, String) {
    if let Some(label) = label {
        return (label.to_owned(), String::new());
    }
    let name = match (litres, cylinders) {
        _ if electric => "Electric".to_owned(),
        (Some(litres), Some(cylinders)) => format!("{litres:.1} L, {cylinders} cylinders"),
        (Some(litres), None) => format!("{litres:.1} L"),
        _ => "Engine not given".to_owned(),
    };
    (name, NOT_MATCHED.to_owned())
}

/// Engine name, note and source of a group, and its ratings.
type Group<'a> = ((String, String, String), Vec<&'a FuelRating>);

fn fuel_lines(ratings: &[FuelRating]) -> Vec<FuelLine> {
    // In first-seen order.
    let mut groups: Vec<Group<'_>> = Vec::new();
    for rating in ratings {
        let electric = rating.fuel_grade == "Electricity";
        let (engine, note) = engine_name(
            rating.engine.as_ref().map(|engine| engine.label.as_str()),
            rating.displacement_l,
            rating.cylinders,
            electric,
        );
        let source = if rating.source == "epa" {
            "EPA"
        } else {
            "NRCan"
        };
        let key = (engine, note, source.to_owned());
        match groups.iter_mut().find(|(known, _)| *known == key) {
            Some((_, members)) => members.push(rating),
            None => groups.push((key, vec![rating])),
        }
    }
    groups
        .into_iter()
        .map(|((engine, note, source), members)| {
            let litres = |pick: fn(&crate::api::types::Consumption) -> Option<f64>| -> Vec<f64> {
                members
                    .iter()
                    .filter_map(|rating| rating.l_100km.as_ref().and_then(pick))
                    .collect()
            };
            let mpg = |pick: fn(&crate::api::types::Mpg) -> Option<u16>| -> Vec<u16> {
                members
                    .iter()
                    .filter_map(|rating| rating.mpg.as_ref().and_then(pick))
                    .collect()
            };
            let mut combined = consumption(litres(|c| c.combined), mpg(|m| m.combined));
            if combined.is_empty() {
                let kwh = span(
                    members
                        .iter()
                        .filter_map(|rating| rating.kwh_100km)
                        .collect(),
                );
                if !kwh.is_empty() {
                    combined = format!("{kwh} kWh/100 km");
                }
            }
            let mut grades: Vec<&str> = Vec::new();
            for rating in &members {
                if !grades.contains(&rating.fuel_grade.as_str()) {
                    grades.push(&rating.fuel_grade);
                }
            }
            FuelLine {
                engine,
                note,
                source,
                combined,
                city: consumption(litres(|c| c.city), mpg(|m| m.city)),
                highway: consumption(litres(|c| c.highway), mpg(|m| m.highway)),
                fuel: grades.join(" or "),
            }
        })
        .collect()
}

fn oil_lines(oil: &[EngineOil]) -> Vec<OilLine> {
    oil.iter()
        .filter(|oil| !oil.grades.is_empty())
        .map(|oil| {
            let (engine, note) = engine_name(
                oil.engine.as_ref().map(|engine| engine.label.as_str()),
                oil.displacement_l,
                oil.cylinders,
                false,
            );
            OilLine {
                engine,
                note,
                grades: oil.grades.join(" or "),
            }
        })
        .collect()
}

/// What is known of a model year's fuel consumption and engine oil, by
/// its vehicle id.
///
/// `None` on a server that holds neither, for an id that names no model
/// year, when nothing is on file for it, and when the data could not be
/// read. The page that asked is then shown as it is without it.
pub async fn brief(state: &AppState, vehicle_id: &str) -> Option<Brief> {
    let fuel_shop = fuel_shop(state, None).ok();
    let oil_shop = state.dataset(Dataset::OilGrade).ok();
    if fuel_shop.is_none() && oil_shop.is_none() {
        return None;
    }
    let id = vehicle_id.to_owned();
    let found = match state.db().run(move |worker| model_year(worker, &id)).await {
        Ok(Ok(found)) => found?,
        Ok(Err(error)) => {
            tracing::warn!(%error, "fuel data left off a page");
            return None;
        }
        Err(error) => {
            tracing::warn!(%error, "fuel data left off a page");
            return None;
        }
    };
    // A failure in either is logged where it is turned into an `ApiError`.
    let fuel = match fuel_shop {
        Some(shop) => fuel_of(state, shop, &found, None).await.ok(),
        None => None,
    };
    let oil = match oil_shop {
        Some(shop) => oil_of(state, shop, &found).await.ok().unwrap_or_default(),
        None => Vec::new(),
    };
    let lines = fuel
        .as_ref()
        .map(|answer| fuel_lines(&answer.ratings))
        .unwrap_or_default();
    let oil = oil_lines(&oil);
    if lines.is_empty() && oil.is_empty() {
        return None;
    }
    let mut as_of = Vec::new();
    if let Some(answer) = &fuel {
        if let Some(day) = &answer.as_of.us {
            as_of.push(format!("EPA ratings as of {day}."));
        }
        if let Some(day) = &answer.as_of.ca {
            as_of.push(format!("Natural Resources Canada ratings as of {day}."));
        }
    }
    Some(Brief {
        name: format!(
            "{} {} {}",
            found.vehicle.year, found.vehicle.make, found.vehicle.model
        ),
        fuel: lines,
        oil,
        as_of: as_of.join(" "),
        oil_source: format!("per {OIL_SOURCE}"),
        attribution: fuel.map(|answer| answer.attribution).unwrap_or_default(),
    })
}

/// The Markdown of a brief, for the page's `.md` twin.
pub fn markdown(brief: &Brief) -> String {
    let mut text = String::new();
    if !brief.fuel.is_empty() {
        text.push_str("\n## Fuel economy\n\n| Engine | Source | Combined | City | Highway | Recommended fuel |\n|---|---|---|---|---|---|\n");
        for line in &brief.fuel {
            let engine = if line.note.is_empty() {
                line.engine.clone()
            } else {
                format!("{} ({})", line.engine, line.note)
            };
            text.push_str(&format!(
                "| {engine} | {} | {} | {} | {} | {} |\n",
                line.source, line.combined, line.city, line.highway, line.fuel
            ));
        }
        if !brief.as_of.is_empty() {
            text.push_str(&format!("\n{}\n", brief.as_of));
        }
        for line in &brief.attribution {
            text.push_str(&format!("\n{line}\n"));
        }
    }
    if !brief.oil.is_empty() {
        text.push_str(&format!(
            "\n## Engine oil\n\nViscosity grade {}. It is the grade the maker certified the engine with, not a reading of the owner's manual, and it gives no capacity.\n\n",
            brief.oil_source
        ));
        for line in &brief.oil {
            let engine = if line.note.is_empty() {
                line.engine.clone()
            } else {
                format!("{} ({})", line.engine, line.note)
            };
            text.push_str(&format!("- {engine}: {}\n", line.grades));
        }
    }
    text
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn figures_are_one_number_or_a_range() {
        assert_eq!(span(vec![7.8, 7.4, 7.4]), "7.4 to 7.8");
        assert_eq!(span(vec![7.4, 7.4]), "7.4");
        assert_eq!(span(vec![9.0]), "9.0");
        assert_eq!(span(vec![]), "");
        assert_eq!(whole_span(vec![32, 30]), "30 to 32");
        assert_eq!(whole_span(vec![32]), "32");
        assert_eq!(
            consumption(vec![7.4, 7.8], vec![32, 30]),
            "7.4 to 7.8 L/100 km (30 to 32 MPG)"
        );
        assert_eq!(consumption(vec![7.4], vec![]), "7.4 L/100 km");
        assert_eq!(consumption(vec![], vec![32]), "");
    }

    #[test]
    fn an_engine_is_named_only_when_one_was_settled() {
        assert_eq!(
            engine_name(Some("2.0L"), Some(2.0), Some(4), false),
            ("2.0L".to_owned(), String::new())
        );
        assert_eq!(
            engine_name(None, Some(1.8), Some(4), false),
            ("1.8 L, 4 cylinders".to_owned(), NOT_MATCHED.to_owned())
        );
        assert_eq!(
            engine_name(None, None, None, true),
            ("Electric".to_owned(), NOT_MATCHED.to_owned())
        );
        assert_eq!(
            engine_name(None, None, None, false),
            ("Engine not given".to_owned(), NOT_MATCHED.to_owned())
        );
    }
}
