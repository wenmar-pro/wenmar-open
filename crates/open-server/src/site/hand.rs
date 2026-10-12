//! What the model-year page shows of the facts read by hand: the engine
//! oil beside EPA's grade, the other fluids, and the tire fitments.
//!
//! Every figure is followed by the document it was read from, the day,
//! and a link that reports an error in that one figure. A model year with
//! no record gets no section: nothing is shown in its place.

use wenmar_shopdata::Dataset;

use crate::api::hand::{recorded_of, tires_of};
use crate::api::recalls::model_year;
use crate::api::types::{FluidRecord, FrontRear, TireFitment};
use crate::site::markdown::escape;
use crate::site::{fuel, report};
use crate::state::AppState;

/// Where a figure was read, and where to report an error in it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Origin {
    /// The document: `Test manual, page 1`.
    pub source: String,
    /// The day it was read.
    pub checked: String,
    /// The address that reports an error in this figure.
    pub report: String,
}

/// What a person read for one engine's oil.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Figure {
    /// `0W-99, TEST-SPEC-1, 99.9 L (105.6 US qt) with filter`.
    pub text: String,
    pub origin: Origin,
}

/// One engine of the oil table.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OilRow {
    pub engine: String,
    /// Empty, or what marks a grade not matched to a catalog engine.
    pub note: String,
    /// EPA's grade, or empty when EPA has none for the engine.
    pub epa: String,
    /// What was read by hand. Empty when nobody has recorded the engine.
    pub figures: Vec<Figure>,
}

/// One line of the table of the other fluids.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FluidRow {
    /// `Transmission fluid`.
    pub fluid: String,
    /// The engine or the transmission the line is for, or `All`.
    pub applies: String,
    /// `TEST ATF 9`, or `R-TEST, 9990 g (352.4 oz)`.
    pub text: String,
    pub origin: Origin,
}

/// One fitment of the tire table.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TireRow {
    /// `999/99R99`, or `998/98R98 front, 997/97R97 rear`.
    pub size: String,
    /// `Base, Deluxe`, or `Not given`.
    pub trims: String,
    /// `990 kPa (144 psi) front, 980 kPa (142 psi) rear`. Empty for none.
    pub pressure: String,
    /// `999 N·m (737 lb·ft)`. Empty for none.
    pub torque: String,
    pub bolt_pattern: String,
    pub origin: Origin,
}

/// What a page shows of a model year's hand-read facts.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Section {
    /// `2023 Hyundai Kona`.
    pub name: String,
    /// Every engine with a grade from EPA or a record. Empty when no oil
    /// was recorded: the page then keeps EPA's table as it is.
    pub oil: Vec<OilRow>,
    pub fluids: Vec<FluidRow>,
    pub tires: Vec<TireRow>,
}

/// A figure with at least one decimal: `99.9`, `88.0`, `0.125`.
fn plain(figure: f64) -> String {
    if figure.fract() == 0.0 {
        format!("{figure:.1}")
    } else {
        format!("{figure}")
    }
}

/// ` (Canada)` for a record of one country, or nothing.
fn market(market: Option<&str>) -> &'static str {
    match market {
        Some("ca") => " (Canada)",
        Some("us") => " (United States)",
        _ => "",
    }
}

fn oil_text(record: &FluidRecord) -> String {
    let mut parts: Vec<String> = Vec::new();
    parts.extend(record.grade.clone());
    parts.extend(record.specification.clone());
    if let (Some(litres), Some(quarts)) = (record.capacity_l, record.capacity_qt) {
        let filter = match record.with_filter {
            Some(true) => " with filter",
            Some(false) => " without filter",
            None => "",
        };
        parts.push(format!(
            "{} L ({} US qt){filter}",
            plain(litres),
            plain(quarts)
        ));
    }
    format!("{}{}", parts.join(", "), market(record.market.as_deref()))
}

fn fluid_text(record: &FluidRecord) -> String {
    let mut parts: Vec<String> = Vec::new();
    parts.extend(record.specification.clone());
    if let (Some(grams), Some(ounces)) = (record.charge_g, record.charge_oz) {
        parts.push(format!("{grams} g ({} oz)", plain(ounces)));
    }
    format!("{}{}", parts.join(", "), market(record.market.as_deref()))
}

/// `990 kPa (144 psi)`, or empty when either figure is missing.
fn pressure_at(kpa: Option<i64>, psi: Option<i64>) -> Option<String> {
    Some(format!("{} kPa ({} psi)", kpa?, psi?))
}

