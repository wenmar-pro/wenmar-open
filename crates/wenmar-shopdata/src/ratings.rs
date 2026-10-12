//! NHTSA's crash test ratings for a model year.
//!
//! NHTSA rates a vehicle by body style and drive, so one model year often
//! has several ratings, and they can differ. Every rating is given with
//! NHTSA's own description of the vehicle it tested; none is picked.

use serde::Serialize;
use wenmar_vehicles::{Source, Value};

use crate::dataset::Dataset;
use crate::error::ShopError;
use crate::shop::ShopData;

/// The stars of one rating, each from 1 to 5. `None` where NHTSA gave none.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize)]
pub struct RatingStars {
    pub overall: Option<u8>,
    /// The frontal crash test: overall, then the driver and the passenger.
    pub frontal: Option<u8>,
    pub frontal_driver: Option<u8>,
    pub frontal_passenger: Option<u8>,
    /// The side crash tests: overall, then the driver and the rear
    /// passenger, the barrier test, the front and the rear seat over both
    /// tests, and the pole test.
    pub side: Option<u8>,
    pub side_driver: Option<u8>,
    pub side_passenger: Option<u8>,
    pub side_barrier: Option<u8>,
    pub side_front_seat: Option<u8>,
    pub side_rear_seat: Option<u8>,
    pub side_pole: Option<u8>,
    pub rollover: Option<u8>,
}

impl RatingStars {
    /// Whether NHTSA gave any star.
    pub fn any(&self) -> bool {
        [
            self.overall,
            self.frontal,
            self.frontal_driver,
            self.frontal_passenger,
            self.side,
            self.side_driver,
            self.side_passenger,
            self.side_barrier,
            self.side_front_seat,
            self.side_rear_seat,
            self.side_pole,
            self.rollover,
        ]
        .iter()
        .any(Option::is_some)
    }
}

/// The safety features NHTSA lists with a rating. A feature is `standard`,
/// `optional`, `both` (standard on some versions and optional on others)
/// or `no`. An evaluation is `meets`, `does_not_meet`, `standard_only`,
/// `upgrade_only` or `pending`. `None` where NHTSA's file does not say.
///
/// For the four driver-assistance features, NHTSA's file means two things
/// by its years. Through model year 2022, `standard`, `optional` and
/// `both` say the feature is fitted *and* met NHTSA's performance
/// criteria, and `no` says it is not fitted or did not meet them; there is
/// no evaluation. From model year 2023 the value says only whether the
/// feature is fitted, and the evaluation says what NHTSA's test found.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
pub struct RatingFeatures {
    /// Electronic stability control.
    pub esc: Option<String>,
    pub forward_collision_warning: Option<String>,
    pub forward_collision_warning_evaluation: Option<String>,
    pub lane_departure_warning: Option<String>,
    pub lane_departure_warning_evaluation: Option<String>,
    /// Automatic emergency braking: the vehicle brakes by itself.
    pub crash_imminent_braking: Option<String>,
    pub crash_imminent_braking_evaluation: Option<String>,
    /// The vehicle adds to the driver's braking.
    pub dynamic_brake_support: Option<String>,
    pub dynamic_brake_support_evaluation: Option<String>,
    pub backup_camera: Option<String>,
    pub blind_spot_detection: Option<String>,
    /// Side air bags for the head, and for the torso.
    pub head_side_airbag: Option<String>,
    pub torso_side_airbag: Option<String>,
}

