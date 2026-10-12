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
/// `fluids`, `tires`, `labor` and `bulletins` are `null` in every answer
/// today, and each is filled when its data arrives. A client must accept
/// an object there.
#[derive(Debug, Clone, PartialEq, Serialize, ToSchema)]
pub struct Overview {
    /// The model year. `null` when a VIN reaches no model year of the
    /// catalog; every block is then `null` too.
    pub vehicle: Option<RecallVehicle>,
    /// What the VIN decoded to. `null` when the overview was asked for by
    /// vehicle id.
    pub decode: Option<OverviewDecode>,
    #[schema(value_type = Option<Object>)]
    pub specs: Option<serde_json::Value>,
    #[schema(value_type = Option<Object>)]
    pub fuel: Option<serde_json::Value>,
    #[schema(value_type = Option<Object>)]
    pub fluids: Option<serde_json::Value>,
    #[schema(value_type = Option<Object>)]
    pub tires: Option<serde_json::Value>,
    /// `null` when this server holds no recalls. A count of `0` with a
    /// day in `as_of` means none was found.
    pub recalls: Option<OverviewRecalls>,
    #[schema(value_type = Option<Object>)]
    pub labor: Option<serde_json::Value>,
    #[schema(value_type = Option<Object>)]
    pub bulletins: Option<serde_json::Value>,
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
