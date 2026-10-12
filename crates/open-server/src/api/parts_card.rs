//! The parts-ordering card for a VIN: what a parts counter asks for, each
//! line with where it came from.
//!
//! Nothing on the card is guessed. A line is there only when its source
//! gives the fact: the decode, the catalog when the decode settles the
//! engine, EPA's emissions file when a rating was matched to that engine,
//! and a recall campaign when it names build dates for the model year.

use axum::Json;
use axum::extract::rejection::{PathRejection, QueryRejection};
use axum::extract::{Path, Query, State};
use serde_json::json;
use wenmar_shopdata::Dataset;

use crate::api::bad_query;
use crate::api::fuel::{VinYearQuery, engines_of, settled, specs_of};
use crate::api::recalls::{NO_CATALOG_VEHICLE, model_year, recall_shop, recalls_of};
use crate::api::types::{PartsCard, PartsCardLine, VinDecode, Warning};
use crate::api::vin::decode_many;
use crate::error::{ApiError, ErrorBody};
use crate::state::AppState;

/// Where a line of the card comes from.
pub const FROM_VIN: &str = "the VIN";
pub const FROM_DECODE: &str = "VIN decode (NHTSA vPIC)";
pub const FROM_CATALOG: &str = "Wenmar Open catalog (NHTSA vPIC)";
pub const FROM_EPA: &str = "EPA emissions file, matched to this engine";

fn line(label: &str, value: impl Into<String>, source: impl Into<String>) -> PartsCardLine {
    PartsCardLine {
        label: label.to_owned(),
        value: value.into(),
        source: source.into(),
    }
}

/// The lines the decode alone gives.
fn decode_lines(decode: &VinDecode) -> Vec<PartsCardLine> {
    let mut lines = vec![line("VIN", decode.vin.clone(), FROM_VIN)];
    let mut add = |label: &str, value: Option<String>| {
        if let Some(value) = value.filter(|value| !value.trim().is_empty()) {
            lines.push(line(label, value, FROM_DECODE));
        }
    };
    add("Model year", decode.year.map(|year| year.to_string()));
    add("Make", decode.make.clone());
    add("Model", decode.model.clone());
    add("Trim", decode.trim.clone());
    add("Body", decode.body.clone());
    add("Drive", decode.drivetrain.clone());
    add("Transmission", decode.transmission.clone());
    if let Some(engine) = &decode.engine {
        add("Engine", engine.label.clone());
        add(
            "Displacement",
            engine.displacement_l.map(|litres| format!("{litres:.1} L")),
        );
        add("Cylinders", engine.cylinders.map(|count| count.to_string()));
        add("Fuel", engine.fuel.clone());
    }
    let plant: Vec<String> = [
        decode.plant.city.clone(),
        decode.plant.state.clone(),
        decode.plant.country.clone(),
    ]
    .into_iter()
    .flatten()
    .filter(|part| !part.trim().is_empty())
    .collect();
    add("Plant", (!plant.is_empty()).then(|| plant.join(", ")));
    // Position 8 and position 11 are read off the VIN itself.
    if let Some(eighth) = decode.vin.chars().nth(7) {
        lines.push(line(
            "VIN position 8 (engine)",
            eighth.to_string(),
            FROM_VIN,
        ));
    }
    lines.push(line(
        "VIN position 11 (plant)",
        decode.plant.code.to_string(),
        FROM_VIN,
    ));
    lines
}

/// The card as plain text: one `Label: value` line each.
pub fn text(lines: &[PartsCardLine]) -> String {
    lines
        .iter()
        .map(|line| format!("{}: {}", line.label, line.value))
        .collect::<Vec<_>>()
        .join("\n")
}