/// One rating that NHTSA gave a vehicle of a model year.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct VehicleRating {
    /// The model, the body style, the vehicle type and the drive as NHTSA
    /// words them: `F-150 SUPER CREW`, `PU/CC`, `TRUCK`, `4x4`.
    pub model: String,
    pub body_style: Option<String>,
    pub vehicle_type: Option<String>,
    pub drive: Option<String>,
    /// 1 for the first release of a model year; 2 and 3 for a later one
    /// that NHTSA rated again.
    pub release: u8,
    /// How NHTSA's model name reached this model: `exact`, `base` (NHTSA
    /// names a body or a version of it) or `curated`.
    #[serde(rename = "match")]
    pub matched: String,
    pub stars: RatingStars,
    /// The chance of a rollover in a single-vehicle crash, in thousandths:
    /// 191 is 19.1%.
    pub rollover_risk_permille: Option<u32>,
    /// Whether the vehicle tipped up in NHTSA's dynamic test.
    pub rollover_tip: Option<bool>,
    /// The weight of the vehicle NHTSA tested, in pounds.
    pub curb_weight_lb: Option<u32>,
    pub features: RatingFeatures,
    /// NHTSA's safety concerns and footnotes: what happened in a test, and
    /// which vehicles the rating does not cover.
    pub notes: Vec<String>,
}

impl VehicleRating {
    /// The vehicle NHTSA rated, in NHTSA's words, on one line:
    /// `F-150 SUPER CREW, PU/CC, 4x4`. A later release says so.
    pub fn description(&self) -> String {
        let mut text = self.model.clone();
        for part in [&self.body_style, &self.drive].into_iter().flatten() {
            text.push_str(", ");
            text.push_str(part);
        }
        match self.release {
            0 | 1 => {}
            2 => text.push_str(", later release"),
            _ => text.push_str(", latest release"),
        }
        text
    }
}

/// The ratings of one model year. `?1` make id, `?2` model id, `?3` year.
pub const RATINGS_FOR_VEHICLE: &str = "
SELECT r.model_text, r.body_style, r.vehicle_type, r.drive, r.release, v.\"match\",
       r.overall, r.frontal, r.frontal_driver, r.frontal_passenger,
       r.side, r.side_driver, r.side_passenger, r.side_barrier, r.side_front_seat, r.side_rear_seat, r.side_pole,
       r.rollover, r.rollover_risk_permille, r.rollover_tip, r.curb_weight_lb,
       r.esc, r.fcw, r.fcw_evaluation, r.ldw, r.ldw_evaluation, r.cib, r.cib_evaluation, r.dbs, r.dbs_evaluation,
       r.backup_camera, r.blind_spot_detection, r.head_side_airbag, r.torso_side_airbag, r.notes
FROM rating_vehicle v
JOIN rating r ON r.id = v.rating_id
WHERE v.make_id = ?1 AND v.model_id = ?2 AND v.year = ?3
ORDER BY r.model_text, r.body_style, r.drive, r.release, r.id";

const TABLE: &str = "rating";

fn text(row: &[Value], column: usize) -> Option<String> {
    row.get(column).and_then(Value::text).map(str::to_owned)
}

fn integer(row: &[Value], column: usize) -> Option<i64> {
    row.get(column).and_then(Value::integer)
}

fn stars(row: &[Value], column: usize) -> Option<u8> {
    integer(row, column)
        .and_then(|stars| u8::try_from(stars).ok())
        .filter(|stars| (1..=5).contains(stars))
}

impl<S: Source> ShopData<S> {
    /// The day the ratings were current, as `2026-08-07`. `None` when the
    /// file does not hold them.
    pub fn ratings_as_of(&self) -> Option<&str> {
        self.datasets()
            .get(Dataset::Ratings)
            .map(|entry| entry.as_of.as_str())
    }

