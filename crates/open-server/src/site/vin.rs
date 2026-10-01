//! The result page, `/vin/{vin}`.
//!
//! It is never indexed and never cached by anyone but the visitor: there is
//! one for every vehicle ever built, and each names one real vehicle.

use askama::Template;
use axum::extract::rejection::PathRejection;
use axum::extract::{Path, State};
use axum::http::{HeaderValue, StatusCode, header};
use axum::response::{IntoResponse, Redirect, Response};
use wenmar_vehicles::VehicleId;

use crate::api::types::{VinDecode, Warning};
use crate::api::vin::decode_many;
use crate::error::ApiError;
use crate::site::{self, Page, Problem};
use crate::state::AppState;

const CACHE_RESULT: &str = "private, max-age=3600";

/// One line of the sheet.
pub struct Row {
    pub label: &'static str,
    pub value: String,
    /// Where the value leads, when this site has a page about it.
    pub href: Option<String>,
}

/// A titled list of lines, shown as a table.
pub struct Group {
    pub name: &'static str,
    pub rows: Vec<Row>,
}

fn push(rows: &mut Vec<Row>, label: &'static str, value: Option<impl ToString>) {
    if let Some(value) = value {
        rows.push(Row {
            label,
            value: value.to_string(),
            href: None,
        });
    }
}

#[derive(Template)]
#[template(path = "vin.html")]
struct Result {
    page: Page,
    vin: String,
    headline: String,
    warnings: Vec<Warning>,
    groups: Vec<Group>,
    /// The sheet as plain text, for the copy button.
    copy: String,
    /// The address and the name of the model-year page.
    model_year: Option<(String, String)>,
    pro: String,
}

/// The year, make, model and trim, or what there is of them.
pub fn headline(decode: &VinDecode) -> String {
    let parts: Vec<String> = [
        decode.year.map(|year| year.to_string()),
        decode.make.clone(),
        decode.model.clone(),
        decode.trim.clone(),
    ]
    .into_iter()
    .flatten()
    .collect();
    if parts.is_empty() {
        format!("A vehicle by {}", decode.manufacturer.name)
    } else {
        parts.join(" ")
    }
}

