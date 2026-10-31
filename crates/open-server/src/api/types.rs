//! The JSON the API sends, as types of this crate.
//!
//! `wenmar-vin` and `wenmar-vehicles` have their own result types. They are
//! copied field by field here for two reasons: the OpenAPI description is
//! generated from these definitions, and the two libraries must not depend
//! on an OpenAPI crate. A test compares the JSON of both for every field.

use std::collections::BTreeMap;

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
    /// Whether the model year is settled. Position 10 repeats every 30
    /// years. For cars, multipurpose vehicles and light trucks position 7
    /// settles which cycle; for other vehicles the year is the one the data
    /// explains better, and it is not settled when both years have data.
    /// Check the year on the vehicle's label when this is `false`.
    pub year_conclusive: bool,
    /// The drive in one of five words. Left out when NHTSA's value fits
    /// none of them, such as `6x4`; `drivetrain` still has it.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub drive: Option<Drive>,
    /// The body in one word. `body` has NHTSA's words.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub body_style: Option<BodyStyle>,
    /// The generation of the model this model year belongs to, from
    /// corgi-rs's table. Left out for a model the table does not list.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub generation: Option<Generation>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub series2: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub trim2: Option<String>,
    /// Cab type of a truck, as NHTSA words it.
    #[serde(skip_serializing_if = "Option::is_none")]
    #[schema(example = "Crew/Super Crew/Crew Max")]
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
    #[serde(skip_serializing_if = "Option::is_none")]
    #[schema(example = "Left-Hand Drive (LHD)")]
    pub steering: Option<String>,
    /// Service brakes, such as `Hydraulic` or `Air`.
    #[serde(skip_serializing_if = "Option::is_none")]
    #[schema(example = "Hydraulic")]
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
    /// Every attribute the decode resolved, by NHTSA's element code as
    /// NHTSA's own API names it, with where it came from. The fields above
    /// are taken from these; this also holds the ones they leave out.
    #[serde(skip_serializing_if = "BTreeMap::is_empty")]
    pub attributes: BTreeMap<String, Attribute>,
    /// The catalog entry this VIN reaches, for prefilling a vehicle form.
    /// Left out when the catalog cannot place the vehicle.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub catalog: Option<Selection>,
    /// How many recall campaigns may apply to the model year. Only in a
    /// batch asked with `include=recalls`, and only when the VIN reaches a
    /// model year of the catalog.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub recalls: Option<VinRecallCount>,
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
    /// Brake horsepower, or the lower end of a range.
    #[serde(skip_serializing_if = "Option::is_none")]
    #[schema(example = 147.0)]
    pub hp: Option<f64>,
    /// The upper end of the horsepower range.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub hp_to: Option<f64>,
    /// Power in kilowatts, as the manufacturer reported it.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub kw: Option<f64>,
    /// Who made the engine.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub manufacturer: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[schema(example = "Dual Overhead Cam (DOHC)")]
    pub valve_train: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[schema(example = "Stoichiometric Gasoline Direct Injection (SGDI)")]
    pub fuel_injection: Option<String>,
    /// Such as `Water` or `Air`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cooling: Option<String>,
    /// The second fuel of a flexible-fuel or bi-fuel engine.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub fuel_secondary: Option<String>,
    /// Two- or four-stroke.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stroke_cycles: Option<u8>,
}

/// The drive, in the words shops use.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, ToSchema)]
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

/// The body, in one word. `suv` takes crossovers; `pickup` takes sport
/// utility trucks; `van` takes cargo, passenger and step vans; `incomplete`
/// is a chassis another company finishes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum BodyStyle {
    Sedan,
    Coupe,
    Convertible,
    Hatchback,
    Wagon,
    Suv,
    Minivan,
    Van,
    Pickup,
    Truck,
    Bus,
    Motorhome,
    Incomplete,
    Motorcycle,
    OffRoad,
    LowSpeed,
    Trailer,
    Other,
}

/// One generation of a model: a run of model years between redesigns.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, ToSchema)]
pub struct Generation {
    /// How the generation is known.
    #[schema(example = "8th generation")]
    pub name: String,
    /// Its number, when it has one that is commonly used.
    #[serde(skip_serializing_if = "Option::is_none")]
    #[schema(example = 8)]
    pub ordinal: Option<u8>,
    /// The maker's platform or chassis code, when there is one.
    #[serde(skip_serializing_if = "Option::is_none")]
    #[schema(example = "CP/CS")]
    pub code: Option<String>,
    #[schema(example = 2008)]
    pub year_from: u16,
    /// Left out while the generation is still made.
    #[serde(skip_serializing_if = "Option::is_none")]
    #[schema(example = 2012)]
    pub year_to: Option<u16>,
}

impl From<vin::Generation> for Generation {
    fn from(generation: vin::Generation) -> Generation {
        Generation {
            name: generation.name,
            ordinal: generation.ordinal,
            code: generation.code,
            year_from: generation.year_from,
            year_to: generation.year_to,
        }
    }
}

/// Battery and charging, as the manufacturer reported them.
#[derive(Debug, Clone, PartialEq, Serialize, ToSchema)]
pub struct Electric {
    #[serde(skip_serializing_if = "Option::is_none")]
    #[schema(example = "Lithium-Ion/Li-Ion")]
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
    #[serde(skip_serializing_if = "Option::is_none")]
    pub drive_unit: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[schema(example = "Level 2")]
    pub charger_level: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub charger_power_kw: Option<f64>,
}

/// Where an attribute's value came from.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, ToSchema)]
#[serde(rename_all = "snake_case")]
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
#[derive(Debug, Clone, PartialEq, Eq, Serialize, ToSchema)]
pub struct Attribute {
    /// NHTSA's name for it.
    #[schema(example = "Engine Brake (hp) From")]
    pub name: String,
    /// Its value as NHTSA words it. Notes that came more than once are
    /// joined with `; `.
    #[schema(example = "147")]
    pub value: String,
    pub source: AttributeSource,
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
    /// A key that must be programmed to the vehicle.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub keyless_ignition: Option<String>,
    /// Such as `LED` or `Halogen`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub headlamp_light_source: Option<String>,
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
    #[serde(skip_serializing_if = "Option::is_none")]
    pub seat_belts: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub pretensioner: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub airbags_seat_cushion: Option<String>,
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
            year_conclusive: decoded.year_conclusive,
            drive: decoded.drive.and_then(Drive::from_vin),
            body_style: decoded.body_style.and_then(BodyStyle::from_vin),
            generation: decoded.generation.map(Generation::from),
            series2: decoded.series2,
            trim2: decoded.trim2,
            cab_type: decoded.cab_type,
            bed_type: decoded.bed_type,
            bed_length_in: decoded.bed_length_in,
            curb_weight_lb: decoded.curb_weight_lb,
            gvwr_to: decoded.gvwr_to,
            wheelbase_in: decoded.wheelbase_in,
            wheelbase_to_in: decoded.wheelbase_to_in,
            track_width_in: decoded.track_width_in,
            wheels: decoded.wheels,
            axles: decoded.axles,
            axle_configuration: decoded.axle_configuration,
            steering: decoded.steering,
            brake_system: decoded.brake_system,
            brake_system_description: decoded.brake_system_description,
            destination_market: decoded.destination_market,
            top_speed_mph: decoded.top_speed_mph,
            electric: decoded.electric.map(|electric| Electric {
                battery_type: electric.battery_type,
                battery_kwh: electric.battery_kwh,
                battery_kwh_to: electric.battery_kwh_to,
                battery_volts: electric.battery_volts,
                battery_volts_to: electric.battery_volts_to,
                battery_info: electric.battery_info,
                drive_unit: electric.drive_unit,
                charger_level: electric.charger_level,
                charger_power_kw: electric.charger_power_kw,
            }),
            notes: decoded.notes,
            attributes: decoded
                .attributes
                .into_iter()
                .map(|(code, attribute)| {
                    let source = match attribute.source {
                        vin::AttributeSource::EngineModel => AttributeSource::EngineModel,
                        vin::AttributeSource::SpecificationSheet => {
                            AttributeSource::SpecificationSheet
                        }
                        _ => AttributeSource::VinPattern,
                    };
                    let attribute = Attribute {
                        name: attribute.name,
                        value: attribute.value,
                        source,
                    };
                    (code, attribute)
                })
                .collect(),
            catalog: selection.map(Selection::from),
            recalls: None,
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
            hp: engine.hp,
            hp_to: engine.hp_to,
            kw: engine.kw,
            manufacturer: engine.manufacturer,
            valve_train: engine.valve_train,
            fuel_injection: engine.fuel_injection,
            cooling: engine.cooling,
            fuel_secondary: engine.fuel_secondary,
            stroke_cycles: engine.stroke_cycles,
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
            keyless_ignition: safety.keyless_ignition,
            headlamp_light_source: safety.headlamp_light_source,
            adaptive_driving_beam: safety.adaptive_driving_beam,
            adaptive_headlights: safety.adaptive_headlights,
            automatic_high_beams: safety.automatic_high_beams,
            daytime_running_lights: safety.daytime_running_lights,
            auto_reverse_windows: safety.auto_reverse_windows,
            crash_notification: safety.crash_notification,
            event_data_recorder: safety.event_data_recorder,
            pedestrian_alert_sound: safety.pedestrian_alert_sound,
            seat_belts: safety.seat_belts,
            pretensioner: safety.pretensioner,
            airbags_seat_cushion: safety.airbags_seat_cushion,
        }
    }
}