    /// Every rating NHTSA gave a vehicle of one model year, by NHTSA's
    /// model name, body style and drive. Empty when NHTSA lists none.
    pub fn ratings_for_vehicle(
        &self,
        make_id: i64,
        model_id: i64,
        year: u16,
    ) -> Result<Vec<VehicleRating>, ShopError> {
        self.require(Dataset::Ratings)?;
        let rows = self.query(
            RATINGS_FOR_VEHICLE,
            &[make_id.into(), model_id.into(), i64::from(year).into()],
        )?;
        let mut found = Vec::with_capacity(rows.len());
        for row in &rows {
            found.push(VehicleRating {
                model: text(row, 0).ok_or(ShopError::Shape(TABLE))?,
                body_style: text(row, 1),
                vehicle_type: text(row, 2),
                drive: text(row, 3),
                release: integer(row, 4)
                    .and_then(|release| u8::try_from(release).ok())
                    .ok_or(ShopError::Shape(TABLE))?,
                matched: text(row, 5).ok_or(ShopError::Shape(TABLE))?,
                stars: RatingStars {
                    overall: stars(row, 6),
                    frontal: stars(row, 7),
                    frontal_driver: stars(row, 8),
                    frontal_passenger: stars(row, 9),
                    side: stars(row, 10),
                    side_driver: stars(row, 11),
                    side_passenger: stars(row, 12),
                    side_barrier: stars(row, 13),
                    side_front_seat: stars(row, 14),
                    side_rear_seat: stars(row, 15),
                    side_pole: stars(row, 16),
                    rollover: stars(row, 17),
                },
                rollover_risk_permille: integer(row, 18)
                    .and_then(|share| u32::try_from(share).ok()),
                rollover_tip: integer(row, 19).map(|tip| tip != 0),
                curb_weight_lb: integer(row, 20).and_then(|pounds| u32::try_from(pounds).ok()),
                features: RatingFeatures {
                    esc: text(row, 21),
                    forward_collision_warning: text(row, 22),
                    forward_collision_warning_evaluation: text(row, 23),
                    lane_departure_warning: text(row, 24),
                    lane_departure_warning_evaluation: text(row, 25),
                    crash_imminent_braking: text(row, 26),
                    crash_imminent_braking_evaluation: text(row, 27),
                    dynamic_brake_support: text(row, 28),
                    dynamic_brake_support_evaluation: text(row, 29),
                    backup_camera: text(row, 30),
                    blind_spot_detection: text(row, 31),
                    head_side_airbag: text(row, 32),
                    torso_side_airbag: text(row, 33),
                },
                notes: text(row, 34)
                    .map(|notes| {
                        notes
                            .lines()
                            .map(str::trim)
                            .filter(|note| !note.is_empty())
                            .map(str::to_owned)
                            .collect()
                    })
                    .unwrap_or_default(),
            });
        }
        Ok(found)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rating(body_style: Option<&str>, drive: Option<&str>, release: u8) -> VehicleRating {
        VehicleRating {
            model: "F-150 SUPER CREW".to_owned(),
            body_style: body_style.map(str::to_owned),
            vehicle_type: Some("TRUCK".to_owned()),
            drive: drive.map(str::to_owned),
            release,
            matched: "base".to_owned(),
            stars: RatingStars::default(),
            rollover_risk_permille: None,
            rollover_tip: None,
            curb_weight_lb: None,
            features: RatingFeatures::default(),
            notes: Vec::new(),
        }
    }

    #[test]
    fn a_rating_is_described_in_nhtsas_words() {
        assert_eq!(
            rating(Some("PU/CC"), Some("4x4"), 1).description(),
            "F-150 SUPER CREW, PU/CC, 4x4"
        );
        assert_eq!(
            rating(Some("PU/CC"), None, 1).description(),
            "F-150 SUPER CREW, PU/CC"
        );
        assert_eq!(rating(None, None, 1).description(), "F-150 SUPER CREW");
        assert_eq!(
            rating(None, Some("AWD"), 2).description(),
            "F-150 SUPER CREW, AWD, later release"
        );
        assert_eq!(
            rating(None, None, 3).description(),
            "F-150 SUPER CREW, latest release"
        );
    }

    #[test]
    fn any_star_counts_as_rated() {
        assert!(!RatingStars::default().any());
        assert!(
            RatingStars {
                rollover: Some(4),
                ..RatingStars::default()
            }
            .any()
        );
        assert!(
            RatingStars {
                side_passenger: Some(3),
                ..RatingStars::default()
            }
            .any()
        );
    }
}
