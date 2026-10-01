use serde::Serialize;

use crate::check_digit::CheckDigit;
use crate::engine::Engine;

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
