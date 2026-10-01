//! The one-line description of a vehicle, and the short forms it uses.

/// The short form of a vPIC body class or transmission style: the part
/// before the first slash, or the abbreviation it carries in brackets.
/// `Sedan/Saloon` gives `Sedan`; `Sport Utility Vehicle [SUV]/...` gives `SUV`.
pub fn short(value: &str) -> String {
    let value = value.trim();
    let head = match value.split('/').next().map(str::trim) {
        Some(head) if !head.is_empty() => head,
        _ => value,
    };
    abbreviation(head).unwrap_or(head).to_owned()
}

/// The first bracketed token of at most six characters with no space in it,
/// such as `SUV` in `Sport Utility Vehicle [SUV]`.
fn abbreviation(text: &str) -> Option<&str> {
    let mut rest = text;
    while let Some(open) = rest.find(['[', '(']) {
        let after = &rest[open + 1..];
        let close = after.find([']', ')'])?;
        let inside = &after[..close];
        if !inside.is_empty()
            && inside.chars().count() <= 6
            && !inside.contains(char::is_whitespace)
            && !inside.contains(['[', '('])
        {
            return Some(inside);
        }
        rest = &after[close + 1..];
    }
    None
}

/// The short form of a vPIC drive type: `FWD/Front-Wheel Drive` gives `FWD`.
/// Values where the slash separates alternatives, such as `2WD/4WD`, are
/// kept whole. This is the rule `wenmar-vin` applies to a decode.
pub fn short_drive(value: &str) -> String {
    match value.split_once('/') {
        Some((short, rest)) if !short.trim().is_empty() && rest.contains("Wheel Drive") => {
            short.trim().to_owned()
        }
        _ => value.trim().to_owned(),
    }
}

/// What a summary line is written from. Values are already in short form.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Parts<'a> {
    pub year: u16,
    pub make: &'a str,
    pub model: &'a str,
    pub submodel: Option<&'a str>,
    pub engine: Option<&'a str>,
    pub transmission: Option<&'a str>,
    pub drive: Option<&'a str>,
    pub body: Option<&'a str>,
}

/// One line naming what is known, such as
/// `2019 Honda Civic Si, 1.5L Turbo, Manual, FWD, Sedan`.
pub fn line(parts: &Parts<'_>) -> String {
    let mut line = format!("{} {} {}", parts.year, parts.make, parts.model);
    if let Some(submodel) = parts.submodel {
        line.push(' ');
        line.push_str(submodel);
    }
    for detail in [parts.engine, parts.transmission, parts.drive, parts.body]
        .into_iter()
        .flatten()
    {
        line.push_str(", ");
        line.push_str(detail);
    }
    line
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn body_and_transmission_names_are_shortened() {
        for (long, expected) in [
            ("Sedan/Saloon", "Sedan"),
            ("Hatchback/Liftback/Notchback", "Hatchback"),
            ("Convertible/Cabriolet", "Convertible"),
            (
                "Sport Utility Vehicle [SUV]/Multipurpose Vehicle [MPV]",
                "SUV",
            ),
            (
                "Sport Utility Vehicle (SUV)/Multi-Purpose Vehicle (MPV)",
                "SUV",
            ),
            ("Crossover Utility Vehicle [CUV]", "CUV"),
            (
                "Off-Road Vehicle - All-Terrain Vehicle [ATV] (Motorcycle-Style)",
                "ATV",
            ),
            ("Pickup", "Pickup"),
            ("  Coupe ", "Coupe"),
            ("Manual/Standard", "Manual"),
            ("Automatic", "Automatic"),
            ("Continuously Variable Transmission (CVT)", "CVT"),
            ("Electronic Continuously Variable (e-CVT)", "e-CVT"),
            ("Dual-clutch Transmission (DCT)", "DCT"),
        ] {
            assert_eq!(short(long), expected, "{long}");
        }
    }

    #[test]
    fn a_bracket_that_is_not_an_abbreviation_is_left_alone() {
        for same in [
            "Incomplete - Chassis Cab (Double Cab)",
            "Motorcycle - Enclosed Three Wheeled or Enclosed Autocycle [1 Rear Wheel]",
            "Truck (",
            "/",
            "",
        ] {
            assert_eq!(short(same), same, "{same}");
        }
    }

    #[test]
    fn drive_types_keep_the_part_before_the_slash() {
        assert_eq!(short_drive("FWD/Front-Wheel Drive"), "FWD");
        assert_eq!(short_drive("4WD/4-Wheel Drive/4x4"), "4WD");
        assert_eq!(short_drive("AWD/All-Wheel Drive"), "AWD");
        assert_eq!(short_drive("2WD/4WD"), "2WD/4WD");
        assert_eq!(short_drive("4x2"), "4x2");
    }

    fn parts<'a>() -> Parts<'a> {
        Parts {
            year: 2019,
            make: "Honda",
            model: "Civic",
            submodel: None,
            engine: None,
            transmission: None,
            drive: None,
            body: None,
        }
    }

    #[test]
    fn the_line_names_what_is_known_in_a_fixed_order() {
        assert_eq!(line(&parts()), "2019 Honda Civic");
        let full = Parts {
            submodel: Some("Si"),
            engine: Some("1.5L Turbo"),
            transmission: Some("Manual"),
            drive: Some("FWD"),
            body: Some("Sedan"),
            ..parts()
        };
        assert_eq!(
            line(&full),
            "2019 Honda Civic Si, 1.5L Turbo, Manual, FWD, Sedan"
        );
        let some = Parts {
            engine: Some("2.0L"),
            body: Some("Sedan"),
            ..parts()
        };
        assert_eq!(line(&some), "2019 Honda Civic, 2.0L, Sedan");
    }
}
