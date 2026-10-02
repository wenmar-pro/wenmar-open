//! SQLite values as JSON, both ways.

use serde_json::Value as Json;
use wenmar_vehicles::Value;

/// A value read from JSON. `None` for an array or an object, which no SQLite
/// value is.
///
/// JavaScript has one kind of number, so a whole number is an integer
/// whichever way it was written, and `true` and `false` are 1 and 0.
pub fn from_json(cell: &Json) -> Option<Value> {
    match cell {
        Json::Null => Some(Value::Null),
        Json::Bool(flag) => Some(Value::Integer(i64::from(*flag))),
        Json::Number(number) => match (number.as_i64(), number.as_f64()) {
            (Some(integer), _) => Some(Value::Integer(integer)),
            (None, Some(real)) if real.fract() == 0.0 && real.abs() < 9.0e15 => {
                // In range by the test above, so the cast is exact.
                Some(Value::Integer(real as i64))
            }
            (None, Some(real)) => Some(Value::Real(real)),
            (None, None) => None,
        },
        Json::String(text) => Some(Value::Text(text.clone())),
        Json::Array(_) | Json::Object(_) => None,
    }
}

pub fn to_json(value: &Value) -> Json {
    match value {
        Value::Null => Json::Null,
        Value::Integer(integer) => Json::from(*integer),
        // A number JSON cannot write, such as infinity, is sent as null.
        Value::Real(real) => serde_json::Number::from_f64(*real).map_or(Json::Null, Json::Number),
        Value::Text(text) => Json::String(text.clone()),
    }
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    #[test]
    fn whole_numbers_are_integers_however_they_are_written() {
        assert_eq!(from_json(&json!(7)), Some(Value::Integer(7)));
        assert_eq!(from_json(&json!(7.0)), Some(Value::Integer(7)));
        assert_eq!(from_json(&json!(-3)), Some(Value::Integer(-3)));
        assert_eq!(from_json(&json!(2.5)), Some(Value::Real(2.5)));
        assert_eq!(from_json(&json!(1e300)), Some(Value::Real(1e300)));
        assert_eq!(
            from_json(&json!(u64::MAX)),
            Some(Value::Real(u64::MAX as f64))
        );
    }

    #[test]
    fn the_other_kinds() {
        assert_eq!(from_json(&json!(null)), Some(Value::Null));
        assert_eq!(from_json(&json!(true)), Some(Value::Integer(1)));
        assert_eq!(
            from_json(&json!("Kona")),
            Some(Value::Text("Kona".to_owned()))
        );
        assert_eq!(from_json(&json!([1])), None);
        assert_eq!(from_json(&json!({ "a": 1 })), None);
    }

    #[test]
    fn a_value_survives_the_trip() {
        for value in [
            Value::Null,
            Value::Integer(2_353_773),
            Value::Real(2.5),
            Value::Text("K2***".to_owned()),
        ] {
            assert_eq!(from_json(&to_json(&value)), Some(value));
        }
        assert_eq!(to_json(&Value::Real(f64::INFINITY)), Json::Null);
    }
}
