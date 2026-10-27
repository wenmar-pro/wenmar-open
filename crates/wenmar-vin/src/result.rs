use std::collections::BTreeMap;

use serde::Serialize;

use crate::check_digit::CheckDigit;
use crate::engine::Engine;
use crate::generation::Generation;

/// A decoded VIN. Fields that could not be determined are left out of JSON.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[non_exhaustive]
pub struct Decoded {
    pub vin: String,
    /// Whether the check digit is correct.
    pub valid: bool,
    pub check_digit: CheckDigit,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub year: Option<u16>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub make: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub model: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub series: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub trim: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub body: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub doors: Option<u8>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub drivetrain: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub transmission: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub transmission_speeds: Option<u8>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub engine: Option<Engine>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub safety: Option<Safety>,
    /// Front wheel diameter in inches.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub wheel_size_front: Option<u8>,
    /// Rear wheel diameter in inches.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub wheel_size_rear: Option<u8>,
    /// Number of seats.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub seats: Option<u8>,
    /// Number of rows of seats.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub seat_rows: Option<u8>,
    /// Gross vehicle weight rating class, as NHTSA words it.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub gvwr: Option<String>,
    /// Manufacturer base price in US dollars, when NHTSA has one.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub base_price_usd: Option<f64>,
    pub manufacturer: ManufacturerInfo,
    pub plant: Plant,
    pub warnings: Vec<Warning>,
    /// Whether the model year is settled. Position 10 repeats every 30
    /// years. For cars, multipurpose vehicles and light trucks position 7
    /// settles which cycle; for other vehicles the year is the one the
    /// data explains better, and it is not settled when both years have
    /// data. Check the year on the vehicle's label when this is `false`.
    pub year_conclusive: bool,
    /// The drive, in one of five words: `FWD`, `RWD`, `AWD`, `4WD` or
    /// `2WD`. Left out when NHTSA's value fits none of them, such as `6x4`;
    /// `drivetrain` still has it.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub drive: Option<Drive>,
    /// The body, in one word from a short list. `body` has NHTSA's words.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub body_style: Option<BodyStyle>,
    /// The generation of the model this model year belongs to.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub generation: Option<Generation>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub series2: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub trim2: Option<String>,
    /// Cab type of a truck, as NHTSA words it, such as `Crew/Super Crew/Crew Max`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cab_type: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub bed_type: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub bed_length_in: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub curb_weight_lb: Option<f64>,
    /// Upper end of the gross vehicle weight rating class, when NHTSA gives
    /// a range.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub gvwr_to: Option<String>,
    /// Wheelbase in inches, or the shorter end of a range.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub wheelbase_in: Option<f64>,
    /// The longer end of the wheelbase range, in inches.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub wheelbase_to_in: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub track_width_in: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub wheels: Option<u8>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub axles: Option<u8>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub axle_configuration: Option<String>,
    /// Steering wheel side, `Left-Hand Drive (LHD)` or `Right-Hand Drive (RHD)`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub steering: Option<String>,
    /// Service brakes, such as `Hydraulic` or `Air`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub brake_system: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub brake_system_description: Option<String>,
    /// The market the vehicle was built for, when NHTSA records one.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub destination_market: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub top_speed_mph: Option<f64>,
    /// Battery and charging, for an electric or hybrid vehicle.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub electric: Option<Electric>,
    /// Free-text notes the manufacturer filed, such as other engine or
    /// restraint information, each once.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub notes: Vec<String>,
    /// Every attribute the decode resolved, by NHTSA's element code (as
    /// NHTSA's own API names it), with where it came from. The fields above
    /// are taken from these.
    #[serde(skip_serializing_if = "BTreeMap::is_empty")]
    pub attributes: BTreeMap<String, Attribute>,
}

/// The drive, in the words shops use.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[non_exhaustive]
pub enum Drive {
    #[serde(rename = "FWD")]
    Fwd,
    #[serde(rename = "RWD")]
    Rwd,
    #[serde(rename = "AWD")]
    Awd,
    #[serde(rename = "4WD")]
    FourWd,
    #[serde(rename = "2WD")]
    TwoWd,
}

impl Drive {
    /// The drive for NHTSA's value, if it is one of the five.
    pub fn from_vpic(value: &str) -> Option<Drive> {
        let short = value.split('/').next().unwrap_or_default().trim();
        Some(match short.to_ascii_uppercase().as_str() {
            "FWD" => Drive::Fwd,
            "RWD" => Drive::Rwd,
            "AWD" => Drive::Awd,
            "4WD" | "4X4" => Drive::FourWd,
            "2WD" if !value.contains("4WD") => Drive::TwoWd,
            "4X2" => Drive::TwoWd,
            _ => return None,
        })
    }
}

/// The body, in one word.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum BodyStyle {
    Sedan,
    Coupe,
    Convertible,
    Hatchback,
    Wagon,
    /// Sport utility and crossover utility vehicles.
    Suv,
    Minivan,
    /// Cargo, passenger and step vans.
    Van,
    /// Pickups, sport utility trucks among them.
    Pickup,
    /// Trucks and truck-tractors.
    Truck,
    Bus,
    Motorhome,
    /// A chassis that another company finishes: chassis cabs, cutaways.
    Incomplete,
    Motorcycle,
    OffRoad,
    LowSpeed,
    Trailer,
    Other,
}

