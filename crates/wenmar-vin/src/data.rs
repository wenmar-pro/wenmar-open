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
}

/// A vehicle attribute a pattern can describe.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
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

    /// Ids of the schemas valid for this manufacturer code and model year.
    fn schemas(&self, wmi: &str, year: u16) -> Result<Vec<i64>, DataError>;

    /// Every pattern belonging to the given schemas.
    fn patterns(&self, schema_ids: &[i64]) -> Result<Vec<Pattern>, DataError>;
}

impl<T: VinData + ?Sized> VinData for &T {
    fn manufacturer(&self, wmi: &str) -> Result<Option<Manufacturer>, DataError> {
        (**self).manufacturer(wmi)
    }

    fn schemas(&self, wmi: &str, year: u16) -> Result<Vec<i64>, DataError> {
        (**self).schemas(wmi, year)
    }

    fn patterns(&self, schema_ids: &[i64]) -> Result<Vec<Pattern>, DataError> {
        (**self).patterns(schema_ids)
    }
}
