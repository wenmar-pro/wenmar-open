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
use crate::site::{self, Page, Problem, report};
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
    report: report::ReportLinks,
    /// Recalls that may apply to the model year the VIN decodes to. `None`
    /// on a server that holds none, and for a VIN the catalog cannot place.
    recalls: Option<site::recalls::Brief>,
    /// The manufacturer's bulletins that list that model year. `None` on a
    /// server that holds none, and for a VIN the catalog cannot place.
    bulletins: Option<site::bulletins::Brief>,
    /// The open NHTSA investigations that list that model year. `None`
    /// when there is none, and on a server that holds none.
    investigations: Option<site::investigations::Notice>,
    /// The parts-ordering card.
    card: crate::api::types::PartsCard,
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
    push(
        &mut vehicle,
        "Generation",
        decode.generation.as_ref().map(generation),
    );
    push(&mut vehicle, "Body", decode.body.as_ref());
    push(&mut vehicle, "Cab", decode.cab_type.as_ref());
    push(&mut vehicle, "Bed", decode.bed_type.as_ref());
    push(&mut vehicle, "Bed length, inches", decode.bed_length_in);
    push(&mut vehicle, "Doors", decode.doors);
    push(&mut vehicle, "Weight rating", decode.gvwr.as_ref());
    push(
        &mut vehicle,
        "Weight rating, up to",
        decode.gvwr_to.as_ref(),
    );
    push(&mut vehicle, "Curb weight, pounds", decode.curb_weight_lb);
    if decode.year.is_some() && !decode.year_conclusive {
        push(
            &mut vehicle,
            "Model year",
            Some("Not settled by the VIN: check the label on the door jamb"),
        );
    }

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
        push(&mut engine, "Horsepower", range(found.hp, found.hp_to));
        push(&mut engine, "Valve train", found.valve_train.as_ref());
        push(&mut engine, "Fuel injection", found.fuel_injection.as_ref());
        push(&mut engine, "Second fuel", found.fuel_secondary.as_ref());
        push(&mut engine, "Cooling", found.cooling.as_ref());
        push(&mut engine, "Engine maker", found.manufacturer.as_ref());
    }

    let mut battery = Vec::new();
    if let Some(found) = &decode.electric {
        push(&mut battery, "Battery", found.battery_type.as_ref());
        push(
            &mut battery,
            "Battery energy, kWh",
            range(found.battery_kwh, found.battery_kwh_to),
        );
        push(
            &mut battery,
            "Battery voltage",
            range(found.battery_volts, found.battery_volts_to),
        );
        push(&mut battery, "Battery details", found.battery_info.as_ref());
        push(&mut battery, "Drive unit", found.drive_unit.as_ref());
        push(&mut battery, "Charger level", found.charger_level.as_ref());
        push(&mut battery, "Charger power, kW", found.charger_power_kw);
    }

    let mut driveline = Vec::new();
    push(&mut driveline, "Transmission", decode.transmission.as_ref());
    push(
        &mut driveline,
        "Transmission speeds",
        decode.transmission_speeds,
    );
    push(&mut driveline, "Drivetrain", decode.drivetrain.as_ref());
    push(&mut driveline, "Brakes", decode.brake_system.as_ref());
    push(
        &mut driveline,
        "Brake details",
        decode.brake_system_description.as_ref(),
    );
    push(&mut driveline, "Axles", decode.axles);
    push(
        &mut driveline,
        "Axle layout",
        decode.axle_configuration.as_ref(),
    );
    push(
        &mut driveline,
        "Wheelbase, inches",
        range(decode.wheelbase_in, decode.wheelbase_to_in),
    );
    push(&mut driveline, "Track width, inches", decode.track_width_in);
    push(&mut driveline, "Steering", decode.steering.as_ref());

    let mut safety = Vec::new();
    let mut assistance = Vec::new();
    let mut lights = Vec::new();
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
            &mut safety,
            "Seat cushion air bags",
            found.airbags_seat_cushion.as_ref(),
        );
        push(&mut safety, "Seat belts", found.seat_belts.as_ref());
        push(&mut safety, "Pretensioners", found.pretensioner.as_ref());
        push(
            &mut safety,
            "Crash notification",
            found.crash_notification.as_ref(),
        );
        push(
            &mut safety,
            "Event data recorder",
            found.event_data_recorder.as_ref(),
        );

        push(
            &mut lights,
            "Headlamps",
            found.headlamp_light_source.as_ref(),
        );
        push(
            &mut lights,
            "Adaptive driving beam",
            found.adaptive_driving_beam.as_ref(),
        );
        push(
            &mut lights,
            "Headlamps that turn",
            found.adaptive_headlights.as_ref(),
        );
        push(
            &mut lights,
            "Automatic high beams",
            found.automatic_high_beams.as_ref(),
        );
        push(
            &mut lights,
            "Daytime running lights",
            found.daytime_running_lights.as_ref(),
        );
        push(
            &mut lights,
            "Keyless ignition",
            found.keyless_ignition.as_ref(),
        );
        push(
            &mut lights,
            "Auto-reverse windows",
            found.auto_reverse_windows.as_ref(),
        );
        push(
            &mut lights,
            "Pedestrian alert sound",
            found.pedestrian_alert_sound.as_ref(),
        );

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
    push(&mut fitted, "Wheels", decode.wheels);

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
    push(&mut built, "Built for", decode.destination_market.as_ref());

    let mut notes = Vec::new();
    for note in &decode.notes {
        push(&mut notes, "Note", Some(note));
    }

    [
        ("Vehicle", vehicle),
        ("Engine", engine),
        ("Battery and charging", battery),
        ("Transmission and drive", driveline),
        ("Safety equipment", safety),
        ("Driver assistance", assistance),
        ("Lights and keys", lights),
        ("Wheels and seats", fitted),
        ("Where it was built", built),
        ("From the manufacturer", notes),
    ]
    .into_iter()
    .filter(|(_, rows)| !rows.is_empty())
    .map(|(name, rows)| Group { name, rows })
    .collect()
}