impl Drive {
    /// The library's drive. `None` only for a kind the library adds later
    /// and this server does not yet know.
    fn from_vin(drive: vin::Drive) -> Option<Drive> {
        Some(match drive {
            vin::Drive::Fwd => Drive::Fwd,
            vin::Drive::Rwd => Drive::Rwd,
            vin::Drive::Awd => Drive::Awd,
            vin::Drive::FourWd => Drive::FourWd,
            vin::Drive::TwoWd => Drive::TwoWd,
            _ => return None,
        })
    }
}

impl BodyStyle {
    /// The library's body style, as [`Drive::from_vin`] treats drives.
    fn from_vin(style: vin::BodyStyle) -> Option<BodyStyle> {
        use vin::BodyStyle as Vin;
        Some(match style {
            Vin::Sedan => BodyStyle::Sedan,
            Vin::Coupe => BodyStyle::Coupe,
            Vin::Convertible => BodyStyle::Convertible,
            Vin::Hatchback => BodyStyle::Hatchback,
            Vin::Wagon => BodyStyle::Wagon,
            Vin::Suv => BodyStyle::Suv,
            Vin::Minivan => BodyStyle::Minivan,
            Vin::Van => BodyStyle::Van,
            Vin::Pickup => BodyStyle::Pickup,
            Vin::Truck => BodyStyle::Truck,
            Vin::Bus => BodyStyle::Bus,
            Vin::Motorhome => BodyStyle::Motorhome,
            Vin::Incomplete => BodyStyle::Incomplete,
            Vin::Motorcycle => BodyStyle::Motorcycle,
            Vin::OffRoad => BodyStyle::OffRoad,
            Vin::LowSpeed => BodyStyle::LowSpeed,
            Vin::Trailer => BodyStyle::Trailer,
            Vin::Other => BodyStyle::Other,
            _ => return None,
        })
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

/// One make, with links to the maker's own pages.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, ToSchema)]
pub struct MakeDetail {
    /// The make's id form.
    #[schema(example = "honda")]
    pub id: String,
    #[schema(example = "Honda")]
    pub name: String,
    /// Whether it is on the list of popular makes.
    pub popular: bool,
    /// `null` when no link has been recorded for the make.
    pub links: Option<MakeLinks>,
}

/// The maker's own pages, found by a person. An address that was not
/// found is `null`. They lead off this service and can move.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, ToSchema)]
pub struct MakeLinks {
    /// The day every link was last opened by a person.
    #[schema(example = "2026-10-12")]
    pub checked: String,
    /// Where an owner checks a VIN for open recalls.
    pub recall_lookup_url: Option<String>,
    pub manual_url: Option<String>,
    pub maintenance_url: Option<String>,
    /// The maker's service information site, where it sells access.
    pub service_info_url: Option<String>,
    pub parts_url: Option<String>,
    /// The maker's telephone line for customers.
    pub customer_line: Option<String>,
}

impl From<&crate::site::links::MakeLinks> for MakeLinks {
    fn from(links: &crate::site::links::MakeLinks) -> MakeLinks {
        let text = |value: Option<&'static str>| value.map(str::to_owned);
        MakeLinks {
            checked: links.checked.to_owned(),
            recall_lookup_url: text(links.recall_lookup_url),
            manual_url: text(links.manual_url),
            maintenance_url: text(links.maintenance_url),
            service_info_url: text(links.service_info_url),
            parts_url: text(links.parts_url),
            customer_line: text(links.customer_line),
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

// ----- recalls -----

/// Where to check one vehicle for open recalls. Neither government
/// publishes recalls by VIN as data, so no answer here can say a recall
/// applies to a VIN: these lookups can.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, ToSchema)]
pub struct RecallLookup {
    /// NHTSA's lookup by VIN, for the United States.
    #[schema(example = "https://www.nhtsa.gov/recalls")]
    pub nhtsa: String,
    /// Transport Canada's recall search, for Canada.
    pub tc: String,
    /// The maker's own lookup by VIN, where one is on file.
    pub maker: Option<String>,
}

/// The day each country's recall data was current. `null` for a country
/// this server holds no recalls for.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, ToSchema)]
pub struct RecallAsOf {
    #[schema(example = "2026-10-11")]
    pub us: Option<String>,
    #[schema(example = "2026-10-11")]
    pub ca: Option<String>,
}

/// The model year an answer is about.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, ToSchema)]
pub struct RecallVehicle {
    /// The id of the year, make and model.
    #[schema(example = "2018_ford_f-150")]
    pub id: String,
    pub year: u16,
    #[schema(example = "Ford")]
    pub make: String,
    #[schema(example = "F-150")]
    pub model: String,
}

/// One recall campaign in a list.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, ToSchema)]
pub struct Recall {
    /// Who published it: `nhtsa` or `tc` (Transport Canada).
    #[schema(example = "nhtsa")]
    pub source: String,
    /// `us` or `ca`.
    #[schema(example = "us")]
    pub country: String,
    /// The publisher's campaign number.
    #[schema(example = "21V983000")]
    pub campaign: String,
    /// The day the campaign was reported.
    #[schema(example = "2021-12-16")]
    pub date: Option<String>,
    pub manufacturer: Option<String>,
    /// The component, as the source words it.
    #[schema(example = "LATCHES/LOCKS/LINKAGES:TAILGATE:LATCH")]
    pub component: Option<String>,
    /// What is wrong, in the source's words.
    pub summary: Option<String>,
    /// The manufacturer's own campaign number, which a dealer looks a
    /// recall up by.
    #[schema(example = "21S53")]
    pub mfr_campaign: Option<String>,
    /// Units potentially affected.
    pub units: Option<u32>,
    /// NHTSA's advisory not to drive the vehicle until it is repaired.
    /// `null` for a Canadian campaign: Transport Canada publishes no such
    /// field.
    pub do_not_drive: Option<bool>,
    /// NHTSA's advisory to park outside, away from buildings.
    pub park_outside: Option<bool>,
    /// Transport Canada's notification type, such as `Safety Mfr`.
    pub notification: Option<String>,
    /// In an answer about one model year: how the campaign's vehicle text
    /// reached that model. `exact`, `base` (the campaign names a series or
    /// trim of it) or `curated`. `null` in a list that is not about one
    /// model year.
    #[serde(rename = "match")]
    #[schema(example = "exact")]
    pub matched: Option<String>,
    /// In an answer about one model year: the model names the campaign
    /// lists that reached it.
    pub models: Vec<String>,
    /// In an answer about one model year: the first and last build date
    /// the campaign gives. Compare them with the date on the door sticker.
    pub built_from: Option<String>,
    pub built_to: Option<String>,
    /// The campaign's page on this site.
    #[schema(example = "https://open.wenmarpro.com/recalls/nhtsa/21V983000")]
    pub url: String,
}

/// Recalls that may apply to one model year.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, ToSchema)]
pub struct VehicleRecalls {
    pub vehicle: RecallVehicle,
    /// Always `true`. A campaign lists a year, make and model; only the
    /// manufacturer can say whether one vehicle is in it. Check the VIN at
    /// the addresses in `lookup`.
    pub may_apply: bool,
    pub as_of: RecallAsOf,
    /// Newest first. Empty when none was found as of the dates in `as_of`.
    pub recalls: Vec<Recall>,
    pub lookup: RecallLookup,
    /// The credit lines the data's licences ask for.
    pub attribution: Vec<String>,
}

/// What a VIN decoded to, as far as recalls need it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, ToSchema)]
pub struct RecallDecode {
    #[schema(example = "KM8K2CAB4PU001140")]
    pub vin: String,
    /// Whether the check digit is right.
    pub valid: bool,
    pub year: Option<u16>,
    pub make: Option<String>,
    pub model: Option<String>,
    pub trim: Option<String>,
}

/// Recalls that may apply to the year, make and model a VIN decodes to.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, ToSchema)]
pub struct VinRecalls {
    pub decode: RecallDecode,
    /// The model year the recalls are for. `null` when the VIN does not
    /// decode to a vehicle in the catalog; `recalls` is then empty and a
    /// warning says so.
    pub vehicle: Option<RecallVehicle>,
    /// Always `true`. This is not a lookup by VIN: the list is for the
    /// model year, and the VIN only names it.
    pub may_apply: bool,
    pub as_of: RecallAsOf,
    pub recalls: Vec<Recall>,
    pub lookup: RecallLookup,
    pub attribution: Vec<String>,
    /// The decode's warnings, and `no_catalog_vehicle` when the VIN names
    /// no model year to list recalls for.
    pub warnings: Vec<Warning>,
}

/// One vehicle a campaign lists.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, ToSchema)]
pub struct RecallCampaignVehicle {
    pub year: Option<u16>,
    /// The make and model as the source words them.
    #[schema(example = "CHEVROLET")]
    pub make: String,
    #[schema(example = "SILVERADO 1500")]
    pub model: String,
    /// The catalog's id for the model year, when the model matched.
    #[schema(example = "2019_chevrolet_silverado")]
    pub vehicle_id: Option<String>,
    /// `exact`, `base` or `curated`; `null` when the model is not in the
    /// catalog, as for most heavy trucks, buses and trailers.
    #[serde(rename = "match")]
    #[schema(example = "base")]
    pub matched: Option<String>,
    pub built_from: Option<String>,
    pub built_to: Option<String>,
}

/// Transport Canada's French text for a campaign.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, ToSchema)]
pub struct RecallFrench {
    pub component: Option<String>,
    pub notification: Option<String>,
    pub summary: Option<String>,
}

/// The newest report of remedy progress a manufacturer filed with NHTSA
/// for a campaign. The counts are totals for the whole campaign at the end
/// of the period named, which may be years past. They say nothing of one
/// vehicle: check a VIN at the addresses in `lookup`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, ToSchema)]
pub struct RecallCompletion {
    /// `quarterly` or `annual`.
    #[schema(example = "annual")]
    pub kind: String,
    /// The report's number among those of its kind, from 1.
    #[schema(example = 2)]
    pub report_number: u32,
    /// The first quarter the report covers, as a year and a quarter.
    #[schema(example = "2025-2")]
    pub period_from: String,
    /// The last quarter it covers. The counts are as of the end of it. A
    /// quarterly report has the same quarter in both.
    #[schema(example = "2026-1")]
    pub period_to: String,
    /// The day the manufacturer filed the report.
    #[schema(example = "2026-04-13")]
    pub submitted: Option<String>,
    /// Vehicles in the campaign.
    #[schema(example = 57591)]
    pub involved: u32,
    /// Of those, reported remedied.
    #[schema(example = 38917)]
    pub remedied: u32,
    /// NHTSA's "total unreachable", as reported.
    #[schema(example = 1011)]
    pub unreachable: u32,
    /// NHTSA's "total removed", as reported.
    #[schema(example = 23)]
    pub removed: u32,
    /// `remedied` as a share of `involved`, in whole percent, rounded
    /// down. `null` when `involved` is 0.
    #[schema(example = 67)]
    pub remedied_percent: Option<u8>,
    /// The day NHTSA's report files were current.
    #[schema(example = "2026-10-12")]
    pub as_of: Option<String>,
}

/// One campaign with every vehicle it lists.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, ToSchema)]
pub struct RecallCampaign {
    #[schema(example = "nhtsa")]
    pub source: String,
    #[schema(example = "us")]
    pub country: String,
    #[schema(example = "19V888000")]
    pub campaign: String,
    pub date: Option<String>,
    /// The day owners were notified.
    pub owner_notified: Option<String>,
    pub manufacturer: Option<String>,
    pub component: Option<String>,
    pub summary: Option<String>,
    pub consequence: Option<String>,
    pub remedy: Option<String>,
    pub notes: Option<String>,
    pub mfr_campaign: Option<String>,
    /// The manufacturer's part numbers, separated by commas.
    pub part_number: Option<String>,
    pub units: Option<u32>,
    /// How far the remedy had got when the manufacturer last reported to
    /// NHTSA. `null` for a Canadian campaign, for a campaign with no
    /// report on file, and on a server that holds no reports.
    pub completion: Option<RecallCompletion>,
    pub do_not_drive: Option<bool>,
    pub park_outside: Option<bool>,
    pub notification: Option<String>,
    /// `null` for a US campaign and for a Canadian one with no French text.
    pub french: Option<RecallFrench>,
    /// Every vehicle the campaign lists, by make, model and year.
    pub vehicles: Vec<RecallCampaignVehicle>,
    /// Always `true`: the campaign may apply to a vehicle it lists.
    pub may_apply: bool,
    /// The day this publisher's data was current.
    pub as_of: Option<String>,
    pub lookup: RecallLookup,
    pub attribution: Vec<String>,
    /// The campaign's page on this site.
    pub url: String,
}

/// One page of campaigns, newest first.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, ToSchema)]
pub struct RecallList {
    /// Always `true`: a campaign may apply to the vehicles it lists.
    pub may_apply: bool,
    pub as_of: RecallAsOf,
    pub recalls: Vec<Recall>,
    /// Hand this back as `cursor` for the next page. `null` on the last.
    pub next_cursor: Option<String>,
    pub lookup: RecallLookup,
    pub attribution: Vec<String>,
}

// ----- the overview -----

/// How many recall campaigns list a model year, for each country. `null`
/// for a country this server holds no recalls for; `0` when it holds them
/// and none lists the model year.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, ToSchema)]
pub struct RecallCount {
    #[schema(example = 7)]
    pub us: Option<u32>,
    #[schema(example = 5)]
    pub ca: Option<u32>,
}

/// What a batch decode says of recalls when `include=recalls` is asked:
/// how many campaigns may apply to the model year the VIN decodes to.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, ToSchema)]
pub struct VinRecallCount {
    /// Always `true`. The count is for the year, make and model, not for
    /// the VIN: check the VIN at the addresses in `lookup`.
    pub may_apply: bool,
    pub count: RecallCount,
    /// Whether any of them carries NHTSA's advisory not to drive.
    pub do_not_drive: bool,
    /// Whether any of them carries NHTSA's advisory to park outside.
    pub park_outside: bool,
    pub lookup: RecallLookup,
}

/// The recalls of a model year, in brief.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, ToSchema)]
pub struct OverviewRecalls {
    /// Always `true`. A campaign lists a year, make and model; only the
    /// manufacturer can say whether one vehicle is in it.
    pub may_apply: bool,
    pub count: RecallCount,
    pub as_of: RecallAsOf,
    /// Whether any campaign carries NHTSA's advisory not to drive.
    pub do_not_drive: bool,
    /// Whether any campaign carries NHTSA's advisory to park outside.
    pub park_outside: bool,
    /// The three newest campaigns. The whole list is at
    /// `/v1/vehicles/{id}/recalls`.
    pub newest: Vec<Recall>,
    pub lookup: RecallLookup,
    /// The model year's recall page on this site.
    #[schema(example = "https://open.wenmarpro.com/makes/ford/f-150/2018/recalls")]
    pub url: String,
}

/// What the VIN of an overview decoded to.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, ToSchema)]
pub struct OverviewDecode {
    #[schema(example = "KM8K2CAB4PU001140")]
    pub vin: String,
    /// Whether the check digit, position 9, is right.
    pub valid: bool,
    pub year: Option<u16>,
    pub make: Option<String>,
    pub model: Option<String>,
    pub trim: Option<String>,
    /// The engine in short form, such as `2.0L`, when the VIN names one.
    #[schema(example = "2.0L")]
    pub engine: Option<String>,
    /// Whether the VIN settles on one engine of the catalog.
    pub engine_settled: bool,
    /// The most specific catalog entry the VIN reaches.
    #[schema(example = "2023_hyundai_kona_se")]
    pub entry_id: Option<String>,
}

/// Where to read more.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, ToSchema)]
pub struct OverviewLinks {
    /// The model year's page on this site. `null` when the VIN reaches no
    /// model year of the catalog.
    #[schema(example = "https://open.wenmarpro.com/makes/ford/f-150/2018")]
    pub page: Option<String>,
    /// The maker's own pages. `null` when none are recorded for the make.
    pub maker: Option<MakeLinks>,
}

/// Everything held for one model year, in one answer.
///
/// A block this server has no data for is `null`: `specs`, `fuel`,
/// `fluids`, `tires` and `labor` are `null` in every answer today, and
/// each is filled when its data arrives. A client must accept
/// an object there.
#[derive(Debug, Clone, PartialEq, Serialize, ToSchema)]
pub struct Overview {
    /// The model year. `null` when a VIN reaches no model year of the
    /// catalog; every block is then `null` too.
    pub vehicle: Option<RecallVehicle>,
    /// What the VIN decoded to. `null` when the overview was asked for by
    /// vehicle id.
    pub decode: Option<OverviewDecode>,
    /// The generation of the model the year belongs to, from corgi-rs's
    /// table, as `/v1/models/{make}/{model}` gives it. `null` for a model
    /// the table does not list, and when a VIN reaches no model year.
    pub generation: Option<Generation>,
    #[schema(value_type = Option<Object>)]
    pub specs: Option<serde_json::Value>,
    #[schema(value_type = Option<Object>)]
    pub fuel: Option<serde_json::Value>,
    #[schema(value_type = Option<Object>)]
    pub fluids: Option<serde_json::Value>,
    /// The original-equipment tire fitments a person recorded, as the
    /// `fitments` of `/v1/vehicles/{id}/tires`: every one, never narrowed.
    /// `null` on a server that holds none.
    #[schema(value_type = Option<Vec<TireFitment>>)]
    pub tires: Option<serde_json::Value>,
    /// `null` when this server holds no recalls. A count of `0` with a
    /// day in `as_of` means none was found.
    pub recalls: Option<OverviewRecalls>,
    /// NHTSA's crash test ratings for the model year, as
    /// `/v1/vehicles/{id}/safety` gives them. `null` when this server
    /// holds no ratings, and when a VIN reaches no model year.
    #[schema(value_type = Option<VehicleSafety>)]
    pub safety: Option<serde_json::Value>,
    #[schema(value_type = Option<Object>)]
    pub labor: Option<serde_json::Value>,
    /// The bulletins of the model year, in brief. `null` when this server
    /// holds no bulletins, and when a VIN reaches no model year.
    pub bulletins: Option<OverviewBulletins>,
    /// What owners reported to NHTSA, as counts. `null` when the server
    /// holds no complaints, and when a VIN reaches no model year.
    pub complaints: Option<OverviewComplaints>,
    pub links: OverviewLinks,
    /// The credit lines the data's licences ask for.
    pub attribution: Vec<String>,
    pub warnings: Vec<Warning>,
}

/// A make or a model: its id form and its name.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, ToSchema)]
pub struct NamedId {
    #[schema(example = "ford")]
    pub id: String,
    #[schema(example = "Ford")]
    pub name: String,
}

/// One model year of a model.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, ToSchema)]
pub struct ModelYearSummary {
    /// The vehicle id of the year, make and model.
    #[schema(example = "2018_ford_f-150")]
    pub id: String,
    pub year: u16,
    /// How many recall campaigns may apply. `null` when this server holds
    /// no recalls.
    pub recalls: Option<RecallCount>,
    /// How many service bulletins are on file. `null` until this server
    /// holds bulletins.
    pub bulletins: Option<u32>,
    /// The generation of the model the year belongs to, from corgi-rs's
    /// table. Left out for a model the table does not list.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub generation: Option<Generation>,
    /// The model year's page on this site.
    #[schema(example = "https://open.wenmarpro.com/makes/ford/f-150/2018")]
    pub url: String,
}

/// Every model year of one model, newest first.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, ToSchema)]
pub struct ModelYears {
    pub make: NamedId,
    pub model: NamedId,
    /// Always `true`: a recall count is of campaigns that may apply to the
    /// model year.
    pub may_apply: bool,
    pub as_of: RecallAsOf,
    pub years: Vec<ModelYearSummary>,
    pub lookup: RecallLookup,
    pub attribution: Vec<String>,
}

/// One campaign of the recent list, with the vehicles it lists.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, ToSchema)]
pub struct RecentRecall {
    pub recall: Recall,
    /// The vehicles the campaign lists: at most 200 of them. Every one is
    /// at `/v1/recalls/{source}/{campaign}`.
    pub vehicles: Vec<RecallCampaignVehicle>,
    /// How many vehicles the campaign lists in all.
    pub vehicles_listed: u32,
}

/// Campaigns reported on or after a day, newest first.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, ToSchema)]
pub struct RecentRecalls {
    /// Always `true`: a campaign may apply to the vehicles it lists.
    pub may_apply: bool,
    /// The day asked for.
    #[schema(example = "2026-09-01")]
    pub since: String,
    pub as_of: RecallAsOf,
    pub recalls: Vec<RecentRecall>,
    /// Hand it back as `cursor` for the next page. `null` on the last.
    pub next_cursor: Option<String>,
    pub lookup: RecallLookup,
    pub attribution: Vec<String>,
}

// ----- investigations -----

/// One NHTSA defect investigation. An investigation is not a finding that a
/// defect exists.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, ToSchema)]
pub struct Investigation {
    /// NHTSA's action number.
    #[schema(example = "EA23001")]
    pub action: String,
    /// The letters the number starts with.
    #[schema(example = "EA")]
    pub kind: String,
    /// The kind in words. `null` for a kind with no name on file.
    #[schema(example = "Engineering analysis")]
    pub kind_name: Option<String>,
    /// The day it was opened.
    #[schema(example = "2023-03-17")]
    pub opened: Option<String>,
    /// The day it was closed. `null` while it is open, and for some old
    /// investigations whose closing NHTSA's file does not record.
    pub closed: Option<String>,
    /// Whether it was opened in 2010 or later and has no day closed.
    pub open: bool,
    /// The manufacturers it names, separated by `; `.
    pub manufacturer: Option<String>,
    /// The components it names, as NHTSA words them, separated by `; `.
    pub component: Option<String>,
    /// NHTSA's one-line subject.
    #[schema(example = "High pressure fuel pump failure")]
    pub subject: Option<String>,
    /// NHTSA's summary. It can be several thousand characters.
    pub summary: Option<String>,
    /// The recall campaigns it led to, as NHTSA numbers them. Empty when it
    /// led to none, or to none yet.
    pub campaigns: Vec<String>,
    /// In an answer about one model year: how the investigation's vehicle
    /// text reached that model. `exact`, `base` (it names a series or trim
    /// of the model) or `curated`. `null` in an answer about a campaign.
    #[serde(rename = "match")]
    #[schema(example = "exact")]
    pub matched: Option<String>,
    /// In an answer about one model year: the model names the
    /// investigation lists that reached it.
    pub models: Vec<String>,
}

/// NHTSA investigations that list one model year.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, ToSchema)]
pub struct VehicleInvestigations {
    pub vehicle: RecallVehicle,
    /// The day NHTSA's investigation file was current.
    #[schema(example = "2026-10-12")]
    pub as_of: String,
    /// Newest first. Empty when none lists the model year.
    pub investigations: Vec<Investigation>,
    /// That an investigation is not a finding of a defect and is not about
    /// one vehicle. Show it wherever the list is shown.
    pub note: String,
    /// The credit lines the data's licence asks for.
    pub attribution: Vec<String>,
}

/// The NHTSA investigations that led to one recall campaign.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, ToSchema)]
pub struct RecallInvestigations {
    /// `nhtsa` or `tc`.
    #[schema(example = "nhtsa")]
    pub source: String,
    #[schema(example = "18V568000")]
    pub campaign: String,
    #[schema(example = "2026-10-12")]
    pub as_of: String,
    /// Oldest first. Empty when the campaign followed no investigation,
    /// and for every Transport Canada campaign.
    pub investigations: Vec<Investigation>,
    pub note: String,
    pub attribution: Vec<String>,
    /// The campaign's page on this site.
    pub url: String,
}

// ----- crash test ratings -----

/// The stars of one rating, each from 1 to 5. `null` where NHTSA gave
/// none: a test it did not run, or a rating older than the overall rating,
/// which began with model year 2011.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, ToSchema)]
pub struct SafetyStars {
    #[schema(example = 5)]
    pub overall: Option<u8>,
    /// The frontal crash test, overall.
    pub frontal: Option<u8>,
    pub frontal_driver: Option<u8>,
    pub frontal_passenger: Option<u8>,
    /// The side crash tests, overall.
    pub side: Option<u8>,
    /// The side barrier test: the driver, and the rear passenger.
    pub side_driver: Option<u8>,
    pub side_passenger: Option<u8>,
    pub side_barrier: Option<u8>,
    /// The front seat and the rear seat, over the barrier and pole tests.
    pub side_front_seat: Option<u8>,
    pub side_rear_seat: Option<u8>,
    pub side_pole: Option<u8>,
    pub rollover: Option<u8>,
}

/// The safety features NHTSA lists with a rating. A feature is `standard`,
/// `optional`, `both` (standard on some versions, optional on others) or
/// `no`. An evaluation is `meets`, `does_not_meet`, `standard_only`,
/// `upgrade_only` or `pending`. `null` where NHTSA's file does not say.
///
/// For the four driver-assistance features (forward collision warning,
/// lane departure warning, crash imminent braking, dynamic brake support)
/// NHTSA's file means two things by its years. Through model year 2022,
/// `standard`, `optional` and `both` say the feature is fitted and met
/// NHTSA's performance criteria, `no` says it is not fitted or did not
/// meet them, and there is no evaluation. From model year 2023 the value
/// says only whether the feature is fitted, and the evaluation says what
/// NHTSA's test found.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, ToSchema)]
pub struct SafetyFeatures {
    /// Electronic stability control.
    #[schema(example = "standard")]
    pub esc: Option<String>,
    pub forward_collision_warning: Option<String>,
    #[schema(example = "meets")]
    pub forward_collision_warning_evaluation: Option<String>,
    pub lane_departure_warning: Option<String>,
    pub lane_departure_warning_evaluation: Option<String>,
    /// Automatic emergency braking: the vehicle brakes by itself.
    pub crash_imminent_braking: Option<String>,
    pub crash_imminent_braking_evaluation: Option<String>,
    /// The vehicle adds to the driver's braking.
    pub dynamic_brake_support: Option<String>,
    pub dynamic_brake_support_evaluation: Option<String>,
    pub backup_camera: Option<String>,
    pub blind_spot_detection: Option<String>,
    /// Side air bags for the head, and for the torso.
    pub head_side_airbag: Option<String>,
    pub torso_side_airbag: Option<String>,
}

/// One rating NHTSA gave a vehicle of a model year.
#[derive(Debug, Clone, PartialEq, Serialize, ToSchema)]
pub struct SafetyRating {
    /// The vehicle NHTSA rated, in NHTSA's words, on one line.
    #[schema(example = "F-150 SUPER CREW, PU/CC, 4x4")]
    pub description: String,
    /// The model, body style, vehicle type and drive as NHTSA words them.
    #[schema(example = "F-150 SUPER CREW")]
    pub model: String,
    #[schema(example = "PU/CC")]
    pub body_style: Option<String>,
    #[schema(example = "TRUCK")]
    pub vehicle_type: Option<String>,
    #[schema(example = "4x4")]
    pub drive: Option<String>,
    /// 1 for the first release of a model year; 2 or 3 for a later one
    /// that NHTSA rated again.
    #[schema(example = 1)]
    pub release: u8,
    /// How NHTSA's model name reached this model: `exact`, `base` (NHTSA
    /// names a body or a version of it) or `curated`.
    #[serde(rename = "match")]
    #[schema(example = "base")]
    pub matched: String,
    pub stars: SafetyStars,
    /// The chance of a rollover in a single-vehicle crash, in percent.
    #[schema(example = 19.1)]
    pub rollover_risk_percent: Option<f64>,
    /// Whether the vehicle tipped up in NHTSA's dynamic test.
    pub rollover_tip: Option<bool>,
    /// The weight of the vehicle NHTSA tested. NHTSA gives pounds; the
    /// kilograms are worked out from them and rounded.
    #[schema(example = 2295)]
    pub curb_weight_kg: Option<u32>,
    #[schema(example = 5060)]
    pub curb_weight_lb: Option<u32>,
    pub features: SafetyFeatures,
    /// NHTSA's safety concerns and footnotes: what happened in a test, and
    /// which vehicles the rating does not cover.
    pub notes: Vec<String>,
}

/// NHTSA's crash test ratings for one model year.
#[derive(Debug, Clone, PartialEq, Serialize, ToSchema)]
pub struct VehicleSafety {
    pub vehicle: RecallVehicle,
    /// The day NHTSA's file was current.
    #[schema(example = "2026-08-07")]
    pub as_of: String,
    /// One entry for each vehicle of the model year NHTSA rated or lists,
    /// by NHTSA's model name, body style and drive. Empty when it lists
    /// none.
    pub ratings: Vec<SafetyRating>,
    /// Who rates the vehicles.
    #[schema(example = "NHTSA New Car Assessment Program")]
    pub source: String,
    /// NHTSA's own pages of ratings.
    #[schema(example = "https://www.nhtsa.gov/ratings")]
    pub source_url: String,
    /// The credit lines the data's licence asks for. NHTSA's asks for none.
    pub attribution: Vec<String>,
}

/// The day each country's fuel ratings were current. Null for a country
/// this server holds no ratings for.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, ToSchema)]
pub struct FuelAsOf {
    #[schema(example = "2026-09-29")]
    pub us: Option<String>,
    #[schema(example = "2026-10-06")]
    pub ca: Option<String>,
}

/// A catalog engine of the model year.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, ToSchema)]
pub struct EngineRef {
    /// The engine's id form, as in a vehicle id.
    #[schema(example = "2-7l-v6")]
    pub id: String,
    #[schema(example = "2.7L V6")]
    pub label: String,
    /// The characters in position 8 of a VIN that mean this engine.
    #[schema(example = "P")]
    pub vin8: Option<String>,
}

/// City, highway and combined consumption in litres per 100 km.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, ToSchema)]
pub struct Consumption {
    pub city: Option<f64>,
    pub highway: Option<f64>,
    pub combined: Option<f64>,
}

/// City, highway and combined economy in miles per US gallon.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, ToSchema)]
pub struct Mpg {
    pub city: Option<u16>,
    pub highway: Option<u16>,
    pub combined: Option<u16>,
}

/// One fuel consumption rating.
#[derive(Debug, Clone, PartialEq, Serialize, ToSchema)]
pub struct FuelRating {
    /// `epa` or `nrcan`.
    #[schema(example = "epa")]
    pub source: String,
    /// `us` or `ca`.
    #[schema(example = "us")]
    pub country: String,
    /// EPA's id for the rating. Null for one from Natural Resources Canada.
    #[schema(example = "39241")]
    pub source_id: Option<String>,
    /// How the model was found: `exact`, `base` or `curated`.
    #[serde(rename = "match")]
    #[schema(example = "exact")]
    pub matched: String,
    /// How far the rating was matched: `model` (the model year only),
    /// `engine` (one engine) or `submodel` (an engine and a trim).
    #[schema(example = "engine")]
    pub confidence: String,
    /// The catalog engine. Null unless `confidence` is `engine` or `submodel`.
    pub engine: Option<EngineRef>,
    /// The source's own name for the vehicle rated.
    #[schema(example = "F150 Pickup 2WD")]
    pub described_as: String,
    pub displacement_l: Option<f64>,
    pub cylinders: Option<u8>,
    #[schema(example = "Automatic (S10)")]
    pub transmission: Option<String>,
    #[schema(example = "Rear-Wheel Drive")]
    pub drive: Option<String>,
    /// The fuel the figures are for, which is the recommended grade.
    #[schema(example = "Regular Gasoline")]
    pub fuel_grade: String,
    /// A second fuel the vehicle takes, such as `E85` or `Electricity`.
    pub second_fuel: Option<String>,
    /// Null when the fuel is not sold by the litre.
    pub l_100km: Option<Consumption>,
    pub mpg: Option<Mpg>,
    /// Combined electricity use.
    pub kwh_100km: Option<f64>,
    /// On a full battery.
    pub range_km: Option<u32>,
    pub range_mi: Option<u32>,
    /// Tailpipe carbon dioxide, combined.
    pub co2_g_km: Option<u32>,
}

/// The fuel ratings of one model year.
#[derive(Debug, Clone, PartialEq, Serialize, ToSchema)]
pub struct VehicleFuel {
    pub vehicle: RecallVehicle,
    pub as_of: FuelAsOf,
    pub ratings: Vec<FuelRating>,
    pub attribution: Vec<String>,
}

/// The fuel ratings for the vehicle a VIN decodes to.
#[derive(Debug, Clone, PartialEq, Serialize, ToSchema)]
pub struct VinFuel {
    pub decode: RecallDecode,
    /// Null when the VIN decodes to no vehicle in the catalog.
    pub vehicle: Option<RecallVehicle>,
    /// The engine the decode settled. Null when it settled none.
    pub engine: Option<EngineRef>,
    /// Whether `ratings` is narrowed to `engine`.
    pub engine_settled: bool,
    pub as_of: FuelAsOf,
    pub ratings: Vec<FuelRating>,
    pub attribution: Vec<String>,
    pub warnings: Vec<Warning>,
}

/// An emission standard a vehicle was certified to.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, ToSchema)]
pub struct SpecEmission {
    /// EPA's sales area code, a number. EPA's column list does not say
    /// what each number means, so no name is given.
    #[schema(example = 3)]
    pub sales_area: u8,
    #[schema(example = "T3B70")]
    pub standard: String,
    #[schema(example = "Federal Tier 3 Bin 70")]
    pub standard_text: Option<String>,
    /// EPA's smog rating, 1 (worst) to 10 (best).
    pub smog_score: Option<u8>,
    /// The engine family (test group).
    #[schema(example = "JFMXT02.73JK")]
    pub engine_family: String,
}

/// The specifications that come with one EPA rating.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, ToSchema)]
pub struct Spec {
    #[schema(example = "epa")]
    pub source: String,
    #[schema(example = "39241")]
    pub source_id: String,
    /// `model`, `engine` or `submodel`, as on a fuel rating.
    pub confidence: String,
    pub engine: Option<EngineRef>,
    #[schema(example = "F150 Pickup 2WD")]
    pub described_as: String,
    pub transmission: Option<String>,
    #[schema(example = "Semi-Automatic")]
    pub transmission_type: Option<String>,
    pub gears: Option<u8>,
    pub drive: Option<String>,
    #[schema(example = "Regular Gasoline")]
    pub fuel_grade: String,
    /// Whether the grade is recommended or required, in EPA's words.
    #[schema(example = "Gasoline (Premium Unleaded Required)")]
    pub fuel_usage: Option<String>,
    #[schema(example = "Standard Pickup Trucks 2WD")]
    pub size_class: String,
    pub turbo: bool,
    pub supercharger: bool,
    pub start_stop: Option<bool>,
    /// EPA's kind of alternative powertrain: `Hybrid`, `Plug-in Hybrid`, `EV`, `FFV`, `Diesel`.
    pub electrification: Option<String>,
    pub ev_motor: Option<String>,
    pub cylinder_deactivation: Option<bool>,
    /// The maker's name for it, such as `Active Fuel Management`.
    pub cylinder_deactivation_name: Option<String>,
    pub variable_valve_timing: Option<bool>,
    pub variable_valve_lift: Option<bool>,
    /// Per cylinder.
    pub intake_valves: Option<u8>,
    pub exhaust_valves: Option<u8>,
    pub passenger_volume_l: Option<u32>,
    pub passenger_volume_ft3: Option<u16>,
    pub luggage_volume_l: Option<u32>,
    pub luggage_volume_ft3: Option<u16>,
    #[schema(example = "Lithium Ion")]
    pub battery_type: Option<String>,
    pub battery_voltage: Option<u16>,
    /// Rated motor power in kW, as EPA's file writes it: `135`, `8 and 30`.
    pub motor_kw: Option<String>,
    pub emission_standards: Vec<SpecEmission>,
}

/// The specifications of one model year.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, ToSchema)]
pub struct VehicleSpecs {
    pub vehicle: RecallVehicle,
    #[schema(example = "2026-09-29")]
    pub as_of: Option<String>,
    pub specs: Vec<Spec>,
    pub attribution: Vec<String>,
}

/// The specifications for the vehicle a VIN decodes to.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, ToSchema)]
pub struct VinSpecs {
    pub decode: RecallDecode,
    pub vehicle: Option<RecallVehicle>,
    pub engine: Option<EngineRef>,
    /// Whether `specs` is narrowed to `engine`.
    pub engine_settled: bool,
    pub as_of: Option<String>,
    pub specs: Vec<Spec>,
    pub attribution: Vec<String>,
    pub warnings: Vec<Warning>,
}

/// The engine oil viscosity grade of one engine.
#[derive(Debug, Clone, PartialEq, Serialize, ToSchema)]
pub struct EngineOil {
    /// `engine` when the grade is for one catalog engine; `model` when
    /// only the model year is known, and then `engine` is null.
    pub confidence: String,
    pub engine: Option<EngineRef>,
    pub displacement_l: Option<f64>,
    pub cylinders: Option<u8>,
    /// Standard grades. More than one when the maker listed more than one.
    #[schema(example = json!(["5W-30"]))]
    pub grades: Vec<String>,
    /// The maker's own spellings the grades were read from.
    #[schema(example = json!(["5W30 GF-5"]))]
    pub as_written: Vec<String>,
    /// Always `EPA fuel economy certification`: the grade the maker
    /// certified with, not a reading of the owner's manual.
    pub source: String,
}

/// One fluid of a model year that a person read from the maker's own
/// document. Which fields are filled depends on the list it is in:
/// `grade`, the capacity and `with_filter` are for engine oil, and the
/// charge is for refrigerant.
#[derive(Debug, Clone, PartialEq, Serialize, ToSchema)]
pub struct FluidRecord {
    /// The catalog engine the record is for. `null` when it is for every
    /// engine of the model year.
    pub engine: Option<EngineRef>,
    /// Which transmission or system the record is for, when it says.
    pub applies_to: Option<String>,
    /// Engine oil viscosity, as the document gives it.
    pub grade: Option<String>,
    /// The maker's specification, or the type of fluid.
    pub specification: Option<String>,
    pub capacity_l: Option<f64>,
    /// US liquid quarts.
    pub capacity_qt: Option<f64>,
    /// The unit the document printed, `l` or `qt`. The other figure was
    /// worked out from it.
    pub capacity_written_in: Option<String>,
    /// Whether the capacity includes the oil filter.
    pub with_filter: Option<bool>,
    /// Refrigerant charge, in grams.
    pub charge_g: Option<i64>,
    /// Refrigerant charge, in ounces.
    pub charge_oz: Option<f64>,
    /// `g`, `kg`, `oz` or `lb`.
    pub charge_written_in: Option<String>,
    /// `us` or `ca` when the document is for one country only.
    pub market: Option<String>,
    /// The document the record was read from.
    pub source: String,
    /// The day it was read.
    #[schema(example = "2026-10-12")]
    pub checked: String,
}

/// The fluids of one model year.
///
/// `engine_oil` is the grade EPA records. The five lists after
/// `checked_as_of` are records a person read from the maker's own
/// documents, each with its `source` and the day it was `checked`; they
/// are empty for a model year nobody has recorded, and on a server that
/// does not hold the records `checked_as_of` is `null`.
#[derive(Debug, Clone, PartialEq, Serialize, ToSchema)]
pub struct VehicleFluids {
    pub vehicle: RecallVehicle,
    /// The day EPA's oil grades were current. `null` on a server without
    /// them.
    #[schema(example = "2026-09-29")]
    pub as_of: Option<String>,
    pub engine_oil: Vec<EngineOil>,
    /// The day of the newest hand-read record the server holds, for any
    /// vehicle. `null` on a server without them.
    #[schema(example = "2026-10-12")]
    pub checked_as_of: Option<String>,
    pub engine_oil_checked: Vec<FluidRecord>,
    pub transmission: Vec<FluidRecord>,
    pub coolant: Vec<FluidRecord>,
    pub refrigerant: Vec<FluidRecord>,
    pub brake: Vec<FluidRecord>,
    pub attribution: Vec<String>,
}

/// The fluids for the vehicle a VIN decodes to.
#[derive(Debug, Clone, PartialEq, Serialize, ToSchema)]
pub struct VinFluids {
    pub decode: RecallDecode,
    pub vehicle: Option<RecallVehicle>,
    pub engine: Option<EngineRef>,
    /// Whether `engine_oil` is narrowed to `engine`.
    pub engine_settled: bool,
    pub as_of: Option<String>,
    pub engine_oil: Vec<EngineOil>,
    pub checked_as_of: Option<String>,
    /// Narrowed as `engine_oil` is. A record for every engine is kept.
    pub engine_oil_checked: Vec<FluidRecord>,
    pub transmission: Vec<FluidRecord>,
    pub coolant: Vec<FluidRecord>,
    pub refrigerant: Vec<FluidRecord>,
    pub brake: Vec<FluidRecord>,
    pub attribution: Vec<String>,
    pub warnings: Vec<Warning>,
}

/// A figure for the front and for the rear.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, ToSchema)]
pub struct FrontRear {
    pub front: Option<i64>,
    pub rear: Option<i64>,
}

/// One tire size the maker fitted to a model year, read by a person from
/// the door placard or the owner's manual.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, ToSchema)]
pub struct TireFitment {
    /// The size as printed. The front size when `size_rear` is given.
    pub size: String,
    /// The rear size, only when it differs from the front.
    pub size_rear: Option<String>,
    /// The trims the document ties the size to, as it names them. Empty
    /// when it names none.
    pub trims: Vec<String>,
    /// Cold pressure in kilopascals. `null` when none is recorded.
    pub pressure_kpa: Option<FrontRear>,
    /// The same in pounds per square inch.
    pub pressure_psi: Option<FrontRear>,
    /// The unit the document printed: `kpa`, `bar` or `psi`. The other
    /// figures were worked out from it.
    pub pressure_written_in: Option<String>,
    pub lug_torque_nm: Option<i64>,
    pub lug_torque_lbft: Option<i64>,
    /// `nm` or `lbft`.
    pub lug_torque_written_in: Option<String>,
    pub bolt_pattern: Option<String>,
    pub market: Option<String>,
    pub source: String,
    #[schema(example = "2026-10-12")]
    pub checked: String,
}

/// Every original-equipment tire fitment on record for one model year.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, ToSchema)]
pub struct VehicleTires {
    pub vehicle: RecallVehicle,
    /// The day of the newest record the server holds, for any vehicle.
    #[schema(example = "2026-10-12")]
    pub as_of: Option<String>,
    /// One entry per size, in the order of the record file. Empty for a
    /// model year nobody has recorded. A size is never picked for the
    /// caller.
    pub fitments: Vec<TireFitment>,
    pub attribution: Vec<String>,
}

/// The tire fitments of the model year a VIN decodes to.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, ToSchema)]
pub struct VinTires {
    pub decode: RecallDecode,
    pub vehicle: Option<RecallVehicle>,
    pub as_of: Option<String>,
    /// Every fitment of the model year. A VIN does not say which wheels a
    /// vehicle left the factory with, so the list is never narrowed.
    pub fitments: Vec<TireFitment>,
    pub attribution: Vec<String>,
    pub warnings: Vec<Warning>,
}

/// One line of a parts-ordering card.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, ToSchema)]
pub struct PartsCardLine {
    #[schema(example = "Engine")]
    pub label: String,
    #[schema(example = "2.0L")]
    pub value: String,
    /// Where the line came from.
    #[schema(example = "VIN decode (NHTSA vPIC)")]
    pub source: String,
}

/// What a parts counter asks for about one vehicle.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, ToSchema)]
pub struct PartsCard {
    pub vin: String,
    /// Null when the VIN decodes to no vehicle in the catalog.
    pub vehicle: Option<RecallVehicle>,
    /// Whether the decode settled one catalog engine. The catalog engine
    /// and emission standard lines are there only when it did.
    pub engine_settled: bool,
    pub lines: Vec<PartsCardLine>,
    /// The lines as plain text, one `Label: value` each.
    pub text: String,
    pub attribution: Vec<String>,
    pub warnings: Vec<Warning>,
}

/// Where NHTSA's copy of a bulletin's document is expected to be.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, ToSchema)]
pub struct BulletinDocument {
    /// The likelier address of NHTSA's copy (a PDF).
    #[schema(example = "https://static.nhtsa.gov/odi/tsbs/2018/MC-10152456-0001.pdf")]
    pub url: String,
    /// The address to try when `url` answers "not found": NHTSA files
    /// documents under one of two endings, and the data does not say
    /// which.
    #[schema(example = "https://static.nhtsa.gov/odi/tsbs/2018/MC-10152456-9999.pdf")]
    pub alternate_url: String,
    /// Always `false`. The addresses are built from a rule that held for
    /// a sample of documents; this API has not asked NHTSA for this one.
    pub confirmed: bool,
}

/// One manufacturer communication in a list: a service bulletin, a
/// service campaign, a warranty extension or an over-the-air update.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, ToSchema)]
pub struct Bulletin {
    /// NHTSA's id number for the communication.
    #[schema(example = 10152456)]
    pub nhtsa_id: i64,
    /// `service_bulletin`, `service_campaign`, `warranty_extension`,
    /// `over_the_air`, `emissions` or `other`.
    #[serde(rename = "type")]
    #[schema(example = "service_bulletin")]
    pub kind: String,
    /// The type in words.
    #[schema(example = "Service bulletin")]
    pub type_name: String,
    /// The day NHTSA published it. Lists are ordered by it.
    #[schema(example = "2019-01-07")]
    pub date_added: String,
    /// The manufacturer's own date.
    #[schema(example = "2018-11-02")]
    pub date: Option<String>,
    /// The manufacturer's own number for the document, which a dealer
    /// looks it up by.
    #[schema(example = "TSB 18-2324")]
    pub document_id: Option<String>,
    /// The manufacturer's campaign id or software version.
    pub mfr_campaign: Option<String>,
    /// NHTSA's components, in its words, separated by `; `.
    #[schema(example = "ENGINE AND ENGINE COOLING:EXHAUST SYSTEM")]
    pub component: Option<String>,
    /// NHTSA's summary of the communication. The document itself is the
    /// manufacturer's and is not here.
    pub summary: Option<String>,
    /// Where NHTSA's copy of the document is expected to be. `null` when
    /// no address can be built for it: a communication with no
    /// manufacturer's date, or one NHTSA received before July 2017.
    pub document: Option<BulletinDocument>,
    /// The bulletin's page on this site.
    #[schema(example = "https://open.wenmarpro.com/bulletins/10152456")]
    pub url: String,
}

/// How many bulletins have one type, component, code or topic.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, ToSchema)]
pub struct BulletinCount {
    /// What to pass as `type`, `component`, `code` or `topic`.
    #[schema(example = "brake-pad")]
    pub id: String,
    #[schema(example = "Brake pads")]
    pub name: String,
    pub count: u64,
}

/// What the bulletins of one model year are, in numbers. They count every
/// bulletin of the model year, whatever the list is narrowed by.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, ToSchema)]
pub struct BulletinCounts {
    pub count: u64,
    /// Every type with at least one bulletin.
    pub by_type: Vec<BulletinCount>,
    /// Every component with at least one, commonest first. A bulletin
    /// about a part is counted under the part and under its system.
    pub components: Vec<BulletinCount>,
    /// The trouble codes the summaries name most, up to 20.
    pub codes: Vec<BulletinCount>,
    /// `maintenance`, when a bulletin is about a schedule or an interval.
    pub topics: Vec<BulletinCount>,
}

/// The bulletins of a model year, in brief, in an overview.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, ToSchema)]
pub struct OverviewBulletins {
    /// Every manufacturer communication that lists the model year.
    pub count: u64,
    /// Every type with at least one, as `counts.by_type` of
    /// `/v1/vehicles/{id}/bulletins`.
    pub by_type: Vec<BulletinCount>,
    /// The day the bulletins were current.
    pub as_of: Option<String>,
    /// The three newest, as `/v1/vehicles/{id}/bulletins` lists them.
    pub newest: Vec<Bulletin>,
    /// The model year's bulletin page on this site.
    #[schema(example = "https://open.wenmarpro.com/makes/ford/f-150/2018/bulletins")]
    pub url: String,
}

/// Bulletins that list one model year.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, ToSchema)]
pub struct VehicleBulletins {
    pub vehicle: RecallVehicle,
    /// The day the bulletins were current.
    #[schema(example = "2026-10-11")]
    pub as_of: Option<String>,
    pub counts: BulletinCounts,
    /// One page, newest first.
    pub bulletins: Vec<Bulletin>,
    /// Pass it back as `cursor` for the next page. `null` on the last.
    pub next_cursor: Option<String>,
    /// What a bulletin is and is not.
    pub note: String,
    /// The credit lines the data's licences ask for.
    pub attribution: Vec<String>,
}

/// Bulletins that list the year, make and model a VIN decodes to.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, ToSchema)]
pub struct VinBulletins {
    pub decode: RecallDecode,
    /// `null` when the VIN decodes to no vehicle in the catalog.
    pub vehicle: Option<RecallVehicle>,
    pub as_of: Option<String>,
    pub counts: BulletinCounts,
    pub bulletins: Vec<Bulletin>,
    pub next_cursor: Option<String>,
    pub note: String,
    pub attribution: Vec<String>,
    pub warnings: Vec<Warning>,
}

/// A component a bulletin is about.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, ToSchema)]
pub struct BulletinComponent {
    #[schema(example = "brake-pad")]
    pub id: String,
    #[schema(example = "Brake pads")]
    pub name: String,
    /// The system a part belongs to. `null` for a system.
    #[schema(example = "brakes")]
    pub parent: Option<String>,
}

/// The model years of one name a bulletin lists.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, ToSchema)]
pub struct BulletinVehicle {
    /// The make and model as the source words them.
    #[schema(example = "FORD")]
    pub make: String,
    #[schema(example = "F-150")]
    pub model: String,
    /// The first and last model year of a run, inclusive. Both `null`
    /// when the bulletin gave no model year.
    pub year_from: Option<u16>,
    pub year_to: Option<u16>,
    /// `exact`, `base` or `curated`; `null` when the model is not in the
    /// catalog for these years.
    #[serde(rename = "match")]
    pub matched: Option<String>,
    /// The vehicle id of each model year in the run, when it matched.
    pub vehicle_ids: Vec<String>,
}

/// One bulletin with every vehicle it lists.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, ToSchema)]
pub struct BulletinDetail {
    #[schema(example = 10152456)]
    pub nhtsa_id: i64,
    #[serde(rename = "type")]
    #[schema(example = "service_bulletin")]
    pub kind: String,
    pub type_name: String,
    pub date_added: String,
    pub date: Option<String>,
    pub document_id: Option<String>,
    pub mfr_campaign: Option<String>,
    /// The number of the bulletin this one replaces. NHTSA no longer
    /// fills it.
    pub replaces: Option<String>,
    pub component: Option<String>,
    /// The manufacturer's own words for the system.
    pub mfr_system: Option<String>,
    pub summary: Option<String>,
    pub document: Option<BulletinDocument>,
    pub url: String,
    /// The components it is about, systems first.
    pub components: Vec<BulletinComponent>,
    /// The trouble codes the summary names.
    pub codes: Vec<String>,
    pub topics: Vec<String>,
    pub vehicles: Vec<BulletinVehicle>,
    pub as_of: Option<String>,
    pub note: String,
    pub attribution: Vec<String>,
}

/// One page of bulletins, newest first.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, ToSchema)]
pub struct BulletinList {
    pub as_of: Option<String>,
    pub bulletins: Vec<Bulletin>,
    pub next_cursor: Option<String>,
    pub note: String,
    pub attribution: Vec<String>,
}

/// The complaints of a model year that name one component.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, ToSchema)]
pub struct ComponentComplaints {
    /// A component id, such as `steering` or `brake-pad`.
    #[schema(example = "steering")]
    pub id: String,
    #[schema(example = "Steering")]
    pub name: String,
    /// The system a part belongs to. `null` for a system.
    pub parent: Option<String>,
    pub complaints: u32,
    /// Complaints that report a crash.
    pub crashes: u32,
    /// Complaints that report a fire.
    pub fires: u32,
    /// Complaints that report at least one person injured.
    pub injuries: u32,
    /// The median of the mileages owners gave, in kilometres. `null` when
    /// fewer than five gave one.
    pub median_km: Option<u32>,
    /// The same in miles, as NHTSA records it.
    pub median_miles: Option<u32>,
}

/// What owners reported to NHTSA about one model year, as counts.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, ToSchema)]
pub struct VehicleComplaints {
    pub vehicle: RecallVehicle,
    /// The day the complaints were current.
    #[schema(example = "2026-10-11")]
    pub as_of: Option<String>,
    /// Always `false`. A complaint is an owner's report. Nobody has
    /// verified it, and a count of complaints is not a finding that
    /// anything is wrong with the vehicle.
    pub verified: bool,
    /// Every complaint about the model year. Not a rate: a model that
    /// sold more has more.
    pub complaints: u32,
    pub crashes: u32,
    pub fires: u32,
    pub injuries: u32,
    pub median_km: Option<u32>,
    pub median_miles: Option<u32>,
    /// The components with at least five complaints, most first, at most
    /// twelve. They do not add up to `complaints`: a complaint may name
    /// several, and a part is counted under its system too.
    pub components: Vec<ComponentComplaints>,
    /// What these counts are and are not.
    pub note: String,
    /// Where an owner files a complaint with NHTSA.
    pub report_url: String,
    pub attribution: Vec<String>,
}

/// The complaints of a model year, in brief, in an overview.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, ToSchema)]
pub struct OverviewComplaints {
    /// Always `false`, as in `VehicleComplaints`.
    pub verified: bool,
    pub complaints: u32,
    pub crashes: u32,
    pub fires: u32,
    pub injuries: u32,
    pub median_km: Option<u32>,
    pub median_miles: Option<u32>,
    /// The three most reported components.
    pub top: Vec<ComponentComplaints>,
    pub as_of: Option<String>,
    /// The section of the model year's page on this site.
    pub url: String,
}

// ----- trouble codes -----

/// A component a trouble code concerns.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, ToSchema)]
pub struct CodeComponent {
    #[schema(example = "oxygen-sensor")]
    pub id: String,
    #[schema(example = "Oxygen sensor")]
    pub name: String,
    /// The system a part belongs to. `null` for a system.
    #[schema(example = "exhaust-emissions")]
    pub parent: Option<String>,
}

/// A trouble code in a list.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, ToSchema)]
pub struct CodeLink {
    #[schema(example = "P0420")]
    pub code: String,
    /// Where the index lists it: a group (`powertrain`, `body`, `chassis`,
    /// `network`), or a group and an area of it.
    #[schema(example = "powertrain/emissions")]
    pub system: String,
    pub title: String,
    /// The code's page on this site.
    #[schema(example = "https://open.wenmarpro.com/codes/P0420")]
    pub url: String,
}

/// One generic trouble code, described in this project's own words.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, ToSchema)]
pub struct Code {
    #[schema(example = "P0420")]
    pub code: String,
    /// Always `true` here: only codes every maker uses the same way are
    /// described.
    pub generic: bool,
    #[schema(example = "powertrain/emissions")]
    pub system: String,
    #[schema(example = "Emission controls")]
    pub system_name: String,
    pub title: String,
    /// What the code tells a service advisor.
    pub meaning: String,
    /// General causes, not about one make.
    pub common_causes: Vec<String>,
    /// What a technician usually verifies first.
    pub checks: Vec<String>,
    pub components: Vec<CodeComponent>,
    /// Related codes that are described too.
    pub related: Vec<CodeLink>,
    /// The GitHub name of the person who approved the text.
    pub reviewer: String,
    /// The day it was approved.
    #[schema(example = "2026-10-12")]
    pub checked: String,
    #[schema(example = "https://open.wenmarpro.com/codes/P0420")]
    pub url: String,
    /// What a description is and is not.
    pub note: String,
    /// The credit lines the data's licences ask for.
    pub attribution: Vec<String>,
}

/// One page of the described codes, by code.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, ToSchema)]
pub struct CodeList {
    /// The newest day a description was approved.
    #[schema(example = "2026-10-12")]
    pub as_of: Option<String>,
    pub codes: Vec<CodeLink>,
    /// Pass it back as `cursor` for the next page. `null` on the last.
    pub next_cursor: Option<String>,
    pub note: String,
    pub attribution: Vec<String>,
}

/// The bulletins of a model year whose summary names a code.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, ToSchema)]
pub struct CodeBulletins {
    /// The day the bulletins were current.
    #[schema(example = "2026-10-11")]
    pub as_of: Option<String>,
    /// How many there are. `bulletins` holds at most ten.
    pub count: u64,
    /// The newest, newest first.
    pub bulletins: Vec<Bulletin>,
    /// The model year's bulletin page on this site, narrowed to the code.
    pub url: String,
}

/// The recalls of a model year about the components a code concerns.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, ToSchema)]
pub struct CodeRecalls {
    /// Always `true`: a campaign lists a model year, never one vehicle.
    pub may_apply: bool,
    /// At most ten, newest first.
    pub recalls: Vec<Recall>,
    pub lookup: RecallLookup,
}

/// What else is held for the model year about the components a code
/// concerns.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, ToSchema)]
pub struct CodeRelated {
    /// The components of the code's description. Empty without one.
    pub components: Vec<CodeComponent>,
    /// Bulletins about those components that do not name the code, at
    /// most ten. `null` where bulletins are not held.
    pub bulletins: Option<Vec<Bulletin>>,
    /// `null` where recalls are not held.
    pub recalls: Option<CodeRecalls>,
}

/// A trouble code on one model year.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, ToSchema)]
pub struct VehicleCode {
    pub vehicle: RecallVehicle,
    #[schema(example = "P0420")]
    pub code: String,
    /// Whether every maker uses the code the same way. A code that is not
    /// generic is never described, and may still be named by bulletins.
    pub generic: bool,
    /// `null` when the code is not generic, when nobody has described it,
    /// or where descriptions are not held.
    pub description: Option<Code>,
    /// `null` where bulletins are not held: they are on the hosted
    /// service only.
    pub bulletins: Option<CodeBulletins>,
    pub related: CodeRelated,
    /// The page of this code on this model year.
    pub url: String,
    pub note: String,
    pub attribution: Vec<String>,
}

// ----- jobs and labour estimates -----

/// A repair job in a list.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, ToSchema)]
pub struct JobLink {
    /// The job's id. It never changes.
    #[schema(example = "brake-pads-front")]
    pub id: String,
    #[schema(example = "Replace front brake pads")]
    pub name: String,
    /// The id of the system the job belongs to.
    #[schema(example = "brakes")]
    pub system: String,
    /// The job's page on this site.
    #[schema(example = "https://open.wenmarpro.com/jobs/brake-pads-front")]
    pub url: String,
}

/// The repair jobs a labour estimate can be about.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, ToSchema)]
pub struct JobList {
    /// The day the list was last changed.
    #[schema(example = "2026-10-12")]
    pub as_of: Option<String>,
    /// In the order of the list.
    pub jobs: Vec<JobLink>,
    pub attribution: Vec<String>,
}

/// A component a job acts on.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, ToSchema)]
pub struct JobComponent {
    #[schema(example = "brake-pad")]
    pub id: String,
    #[schema(example = "Brake pads")]
    pub name: String,
    /// The system a part belongs to. `null` for a system.
    pub parent: Option<String>,
}

/// Hours, as numbers of hours.
#[derive(Debug, Clone, PartialEq, Serialize, ToSchema)]
pub struct LaborHours {
    pub low: f64,
    /// A vehicle in ordinary condition.
    pub typical: f64,
    /// Allowing for rust and seized fasteners.
    pub high: f64,
}

/// The first and last model year a figure covers.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, ToSchema)]
pub struct LaborYears {
    pub from: u16,
    pub to: u16,
}

/// What stands behind a measured figure.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, ToSchema)]
pub struct LaborSample {
    /// Repair orders.
    pub jobs: i64,
    /// Shops they came from.
    pub shops: i64,
}

/// One labour estimate: the hours a job takes, and what the figure is for.
#[derive(Debug, Clone, PartialEq, Serialize, ToSchema)]
pub struct LaborFigure {
    /// The job's id.
    #[schema(example = "brake-pads-front")]
    pub job: String,
    pub name: String,
    pub hours: LaborHours,
    /// `expert`: written by a person who does the work. `observed`:
    /// measured from repair orders; `sample` then says how many. `ai`:
    /// worked out by a model from the job and the repair steps, not checked
    /// by a person; `confidence_pct`, `model`, `method` and `inputs` then
    /// say how. Within one scope: observed, then expert, then ai.
    #[schema(example = "expert")]
    pub basis: String,
    /// What the figure is for, and so how far to trust it for one
    /// vehicle: `engine`, `generation`, `model` or `class`.
    #[schema(example = "class")]
    pub scope: String,
    /// The class, for a class-scope figure.
    #[schema(example = "small-suv")]
    pub class: Option<String>,
    #[schema(example = "Small SUV")]
    pub class_name: Option<String>,
    /// The model years, for a generation or an engine.
    pub years: Option<LaborYears>,
    /// The engine's id form, for an engine-scope figure.
    pub engine: Option<String>,
    /// `null` for an `expert` figure.
    pub sample: Option<LaborSample>,
    /// For an `ai` figure: how sure the model was, in percent (at most 60).
    /// `null` for the others.
    #[schema(example = 45)]
    pub confidence_pct: Option<i64>,
    /// For an `ai` figure: the model that answered. `null` for the others.
    pub model: Option<String>,
    /// For an `ai` figure: the prompt version. `null` for the others.
    #[schema(example = "open-labour-ai-1")]
    pub method: Option<String>,
    /// For an `ai` figure: what it was worked out from, of `vehicle`,
    /// `job`, `wenmar-figures` and `procedure` (a repair procedure's steps,
    /// never its numbers). `null` for the others.
    pub inputs: Option<Vec<String>>,
    pub notes: Option<String>,
    /// The day the figure was last changed.
    #[schema(example = "2026-10-12")]
    pub updated: String,
    /// The job's page on this site.
    pub url: String,
}

/// One repair job: what it covers, and what it typically takes by class
/// of vehicle.
#[derive(Debug, Clone, PartialEq, Serialize, ToSchema)]
pub struct JobDetail {
    #[schema(example = "brake-pads-front")]
    pub id: String,
    pub name: String,
    #[schema(example = "brakes")]
    pub system: String,
    #[schema(example = "Brakes")]
    pub system_name: String,
    pub components: Vec<JobComponent>,
    /// The work the job covers.
    pub includes: Vec<String>,
    /// Work often done at the same visit that is not part of this job.
    pub excludes: Vec<String>,
    pub related: Vec<JobLink>,
    /// The job's class-scope figures. `null` where labour estimates are
    /// not held.
    pub estimates: Option<Vec<LaborFigure>>,
    pub url: String,
    /// What an estimate is not.
    pub note: String,
    /// The credit the labour estimates ask for. Empty without them.
    pub attribution: Vec<String>,
}

/// The class a model year is in.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, ToSchema)]
pub struct LaborClass {
    #[schema(example = "small-suv")]
    pub id: String,
    #[schema(example = "Small SUV")]
    pub name: String,
    /// `epa`: EPA's size class for the model year. `nearest`: that of the
    /// nearest model year of the same model EPA rated.
    pub source: String,
}

/// Labour estimates for one model year: for each job, the figure of the
/// narrowest scope that has one.
#[derive(Debug, Clone, PartialEq, Serialize, ToSchema)]
pub struct VehicleLabor {
    pub vehicle: RecallVehicle,
    /// The engine the vehicle id named, as an id form. `null` when it
    /// named none: no engine-scope figure can then answer.
    pub engine: Option<String>,
    /// `null` when the model year has no class: no class-scope figure
    /// reaches it.
    pub class: Option<LaborClass>,
    /// The newest day an estimate was changed.
    pub as_of: Option<String>,
    /// One for each job that has a figure covering the vehicle, in the
    /// order of the job list.
    pub estimates: Vec<LaborFigure>,
    /// "An estimate, not a quote. …"
    pub note: String,
    pub attribution: Vec<String>,
}

/// Labour estimates for the vehicle a VIN decodes to.
#[derive(Debug, Clone, PartialEq, Serialize, ToSchema)]
pub struct VinLabor {
    pub decode: RecallDecode,
    /// `null` when the VIN decodes to no vehicle in the catalog.
    pub vehicle: Option<RecallVehicle>,
    pub engine: Option<String>,
    pub class: Option<LaborClass>,
    pub as_of: Option<String>,
    pub estimates: Vec<LaborFigure>,
    pub note: String,
    pub attribution: Vec<String>,
    pub warnings: Vec<Warning>,
}