fn pressure_text(kpa: Option<FrontRear>, psi: Option<FrontRear>) -> String {
    let (Some(kpa), Some(psi)) = (kpa, psi) else {
        return String::new();
    };
    let front = pressure_at(kpa.front, psi.front);
    let rear = pressure_at(kpa.rear, psi.rear);
    match (front, rear) {
        (Some(front), Some(rear)) if front == rear => format!("{front} front and rear"),
        (Some(front), Some(rear)) => format!("{front} front, {rear} rear"),
        (Some(front), None) => format!("{front} front"),
        (None, Some(rear)) => format!("{rear} rear"),
        (None, None) => String::new(),
    }
}

fn tire_row(tire: &TireFitment, origin: Origin) -> TireRow {
    TireRow {
        size: match &tire.size_rear {
            Some(rear) => format!("{} front, {rear} rear", tire.size),
            None => tire.size.clone(),
        },
        trims: if tire.trims.is_empty() {
            "Not given".to_owned()
        } else {
            tire.trims.join(", ")
        },
        pressure: format!(
            "{}{}",
            pressure_text(tire.pressure_kpa, tire.pressure_psi),
            market(tire.market.as_deref())
        ),
        torque: match (tire.lug_torque_nm, tire.lug_torque_lbft) {
            (Some(nm), Some(lbft)) => format!("{nm} N·m ({lbft} lb·ft)"),
            _ => String::new(),
        },
        bolt_pattern: tire.bolt_pattern.clone().unwrap_or_default(),
        origin,
    }
}

/// The hand-read facts of a model year, by its vehicle id. `path` is the
/// page's own, for the report links. `epa` is the fuel brief of the same
/// page, whose oil lines the oil table starts from.
///
/// `None` on a server that holds neither dataset, for an id that names no
/// model year, when nothing is recorded for it, and when the data could
/// not be read. The page that asked is then shown as it is without it.
pub async fn section(
    state: &AppState,
    vehicle_id: &str,
    path: &str,
    epa: Option<&fuel::Brief>,
) -> Option<Section> {
    let fluid_shop = state.dataset(Dataset::Fluids).ok();
    let tire_shop = state.dataset(Dataset::Tires).ok();
    if fluid_shop.is_none() && tire_shop.is_none() {
        return None;
    }
    let id = vehicle_id.to_owned();
    let found = match state.db().run(move |worker| model_year(worker, &id)).await {
        Ok(Ok(found)) => found?,
        Ok(Err(error)) => {
            tracing::warn!(%error, "hand-read facts left off a page");
            return None;
        }
        Err(error) => {
            tracing::warn!(%error, "hand-read facts left off a page");
            return None;
        }
    };
    // A failure in either is logged where it is turned into an `ApiError`.
    let recorded = match fluid_shop {
        Some(shop) => recorded_of(state, shop, &found)
            .await
            .ok()
            .unwrap_or_default(),
        None => Default::default(),
    };
    let fitments = match tire_shop {
        Some(shop) => tires_of(shop, &found)
            .await
            .ok()
            .map(|answer| answer.fitments)
            .unwrap_or_default(),
        None => Vec::new(),
    };
    if recorded.is_empty() && fitments.is_empty() {
        return None;
    }
    let name = format!(
        "{} {} {}",
        found.vehicle.year, found.vehicle.make, found.vehicle.model
    );
    let origin = |source: &str, checked: &str, field: &str| {
        let links = report::field_links(state.config(), path, &name, field);
        Origin {
            source: source.to_owned(),
            checked: checked.to_owned(),
            report: links.form.unwrap_or(links.issue),
        }
    };

    let mut oil: Vec<OilRow> = Vec::new();
    if !recorded.engine_oil.is_empty() {
        for line in epa.map(|brief| brief.oil.as_slice()).unwrap_or_default() {
            oil.push(OilRow {
                engine: line.engine.clone(),
                note: line.note.clone(),
                epa: line.grades.clone(),
                figures: Vec::new(),
            });
        }
        for record in &recorded.engine_oil {
            // An oil record always names its engine.
            let Some(engine) = &record.engine else {
                continue;
            };
            let figure = Figure {
                text: oil_text(record),
                origin: origin(
                    &record.source,
                    &record.checked,
                    &format!("Engine oil, {}", engine.label),
                ),
            };
            match oil
                .iter_mut()
                .find(|row| row.note.is_empty() && row.engine == engine.label)
            {
                Some(row) => row.figures.push(figure),
                None => oil.push(OilRow {
                    engine: engine.label.clone(),
                    note: String::new(),
                    epa: String::new(),
                    figures: vec![figure],
                }),
            }
        }
    }

    let mut fluids = Vec::new();
    for (label, records) in [
        ("Transmission fluid", &recorded.transmission),
        ("Coolant", &recorded.coolant),
        ("A/C refrigerant", &recorded.refrigerant),
        ("Brake fluid", &recorded.brake),
    ] {
        for record in records {
            let mut applies: Vec<&str> = Vec::new();
            if let Some(engine) = &record.engine {
                applies.push(&engine.label);
            }
            if let Some(what) = &record.applies_to {
                applies.push(what);
            }
            let applies = if applies.is_empty() {
                "All".to_owned()
            } else {
                applies.join(", ")
            };
            let field = if applies == "All" {
                label.to_owned()
            } else {
                format!("{label}, {applies}")
            };
            fluids.push(FluidRow {
                fluid: label.to_owned(),
                applies,
                text: fluid_text(record),
                origin: origin(&record.source, &record.checked, &field),
            });
        }
    }

    let tires = fitments
        .iter()
        .map(|tire| {
            tire_row(
                tire,
                origin(
                    &tire.source,
                    &tire.checked,
                    &format!("Tires, {}", tire.size),
                ),
            )
        })
        .collect();

    Some(Section {
        name,
        oil,
        fluids,
        tires,
    })
}

