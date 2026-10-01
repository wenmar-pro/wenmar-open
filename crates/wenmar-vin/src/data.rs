/// An error from the underlying data store.
pub type DataError = Box<dyn std::error::Error + Send + Sync>;

/// Who a manufacturer code belongs to.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Manufacturer {
    /// The code that was looked up: three characters, or six for a
    /// low-volume manufacturer.
    pub wmi: String,
    pub name: String,
    pub make: Option<String>,
    pub country: Option<String>,
    pub vehicle_type: Option<String>,
    /// Passenger car, multipurpose vehicle, or light truck. Only these follow
    /// the rule that position 7 selects the model-year cycle.
    pub light_vehicle: bool,
}

impl Manufacturer {
    /// A manufacturer with only the required fields, treated as a light vehicle.
    pub fn new(wmi: &str, name: &str) -> Self {
        Self {
            wmi: wmi.to_owned(),
            name: name.to_owned(),
            make: None,
            country: None,
            vehicle_type: None,
            light_vehicle: true,
        }
    }
}

/// A vehicle attribute a pattern can describe.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
#[non_exhaustive]
pub enum Element {
    Make,
    Model,
    Series,
    Trim,
    BodyClass,
    Doors,
    DriveType,
    TransmissionStyle,
    TransmissionSpeeds,
    EngineModel,
    EngineConfiguration,
    EngineCylinders,
    DisplacementL,
    DisplacementCc,
    DisplacementCi,
    FuelTypePrimary,
    Turbo,
    ElectrificationLevel,
    PlantCity,
    PlantState,
    PlantCountry,
    PlantCompany,
    Abs,
    Esc,
    TpmsType,
    AdaptiveCruise,
    LaneKeep,
    LaneDeparture,
    BlindSpot,
    ForwardCollision,
    AutoBrake,
    BackupCamera,
    AirbagsFront,
    AirbagsSide,
    AirbagsCurtain,
    AirbagsKnee,
    /// A vPIC element this crate does not interpret, by its `element.id`.
    /// It is never shown in a decode, but it counts when NHTSA's rule for
    /// choosing between two model years is applied.
    Other(i64),
}

impl Element {
    /// How much NHTSA's decoder counts this element when it compares two
    /// candidate model years (`element.weight` in vPIC). Displacement is
    /// weighed once, by the decoder, whatever unit it is in.
    pub fn weight(self) -> u32 {
        match self {
            Element::Make | Element::Model | Element::BodyClass | Element::PlantCountry => 99,
            Element::PlantCity | Element::PlantState => 98,
            Element::FuelTypePrimary => 91,
            Element::EngineCylinders => 88,
            Element::AirbagsFront => 64,
            Element::Series | Element::Trim => 61,
            Element::EngineModel => 55,
            // Gross vehicle weight rating, seat belt type, axles.
            Element::Other(25) => 70,
            Element::Other(79) => 65,
            Element::Other(41) => 15,
            _ => 0,
        }
    }

    /// The element for a vPIC `element.id`, if this crate uses it.
    pub fn from_vpic_id(id: i64) -> Option<Element> {
        Some(match id {
            26 => Element::Make,
            28 => Element::Model,
            34 => Element::Series,
            38 => Element::Trim,
            5 => Element::BodyClass,
            14 => Element::Doors,
            15 => Element::DriveType,
            37 => Element::TransmissionStyle,
            63 => Element::TransmissionSpeeds,
            18 => Element::EngineModel,
            64 => Element::EngineConfiguration,
            9 => Element::EngineCylinders,
            13 => Element::DisplacementL,
            11 => Element::DisplacementCc,
            12 => Element::DisplacementCi,
            24 => Element::FuelTypePrimary,
            135 => Element::Turbo,
            126 => Element::ElectrificationLevel,
            31 => Element::PlantCity,
            77 => Element::PlantState,
            75 => Element::PlantCountry,
            76 => Element::PlantCompany,
            86 => Element::Abs,
            99 => Element::Esc,
            168 => Element::TpmsType,
            81 => Element::AdaptiveCruise,
            103 => Element::LaneKeep,
            102 => Element::LaneDeparture,
            88 => Element::BlindSpot,
            101 => Element::ForwardCollision,
            87 => Element::AutoBrake,
            104 => Element::BackupCamera,
            65 => Element::AirbagsFront,
            107 => Element::AirbagsSide,
            55 => Element::AirbagsCurtain,
            69 => Element::AirbagsKnee,
            _ => return None,
        })
    }
}

/// A schema that applies to a manufacturer code and model year.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SchemaRef {
    pub id: i64,
    /// First model year this schema applies to for this manufacturer code.
    /// When two patterns disagree, the later schema wins.
    pub year_from: u16,
}

/// One row of decoding data: when `keys` matches a VIN, `element` is `value`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Pattern {
    pub id: i64,
    pub schema_id: i64,
    pub keys: String,
    pub element: Element,
    /// The resolved, human-readable value.
    pub value: String,
    /// When the pattern was last changed, as a timestamp that sorts correctly
    /// as text (`2015-03-04 10:05:33.893`). Empty when unknown.
    pub changed_on: String,
}

/// Where the decoder gets its data.
///
/// Implementations must:
/// - set [`Manufacturer::wmi`] to the code that was looked up;
/// - return resolved values in [`Pattern::value`], not lookup ids;
/// - return a [`Element::Make`] pattern alongside every [`Element::Model`]
///   pattern, with the same `keys` and `schema_id`;
/// - leave out attributes [`Element`] does not list.
pub trait VinData {
    /// The manufacturer for a three- or six-character code.
    fn manufacturer(&self, wmi: &str) -> Result<Option<Manufacturer>, DataError>;

    /// The schemas valid for this manufacturer code and model year. A schema
    /// linked more than once may appear more than once.
    fn schemas(&self, wmi: &str, year: u16) -> Result<Vec<SchemaRef>, DataError>;

    /// Patterns belonging to the given schemas.
    ///
    /// `match_key` is the VIN's match key (see [`crate::Vin::match_key`]). An
    /// implementation may use it to leave out patterns that cannot match, but
    /// must return every pattern that does. The decoder checks each one again.
    fn patterns(&self, schema_ids: &[i64], match_key: &str) -> Result<Vec<Pattern>, DataError>;
}

impl<T: VinData + ?Sized> VinData for &T {
    fn manufacturer(&self, wmi: &str) -> Result<Option<Manufacturer>, DataError> {
        (**self).manufacturer(wmi)
    }

    fn schemas(&self, wmi: &str, year: u16) -> Result<Vec<SchemaRef>, DataError> {
        (**self).schemas(wmi, year)
    }

    fn patterns(&self, schema_ids: &[i64], match_key: &str) -> Result<Vec<Pattern>, DataError> {
        (**self).patterns(schema_ids, match_key)
    }
}
