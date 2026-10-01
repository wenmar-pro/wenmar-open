use crate::vin::Vin;

/// Position 10 codes in year order, starting at 1980. I, O, Q, U, Z and 0 are
/// never used for a year.
const YEAR_CODES: &str = "ABCDEFGHJKLMNPRSTVWXY123456789";
const FIRST_YEAR: u16 = 1980;
const CYCLE: u16 = 30;

/// Model years this VIN could have, most likely first.
///
/// Empty when position 10 does not encode a year. Nothing later than
/// `current_year + 2` is returned, which is NHTSA's bound.
///
/// For light vehicles a digit in position 7 means the 1980 to 2009 cycle and
/// a letter the 2010 to 2039 cycle. Other vehicles do not follow that rule,
/// so the later cycle is tried first.
pub fn candidates(vin: &Vin, current_year: u16, light_vehicle: bool) -> Vec<u16> {
    let Some(index) = YEAR_CODES.find(vin.year_char()) else {
        return Vec::new();
    };
    let offset = u16::try_from(index).unwrap_or(0);
    let early = FIRST_YEAR + offset;
    let late = early + CYCLE;
    let latest = current_year.saturating_add(2);

    let early_first = light_vehicle && vin.char_at(7).is_ascii_digit();
    let ordered = if early_first {
        [early, late]
    } else {
        [late, early]
    };
    ordered.into_iter().filter(|year| *year <= latest).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn years(vin: &str, light_vehicle: bool) -> Vec<u16> {
        candidates(&Vin::parse(vin).unwrap(), 2026, light_vehicle)
    }

    #[test]
    fn light_vehicles_use_position_7_to_pick_the_cycle() {
        assert_eq!(years("KM8K2CAB4PU001140", true), vec![2023, 1993]);
        assert_eq!(years("1M8GDM9AXKP042788", true), vec![1989, 2019]);
    }

    #[test]
    fn other_vehicles_prefer_the_later_cycle_whatever_position_7_holds() {
        assert_eq!(years("1M8GDM9AXKP042788", false), vec![2019, 1989]);
        assert_eq!(years("KM8K2CAB4PU001140", false), vec![2023, 1993]);
    }

    #[test]
    fn years_more_than_two_ahead_are_dropped() {
        assert_eq!(years("1HGCM82633A004352", true), vec![2003]);
        assert_eq!(years("1FTFW1E50YFA00001", true), vec![2000]);
        assert_eq!(years("1FTFW1E50YFA00001", false), vec![2000]);
    }

    #[test]
    fn the_bound_is_the_current_year_plus_two() {
        let vin = Vin::parse("1FTFW1E50WFA00001").unwrap();
        assert_eq!(candidates(&vin, 2026, true), vec![2028, 1998]);
        assert_eq!(candidates(&vin, 2025, true), vec![1998]);
    }

    #[test]
    fn position_10_may_carry_no_year() {
        assert_eq!(years("KM8K2CAB40U001140", true), Vec::<u16>::new());
    }

    #[test]
    fn extreme_current_years_do_not_overflow() {
        let vin = Vin::parse("KM8K2CAB4PU001140").unwrap();
        assert_eq!(candidates(&vin, u16::MAX, true), vec![2023, 1993]);
        assert_eq!(candidates(&vin, 0, true), Vec::<u16>::new());
    }
}
