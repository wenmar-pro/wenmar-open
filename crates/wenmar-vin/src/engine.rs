use serde::Serialize;

/// What is known about a vehicle's engine.
#[derive(Debug, Clone, PartialEq, Default, Serialize)]
pub struct Engine {
    /// Short form such as `2.0L Turbo`. Filled in by [`label`].
    #[serde(skip_serializing_if = "Option::is_none")]
    pub label: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub model: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub displacement_l: Option<f32>,
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

/// The short engine name shops use, such as `3.5L V6` or `2.5L Hybrid`.
pub fn label(engine: &Engine) -> Option<String> {
    let electrification = engine
        .electrification
        .as_deref()
        .unwrap_or_default()
        .to_ascii_uppercase();
    let fuel = engine
        .fuel
        .as_deref()
        .unwrap_or_default()
        .to_ascii_uppercase();

    let Some(displacement) = engine.displacement_l else {
        let electric = electrification.contains("BEV") || fuel == "ELECTRIC";
        return electric.then(|| "Electric".to_owned());
    };

    let mut label = format!("{displacement:.1}L");
    if engine.turbo == Some(true) {
        label.push_str(" Turbo");
    }
    if let (Some(configuration), Some(cylinders)) = (&engine.configuration, engine.cylinders)
        && configuration.to_ascii_uppercase().starts_with('V')
    {
        label.push_str(" V");
        label.push_str(&cylinders.to_string());
    }
    if fuel.contains("DIESEL") {
        label.push_str(" Diesel");
    }
    if electrification.contains("PHEV") {
        label.push_str(" PHEV");
    } else if electrification.contains("HEV") {
        label.push_str(" Hybrid");
    }
    Some(label)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn engine(displacement_l: f32) -> Engine {
        Engine {
            displacement_l: Some(displacement_l),
            ..Engine::default()
        }
    }

    #[test]
    fn displacement_alone() {
        assert_eq!(label(&engine(2.0)).as_deref(), Some("2.0L"));
        assert_eq!(label(&engine(5.3)).as_deref(), Some("5.3L"));
    }

    #[test]
    fn turbo() {
        let turbo = Engine {
            turbo: Some(true),
            ..engine(2.0)
        };
        assert_eq!(label(&turbo).as_deref(), Some("2.0L Turbo"));
        let not_turbo = Engine {
            turbo: Some(false),
            ..engine(2.0)
        };
        assert_eq!(label(&not_turbo).as_deref(), Some("2.0L"));
    }

    #[test]
    fn v_configuration_needs_a_cylinder_count() {
        let v6 = Engine {
            configuration: Some("V-Shaped".to_owned()),
            cylinders: Some(6),
            ..engine(3.5)
        };
        assert_eq!(label(&v6).as_deref(), Some("3.5L V6"));
        let inline = Engine {
            configuration: Some("In-Line".to_owned()),
            cylinders: Some(4),
            ..engine(2.0)
        };
        assert_eq!(label(&inline).as_deref(), Some("2.0L"));
        let unknown_count = Engine {
            configuration: Some("V-Shaped".to_owned()),
            ..engine(3.5)
        };
        assert_eq!(label(&unknown_count).as_deref(), Some("3.5L"));
    }

    #[test]
    fn diesel_follows_the_configuration() {
        let diesel = Engine {
            configuration: Some("V-Shaped".to_owned()),
            cylinders: Some(8),
            fuel: Some("Diesel".to_owned()),
            ..engine(6.7)
        };
        assert_eq!(label(&diesel).as_deref(), Some("6.7L V8 Diesel"));
    }

    #[test]
    fn hybrids() {
        let hybrid = Engine {
            electrification: Some("Strong HEV (Hybrid Electric Vehicle)".to_owned()),
            ..engine(2.5)
        };
        assert_eq!(label(&hybrid).as_deref(), Some("2.5L Hybrid"));
        let plug_in = Engine {
            electrification: Some("PHEV (Plug-in Hybrid Electric Vehicle)".to_owned()),
            ..engine(2.5)
        };
        assert_eq!(label(&plug_in).as_deref(), Some("2.5L PHEV"));
    }

    #[test]
    fn electric_vehicles_have_no_displacement() {
        let battery = Engine {
            electrification: Some("BEV (Battery Electric Vehicle)".to_owned()),
            ..Engine::default()
        };
        assert_eq!(label(&battery).as_deref(), Some("Electric"));
        let by_fuel = Engine {
            fuel: Some("Electric".to_owned()),
            ..Engine::default()
        };
        assert_eq!(label(&by_fuel).as_deref(), Some("Electric"));
    }

    #[test]
    fn nothing_known_means_no_label() {
        assert_eq!(label(&Engine::default()), None);
    }
}
