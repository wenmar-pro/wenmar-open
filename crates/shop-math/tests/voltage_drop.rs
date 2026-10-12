use shop_math::voltage_drop::{
    Circuit, DEFAULT_ALLOWED, PER_HUNDREDTH_FOOT, PER_HUNDREDTH_METRE, from_ohms_per_1000_ft,
    from_ohms_per_km, parse_current, parse_length, parse_resistance, voltage_drop, volts,
};
use shop_math::{ParseError, Percent};

/// 20 A through 5 m of wire at 12 V, with 3% allowed. The resistances in this file are
/// made up: 10, 5 and 2 ohms a kilometre are no real wire.
fn circuit() -> Circuit {
    Circuit {
        current_tenths_amp: 200,
        length_micrometres: 500 * PER_HUNDREDTH_METRE,
        system_millivolts: 12_000,
        allowed: DEFAULT_ALLOWED,
    }
}

#[test]
fn the_drop_is_current_times_resistance_times_length() {
    // 20 A × 0.010 Ω/m × 5 m = 1.000 V, which is 8.333% of 12 V.
    let thin = voltage_drop(&circuit(), from_ohms_per_km(100_000)).unwrap();
    assert_eq!(
        (thin.millivolts, thin.percent, thin.within),
        (1_000, Percent::from_thousandths(8_333), false)
    );
    // 20 A × 0.002 Ω/m × 5 m = 0.200 V, which is 1.667%.
    let thick = voltage_drop(&circuit(), from_ohms_per_km(20_000)).unwrap();
    assert_eq!(
        (thick.millivolts, thick.percent, thick.within),
        (200, Percent::from_thousandths(1_667), true)
    );
    // The same wire at 24 V drops the same volts and half the percent.
    let at_24 = Circuit {
        system_millivolts: 24_000,
        ..circuit()
    };
    let thin = voltage_drop(&at_24, from_ohms_per_km(100_000)).unwrap();
    assert_eq!(
        (thin.millivolts, thin.percent.to_string(), thin.within),
        (1_000, "4.167%".to_owned(), false)
    );
    assert_eq!(
        (volts(1_000), volts(236), volts(0)),
        ("1.000".to_owned(), "0.236".to_owned(), "0.000".to_owned())
    );
}

#[test]
fn feet_are_exact_and_the_drop_is_rounded_once() {
    // 15.5 A × 0.005 Ω/m × 3.048 m = 0.23622 V, shown as 0.236 V: 1.967% of 12 V.
    let feet = Circuit {
        current_tenths_amp: 155,
        length_micrometres: 1_000 * PER_HUNDREDTH_FOOT,
        ..circuit()
    };
    let drop = voltage_drop(&feet, from_ohms_per_km(50_000)).unwrap();
    assert_eq!(
        (drop.millivolts, drop.percent.to_string(), drop.within),
        (236, "1.967%".to_owned(), true)
    );
    let strict = Circuit {
        allowed: Percent::from_thousandths(1_900),
        ..feet
    };
    assert!(
        !voltage_drop(&strict, from_ohms_per_km(50_000))
            .unwrap()
            .within
    );
}

#[test]
fn a_drop_equal_to_what_is_allowed_is_within_it() {
    // 1.000 V of 12 V is 8.3333...%: over 8.333%, within 8.334%.
    let wire = from_ohms_per_km(100_000);
    let at = |thousandths| Circuit {
        allowed: Percent::from_thousandths(thousandths),
        ..circuit()
    };
    assert!(!voltage_drop(&at(8_333), wire).unwrap().within);
    assert!(voltage_drop(&at(8_334), wire).unwrap().within);
    // 7.2 A × 0.010 Ω/m × 5 m = 0.360 V, which is 3% of 12 V exactly.
    let exact = Circuit {
        current_tenths_amp: 72,
        ..circuit()
    };
    let drop = voltage_drop(&exact, wire).unwrap();
    assert_eq!(
        (drop.millivolts, drop.percent, drop.within),
        (360, Percent::whole(3), true)
    );
    assert!(
        voltage_drop(
            &Circuit {
                current_tenths_amp: i64::MAX,
                ..circuit()
            },
            i64::MAX
        )
        .is_err()
    );
}

#[test]
fn a_resistance_is_read_and_brought_to_nano_ohms_per_metre() {
    assert_eq!(parse_resistance("10"), Ok(100_000));
    assert_eq!(parse_resistance("3.0480"), Ok(30_480));
    assert_eq!(
        parse_resistance("0.00001"),
        Err(ParseError::TooManyDecimals(4))
    );
    assert!(
        parse_resistance("0").is_err()
            && parse_resistance("1001").is_err()
            && parse_resistance("-1").is_err()
    );
    assert_eq!(from_ohms_per_km(100_000), 10_000_000);
    // 3.048 Ω per 1000 ft is 0.010 Ω/m. 1 Ω per 1000 ft is 0.00328084 Ω/m.
    assert_eq!(from_ohms_per_1000_ft(30_480), 10_000_000);
    assert_eq!(from_ohms_per_1000_ft(10_000), 3_280_840);
}

#[test]
fn a_current_and_a_length_are_read() {
    assert_eq!(
        (
            parse_current("20"),
            parse_current("15.5"),
            parse_current("500")
        ),
        (Ok(200), Ok(155), Ok(5_000))
    );
    assert_eq!(parse_current("15.55"), Err(ParseError::TooManyDecimals(1)));
    assert_eq!(
        parse_current("0").unwrap_err().to_string(),
        "This must be from 0.1 to 500."
    );
    assert_eq!(
        parse_current("500.1").unwrap_err().to_string(),
        "This must be from 0.1 to 500."
    );
    assert_eq!(
        (
            parse_length("5"),
            parse_length("10.25"),
            parse_length("1,000")
        ),
        (Ok(500), Ok(1_025), Ok(100_000))
    );
    assert_eq!(
        parse_length("0").unwrap_err().to_string(),
        "This must be from 0.01 to 1,000."
    );
    assert_eq!(parse_length(""), Err(ParseError::Empty));
    assert_eq!(DEFAULT_ALLOWED, Percent::whole(3));
}