impl BodyStyle {
    /// The style of NHTSA's body class. The words are matched without
    /// regard to case or brackets, which NHTSA has changed before.
    pub fn from_vpic(value: &str) -> Option<BodyStyle> {
        let text = value.trim().to_lowercase();
        if text.is_empty() {
            return None;
        }
        let has = |word: &str| text.contains(word);
        let starts = |word: &str| text.starts_with(word);
        Some(if starts("incomplete") {
            BodyStyle::Incomplete
        } else if starts("motorcycle") {
            BodyStyle::Motorcycle
        } else if starts("off-road") || starts("off road") {
            BodyStyle::OffRoad
        } else if starts("low speed") {
            BodyStyle::LowSpeed
        } else if starts("trailer") {
            BodyStyle::Trailer
        } else if starts("sedan") {
            BodyStyle::Sedan
        } else if starts("coupe") {
            BodyStyle::Coupe
        } else if starts("convertible") || starts("roadster") {
            BodyStyle::Convertible
        } else if starts("hatchback") {
            BodyStyle::Hatchback
        } else if starts("wagon") {
            BodyStyle::Wagon
        } else if has("sport utility truck") || starts("pickup") {
            BodyStyle::Pickup
        } else if has("sport utility vehicle") || has("crossover utility") {
            BodyStyle::Suv
        } else if starts("minivan") {
            BodyStyle::Minivan
        } else if has("van") {
            BodyStyle::Van
        } else if starts("truck") {
            BodyStyle::Truck
        } else if starts("bus") || starts("streetcar") {
            BodyStyle::Bus
        } else if starts("motorhome") {
            BodyStyle::Motorhome
        } else {
            BodyStyle::Other
        })
    }
}

/// Battery and charging, as the manufacturer reported them.
#[derive(Debug, Clone, PartialEq, Default, Serialize)]
#[non_exhaustive]
pub struct Electric {
    /// Chemistry, such as `Lithium-Ion/Li-Ion`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub battery_type: Option<String>,
    /// Energy in kWh, or the lower end of a range.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub battery_kwh: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub battery_kwh_to: Option<f64>,
    /// Voltage, or the lower end of a range.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub battery_volts: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub battery_volts_to: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub battery_info: Option<String>,
    /// Motors and where they sit, such as `Dual Motor`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub drive_unit: Option<String>,
    /// Such as `Level 2`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub charger_level: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub charger_power_kw: Option<f64>,
}

/// Where an attribute's value came from.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum AttributeSource {
    /// A pattern of the VIN's own characters.
    VinPattern,
    /// What NHTSA records for the engine model a pattern named.
    EngineModel,
    /// A specification sheet the manufacturer filed for the make, model and
    /// year, whose conditions the decode met.
    SpecificationSheet,
}

/// One attribute of a decode.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[non_exhaustive]
pub struct Attribute {
    /// NHTSA's name for it, such as `Engine Brake (hp) From`.
    pub name: String,
    /// Its value, as NHTSA words it. Notes that came more than once are
    /// joined with `; `.
    pub value: String,
    pub source: AttributeSource,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[non_exhaustive]
pub struct ManufacturerInfo {
    pub wmi: String,
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub country: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub vehicle_type: Option<String>,
}

/// Where the vehicle was built. `code` is position 11 of the VIN.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[non_exhaustive]
pub struct Plant {
    pub code: char,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub city: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub state: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub country: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub company: Option<String>,
}

/// Safety equipment as the manufacturer reported it, for example `Standard`,
/// `Optional`, or a system type such as `Direct`.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize)]
#[non_exhaustive]
pub struct Safety {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub abs: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub esc: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    /// How tire pressure is monitored, as NHTSA words it, such as `Direct`.
    pub tpms: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub adaptive_cruise: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub lane_keep: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub lane_departure: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub blind_spot: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub forward_collision: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub auto_brake: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub backup_camera: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub airbags_front: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub airbags_side: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub airbags_curtain: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub airbags_knee: Option<String>,
    /// Traction control.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub traction_control: Option<String>,
    /// Dynamic brake support.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub dynamic_brake_support: Option<String>,
    /// Rear cross traffic alert.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub rear_cross_traffic: Option<String>,
    /// Parking assist.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub park_assist: Option<String>,
    /// Pedestrian automatic emergency braking.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub pedestrian_braking: Option<String>,
    /// Lane centering assistance.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub lane_centering: Option<String>,
    /// Keyless ignition: a key that must be programmed to the vehicle.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub keyless_ignition: Option<String>,
    /// Headlamp light source, such as `LED` or `Halogen`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub headlamp_light_source: Option<String>,
    /// Adaptive driving beam.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub adaptive_driving_beam: Option<String>,
    /// Headlamps that turn with the steering.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub adaptive_headlights: Option<String>,
    /// Semiautomatic headlamp beam switching.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub automatic_high_beams: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub daytime_running_lights: Option<String>,
    /// Auto-reverse for power windows and sunroofs.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub auto_reverse_windows: Option<String>,
    /// Automatic or advanced automatic crash notification.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub crash_notification: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub event_data_recorder: Option<String>,
    /// The sound a hybrid or electric vehicle makes for pedestrians.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub pedestrian_alert_sound: Option<String>,
    /// Seat belt type, such as `Manual`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub seat_belts: Option<String>,
    /// Seat belt pretensioners.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub pretensioner: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub airbags_seat_cushion: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum WarningCode {
    /// Position 9 does not match the rest of the VIN.
    InvalidCheckDigit,
    /// Position 10 does not encode a model year.
    ModelYearUnknown,
    /// The manufacturer is known but nothing else matched.
    NoPatterns,
    /// Some details matched but the model did not.
    ModelUnresolved,
}

/// Something the caller should know about a decode that still succeeded.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[non_exhaustive]
pub struct Warning {
    pub code: WarningCode,
    pub message: String,
    /// VINs the caller may have meant.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub suggestions: Vec<String>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn warnings_serialize_with_snake_case_codes_and_no_empty_suggestions() {
        let warning = Warning {
            code: WarningCode::ModelYearUnknown,
            message: "No year.".to_owned(),
            suggestions: Vec::new(),
        };
        assert_eq!(
            serde_json::to_value(&warning).unwrap(),
            json!({ "code": "model_year_unknown", "message": "No year." })
        );
    }