/// The decode as short tables, in the order a service advisor reads it.
/// Empty groups are left out.
pub fn groups(decode: &VinDecode) -> Vec<Group> {
    let mut vehicle = Vec::new();
    push(&mut vehicle, "Year", decode.year);
    push(&mut vehicle, "Make", decode.make.as_ref());
    push(&mut vehicle, "Model", decode.model.as_ref());
    push(&mut vehicle, "Series", decode.series.as_ref());
    push(&mut vehicle, "Trim", decode.trim.as_ref());
    push(&mut vehicle, "Body", decode.body.as_ref());
    push(&mut vehicle, "Doors", decode.doors);
    push(&mut vehicle, "Weight rating", decode.gvwr.as_ref());

    let mut engine = Vec::new();
    if let Some(found) = &decode.engine {
        push(&mut engine, "Engine", found.label.as_ref());
        push(&mut engine, "Engine model", found.model.as_ref());
        push(&mut engine, "Displacement, litres", found.displacement_l);
        push(&mut engine, "Cylinders", found.cylinders);
        push(&mut engine, "Layout", found.configuration.as_ref());
        push(&mut engine, "Fuel", found.fuel.as_ref());
        push(
            &mut engine,
            "Turbo",
            found.turbo.map(|turbo| if turbo { "Yes" } else { "No" }),
        );
        push(
            &mut engine,
            "Electrification",
            found.electrification.as_ref(),
        );
    }

    let mut driveline = Vec::new();
    push(&mut driveline, "Transmission", decode.transmission.as_ref());
    push(
        &mut driveline,
        "Transmission speeds",
        decode.transmission_speeds,
    );
    push(&mut driveline, "Drivetrain", decode.drivetrain.as_ref());

    let mut safety = Vec::new();
    let mut assistance = Vec::new();
    if let Some(found) = &decode.safety {
        push(&mut safety, "ABS", found.abs.as_ref());
        push(&mut safety, "Stability control", found.esc.as_ref());
        push(
            &mut safety,
            "Traction control",
            found.traction_control.as_ref(),
        );
        push(&mut safety, "Tire pressure monitoring", found.tpms.as_ref());
        push(&mut safety, "Front air bags", found.airbags_front.as_ref());
        push(&mut safety, "Side air bags", found.airbags_side.as_ref());
        push(
            &mut safety,
            "Curtain air bags",
            found.airbags_curtain.as_ref(),
        );
        push(&mut safety, "Knee air bags", found.airbags_knee.as_ref());

        push(
            &mut assistance,
            "Adaptive cruise",
            found.adaptive_cruise.as_ref(),
        );
        push(
            &mut assistance,
            "Forward collision warning",
            found.forward_collision.as_ref(),
        );
        push(
            &mut assistance,
            "Automatic braking",
            found.auto_brake.as_ref(),
        );
        push(
            &mut assistance,
            "Pedestrian braking",
            found.pedestrian_braking.as_ref(),
        );
        push(
            &mut assistance,
            "Dynamic brake support",
            found.dynamic_brake_support.as_ref(),
        );
        push(
            &mut assistance,
            "Lane departure warning",
            found.lane_departure.as_ref(),
        );
        push(&mut assistance, "Lane keeping", found.lane_keep.as_ref());
        push(
            &mut assistance,
            "Lane centering",
            found.lane_centering.as_ref(),
        );
        push(
            &mut assistance,
            "Blind spot warning",
            found.blind_spot.as_ref(),
        );
        push(
            &mut assistance,
            "Rear cross traffic alert",
            found.rear_cross_traffic.as_ref(),
        );
        push(
            &mut assistance,
            "Backup camera",
            found.backup_camera.as_ref(),
        );
        push(
            &mut assistance,
            "Parking assist",
            found.park_assist.as_ref(),
        );
    }

    let mut fitted = Vec::new();
    push(&mut fitted, "Front wheels, inches", decode.wheel_size_front);
    push(&mut fitted, "Rear wheels, inches", decode.wheel_size_rear);
    push(&mut fitted, "Seats", decode.seats);
    push(&mut fitted, "Seat rows", decode.seat_rows);

    let mut built = Vec::new();
    push(&mut built, "Manufacturer", Some(&decode.manufacturer.name));
    // The code is from the data file, so it is checked before it goes into
    // an address.
    let code = &decode.manufacturer.wmi;
    built.push(Row {
        label: "Manufacturer code",
        value: code.clone(),
        href: site::wmi::is_code(code).then(|| format!("/wmi/{code}")),
    });
    push(&mut built, "Country", decode.manufacturer.country.as_ref());
    push(
        &mut built,
        "Vehicle type",
        decode.manufacturer.vehicle_type.as_ref(),
    );
    push(&mut built, "Plant code", Some(decode.plant.code));
    push(&mut built, "Plant city", decode.plant.city.as_ref());
    push(&mut built, "Plant state", decode.plant.state.as_ref());
    push(&mut built, "Plant country", decode.plant.country.as_ref());

    [
        ("Vehicle", vehicle),
        ("Engine", engine),
        ("Transmission and drive", driveline),
        ("Safety equipment", safety),
        ("Driver assistance", assistance),
        ("Wheels and seats", fitted),
        ("Where it was built", built),
    ]
    .into_iter()
    .filter(|(_, rows)| !rows.is_empty())
    .map(|(name, rows)| Group { name, rows })
    .collect()
}

/// The sheet as plain text, for the clipboard: the vehicle, the VIN, then
/// every group and every line.
pub fn summary(headline: &str, vin: &str, groups: &[Group]) -> String {
    let mut text = format!("{headline}\nVIN {vin}\n");
    for group in groups {
        text.push_str(&format!("\n{}\n", group.name));
        for row in &group.rows {
            text.push_str(&format!("{}: {}\n", row.label, row.value));
        }
    }
    text
}