fn origin_cells(origin: &Origin) -> String {
    format!(
        "{} | {} | [Report an error]({})",
        escape(&origin.source),
        origin.checked,
        origin.report
    )
}

/// The Markdown of a section, for the page's `.md` twin.
pub fn markdown(section: &Section) -> String {
    let mut text = String::new();
    if !section.oil.is_empty() {
        text.push_str("\n## Engine oil\n\nThe grade in the second column is the one the maker certified the engine with, as EPA records it. The third column was read by a person from the document named beside it. An empty cell means nobody has recorded it.\n\n| Engine | Grade, per EPA fuel economy certification | Read from the maker's document | Source | Checked | |\n|---|---|---|---|---|---|\n");
        for row in &section.oil {
            let engine = if row.note.is_empty() {
                escape(&row.engine)
            } else {
                format!("{} ({})", escape(&row.engine), row.note)
            };
            if row.figures.is_empty() {
                text.push_str(&format!("| {engine} | {} | | | | |\n", escape(&row.epa)));
            }
            for figure in &row.figures {
                text.push_str(&format!(
                    "| {engine} | {} | {} | {} |\n",
                    escape(&row.epa),
                    escape(&figure.text),
                    origin_cells(&figure.origin)
                ));
            }
        }
    }
    if !section.fluids.is_empty() {
        text.push_str("\n## Other fluids\n\n| Fluid | For | Specification | Source | Checked | |\n|---|---|---|---|---|---|\n");
        for row in &section.fluids {
            text.push_str(&format!(
                "| {} | {} | {} | {} |\n",
                row.fluid,
                escape(&row.applies),
                escape(&row.text),
                origin_cells(&row.origin)
            ));
        }
    }
    if !section.tires.is_empty() {
        text.push_str("\n## Tires and wheels\n\nEvery original-equipment size on record, with the trims it covers. The door placard of a vehicle says which it has. Pressures are cold.\n\n| Size | Trims | Cold pressure | Wheel nut torque | Bolt pattern | Source | Checked | |\n|---|---|---|---|---|---|---|---|\n");
        for row in &section.tires {
            text.push_str(&format!(
                "| {} | {} | {} | {} | {} | {} |\n",
                escape(&row.size),
                escape(&row.trims),
                row.pressure,
                row.torque,
                escape(&row.bolt_pattern),
                origin_cells(&row.origin)
            ));
        }
    }
    text
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_pressure_is_said_once_when_front_and_rear_agree() {
        let pair = |front, rear| Some(FrontRear { front, rear });
        assert_eq!(
            pressure_text(pair(Some(990), Some(980)), pair(Some(144), Some(142))),
            "990 kPa (144 psi) front, 980 kPa (142 psi) rear"
        );
        assert_eq!(
            pressure_text(pair(Some(990), Some(990)), pair(Some(144), Some(144))),
            "990 kPa (144 psi) front and rear"
        );
        assert_eq!(pressure_text(None, None), "");
        assert_eq!(plain(99.9), "99.9");
        assert_eq!(plain(88.0), "88.0");
        assert_eq!(plain(0.125), "0.125");
    }
}
