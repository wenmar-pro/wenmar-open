//! The JSON the API sends, as types of this crate.
//!
//! `wenmar-vin` and `wenmar-vehicles` have their own result types. They are
//! copied field by field here for two reasons: the OpenAPI description is
//! generated from these definitions, and the two libraries must not depend
//! on an OpenAPI crate. A test compares the JSON of both for every field.

use serde::Serialize;
use utoipa::ToSchema;
use wenmar_vehicles as vehicles;
use wenmar_vin as vin;

/// A decoded VIN. A field that could not be determined is left out.
#[derive(Debug, Clone, PartialEq, Serialize, ToSchema)]
pub struct VinDecode {
    #[schema(example = "KM8K2CAB4PU001140")]
    pub vin: String,
    /// Whether the check digit, position 9, is right. Many genuine VINs from
    /// outside North America fail it.
    pub valid: bool,
    pub check_digit: CheckDigit,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[schema(example = 2023)]
    pub year: Option<u16>,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[schema(example = "Hyundai")]
    pub make: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[schema(example = "Kona")]
    pub model: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub series: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[schema(example = "SE")]
    pub trim: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub body: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub doors: Option<u8>,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[schema(example = "FWD")]
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
    #[serde(skip_serializing_if = "Option::is_none")]
    pub seats: Option<u8>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub seat_rows: Option<u8>,
    /// Gross vehicle weight rating class, as NHTSA words it.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub gvwr: Option<String>,
    /// Manufacturer base price in US dollars, when NHTSA has one.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub base_price_usd: Option<f64>,
    pub manufacturer: Manufacturer,
    pub plant: Plant,
    /// Things to know about a decode that still succeeded.
    pub warnings: Vec<Warning>,
    /// The catalog entry this VIN reaches, for prefilling a vehicle form.
    /// Left out when the catalog cannot place the vehicle.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub catalog: Option<Selection>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, ToSchema)]
pub struct CheckDigit {
    pub valid: bool,
    /// What position 9 should be.
    #[schema(value_type = String, example = "4")]
    pub expected: char,
    /// What position 9 is.
    #[schema(value_type = String, example = "4")]
    pub actual: char,
}

#[derive(Debug, Clone, PartialEq, Serialize, ToSchema)]
pub struct Engine {
    /// The short name shops use, such as `2.0L Turbo` or `3.5L V6`.
    #[serde(skip_serializing_if = "Option::is_none")]
    #[schema(example = "2.0L")]
    pub label: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub model: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub displacement_l: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub displacement_cc: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cylinders: Option<u8>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub configuration: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub fuel: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub turbo: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub electrification: Option<String>,
}

/// Safety equipment as the manufacturer reported it: `Standard`, `Optional`,
/// or a system type such as `Direct`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, ToSchema)]
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
    #[serde(skip_serializing_if = "Option::is_none")]
    pub traction_control: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub dynamic_brake_support: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub rear_cross_traffic: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub park_assist: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub pedestrian_braking: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub lane_centering: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, ToSchema)]
pub struct Manufacturer {
    /// The manufacturer code: three characters, or six for a low-volume
    /// maker.
    #[schema(example = "KM8")]
    pub wmi: String,
    #[schema(example = "Hyundai Motor Co")]
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub country: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub vehicle_type: Option<String>,
}

