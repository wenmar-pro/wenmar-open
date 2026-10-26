//! vPIC's elements: the attributes a VIN pattern, an engine model or a
//! specification sheet can give, by `element.id`. The codes and names are
//! NHTSA's own, from the Element table of vPIC.

/// One vPIC element.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ElementInfo {
    /// vPIC's `element.id`.
    pub id: i64,
    /// vPIC's code for it, such as `EngineHP`, as NHTSA's API names it.
    pub code: &'static str,
    /// vPIC's name for it, such as `Engine Brake (hp) From`.
    pub name: &'static str,
}

/// Every element vPIC defines, by id.
pub const ELEMENTS: &[ElementInfo] = &[
    ElementInfo {
        id: 1,
        code: "BatteryInfo",
        name: "Other Battery Info",
    },
    ElementInfo {
        id: 2,
        code: "BatteryType",
        name: "Battery Type",
    },
    ElementInfo {
        id: 3,
        code: "BedType",
        name: "Bed Type",
    },
    ElementInfo {
        id: 4,
        code: "BodyCabType",
        name: "Cab Type",
    },
    ElementInfo {
        id: 5,
        code: "BodyClass",
        name: "Body Class",
    },
    ElementInfo {
        id: 8,
        code: "Country",
        name: "Country Name",
    },
    ElementInfo {
        id: 9,
        code: "EngineCylinders",
        name: "Engine Number of Cylinders",
    },
    ElementInfo {
        id: 10,
        code: "DestinationMarket",
        name: "Destination Market",
    },
    ElementInfo {
        id: 11,
        code: "DisplacementCC",
        name: "Displacement (CC)",
    },
    ElementInfo {
        id: 12,
        code: "DisplacementCI",
        name: "Displacement (CI)",
    },
    ElementInfo {
        id: 13,
        code: "DisplacementL",
        name: "Displacement (L)",
    },
    ElementInfo {
        id: 14,
        code: "Doors",
        name: "Doors",
    },
    ElementInfo {
        id: 15,
        code: "DriveType",
        name: "Drive Type",
    },
    ElementInfo {
        id: 16,
        code: "DriverAssist",
        name: "Driver Assist",
    },
    ElementInfo {
        id: 17,
        code: "EngineCycles",
        name: "Engine Stroke Cycles",
    },
    ElementInfo {
        id: 18,
        code: "EngineModel",
        name: "Engine Model",
    },
    ElementInfo {
        id: 21,
        code: "EngineKW",
        name: "Engine Power (kW)",
    },
    ElementInfo {
        id: 23,
        code: "EntertainmentSystem",
        name: "Entertainment System",
    },
    ElementInfo {
        id: 24,
        code: "FuelTypePrimary",
        name: "Fuel Type - Primary",
    },
    ElementInfo {
        id: 25,
        code: "GVWR",
        name: "Gross Vehicle Weight Rating From",
    },
    ElementInfo {
        id: 26,
        code: "Make",
        name: "Make",
    },
    ElementInfo {
        id: 27,
        code: "Manufacturer",
        name: "Manufacturer Name",
    },
    ElementInfo {
        id: 28,
        code: "Model",
        name: "Model",
    },
    ElementInfo {
        id: 29,
        code: "ModelYear",
        name: "Model Year",
    },
    ElementInfo {
        id: 31,
        code: "PlantCity",
        name: "Plant City",
    },
    ElementInfo {
        id: 33,
        code: "Seats",
        name: "Number of Seats",
    },
    ElementInfo {
        id: 34,
        code: "Series",
        name: "Series",
    },
    ElementInfo {
        id: 36,
        code: "SteeringLocation",
        name: "Steering Location",
    },
    ElementInfo {
        id: 37,
        code: "TransmissionStyle",
        name: "Transmission Style",
    },
    ElementInfo {
        id: 38,
        code: "Trim",
        name: "Trim",
    },
    ElementInfo {
        id: 39,
        code: "VehicleType",
        name: "Vehicle Type",
    },
    ElementInfo {
        id: 40,
        code: "Windows",
        name: "Windows",
    },
    ElementInfo {
        id: 41,
        code: "Axles",
        name: "Axles",
    },
    ElementInfo {
        id: 42,
        code: "BrakeSystemType",
        name: "Brake System Type",
    },
    ElementInfo {
        id: 45,
        code: "NCICCode",
        name: "NCIC Code",
    },
    ElementInfo {
        id: 48,
        code: "BatteryCells",
        name: "Number of Battery Cells per Module",
    },
    ElementInfo {
        id: 49,
        code: "BedLengthIN",
        name: "Bed Length (inches)",
    },
    ElementInfo {
        id: 52,
        code: "BrakeSystemDesc",
        name: "Brake System Description",
    },
    ElementInfo {
        id: 54,
        code: "CurbWeightLB",
        name: "Curb Weight (pounds)",
    },
    ElementInfo {
        id: 55,
        code: "AirBagLocCurtain",
        name: "Curtain Air Bag Locations",
    },
    ElementInfo {
        id: 56,
        code: "AirBagLocSeatCushion",
        name: "Seat Cushion Air Bag Locations",
    },
    ElementInfo {
        id: 57,
        code: "BatteryA",
        name: "Battery Current (Amps) From",
    },
    ElementInfo {
        id: 58,
        code: "BatteryV",
        name: "Battery Voltage (Volts) From",
    },
    ElementInfo {
        id: 59,
        code: "BatteryKWh",
        name: "Battery Energy (kWh) From",
    },
    ElementInfo {
        id: 60,
        code: "WheelBaseType",
        name: "Wheelbase Type",
    },
    ElementInfo {
        id: 61,
        code: "SeatRows",
        name: "Number of Seat Rows",
    },
    ElementInfo {
        id: 62,
        code: "ValveTrainDesign",
        name: "Valve Train Design",
    },
    ElementInfo {
        id: 63,
        code: "TransmissionSpeeds",
        name: "Transmission Speeds",
    },
    ElementInfo {
        id: 64,
        code: "EngineConfiguration",
        name: "Engine Configuration",
    },
    ElementInfo {
        id: 65,
        code: "AirBagLocFront",
        name: "Front Air Bag Locations",
    },
    ElementInfo {
        id: 66,
        code: "FuelTypeSecondary",
        name: "Fuel Type - Secondary",
    },
    ElementInfo {
        id: 67,
        code: "FuelInjectionType",
        name: "Fuel Delivery/Fuel Injection Type",
    },
    ElementInfo {
        id: 69,
        code: "AirBagLocKnee",
        name: "Knee Air Bag Locations",
    },
    ElementInfo {
        id: 71,
        code: "EngineHP",
        name: "Engine Brake (hp) From",
    },
    ElementInfo {
        id: 72,
        code: "EVDriveUnit",
        name: "EV Drive Unit",
    },
    ElementInfo {
        id: 75,
        code: "PlantCountry",
        name: "Plant Country",
    },
    ElementInfo {
        id: 76,
        code: "PlantCompanyName",
        name: "Plant Company Name",
    },
    ElementInfo {
        id: 77,
        code: "PlantState",
        name: "Plant State",
    },
    ElementInfo {
        id: 78,
        code: "Pretensioner",
        name: "Pretensioner",
    },
    ElementInfo {
        id: 79,
        code: "SeatBeltsAll",
        name: "Seat Belt Type",
    },
    ElementInfo {
        id: 81,
        code: "AdaptiveCruiseControl",
        name: "Adaptive Cruise Control (ACC)",
    },
    ElementInfo {
        id: 82,
        code: "AdaptiveHeadlights",
        name: "Adaptive Headlights",
    },
    ElementInfo {
        id: 86,
        code: "ABS",
        name: "Antilock Braking System (ABS)",
    },
    ElementInfo {
        id: 87,
        code: "CIB",
        name: "Crash Imminent Braking (CIB)",
    },
    ElementInfo {
        id: 88,
        code: "BlindSpotMon",
        name: "Blind Spot Warning (BSW)",
    },
    ElementInfo {
        id: 89,
        code: "CAFEBodyType",
        name: "CAFE Body Type",
    },
    ElementInfo {
        id: 90,
        code: "CAFEMake",
        name: "CAFE Make",
    },
    ElementInfo {
        id: 91,
        code: "CAFEModel",
        name: "CAFE Model",
    },
    ElementInfo {
        id: 92,
        code: "NCAPBodyType",
        name: "NCAP Body Type",
    },
    ElementInfo {
        id: 93,
        code: "NCAPMake",
        name: "NCAP Make",
    },
    ElementInfo {
        id: 94,
        code: "NCAPModel",
        name: "NCAP Model",
    },
    ElementInfo {
        id: 96,
        code: "NCSABodyType",
        name: "NCSA Body Type",
    },
    ElementInfo {
        id: 97,
        code: "NCSAMake",
        name: "NCSA Make",
    },
    ElementInfo {
        id: 98,
        code: "NCSAModel",
        name: "NCSA Model",
    },
    ElementInfo {
        id: 99,
        code: "ESC",
        name: "Electronic Stability Control (ESC)",
    },
    ElementInfo {
        id: 100,
        code: "TractionControl",
        name: "Traction Control",
    },
    ElementInfo {
        id: 101,
        code: "ForwardCollisionWarning",
        name: "Forward Collision Warning (FCW)",
    },
    ElementInfo {
        id: 102,
        code: "LaneDepartureWarning",
        name: "Lane Departure Warning (LDW)",
    },
    ElementInfo {
        id: 103,
        code: "LaneKeepSystem",
        name: "Lane Keeping Assistance (LKA)",
    },
    ElementInfo {
        id: 104,
        code: "RearVisibilitySystem",
        name: "Backup Camera",
    },
    ElementInfo {
        id: 105,
        code: "ParkAssist",
        name: "Parking Assist",
    },
    ElementInfo {
        id: 107,
        code: "AirBagLocSide",
        name: "Side Air Bag Locations",
    },
    ElementInfo {
        id: 109,
        code: "Trim2",
        name: "Trim2",
    },
    ElementInfo {
        id: 110,
        code: "Series2",
        name: "Series2",
    },
    ElementInfo {
        id: 111,
        code: "WheelBaseShort",
        name: "Wheelbase (inches) From",
    },
    ElementInfo {
        id: 112,
        code: "WheelBaseLong",
        name: "Wheelbase (inches) To",
    },
    ElementInfo {
        id: 114,
        code: "Note",
        name: "Note",
    },
    ElementInfo {
        id: 115,
        code: "Wheels",
        name: "Number of Wheels",
    },
    ElementInfo {
        id: 116,
        code: "TrailerType",
        name: "Trailer Type Connection",
    },
    ElementInfo {
        id: 117,
        code: "TrailerBodyType",
        name: "Trailer Body Type",
    },
    ElementInfo {
        id: 118,
        code: "TrailerLength",
        name: "Trailer Length (feet)",
    },
    ElementInfo {
        id: 119,
        code: "WheelSizeFront",
        name: "Wheel Size Front (inches)",
    },
    ElementInfo {
        id: 120,
        code: "WheelSizeRear",
        name: "Wheel Size Rear (inches)",
    },
    ElementInfo {
        id: 121,
        code: "OtherRestraintSystemInfo",
        name: "Other Restraint System Info",
    },
    ElementInfo {
        id: 122,
        code: "CoolingType",
        name: "Cooling Type",
    },
    ElementInfo {
        id: 125,
        code: "EngineHP_to",
        name: "Engine Brake (hp) To",
    },
    ElementInfo {
        id: 126,
        code: "ElectrificationLevel",
        name: "Electrification Level",
    },
    ElementInfo {
        id: 127,
        code: "ChargerLevel",
        name: "Charger Level",
    },
    ElementInfo {
        id: 128,
        code: "ChargerPowerKW",
        name: "Charger Power (kW)",
    },
    ElementInfo {
        id: 129,
        code: "OtherEngineInfo",
        name: "Other Engine Info",
    },
    ElementInfo {
        id: 132,
        code: "BatteryA_to",
        name: "Battery Current (Amps) To",
    },
    ElementInfo {
        id: 133,
        code: "BatteryV_to",
        name: "Battery Voltage (Volts) To",
    },
    ElementInfo {
        id: 134,
        code: "BatteryKWh_to",
        name: "Battery Energy (kWh) To",
    },
    ElementInfo {
        id: 135,
        code: "Turbo",
        name: "Turbo",
    },
    ElementInfo {
        id: 136,
        code: "BasePrice",
        name: "Base Price ($)",
    },
    ElementInfo {
        id: 137,
        code: "BatteryModules",
        name: "Number of Battery Modules per Pack",
    },
    ElementInfo {
        id: 138,
        code: "BatteryPacks",
        name: "Number of Battery Packs per Vehicle",
    },
    ElementInfo {
        id: 139,
        code: "TopSpeedMPH",
        name: "Top Speed (mph)",
    },
    ElementInfo {
        id: 140,
        code: "EquipmentType",
        name: "EquipmentType",
    },
    ElementInfo {
        id: 141,
        code: "ManufacturerType",
        name: "Manufacturer Type",
    },
    ElementInfo {
        id: 142,
        code: "SuggestedVIN",
        name: "Suggested VIN",
    },
    ElementInfo {
        id: 143,
        code: "ErrorCode",
        name: "Error Code",
    },
    ElementInfo {
        id: 144,
        code: "PossibleValues",
        name: "Possible Values",
    },
    ElementInfo {
        id: 145,
        code: "AxleConfiguration",
        name: "Axle Configuration",
    },
    ElementInfo {
        id: 146,
        code: "EngineManufacturer",
        name: "Engine Manufacturer",
    },
    ElementInfo {
        id: 147,
        code: "BusLength",
        name: "Bus Length (feet)",
    },
    ElementInfo {
        id: 148,
        code: "BusFloorConfigType",
        name: "Bus Floor Configuration Type",
    },
    ElementInfo {
        id: 149,
        code: "BusType",
        name: "Bus Type",
    },
    ElementInfo {
        id: 150,
        code: "OtherBusInfo",
        name: "Other Bus Info",
    },
    ElementInfo {
        id: 151,
        code: "CustomMotorcycleType",
        name: "Custom Motorcycle Type",
    },
    ElementInfo {
        id: 152,
        code: "MotorcycleSuspensionType",
        name: "Motorcycle Suspension Type",
    },
    ElementInfo {
        id: 153,
        code: "MotorcycleChassisType",
        name: "Motorcycle Chassis Type",
    },
    ElementInfo {
        id: 154,
        code: "OtherMotorcycleInfo",
        name: "Other Motorcycle Info",
    },
    ElementInfo {
        id: 155,
        code: "OtherTrailerInfo",
        name: "Other Trailer Info",
    },
    ElementInfo {
        id: 156,
        code: "AdditionalErrorText",
        name: "Additional Error Text",
    },
    ElementInfo {
        id: 158,
        code: "CashForClunkers",
        name: "Cash For Clunkers",
    },
    ElementInfo {
        id: 159,
        code: "TrackWidth",
        name: "Track Width (inches)",
    },
    ElementInfo {
        id: 168,
        code: "TPMS",
        name: "Tire Pressure Monitoring System (TPMS) Type",
    },
    ElementInfo {
        id: 169,
        code: "ActiveSafetySysNote",
        name: "Active Safety System Note",
    },
    ElementInfo {
        id: 170,
        code: "DynamicBrakeSupport",
        name: "Dynamic Brake Support (DBS)",
    },
    ElementInfo {
        id: 171,
        code: "PedestrianAutomaticEmergencyBraking",
        name: "Pedestrian Automatic Emergency Braking (PAEB)",
    },
    ElementInfo {
        id: 172,
        code: "AutoReverseSystem",
        name: "Auto-Reverse System for Windows and Sunroofs",
    },
    ElementInfo {
        id: 173,
        code: "AutomaticPedestrianAlertingSound",
        name: "Automatic Pedestrian Alerting Sound (for Hybrid and EV only)",
    },
    ElementInfo {
        id: 174,
        code: "CAN_AACN",
        name: "Automatic Crash Notification (ACN)/Advanced Automatic Crash Notification (AACN)",
    },
    ElementInfo {
        id: 175,
        code: "EDR",
        name: "Event Data Recorder (EDR)",
    },
    ElementInfo {
        id: 176,
        code: "KeylessIgnition",
        name: "Keyless Ignition",
    },
    ElementInfo {
        id: 177,
        code: "DaytimeRunningLight",
        name: "Daytime Running Light (DRL)",
    },
    ElementInfo {
        id: 178,
        code: "LowerBeamHeadlampLightSource",
        name: "Headlamp Light Source",
    },
    ElementInfo {
        id: 179,
        code: "SemiautomaticHeadlampBeamSwitching",
        name: "Semiautomatic Headlamp Beam Switching",
    },
    ElementInfo {
        id: 180,
        code: "AdaptiveDrivingBeam",
        name: "Adaptive Driving Beam (ADB)",
    },
    ElementInfo {
        id: 181,
        code: "SAEAutomationLevel",
        name: "SAE Automation Level From",
    },
    ElementInfo {
        id: 182,
        code: "SAEAutomationLevel_to",
        name: "SAE Automation Level To",
    },
    ElementInfo {
        id: 183,
        code: "RearCrossTrafficAlert",
        name: "Rear Cross Traffic Alert",
    },
    ElementInfo {
        id: 184,
        code: "GCWR",
        name: "Gross Combination Weight Rating From",
    },
    ElementInfo {
        id: 185,
        code: "GCWR_to",
        name: "Gross Combination Weight Rating To",
    },
    ElementInfo {
        id: 186,
        code: "NCSANote",
        name: "NCSA Note",
    },
    ElementInfo {
        id: 187,
        code: "NCSAMappingException",
        name: "NCSA Mapping Exception",
    },
    ElementInfo {
        id: 188,
        code: "NCSAMapExcApprovedOn",
        name: "NCSA Mapping Exception Approved On",
    },
    ElementInfo {
        id: 189,
        code: "NCSAMapExcApprovedBy",
        name: "NCSA Mapping Exception Approved By",
    },
    ElementInfo {
        id: 190,
        code: "GVWR_to",
        name: "Gross Vehicle Weight Rating To",
    },
    ElementInfo {
        id: 191,
        code: "ErrorText",
        name: "Error Text",
    },
    ElementInfo {
        id: 192,
        code: "RearAutomaticEmergencyBraking",
        name: "Rear Automatic Emergency Braking",
    },
    ElementInfo {
        id: 193,
        code: "BlindSpotIntervention",
        name: "Blind Spot Intervention (BSI)",
    },
    ElementInfo {
        id: 194,
        code: "LaneCenteringAssistance",
        name: "Lane Centering Assistance",
    },
    ElementInfo {
        id: 195,
        code: "NonLandUse",
        name: "Non-Land Use",
    },
    ElementInfo {
        id: 196,
        code: "VehicleDescriptor",
        name: "Vehicle Descriptor",
    },
    ElementInfo {
        id: 200,
        code: "FuelTankType",
        name: "Fuel-Tank Type",
    },
    ElementInfo {
        id: 201,
        code: "FuelTankMaterial",
        name: "Fuel-Tank Material",
    },
    ElementInfo {
        id: 202,
        code: "CombinedBrakingSystem",
        name: "Combined Braking System (CBS)",
    },
    ElementInfo {
        id: 203,
        code: "WheelieMitigation",
        name: "Wheelie Mitigation",
    },
];

/// The element with this id, if vPIC defines one.
pub fn element(id: i64) -> Option<&'static ElementInfo> {
    ELEMENTS
        .binary_search_by_key(&id, |element| element.id)
        .ok()
        .and_then(|index| ELEMENTS.get(index))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_table_is_sorted_so_it_can_be_searched() {
        assert!(ELEMENTS.windows(2).all(|pair| pair[0].id < pair[1].id));
        assert_eq!(element(71).map(|found| found.code), Some("EngineHP"));
        assert_eq!(element(0), None);
    }
}