/// The card for a decoded VIN. What the shop file or the catalog cannot
/// give is left off: a failure reading them leaves those lines out and
/// never fails the card.
pub async fn card_of(state: &AppState, decode: &VinDecode) -> PartsCard {
    let mut lines = decode_lines(decode);
    let mut attribution: Vec<String> = Vec::new();
    let found = match decode.catalog.as_ref() {
        Some(selection) => {
            let id = selection.vehicle_id.clone();
            match state.db().run(move |worker| model_year(worker, &id)).await {
                Ok(Ok(found)) => found,
                _ => None,
            }
        }
        None => None,
    };
    let wanted = decode
        .catalog
        .as_ref()
        .and_then(|selection| selection.engine_id.clone());
    let mut engine = None;
    if let Some(found) = &found {
        if let Ok(engines) = engines_of(state, found).await {
            engine = settled(&engines, wanted.as_deref()).map(|(_, engine)| engine);
        }
        if let Some(engine) = &engine {
            let value = match &engine.vin8 {
                Some(vin8) => format!("{} (VIN position 8: {vin8})", engine.label),
                None => engine.label.clone(),
            };
            lines.push(line("Catalog engine", value, FROM_CATALOG));
            // Emission standards only of ratings matched to this engine.
            if let Ok(shop) = state.dataset(Dataset::Specs)
                && let Ok(specs) = specs_of(state, shop, found).await
            {
                let mut seen: Vec<(u8, String)> = Vec::new();
                for spec in specs
                    .specs
                    .iter()
                    .filter(|spec| spec.engine.as_ref() == Some(engine))
                {
                    for standard in &spec.emission_standards {
                        let key = (standard.sales_area, standard.standard.clone());
                        if seen.contains(&key) {
                            continue;
                        }
                        seen.push(key);
                        let value = match &standard.standard_text {
                            Some(words) => format!("{words} ({})", standard.standard),
                            None => standard.standard.clone(),
                        };
                        lines.push(line(
                            &format!("Emission standard, EPA sales area {}", standard.sales_area),
                            value,
                            FROM_EPA,
                        ));
                    }
                }
            }
        }
        // Build dates a recall campaign names for this model year.
        if let Ok(shop) = recall_shop(state, None)
            && let Ok(recalls) = recalls_of(state, shop, found, None).await
        {
            for recall in &recalls.recalls {
                let value = match (&recall.built_from, &recall.built_to) {
                    (Some(from), Some(to)) => format!("{from} to {to}"),
                    (Some(from), None) => format!("from {from}"),
                    (None, Some(to)) => format!("to {to}"),
                    (None, None) => continue,
                };
                let office = if recall.source == "nhtsa" {
                    "NHTSA"
                } else {
                    "Transport Canada"
                };
                lines.push(line(
                    &format!("Build dates covered by recall {}", recall.campaign),
                    value,
                    format!("{office} recall {}", recall.campaign),
                ));
            }
            for credit in recalls.attribution {
                if !attribution.contains(&credit) {
                    attribution.push(credit);
                }
            }
        }
    }
    let mut warnings = decode.warnings.clone();
    if found.is_none() {
        warnings.push(Warning {
            code: "no_catalog_vehicle".to_owned(),
            message: NO_CATALOG_VEHICLE.to_owned(),
            suggestions: Vec::new(),
        });
    }
    PartsCard {
        vin: decode.vin.clone(),
        vehicle: found.map(|found| found.vehicle),
        engine_settled: engine.is_some(),
        text: text(&lines),
        lines,
        attribution,
        warnings,
    }
}

pub async fn parts_card_op(
    state: &AppState,
    vin: String,
    year: Option<u16>,
) -> Result<PartsCard, ApiError> {
    let mut decoded = decode_many(state, vec![vin], year).await?;
    let decode = match decoded.pop() {
        Some(result) => result?,
        None => return Err(ApiError::internal("a decode gave no answer")),
    };
    Ok(card_of(state, &decode).await)
}

/// The parts-ordering card for a VIN.
///
/// What a parts counter asks for, one line each, and where each line came
/// from: the decode, the catalog engine when the decode settles it, the
/// emission standard when EPA's file was matched to that engine, and the
/// build dates a recall campaign names for the model year. A build-date
/// line is the range a campaign covers, not this vehicle's build date.
/// `text` is the card as plain text, ready to paste. Works without shop
/// data: the lines that need it are then left off.
#[utoipa::path(
    get,
    path = "/vin/{vin}/parts-card",
    tag = "VIN",
    params(
        ("vin" = String, Path, description = "A 17-character VIN.", example = "KM8K2CAB4PU001140"),
        VinYearQuery
    ),
    responses(
        (status = 200, description = "The card.", body = PartsCard),
        (status = 400, description = "`invalid_vin`, or `validation_failed` for `year`.", body = ErrorBody),
        (status = 404, description = "`not_found`: no manufacturer is registered for the first three characters.", body = ErrorBody)
    )
)]
pub async fn parts_card(
    State(state): State<AppState>,
    path: Result<Path<String>, PathRejection>,
    query: Result<Query<VinYearQuery>, QueryRejection>,
) -> Result<Json<PartsCard>, ApiError> {
    let Ok(Path(vin)) = path else {
        return Err(ApiError::InvalidVin {
            message: "a VIN uses only digits and letters other than I, O and Q".to_owned(),
            details: json!({ "suggestions": [] }),
        });
    };
    let Query(query) = query.map_err(bad_query)?;
    Ok(Json(parts_card_op(&state, vin, query.year).await?))
}