fn private(mut response: Response) -> Response {
    // A failure is not kept at all: the next try may succeed.
    let failed = response.status().is_client_error() || response.status().is_server_error();
    let lifetime = if failed { "no-store" } else { CACHE_RESULT };
    let headers = response.headers_mut();
    headers.insert(header::CACHE_CONTROL, HeaderValue::from_static(lifetime));
    headers.insert("x-robots-tag", HeaderValue::from_static("noindex"));
    response
}

fn problem(
    state: &AppState,
    status: StatusCode,
    heading: &str,
    message: String,
    suggestions: Vec<String>,
) -> Response {
    private(site::html(
        status,
        &Problem {
            page: Page::new(state, format!("{heading} - Wenmar Open"), heading).with_mono(),
            heading: heading.to_owned(),
            message,
            suggestions,
            retry: true,
        },
    ))
}

pub async fn result(
    State(state): State<AppState>,
    path: std::result::Result<Path<String>, PathRejection>,
) -> Response {
    let Ok(Path(typed)) = path else {
        return problem(
            &state,
            StatusCode::BAD_REQUEST,
            "That is not a VIN",
            "A VIN uses only digits and letters.".to_owned(),
            Vec::new(),
        );
    };
    let decoded = match decode_many(&state, vec![typed.clone()], None).await {
        Ok(mut decoded) => decoded.pop(),
        Err(error) => return private(site::failed(&state, &error)),
    };
    let decode = match decoded {
        Some(Ok(decode)) => decode,
        Some(Err(ApiError::InvalidVin { message, details })) => {
            let suggestions = details["suggestions"]
                .as_array()
                .map(|list| {
                    list.iter()
                        .filter_map(|item| item.as_str())
                        // A suggestion is a well-formed VIN, so it is safe in
                        // an address. Check anyway.
                        .filter(|item| item.chars().all(|c| c.is_ascii_alphanumeric()))
                        .map(str::to_owned)
                        .collect()
                })
                .unwrap_or_default();
            let mut message = message;
            if let Some(first) = message.get_mut(0..1) {
                first.make_ascii_uppercase();
            }
            return problem(
                &state,
                StatusCode::BAD_REQUEST,
                "That is not a VIN",
                format!("{message}."),
                suggestions,
            );
        }
        Some(Err(ApiError::NotFound(message))) => {
            return problem(
                &state,
                StatusCode::NOT_FOUND,
                "We do not know this manufacturer",
                format!(
                    "{message} The first three characters of a VIN name who built the vehicle. Check them against the vehicle."
                ),
                Vec::new(),
            );
        }
        Some(Err(error)) => return private(site::failed(&state, &error)),
        None => return private(site::failed(&state, &ApiError::Internal)),
    };
    // One vehicle, one address: `km8-k2...` goes to `KM8K2...`.
    if typed != decode.vin {
        return private(Redirect::permanent(&format!("/vin/{}", decode.vin)).into_response());
    }

    let headline = headline(&decode);
    let model_year = decode
        .catalog
        .as_ref()
        .and_then(|selection| VehicleId::parse(&selection.vehicle_id))
        .map(|id| {
            (
                format!("/makes/{}/{}/{}", id.make, id.model, id.year),
                format!(
                    "{} {} {}",
                    id.year,
                    decode.make.clone().unwrap_or_default(),
                    decode.model.clone().unwrap_or_default()
                ),
            )
        });
    // A suggestion goes into an address, so it is checked like the ones on
    // the "not a VIN" page, though the decoder only offers well-formed VINs.
    let warnings: Vec<Warning> = decode
        .warnings
        .iter()
        .cloned()
        .map(|mut warning| {
            warning.suggestions.retain(|suggestion| {
                suggestion.len() == 17
                    && suggestion
                        .chars()
                        .all(|character| character.is_ascii_alphanumeric())
            });
            warning
        })
        .collect();
    let groups = groups(&decode);
    let copy = summary(&headline, &decode.vin, &groups);
    private(site::html(
        StatusCode::OK,
        &Result {
            page: Page::new(
                &state,
                format!("{headline} - Wenmar Open"),
                "A decoded VIN: year, make, model, trim, engine and safety equipment.",
            )
            .with_mono(),
            vin: decode.vin.clone(),
            headline,
            warnings,
            groups,
            copy,
            model_year,
            pro: site::pro_link("vin-result"),
        },
    ))
}

