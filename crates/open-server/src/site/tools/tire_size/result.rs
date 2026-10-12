//! What the tire size page shows for two sizes that were worked out.
//!
//! Every figure comes from `shop-math` and is written with its `Display`
//! or its length methods. The same tables and sentences are the worked
//! example of the Markdown version, so the two cannot disagree.

use shop_math::tire_size::{Length, TireSize, compare};

use crate::site::markdown::Table;

/// What the page says of what a tire size cannot tell.
pub const LIMITS: &str = "A tire's real diameter differs a little from its nominal size, by maker and by wear. This page does not say whether a size fits a vehicle, clears its body and suspension, or carries its load.";

/// The result, in the order the page shows it.
#[derive(Debug, Clone)]
pub struct Shown {
    /// The figures of each size, side by side.
    pub sizes: Table,
    /// The second size measured against the first.
    pub difference: Table,
    /// What the difference does to the speedometer.
    pub speedometer: String,
}

/// A length as the page writes it: `16.1 mm (0.63 in)`.
fn both(length: Length) -> String {
    format!("{} mm ({} in)", length.mm(), length.inches())
}

pub fn shown(first: &TireSize, second: &TireSize) -> Shown {
    let comparison = compare(first, second);
    let (one, two) = (comparison.first, comparison.second);
    let sizes = Table {
        caption: "The two sizes".to_owned(),
        head: vec!["Figure".to_owned(), first.to_string(), second.to_string()],
        rows: vec![
            vec![
                "Sidewall height".to_owned(),
                both(one.sidewall),
                both(two.sidewall),
            ],
            vec![
                "Overall diameter".to_owned(),
                both(one.diameter),
                both(two.diameter),
            ],
            vec![
                "Circumference".to_owned(),
                format!("{} mm", one.circumference),
                format!("{} mm", two.circumference),
            ],
            vec![
                "Revolutions per kilometre".to_owned(),
                one.revs_per_km.to_string(),
                two.revs_per_km.to_string(),
            ],
        ],
        prose: false,
    };
    let difference = Table {
        caption: "The second size against the first".to_owned(),
        head: vec!["Figure".to_owned(), "Amount".to_owned()],
        rows: vec![
            vec![
                "Difference in diameter".to_owned(),
                both(comparison.diameter_difference),
            ],
            vec![
                "Difference as a percent of the first diameter".to_owned(),
                comparison.diameter_change.to_string(),
            ],
            vec![
                "Change in ride height".to_owned(),
                both(comparison.ride_height_change),
            ],
            vec![
                "True speed when the speedometer reads 100 km/h".to_owned(),
                format!("{} km/h", comparison.actual_at_100),
            ],
        ],
        prose: false,
    };
    let speedometer = match comparison.diameter_difference.hundredths_mm().cmp(&0) {
        std::cmp::Ordering::Greater => {
            "With the second size fitted and the speedometer not corrected, the vehicle goes faster than the speedometer reads, and the odometer counts less distance than was driven."
        }
        std::cmp::Ordering::Less => {
            "With the second size fitted and the speedometer not corrected, the vehicle goes slower than the speedometer reads, and the odometer counts more distance than was driven."
        }
        std::cmp::Ordering::Equal => {
            "The two sizes have the same diameter, so the speedometer and the odometer read as before."
        }
    };
    Shown {
        sizes,
        difference,
        speedometer: speedometer.to_owned(),
    }
}

/// How the page works its figures out, in words and symbols. The page and
/// its Markdown version both show these lines.
pub fn formulas() -> Vec<String> {
    [
        "In 275/65R18, 275 is the width in millimetres, 65 is the sidewall height as a percent of the width, and 18 is the rim diameter in inches. The letters P and LT say what the tire is built for and change no figure here.",
        "Sidewall height = width \u{d7} aspect ratio \u{f7} 100.",
        "Overall diameter = rim diameter \u{d7} 25.4 + 2 \u{d7} sidewall height, in millimetres.",
        "Circumference = \u{3c0} \u{d7} overall diameter. Revolutions per kilometre = 1,000,000 \u{f7} circumference in millimetres.",
        "Difference as a percent = (second diameter \u{2212} first diameter) \u{f7} first diameter. Change in ride height = half the difference in diameter. True speed at a reading of 100 km/h = 100 \u{d7} second diameter \u{f7} first diameter.",
        "Each figure is worked from the exact diameter and rounded once, where it is shown.",
    ]
    .map(str::to_owned)
    .to_vec()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_example_is_shown_as_worked_by_hand() {
        let (first, second) = (
            TireSize::parse("275/65R18").unwrap(),
            TireSize::parse("LT285/70R17").unwrap(),
        );
        let rows = |table: &Table| -> Vec<String> {
            table.rows.iter().map(|row| row.join(" | ")).collect()
        };
        let example = shown(&first, &second);
        assert_eq!(example.sizes.head, ["Figure", "275/65R18", "LT285/70R17"]);
        assert_eq!(
            rows(&example.sizes),
            [
                "Sidewall height | 178.8 mm (7.04 in) | 199.5 mm (7.85 in)",
                "Overall diameter | 814.7 mm (32.07 in) | 830.8 mm (32.71 in)",
                "Circumference | 2559.5 mm | 2610.0 mm",
                "Revolutions per kilometre | 390.7 | 383.1",
            ]
        );
        assert_eq!(
            rows(&example.difference),
            [
                "Difference in diameter | 16.1 mm (0.63 in)",
                "Difference as a percent of the first diameter | 1.976%",
                "Change in ride height | 8.1 mm (0.32 in)",
                "True speed when the speedometer reads 100 km/h | 102.0 km/h",
            ]
        );
        assert!(
            example
                .speedometer
                .contains("goes faster than the speedometer reads")
        );
        let smaller = shown(&second, &first);
        assert!(
            smaller
                .speedometer
                .contains("goes slower than the speedometer reads")
        );
        assert_eq!(smaller.difference.rows[0][1], "-16.1 mm (-0.63 in)");
        assert!(
            shown(&first, &first)
                .speedometer
                .starts_with("The two sizes have the same diameter")
        );
        assert_eq!(formulas().len(), 6);
    }
}