/// A generation as a line: `8th generation (CP/CS), 2008-2012`.
fn generation(found: &crate::api::types::Generation) -> String {
    let code = found
        .code
        .as_deref()
        .filter(|code| *code != found.name)
        .map(|code| format!(" ({code})"))
        .unwrap_or_default();
    let years = match found.year_to {
        Some(to) => format!("{}-{to}", found.year_from),
        None => format!("{} on", found.year_from),
    };
    format!("{}{code}, {years}", found.name)
}

/// An amount, or a range of two.
fn range(from: Option<f64>, to: Option<f64>) -> Option<String> {
    match (from, to) {
        (Some(from), Some(to)) if to > from => Some(format!("{from}-{to}")),
        (Some(from), _) => Some(from.to_string()),
        (None, Some(to)) => Some(format!("up to {to}")),
        (None, None) => None,
    }
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
    let recalls = match decode.catalog.as_ref() {
        Some(selection) => site::recalls::brief(&state, &selection.vehicle_id).await,
        None => None,
    };
    let bulletins = match decode.catalog.as_ref() {
        Some(selection) => site::bulletins::brief(&state, &selection.vehicle_id).await,
        None => None,
    };
    let investigations = match decode.catalog.as_ref() {
        Some(selection) => site::investigations::open_for(&state, &selection.vehicle_id).await,
        None => None,
    };
    let card = crate::api::parts_card::card_of(&state, &decode).await;
    // A report is public, so it names the vehicle by the first 11
    // characters of the VIN and never by the whole of it.
    let stem = report::vin_stem(&decode.vin);
    let report = report::links(
        state.config(),
        &format!("/vin/{stem}"),
        &format!("{headline}, VIN {stem}"),
    );
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
            report,
            recalls,
            bulletins,
            card,
            investigations,
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
    fn the_new_attributes_go_in_their_groups() {
        let mut data = MemoryData::new()
            .with_manufacturer(Manufacturer::new("KM8", "Hyundai Motor Co"))
            .with_schema("KM8", 1, 2022, None);
        for (element, value) in [
            (Element::Model, "Kona"),
            (Element::Make, "Hyundai"),
            (Element::Other(71), "147"),
            (Element::Other(125), "201"),
            (Element::Other(42), "Hydraulic"),
            (Element::Other(59), "64.8"),
            (Element::Other(176), "Standard"),
            (Element::Other(129), "Gasoline Direct Injection"),
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
        let groups = groups(&VinDecode::new(decoded, None));
        let find = |group: &str, label: &str| {
            groups
                .iter()
                .find(|found| found.name == group)
                .and_then(|found| found.rows.iter().find(|row| row.label == label))
                .map(|row| row.value.clone())
        };
        assert_eq!(find("Engine", "Horsepower").as_deref(), Some("147-201"));
        assert_eq!(
            find("Transmission and drive", "Brakes").as_deref(),
            Some("Hydraulic")
        );
        assert_eq!(
            find("Battery and charging", "Battery energy, kWh").as_deref(),
            Some("64.8")
        );
        assert_eq!(
            find("Lights and keys", "Keyless ignition").as_deref(),
            Some("Standard")
        );
        assert_eq!(
            find("From the manufacturer", "Note").as_deref(),
            Some("Gasoline Direct Injection")
        );
        assert_eq!(
            find("Vehicle", "Generation").as_deref(),
            Some("OS, 2018-2023")
        );
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