    #[test]
    fn unknown_safety_fields_are_left_out() {
        let safety = Safety {
            tpms: Some("Direct".to_owned()),
            ..Safety::default()
        };
        assert_eq!(
            serde_json::to_value(&safety).unwrap(),
            json!({ "tpms": "Direct" })
        );
    }

    #[test]
    fn nhtsa_drive_types_read_as_the_five_words() {
        let cases = [
            ("FWD/Front-Wheel Drive", Some(Drive::Fwd)),
            ("RWD/Rear-Wheel Drive", Some(Drive::Rwd)),
            ("AWD/All-Wheel Drive", Some(Drive::Awd)),
            ("4WD/4-Wheel Drive/4x4", Some(Drive::FourWd)),
            ("4x2", Some(Drive::TwoWd)),
            ("2WD/4WD", None),
            ("6x4", None),
            ("Other", None),
        ];
        for (value, drive) in cases {
            assert_eq!(Drive::from_vpic(value), drive, "{value}");
        }
        assert_eq!(serde_json::to_value(Drive::FourWd).unwrap(), json!("4WD"));
    }

    #[test]
    fn nhtsa_body_classes_read_as_one_word() {
        let cases = [
            ("Sedan/Saloon", BodyStyle::Sedan),
            (
                "Sport Utility Vehicle [SUV]/Multipurpose Vehicle [MPV]",
                BodyStyle::Suv,
            ),
            (
                "Sport Utility Vehicle (SUV)/Multipurpose Vehicle (MPV)",
                BodyStyle::Suv,
            ),
            ("Crossover Utility Vehicle [CUV]", BodyStyle::Suv),
            ("Sport Utility Truck [SUT]", BodyStyle::Pickup),
            ("Pickup", BodyStyle::Pickup),
            ("Hatchback/Liftback/Notchback", BodyStyle::Hatchback),
            ("Convertible/Cabriolet", BodyStyle::Convertible),
            ("Roadster", BodyStyle::Convertible),
            ("Minivan", BodyStyle::Minivan),
            ("Cargo Van", BodyStyle::Van),
            ("Step Van/Walk-in Van", BodyStyle::Van),
            ("Truck-Tractor", BodyStyle::Truck),
            ("Bus - School Bus", BodyStyle::Bus),
            (
                "Incomplete - Chassis Cab (Single Cab)",
                BodyStyle::Incomplete,
            ),
            ("Incomplete - Cutaway", BodyStyle::Incomplete),
            ("Motorcycle - Scooter", BodyStyle::Motorcycle),
            ("Off-road Vehicle - Snowmobile", BodyStyle::OffRoad),
            (
                "Low Speed Vehicle [LSV]/Neighborhood Electric Vehicle [NEV]",
                BodyStyle::LowSpeed,
            ),
            ("Trailer", BodyStyle::Trailer),
            ("Motorhome", BodyStyle::Motorhome),
            ("Fire Apparatus", BodyStyle::Other),
        ];
        for (value, style) in cases {
            assert_eq!(BodyStyle::from_vpic(value), Some(style), "{value}");
        }
        assert_eq!(BodyStyle::from_vpic(" "), None);
        assert_eq!(
            serde_json::to_value(BodyStyle::OffRoad).unwrap(),
            json!("off_road")
        );
    }

    #[test]
    fn a_plant_always_has_its_code() {
        let plant = Plant {
            code: 'U',
            city: None,
            state: None,
            country: None,
            company: None,
        };
        assert_eq!(
            serde_json::to_value(&plant).unwrap(),
            json!({ "code": "U" })
        );
    }
}
