use shop_math::Percent;
use shop_math::tire_size::{Length, Prefix, TireSize, TireSizeError, compare, dimensions};

fn size(text: &str) -> TireSize {
    TireSize::parse(text).unwrap()
}

/// Sidewall mm, sidewall in, diameter mm, diameter in, circumference mm, revolutions per km.
fn measured(text: &str) -> [String; 6] {
    let d = dimensions(&size(text));
    [
        d.sidewall.mm(),
        d.sidewall.inches(),
        d.diameter.mm(),
        d.diameter.inches(),
        d.circumference.to_string(),
        d.revs_per_km.to_string(),
    ]
}

/// Diameter difference mm and in, percent, ride height mm and in, true speed at a reading of 100.
fn compared(first: &str, second: &str) -> [String; 6] {
    let c = compare(&size(first), &size(second));
    [
        c.diameter_difference.mm(),
        c.diameter_difference.inches(),
        c.diameter_change.to_string(),
        c.ride_height_change.mm(),
        c.ride_height_change.inches(),
        c.actual_at_100.to_string(),
    ]
}

#[test]
fn a_size_is_read_with_or_without_its_letters() {
    let plain = TireSize {
        prefix: Prefix::None,
        width_mm: 275,
        aspect: 65,
        rim_tenths_inch: 180,
    };
    for text in ["275/65R18", " 275/65 r18 ", "275/65ZR18", "275/65R18C"] {
        assert_eq!(TireSize::parse(text), Ok(plain), "{text}");
    }
    assert_eq!(size("P275/65R18").prefix, Prefix::Passenger);
    assert_eq!(size("lt275/65R18").prefix, Prefix::LightTruck);
    assert_eq!(size("275/65R18LT").prefix, Prefix::LightTruck);
    assert_eq!(size("225/70R19.5").rim_tenths_inch, 195);
    assert_eq!(size("lt285/70r17").to_string(), "LT285/70R17");
    assert_eq!(size("P215/65R15").to_string(), "P215/65R15");
    assert_eq!(size("225/70 R19.5").to_string(), "225/70R19.5");
}

#[test]
fn what_is_not_a_size_is_refused_with_a_message() {
    assert_eq!(TireSize::parse("  "), Err(TireSizeError::Empty));
    for text in [
        "275",
        "275/65R",
        "275-65R18",
        "2755/65R18",
        "275/655R18",
        "275/65R185",
        "275/65R18.0",
        "275/65D18",
        "35x12.50R17",
        "P275/65R18LT",
        "LT275/65R18C",
        "275/65R18 116T",
        "\u{ff12}75/65R18",
        "<b>",
    ] {
        assert_eq!(
            TireSize::parse(text),
            Err(TireSizeError::NotASize),
            "{text}"
        );
    }
    for (text, error, message) in [
        (
            "124/65R18",
            TireSizeError::Width,
            "The width must be from 125 to 455 millimetres.",
        ),
        (
            "456/65R18",
            TireSizeError::Width,
            "The width must be from 125 to 455 millimetres.",
        ),
        (
            "275/19R18",
            TireSizeError::Aspect,
            "The aspect ratio must be from 20 to 95.",
        ),
        (
            "275/96R18",
            TireSizeError::Aspect,
            "The aspect ratio must be from 20 to 95.",
        ),
        (
            "275/65R09",
            TireSizeError::Rim,
            "The rim must be from 10 to 30 inches.",
        ),
        (
            "275/65R30.5",
            TireSizeError::Rim,
            "The rim must be from 10 to 30 inches.",
        ),
    ] {
        assert_eq!(TireSize::parse(text), Err(error), "{text}");
        assert_eq!(error.to_string(), message);
    }
    assert!(TireSize::parse("125/20R10").is_ok() && TireSize::parse("455/95R30").is_ok());
    assert_eq!(TireSizeError::Empty.to_string(), "Enter a tire size.");
    assert_eq!(
        TireSizeError::NotASize.to_string(),
        "This is not a tire size this page reads. Write it like 275/65R18 or LT285/70R17."
    );
}

#[test]
fn a_size_is_measured() {
    // 275 × 65 ÷ 100 = 178.75 mm of sidewall. 18 × 25.4 = 457.2 mm of rim.
    // 457.2 + 2 × 178.75 = 814.7 mm. π × 814.7 = 2559.4555 mm. 1,000,000 ÷ 2559.4555 = 390.71.
    assert_eq!(
        measured("275/65R18"),
        ["178.8", "7.04", "814.7", "32.07", "2559.5", "390.7"]
    );
    assert_eq!(
        measured("LT285/70R17"),
        ["199.5", "7.85", "830.8", "32.71", "2610.0", "383.1"]
    );
    assert_eq!(
        measured("205/55R16"),
        ["112.8", "4.44", "631.9", "24.88", "1985.2", "503.7"]
    );
    assert_eq!(
        measured("225/45R17"),
        ["101.3", "3.99", "634.3", "24.97", "1992.7", "501.8"]
    );
    assert_eq!(
        measured("235/75R15"),
        ["176.3", "6.94", "733.5", "28.88", "2304.4", "434.0"]
    );
    // A rim of 19.5 inches is 495.3 mm.
    assert_eq!(
        measured("225/70R19.5"),
        ["157.5", "6.20", "810.3", "31.90", "2545.6", "392.8"]
    );
    // The smallest and the largest size read.
    assert_eq!(
        measured("125/20R10"),
        ["25.0", "0.98", "304.0", "11.97", "955.0", "1047.1"]
    );
    assert_eq!(
        measured("455/95R30"),
        ["432.3", "17.02", "1626.5", "64.04", "5109.8", "195.7"]
    );
    let exact = dimensions(&size("275/65R18"));
    assert_eq!(exact.sidewall, Length::from_hundredths_mm(17_875));
    assert_eq!(exact.diameter.hundredths_mm(), 81_470);
    assert_eq!(exact.circumference.tenths(), 25_595);
    assert_eq!(exact.revs_per_km.tenths(), 3_907);
}

#[test]
fn two_sizes_are_compared() {
    // 830.8 − 814.7 = 16.1 mm, which is 1.976% of 814.7. Half of it, 8.05 mm, is the
    // change in ride height. 100 × 830.8 ÷ 814.7 = 101.98 km/h.
    assert_eq!(
        compared("275/65R18", "LT285/70R17"),
        ["16.1", "0.63", "1.976%", "8.1", "0.32", "102.0"]
    );
    assert_eq!(
        compared("205/55R16", "225/45R17"),
        ["2.4", "0.09", "0.38%", "1.2", "0.05", "100.4"]
    );
    assert_eq!(
        compared("265/70R17", "275/65R18"),
        ["11.9", "0.47", "1.482%", "6.0", "0.23", "101.5"]
    );
    assert_eq!(
        compared("LT285/70R17", "205/55R16"),
        ["-198.9", "-7.83", "-23.941%", "-99.5", "-3.92", "76.1"]
    );
    assert_eq!(
        compared("275/65R18", "P275/65R18"),
        ["0.0", "0.00", "0%", "0.0", "0.00", "100.0"]
    );
    let larger = compare(&size("275/65R18"), &size("LT285/70R17"));
    assert_eq!(larger.diameter_change, Percent::from_thousandths(1_976));
    assert_eq!(larger.ride_height_change.hundredths_mm(), 805);
    assert_eq!(larger.first, dimensions(&size("275/65R18")));
    assert_eq!(larger.second, dimensions(&size("285/70R17")));
}