#[cfg(test)]
mod tests {
    use wenmar_vin::{DecodeOptions, Decoder, Element, Manufacturer, MemoryData};

    use super::*;

    /// A decode with something in every group of the sheet.
    fn full() -> VinDecode {
        let mut data = MemoryData::new()
            .with_manufacturer(Manufacturer {
                wmi: "KM8".to_owned(),
                name: "Hyundai Motor Co".to_owned(),
                make: Some("Hyundai".to_owned()),
                country: Some("South Korea".to_owned()),
                vehicle_type: None,
                light_vehicle: true,
            })
            .with_schema("KM8", 1, 2022, None);
        for (element, value) in [
            (Element::Model, "Kona"),
            (Element::Trim, "SE"),
            (Element::FuelTypePrimary, "Gasoline"),
            (Element::TransmissionStyle, "Automatic"),
            (Element::DriveType, "FWD"),
            (Element::Abs, "Standard"),
            (Element::AdaptiveCruise, "Optional"),
            (Element::WheelSizeFront, "17"),
            (Element::Seats, "5"),
            (Element::PlantCity, "Ulsan"),
        ] {
            data = data.with_pattern(1, "K2***", element, value);
        }
        let decoded = Decoder::new(data)
            .decode(
                "KM8K2CAB4PU001140",
                DecodeOptions {
                    current_year: Some(2026),
                    ..DecodeOptions::default()
                },
            )
            .unwrap();
        VinDecode::new(decoded, None)
    }

    #[test]
    fn the_sheet_has_seven_short_groups_in_reading_order() {
        let groups = groups(&full());
        let names: Vec<&str> = groups.iter().map(|group| group.name).collect();
        assert_eq!(
            names,
            [
                "Vehicle",
                "Engine",
                "Transmission and drive",
                "Safety equipment",
                "Driver assistance",
                "Wheels and seats",
                "Where it was built",
            ]
        );
        let find = |group: &str, label: &str| {
            groups
                .iter()
                .find(|found| found.name == group)
                .and_then(|found| found.rows.iter().find(|row| row.label == label))
                .map(|row| (row.value.clone(), row.href.clone()))
        };
        assert_eq!(
            find("Transmission and drive", "Drivetrain"),
            Some(("FWD".to_owned(), None))
        );
        assert_eq!(
            find("Driver assistance", "Adaptive cruise"),
            Some(("Optional".to_owned(), None))
        );
        assert_eq!(
            find("Wheels and seats", "Seats"),
            Some(("5".to_owned(), None))
        );
        // The one value that leads somewhere.
        assert_eq!(
            find("Where it was built", "Manufacturer code"),
            Some(("KM8".to_owned(), Some("/wmi/KM8".to_owned())))
        );
        // Nothing has moved into a group it does not belong in.
        assert_eq!(find("Vehicle", "Transmission"), None);
        assert_eq!(find("Vehicle", "Seats"), None);
    }

    #[test]
    fn the_sheet_as_text_has_every_line() {
        let decode = full();
        let groups = groups(&decode);
        let text = summary(&headline(&decode), &decode.vin, &groups);
        assert!(
            text.starts_with("2023 Hyundai Kona SE\nVIN KM8K2CAB4PU001140\n\nVehicle\nYear: 2023\nMake: Hyundai\n"),
            "{text}"
        );
        assert!(
            text.contains("\nTransmission and drive\nTransmission: Automatic\nDrivetrain: FWD\n")
        );
        assert!(text.ends_with("Plant city: Ulsan\n"), "{text}");
        let rows: usize = groups.iter().map(|group| group.rows.len()).sum();
        assert_eq!(text.lines().count(), 2 + groups.len() * 2 + rows);
    }
}