/// Where the vehicle was built. `code` is position 11 of the VIN.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, ToSchema)]
pub struct Plant {
    #[schema(value_type = String, example = "U")]
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

#[derive(Debug, Clone, PartialEq, Eq, Serialize, ToSchema)]
pub struct Warning {
    /// `invalid_check_digit`, `model_year_unknown`, `no_patterns` or
    /// `model_unresolved`. More may be added.
    #[schema(example = "invalid_check_digit")]
    pub code: String,
    pub message: String,
    /// VINs the caller may have meant.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub suggestions: Vec<String>,
}

/// A make in the make step of a vehicle form.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, ToSchema)]
pub struct Make {
    /// The make's id form.
    #[schema(example = "mercedes-benz")]
    pub id: String,
    #[schema(example = "Mercedes-Benz")]
    pub name: String,
    /// Whether it is on the list of popular makes, which are given first.
    pub popular: bool,
}

/// A model in the model step.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, ToSchema)]
pub struct Model {
    /// The model's id form. Unique within its make.
    #[schema(example = "f-150")]
    pub id: String,
    #[schema(example = "F-150")]
    pub name: String,
    /// The first and last model year the model exists in.
    pub year_from: u16,
    pub year_to: u16,
}

/// A submodel: a trim, or a series where the data has no trim.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, ToSchema)]
pub struct Submodel {
    #[schema(example = "ex-l")]
    pub id: String,
    #[schema(example = "EX-L")]
    pub name: String,
    /// `trim` or `series` as NHTSA has it, or `preset` from this project's
    /// own list.
    #[schema(example = "trim")]
    pub kind: String,
}

/// An engine in the engine step.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, ToSchema)]
pub struct EngineOption {
    #[schema(example = "3-5l-turbo-v6")]
    pub id: String,
    #[schema(example = "3.5L Turbo V6")]
    pub label: String,
    /// The characters in position 8 of a VIN that mean this engine and no
    /// other for this model year, where the data settles it.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub vin8: Option<String>,
    /// Whether the engine comes from this project's list instead of NHTSA.
    pub preset: bool,
}

/// One vehicle of the catalog, down to whatever level was asked for.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, ToSchema)]
pub struct Entry {
    /// The stable id. It stays the same from one data release to the next
    /// for as long as the names do.
    #[schema(example = "2019_honda_civic_si")]
    pub id: String,
    pub year: u16,
    pub make: String,
    pub model: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub submodel: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub engine: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub transmission: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub drive: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub body: Option<String>,
    /// NHTSA's vehicle types for this model year, such as `Truck`.
    pub vehicle_types: Vec<String>,
    /// One line naming everything above that is known.
    #[schema(example = "2019 Honda Civic Si, Manual, FWD, Sedan")]
    pub summary: String,
}

/// The catalog's answer for one decoded VIN.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, ToSchema)]
pub struct Selection {
    /// The most specific entry the decode reaches.
    pub entry: Entry,
    /// The id of the year, make and model.
    #[schema(example = "2023_hyundai_kona")]
    pub vehicle_id: String,
    /// The submodel's id form, when the decode names exactly one.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub submodel_id: Option<String>,
    /// The engine's id form, when the decode settles on one.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub engine_id: Option<String>,
}

impl VinDecode {
    pub fn new(decoded: vin::Decoded, selection: Option<vehicles::Selection>) -> VinDecode {
        VinDecode {
            vin: decoded.vin,
            valid: decoded.valid,
            check_digit: CheckDigit {
                valid: decoded.check_digit.valid,
                expected: decoded.check_digit.expected,
                actual: decoded.check_digit.actual,
            },
            year: decoded.year,
            make: decoded.make,
            model: decoded.model,
            series: decoded.series,
            trim: decoded.trim,
            body: decoded.body,
            doors: decoded.doors,
            drivetrain: decoded.drivetrain,
            transmission: decoded.transmission,
            transmission_speeds: decoded.transmission_speeds,
            engine: decoded.engine.map(Engine::from),
            safety: decoded.safety.map(Safety::from),
            wheel_size_front: decoded.wheel_size_front,
            wheel_size_rear: decoded.wheel_size_rear,
            seats: decoded.seats,
            seat_rows: decoded.seat_rows,
            gvwr: decoded.gvwr,
            base_price_usd: decoded.base_price_usd,
            manufacturer: Manufacturer {
                wmi: decoded.manufacturer.wmi,
                name: decoded.manufacturer.name,
                country: decoded.manufacturer.country,
                vehicle_type: decoded.manufacturer.vehicle_type,
            },
            plant: Plant {
                code: decoded.plant.code,
                city: decoded.plant.city,
                state: decoded.plant.state,
                country: decoded.plant.country,
                company: decoded.plant.company,
            },
            warnings: decoded.warnings.into_iter().map(Warning::from).collect(),
            catalog: selection.map(Selection::from),
        }
    }
}

impl From<vin::Engine> for Engine {
    fn from(engine: vin::Engine) -> Engine {
        Engine {
            label: engine.label,
            model: engine.model,
            displacement_l: engine.displacement_l,
            displacement_cc: engine.displacement_cc,
            cylinders: engine.cylinders,
            configuration: engine.configuration,
            fuel: engine.fuel,
            turbo: engine.turbo,
            electrification: engine.electrification,
        }
    }
}

impl From<vin::Safety> for Safety {
    fn from(safety: vin::Safety) -> Safety {
        Safety {
            abs: safety.abs,
            esc: safety.esc,
            tpms: safety.tpms,
            adaptive_cruise: safety.adaptive_cruise,
            lane_keep: safety.lane_keep,
            lane_departure: safety.lane_departure,
            blind_spot: safety.blind_spot,
            forward_collision: safety.forward_collision,
            auto_brake: safety.auto_brake,
            backup_camera: safety.backup_camera,
            airbags_front: safety.airbags_front,
            airbags_side: safety.airbags_side,
            airbags_curtain: safety.airbags_curtain,
            airbags_knee: safety.airbags_knee,
            traction_control: safety.traction_control,
            dynamic_brake_support: safety.dynamic_brake_support,
            rear_cross_traffic: safety.rear_cross_traffic,
            park_assist: safety.park_assist,
            pedestrian_braking: safety.pedestrian_braking,
            lane_centering: safety.lane_centering,
        }
    }
}

impl From<vin::Warning> for Warning {
    fn from(warning: vin::Warning) -> Warning {
        // The code is written the way `wenmar-vin` serializes it, so a code
        // added there needs no change here.
        let code = serde_json::to_value(warning.code)
            .ok()
            .and_then(|value| value.as_str().map(str::to_owned))
            .unwrap_or_else(|| "unknown".to_owned());
        Warning {
            code,
            message: warning.message,
            suggestions: warning.suggestions,
        }
    }
}

impl From<vehicles::Make> for Make {
    fn from(make: vehicles::Make) -> Make {
        Make {
            id: make.id,
            name: make.name,
            popular: make.popular,
        }
    }
}

impl From<vehicles::Model> for Model {
    fn from(model: vehicles::Model) -> Model {
        Model {
            id: model.id,
            name: model.name,
            year_from: model.year_from,
            year_to: model.year_to,
        }
    }
}

impl From<vehicles::Submodel> for Submodel {
    fn from(submodel: vehicles::Submodel) -> Submodel {
        Submodel {
            id: submodel.id,
            name: submodel.name,
            kind: submodel.kind,
        }
    }
}

impl From<vehicles::EngineOption> for EngineOption {
    fn from(engine: vehicles::EngineOption) -> EngineOption {
        EngineOption {
            id: engine.id,
            label: engine.label,
            vin8: engine.vin8,
            preset: engine.preset,
        }
    }
}

impl From<vehicles::Entry> for Entry {
    fn from(entry: vehicles::Entry) -> Entry {
        Entry {
            id: entry.id,
            year: entry.year,
            make: entry.make,
            model: entry.model,
            submodel: entry.submodel,
            engine: entry.engine,
            transmission: entry.transmission,
            drive: entry.drive,
            body: entry.body,
            vehicle_types: entry.vehicle_types,
            summary: entry.summary,
        }
    }
}

impl From<vehicles::Selection> for Selection {
    fn from(selection: vehicles::Selection) -> Selection {
        Selection {
            entry: Entry::from(selection.entry),
            vehicle_id: selection.vehicle_id,
            submodel_id: selection.submodel_id,
            engine_id: selection.engine_id,
        }
    }
}
