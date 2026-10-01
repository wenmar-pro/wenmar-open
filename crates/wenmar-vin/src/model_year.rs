use crate::vin::Vin;

/// Position 10 codes in year order, starting at 1980. I, O, Q, U, Z and 0 are
/// never used for a year.
const YEAR_CODES: &str = "ABCDEFGHJKLMNPRSTVWXY123456789";
const FIRST_YEAR: u16 = 1980;
const CYCLE: u16 = 30;

/// Model years this VIN could have, most likely first.
///
/// Empty when position 10 does not encode a year. `current_year` bounds the
/// answer: nothing later than next year is returned.
pub fn candidates(vin: &Vin, current_year: u16) -> Vec<u16> {
    let Some(index) = YEAR_CODES.find(vin.year_char()) else {
        return Vec::new();
    };
    let offset = u16::try_from(index).unwrap_or(0);
    let early = FIRST_YEAR + offset;
    let late = early + CYCLE;
    let latest = current_year.saturating_add(1);

    let ordered = if vin.char_at(7).is_ascii_digit() {
        [early, late]
    } else {
        [late, early]
    };
    ordered.into_iter().filter(|year| *year <= latest).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn years(vin: &str) -> Vec<u16> {
        candidates(&Vin::parse(vin).unwrap(), 2026)
    }

    #[test]
    fn a_letter_in_position_7_prefers_the_later_cycle() {
        assert_eq!(years("KM8K2CAB4PU001140"), vec![2023, 1993]);
    }

    #[test]
    fn a_digit_in_position_7_prefers_the_earlier_cycle() {
        assert_eq!(years("1M8GDM9AXKP042788"), vec![1989, 2019]);
    }

    #[test]
    fn years_that_have_not_happened_yet_are_dropped() {
        assert_eq!(years("1HGCM82633A004352"), vec![2003]);
        assert_eq!(years("1FTFW1E50YFA00001"), vec![2000]);
    }

    #[test]
    fn next_model_year_is_allowed() {
        let vin = Vin::parse("1FTFW1E50VFA00001").unwrap();
        assert_eq!(candidates(&vin, 2026), vec![2027, 1997]);
        assert_eq!(candidates(&vin, 2025), vec![1997]);
    }

    #[test]
    fn position_10_may_carry_no_year() {
        assert_eq!(years("KM8K2CAB40U001140"), Vec::<u16>::new());
    }

    #[test]
    fn extreme_current_years_do_not_overflow() {
        let vin = Vin::parse("KM8K2CAB4PU001140").unwrap();
        assert_eq!(candidates(&vin, u16::MAX), vec![2023, 1993]);
        assert_eq!(candidates(&vin, 0), Vec::<u16>::new());
    }
}
